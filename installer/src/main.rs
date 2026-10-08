//! Pious Setup: installs Pious into a folder of its own (with FFmpeg next
//! to it), keeps it updated, and uninstalls it.
//!
//!   pious-setup.exe                          install (or reinstall)
//!   pious-setup.exe --update --dir D --wait P  update the copy in D once
//!                                            process P (the old Pious) exits
//!   uninstall.exe                            uninstall (copied into the
//!                                            install folder by the installer)
//!
//! Installs are per user (no administrator rights): files go to
//! %LOCALAPPDATA%\Programs\Pious by default, with Start menu and desktop
//! shortcuts and an entry in Windows' installed apps.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

/// The app, zipped: pious.exe, ffmpeg.exe and friends.
static PAYLOAD: &[u8] = include_bytes!(env!("PIOUS_PAYLOAD_FILE"));
const VERSION: &str = env!("PIOUS_VERSION");
/// This version's changes, one per line (from CHANGELOG.md).
const NOTES: &str = include_str!(concat!(env!("OUT_DIR"), "/notes.txt"));
const APP_EXE: &str = "pious.exe";
const UNINSTALLER: &str = "uninstall.exe";
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Pious";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Install,
    Update,
    Uninstall,
}

struct Options {
    mode: Mode,
    dir: Option<PathBuf>,
    wait: Option<u32>,
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mode = if args.iter().any(|a| a == "--update") {
        Mode::Update
    } else if args.iter().any(|a| a == "--uninstall") || exe_name == UNINSTALLER {
        Mode::Uninstall
    } else {
        Mode::Install
    };
    Options { mode, dir: value("--dir").map(PathBuf::from), wait: value("--wait").and_then(|p| p.parse().ok()) }
}

fn default_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\")).join("Programs").join("Pious")
}

/// Where Pious is installed now, according to Windows' app list.
fn installed_dir() -> Option<PathBuf> {
    registry::read(UNINSTALL_KEY, "InstallLocation").map(PathBuf::from).filter(|d| d.join(APP_EXE).is_file())
}

fn payload_size() -> u64 {
    zip::ZipArchive::new(std::io::Cursor::new(PAYLOAD))
        .map(|mut z| (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.size())).sum())
        .unwrap_or(0)
}

// ── Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
fn info() -> serde_json::Value {
    let opts = options();
    let existing = installed_dir();
    let dir = opts.dir.clone().or(existing.clone()).unwrap_or_else(default_dir);
    json!({
        "mode": opts.mode,
        "version": VERSION,
        "dir": dir,
        "existing": existing,
        "existing_version": registry::read(UNINSTALL_KEY, "DisplayVersion"),
        "size": payload_size(),
        "empty": PAYLOAD.len() < 100,
        "elsewhere": processes::running_elsewhere(&dir),
        "notes": NOTES.lines().filter(|l| !l.is_empty()).collect::<Vec<_>>(),
    })
}

#[tauri::command]
async fn pick_dir(current: String) -> Option<PathBuf> {
    let start = PathBuf::from(&current);
    let parent = start.parent().map(Path::to_path_buf).unwrap_or(start);
    rfd::AsyncFileDialog::new()
        .set_title("Choose where to install Pious")
        .set_directory(parent)
        .pick_folder()
        .await
        .map(|f| {
            let picked = f.path().to_path_buf();
            // A folder of its own inside what was picked.
            if picked.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("pious")) {
                picked
            } else {
                picked.join("Pious")
            }
        })
}

fn progress(app: &AppHandle, stage: &str, done: u64, total: u64) {
    let _ = app.emit("progress", json!({ "stage": stage, "done": done, "total": total }));
}

#[tauri::command]
async fn install(app: AppHandle, dir: PathBuf, desktop: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install_now(&app, &dir, desktop, None))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn update(app: AppHandle) -> Result<(), String> {
    let opts = options();
    let dir = opts.dir.or_else(installed_dir).ok_or("Pious's folder wasn't given.")?;
    tauri::async_runtime::spawn_blocking(move || {
        install_now(&app, &dir, false, opts.wait)?;
        launch_app(&dir, true)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn install_now(app: &AppHandle, dir: &Path, desktop: bool, wait: Option<u32>) -> Result<(), String> {
    if PAYLOAD.len() < 100 {
        return Err("This installer was built without Pious inside it.".into());
    }
    // The old Pious is closing for an update: give it a moment.
    if let Some(pid) = wait {
        progress(app, "Waiting for Pious to close", 0, 1);
        for _ in 0..150 {
            if !processes::alive(pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    progress(app, "Closing Pious", 0, 1);
    processes::close_in(dir);

    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {} ({e}).", dir.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(PAYLOAD)).map_err(|e| e.to_string())?;
    let total: u64 = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.size())).sum();
    let mut done = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(relative) = entry.enclosed_name() else { continue };
        let target = dir.join(&relative);
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&target);
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let name = relative.display().to_string();
        let partial = target.with_extension("part");
        let mut out = std::fs::File::create(&partial).map_err(|e| format!("Couldn't write {name} ({e})."))?;
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let n = entry.read(&mut buffer).map_err(|e| format!("The installer is damaged ({e})."))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut out, &buffer[..n]).map_err(|e| format!("Couldn't write {name} ({e})."))?;
            done += n as u64;
            progress(app, &format!("Copying {name}"), done, total);
        }
        drop(out);
        // A file still in use: move it aside so the new one can go in.
        if std::fs::rename(&partial, &target).is_err() {
            let aside = target.with_extension("old");
            let _ = std::fs::remove_file(&aside);
            let _ = std::fs::rename(&target, &aside);
            std::fs::rename(&partial, &target).map_err(|e| format!("Couldn't replace {name} ({e}). Close Pious and try again."))?;
        }
    }

    progress(app, "Finishing", total, total);
    // The uninstaller is this program under another name.
    if let Ok(me) = std::env::current_exe() {
        let uninstaller = dir.join(UNINSTALLER);
        if me != uninstaller {
            let _ = std::fs::copy(&me, &uninstaller);
        }
    }
    let exe = dir.join(APP_EXE);
    let start_menu = dirs::data_dir().map(|d| d.join("Microsoft\\Windows\\Start Menu\\Programs\\Pious.lnk"));
    if let Some(link) = &start_menu {
        shortcut(link, &exe, dir);
    }
    if desktop {
        if let Some(link) = dirs::desktop_dir().map(|d| d.join("Pious.lnk")) {
            shortcut(&link, &exe, dir);
        }
    }
    let size_kb = (total / 1024) as u32;
    let exe_text = exe.display().to_string();
    let dir_text = dir.display().to_string();
    let uninstall = format!("\"{}\"", dir.join(UNINSTALLER).display());
    registry::write(UNINSTALL_KEY, "DisplayName", "Pious");
    registry::write(UNINSTALL_KEY, "DisplayVersion", VERSION);
    registry::write(UNINSTALL_KEY, "Publisher", "Pious");
    registry::write(UNINSTALL_KEY, "DisplayIcon", &exe_text);
    registry::write(UNINSTALL_KEY, "InstallLocation", &dir_text);
    registry::write(UNINSTALL_KEY, "UninstallString", &uninstall);
    registry::write(UNINSTALL_KEY, "QuietUninstallString", &uninstall);
    registry::write(UNINSTALL_KEY, "URLInfoAbout", "https://github.com/woogi999/pious-bootstrapper");
    registry::write_dword(UNINSTALL_KEY, "EstimatedSize", size_kb);
    registry::write_dword(UNINSTALL_KEY, "NoModify", 1);
    registry::write_dword(UNINSTALL_KEY, "NoRepair", 1);
    // Starting with Windows points at the copy just installed.
    if registry::read(RUN_KEY, "Pious").is_some() {
        registry::write(RUN_KEY, "Pious", &format!("\"{exe_text}\" --background"));
    }
    progress(app, "Done", total, total);
    Ok(())
}

/// Makes a shortcut through Windows' own scripting (no extra files).
fn shortcut(link: &Path, target: &Path, dir: &Path) {
    let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:PIOUS_LINK); \
                  $s.TargetPath = $env:PIOUS_TARGET; $s.WorkingDirectory = $env:PIOUS_DIR; \
                  $s.IconLocation = $env:PIOUS_TARGET + ',0'; $s.Description = 'Pious'; $s.Save()";
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .env("PIOUS_LINK", link)
        .env("PIOUS_TARGET", target)
        .env("PIOUS_DIR", dir);
    hide(&mut command);
    let _ = command.status();
}

fn hide(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

fn launch_app(dir: &Path, after_update: bool) -> Result<(), String> {
    let mut command = std::process::Command::new(dir.join(APP_EXE));
    command.current_dir(dir);
    if after_update {
        command.arg("--after-update");
    }
    command.spawn().map(|_| ()).map_err(|e| format!("Couldn't open Pious ({e})."))
}

#[tauri::command]
fn open_pious(dir: PathBuf) -> Result<(), String> {
    launch_app(&dir, false)
}

#[tauri::command]
async fn uninstall(app: AppHandle, remove_data: bool) -> Result<(), String> {
    let dir = options().dir.or_else(installed_dir).ok_or("Pious doesn't seem to be installed.")?;
    tauri::async_runtime::spawn_blocking(move || {
        progress(&app, "Closing Pious", 0, 4);
        processes::close_in(&dir);
        progress(&app, "Removing shortcuts", 1, 4);
        for link in [
            dirs::data_dir().map(|d| d.join("Microsoft\\Windows\\Start Menu\\Programs\\Pious.lnk")),
            dirs::desktop_dir().map(|d| d.join("Pious.lnk")),
        ]
        .into_iter()
        .flatten()
        {
            let _ = std::fs::remove_file(link);
        }
        registry::delete_tree(UNINSTALL_KEY);
        registry::delete_value(RUN_KEY, "Pious");
        progress(&app, "Removing files", 2, 4);
        // Only what Pious put there, in case the folder holds anything else.
        remove_installed_files(&dir);
        if remove_data {
            progress(&app, "Removing your data", 3, 4);
            if let Some(data) = dirs::data_local_dir().map(|d| d.join("Pious")) {
                let _ = std::fs::remove_dir_all(data);
            }
        }
        progress(&app, "Done", 4, 4);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes the files the installer put in `dir` (and their leftovers from
/// updates), then the folder itself only if nothing else is in it.
fn remove_installed_files(dir: &Path) {
    let mut names: Vec<PathBuf> = vec![APP_EXE.into(), "ffmpeg.exe".into(), UNINSTALLER.into()];
    if let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(PAYLOAD)) {
        for i in 0..zip.len() {
            if let Some(relative) = zip.by_index(i).ok().filter(|e| !e.is_dir()).and_then(|e| e.enclosed_name()) {
                names.push(relative);
            }
        }
    }
    for name in names {
        let target = dir.join(name);
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(target.with_extension("old"));
        let _ = std::fs::remove_file(target.with_extension("part"));
    }
    let _ = std::fs::remove_file(dir.join("pious.old.exe"));
    let _ = std::fs::remove_dir(dir);
}

#[tauri::command]
fn quit(app: AppHandle) {
    // A copy of the uninstaller running from the temp folder removes itself.
    if options().mode == Mode::Uninstall {
        if let Ok(me) = std::env::current_exe() {
            if me.starts_with(std::env::temp_dir()) {
                let mut command = std::process::Command::new("cmd.exe");
                command.args(["/C", "ping 127.0.0.1 -n 3 > nul & del /f /q"]).arg(&me);
                hide(&mut command);
                let _ = command.spawn();
            }
        }
    }
    app.exit(0);
}

fn main() {
    let opts = options();
    // The uninstaller can't delete the folder it runs from: run a copy
    // from the temp folder instead.
    if opts.mode == Mode::Uninstall {
        if let Ok(me) = std::env::current_exe() {
            if !me.starts_with(std::env::temp_dir()) {
                let dir = me.parent().map(Path::to_path_buf).or_else(installed_dir).unwrap_or_else(default_dir);
                let copy = std::env::temp_dir().join(format!("pious-uninstall-{}.exe", std::process::id()));
                if std::fs::copy(&me, &copy).is_ok() {
                    let _ = std::process::Command::new(&copy).arg("--uninstall").arg("--dir").arg(&dir).spawn();
                    return;
                }
            }
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window_vibrancy::apply_acrylic(&window, Some((12, 12, 14, 150)));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![info, pick_dir, install, update, open_pious, uninstall, quit])
        .run(tauri::generate_context!())
        .expect("error while running Pious Setup");
}

mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteKeyValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open(path: &str) -> Option<HKEY> {
        let mut key: HKEY = std::ptr::null_mut();
        let ok = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE | KEY_READ,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        (ok == 0).then_some(key)
    }

    pub fn write(path: &str, name: &str, value: &str) {
        let Some(key) = open(path) else { return };
        let data = wide(value);
        unsafe {
            RegSetValueExW(key, wide(name).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32);
            RegCloseKey(key);
        }
    }

    pub fn write_dword(path: &str, name: &str, value: u32) {
        let Some(key) = open(path) else { return };
        unsafe {
            RegSetValueExW(key, wide(name).as_ptr(), 0, REG_DWORD, &value as *const u32 as *const u8, 4);
            RegCloseKey(key);
        }
    }

    pub fn read(path: &str, name: &str) -> Option<String> {
        let mut key: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return None;
        }
        let mut buffer = vec![0u16; 1024];
        let mut size = (buffer.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut _,
                &mut size,
            )
        };
        unsafe { RegCloseKey(key) };
        if ok != 0 {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..len]))
    }

    pub fn delete_tree(path: &str) {
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr());
            let (parent, leaf) = path.rsplit_once('\\').unwrap_or(("", path));
            let mut key: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, wide(parent).as_ptr(), 0, KEY_WRITE, &mut key) == 0 {
                windows_sys::Win32::System::Registry::RegDeleteKeyW(key, wide(leaf).as_ptr());
                RegCloseKey(key);
            }
        }
    }

    pub fn delete_value(path: &str, name: &str) {
        unsafe {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(path).as_ptr(), wide(name).as_ptr());
        }
    }
}

mod processes {
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        QueryFullProcessImageNameW, TerminateProcess,
    };

    /// Every running pious.exe: (process ID, full path).
    fn pious() -> Vec<(u32, std::path::PathBuf)> {
        let mut out = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(super::APP_EXE) {
                    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    if !process.is_null() {
                        let mut buffer = vec![0u16; 1024];
                        let mut size = buffer.len() as u32;
                        if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size) != 0 {
                            out.push((entry.th32ProcessID, String::from_utf16_lossy(&buffer[..size as usize]).into()));
                        }
                        CloseHandle(process);
                    }
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        out
    }

    fn same_folder(exe: &Path, dir: &Path) -> bool {
        exe.parent()
            .map(|p| p.to_string_lossy().to_lowercase().trim_end_matches('\\').to_owned())
            == Some(dir.to_string_lossy().to_lowercase().trim_end_matches('\\').to_owned())
    }

    /// A copy of Pious running from somewhere else (it would keep the new
    /// one from opening).
    pub fn running_elsewhere(dir: &Path) -> bool {
        pious().iter().any(|(_, exe)| !same_folder(exe, dir))
    }

    /// Closes the Pious running from `dir`, if any.
    pub fn close_in(dir: &Path) {
        for (pid, exe) in pious() {
            if same_folder(&exe, dir) {
                unsafe {
                    let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
                    if !process.is_null() {
                        TerminateProcess(process, 0);
                        CloseHandle(process);
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            ok != 0 && code == 259
        }
    }
}

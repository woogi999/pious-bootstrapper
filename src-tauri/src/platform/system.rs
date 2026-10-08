//! Small pieces of system integration that don't need a window: the
//! per-user registry (which program opens Roblox links) and making sure
//! only one copy of Pious runs at a time.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteTreeW, RegGetValueW, RegSetKeyValueW,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Reads a string value under `HKEY_CURRENT_USER`. An empty `name`
    /// reads the key's default value.
    pub fn read_user_value(key: &str, name: &str) -> Option<String> {
        let key = wide(key);
        let name = wide(name);
        let mut size = 0u32;
        unsafe {
            let status = RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            );
            if status != 0 || size == 0 {
                return None;
            }
            let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
            let status = RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            );
            if status != 0 {
                return None;
            }
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Some(String::from_utf16_lossy(&buffer[..len]))
        }
    }

    /// Writes a string value under `HKEY_CURRENT_USER`, creating the key.
    pub fn write_user_value(key: &str, name: &str, value: &str) -> Result<(), String> {
        let key = wide(key);
        let name = wide(name);
        let data = wide(value);
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if status == 0 { Ok(()) } else { Err(format!("Windows refused to update the registry (error {status}).")) }
    }

    /// Deletes one value from a key under `HKEY_CURRENT_USER`.
    pub fn delete_user_value(key: &str, name: &str) {
        use windows_sys::Win32::System::Registry::RegDeleteKeyValueW;
        let key = wide(key);
        let name = wide(name);
        unsafe {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr());
        }
    }

    /// Deletes a key and everything under it from `HKEY_CURRENT_USER`.
    pub fn delete_user_key(key: &str) {
        let key = wide(key);
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr());
        }
    }

    /// Claims the "Pious is running" marker. Returns `false` when
    /// another copy already holds it. The marker lives until this process
    /// exits.
    pub fn claim_single_instance() -> bool {
        // One copy per data folder, so portable copies with their own
        // folder (PIOUS_DATA) can run side by side.
        let folder = crate::core::store::data_dir().display().to_string().to_lowercase();
        let tag: u64 = folder.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
        let name = wide(&format!("Local\\Pious.Running.{tag:x}"));
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return false;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                // Let go, so the marker disappears once the other copy exits.
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return false;
            }
            true
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn read_user_value(_key: &str, _name: &str) -> Option<String> {
        None
    }
    pub fn write_user_value(_key: &str, _name: &str, _value: &str) -> Result<(), String> {
        Err("Opening Roblox links is only supported on Windows.".into())
    }
    pub fn delete_user_key(_key: &str) {}
    pub fn delete_user_value(_key: &str, _name: &str) {}
    pub fn claim_single_instance() -> bool {
        true
    }
}

pub use imp::*;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "Pious";
/// What the start-up entry was called before the rename.
const OLD_RUN_NAME: &str = "Pious Library";

/// Starts Pious with Windows (in the tray when `hidden`), or stops that.
pub fn set_run_at_startup(on: bool, hidden: bool) -> Result<(), String> {
    delete_user_value(RUN_KEY, OLD_RUN_NAME);
    if !on {
        delete_user_value(RUN_KEY, RUN_NAME);
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = if hidden { format!("\"{}\" --background", exe.display()) } else { format!("\"{}\"", exe.display()) };
    write_user_value(RUN_KEY, RUN_NAME, &command)
}

const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// The uninstall command Windows has for a bootstrapper, or for Roblox
/// itself (`"Roblox"`). Only these, never arbitrary programs.
pub fn uninstall_command(name: &str) -> Option<String> {
    let key = match name {
        "Roblox" => "roblox-player",
        "Bloxstrap" | "Fishstrap" | "Voidstrap" | "Lunastrap" | "Froststrap" => name,
        _ => return None,
    };
    read_user_value(&format!("{UNINSTALL_KEY}\\{key}"), "UninstallString").filter(|c| !c.trim().is_empty())
}

/// Runs a Windows command line like `"C:\\path\\app.exe" -uninstall`.
pub fn run_command_line(command: &str) -> Result<(), String> {
    let command = command.trim();
    let (exe, args) = match command.strip_prefix('"') {
        Some(rest) => {
            let end = rest.find('"').ok_or("Bad uninstall command")?;
            (&rest[..end], rest[end + 1..].trim())
        }
        None => match command.find(' ') {
            Some(i) => (&command[..i], command[i + 1..].trim()),
            None => (command, ""),
        },
    };
    let mut process = std::process::Command::new(exe);
    if !args.is_empty() {
        process.args(args.split_whitespace());
    }
    process.spawn().map(|_| ()).map_err(|e| format!("Couldn't start the uninstaller ({e})."))
}

/// The URL schemes Roblox uses to start the player from a browser.
pub const ROBLOX_SCHEMES: [&str; 2] = ["roblox-player", "roblox"];

fn command_key(scheme: &str) -> String {
    format!("Software\\Classes\\{scheme}\\shell\\open\\command")
}

/// The command line that opens a scheme's links right now, if any.
pub fn link_handler(scheme: &str) -> Option<String> {
    read_user_value(&command_key(scheme), "").filter(|c| !c.trim().is_empty())
}

/// The command line Pious registers for Roblox links.
pub fn pious_link_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\" --roblox-link \"%1\"", exe.display()))
}

/// Whether a handler command line belongs to Pious.
pub fn is_pious_command(command: &str) -> bool {
    command.contains("--roblox-link")
}

/// Makes Pious open `scheme` links.
pub fn register_link_handler(scheme: &str) -> Result<(), String> {
    let command = pious_link_command().ok_or("Couldn't find where Pious is installed.")?;
    let base = format!("Software\\Classes\\{scheme}");
    write_user_value(&base, "", &format!("URL: {scheme} Protocol"))?;
    write_user_value(&base, "URL Protocol", "")?;
    if let Ok(exe) = std::env::current_exe() {
        write_user_value(&format!("{base}\\DefaultIcon"), "", &exe.display().to_string())?;
    }
    write_user_value(&command_key(scheme), "", &command)
}

/// Makes `command` open `scheme` links (e.g. a bootstrapper's).
pub fn set_link_command(scheme: &str, command: &str) -> Result<(), String> {
    let base = format!("Software\\Classes\\{scheme}");
    write_user_value(&base, "", &format!("URL: {scheme} Protocol"))?;
    write_user_value(&base, "URL Protocol", "")?;
    write_user_value(&command_key(scheme), "", command)
}

/// Hands `scheme` links back to `previous`, or removes Pious's handler when
/// nothing opened them before.
pub fn restore_link_handler(scheme: &str, previous: Option<&str>) -> Result<(), String> {
    match previous {
        Some(command) => write_user_value(&command_key(scheme), "", command),
        None => {
            delete_user_key(&format!("Software\\Classes\\{scheme}"));
            Ok(())
        }
    }
}

/// Makes a desktop shortcut that opens a game with Pious (it runs
/// `pious.exe --roblox-link roblox://experiences/start?placeId=…`).
/// Returns the shortcut's path.
pub fn create_game_shortcut(name: &str, place_id: u64) -> Result<std::path::PathBuf, String> {
    let desktop = dirs::desktop_dir().ok_or("Couldn't find your desktop.")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Only characters Windows allows in file names.
    let cleaned: String = name.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) || c.is_control() { ' ' } else { c }).collect();
    let file = match cleaned.trim() {
        "" => "Roblox game".to_owned(),
        name => name.chars().take(80).collect(),
    };
    let path = desktop.join(format!("{file}.lnk"));
    // Everything goes in through the environment, never into the script.
    let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:PIOUS_LNK); \
                  $s.TargetPath = $env:PIOUS_EXE; $s.Arguments = $env:PIOUS_ARGS; \
                  $s.IconLocation = $env:PIOUS_EXE + ',0'; $s.Description = $env:PIOUS_DESC; $s.Save()";
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .env("PIOUS_LNK", &path)
        .env("PIOUS_EXE", &exe)
        .env("PIOUS_ARGS", format!("--roblox-link \"roblox://experiences/start?placeId={place_id}\""))
        .env("PIOUS_DESC", format!("Play {name} with Pious"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|e| format!("Couldn't make the shortcut ({e})."))?;
    if !output.status.success() || !path.is_file() {
        return Err("Windows didn't make the shortcut.".into());
    }
    Ok(path)
}

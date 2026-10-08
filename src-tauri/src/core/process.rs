//! Operating-system control of Roblox client windows: multi-instance,
//! finding, focusing and closing windows, anti-AFK input, window placement
//! and screen capture.

/// Saved size and position of a window, to put a new one in the same spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

/// What anti-AFK presses in a Roblox window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nudge {
    /// Space: a jump.
    Jump,
    /// W then S: a step forward and back.
    Walk,
    /// I then O: zoom the camera in and out (the character stays put).
    Zoom,
}

/// A captured screen area as RGBA pixels.
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[cfg(windows)]
mod imp {
    use std::sync::Mutex;
    use std::thread::sleep;
    use std::time::Duration;

    use super::{Capture, Nudge, Placement};
    use windows_sys::Win32::Foundation::{
        BOOL, CloseHandle, FALSE, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, POINT, RECT, TRUE,
    };
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GetDC, GetDIBits, HALFTONE, ReleaseDC, SRCCOPY, SelectObject, SetStretchBltMode,
        StretchBlt,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, CreateMutexW, GetCurrentThreadId, GetExitCodeProcess, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetLastInputInfo, KEYEVENTF_KEYUP, LASTINPUTINFO, MAPVK_VK_TO_VSC, MapVirtualKeyW, VK_MENU, VK_SPACE,
        keybd_event,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowPlacement, GetWindowRect,
        GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsWindow, IsWindowVisible, SW_MAXIMIZE,
        SW_MINIMIZE, SW_RESTORE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPlacement,
        SetWindowPos, ShowWindow, WINDOWPLACEMENT,
    };

    const STILL_ACTIVE: u32 = 259;

    /// Handles to Roblox's singleton names while multi-instance is on.
    static SINGLETON: Mutex<Vec<usize>> = Mutex::new(Vec::new());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Roblox lets one client run at a time by creating a named object;
    /// owning the name first lets every new client start on its own.
    /// Current builds use `ROBLOX_singletonEvent`, older ones the mutex.
    pub fn set_multi_instance(enabled: bool) {
        let mut held = SINGLETON.lock().unwrap_or_else(|e| e.into_inner());
        if enabled && held.is_empty() {
            for name in ["ROBLOX_singletonEvent", "ROBLOX_singletonMutex"] {
                let name = wide(name);
                let handle = unsafe { CreateMutexW(std::ptr::null(), TRUE, name.as_ptr()) };
                if !handle.is_null() {
                    held.push(handle as usize);
                }
            }
        } else if !enabled {
            for handle in held.drain(..) {
                unsafe { CloseHandle(handle as HANDLE) };
            }
        }
    }

    pub fn is_alive(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code == STILL_ACTIVE
        }
    }

    pub fn terminate(pid: u32) -> Result<(), String> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
            if handle.is_null() {
                return Err("The instance is no longer running.".into());
            }
            let ok = TerminateProcess(handle, 0) != 0;
            CloseHandle(handle);
            if ok { Ok(()) } else { Err("Windows refused to close the instance.".into()) }
        }
    }

    struct Search {
        pid: u32,
        found: Vec<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut window_pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut window_pid) };
        if (search.pid == 0 || window_pid == search.pid) && unsafe { IsWindowVisible(hwnd) } != 0 {
            search.found.push(hwnd);
        }
        TRUE
    }

    fn windows(pid: u32) -> Vec<HWND> {
        let mut search = Search { pid, found: Vec::new() };
        unsafe { EnumWindows(Some(enum_proc), &mut search as *mut Search as LPARAM) };
        search.found
    }

    /// The main window of a process (its largest visible one).
    pub fn window_of(pid: u32) -> Option<isize> {
        windows(pid)
            .into_iter()
            .max_by_key(|&w| {
                let mut rect: RECT = unsafe { std::mem::zeroed() };
                unsafe { GetWindowRect(w, &mut rect) };
                (rect.right - rect.left) as i64 * (rect.bottom - rect.top) as i64
            })
            .map(|w| w as isize)
    }

    pub fn focus(pid: u32) -> bool {
        window_of(pid).is_some_and(bring_to_front)
    }

    pub fn foreground_window() -> isize {
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn window_pid(hwnd: isize) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd as HWND, &mut pid) };
        pid
    }

    /// Brings a window to the front reliably. Windows only lets the
    /// foreground app do that, so this briefly attaches to the input of
    /// the window that has focus now and pins the target on top.
    pub fn bring_to_front(hwnd: isize) -> bool {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return false;
        }
        // A window that isn't answering (Roblox does this while it loads)
        // would block every call below until it answers again.
        if is_hung(hwnd as isize) {
            return false;
        }
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            let current = GetCurrentThreadId();
            let front = GetForegroundWindow();
            let foreground = GetWindowThreadProcessId(front, std::ptr::null_mut());
            // Attaching to a hung thread's input hangs this one too.
            let attached = foreground != 0
                && foreground != current
                && !is_hung(front as isize)
                && AttachThreadInput(current, foreground, TRUE) != 0;
            // A tap of Alt also unlocks SetForegroundWindow.
            keybd_event(VK_MENU as u8, 0, 0, 0);
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            BringWindowToTop(hwnd);
            let ok = SetForegroundWindow(hwnd) != 0;
            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            if attached {
                AttachThreadInput(current, foreground, FALSE);
            }
            ok
        }
    }

    /// Seconds since the user last touched the keyboard or mouse.
    pub fn idle_seconds() -> u32 {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        unsafe {
            if GetLastInputInfo(&mut info) == 0 {
                return 0;
            }
            GetTickCount().wrapping_sub(info.dwTime) / 1000
        }
    }

    /// Presses a key the way a keyboard does. Roblox reads keys by their
    /// hardware scan code, so a virtual-key-only event is ignored.
    fn tap(vk: u16) {
        unsafe {
            let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u8;
            keybd_event(vk as u8, scan, 0, 0);
            sleep(Duration::from_millis(40));
            keybd_event(vk as u8, scan, KEYEVENTF_KEYUP, 0);
            sleep(Duration::from_millis(30));
        }
    }

    /// Gives one Roblox window a moment of input so Roblox doesn't count
    /// the player as idle. The window comes to the front for a fraction of a
    /// second, and whatever was in front before (and the window's own
    /// minimized state) is restored right after.
    pub fn nudge(pid: u32, nudge: Nudge) -> bool {
        let Some(window) = window_of(pid) else { return false };
        if is_hung(window) {
            return false;
        }
        let target = window as HWND;
        let previous = foreground_window();
        let was_minimized = unsafe { IsIconic(target) } != 0;
        if previous != window && !bring_to_front(window) {
            return false;
        }
        sleep(Duration::from_millis(60));
        match nudge {
            Nudge::Jump => tap(VK_SPACE),
            Nudge::Walk => {
                tap(b'W' as u16);
                tap(b'S' as u16);
            }
            Nudge::Zoom => {
                tap(b'I' as u16);
                tap(b'O' as u16);
            }
        }
        sleep(Duration::from_millis(40));
        if was_minimized {
            unsafe { ShowWindow(target, SW_MINIMIZE) };
        }
        if previous != window && previous != 0 {
            bring_to_front(previous);
        }
        true
    }

    /// Every running process: (process ID, executable name).
    pub fn processes() -> Vec<(u32, String)> {
        let mut out = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
            while ok {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
                out.push((entry.th32ProcessID, String::from_utf16_lossy(&entry.szExeFile[..len])));
                ok = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        out
    }

    /// Process IDs of every running Roblox player.
    pub fn roblox_pids() -> Vec<u32> {
        pids_named("RobloxPlayerBeta.exe")
    }

    /// Process IDs of every running program with this executable name.
    pub fn pids_named(name: &str) -> Vec<u32> {
        processes().into_iter().filter(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(pid, _)| pid).collect()
    }

    /// A window's title. Reads it without asking the window, so a hung
    /// window can't block this.
    pub fn window_title(hwnd: isize) -> String {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, InternalGetWindowText};
        let len = unsafe { GetWindowTextLengthW(hwnd as HWND) }.max(0) as usize;
        let mut buffer = vec![0u16; len + 1];
        let n = unsafe { InternalGetWindowText(hwnd as HWND, buffer.as_mut_ptr(), buffer.len() as i32) };
        String::from_utf16_lossy(&buffer[..n.max(0) as usize])
    }

    pub fn placement(pid: u32) -> Option<Placement> {
        let window = window_of(pid)? as HWND;
        let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        if unsafe { GetWindowPlacement(window, &mut wp) } == 0 {
            return None;
        }
        let r = wp.rcNormalPosition;
        Some(Placement {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
            maximized: wp.showCmd == SW_MAXIMIZE as u32,
        })
    }

    pub fn place(pid: u32, placement: Placement) -> bool {
        let Some(window) = window_of(pid) else { return false };
        let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        wp.showCmd = if placement.maximized { SW_MAXIMIZE } else { SW_RESTORE } as u32;
        wp.ptMinPosition = POINT { x: -1, y: -1 };
        wp.ptMaxPosition = POINT { x: -1, y: -1 };
        wp.rcNormalPosition = RECT {
            left: placement.x,
            top: placement.y,
            right: placement.x + placement.width,
            bottom: placement.y + placement.height,
        };
        unsafe { SetWindowPlacement(window as HWND, &wp) != 0 }
    }

    /// Renames a process's window. Gives up after half a second if the
    /// window doesn't answer, instead of waiting forever like
    /// `SetWindowTextW` does.
    pub fn set_title(pid: u32, title: &str) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SMTO_ABORTIFHUNG, SMTO_NORMAL, SendMessageTimeoutW, WM_GETTEXT, WM_SETTEXT,
        };
        let Some(window) = window_of(pid) else { return };
        let title = wide(title);
        unsafe {
            // Already called that: nothing to do.
            let mut current = vec![0u16; title.len() + 1];
            let mut copied = 0usize;
            let asked = SendMessageTimeoutW(
                window as HWND,
                WM_GETTEXT,
                current.len(),
                current.as_mut_ptr() as isize,
                SMTO_NORMAL | SMTO_ABORTIFHUNG,
                500,
                &mut copied,
            );
            if asked != 0 && copied == title.len() - 1 && current[..copied] == title[..copied] {
                return;
            }
            SendMessageTimeoutW(
                window as HWND,
                WM_SETTEXT,
                0,
                title.as_ptr() as isize,
                SMTO_NORMAL | SMTO_ABORTIFHUNG,
                500,
                std::ptr::null_mut(),
            );
        }
    }

    /// Windows considers the window not responding.
    pub fn is_hung(hwnd: isize) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::IsHungAppWindow;
        hwnd != 0 && unsafe { IsHungAppWindow(hwnd as HWND) } != 0
    }

    /// Captures a screen rectangle (what's visible there right now),
    /// scaled down by `shrink` since it's only used blurred.
    pub fn capture(x: i32, y: i32, width: i32, height: i32, shrink: i32) -> Option<Capture> {
        let (w, h) = ((width / shrink).max(1), (height / shrink).max(1));
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return None;
            }
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap);
            let copied = if shrink == 1 {
                BitBlt(dc, 0, 0, w, h, screen, x, y, SRCCOPY)
            } else {
                SetStretchBltMode(dc, HALFTONE);
                StretchBlt(dc, 0, 0, w, h, screen, x, y, width, height, SRCCOPY)
            } != 0;

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(dc, bitmap, 0, h as u32, pixels.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);

            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            if !copied || lines == 0 {
                return None;
            }
            // BGRA → RGBA, opaque.
            for px in pixels.chunks_exact_mut(4) {
                px.swap(0, 2);
                px[3] = 255;
            }
            Some(Capture { width: w as u32, height: h as u32, rgba: pixels })
        }
    }

    /// Keeps a window out of screen captures (so capturing the screen for
    /// a blur never captures the window doing the blurring), or lets it
    /// back in.
    pub fn exclude_from_capture(hwnd: isize, exclude: bool) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE};
        let affinity = if exclude { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
        unsafe { SetWindowDisplayAffinity(hwnd as HWND, affinity) };
    }

    /// A window's drawable area on screen: (x, y, width, height).
    pub fn client_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
        use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let mut origin = POINT { x: 0, y: 0 };
        let ok = unsafe { GetClientRect(hwnd as HWND, &mut rect) != 0 && ClientToScreen(hwnd as HWND, &mut origin) != 0 };
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        (ok && w > 0 && h > 0).then_some((origin.x, origin.y, w, h))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Capture, Nudge, Placement};

    pub fn set_multi_instance(_enabled: bool) {}
    pub fn is_alive(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    pub fn terminate(pid: u32) -> Result<(), String> {
        std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()
            .map_err(|e| e.to_string())
            .and_then(|s| if s.success() { Ok(()) } else { Err("Couldn't close the instance.".into()) })
    }
    pub fn window_of(_pid: u32) -> Option<isize> {
        None
    }
    pub fn focus(_pid: u32) -> bool {
        false
    }
    pub fn foreground_window() -> isize {
        0
    }
    pub fn window_pid(_hwnd: isize) -> u32 {
        0
    }
    pub fn bring_to_front(_hwnd: isize) -> bool {
        false
    }
    pub fn idle_seconds() -> u32 {
        0
    }
    pub fn nudge(_pid: u32, _nudge: Nudge) -> bool {
        false
    }
    pub fn roblox_pids() -> Vec<u32> {
        Vec::new()
    }
    pub fn processes() -> Vec<(u32, String)> {
        Vec::new()
    }
    pub fn pids_named(_name: &str) -> Vec<u32> {
        Vec::new()
    }
    pub fn window_title(_hwnd: isize) -> String {
        String::new()
    }
    pub fn placement(_pid: u32) -> Option<Placement> {
        None
    }
    pub fn place(_pid: u32, _placement: Placement) -> bool {
        false
    }
    pub fn set_title(_pid: u32, _title: &str) {}
    pub fn is_hung(_hwnd: isize) -> bool {
        false
    }
    pub fn capture(_x: i32, _y: i32, _w: i32, _h: i32, _shrink: i32) -> Option<Capture> {
        None
    }
    pub fn exclude_from_capture(_hwnd: isize, _exclude: bool) {}
    pub fn client_rect(_hwnd: isize) -> Option<(i32, i32, i32, i32)> {
        None
    }
}

pub use imp::*;

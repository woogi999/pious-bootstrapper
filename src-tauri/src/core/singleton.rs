//! Multi-instance: Roblox allows one client at a time by keeping a named
//! event, `ROBLOX_singletonEvent`, open in every client; a new client that
//! finds it hands its game to the running one and quits. (Older builds used
//! the mutex `ROBLOX_singletonMutex` the same way.)
//!
//! Two ways around it, both used:
//! - Own the names before any client starts (`process::set_multi_instance`).
//!   That only works when Pious was running before every Roblox client.
//! - Close the event inside clients that already have it: with no client
//!   holding it, the next one starts on its own. This covers Roblox started
//!   before Pious, by another launcher, or from the website.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::{CloseHandle, DUPLICATE_CLOSE_SOURCE, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(class: u32, info: *mut c_void, length: u32, returned: *mut u32) -> i32;
        fn NtQueryObject(handle: HANDLE, class: u32, info: *mut c_void, length: u32, returned: *mut u32) -> i32;
    }

    const SYSTEM_EXTENDED_HANDLE_INFORMATION: u32 = 64;
    const OBJECT_NAME_INFORMATION: u32 = 1;
    const OBJECT_TYPE_INFORMATION: u32 = 2;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

    /// One entry of SYSTEM_HANDLE_INFORMATION_EX.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Entry {
        object: *mut c_void,
        pid: usize,
        handle: usize,
        access: u32,
        back_trace: u16,
        type_index: u16,
        attributes: u32,
        reserved: u32,
    }

    /// UNICODE_STRING at the start of OBJECT_NAME/TYPE_INFORMATION.
    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum: u16,
        buffer: *const u16,
    }

    fn text(info: &[u8]) -> String {
        if info.len() < std::mem::size_of::<UnicodeString>() {
            return String::new();
        }
        let s = unsafe { &*(info.as_ptr() as *const UnicodeString) };
        if s.buffer.is_null() || s.length == 0 {
            return String::new();
        }
        let chars = unsafe { std::slice::from_raw_parts(s.buffer, s.length as usize / 2) };
        String::from_utf16_lossy(chars)
    }

    fn query(handle: HANDLE, class: u32) -> String {
        let mut info = vec![0u8; 1024];
        let mut returned = 0u32;
        let status = unsafe { NtQueryObject(handle, class, info.as_mut_ptr() as _, info.len() as u32, &mut returned) };
        if status < 0 { String::new() } else { text(&info) }
    }

    /// Every open handle on the PC (`None` if Windows won't say).
    fn handles() -> Option<Vec<Entry>> {
        let mut size: usize = 1 << 20;
        for _ in 0..8 {
            let mut buffer = vec![0u8; size];
            let mut returned = 0u32;
            let status = unsafe {
                NtQuerySystemInformation(SYSTEM_EXTENDED_HANDLE_INFORMATION, buffer.as_mut_ptr() as _, buffer.len() as u32, &mut returned)
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                size = (returned as usize).max(size * 2) + (64 << 10);
                continue;
            }
            if status < 0 {
                return None;
            }
            let count = unsafe { *(buffer.as_ptr() as *const usize) };
            let header = 2 * std::mem::size_of::<usize>();
            let fits = (buffer.len() - header) / std::mem::size_of::<Entry>();
            let entries = unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(header) as *const Entry, count.min(fits)) };
            return Some(entries.to_vec());
        }
        None
    }

    /// Closes Roblox's singleton event inside the client `pid`. True when
    /// it was found (and closed).
    pub fn release(pid: u32) -> bool {
        let Some(all) = handles() else { return false };
        let process = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, pid) };
        if process.is_null() {
            return false;
        }
        let me = unsafe { GetCurrentProcess() };
        let mut released = false;
        for entry in all.iter().filter(|e| e.pid == pid as usize) {
            let mut copy: HANDLE = std::ptr::null_mut();
            let ok = unsafe { DuplicateHandle(process, entry.handle as HANDLE, me, &mut copy, 0, 0, DUPLICATE_SAME_ACCESS) };
            if ok == 0 {
                continue;
            }
            // The type first: asking some other kinds (pipes) for their
            // name can hang, events never do.
            let wanted = query(copy, OBJECT_TYPE_INFORMATION) == "Event" && query(copy, OBJECT_NAME_INFORMATION).ends_with("\\ROBLOX_singletonEvent");
            unsafe { CloseHandle(copy) };
            if wanted {
                let closed = unsafe {
                    DuplicateHandle(process, entry.handle as HANDLE, std::ptr::null_mut(), std::ptr::null_mut(), 0, 0, DUPLICATE_CLOSE_SOURCE)
                };
                released |= closed != 0;
            }
        }
        unsafe { CloseHandle(process) };
        released
    }
}

#[cfg(windows)]
pub use imp::release;

#[cfg(not(windows))]
pub fn release(_pid: u32) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Makes an event with Roblox's name in a child process and checks it
    /// gets closed there. (A child, because closing our own copy proves
    /// nothing about another process.)
    #[test]
    fn closes_the_event_in_another_process() {
        // PowerShell holds the event for a while.
        let script = "$e = New-Object System.Threading.EventWaitHandle($false, 'ManualReset', 'ROBLOX_singletonEvent'); Start-Sleep 20";
        let mut child = std::process::Command::new("powershell.exe").args(["-NoProfile", "-Command", script]).spawn().unwrap();
        // Wait for it to exist.
        let mut released = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if release(child.id()) {
                released = true;
                break;
            }
        }
        let _ = child.kill();
        assert!(released, "the event wasn't found in the child");
        assert!(!release(child.id()), "already closed");
    }
}

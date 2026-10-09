//! How much of the PC a process is using: its share of the processor since
//! it was last asked, and its memory. For the stats overlay.

use std::time::Instant;

/// Remembers the last reading, so each call gives the share since then.
pub struct Usage {
    pid: u32,
    last: Option<(Instant, u64)>,
}

impl Usage {
    pub fn new(pid: u32) -> Self {
        Usage { pid, last: None }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The share of the whole processor (all cores = 100) since the last
    /// call; `None` on the first call or if the process is gone.
    pub fn cpu(&mut self) -> Option<f32> {
        let busy = cpu_time(self.pid)?;
        let now = Instant::now();
        let share = self.last.and_then(|(at, before)| {
            let wall = now.duration_since(at).as_secs_f64();
            (wall > 0.05).then(|| {
                let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
                // 100-nanosecond units.
                ((busy.saturating_sub(before) as f64 / 1e7) / wall / cores * 100.0).clamp(0.0, 100.0) as f32
            })
        });
        self.last = Some((now, busy));
        share
    }
}

#[cfg(windows)]
fn cpu_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let ok = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        CloseHandle(process);
        let value = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        ok.then(|| value(kernel) + value(user))
    }
}

#[cfg(not(windows))]
fn cpu_time(_pid: u32) -> Option<u64> {
    None
}

/// The memory a process is using (its private working set), in bytes.
#[cfg(windows)]
pub fn memory(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let ok = GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0;
        CloseHandle(process);
        ok.then_some(counters.WorkingSetSize as u64)
    }
}

#[cfg(not(windows))]
pub fn memory(_pid: u32) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_process() {
        let me = std::process::id();
        let mut usage = Usage::new(me);
        assert!(usage.cpu().is_none(), "nothing to compare with yet");
        // Some work, then a reading.
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        std::thread::sleep(std::time::Duration::from_millis(60));
        let share = usage.cpu().expect("a reading");
        assert!((0.0..=100.0).contains(&share));
        assert!(memory(me).unwrap() > 1 << 20);
    }
}

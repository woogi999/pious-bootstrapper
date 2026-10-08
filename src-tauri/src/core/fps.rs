//! Roblox's frame rate, for the input overlay's FPS counter.
//!
//! Roblox doesn't say how fast it's drawing, and reading it from inside the
//! game would mean touching the client. Instead this watches the screen
//! with Windows' Desktop Duplication (what the recorder uses) and counts the
//! frames in which the game's part of the screen changed. That's what you
//! actually see, so it tops out at the monitor's refresh rate.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// A screen area: left, top, width, height.
pub type Area = (i32, i32, i32, i32);

/// Counts frames in the background until dropped.
pub struct Meter {
    stop: Arc<AtomicBool>,
    fps: Arc<AtomicU32>,
    target: Arc<Mutex<Option<Area>>>,
}

impl Meter {
    pub fn start() -> Meter {
        let meter = Meter {
            stop: Arc::new(AtomicBool::new(false)),
            fps: Arc::new(AtomicU32::new(0)),
            target: Arc::new(Mutex::new(None)),
        };
        let (stop, fps, target) = (meter.stop.clone(), meter.fps.clone(), meter.target.clone());
        std::thread::Builder::new()
            .name("fps-meter".into())
            .spawn(move || imp::run(&stop, &fps, &target))
            .ok();
        meter
    }

    /// The game's window on screen, or `None` while there's no game to watch.
    pub fn watch(&self, area: Option<Area>) {
        if let Ok(mut target) = self.target.lock() {
            *target = area;
        }
    }

    /// Frames the game showed in the last second.
    pub fn fps(&self) -> u32 {
        self.fps.load(Ordering::Relaxed)
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{HMODULE, RECT};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
    use windows::Win32::Graphics::Direct3D11::{D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device};
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIAdapter, IDXGIFactory1, IDXGIOutput1,
        IDXGIOutputDuplication, IDXGIResource,
    };
    use windows::core::Interface;

    pub fn run(stop: &AtomicBool, fps: &AtomicU32, target: &Mutex<Option<Area>>) {
        let area = || target.lock().ok().and_then(|t| *t);
        while !stop.load(Ordering::Relaxed) {
            let Some(game) = area() else {
                fps.store(0, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(250));
                continue;
            };
            match open(game) {
                Some((duplication, screen)) => watch(&duplication, screen, stop, fps, &area),
                // Another program may hold the screen for a moment (or the
                // game is on a screen that can't be watched).
                None => {
                    fps.store(0, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    /// Starts watching the screen the middle of `game` is on.
    fn open(game: Area) -> Option<(IDXGIOutputDuplication, RECT)> {
        let (cx, cy) = (game.0 + game.2 / 2, game.1 + game.3 / 2);
        unsafe {
            let factory = CreateDXGIFactory1::<IDXGIFactory1>().ok()?;
            for a in 0..8u32 {
                let Ok(adapter) = factory.EnumAdapters1(a) else { break };
                for o in 0..16u32 {
                    let Ok(output) = adapter.EnumOutputs(o) else { break };
                    let Ok(desc) = output.GetDesc() else { continue };
                    let r = desc.DesktopCoordinates;
                    if !(cx >= r.left && cx < r.right && cy >= r.top && cy < r.bottom) {
                        continue;
                    }
                    let mut device: Option<ID3D11Device> = None;
                    let base: IDXGIAdapter = adapter.cast().ok()?;
                    D3D11CreateDevice(
                        &base,
                        D3D_DRIVER_TYPE_UNKNOWN,
                        HMODULE::default(),
                        D3D11_CREATE_DEVICE_FLAG(0),
                        None,
                        D3D11_SDK_VERSION,
                        Some(&mut device),
                        None,
                        None,
                    )
                    .ok()?;
                    let output: IDXGIOutput1 = output.cast().ok()?;
                    let duplication = output.DuplicateOutput(&device?).ok()?;
                    return Some((duplication, r));
                }
            }
            None
        }
    }

    /// Counts frames that changed the game's part of the screen, until the
    /// game moves to another screen, the watch is lost or it's stopped.
    fn watch(duplication: &IDXGIOutputDuplication, screen: RECT, stop: &AtomicBool, fps: &AtomicU32, area: &dyn Fn() -> Option<Area>) {
        let mut frames = 0u32;
        let mut since = Instant::now();
        let mut rects: Vec<RECT> = vec![RECT::default(); 64];
        while !stop.load(Ordering::Relaxed) {
            let Some(game) = area() else { return };
            let (cx, cy) = (game.0 + game.2 / 2, game.1 + game.3 / 2);
            if !(cx >= screen.left && cx < screen.right && cy >= screen.top && cy < screen.bottom) {
                return;
            }
            // The game, in the screen's own coordinates.
            let (gl, gt) = (game.0 - screen.left, game.1 - screen.top);
            let (gr, gb) = (gl + game.2, gt + game.3);

            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            match unsafe { duplication.AcquireNextFrame(100, &mut info, &mut resource) } {
                Ok(()) => {
                    if info.LastPresentTime != 0 {
                        let mut needed = 0u32;
                        let size = (rects.len() * std::mem::size_of::<RECT>()) as u32;
                        let changed = match unsafe { duplication.GetFrameDirtyRects(size, rects.as_mut_ptr(), &mut needed) } {
                            Ok(()) => {
                                let count = needed as usize / std::mem::size_of::<RECT>();
                                rects[..count.min(rects.len())].iter().any(|r| r.left < gr && r.right > gl && r.top < gb && r.bottom > gt)
                            }
                            // Too many changes to list: count it.
                            Err(_) => {
                                if needed as usize > rects.len() * std::mem::size_of::<RECT>() {
                                    rects.resize(needed as usize / std::mem::size_of::<RECT>() + 8, RECT::default());
                                }
                                true
                            }
                        };
                        if changed {
                            frames += info.AccumulatedFrames.max(1);
                        }
                    }
                    drop(resource);
                    let _ = unsafe { duplication.ReleaseFrame() };
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {}
                // Lost (resolution change, a full-screen switch, the lock
                // screen): start over.
                Err(_) => return,
            }
            let elapsed = since.elapsed();
            if elapsed >= Duration::from_millis(500) {
                fps.store((frames as f64 / elapsed.as_secs_f64()).round() as u32, Ordering::Relaxed);
                frames = 0;
                since = Instant::now();
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn run(stop: &AtomicBool, _fps: &AtomicU32, _target: &Mutex<Option<Area>>) {
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
}

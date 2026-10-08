//! What's playing on the PC (Spotify, a YouTube video in a browser, any
//! app that shows up in Windows' media flyout), and its play, pause, next
//! and previous buttons. The overlay's media controls use this.

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NowPlaying {
    /// The app playing it ("Spotify", "Microsoft Edge").
    pub app: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub can_previous: bool,
    pub can_next: bool,
    pub can_pause: bool,
    /// Seconds into it and how long it is, when the app says.
    pub position: Option<f64>,
    pub duration: Option<f64>,
    /// The cover art as a data: URL.
    pub artwork: Option<String>,
}

/// What a media button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Toggle,
    Next,
    Previous,
}

/// A friendlier name for an app's ID ("Spotify.exe", "MSEdge",
/// "308046B0AF4A39CB" for Firefox).
fn app_name(id: &str) -> String {
    let lower = id.to_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("msedge", "Microsoft Edge"),
        ("chrome", "Google Chrome"),
        ("firefox", "Firefox"),
        ("308046b0af4a39cb", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vivaldi", "Vivaldi"),
        ("zunemusic", "Media Player"),
        ("applemusic", "Apple Music"),
        ("tidal", "TIDAL"),
        ("deezer", "Deezer"),
        ("vlc", "VLC"),
        ("discord", "Discord"),
    ];
    if let Some((_, name)) = known.iter().find(|(key, _)| lower.contains(key)) {
        return (*name).to_owned();
    }
    let base = id.rsplit(['\\', '!']).next().unwrap_or(id);
    base.trim_end_matches(".exe").to_owned()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use base64::Engine;
    use std::sync::Mutex;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::DataReader;
    use windows::core::Interface;
    use windows_future::IAsyncOperation;

    fn session() -> Option<Session> {
        Manager::RequestAsync().ok()?.join().ok()?.GetCurrentSession().ok()
    }

    /// The last cover art, by song, so it's only read once per song.
    static ARTWORK: Mutex<Option<(String, Option<String>)>> = Mutex::new(None);

    pub fn now_playing() -> Option<NowPlaying> {
        let session = session()?;
        let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let info = session.GetPlaybackInfo().ok()?;
        let controls = info.Controls().ok()?;
        let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        let app = app_name(&session.SourceAppUserModelId().map(|s| s.to_string()).unwrap_or_default());
        let timeline = session.GetTimelineProperties().ok();
        // TimeSpan counts 100 ns ticks.
        let seconds = |ticks: i64| ticks as f64 / 10_000_000.0;
        let duration = timeline.as_ref().and_then(|t| t.EndTime().ok()).map(|t| seconds(t.Duration)).filter(|d| *d > 0.0);
        let position = timeline.as_ref().and_then(|t| t.Position().ok()).map(|t| seconds(t.Duration)).filter(|_| duration.is_some());

        let key = format!("{app}\n{title}\n{artist}");
        let artwork = {
            let cached = ARTWORK.lock().ok().and_then(|a| a.clone()).filter(|(k, _)| *k == key);
            match cached {
                Some((_, art)) => art,
                None => {
                    let art = read_artwork(&props);
                    if let Ok(mut slot) = ARTWORK.lock() {
                        *slot = Some((key, art.clone()));
                    }
                    art
                }
            }
        };

        Some(NowPlaying {
            app,
            title,
            artist,
            playing: info.PlaybackStatus().ok() == Some(Status::Playing),
            can_previous: controls.IsPreviousEnabled().unwrap_or(false),
            can_next: controls.IsNextEnabled().unwrap_or(false),
            // Apps report the toggle, or play and pause separately.
            can_pause: controls.IsPlayPauseToggleEnabled().unwrap_or(false)
                || controls.IsPlayEnabled().unwrap_or(false)
                || controls.IsPauseEnabled().unwrap_or(false),
            position,
            duration,
            artwork,
        })
    }

    fn read_artwork(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<String> {
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let size = stream.Size().ok()?.min(4 * 1024 * 1024) as u32;
        if size == 0 {
            return None;
        }
        let kind = stream.ContentType().map(|s| s.to_string()).ok().filter(|t| t.starts_with("image/")).unwrap_or_else(|| "image/png".into());
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        let loaded = reader.LoadAsync(size).ok()?.cast::<IAsyncOperation<u32>>().ok()?.join().ok()?;
        let mut bytes = vec![0u8; loaded as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        Some(format!("data:{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    pub fn control(action: Action) -> Result<(), String> {
        let session = session().ok_or("Nothing is playing.")?;
        let op = match action {
            Action::Toggle => session.TryTogglePlayPauseAsync(),
            Action::Next => session.TrySkipNextAsync(),
            Action::Previous => session.TrySkipPreviousAsync(),
        }
        .map_err(|e| e.to_string())?;
        match op.join() {
            Ok(true) => Ok(()),
            Ok(false) => Err("That app didn't take the button.".into()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn now_playing() -> Option<NowPlaying> {
        None
    }
    pub fn control(_action: Action) -> Result<(), String> {
        Err("Media controls need Windows.".into())
    }
}

pub use imp::{control, now_playing};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_apps() {
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(app_name("MSEdge"), "Microsoft Edge");
        assert_eq!(app_name("308046B0AF4A39CB"), "Firefox");
        assert_eq!(app_name("C:\\Apps\\Player.exe"), "Player");
    }
}

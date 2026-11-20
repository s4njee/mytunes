use std::io::Write;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use mt_app::{ArtworkSize, TrackRow, library::DesktopLibrary};
use mt_audio::{PlayerEvent, PlayerState};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Manager, Url, WebviewWindow, async_runtime};

use crate::{
    events::DesktopChannels,
    playback::{DesktopPlayer, PlayerAction},
};

pub struct DesktopMedia(mpsc::SyncSender<Option<PlayerEvent>>);

impl DesktopMedia {
    pub fn start(
        app: &AppHandle,
        _window: &WebviewWindow,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "windows")]
        let hwnd = Some(_window.hwnd()?.0 as usize);
        #[cfg(not(target_os = "windows"))]
        let hwnd: Option<usize> = None;
        let (send, receive) = mpsc::sync_channel(32);
        let app = app.clone();
        std::thread::Builder::new()
            .name("desktop-media-controls".into())
            .spawn(move || {
                if let Err(error) = run(&app, receive, hwnd) {
                    eprintln!("system media controls: {error}");
                }
            })?;
        Ok(Self(send))
    }

    pub fn publish(&self, event: PlayerEvent) {
        let _ = self.0.try_send(Some(event));
    }

    pub fn stop(&self) {
        let _ = self.0.try_send(None);
    }
}

fn run(
    app: &AppHandle,
    receive: mpsc::Receiver<Option<PlayerEvent>>,
    hwnd: Option<usize>,
) -> Result<(), souvlaki::Error> {
    let mut controls = MediaControls::new(PlatformConfig {
        display_name: "MyTunes",
        dbus_name: "mytunes",
        hwnd: hwnd.map(|handle| handle as *mut std::ffi::c_void),
    })?;
    let handle = app.clone();
    controls.attach(move |event| {
        let app = handle.clone();
        async_runtime::spawn_blocking(move || media_action(&app, event));
    })?;
    let library = app.state::<Arc<DesktopLibrary>>();
    let mut active_track = None;
    #[cfg(target_os = "linux")]
    let mut last_volume = None;

    let initial = app.state::<DesktopChannels>().player.latest();
    let updates = initial
        .into_iter()
        .chain(receive.into_iter().map_while(|event| event));
    for update in updates {
        if active_track != update.track_id {
            active_track.clone_from(&update.track_id);
            let row = active_track.as_ref().and_then(|id| library.track(id).ok());
            let cover = row.as_ref().and_then(|row| cover_url(app, &library, row));
            let mut metadata = MediaMetadata::default();
            if let Some(row) = &row {
                metadata.title = row.title.as_deref();
                metadata.artist = row.artist.as_deref();
                metadata.album = row.album.as_deref();
                metadata.duration = row
                    .duration_ms
                    .and_then(|duration| u64::try_from(duration).ok())
                    .map(Duration::from_millis);
                metadata.cover_url = cover.as_ref().map(Url::as_str);
            }
            controls.set_metadata(metadata)?;
        }
        let progress = Some(MediaPosition(Duration::from_millis(update.position_ms)));
        controls.set_playback(match update.state {
            PlayerState::Playing => MediaPlayback::Playing { progress },
            PlayerState::Paused => MediaPlayback::Paused { progress },
            PlayerState::Empty | PlayerState::Ended | PlayerState::Error => MediaPlayback::Stopped,
        })?;
        #[cfg(target_os = "linux")]
        if last_volume != Some(update.volume_percent) {
            controls.set_volume(f64::from(update.volume_percent) / 100.0)?;
            last_volume = Some(update.volume_percent);
        }
    }
    Ok(())
}

fn cover_url(app: &AppHandle, library: &DesktopLibrary, row: &TrackRow) -> Option<Url> {
    let id = row.artwork_id.as_deref()?;
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let folder = app.path().app_cache_dir().ok()?.join("media-artwork");
    let path = folder.join(format!("{id}.jpg"));
    if !path.is_file() {
        let cover = library.artwork(id, ArtworkSize::Large).ok()??;
        std::fs::create_dir_all(&folder).ok()?;
        let mut staged = tempfile::NamedTempFile::new_in(&folder).ok()?;
        staged.write_all(&cover).ok()?;
        staged.persist(&path).ok()?;
    }
    Url::from_file_path(path).ok()
}

fn media_action(app: &AppHandle, event: MediaControlEvent) {
    let action = match event {
        MediaControlEvent::Play => PlayerAction::Play,
        MediaControlEvent::Pause => PlayerAction::Pause,
        MediaControlEvent::Toggle => PlayerAction::Toggle,
        MediaControlEvent::Next => PlayerAction::Next,
        MediaControlEvent::Previous => PlayerAction::Previous,
        MediaControlEvent::Stop => PlayerAction::Stop,
        MediaControlEvent::SetPosition(position) => seek_to(app, millis(position.0)),
        MediaControlEvent::Seek(direction) => seek_by(app, direction, 10_000),
        MediaControlEvent::SeekBy(direction, duration) => seek_by(app, direction, millis(duration)),
        MediaControlEvent::SetVolume(volume) if volume.is_finite() => PlayerAction::Volume {
            percent: (volume.clamp(0.0, 1.0) * 100.0).round() as u8,
        },
        MediaControlEvent::Raise => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            return;
        }
        MediaControlEvent::Quit => {
            app.exit(0);
            return;
        }
        _ => return,
    };
    if let Err(error) = app.state::<DesktopPlayer>().control(app, action) {
        eprintln!("system media action: {error}");
    }
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn seek_by(app: &AppHandle, direction: SeekDirection, offset: u64) -> PlayerAction {
    let position = app
        .state::<DesktopChannels>()
        .player
        .latest()
        .map_or(0, |update| update.position_ms);
    seek_to(
        app,
        match direction {
            SeekDirection::Forward => position.saturating_add(offset),
            SeekDirection::Backward => position.saturating_sub(offset),
        },
    )
}

fn seek_to(app: &AppHandle, position: u64) -> PlayerAction {
    let library = app.state::<Arc<DesktopLibrary>>();
    let duration = app
        .state::<DesktopChannels>()
        .player
        .latest()
        .and_then(|update| library.track(update.track_id.as_ref()?).ok())
        .and_then(|row| u64::try_from(row.duration_ms?).ok())
        .unwrap_or(u64::MAX);
    PlayerAction::Seek {
        position_ms: position.min(duration),
    }
}

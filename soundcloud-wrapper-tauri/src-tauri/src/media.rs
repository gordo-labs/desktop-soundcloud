use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadataPayload {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    #[serde(alias = "artwork", default)]
    pub artwork: Option<Vec<ArtworkEntry>>, // used for parsing arrays in JS payload
    #[serde(alias = "artworkUrl")]
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArtworkEntry {
    pub src: Option<String>,
}

impl MediaMetadataPayload {
    pub fn into_metadata(self) -> MediaMetadata {
        let artwork_url = if let Some(url) = self.artwork_url {
            Some(url)
        } else {
            self.artwork
                .and_then(|entries| entries.into_iter().find_map(|entry| entry.src))
        };

        MediaMetadata {
            title: self.title,
            artist: self.artist,
            album: self.album,
            artwork_url,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaUpdatePayload {
    pub playback_state: Option<String>,
    #[serde(default)]
    pub metadata: Option<MediaMetadataPayload>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeChangePayload {
    pub theme: Option<String>,
    #[serde(default)]
    pub metadata: Option<MediaMetadataPayload>,
}

#[derive(Debug, Clone, Default)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        PlaybackStatus::Paused
    }
}

#[derive(Debug, Clone, Default)]
pub struct MediaUpdate {
    pub playback: PlaybackStatus,
    pub metadata: Option<MediaMetadata>,
}

impl MediaUpdate {
    pub fn from_payload(payload: MediaUpdatePayload) -> Option<Self> {
        let playback = payload
            .playback_state
            .as_deref()
            .and_then(PlaybackStatus::from_str)
            .unwrap_or_default();

        let metadata = payload.metadata.map(MediaMetadataPayload::into_metadata);
        if metadata.is_none() && payload.playback_state.is_none() {
            None
        } else {
            Some(MediaUpdate { playback, metadata })
        }
    }

    pub fn playback(&self) -> PlaybackStatus {
        self.playback
    }

    pub fn metadata(&self) -> Option<&MediaMetadata> {
        self.metadata.as_ref()
    }
}

impl PlaybackStatus {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "playing" => Some(PlaybackStatus::Playing),
            "paused" => Some(PlaybackStatus::Paused),
            "stopped" => Some(PlaybackStatus::Stopped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MediaCache {
    pub playback: PlaybackStatus,
    pub metadata: Option<MediaMetadata>,
}

impl MediaCache {
    pub fn update(&mut self, update: &MediaUpdate) {
        self.playback = update.playback;
        if let Some(metadata) = &update.metadata {
            self.metadata = Some(metadata.clone());
        }
    }
}

#[derive(Default)]
pub struct MediaIntegration {
    #[cfg(target_os = "linux")]
    linux: Option<linux::LinuxIntegration>,
    #[cfg(target_os = "windows")]
    windows: Option<windows::WindowsIntegration>,
    #[cfg(target_os = "macos")]
    macos: Option<macos::MacIntegration>,
}

impl MediaIntegration {
    pub fn initialize(app: &AppHandle) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            linux: linux::LinuxIntegration::new(app),
            #[cfg(target_os = "windows")]
            windows: windows::WindowsIntegration::new(app),
            #[cfg(target_os = "macos")]
            macos: macos::MacIntegration::new(),
        }
    }

    pub fn update(&self, update: &MediaUpdate) {
        #[cfg(target_os = "linux")]
        if let Some(integration) = &self.linux {
            integration.update(update);
        }

        #[cfg(target_os = "windows")]
        if let Some(integration) = &self.windows {
            integration.update(update);
        }

        #[cfg(target_os = "macos")]
        if let Some(integration) = &self.macos {
            integration.update(update);
        }
    }
}

#[cfg(all(target_os = "linux", feature = "mpris-linux"))]
mod linux {
    use super::*;
    use crate::{
        emit_media_event, MEDIA_NEXT_EVENT, MEDIA_PAUSE_EVENT, MEDIA_PLAY_EVENT, MEDIA_PREVIOUS_EVENT, MEDIA_TOGGLE_EVENT,
    };
    use glib::{source::Priority, Continue, MainContext, MainLoop};
    use mpris_player::{LoopStatus, Metadata as MprisMetadata, MprisPlayer, PlaybackStatus as MprisPlaybackStatus};
    use std::sync::mpsc;

    #[derive(Debug, Clone)]
    enum Command {
        Update(MediaUpdate),
    }

    #[derive(Clone)]
    pub struct LinuxIntegration {
        sender: glib::Sender<Command>,
    }

    impl LinuxIntegration {
        pub fn new(app: &AppHandle) -> Option<Self> {
            let (ready_tx, ready_rx) = mpsc::channel();
            let app = app.clone();

            std::thread::spawn(move || {
                if let Err(error) = Self::run(app, ready_tx) {
                    eprintln!("[soundcloud-wrapper] Failed to initialize MPRIS service: {error}");
                }
            });

            ready_rx.recv().ok().map(|sender| Self { sender })
        }

        pub fn update(&self, update: &MediaUpdate) {
            let _ = self.sender.send(Command::Update(update.clone()));
        }

        fn run(app: AppHandle, ready_tx: mpsc::Sender<glib::Sender<Command>>) -> Result<(), String> {
            let context = MainContext::new();
            let _guard = context
                .acquire()
                .map_err(|_| "failed to acquire GLib main context".to_string())?;

            let main_loop = MainLoop::new(Some(&context), false);

            let player = MprisPlayer::new(
                "soundcloudwrapper".to_string(),
                "SoundCloud Wrapper".to_string(),
                "soundcloud-wrapper".to_string(),
            );

            player.set_can_raise(true);
            player.set_can_quit(false);
            player.set_can_play(true);
            player.set_can_pause(true);
            player.set_can_go_next(true);
            player.set_can_go_previous(true);
            player.set_can_seek(false);
            player.set_can_control(true);
            player.set_has_track_list(false);
            player.set_loop_status(LoopStatus::None);

            {
                let handle = app.clone();
                player.connect_play(move || emit_media_event(&handle, MEDIA_PLAY_EVENT));
            }
            {
                let handle = app.clone();
                player.connect_pause(move || emit_media_event(&handle, MEDIA_PAUSE_EVENT));
            }
            {
                let handle = app.clone();
                player.connect_play_pause(move || emit_media_event(&handle, MEDIA_TOGGLE_EVENT));
            }
            {
                let handle = app.clone();
                player.connect_next(move || emit_media_event(&handle, MEDIA_NEXT_EVENT));
            }
            {
                let handle = app.clone();
                player.connect_previous(move || emit_media_event(&handle, MEDIA_PREVIOUS_EVENT));
            }

            let (sender, receiver) = MainContext::channel::<Command>(Priority::default());
            ready_tx
                .send(sender)
                .map_err(|_| "failed to send MPRIS channel".to_string())?;

            receiver.attach(Some(&context), move |command| {
                match command {
                    Command::Update(update) => apply_update(&player, &update),
                }
                Continue(true)
            });

            main_loop.run();
            Ok(())
        }
    }

    fn apply_update(player: &MprisPlayer, update: &MediaUpdate) {
        let status = match update.playback {
            PlaybackStatus::Playing => MprisPlaybackStatus::Playing,
            PlaybackStatus::Paused => MprisPlaybackStatus::Paused,
            PlaybackStatus::Stopped => MprisPlaybackStatus::Stopped,
        };
        player.set_playback_status(status);

        if let Some(metadata) = &update.metadata {
            let mut payload = MprisMetadata::new();
            payload.title = metadata.title.clone();
            payload.artist = metadata
                .artist
                .clone()
                .map(|artist| vec![artist])
                .or_else(|| metadata.title.clone().map(|title| vec![title]));
            payload.album = metadata.album.clone();
            payload.art_url = metadata.artwork_url.clone();
            player.set_metadata(payload);
        }
    }
}

#[cfg(all(target_os = "linux", not(feature = "mpris-linux")))]
mod linux {
    use super::*;

    pub struct LinuxIntegration;

    impl LinuxIntegration {
        pub fn new(_app: &AppHandle) -> Option<Self> {
            eprintln!(
                "[soundcloud-wrapper] MPRIS integration disabled. Enable the 'mpris-linux' feature and install GLib development files to activate."
            );
            None
        }

        pub fn update(&self, _update: &MediaUpdate) {}
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use crate::{emit_media_event, MEDIA_NEXT_EVENT, MEDIA_PAUSE_EVENT, MEDIA_PLAY_EVENT, MEDIA_PREVIOUS_EVENT};
    use tauri::Manager;
    use windows::core::{factory, HSTRING};
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Media::MediaPlaybackStatus;
    use windows::Media::Playback::MediaPlaybackType;
    use windows::Media::SystemMediaTransportControls;
    use windows::Media::SystemMediaTransportControlsButton;
    use windows::Media::SystemMediaTransportControlsDisplayUpdater;
    use windows::Media::SystemMediaTransportControlsProperty;
    use windows::Media::SystemMediaTransportControlsTimelineProperties;
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

    pub struct WindowsIntegration {
        smtc: SystemMediaTransportControls,
        _button_token: i64,
        _property_token: i64,
    }

    impl WindowsIntegration {
        pub fn new(app: &AppHandle) -> Option<Self> {
            let window = app.get_window("main")?;
            let hwnd = window.hwnd().ok()?;

            let interop: ISystemMediaTransportControlsInterop = factory::<
                SystemMediaTransportControls,
                ISystemMediaTransportControlsInterop,
            >()
            .ok()?;

            let smtc: SystemMediaTransportControls = unsafe { interop.GetForWindow::<SystemMediaTransportControls>(HWND(hwnd.0)) }
                .ok()?;

            let play_handle = app.clone();
            let handler = TypedEventHandler::new(move |_, args: Option<_>| {
                if let Some(args) = args {
                    if let Ok(button) = args.Button() {
                        match button {
                            SystemMediaTransportControlsButton::Play => {
                                emit_media_event(&play_handle, MEDIA_PLAY_EVENT);
                            }
                            SystemMediaTransportControlsButton::Pause => {
                                emit_media_event(&play_handle, MEDIA_PAUSE_EVENT);
                            }
                            SystemMediaTransportControlsButton::PlayPause => {
                                emit_media_event(&play_handle, MEDIA_PLAY_EVENT);
                            }
                            SystemMediaTransportControlsButton::Next => {
                                emit_media_event(&play_handle, MEDIA_NEXT_EVENT);
                            }
                            SystemMediaTransportControlsButton::Previous => {
                                emit_media_event(&play_handle, MEDIA_PREVIOUS_EVENT);
                            }
                            _ => {}
                        }
                    }
                }
                Ok(())
            });

            smtc.SetIsEnabled(true).ok()?;
            smtc.SetIsPlayEnabled(true).ok()?;
            smtc.SetIsPauseEnabled(true).ok()?;
            smtc.SetIsStopEnabled(true).ok()?;
            smtc.SetIsNextEnabled(true).ok()?;
            smtc.SetIsPreviousEnabled(true).ok()?;

            let button_token = smtc.ButtonPressed(&handler).ok()?;

            let property_handler = TypedEventHandler::new(move |_, _| Ok(()));
            let property_token = smtc.PropertyChanged(&property_handler).ok()?;

            Some(Self {
                smtc,
                _button_token: button_token,
                _property_token: property_token,
            })
        }

        pub fn update(&self, update: &MediaUpdate) {
            let status = match update.playback {
                PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
                PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
                PlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
            };
            if let Err(error) = self.smtc.SetPlaybackStatus(status) {
                eprintln!("[soundcloud-wrapper] Failed to set SMTC status: {error:?}");
            }

            if let Some(metadata) = &update.metadata {
                if let Err(error) = update_display(&self.smtc, metadata) {
                    eprintln!("[soundcloud-wrapper] Failed to update SMTC metadata: {error:?}");
                }
            }
        }
    }

    fn update_display(
        smtc: &SystemMediaTransportControls,
        metadata: &MediaMetadata,
    ) -> windows::core::Result<()> {
        let updater: SystemMediaTransportControlsDisplayUpdater = smtc.DisplayUpdater()?;
        updater.SetType(MediaPlaybackType::Music)?;
        let music = updater.MusicProperties()?;

        if let Some(title) = &metadata.title {
            music.SetTitle(&HSTRING::from(title))?;
        }
        if let Some(artist) = &metadata.artist {
            music.SetArtist(&HSTRING::from(artist))?;
        }
        if let Some(album) = &metadata.album {
            music.SetAlbumTitle(&HSTRING::from(album))?;
        }

        if let Some(art) = &metadata.artwork_url {
            if let Ok(uri) = Uri::CreateUri(&HSTRING::from(art)) {
                if let Ok(stream) = RandomAccessStreamReference::CreateFromUri(&uri) {
                    updater.SetThumbnail(stream)?;
                }
            }
        }

        updater.Update()?;

        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(windows::Foundation::TimeSpan { Duration: 0 })?;
        timeline.SetPosition(windows::Foundation::TimeSpan { Duration: 0 })?;
        timeline.SetEndTime(windows::Foundation::TimeSpan { Duration: 0 })?;
        smtc.UpdateTimelineProperties(timeline)?;

        Ok(())
    }

    impl Drop for WindowsIntegration {
        fn drop(&mut self) {
            let _ = self.smtc.RemoveButtonPressed(self._button_token);
            let _ = self.smtc.RemovePropertyChanged(self._property_token);
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::Class;
    use objc2::{msg_send, sel, sel_impl};
    use objc2_foundation::{ns_string, NSDictionary, NSNumber, NSString};

    pub struct MacIntegration;

    impl MacIntegration {
        pub fn new() -> Option<Self> {
            unsafe { Class::get("MPNowPlayingInfoCenter").map(|_| MacIntegration) }
        }

        pub fn update(&self, update: &MediaUpdate) {
            autoreleasepool(|_| unsafe {
                let Some(class) = Class::get("MPNowPlayingInfoCenter") else {
                    return;
                };
                let center: *mut objc2::runtime::Object = msg_send![class, defaultCenter];
                if center.is_null() {
                    return;
                }

                let mut entries: Vec<(&NSString, &objc2::runtime::Object)> = Vec::new();

                if let Some(metadata) = &update.metadata {
                    if let Some(title) = &metadata.title {
                        let value = NSString::from_str(title);
                        entries.push((ns_string!("MPMediaItemPropertyTitle"), value.as_ref()));
                    }
                    if let Some(artist) = &metadata.artist {
                        let value = NSString::from_str(artist);
                        entries.push((ns_string!("MPMediaItemPropertyArtist"), value.as_ref()));
                    }
                    if let Some(album) = &metadata.album {
                        let value = NSString::from_str(album);
                        entries.push((ns_string!("MPMediaItemPropertyAlbumTitle"), value.as_ref()));
                    }
                    if let Some(artwork) = &metadata.artwork_url {
                        let value = NSString::from_str(artwork);
                        entries.push((ns_string!("MPNowPlayingInfoPropertyAssetURL"), value.as_ref()));
                    }
                }

                let rate = match update.playback {
                    PlaybackStatus::Playing => 1.0,
                    _ => 0.0,
                };
                let rate_number = NSNumber::new_f64(rate);
                entries.push((ns_string!("MPNowPlayingInfoPropertyPlaybackRate"), rate_number.as_ref()));

                let (keys, values): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
                let dict = NSDictionary::from_slices(&keys, &values);
                let _: () = msg_send![center, setNowPlayingInfo: dict];
            });
        }
    }
}

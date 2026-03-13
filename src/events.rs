use uuid::Uuid;
use std::path::PathBuf;
use crate::player::PlaybackState;
use crate::app::StatusLevel;

#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    FocusNext,
    MoveUp,
    MoveDown,
    Select,

    // Feed management
    AddFeed,
    ConfirmAddFeed,
    CancelInput,
    DeleteFeed,
    RefreshFeed,
    RefreshAllFeeds,

    // Playback
    PlayPause,
    SeekForward(f64),
    SeekBackward(f64),
    VolumeUp,
    VolumeDown,
    SpeedUp,
    SpeedDown,
    NextChapter,
    PrevChapter,
    InputChar(char),
    InputBackspace,

    // Download
    DownloadEpisode,

    // App
    Quit,
    Resize(u16, u16),

    // Internal (background tasks → main loop)
    FeedRefreshed(Uuid, String, Vec<crate::rss::types::Episode>),
    FeedRefreshError(Uuid, String),
    PlaybackProgress(PlaybackState),
    DownloadProgress(Uuid, f32),
    DownloadComplete(Uuid, PathBuf),
    DownloadError(Uuid, String),
    StatusMessage(String, StatusLevel),
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub last_refreshed: Option<DateTime<Utc>>,
    pub episodes: Vec<Episode>,
}

impl Feed {
    pub fn new(url: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: url.clone(),
            url,
            description: None,
            last_refreshed: None,
            episodes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Uuid,
    pub feed_id: Uuid,
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub audio_url: String,
    pub published: Option<DateTime<Utc>>,
    pub duration_secs: Option<u64>,
    pub is_new: bool,
    pub download: DownloadState,
    pub listen_progress_secs: u64,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadState {
    NotDownloaded,
    Downloading { progress: f32 },
    Downloaded { path: PathBuf },
    Failed { reason: String },
}

impl Default for DownloadState {
    fn default() -> Self {
        Self::NotDownloaded
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub start_secs: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("RSS parse error: {0}")]
    Parse(String),
    #[error("No audio enclosure in episode: {guid}")]
    NoEnclosure { guid: String },
}

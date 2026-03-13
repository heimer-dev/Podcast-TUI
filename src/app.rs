use std::collections::HashSet;
use uuid::Uuid;
use crate::rss::types::Feed;
use crate::player::PlaybackState;

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Feeds,
    Episodes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    AddFeedUrl,
}

#[derive(Debug, Clone)]
pub enum StatusLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
    pub expires_at: std::time::Instant,
}

pub struct AppState {
    pub focus: Focus,
    pub selected_feed_idx: usize,
    pub selected_episode_idx: usize,
    pub feeds: Vec<Feed>,
    pub playback: PlaybackState,
    pub status_message: Option<StatusMessage>,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub downloading: HashSet<Uuid>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(feeds: Vec<Feed>) -> Self {
        Self {
            focus: Focus::Feeds,
            selected_feed_idx: 0,
            selected_episode_idx: 0,
            feeds,
            playback: PlaybackState::new(),
            status_message: None,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            downloading: HashSet::new(),
            should_quit: false,
        }
    }

    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            level,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(4),
        });
    }

    pub fn current_feed(&self) -> Option<&Feed> {
        self.feeds.get(self.selected_feed_idx)
    }

    pub fn current_episodes(&self) -> &[crate::rss::types::Episode] {
        self.current_feed()
            .map(|f| f.episodes.as_slice())
            .unwrap_or(&[])
    }

    pub fn current_episode(&self) -> Option<&crate::rss::types::Episode> {
        self.current_episodes().get(self.selected_episode_idx)
    }

    /// Number of items in the feeds panel (feeds + "[+] Add" entry)
    pub fn feeds_list_len(&self) -> usize {
        self.feeds.len() + 1
    }
}

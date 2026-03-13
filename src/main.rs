mod app;
mod config;
mod download;
mod events;
mod player;
mod rss;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tracing::{debug, info};
use tracing_subscriber;

use app::{AppState, Focus, InputMode, StatusLevel};
use config::Config;
use download::DownloadManager;
use events::Action;
use player::{MpvController, PlayStatus};
use rss::types::DownloadState;
use ui::UiState;

const MPV_SOCKET: &str = "/tmp/podcast-tui.sock";
const TICK_MS: u64 = 50;

#[tokio::main]
async fn main() -> Result<()> {
    // Panic hook to restore terminal on crash
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // File logging (never to stdout — ratatui owns the terminal)
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("podcast-tui");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::never(&log_dir, "debug.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .init();
    info!("podcast-tui starting, log: {}/debug.log", log_dir.display());

    // Load config
    let cfg = config::load().unwrap_or_default();
    let mut state = AppState::new(cfg.feeds.clone());
    state.playback.speed = cfg.settings.default_speed;
    state.playback.volume = cfg.settings.default_volume;

    let download_dir = cfg.settings.download_dir.clone();
    let mut current_cfg = cfg;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channels: background tasks + keyboard → main loop
    let (tx, mut rx) = mpsc::channel::<Action>(256);
    let download_manager = DownloadManager::new(tx.clone());

    // Dedicated blocking thread for crossterm keyboard input
    let input_tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        info!("input thread started");
        loop {
            match event::read() {
                Ok(Event::Key(k)) => {
                    debug!("raw key: code={:?} kind={:?} modifiers={:?}", k.code, k.kind, k.modifiers);
                    if input_tx.blocking_send(Action::CrosstermKey(k)).is_err() {
                        info!("input channel closed, exiting thread");
                        break;
                    }
                }
                Ok(Event::Resize(w, h)) => {
                    let _ = input_tx.blocking_send(Action::Resize(w, h));
                }
                Ok(other) => {
                    debug!("other event: {:?}", other);
                }
                Err(e) => {
                    info!("event::read error: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn mpv
    let mpv_socket = PathBuf::from(MPV_SOCKET);
    // Remove stale socket
    let _ = std::fs::remove_file(&mpv_socket);

    let mpv_result = MpvController::spawn(&mpv_socket).await;
    let (mpv, _mpv_child) = match mpv_result {
        Ok((ctrl, read_half, child)) => {
            // Poll events from the SAME connection read_half (observe_property works)
            let poll_tx = tx.clone();
            tokio::spawn(player::poll_mpv(read_half, poll_tx));
            ctrl.set_volume(current_cfg.settings.default_volume).await.ok();
            ctrl.set_speed(current_cfg.settings.default_speed).await.ok();
            (Some(ctrl), Some(child))
        }
        Err(e) => {
            state.set_status(
                format!("mpv not available: {} — playback disabled", e),
                StatusLevel::Warning,
            );
            (None, None)
        }
    };

    let mut ui_state = UiState::new();
    let mut last_save = std::time::Instant::now();
    let mut render_interval = tokio::time::interval(Duration::from_millis(TICK_MS));

    loop {
        // Expire status messages
        if let Some(msg) = &state.status_message {
            if msg.expires_at <= std::time::Instant::now() {
                state.status_message = None;
            }
        }

        // Either a tick (for re-render) or an action from keyboard/background
        tokio::select! {
            _ = render_interval.tick() => {
                terminal.draw(|frame| ui::render(frame, &state, &mut ui_state))?;
            }
            Some(action) = rx.recv() => {
                // Map raw key events using current state
                let action = match action {
                    Action::CrosstermKey(k) => map_key(k, &state),
                    other => Some(other),
                };
                if let Some(action) = action {
                    process_action(
                        action,
                        &mut state,
                        &mut current_cfg,
                        &mpv,
                        &download_manager,
                        &download_dir,
                        tx.clone(),
                    )
                    .await;
                }
            }
        }

        // Auto-save every 30s
        if last_save.elapsed() > Duration::from_secs(30) {
            save_state(&state, &mut current_cfg);
            last_save = std::time::Instant::now();
        }

        if state.should_quit {
            break;
        }
    }

    // Graceful shutdown
    if let Some(mpv) = &mpv {
        mpv.quit().await.ok();
    }
    save_state(&state, &mut current_cfg);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

fn map_key(key: crossterm::event::KeyEvent, state: &AppState) -> Option<Action> {
    use crossterm::event::KeyEventKind;
    if key.kind != KeyEventKind::Press {
        debug!("map_key: skipping non-press event kind={:?}", key.kind);
        return None;
    }
    debug!("map_key: processing code={:?}", key.code);

    if state.input_mode == InputMode::AddFeedUrl {
        return match key.code {
            KeyCode::Enter => Some(Action::ConfirmAddFeed),
            KeyCode::Esc => Some(Action::CancelInput),
            KeyCode::Char(c) => Some(Action::InputChar(c)),
            KeyCode::Backspace => Some(Action::InputBackspace),
            _ => None,
        };
    }

    // Help overlay eats all keys except close keys
    if state.show_help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Char('a') => Some(Action::AddFeed),
        KeyCode::Char('d') => Some(Action::DeleteFeed),
        KeyCode::Char('r') => Some(Action::RefreshFeed),
        KeyCode::Char('R') => Some(Action::RefreshAllFeeds),
        KeyCode::Char(' ') => Some(Action::PlayPause),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::SeekForward(10.0)),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::SeekBackward(10.0)),
        KeyCode::Char('L') => Some(Action::SeekForward(60.0)),
        KeyCode::Char('H') => Some(Action::SeekBackward(60.0)),
        KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') => Some(Action::VolumeDown),
        KeyCode::Char('>') => Some(Action::SpeedUp),
        KeyCode::Char('<') => Some(Action::SpeedDown),
        KeyCode::Char(']') => Some(Action::NextChapter),
        KeyCode::Char('[') => Some(Action::PrevChapter),
        KeyCode::Char('D') => Some(Action::DownloadEpisode),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        _ => None,
    }
}

async fn process_action(
    action: Action,
    state: &mut AppState,
    cfg: &mut Config,
    mpv: &Option<MpvController>,
    dl: &DownloadManager,
    download_dir: &PathBuf,
    tx: mpsc::Sender<Action>,
) {
    debug!("process_action: {:?}", action);
    match action {
        Action::Quit => state.should_quit = true,

        Action::ToggleHelp => state.show_help = !state.show_help,

        Action::Resize(_, _) => {}

        Action::FocusNext => {
            state.focus = match state.focus {
                Focus::Feeds => Focus::Episodes,
                Focus::Episodes => Focus::Feeds,
            };
        }

        Action::MoveUp => match state.focus {
            Focus::Feeds => {
                if state.selected_feed_idx > 0 {
                    state.selected_feed_idx -= 1;
                    state.selected_episode_idx = 0;
                }
            }
            Focus::Episodes => {
                if state.selected_episode_idx > 0 {
                    state.selected_episode_idx -= 1;
                }
            }
        },

        Action::MoveDown => match state.focus {
            Focus::Feeds => {
                if state.selected_feed_idx + 1 < state.feeds_list_len() {
                    state.selected_feed_idx += 1;
                    state.selected_episode_idx = 0;
                }
            }
            Focus::Episodes => {
                let max = state.current_episodes().len().saturating_sub(1);
                if state.selected_episode_idx < max {
                    state.selected_episode_idx += 1;
                }
            }
        },

        Action::Select => {
            if state.focus == Focus::Feeds && state.selected_feed_idx == state.feeds.len() {
                state.input_mode = InputMode::AddFeedUrl;
                state.input_buffer.clear();
                return;
            }
            if state.focus == Focus::Episodes {
                if let Some(ep) = state.current_episode().cloned() {
                    let audio_url = match &ep.download {
                        DownloadState::Downloaded { path } => {
                            path.to_string_lossy().to_string()
                        }
                        _ => ep.audio_url.clone(),
                    };
                    if let Some(mpv) = mpv {
                        info!("loading: {}", audio_url);
                        if let Err(e) = mpv.load_file(&audio_url).await {
                            info!("load_file error: {}", e);
                        }
                        mpv.set_speed(state.playback.speed).await.ok();
                        mpv.set_volume(state.playback.volume).await.ok();
                        state.playback.episode_id = Some(ep.id);
                        state.playback.episode_title = Some(ep.title.clone());
                        state.playback.status = PlayStatus::Playing;
                        state.playback.position_secs = 0.0;
                        if let Some(feed) = state.feeds.get_mut(state.selected_feed_idx) {
                            if let Some(episode) = feed.episodes.get_mut(state.selected_episode_idx) {
                                episode.is_new = false;
                            }
                        }
                    } else {
                        state.set_status("mpv not available", StatusLevel::Error);
                    }
                }
            }
        }

        Action::AddFeed => {
            state.input_mode = InputMode::AddFeedUrl;
            state.input_buffer.clear();
        }

        Action::InputChar(c) => {
            state.input_buffer.push(c);
        }

        Action::InputBackspace => {
            state.input_buffer.pop();
        }

        Action::CancelInput => {
            state.input_mode = InputMode::Normal;
            state.input_buffer.clear();
        }

        Action::ConfirmAddFeed => {
            let url = state.input_buffer.trim().to_string();
            state.input_mode = InputMode::Normal;
            state.input_buffer.clear();

            if url.is_empty() {
                return;
            }

            let feed = rss::types::Feed::new(url.clone());
            let feed_id = feed.id;
            state.feeds.push(feed);
            state.selected_feed_idx = state.feeds.len() - 1;
            state.set_status("Fetching feed…", StatusLevel::Info);

            let tx2 = tx.clone();
            tokio::spawn(async move {
                match rss::fetch_feed(feed_id, &url).await {
                    Ok((title, episodes)) => {
                        let _ = tx2.send(Action::FeedRefreshed(feed_id, title, episodes)).await;
                    }
                    Err(e) => {
                        let _ = tx2.send(Action::FeedRefreshError(feed_id, e.to_string())).await;
                    }
                }
            });
        }

        Action::DeleteFeed => {
            if state.focus == Focus::Feeds && state.selected_feed_idx < state.feeds.len() {
                state.feeds.remove(state.selected_feed_idx);
                if state.selected_feed_idx > 0 {
                    state.selected_feed_idx -= 1;
                }
                state.selected_episode_idx = 0;
                save_state(state, cfg);
                state.set_status("Feed removed", StatusLevel::Info);
            }
        }

        Action::RefreshFeed => {
            let feed_idx = state.selected_feed_idx;
            if let Some(feed) = state.feeds.get(feed_idx) {
                let feed_id = feed.id;
                let url = feed.url.clone();
                let tx2 = tx.clone();
                state.set_status("Refreshing…", StatusLevel::Info);
                tokio::spawn(async move {
                    match rss::fetch_feed(feed_id, &url).await {
                        Ok((title, episodes)) => {
                            let _ = tx2.send(Action::FeedRefreshed(feed_id, title, episodes)).await;
                        }
                        Err(e) => {
                            let _ = tx2.send(Action::FeedRefreshError(feed_id, e.to_string())).await;
                        }
                    }
                });
            }
        }

        Action::RefreshAllFeeds => {
            state.set_status("Refreshing all feeds…", StatusLevel::Info);
            for feed in &state.feeds {
                let feed_id = feed.id;
                let url = feed.url.clone();
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    match rss::fetch_feed(feed_id, &url).await {
                        Ok((title, episodes)) => {
                            let _ = tx2.send(Action::FeedRefreshed(feed_id, title, episodes)).await;
                        }
                        Err(e) => {
                            let _ = tx2.send(Action::FeedRefreshError(feed_id, e.to_string())).await;
                        }
                    }
                });
            }
        }

        Action::FeedRefreshed(id, title, mut fresh_episodes) => {
            if let Some(feed) = state.feeds.iter_mut().find(|f| f.id == id) {
                rss::merge_episodes(&mut feed.episodes, fresh_episodes.drain(..).collect());
                feed.title = title;
                feed.last_refreshed = Some(chrono::Utc::now());
                let count = feed.episodes.len();
                state.set_status(format!("Refreshed — {} episodes", count), StatusLevel::Info);
                save_state(state, cfg);
            }
        }

        Action::FeedRefreshError(id, err) => {
            if let Some(feed) = state.feeds.iter().find(|f| f.id == id) {
                let title = feed.title.clone();
                state.set_status(format!("Error: {} — {}", title, err), StatusLevel::Error);
            }
        }

        Action::PlayPause => {
            if let Some(mpv) = mpv {
                info!("toggle_pause called");
                if let Err(e) = mpv.toggle_pause().await {
                    info!("toggle_pause error: {}", e);
                }
            } else {
                info!("PlayPause: mpv is None");
            }
        }

        Action::SeekForward(secs) => {
            if let Some(mpv) = mpv {
                mpv.seek(secs, player::SeekMode::Relative).await.ok();
            }
        }

        Action::SeekBackward(secs) => {
            if let Some(mpv) = mpv {
                mpv.seek(-secs, player::SeekMode::Relative).await.ok();
            }
        }

        Action::VolumeUp => {
            let vol = (state.playback.volume as u32 + 5).min(100) as u8;
            state.playback.volume = vol;
            if let Some(mpv) = mpv {
                mpv.set_volume(vol).await.ok();
            }
        }

        Action::VolumeDown => {
            let vol = state.playback.volume.saturating_sub(5);
            state.playback.volume = vol;
            if let Some(mpv) = mpv {
                mpv.set_volume(vol).await.ok();
            }
        }

        Action::SpeedUp => {
            let speed = ((state.playback.speed + 0.25) * 100.0).round() / 100.0;
            let speed = speed.min(3.0);
            state.playback.speed = speed;
            if let Some(mpv) = mpv {
                mpv.set_speed(speed).await.ok();
            }
        }

        Action::SpeedDown => {
            let speed = ((state.playback.speed - 0.25) * 100.0).round() / 100.0;
            let speed = speed.max(0.25);
            state.playback.speed = speed;
            if let Some(mpv) = mpv {
                mpv.set_speed(speed).await.ok();
            }
        }

        Action::NextChapter => {
            if let Some(mpv) = mpv {
                mpv.next_chapter().await.ok();
            }
        }

        Action::PrevChapter => {
            if let Some(mpv) = mpv {
                mpv.prev_chapter().await.ok();
            }
        }

        Action::DownloadEpisode => {
            if state.focus == Focus::Episodes {
                if let Some(ep) = state.current_episode().cloned() {
                    match &ep.download {
                        DownloadState::NotDownloaded | DownloadState::Failed { .. } => {
                            let ep_title = ep.title.clone();
                            if let Some(feed) = state.feeds.get_mut(state.selected_feed_idx) {
                                if let Some(episode) = feed.episodes.get_mut(state.selected_episode_idx) {
                                    episode.download = DownloadState::Downloading { progress: 0.0 };
                                }
                            }
                            dl.start_download(&ep, download_dir);
                            state.set_status(format!("Downloading: {}", ep_title), StatusLevel::Info);
                        }
                        DownloadState::Downloaded { path } => {
                            state.set_status(
                                format!("Already at: {}", path.display()),
                                StatusLevel::Info,
                            );
                        }
                        DownloadState::Downloading { .. } => {
                            state.set_status("Download in progress…", StatusLevel::Info);
                        }
                    }
                }
            }
        }

        Action::PlaybackProgress(pb) => {
            state.playback.position_secs = pb.position_secs;
            state.playback.duration_secs = pb.duration_secs;
            state.playback.status = pb.status;
            state.playback.current_chapter_idx = pb.current_chapter_idx;
        }

        Action::DownloadProgress(id, progress) => {
            for feed in &mut state.feeds {
                if let Some(ep) = feed.episodes.iter_mut().find(|e| e.id == id) {
                    ep.download = DownloadState::Downloading { progress };
                    break;
                }
            }
        }

        Action::DownloadComplete(id, path) => {
            for feed in &mut state.feeds {
                if let Some(ep) = feed.episodes.iter_mut().find(|e| e.id == id) {
                    let fname = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    ep.download = DownloadState::Downloaded { path };
                    state.set_status(format!("Downloaded: {}", fname), StatusLevel::Info);
                    break;
                }
            }
            save_state(state, cfg);
        }

        Action::DownloadError(id, err) => {
            for feed in &mut state.feeds {
                if let Some(ep) = feed.episodes.iter_mut().find(|e| e.id == id) {
                    ep.download = DownloadState::Failed { reason: err.clone() };
                    state.set_status(format!("Download failed: {}", err), StatusLevel::Error);
                    break;
                }
            }
        }

        Action::StatusMessage(text, level) => {
            state.set_status(text, level);
        }

        // Already mapped before reaching process_action
        Action::CrosstermKey(_) => {}
    }
}

fn save_state(state: &AppState, cfg: &mut Config) {
    cfg.feeds = state.feeds.clone();
    config::save(cfg).ok();
}

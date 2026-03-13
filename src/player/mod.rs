use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::events::Action;
use crate::rss::types::Chapter;

#[derive(Debug, Clone, Default)]
pub struct PlaybackState {
    pub episode_id: Option<Uuid>,
    pub episode_title: Option<String>,
    pub status: PlayStatus,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub speed: f32,
    pub volume: u8,
    pub current_chapter_idx: Option<usize>,
    pub chapters: Vec<Chapter>,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            speed: 1.0,
            volume: 80,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum PlayStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

pub enum SeekMode {
    Relative,
    Absolute,
}

pub struct MpvController {
    writer: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    request_id: Arc<AtomicU64>,
}

impl MpvController {
    /// Spawns mpv and returns (controller, read_half_for_polling, child).
    pub async fn spawn(socket_path: &Path) -> Result<(Self, OwnedReadHalf, tokio::process::Child)> {
        info!("spawning mpv with socket {}", socket_path.display());
        let child = tokio::process::Command::new("mpv")
            .arg("--no-video")
            .arg("--idle=yes")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .arg("--really-quiet")
            .arg("--no-terminal")
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn mpv — is it installed?")?;

        info!("mpv spawned (pid {:?}), waiting for socket", child.id());

        let mut attempts = 0u32;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            if socket_path.exists() {
                info!("socket appeared after {}ms", attempts * 150);
                break;
            }
            attempts += 1;
            if attempts > 30 {
                anyhow::bail!("mpv socket did not appear after 5s");
            }
        }

        let stream = UnixStream::connect(socket_path)
            .await
            .context("Failed to connect to mpv IPC socket")?;
        info!("connected to mpv IPC socket");

        // Keep BOTH halves of the same connection
        let (read_half, write_half) = stream.into_split();
        let writer = Arc::new(Mutex::new(write_half));
        let request_id = Arc::new(AtomicU64::new(1));

        let ctrl = Self { writer, request_id };

        // observe_property on this connection → events arrive on read_half
        for (id, prop) in [(1, "time-pos"), (2, "duration"), (3, "pause"), (4, "chapter")] {
            if let Err(e) = ctrl.send_raw(&json!({"command": ["observe_property", id, prop]})).await {
                warn!("observe_property {} failed: {}", prop, e);
            } else {
                debug!("observe_property {} ok", prop);
            }
        }

        Ok((ctrl, read_half, child))
    }

    async fn send_raw(&self, cmd: &Value) -> Result<()> {
        let mut line = serde_json::to_string(cmd)?;
        line.push('\n');
        debug!("mpv send: {}", line.trim());
        let mut w = self.writer.lock().await;
        match w.write_all(line.as_bytes()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("mpv write failed: {}", e);
                Err(e.into())
            }
        }
    }

    async fn send(&self, cmd: &Value) -> Result<()> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let mut val = cmd.clone();
        val["request_id"] = json!(id);
        self.send_raw(&val).await
    }

    pub async fn load_file(&self, path: &str) -> Result<()> {
        self.send(&json!({"command": ["loadfile", path, "replace"]})).await
    }

    pub async fn toggle_pause(&self) -> Result<()> {
        self.send(&json!({"command": ["cycle", "pause"]})).await
    }

    pub async fn seek(&self, seconds: f64, mode: SeekMode) -> Result<()> {
        let mode_str = match mode {
            SeekMode::Relative => "relative",
            SeekMode::Absolute => "absolute",
        };
        self.send(&json!({"command": ["seek", seconds, mode_str]})).await
    }

    pub async fn set_speed(&self, speed: f32) -> Result<()> {
        self.send(&json!({"command": ["set_property", "speed", speed]})).await
    }

    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        self.send(&json!({"command": ["set_property", "volume", volume]})).await
    }

    pub async fn next_chapter(&self) -> Result<()> {
        self.send(&json!({"command": ["add", "chapter", 1]})).await
    }

    pub async fn prev_chapter(&self) -> Result<()> {
        self.send(&json!({"command": ["add", "chapter", -1]})).await
    }

    pub async fn quit(&self) -> Result<()> {
        self.send(&json!({"command": ["quit"]})).await
    }
}

/// Reads mpv events from the same connection read_half and sends PlaybackProgress.
pub async fn poll_mpv(read_half: OwnedReadHalf, tx: tokio::sync::mpsc::Sender<Action>) {
    info!("poll_mpv started");
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let mut state = PlaybackState::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                info!("poll_mpv: EOF");
                break;
            }
            Err(e) => {
                info!("poll_mpv read error: {}", e);
                break;
            }
            Ok(_) => {}
        }

        debug!("mpv recv: {}", line.trim());

        let val = match serde_json::from_str::<Value>(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event = val.get("event").and_then(|e| e.as_str());
        match event {
            Some("property-change") => {
                let name = val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let data = &val["data"];
                match name {
                    "time-pos" => {
                        if let Some(pos) = data.as_f64() {
                            state.position_secs = pos;
                        }
                    }
                    "duration" => {
                        if let Some(dur) = data.as_f64() {
                            state.duration_secs = dur;
                        }
                    }
                    "pause" => {
                        if let Some(paused) = data.as_bool() {
                            state.status = if paused {
                                PlayStatus::Paused
                            } else {
                                PlayStatus::Playing
                            };
                            info!("mpv pause={}", paused);
                        }
                    }
                    "chapter" => {
                        state.current_chapter_idx = data.as_u64().map(|c| c as usize);
                    }
                    _ => {}
                }
                let _ = tx.try_send(Action::PlaybackProgress(state.clone()));
            }
            Some("end-file") => {
                info!("mpv end-file: {:?}", val.get("reason"));
                state.status = PlayStatus::Stopped;
                state.position_secs = 0.0;
                let _ = tx.try_send(Action::PlaybackProgress(state.clone()));
            }
            Some("start-file") => {
                info!("mpv start-file");
                state.status = PlayStatus::Playing;
            }
            Some(other) => {
                debug!("mpv event: {}", other);
            }
            None => {}
        }
    }
}

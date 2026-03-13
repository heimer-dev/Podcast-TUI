use anyhow::Result;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::events::Action;
use crate::rss::types::Episode;

pub struct DownloadManager {
    tx: tokio::sync::mpsc::Sender<Action>,
    semaphore: Arc<Semaphore>,
}

impl DownloadManager {
    pub fn new(tx: tokio::sync::mpsc::Sender<Action>) -> Self {
        Self {
            tx,
            semaphore: Arc::new(Semaphore::new(2)),
        }
    }

    pub fn start_download(&self, episode: &Episode, dest_dir: &Path) {
        let episode_id = episode.id;
        let url = episode.audio_url.clone();
        let dest = dest_dir.join(safe_filename(&episode.title));
        let tx = self.tx.clone();
        let semaphore = self.semaphore.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();
            if let Err(e) = download_task(episode_id, url, dest, tx.clone()).await {
                let _ = tx.send(Action::DownloadError(episode_id, e.to_string())).await;
            }
        });
    }
}

async fn download_task(
    episode_id: Uuid,
    url: String,
    dest: PathBuf,
    tx: tokio::sync::mpsc::Sender<Action>,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let client = reqwest::Client::builder()
        .user_agent("podcast-tui/0.1")
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let response = client.get(&url).send().await?;
    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&dest).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        downloaded += chunk.len() as u64;
        if total > 0 {
            let progress = downloaded as f32 / total as f32;
            let _ = tx.try_send(Action::DownloadProgress(episode_id, progress));
        }
    }

    let _ = tx.send(Action::DownloadComplete(episode_id, dest)).await;
    Ok(())
}

fn safe_filename(title: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .chars()
        .take(80)
        .collect();
    // Determine extension from common audio formats — default mp3
    format!("{}.mp3", slug)
}

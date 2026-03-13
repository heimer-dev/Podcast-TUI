pub mod types;

use anyhow::Result;
use chrono::DateTime;
use uuid::Uuid;
use crate::rss::types::{DownloadState, Episode, FeedError};

/// Parse itunes:duration strings like "1:23:45", "23:45", or "1234"
fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => s.parse::<u64>().ok(),
        2 => {
            let m = parts[0].parse::<u64>().ok()?;
            let s = parts[1].parse::<u64>().ok()?;
            Some(m * 60 + s)
        }
        3 => {
            let h = parts[0].parse::<u64>().ok()?;
            let m = parts[1].parse::<u64>().ok()?;
            let s = parts[2].parse::<u64>().ok()?;
            Some(h * 3600 + m * 60 + s)
        }
        _ => None,
    }
}

/// Fetch and parse an RSS feed. Returns (feed_title, episodes).
pub async fn fetch_feed(feed_id: Uuid, url: &str) -> Result<(String, Vec<Episode>), FeedError> {
    let client = reqwest::Client::builder()
        .user_agent("podcast-tui/0.1 (https://github.com/podcast-tui)")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let response = client.get(url).send().await?.bytes().await?;
    let channel = rss::Channel::read_from(&response[..])
        .map_err(|e| FeedError::Parse(e.to_string()))?;

    let feed_title = channel.title().to_string();
    let mut episodes = Vec::new();

    for item in channel.items() {
        let guid = item
            .guid()
            .map(|g| g.value().to_string())
            .or_else(|| item.title().map(|t| t.to_string()))
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let audio_url = match item.enclosure() {
            Some(enc) => enc.url().to_string(),
            None => continue, // skip episodes without audio
        };

        let title = item.title().unwrap_or("(Untitled)").to_string();
        let description = item.description().map(|d| {
            // Strip basic HTML tags
            let re = d.replace('<', " <");
            strip_html(&re)
        });

        let published = item
            .pub_date()
            .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
            .map(|d| d.with_timezone(&chrono::Utc));

        let duration_secs = item
            .itunes_ext()
            .and_then(|ext| ext.duration())
            .and_then(|d| parse_duration(d));

        episodes.push(Episode {
            id: Uuid::new_v4(),
            feed_id,
            guid,
            title,
            description,
            audio_url,
            published,
            duration_secs,
            is_new: true,
            download: DownloadState::NotDownloaded,
            listen_progress_secs: 0,
            chapters: Vec::new(),
        });
    }

    Ok((feed_title, episodes))
}

/// Merge freshly fetched episodes into existing ones, preserving local state.
pub fn merge_episodes(existing: &mut Vec<Episode>, mut fresh: Vec<Episode>) {
    // Mark existing guids
    let known: std::collections::HashMap<String, usize> = existing
        .iter()
        .enumerate()
        .map(|(i, e)| (e.guid.clone(), i))
        .collect();

    for ep in fresh.iter_mut() {
        if let Some(&idx) = known.get(&ep.guid) {
            // Preserve local state
            ep.id = existing[idx].id;
            ep.is_new = existing[idx].is_new;
            ep.download = existing[idx].download.clone();
            ep.listen_progress_secs = existing[idx].listen_progress_secs;
            ep.chapters = existing[idx].chapters.clone();
        }
    }

    *existing = fresh;
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // Collapse whitespace
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

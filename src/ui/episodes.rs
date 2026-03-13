use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Row, Table, TableState},
    Frame,
};

use crate::app::{AppState, Focus};
use crate::rss::types::DownloadState;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, table_state: &mut TableState) {
    let focused = state.focus == Focus::Episodes;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title_style = if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .title(Span::styled(" EPISODES ", title_style))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let episodes = state.current_episodes();

    if episodes.is_empty() {
        let msg = if state.feeds.is_empty() {
            "No feeds yet — press [a] to add one"
        } else {
            "No episodes — press [r] to refresh"
        };
        let empty = ratatui::widgets::Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center)
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    // Calculate available width for title column
    let fixed_cols = 6 + 7 + 8 + 4; // badge + duration + date + download indicator
    let title_width = (area.width as usize)
        .saturating_sub(fixed_cols + 4) // 4 for borders/padding
        .max(10) as u16;

    let rows: Vec<Row> = episodes
        .iter()
        .enumerate()
        .map(|(i, ep)| {
            let is_selected = i == state.selected_episode_idx;
            let is_playing = Some(ep.id) == state.playback.episode_id;

            let row_style = if is_selected && focused {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            // Play indicator
            let play_icon = if is_playing {
                Span::styled("▶ ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            };

            // Title
            let title = truncate(&ep.title, title_width as usize);
            let title_style = if is_playing {
                Style::default().fg(Color::Green)
            } else if ep.is_new {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            // New badge
            let badge = if ep.is_new {
                Span::styled("[NEW]", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("     ")
            };

            // Duration
            let dur_str = ep
                .duration_secs
                .map(format_duration)
                .unwrap_or_else(|| "     ".to_string());
            let duration = Span::styled(
                format!("{:>6}", dur_str),
                Style::default().fg(Color::DarkGray),
            );

            // Date
            let date_str = ep
                .published
                .map(|d| d.format("%b %d").to_string())
                .unwrap_or_else(|| "      ".to_string());
            let date = Span::styled(
                format!("{:>8}", date_str),
                Style::default().fg(Color::DarkGray),
            );

            // Download indicator
            let dl = match &ep.download {
                DownloadState::NotDownloaded => Span::raw("    "),
                DownloadState::Downloading { progress } => {
                    Span::styled(format!("{:3.0}%", progress * 100.0), Style::default().fg(Color::Blue))
                }
                DownloadState::Downloaded { .. } => {
                    Span::styled(" ✓  ", Style::default().fg(Color::Green))
                }
                DownloadState::Failed { .. } => {
                    Span::styled(" ✗  ", Style::default().fg(Color::Red))
                }
            };

            Row::new(vec![
                ratatui::text::Text::from(Line::from(vec![play_icon, Span::styled(title, title_style)])),
                ratatui::text::Text::from(Line::from(badge)),
                ratatui::text::Text::from(Line::from(duration)),
                ratatui::text::Text::from(Line::from(date)),
                ratatui::text::Text::from(Line::from(dl)),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(title_width),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(4),
        ],
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(table, area, table_state);
}

fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}min", secs / 60)
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

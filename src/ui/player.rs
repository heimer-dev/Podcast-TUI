use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::player::PlayStatus;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let playback = &state.playback;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title + status
            Constraint::Length(1), // progress bar
            Constraint::Min(0),    // hints
        ])
        .split(inner);

    // --- Row 1: Title + Speed + Volume ---
    let title = playback
        .episode_title
        .as_deref()
        .unwrap_or("No episode playing");
    let status_icon = match playback.status {
        PlayStatus::Playing => "▶",
        PlayStatus::Paused => "⏸",
        PlayStatus::Stopped => "⏹",
    };
    let speed_vol = format!(
        "[{:.1}x] ♪ {}%",
        playback.speed, playback.volume
    );

    let available_title = (chunks[0].width as usize)
        .saturating_sub(speed_vol.len() + 3);
    let title_truncated = truncate(title, available_title);

    let title_style = match playback.status {
        PlayStatus::Playing => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        PlayStatus::Paused => Style::default().fg(Color::Yellow),
        PlayStatus::Stopped => Style::default().fg(Color::DarkGray),
    };

    let title_line = Line::from(vec![
        Span::styled(format!("{} ", status_icon), title_style),
        Span::styled(title_truncated, title_style),
    ]);

    let speed_span = Span::styled(speed_vol, Style::default().fg(Color::Cyan));
    let speed_line = Line::from(vec![speed_span]);

    let row1_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(15)])
        .split(chunks[0]);

    frame.render_widget(Paragraph::new(title_line), row1_chunks[0]);
    frame.render_widget(
        Paragraph::new(speed_line).alignment(Alignment::Right),
        row1_chunks[1],
    );

    // --- Row 2: Progress gauge ---
    let ratio = if playback.duration_secs > 0.0 {
        (playback.position_secs / playback.duration_secs).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let pos_str = format_time(playback.position_secs as u64);
    let dur_str = format_time(playback.duration_secs as u64);
    let label = format!("{} / {}", pos_str, dur_str);

    // Chapter info
    let chapter_label = if let Some(idx) = playback.current_chapter_idx {
        if let Some(ch) = playback.chapters.get(idx) {
            format!(" § {}", truncate(&ch.title, 20))
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(ratio)
        .label(format!("{}{}", label, chapter_label));

    frame.render_widget(gauge, chunks[1]);

    // --- Row 3: Key hints ---
    if chunks[2].height > 0 {
        let hints = "[Space] ▶/⏸  [h/l] seek  [+/-] vol  [</>] speed  [[]]] chapter  [a] add feed  [r] refresh  [?] help  [q] quit";
        let hints_p = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(hints_p, chunks[2]);
    }
}

fn format_time(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
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

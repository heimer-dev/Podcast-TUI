use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{AppState, Focus};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, list_state: &mut ListState) {
    let focused = state.focus == Focus::Feeds;
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
        .title(Span::styled(" FEEDS ", title_style))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let items: Vec<ListItem> = state
        .feeds
        .iter()
        .enumerate()
        .map(|(i, feed)| {
            let is_selected = i == state.selected_feed_idx;
            let is_playing = state
                .playback
                .episode_id
                .map(|_| {
                    state.feeds[i]
                        .episodes
                        .iter()
                        .any(|e| Some(e.id) == state.playback.episode_id)
                })
                .unwrap_or(false);

            let prefix = if is_playing {
                Span::styled("▶ ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            };

            let name_style = if is_selected && focused {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };

            let new_count = feed.episodes.iter().filter(|e| e.is_new).count();
            let badge = if new_count > 0 {
                format!(" [{}]", new_count)
            } else {
                String::new()
            };

            let title = truncate(&feed.title, (area.width as usize).saturating_sub(8));
            ListItem::new(Line::from(vec![
                prefix,
                Span::styled(format!("{}{}", title, badge), name_style),
            ]))
        })
        .chain(std::iter::once({
            // "[+] Add Feed" entry
            let add_idx = state.feeds.len();
            let is_selected = add_idx == state.selected_feed_idx;
            let style = if is_selected && focused {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Yellow)
            };
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("[+] Add Feed", style),
            ]))
        }))
        .collect();

    let list = List::new(items).block(block);
    frame.render_stateful_widget(list, area, list_state);
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

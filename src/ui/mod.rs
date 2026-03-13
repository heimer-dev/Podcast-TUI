pub mod episodes;
pub mod feeds;
pub mod player;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{AppState, InputMode, StatusLevel};

pub struct UiState {
    pub feeds_list_state: ratatui::widgets::ListState,
    pub episodes_table_state: ratatui::widgets::TableState,
}

impl UiState {
    pub fn new() -> Self {
        let mut feeds = ratatui::widgets::ListState::default();
        feeds.select(Some(0));
        let mut episodes = ratatui::widgets::TableState::default();
        episodes.select(Some(0));
        Self {
            feeds_list_state: feeds,
            episodes_table_state: episodes,
        }
    }

    pub fn sync_feed_selection(&mut self, state: &AppState) {
        let idx = state.selected_feed_idx;
        self.feeds_list_state.select(Some(idx));
    }

    pub fn sync_episode_selection(&mut self, state: &AppState) {
        let idx = state.selected_episode_idx;
        self.episodes_table_state.select(Some(idx));
    }
}

pub fn render(frame: &mut Frame, state: &AppState, ui_state: &mut UiState) {
    let size = frame.area();

    // Vertical split: title(1) | main(fill) | player(4)
    let player_height = if size.height > 8 { 4 } else { 3 };
    let title_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(player_height),
        ])
        .split(size);

    // --- Title bar ---
    render_title_bar(frame, title_chunks[0], state);

    // --- Main area: feeds | episodes ---
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(title_chunks[1]);

    ui_state.sync_feed_selection(state);
    ui_state.sync_episode_selection(state);

    feeds::render(
        frame,
        main_chunks[0],
        state,
        &mut ui_state.feeds_list_state,
    );
    episodes::render(
        frame,
        main_chunks[1],
        state,
        &mut ui_state.episodes_table_state,
    );

    // --- Player bar ---
    player::render(frame, title_chunks[2], state);

    // --- Overlays ---
    if state.input_mode == InputMode::AddFeedUrl {
        render_input_modal(frame, size, state);
    } else if state.show_help {
        render_help_modal(frame, size);
    }

    if let Some(msg) = &state.status_message {
        if msg.expires_at > std::time::Instant::now() {
            render_status(frame, size, msg);
        }
    }
}

fn render_help_modal(frame: &mut Frame, area: Rect) {
    const W: u16 = 52;
    const H: u16 = 26;
    let x = area.width.saturating_sub(W) / 2;
    let y = area.height.saturating_sub(H) / 2;
    let modal = Rect::new(x, y, W.min(area.width), H.min(area.height));

    frame.render_widget(Clear, modal);

    let block = Block::default()
        .title(Span::styled(
            " Tastenkürzel — [?] oder [Esc] schließen ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let key = |k: &str| Span::styled(format!("{:>10}", k), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let sep = || Span::raw("  ");
    let desc = |d: &str| Span::styled(d.to_string(), Style::default().fg(Color::White));
    let header = |t: &str| Line::from(Span::styled(
        format!("  {}", t),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ));
    let blank = || Line::from("");

    let lines = vec![
        blank(),
        header("Navigation"),
        Line::from(vec![key("Tab"),       sep(), desc("Fokus: Feeds ↔ Episoden")]),
        Line::from(vec![key("j / k"),     sep(), desc("Liste hoch / runter")]),
        Line::from(vec![key("↑ / ↓"),     sep(), desc("Liste hoch / runter")]),
        Line::from(vec![key("Enter"),     sep(), desc("Episode abspielen")]),
        blank(),
        header("Feed-Verwaltung"),
        Line::from(vec![key("a"),         sep(), desc("Feed hinzufügen (URL)")]),
        Line::from(vec![key("d"),         sep(), desc("Feed löschen")]),
        Line::from(vec![key("r"),         sep(), desc("Feed aktualisieren")]),
        Line::from(vec![key("R"),         sep(), desc("Alle Feeds aktualisieren")]),
        blank(),
        header("Wiedergabe"),
        Line::from(vec![key("Space"),     sep(), desc("Play / Pause")]),
        Line::from(vec![key("l / →"),     sep(), desc("10 Sek. vor")]),
        Line::from(vec![key("h / ←"),     sep(), desc("10 Sek. zurück")]),
        Line::from(vec![key("L / H"),     sep(), desc("1 Min. vor / zurück")]),
        Line::from(vec![key("+ / -"),     sep(), desc("Lautstärke ±5%")]),
        Line::from(vec![key("> / <"),     sep(), desc("Geschwindigkeit ±0.25x")]),
        Line::from(vec![key("] / ["),     sep(), desc("Nächstes / Voriges Kapitel")]),
        blank(),
        header("Download & App"),
        Line::from(vec![key("D"),         sep(), desc("Episode herunterladen")]),
        Line::from(vec![key("?"),         sep(), desc("Diese Hilfe öffnen/schließen")]),
        Line::from(vec![key("q"),         sep(), desc("Beenden")]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_title_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let feed_count = state.feeds.len();
    let right = format!("{} feed(s) | [?] hilfe [q] quit", feed_count);
    let left = " ♪  PodcastTUI";

    let bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right.len() as u16 + 1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(left).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        bar[0],
    );
    frame.render_widget(
        Paragraph::new(right.as_str())
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Right),
        bar[1],
    );
}

fn render_input_modal(frame: &mut Frame, area: Rect, state: &AppState) {
    let modal_width = 60.min(area.width.saturating_sub(4));
    let modal_height = 5;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(Span::styled(
            " Add Feed — paste RSS URL ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let text = format!("{}█", state.input_buffer);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(text, Style::default().fg(Color::White))),
        Line::from(Span::styled(
            "[Enter] confirm   [Esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_status(frame: &mut Frame, area: Rect, msg: &crate::app::StatusMessage) {
    let color = match msg.level {
        StatusLevel::Info => Color::Cyan,
        StatusLevel::Warning => Color::Yellow,
        StatusLevel::Error => Color::Red,
    };

    let text_len = msg.text.len() as u16 + 4;
    let width = text_len.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(5);
    let status_area = Rect::new(x, y, width, 1);

    frame.render_widget(Clear, status_area);
    frame.render_widget(
        Paragraph::new(format!(" {} ", msg.text))
            .style(Style::default().fg(Color::Black).bg(color))
            .alignment(ratatui::layout::Alignment::Center),
        status_area,
    );
}

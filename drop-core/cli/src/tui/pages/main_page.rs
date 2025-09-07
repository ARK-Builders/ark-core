use crate::tui::app::{App, Page};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_main_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Welcome message
            Constraint::Min(0),    // Menu
        ])
        .split(area);

    // Welcome message
    let welcome = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Welcome to ", Style::default()),
            Span::styled(
                "ARK Drop",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from("A secure file transfer tool"),
        Line::from(""),
        Line::from("Choose an option below:"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Welcome"),
    )
    .style(Style::default().fg(Color::White));

    f.render_widget(welcome, chunks[0]);

    // Main menu
    let menu_items = vec![
        ListItem::new("📤 Send Files").style(Style::default().fg(Color::Green)),
        ListItem::new("📥 Receive Files")
            .style(Style::default().fg(Color::Blue)),
        ListItem::new("⚙️  Configuration")
            .style(Style::default().fg(Color::Yellow)),
        ListItem::new("❓ Help").style(Style::default().fg(Color::Magenta)),
    ];

    let menu = List::new(menu_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Main Menu"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("→ ");

    f.render_stateful_widget(menu, chunks[1], &mut app.main_menu_state);
}

pub async fn handle_main_page_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Up => {
            let selected = app.main_menu_state.selected().unwrap_or(0);
            if selected > 0 {
                app.main_menu_state.select(Some(selected - 1));
            } else {
                app.main_menu_state.select(Some(3)); // Wrap to bottom
            }
        }
        KeyCode::Down => {
            let selected = app.main_menu_state.selected().unwrap_or(0);
            if selected < 3 {
                app.main_menu_state.select(Some(selected + 1));
            } else {
                app.main_menu_state.select(Some(0)); // Wrap to top
            }
        }
        KeyCode::Enter => match app.main_menu_state.selected() {
            Some(0) => app.navigate_to(Page::Send),
            Some(1) => app.navigate_to(Page::Receive),
            Some(2) => app.navigate_to(Page::Config),
            Some(3) => app.navigate_to(Page::Help),
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

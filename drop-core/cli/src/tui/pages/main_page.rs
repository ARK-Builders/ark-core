use crate::tui::app::{App, Page};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_main_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(50), // Left side - welcome & info
            Constraint::Percentage(50), // Right side - menu
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Welcome message
            Constraint::Length(6), // Features
            Constraint::Min(0),    // Status info
        ])
        .split(chunks[0]);

    // Welcome message
    let welcome_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("👋 ", Style::default().fg(Color::Yellow)),
            Span::styled("Welcome to ", Style::default().fg(Color::White)),
            Span::styled("ARK Drop", Style::default().fg(Color::Cyan).bold()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("🔒 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Secure peer-to-peer file transfer",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Fast and reliable connections",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
    ];

    let welcome_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" About ")
        .title_style(Style::default().fg(Color::White).bold());

    let welcome = Paragraph::new(welcome_content)
        .block(welcome_block)
        .alignment(Alignment::Left);
    f.render_widget(welcome, left_chunks[0]);

    // Features overview
    let features_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "No file size limits",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Green)),
            Span::styled(
                "End-to-end encryption",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Blue)),
            Span::styled(
                "Works across networks",
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let features_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Green))
        .title(" Features ")
        .title_style(Style::default().fg(Color::White).bold());

    let features = Paragraph::new(features_content)
        .block(features_block)
        .alignment(Alignment::Left);
    f.render_widget(features, left_chunks[1]);

    // Status information
    let status_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("🟢 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Ready to transfer files",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Get started by choosing an option →",
            Style::default().fg(Color::Gray).italic(),
        )]),
    ];

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Status ")
        .title_style(Style::default().fg(Color::White).bold());

    let status = Paragraph::new(status_content)
        .block(status_block)
        .alignment(Alignment::Left);
    f.render_widget(status, left_chunks[2]);

    // Main menu
    let menu_items = vec![
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("📤 ", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    "Send Files",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Share files with others",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("📥 ", Style::default().fg(Color::Blue).bold()),
                Span::styled(
                    "Receive Files",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Download files from sender",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("⚙️ ", Style::default().fg(Color::Yellow).bold()),
                Span::styled(
                    "Configuration",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Adjust settings and preferences",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("❓ ", Style::default().fg(Color::Magenta).bold()),
                Span::styled("Help", Style::default().fg(Color::White).bold()),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "View help and keyboard shortcuts",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
    ];

    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::White))
        .title(" Main Menu ")
        .title_style(Style::default().fg(Color::White).bold());

    let menu = List::new(menu_items)
        .block(menu_block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

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

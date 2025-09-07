use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_config_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Current settings
            Constraint::Min(0),    // Menu
        ])
        .split(area);

    // Title
    let title = Paragraph::new("⚙️ Configuration")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Current settings
    let default_dir = app
        .default_receive_dir
        .as_deref()
        .unwrap_or("Not set");

    let settings = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Default receive directory: ", Style::default()),
            Span::styled(default_dir, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from("Use the options below to manage settings:"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Current Settings"),
    );
    f.render_widget(settings, chunks[1]);

    // Configuration menu
    let menu_items = vec![
        ListItem::new("📁 Set Default Receive Directory")
            .style(Style::default().fg(Color::Cyan)),
        ListItem::new("🗑️  Clear Default Receive Directory")
            .style(Style::default().fg(Color::Red)),
        ListItem::new("📋 Show All Settings")
            .style(Style::default().fg(Color::Green)),
    ];

    let menu = List::new(menu_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configuration Options"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("→ ");

    f.render_stateful_widget(menu, chunks[2], &mut app.config_menu_state);
}

pub async fn handle_config_page_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Up => {
            let selected = app.config_menu_state.selected().unwrap_or(0);
            if selected > 0 {
                app.config_menu_state.select(Some(selected - 1));
            } else {
                app.config_menu_state.select(Some(2)); // Wrap to bottom
            }
        }
        KeyCode::Down => {
            let selected = app.config_menu_state.selected().unwrap_or(0);
            if selected < 2 {
                app.config_menu_state.select(Some(selected + 1));
            } else {
                app.config_menu_state.select(Some(0)); // Wrap to top
            }
        }
        KeyCode::Enter => {
            match app.config_menu_state.selected() {
                Some(0) => {
                    // Set default receive directory - in a real implementation,
                    // you'd show an input dialog or file browser
                    app.show_error("Feature not implemented in TUI mode yet. Use CLI: arkdrop config set-receive-dir <path>".to_string());
                }
                Some(1) => {
                    // Clear default receive directory
                    if let Err(e) = arkdrop::clear_default_receive_dir() {
                        app.show_error(format!(
                            "Failed to clear default directory: {}",
                            e
                        ));
                    } else {
                        app.default_receive_dir = None;
                        app.show_success(
                            "Default receive directory cleared successfully"
                                .to_string(),
                        );
                    }
                }
                Some(2) => {
                    // Show all settings
                    let settings_info = format!(
                        "Current Settings:\n\nDefault receive directory: {}\n\nNote: More settings will be available in future versions.",
                        app.default_receive_dir
                            .as_deref()
                            .unwrap_or("Not set")
                    );
                    app.show_success(settings_info);
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

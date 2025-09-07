use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_config_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(60), // Left side - current settings
            Constraint::Percentage(40), // Right side - configuration menu
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(8), // Current settings
            Constraint::Min(0),    // Additional info
        ])
        .split(main_chunks[0]);

    // Enhanced page title
    let title_content = vec![Line::from(vec![
        Span::styled("⚙️ ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("Configuration", Style::default().fg(Color::White).bold()),
    ])];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Settings Management ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);
    f.render_widget(title, left_chunks[0]);

    // Current settings
    let default_dir = app
        .default_receive_dir
        .as_deref()
        .unwrap_or("Not configured");

    let settings_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("📂 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Default Download Location:",
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                default_dir,
                if app.default_receive_dir.is_some() {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray).italic()
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Configure settings using the menu →",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
    ];

    let settings_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Current Settings ")
        .title_style(Style::default().fg(Color::White).bold());

    let settings = Paragraph::new(settings_content)
        .block(settings_block)
        .alignment(Alignment::Left);
    f.render_widget(settings, left_chunks[1]);

    // Additional configuration info
    let info_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
            Span::styled(
                "Configuration Notes",
                Style::default().fg(Color::Blue).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Settings are saved automatically",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Default directory applies to all transfers",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Gray)),
            Span::styled(
                "You can override on each transfer",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("📝 ", Style::default().fg(Color::Green)),
            Span::styled(
                "More settings coming soon!",
                Style::default().fg(Color::Green),
            ),
        ]),
    ];

    let info_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Information ")
        .title_style(Style::default().fg(Color::White).bold());

    let info = Paragraph::new(info_content)
        .block(info_block)
        .alignment(Alignment::Left);
    f.render_widget(info, left_chunks[2]);

    // Enhanced configuration menu
    let menu_items = vec![
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    "Set Default Directory",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Configure where files are saved",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("🗑️ ", Style::default().fg(Color::Red).bold()),
                Span::styled(
                    "Clear Default Directory",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Remove saved default location",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("📋 ", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    "View All Settings",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Display complete configuration",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
        ListItem::new(vec![
            Line::from(vec![
                Span::styled("🔄 ", Style::default().fg(Color::Magenta).bold()),
                Span::styled(
                    "Reset All Settings",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    "Restore factory defaults",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]),
    ];

    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::White))
        .title(" Configuration Options ")
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

    f.render_stateful_widget(menu, main_chunks[1], &mut app.config_menu_state);
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
                app.config_menu_state.select(Some(3)); // Wrap to bottom
            }
        }
        KeyCode::Down => {
            let selected = app.config_menu_state.selected().unwrap_or(0);
            if selected < 3 {
                app.config_menu_state.select(Some(selected + 1));
            } else {
                app.config_menu_state.select(Some(0)); // Wrap to top
            }
        }
        KeyCode::Enter => {
            match app.config_menu_state.selected() {
                Some(0) => {
                    // TODO: implement file browser
                }
                Some(1) => {
                    // Clear default receive directory
                    if let Err(e) = arkdrop::clear_default_receive_dir() {
                        app.show_error(format!(
                            "❌ Failed to clear default directory:\n\n{}",
                            e
                        ));
                    } else {
                        app.default_receive_dir = None;
                        app.show_success(
                            "✅ Default receive directory cleared successfully!\n\nFiles will now be saved to the system default location."
                                .to_string(),
                        );
                    }
                }
                Some(2) => {
                    // Show all settings
                    let settings_info = format!(
                        "📋 Current Configuration:\n\n🗂️  Default receive directory:\n   {}\n\n🔧 Application settings:\n   • TUI Mode: Enabled\n   • Auto-update progress: Yes\n   • Secure transfers: Always\n\n💡 Note: More settings will be available in future versions.\n\nUse CLI commands for advanced configuration:\n  arkdrop config --help",
                        app.default_receive_dir
                            .as_deref()
                            .unwrap_or("Not configured (using system default)")
                    );
                    app.show_success(settings_info);
                }
                Some(3) => {
                    // TODO: RESET FUNCTIONALITY
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

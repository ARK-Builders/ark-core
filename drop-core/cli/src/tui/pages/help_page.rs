use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_help_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Help content in columns
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Left column
            Constraint::Percentage(50), // Right column
        ])
        .split(main_chunks[1]);

    // Title
    let title_content = vec![Line::from(vec![
        Span::styled("❓ ", Style::default().fg(Color::Magenta).bold()),
        Span::styled(
            "Help & Documentation",
            Style::default().fg(Color::White).bold(),
        ),
    ])];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" User Guide ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);
    f.render_widget(title, main_chunks[0]);

    // Left column - Navigation and Basic Usage
    let left_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("🧬 ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Navigation Controls",
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "↑/↓ Tab/Shift+Tab",
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(": Navigate", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled("Enter", Style::default().fg(Color::White).bold()),
            Span::styled(": Select option", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled("Esc", Style::default().fg(Color::White).bold()),
            Span::styled(": Go back", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled("Q", Style::default().fg(Color::White).bold()),
            Span::styled(
                ": Quit application",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled("H", Style::default().fg(Color::White).bold()),
            Span::styled(": Show help", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("📤 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Sending Files",
                Style::default().fg(Color::Green).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("1. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Select 'Send Files' from menu",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("2. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Enter file paths to add",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("3. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Set display name (optional)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("4. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Click Send to start transfer",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("5. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Share ticket with receiver",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("📥 ", Style::default().fg(Color::Blue)),
            Span::styled(
                "Receiving Files",
                Style::default().fg(Color::Blue).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("1. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Select 'Receive Files' from menu",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("2. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Enter transfer ticket",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("3. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Enter confirmation code",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("4. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Set output folder (optional)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("5. ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Click Receive to start",
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Quick Start Guide ")
        .title_style(Style::default().fg(Color::White).bold());

    let left_panel = Paragraph::new(left_content)
        .block(left_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(left_panel, content_chunks[0]);

    // Right column - Advanced Features and Tips
    let right_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("⚙️ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Configuration",
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Set default receive directory",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "View current settings",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Clear saved preferences",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("💻 ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Command Line Usage",
                Style::default().fg(Color::Magenta).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Send: ", Style::default().fg(Color::Green).bold()),
            Span::styled(
                "arkdrop send <files>",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Receive: ", Style::default().fg(Color::Blue).bold()),
            Span::styled(
                "arkdrop receive <ticket>",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Config: ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "arkdrop config <option>",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Tips & Best Practices",
                Style::default().fg(Color::Cyan).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                "Files are encrypted end-to-end",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                "Both users must be online",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                "Check available disk space",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                "Stable network recommended",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("🔒 ", Style::default().fg(Color::Red)),
            Span::styled(
                "Security Notes",
                Style::default().fg(Color::Red).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Red)),
            Span::styled(
                "Only share tickets with trusted users",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Red)),
            Span::styled(
                "Tickets expire after use",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Red)),
            Span::styled(
                "Transfers are peer-to-peer",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
            Span::styled(
                "Need more help? Visit our docs",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
    ];

    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Green))
        .title(" Advanced Features & Tips ")
        .title_style(Style::default().fg(Color::White).bold());

    let right_panel = Paragraph::new(right_content)
        .block(right_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(right_panel, content_chunks[1]);
}

pub async fn handle_help_page_input(
    _app: &mut App,
    _key: KeyCode,
) -> Result<()> {
    // Help page doesn't need special input handling
    // Navigation is handled by the main event handler
    Ok(())
}

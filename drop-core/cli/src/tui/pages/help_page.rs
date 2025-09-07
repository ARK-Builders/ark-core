use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_help_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Help content
        ])
        .split(area);

    // Title
    let title = Paragraph::new("❓ Help")
        .style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Help content
    let help_content = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("ARK Drop", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" - Secure File Transfer Tool", Style::default()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("• ↑/↓ or Tab/Shift+Tab: Navigate between options"),
        Line::from("• Enter: Select option or confirm action"),
        Line::from("• Esc: Go back to previous page"),
        Line::from("• Q: Quit application"),
        Line::from("• H: Show this help page (from main menu)"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Sending Files:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("1. Select 'Send Files' from main menu"),
        Line::from("2. Add files by entering their full path"),
        Line::from("3. Optionally set your display name and avatar"),
        Line::from("4. Press Enter on 'Send Files' button to start"),
        Line::from("5. Share the generated ticket and confirmation with receiver"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Receiving Files:", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("1. Select 'Receive Files' from main menu"),
        Line::from("2. Enter the transfer ticket from sender"),
        Line::from("3. Enter the confirmation code from sender"),
        Line::from("4. Optionally set output directory and display name"),
        Line::from("5. Press Enter on 'Receive Files' button to start"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Configuration:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("• Set a default receive directory to avoid specifying it each time"),
        Line::from("• View current configuration settings"),
        Line::from("• Clear previously set defaults"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Command Line Mode:", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("You can also use this tool from command line:"),
        Line::from("• arkdrop send <files> --name \"Your Name\""),
        Line::from("• arkdrop receive <ticket> <confirmation> -o /output/dir"),
        Line::from("• arkdrop config set-receive-dir /default/dir"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tips:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("• Files are transferred securely using encryption"),
        Line::from("• Both sender and receiver must be online during transfer"),
        Line::from("• Large files may take longer to transfer"),
        Line::from("• Ensure you have sufficient disk space for received files"),
    ])
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title("User Guide"));

    f.render_widget(help_content, chunks[1]);
}

pub async fn handle_help_page_input(
    _app: &mut App,
    _key: KeyCode,
) -> Result<()> {
    // Help page doesn't need special input handling
    // Navigation is handled by the main event handler
    Ok(())
}

use std::sync::Arc;

use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{App, AppBackend};

pub struct HelpApp {
    b: Arc<dyn AppBackend>,
}

impl App for HelpApp {
    fn draw(&self, f: &mut Frame, area: Rect) {
        let main_blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Help content in columns
            ])
            .split(area);

        let content_blocks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Left column
                Constraint::Percentage(50), // Right column
            ])
            .split(main_blocks[1]);

        draw_title(f, main_blocks[0]);
        draw_left_content(f, content_blocks[0]);
        draw_right_content(f, content_blocks[1])
    }

    fn handle_control(&self, ev: &Event) {
        if let Event::Key(key) = ev {
            match key.code {
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            }
        }
    }
}

fn draw_title(f: &mut Frame<'_>, area: Rect) {
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

    f.render_widget(title, area);
}

fn draw_left_content(f: &mut Frame<'_>, area: Rect) {
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
            Span::styled("CTRL-Q", Style::default().fg(Color::White).bold()),
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

    f.render_widget(left_panel, area);
}

fn draw_right_content(f: &mut Frame<'_>, area: Rect) {
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
            Span::styled("� ", Style::default().fg(Color::Red)),
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

    f.render_widget(right_panel, area);
}

impl HelpApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self { b }
    }
}

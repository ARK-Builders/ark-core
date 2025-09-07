use crate::tui::app::{App, AppState};
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};

pub fn render_send_progress_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    render_progress_page(f, app, area, "📤 Sending Files", Color::Green);
}

pub fn render_receive_progress_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    render_progress_page(f, app, area, "📥 Receiving Files", Color::Blue);
}

fn render_progress_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
    title: &str,
    color: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(8), // Status info
            Constraint::Length(5), // Progress bar
            Constraint::Min(0),    // Details/logs
            Constraint::Length(3), // Footer
        ])
        .split(area);

    // Title
    let title_widget = Paragraph::new(title)
        .style(
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title_widget, chunks[0]);

    // Status information
    let elapsed_time = if let Some(start_time) = app.operation_start_time {
        let elapsed = start_time.elapsed();
        format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else {
        "00:00".to_string()
    };

    let estimated_remaining = if app.progress_percentage > 0.0
        && app.progress_percentage < 100.0
    {
        let elapsed_secs = app
            .operation_start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let total_estimated = elapsed_secs * 100.0 / app.progress_percentage;
        let remaining = (total_estimated - elapsed_secs).max(0.0);
        format!(
            "~{}:{:02}",
            (remaining as u64) / 60,
            (remaining as u64) % 60
        )
    } else {
        "--:--".to_string()
    };

    let status_info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default()),
            Span::styled(
                match app.state {
                    AppState::Sending => "Sending...",
                    AppState::Receiving => "Receiving...",
                    _ => "Processing...",
                },
                Style::default().fg(color),
            ),
        ]),
        Line::from(vec![
            Span::styled("Progress: ", Style::default()),
            Span::styled(
                format!("{:.1}%", app.progress_percentage),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("Elapsed: ", Style::default()),
            Span::styled(elapsed_time, Style::default().fg(Color::Yellow)),
            Span::styled("  Remaining: ", Style::default()),
            Span::styled(
                estimated_remaining,
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(app.progress_message.as_str()),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Transfer Status"),
    );
    f.render_widget(status_info, chunks[1]);

    // Progress bar
    let progress = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Progress"),
        )
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .percent(app.progress_percentage as u16)
        .label(format!("{:.1}%", app.progress_percentage));
    f.render_widget(progress, chunks[2]);

    // Transfer details
    let details = match app.state {
        AppState::Sending => Paragraph::new(vec![
            Line::from("Files being sent:"),
            Line::from(""),
            Line::from("Transfer Information:"),
            Line::from("• Connection established with receiver"),
            Line::from("• Files are encrypted during transfer"),
            Line::from("• Transfer will complete automatically"),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Note: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Keep this application running until transfer completes",
                    Style::default(),
                ),
            ]),
        ]),
        AppState::Receiving => Paragraph::new(vec![
            Line::from("Files being received:"),
            Line::from(""),
            Line::from("Transfer Information:"),
            Line::from("• Connected to sender"),
            Line::from("• Files are being decrypted and saved"),
            Line::from("• Transfer will complete automatically"),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Note: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Files will be saved to the specified directory",
                    Style::default(),
                ),
            ]),
        ]),
        _ => Paragraph::new("Processing..."),
    }
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Details"),
    )
    .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(details, chunks[3]);

    // Footer
    let footer_text = if app.progress_percentage >= 100.0 {
        "Transfer completed! Press any key to continue..."
    } else {
        "Transfer in progress... Press Q to cancel and quit"
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[4]);
}

pub async fn handle_progress_page_input(
    _app: &mut App,
    _key: KeyCode,
) -> Result<()> {
    // Progress pages don't need special input handling
    // The main handler already handles 'q' for quit
    Ok(())
}

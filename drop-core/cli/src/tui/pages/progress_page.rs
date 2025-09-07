use crate::tui::app::{App, AppState};
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
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
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(12), // Progress section
            Constraint::Min(0),     // Details/logs
            Constraint::Length(4),  // Footer
        ])
        .split(area);

    let progress_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Status info
            Constraint::Percentage(50), // Progress visualization
        ])
        .split(main_chunks[1]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Progress bar
            Constraint::Min(0),    // Transfer stats
        ])
        .split(progress_chunks[1]);

    // Title
    let progress_icon = match (app.progress_percentage as u8) % 4 {
        0 => "◜",
        1 => "◝",
        2 => "◞",
        _ => "◟",
    };

    let title_content = vec![Line::from(vec![
        Span::styled(
            format!("{} ", progress_icon),
            Style::default().fg(color).bold(),
        ),
        Span::styled(title, Style::default().fg(Color::White).bold()),
        Span::styled(
            format!(" {:.1}%", app.progress_percentage),
            Style::default().fg(Color::Cyan),
        ),
    ])];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(color))
        .title(" Transfer in Progress ")
        .title_style(Style::default().fg(Color::White).bold());

    let title_widget = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);
    f.render_widget(title_widget, main_chunks[0]);

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
        format!("{}:{:02}", (remaining as u64) / 60, (remaining as u64) % 60)
    } else {
        "--:--".to_string()
    };

    let status_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("🔄 ", Style::default().fg(color)),
            Span::styled("Status: ", Style::default().fg(Color::White).bold()),
            Span::styled(
                match app.state {
                    AppState::Sending => "Sending Files",
                    AppState::Receiving => "Receiving Files",
                    _ => "Processing",
                },
                Style::default().fg(color).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("⏱️ ", Style::default().fg(Color::Yellow)),
            Span::styled("Elapsed: ", Style::default().fg(Color::White)),
            Span::styled(elapsed_time, Style::default().fg(Color::Cyan).bold()),
        ]),
        Line::from(vec![
            Span::styled("⏰ ", Style::default().fg(Color::Yellow)),
            Span::styled("Remaining: ", Style::default().fg(Color::White)),
            Span::styled(
                estimated_remaining,
                Style::default().fg(Color::Cyan).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("💬 ", Style::default().fg(Color::Blue)),
            Span::styled(
                app.progress_message.as_str(),
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
        Line::from(""),
    ];

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(color))
        .title(" Status ")
        .title_style(Style::default().fg(Color::White).bold());

    let status_info = Paragraph::new(status_content)
        .block(status_block)
        .alignment(Alignment::Left);
    f.render_widget(status_info, progress_chunks[0]);

    // Progress bar
    let progress_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(color))
        .title(" Progress ")
        .title_style(Style::default().fg(Color::White).bold());

    let progress = Gauge::default()
        .block(progress_block)
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .percent(app.progress_percentage as u16)
        .label(Span::styled(
            format!("{:.1}%", app.progress_percentage),
            Style::default().fg(Color::White).bold(),
        ));
    f.render_widget(progress, right_chunks[0]);

    // Transfer statistics
    let files_count = if matches!(app.state, AppState::Sending) {
        app.send_files.len()
    } else {
        0 // In a real implementation, this would track received files
    };

    let stats_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("📁 ", Style::default().fg(Color::Blue)),
            Span::styled("Files: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", files_count),
                Style::default().fg(Color::Cyan).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("📊 ", Style::default().fg(Color::Green)),
            Span::styled("Speed: ", Style::default().fg(Color::White)),
            Span::styled(
                "Calculating...",
                Style::default().fg(Color::Gray).italic(),
            ),
        ]),
    ];

    let stats_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Statistics ")
        .title_style(Style::default().fg(Color::White).bold());

    let stats = Paragraph::new(stats_content)
        .block(stats_block)
        .alignment(Alignment::Left);
    f.render_widget(stats, right_chunks[1]);

    // Transfer details
    let details_content = match app.state {
        AppState::Sending => vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("📤 ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Sending Files",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Connection established with receiver",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Files are encrypted during transfer",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Transfer will complete automatically",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Tip: ",
                    Style::default().fg(Color::Yellow).bold(),
                ),
                Span::styled(
                    "Keep this application running until transfer completes",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Red)),
                Span::styled(
                    "Do not close this window during transfer",
                    Style::default().fg(Color::LightRed),
                ),
            ]),
        ],
        AppState::Receiving => vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("📥 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Receiving Files",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Connected to sender",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Files are being decrypted and saved",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Transfer will complete automatically",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💾 ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "Files will be saved to: ",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    if app.receive_output_dir.is_empty() {
                        app.default_receive_dir
                            .as_deref()
                            .unwrap_or("~/Downloads/ARK-Drop")
                    } else {
                        &app.receive_output_dir
                    },
                    Style::default().fg(Color::Cyan).italic(),
                ),
            ]),
        ],
        _ => vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Processing transfer...",
                    Style::default().fg(Color::White),
                ),
            ]),
        ],
    };

    let details_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::White))
        .title(" Transfer Details ")
        .title_style(Style::default().fg(Color::White).bold());

    let details = Paragraph::new(details_content)
        .block(details_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(details, main_chunks[2]);

    // Footer
    let (footer_text, footer_color, footer_icon) =
        if app.progress_percentage >= 100.0 {
            (
                "Transfer completed! Press ESC to continue...",
                Color::Green,
                "✅",
            )
        } else {
            (
                "Transfer in progress... Press Q to cancel",
                Color::Yellow,
                "⏳",
            )
        };

    let footer_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{} ", footer_icon),
                Style::default().fg(footer_color),
            ),
            Span::styled(footer_text, Style::default().fg(Color::White)),
        ]),
    ];

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(footer_color))
        .title(" Status ")
        .title_style(Style::default().fg(Color::White).bold());

    let footer = Paragraph::new(footer_content)
        .block(footer_block)
        .alignment(Alignment::Center);
    f.render_widget(footer, main_chunks[3]);
}

pub async fn handle_progress_page_input(
    _app: &mut App,
    _key: KeyCode,
) -> Result<()> {
    // Progress pages don't need special input handling
    // The main handler already handles 'q' for quit
    Ok(())
}

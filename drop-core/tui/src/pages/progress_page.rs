use crate::{App, AppState, components::qr_code::render_qr_code_widget};
use arkdrop_common::get_default_out_dir;
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
            Constraint::Min(0),     // Details/logs or QR code
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
    let progress_icon =
        match (app.progress_percentage.read().unwrap().clone() as u8) % 4 {
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
            format!(" {:.1}%", app.progress_percentage.read().unwrap().clone()),
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
    let elapsed_time = if let Some(start_time) =
        app.operation_start_time.read().unwrap().clone()
    {
        let elapsed = start_time.elapsed();
        format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else {
        "00:00".to_string()
    };

    let estimated_remaining = if app.progress_percentage.read().unwrap().clone()
        > 0.0
        && app.progress_percentage.read().unwrap().clone() < 100.0
    {
        let elapsed_secs = app
            .operation_start_time
            .read()
            .unwrap()
            .clone()
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let total_estimated = elapsed_secs * 100.0
            / app.progress_percentage.read().unwrap().clone();
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
                match app.state.read().unwrap().clone() {
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
                app.progress_message.read().unwrap().clone(),
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
        .percent(app.progress_percentage.read().unwrap().clone() as u16)
        .label(Span::styled(
            format!("{:.1}%", app.progress_percentage.read().unwrap().clone()),
            Style::default().fg(Color::White).bold(),
        ));
    f.render_widget(progress, right_chunks[0]);

    // Transfer statistics
    let files_count =
        if matches!(app.state.read().unwrap().clone(), AppState::Sending) {
            app.sender_files.read().unwrap().len()
        } else {
            0 // TODO: this should track received files
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

    // Transfer details or QR Code for sender
    match app.state.read().unwrap().clone() {
        AppState::Sending => {
            // Check if we have a ticket and confirmation to display QR code
            if let Some(bubble) = app.send_files_bubble.read().unwrap().as_ref()
            {
                let qr_data = format!(
                    "drop://receive?ticket={}&confirmation={}",
                    bubble.get_ticket(),
                    bubble.get_confirmation()
                );

                // Split the area for QR code and details
                let qr_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50), // Details
                        Constraint::Percentage(50), // QR Code
                    ])
                    .split(main_chunks[2]);

                // Details on the left
                let details_content = vec![
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
                        Span::styled("🔑 ", Style::default().fg(Color::Blue)),
                        Span::styled(
                            "Transfer Ticket: ",
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            bubble.get_ticket(),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("🔒 ", Style::default().fg(Color::Blue)),
                        Span::styled(
                            "Confirmation Code: ",
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            bubble.get_confirmation().to_string(),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            "Share QR Code or ticket with receiver",
                            Style::default().fg(Color::Gray),
                        ),
                    ]),
                ];

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
                f.render_widget(details, qr_chunks[0]);

                // QR Code on the right
                render_qr_code_widget(
                    f,
                    &qr_data,
                    qr_chunks[1],
                    " Transfer QR Code ",
                    Color::Green,
                )
                .ok();
            } else {
                // Fallback to regular details if no ticket/confirmation
                let details_content = vec![
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
                ];

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
            }
        }
        AppState::Receiving => {
            let details_content = vec![
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
                    Span::styled("💾 ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        "Files will be saved to: ",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        get_default_out_dir()
                            .to_string_lossy()
                            .to_string(),
                        Style::default().fg(Color::Cyan).italic(),
                    ),
                ]),
            ];

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
        }
        _ => {
            let details_content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "Processing transfer...",
                        Style::default().fg(Color::White),
                    ),
                ]),
            ];

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
        }
    }

    // Footer
    let (footer_text, footer_color, footer_icon) = match app
        .state
        .read()
        .unwrap()
        .clone()
    {
        AppState::Sending => {
            if let Some(bubble) = app.send_files_bubble.read().unwrap().as_ref()
            {
                if app.progress_percentage.read().unwrap().clone() >= 100.0 {
                    (
                        "Transfer completed! Press ESC to continue..."
                            .to_string(),
                        Color::Green,
                        "✅",
                    )
                } else {
                    (
                        format!(
                            "Transfer code: {} {}",
                            bubble.get_ticket(),
                            bubble.get_confirmation()
                        )
                        .to_string(),
                        Color::Blue,
                        "🔑",
                    )
                }
            } else if app.progress_percentage.read().unwrap().clone() >= 100.0 {
                (
                    "Transfer completed! Press ESC to continue...".to_string(),
                    Color::Green,
                    "✅",
                )
            } else {
                (
                    "Transfer in progress... Press Q to cancel".to_string(),
                    Color::Yellow,
                    "⏳",
                )
            }
        }
        AppState::Receiving => {
            if app.progress_percentage.read().unwrap().clone() >= 100.0 {
                (
                    "Transfer completed! Press ESC to continue...".to_string(),
                    Color::Green,
                    "✅",
                )
            } else {
                (
                    "Transfer in progress... Press Q to cancel".to_string(),
                    Color::Blue,
                    "⏳",
                )
            }
        }
        _ => ("Processing...".to_string(), Color::Gray, "⏳"),
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

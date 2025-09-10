use std::{
    ops::Div,
    sync::{Arc, RwLock},
};

use crate::{App, AppState, components::qr_code::render_qr_code_widget};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_send_progress_page(
    f: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
) {
    render_progress_page(f, app, area, "📤 Sending Files", Color::Green);
}

pub fn render_receive_progress_page(
    f: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
) {
    render_progress_page(f, app, area, "📥 Receiving Files", Color::Blue);
}

fn render_progress_page(
    f: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    title: &str,
    color: Color,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(36),   // Content
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
    let pct = app
        .read()
        .unwrap()
        .transfer_files
        .iter()
        .map(|f| f.get_pct())
        .sum::<f64>()
        .div(app.read().unwrap().transfer_files.len().into());
    let progress_icon = match (pct as u8) % 4 {
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
            format!(
                " {:.1}%",
                app.read()
                    .unwrap()
                    .progress_percentage
                    .read()
                    .unwrap()
                    .clone()
            ),
            Style::default().fg(Color::Cyan),
        ),
    ])];

    let is_sender_connected = app
        .read()
        .unwrap()
        .sender_connected
        .read()
        .unwrap()
        .clone();
    let is_receiver_connected = app
        .read()
        .unwrap()
        .receiver_connected
        .read()
        .unwrap()
        .clone();

    // Header title
    let title_text = match is_sender_connected | is_receiver_connected {
        true => " Transfer in Progress ",
        false => " Waiting for Peer ",
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(color))
        .title(title_text)
        .title_style(Style::default().fg(Color::White).bold());

    let title_widget = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);

    f.render_widget(title_widget, main_chunks[0]);

    // Status information
    let elapsed_time = if let Some(start_time) = app
        .read()
        .unwrap()
        .transfer_start_time
        .read()
        .unwrap()
        .clone()
    {
        let elapsed = start_time.elapsed();
        format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else {
        "00:00".to_string()
    };

    let estimated_remaining = if app
        .read()
        .unwrap()
        .progress_percentage
        .read()
        .unwrap()
        .clone()
        > 0.0
        && app
            .read()
            .unwrap()
            .progress_percentage
            .read()
            .unwrap()
            .clone()
            < 100.0
    {
        let elapsed_secs = app
            .transfer_start_time
            .read()
            .unwrap()
            .clone()
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let total_estimated = elapsed_secs * 100.0
            / app
                .read()
                .unwrap()
                .progress_percentage
                .read()
                .unwrap()
                .clone();
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
                if is_sender_connected | is_receiver_connected {
                    match app.read().unwrap().state.read().unwrap().clone() {
                        AppState::Sending => "Sending Files",
                        AppState::Receiving => "Receiving Files",
                        _ => "Processing",
                    }
                } else {
                    "Waiting for peer"
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
                app.read()
                    .unwrap()
                    .progress_message
                    .read()
                    .unwrap()
                    .clone(),
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

    f.render_widget(status_info, right_chunks[0]);

    // Transfer statistics
    let files_count = if matches!(
        app.read().unwrap().state.read().unwrap().clone(),
        AppState::Sending
    ) {
        app.read()
            .unwrap()
            .sender_files_in
            .read()
            .unwrap()
            .len()
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

    if is_sender_connected | is_receiver_connected {
        render_progress_list_widget(
            f,
            app,
            &main_chunks,
            &right_chunks,
            title_text,
        );
    } else {
        // Transfer details or QR Code for sender
        render_qr_code_ww(f, app, &main_chunks, right_chunks, title_text);
    }

    // Footer
    let (footer_text, footer_color, footer_icon) =
        match app.state.read().unwrap().clone() {
            AppState::Sending => {
                if let Some(bubble) = app
                    .read()
                    .unwrap()
                    .send_files_bubble
                    .read()
                    .unwrap()
                    .as_ref()
                {
                    if app
                        .read()
                        .unwrap()
                        .progress_percentage
                        .read()
                        .unwrap()
                        .clone()
                        >= 100.0
                    {
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
                } else if app
                    .read()
                    .unwrap()
                    .progress_percentage
                    .read()
                    .unwrap()
                    .clone()
                    >= 100.0
                {
                    (
                        "Transfer completed! Press ESC to continue..."
                            .to_string(),
                        Color::Green,
                        "✅",
                    )
                } else {
                    // TODO: this scenario should not exist
                    (
                        "Transfer in progress... Press Q to cancel".to_string(),
                        Color::Yellow,
                        "⏳",
                    )
                }
            }
            AppState::Receiving => {
                if app
                    .read()
                    .unwrap()
                    .progress_percentage
                    .read()
                    .unwrap()
                    .clone()
                    >= 100.0
                {
                    (
                        "Transfer completed! Press ESC to continue..."
                            .to_string(),
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

    f.render_widget(footer, main_chunks[2]);
}

fn render_qr_code_ww(
    f: &mut Frame<'_>,
    app: &App,
    main_chunks: &std::rc::Rc<[ratatui::prelude::Rect]>,
    right_chunks: std::rc::Rc<[ratatui::prelude::Rect]>,
    title_text: &'static str,
) {
    match app.read().unwrap().state.read().unwrap().clone() {
        AppState::Sending => {
            // Check if we have a ticket and confirmation to display QR code
            if let Some(bubble) = app
                .read()
                .unwrap()
                .send_files_bubble
                .read()
                .unwrap()
                .as_ref()
            {
                let qr_data = format!(
                    "drop://receive?ticket={}&confirmation={}",
                    bubble.get_ticket(),
                    bubble.get_confirmation()
                );

                // QR Code on the right
                render_qr_code_widget(
                    f,
                    &qr_data,
                    right_chunks[1],
                    " Transfer QR Code ",
                    Color::Green,
                )
                .ok();
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
                    Span::styled(title_text, Style::default().fg(Color::White)),
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
                        app.read()
                            .unwrap()
                            .get_transfer_out_dir()
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

            f.render_widget(details, *main_chunks[1]);
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

            f.render_widget(details, *main_chunks[1]);
        }
    }
}

fn render_progress_list_widget(
    f: &mut Frame<'_>,
    app: &App,
    main_chunks: &std::rc::Rc<[ratatui::prelude::Rect]>,
    right_chunks: &std::rc::Rc<[ratatui::prelude::Rect]>,
    title_text: &'static str,
) {
    match app.read().unwrap().state.read().unwrap().clone() {
        AppState::Sending => {
            // Check if we have a ticket and confirmation to display QR code
            if let Some(bubble) = app
                .read()
                .unwrap()
                .send_files_bubble
                .read()
                .unwrap()
                .as_ref()
            {
                let qr_data = format!(
                    "drop://receive?ticket={}&confirmation={}",
                    bubble.get_ticket(),
                    bubble.get_confirmation()
                );

                // QR Code on the right
                render_qr_code_widget(
                    f,
                    &qr_data,
                    *right_chunks[1],
                    " Transfer QR Code ",
                    Color::Green,
                )
                .ok();
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
                    Span::styled(title_text, Style::default().fg(Color::White)),
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
                        app.read()
                            .unwrap()
                            .get_transfer_out_dir()
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

            f.render_widget(details, *main_chunks[1]);
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

            f.render_widget(details, *main_chunks[1]);
        }
    }
}

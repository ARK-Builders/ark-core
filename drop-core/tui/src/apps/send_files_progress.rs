use std::{
    sync::{Arc, RwLock, atomic::AtomicU32},
    time::Instant,
};

use crate::{App, AppBackend};
use ratatui::{
    Frame,
    layout::Alignment,
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};

pub struct SendFilesProgressApp {
    b: Arc<dyn AppBackend>,

    progress_pct: AtomicU32,
    operation_start_time: RwLock<Option<Instant>>,
}

impl App for SendFilesProgressApp {
    fn draw(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        // let main_blocks = Layout::default()
        //     .direction(Direction::Vertical)
        //     .margin(1)
        //     .constraints([
        //         Constraint::Length(3),  // Title
        //         Constraint::Length(12), // Progress section
        //         Constraint::Min(0),     // Details/logs or QR code
        //         Constraint::Length(4),  // Footer
        //     ])
        //     .split(area);

        // let progress_blocks = Layout::default()
        //     .direction(Direction::Horizontal)
        //     .constraints([
        //         Constraint::Percentage(50), // Status info
        //         Constraint::Percentage(50), // Progress visualization
        //     ])
        //     .split(main_blocks[1]);

        // let right_blocks = Layout::default()
        //     .direction(Direction::Vertical)
        //     .constraints([
        //         Constraint::Length(6), // Progress bar
        //         Constraint::Min(0),    // Transfer stats
        //     ])
        //     .split(progress_blocks[1]);
    }

    fn handle_control(&self, ev: &ratatui::crossterm::event::Event) {
        todo!()
    }
}

impl SendFilesProgressApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self {
            b,

            progress_pct: AtomicU32::new(0),
            operation_start_time: RwLock::new(None),
        }
    }

    fn get_progress_pct(&self) -> f64 {
        let v = self
            .progress_pct
            .load(std::sync::atomic::Ordering::Relaxed);
        return f64::from(v);
    }

    fn draw_title(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();

        let progress_icon = match (progress_pct as u8) % 4 {
            0 => "◜",
            1 => "◝",
            2 => "◞",
            _ => "◟",
        };

        let title_content = vec![Line::from(vec![
            Span::styled(
                format!("{} ", progress_icon),
                Style::default().fg(Color::Blue).bold(),
            ),
            Span::styled(
                "📤 Sending Files",
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                format!(" {:.1}%", progress_pct),
                Style::default().fg(Color::Cyan),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Transfer in Progress ") // TODO: extra | dynamic title
            .title_style(Style::default().fg(Color::White).bold());

        let title_widget = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title_widget, area);
    }

    fn get_operation_start_time(&self) -> Option<Instant> {
        self.operation_start_time.read().unwrap().clone()
    }

    fn draw_status(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();
        let operation_start_time = self.get_operation_start_time();

        let elapsed_time = if let Some(start_time) = operation_start_time {
            let elapsed = start_time.elapsed();
            format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
        } else {
            "00:00".to_string()
        };

        let estimated_remaining = if progress_pct > 0.0 && progress_pct < 100.0
        {
            let elapsed_secs = operation_start_time
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0);
            let total_estimated = elapsed_secs * 100.0 / progress_pct;
            let remaining = (total_estimated - elapsed_secs).max(0.0);
            format!(
                "{}:{:02}",
                (remaining as u64) / 60,
                (remaining as u64) % 60
            )
        } else {
            "--:--".to_string()
        };

        let status_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("🔄 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Status: ",
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(
                    "Sending Files",
                    Style::default().fg(Color::Blue).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("⏱️ ", Style::default().fg(Color::Yellow)),
                Span::styled("Elapsed: ", Style::default().fg(Color::White)),
                Span::styled(
                    elapsed_time,
                    Style::default().fg(Color::Cyan).bold(),
                ),
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
                    "TODO: message",
                    Style::default().fg(Color::Gray).italic(),
                ),
            ]),
            Line::from(""),
        ];

        let status_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let status_info = Paragraph::new(status_content)
            .block(status_block)
            .alignment(Alignment::Left);

        f.render_widget(status_info, area);
    }

    fn draw_progress(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();

        let progress_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Progress ")
            .title_style(Style::default().fg(Color::White).bold());

        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(
                Style::default()
                    .fg(Color::Blue)
                    .bg(Color::DarkGray),
            )
            .percent(progress_pct as u16)
            .label(Span::styled(
                format!("{:.1}%", progress_pct),
                Style::default().fg(Color::White).bold(),
            ));
        f.render_widget(progress, area);
    }

    fn draw_statistics(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        // Transfer statistics
        let files_count = 0; // TODO: this should track sent/received files

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

        f.render_widget(stats, area);
    }

    fn draw_info(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        // Check if we have a ticket and confirmation to display QR code
        // if let Some(bubble) = app
        //     .send_files_bubble
        //     .read()
        //     .unwrap()
        //     .as_ref(&self)
        // {
        //     let qr_data = format!(
        //         "{} {}",
        //         bubble.get_ticket(),
        //         bubble.get_confirmation()
        //     );

        //     // Split the area for QR code and details
        //     let qr_chunks = Layout::default()
        //         .direction(Direction::Horizontal)
        //         .constraints([
        //             Constraint::Percentage(50), // Details
        //             Constraint::Percentage(50), // QR Code
        //         ])
        //         .split(area);

        //     // Details on the left
        //     let details_content = vec![
        //         Line::from(""),
        //         Line::from(vec![
        //             Span::styled("📤 ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Sending Files",
        //                 Style::default().fg(Color::White).bold(),
        //             ),
        //         ]),
        //         Line::from(""),
        //         Line::from(vec![
        //             Span::styled("✓ ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Connection established with receiver",
        //                 Style::default().fg(Color::White),
        //             ),
        //         ]),
        //         Line::from(vec![
        //             Span::styled("🔑 ", Style::default().fg(Color::Blue)),
        //             Span::styled(
        //                 "Transfer Ticket: ",
        //                 Style::default().fg(Color::White),
        //             ),
        //             Span::styled(
        //                 bubble.get_ticket(),
        //                 Style::default().fg(Color::Cyan),
        //             ),
        //         ]),
        //         Line::from(vec![
        //             Span::styled("🔒 ", Style::default().fg(Color::Blue)),
        //             Span::styled(
        //                 "Confirmation Code: ",
        //                 Style::default().fg(Color::White),
        //             ),
        //             Span::styled(
        //                 bubble.get_confirmation().to_string(),
        //                 Style::default().fg(Color::Cyan),
        //             ),
        //         ]),
        //         Line::from(""),
        //         Line::from(vec![
        //             Span::styled("💡 ", Style::default().fg(Color::Yellow)),
        //             Span::styled(
        //                 "Share QR Code or ticket with receiver",
        //                 Style::default().fg(Color::Gray),
        //             ),
        //         ]),
        //     ];

        //     let details_block = Block::default()
        //         .borders(Borders::ALL)
        //         .border_set(border::ROUNDED)
        //         .border_style(Style::default().fg(Color::White))
        //         .title(" Transfer Details ")
        //         .title_style(Style::default().fg(Color::White).bold());

        //     let details = Paragraph::new(details_content)
        //         .block(details_block)
        //         .wrap(Wrap { trim: true })
        //         .alignment(Alignment::Left);
        //     f.render_widget(details, qr_chunks[0]);

        //     // QR Code on the right
        //     render_qr_code_widget(
        //         f,
        //         &qr_data,
        //         qr_chunks[1],
        //         " Transfer QR Code ",
        //         Color::Green,
        //     )
        //     .ok();
        // } else {
        //     // Fallback to regular details if no ticket/confirmation
        //     let details_content = vec![
        //         Line::from(""),
        //         Line::from(vec![
        //             Span::styled("📤 ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Sending Files",
        //                 Style::default().fg(Color::White).bold(),
        //             ),
        //         ]),
        //         Line::from(""),
        //         Line::from(vec![
        //             Span::styled("✓ ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Connection established with receiver",
        //                 Style::default().fg(Color::White),
        //             ),
        //         ]),
        //         Line::from(vec![
        //             Span::styled("✓ ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Files are encrypted during transfer",
        //                 Style::default().fg(Color::White),
        //             ),
        //         ]),
        //         Line::from(vec![
        //             Span::styled("✓ ", Style::default().fg(Color::Green)),
        //             Span::styled(
        //                 "Transfer will complete automatically",
        //                 Style::default().fg(Color::White),
        //             ),
        //         ]),
        //     ];

        //     let details_block = Block::default()
        //         .borders(Borders::ALL)
        //         .border_set(border::ROUNDED)
        //         .border_style(Style::default().fg(Color::White))
        //         .title(" Transfer Details ")
        //         .title_style(Style::default().fg(Color::White).bold());

        //     let details = Paragraph::new(details_content)
        //         .block(details_block)
        //         .wrap(Wrap { trim: true })
        //         .alignment(Alignment::Left);

        //     f.render_widget(details, main_blocks[2]);
        // }
    }

    fn draw_footer(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();

        // Footer
        let (footer_text, footer_color, footer_icon) = if let Some(bubble) =
            self.b
                .get_send_files_manager()
                .get_send_files_bubble()
        {
            if progress_pct >= 100.0 {
                (
                    "Transfer completed! Press ESC to continue...".to_string(),
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
        } else if progress_pct >= 100.0 {
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

        f.render_widget(footer, area);
    }
}

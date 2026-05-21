use qrcode::QrCode;
use ratatui::{
    Frame,
    layout::Alignment,
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Error type for QR code rendering failures.
#[derive(Debug)]
pub struct QrRenderError {
    pub message: String,
}

impl std::fmt::Display for QrRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QR render error: {}", self.message)
    }
}

impl std::error::Error for QrRenderError {}

/// Reusable QR code rendering utilities for TUI applications.
pub struct QrCodeRenderer;

impl QrCodeRenderer {
    /// Generates QR code as a vector of styled lines for display.
    ///
    /// The QR code uses doubled-width blocks for better terminal visibility.
    pub fn render_qr_lines(
        data: &str,
    ) -> Result<Vec<Line<'static>>, QrRenderError> {
        let qr_code = QrCode::new(data).map_err(|e| QrRenderError {
            message: e.to_string(),
        })?;

        let qr_matrix = qr_code
            .render::<char>()
            .quiet_zone(false)
            .module_dimensions(1, 1)
            .build();

        let lines: Vec<Line<'static>> = qr_matrix
            .lines()
            .map(|line| {
                Line::from(vec![Span::styled(
                    line.replace('█', "██").replace(' ', "  "),
                    Style::default().fg(Color::White).bg(Color::Black),
                )])
            })
            .collect();

        Ok(lines)
    }

    /// Creates a styled block for QR code display.
    pub fn create_qr_block(title: &str, color: Color) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(color))
            .title(format!(" {} ", title))
            .title_style(Style::default().fg(Color::White).bold())
    }

    /// Renders a waiting state when QR code is not yet available.
    pub fn render_waiting(
        f: &mut Frame,
        area: ratatui::prelude::Rect,
        block: Block,
        message: &str,
    ) {
        let waiting_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    message,
                    Style::default().fg(Color::Yellow).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "QR code will appear when ready",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let waiting_widget = Paragraph::new(waiting_content)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(waiting_widget, area);
    }

    /// Renders an error state when QR code generation fails.
    pub fn render_error(
        f: &mut Frame,
        area: ratatui::prelude::Rect,
        block: Block,
        error_message: &str,
    ) {
        let error_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("❌ ", Style::default().fg(Color::Red)),
                Span::styled(
                    "Failed to generate QR code",
                    Style::default().fg(Color::Red).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                error_message,
                Style::default().fg(Color::Gray),
            )]),
        ];

        let error_widget = Paragraph::new(error_content)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(error_widget, area);
    }

    /// Renders a complete QR code with the given data.
    ///
    /// This is a convenience method that handles all rendering states:
    /// - Shows the QR code if generation succeeds
    /// - Shows an error message if generation fails
    pub fn render_qr_code(
        f: &mut Frame,
        area: ratatui::prelude::Rect,
        block: Block,
        data: &str,
    ) {
        match Self::render_qr_lines(data) {
            Ok(qr_lines) => {
                let qr_widget = Paragraph::new(qr_lines)
                    .block(block)
                    .alignment(Alignment::Center);
                f.render_widget(qr_widget, area);
            }
            Err(e) => {
                Self::render_error(f, area, block, &e.message);
            }
        }
    }
}

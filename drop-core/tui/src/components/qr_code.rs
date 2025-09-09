use anyhow::Result;
use qrcode::QrCode;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color as TuiColor, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render_qr_code_widget(
    f: &mut Frame,
    data: &str,
    area: Rect,
    title: &str,
    border_color: TuiColor,
) -> Result<()> {
    match QrCode::new(data) {
        Ok(code) => {
            let code_image = code
                .render::<char>()
                .quiet_zone(false)
                .module_dimensions(2, 1)
                .light_color(' ')
                .dark_color('#')
                .build();
            let code_image_lines: Vec<Line> = code_image
                .lines()
                .map(|line| {
                    Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(TuiColor::White)
                            .bg(TuiColor::Black),
                    )])
                })
                .collect();
            let qr_widget = Paragraph::new(code_image_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .title(title)
                        .title_style(
                            Style::default().fg(TuiColor::White).bold(),
                        ),
                )
                .alignment(Alignment::Center);

            f.render_widget(qr_widget, area);
        }
        Err(_) => {
            // Fallback to text display if QR generation fails
            let fallback_content = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "QR Code Generation Failed",
                    Style::default().fg(TuiColor::Red),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Transfer Code:",
                    Style::default().fg(TuiColor::Yellow).bold(),
                )]),
                Line::from(vec![Span::styled(
                    data,
                    Style::default().fg(TuiColor::White).bold(),
                )]),
                Line::from(""),
            ];

            let fallback_widget = Paragraph::new(fallback_content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .title(title)
                        .title_style(
                            Style::default().fg(TuiColor::White).bold(),
                        ),
                )
                .alignment(Alignment::Center);

            f.render_widget(fallback_widget, area);
        }
    }

    Ok(())
}

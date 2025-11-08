use ratatui::{
    layout::Alignment,
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct HelperFooterControl {
    pub title: String,
    pub description: String,
}

impl HelperFooterControl {
    pub fn new(title: &str, description: &str) -> Self {
        return Self {
            title: title.to_string(),
            description: description.to_string(),
        };
    }
}

pub fn create_helper_footer(
    controls: Vec<HelperFooterControl>,
) -> Paragraph<'static> {
    let controls_text = create_controls_text(controls);

    let footer_content = vec![
        Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(controls_text, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Controls ")
        .title_style(Style::default().fg(Color::White).bold());

    let footer = Paragraph::new(footer_content)
        .block(footer_block)
        .alignment(Alignment::Center);

    return footer;
}

fn create_controls_text(controls: Vec<HelperFooterControl>) -> String {
    let mut controls_text = String::with_capacity(controls.len() * 21);

    for (i, c) in controls.iter().enumerate() {
        if i > 0 {
            controls_text.push_str(" • ");
        }
        controls_text.push_str(&c.title);
        controls_text.push_str(" ");
        controls_text.push_str(&c.description);
    }

    controls_text
}

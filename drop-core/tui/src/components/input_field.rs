use ratatui::{
    Frame,
    backend::Backend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct InputField {
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub focused: bool,
    pub password: bool,
}

impl InputField {
    pub fn new(label: String) -> Self {
        Self {
            label,
            value: String::new(),
            placeholder: String::new(),
            focused: false,
            password: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: String) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn with_value(mut self, value: String) -> Self {
        self.value = value;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn render<B: Backend>(&self, f: &mut Frame, area: Rect) {
        let border_style = if self.focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let display_text = if self.value.is_empty() {
            &self.placeholder
        } else if self.password {
            &"*".repeat(self.value.len())
        } else {
            &self.value
        };

        let text_style = if self.value.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let input = Paragraph::new(vec![
            Line::from(format!("{}:", self.label)),
            Line::from(""),
            Line::from(Span::styled(display_text, text_style)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(self.label.clone())
                .border_style(border_style),
        );

        f.render_widget(input, area);
    }

    pub fn push_char(&mut self, c: char) {
        self.value.push(c);
    }

    pub fn pop_char(&mut self) {
        self.value.pop();
    }

    pub fn clear(&mut self) {
        self.value.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
    }
}

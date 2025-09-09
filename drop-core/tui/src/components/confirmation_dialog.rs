use ratatui::{
    Frame,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub struct ConfirmationDialog {
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub cancel_text: String,
    pub selected: bool, // true for confirm, false for cancel
}

impl ConfirmationDialog {
    pub fn new(title: String, message: String) -> Self {
        Self {
            title,
            message,
            confirm_text: "Yes".to_string(),
            cancel_text: "No".to_string(),
            selected: false, // Default to cancel for safety
        }
    }

    pub fn with_buttons(
        mut self,
        confirm_text: String,
        cancel_text: String,
    ) -> Self {
        self.confirm_text = confirm_text;
        self.cancel_text = cancel_text;
        self
    }

    pub fn render<B: Backend>(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 30, f.area());

        // Clear the background
        f.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Message
                Constraint::Length(3), // Buttons
            ])
            .split(area);

        // Message
        let message_block = Paragraph::new(self.message.as_str())
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.title.clone())
                    .style(Style::default().fg(Color::Yellow)),
            );
        f.render_widget(message_block, chunks[0]);

        // Buttons
        let button_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chunks[1]);

        let confirm_style = if self.selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        let cancel_style = if !self.selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red)
        };

        let confirm_button =
            Paragraph::new(format!("[ {} ]", self.confirm_text))
                .alignment(Alignment::Center)
                .style(confirm_style)
                .block(Block::default().borders(Borders::ALL));
        f.render_widget(confirm_button, button_area[0]);

        let cancel_button = Paragraph::new(format!("[ {} ]", self.cancel_text))
            .alignment(Alignment::Center)
            .style(cancel_style)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(cancel_button, button_area[1]);
    }

    pub fn toggle_selection(&mut self) {
        self.selected = !self.selected;
    }

    pub fn is_confirm_selected(&self) -> bool {
        self.selected
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

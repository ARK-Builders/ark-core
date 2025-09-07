use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render_receive_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Ticket field
            Constraint::Length(5), // Confirmation field
            Constraint::Length(5), // Output directory field
            Constraint::Length(5), // Name field
            Constraint::Length(5), // Avatar field
            Constraint::Min(0),    // Instructions/Button
        ])
        .split(area);

    // Title
    let title = Paragraph::new("📥 Receive Files")
        .style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Ticket field
    let ticket_style = if app.receive_focused_field == 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let ticket_text = if app.receive_ticket.is_empty() {
        "Enter transfer ticket..."
    } else {
        &app.receive_ticket
    };

    let ticket_field = Paragraph::new(vec![
        Line::from("Transfer Ticket:"),
        Line::from(""),
        Line::from(Span::styled(
            ticket_text,
            if app.receive_ticket.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Ticket")
            .border_style(ticket_style),
    );
    f.render_widget(ticket_field, chunks[1]);

    // Confirmation field
    let confirmation_style = if app.receive_focused_field == 1 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let confirmation_text = if app.receive_confirmation.is_empty() {
        "Enter confirmation code..."
    } else {
        &app.receive_confirmation
    };

    let confirmation_field = Paragraph::new(vec![
        Line::from("Confirmation Code:"),
        Line::from(""),
        Line::from(Span::styled(
            confirmation_text,
            if app.receive_confirmation.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Confirmation")
            .border_style(confirmation_style),
    );
    f.render_widget(confirmation_field, chunks[2]);

    // Output directory field
    let output_style = if app.receive_focused_field == 2 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let output_text = if app.receive_output_dir.is_empty() {
        if let Some(ref default_dir) = app.default_receive_dir {
            default_dir.as_str()
        } else {
            "Using default directory..."
        }
    } else {
        &app.receive_output_dir
    };

    let output_field = Paragraph::new(vec![
        Line::from("Output Directory (optional):"),
        Line::from(""),
        Line::from(Span::styled(
            output_text,
            if app.receive_output_dir.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Output Directory")
            .border_style(output_style),
    );
    f.render_widget(output_field, chunks[3]);

    // Name field
    let name_style = if app.receive_focused_field == 3 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let name_field = Paragraph::new(vec![
        Line::from("Your display name:"),
        Line::from(""),
        Line::from(Span::styled(
            &app.receive_name,
            Style::default().fg(Color::White),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Receiver Name")
            .border_style(name_style),
    );
    f.render_widget(name_field, chunks[4]);

    // Avatar field
    let avatar_style = if app.receive_focused_field == 4 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let avatar_text = app
        .receive_avatar_path
        .as_deref()
        .unwrap_or("None");
    let avatar_field = Paragraph::new(vec![
        Line::from("Avatar image (optional):"),
        Line::from(""),
        Line::from(Span::styled(
            avatar_text,
            Style::default().fg(Color::White),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Avatar")
            .border_style(avatar_style),
    );
    f.render_widget(avatar_field, chunks[5]);

    // Instructions and button
    let can_receive =
        !app.receive_ticket.is_empty() && !app.receive_confirmation.is_empty();
    let instructions = if can_receive {
        "Press Enter when focused on Receive button to start transfer"
    } else {
        "Enter both ticket and confirmation code to proceed"
    };

    let receive_button_style = if app.receive_focused_field == 5 {
        if can_receive {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Black)
        }
    } else {
        if can_receive {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let footer = Paragraph::new(vec![
        Line::from(instructions),
        Line::from(""),
        Line::from(vec![
            Span::styled("[ ", Style::default()),
            Span::styled("Receive Files", receive_button_style),
            Span::styled(" ]", Style::default()),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[6]);
}

pub async fn handle_receive_page_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Tab => {
            app.receive_focused_field = (app.receive_focused_field + 1) % 6;
        }
        KeyCode::BackTab => {
            app.receive_focused_field = if app.receive_focused_field == 0 {
                5
            } else {
                app.receive_focused_field - 1
            };
        }
        KeyCode::Enter => {
            if app.receive_focused_field == 5 {
                // Receive files
                app.start_receive_operation();
            }
        }
        KeyCode::Char(c) => match app.receive_focused_field {
            0 => {
                app.receive_ticket.push(c);
            }
            1 => {
                app.receive_confirmation.push(c);
            }
            2 => {
                app.receive_output_dir.push(c);
            }
            3 => {
                app.receive_name.push(c);
            }
            4 => {
                if app.receive_avatar_path.is_none() {
                    app.receive_avatar_path = Some(String::new());
                }
                if let Some(ref mut avatar_path) = app.receive_avatar_path {
                    avatar_path.push(c);
                }
            }
            _ => {}
        },
        KeyCode::Backspace => match app.receive_focused_field {
            0 => {
                app.receive_ticket.pop();
            }
            1 => {
                app.receive_confirmation.pop();
            }
            2 => {
                app.receive_output_dir.pop();
            }
            3 => {
                app.receive_name.pop();
            }
            4 => {
                if let Some(ref mut avatar_path) = app.receive_avatar_path {
                    if avatar_path.is_empty() {
                        app.receive_avatar_path = None;
                    } else {
                        avatar_path.pop();
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

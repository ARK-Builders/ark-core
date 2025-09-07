use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render_receive_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(50), // Left side - transfer details
            Constraint::Percentage(50), // Right side - profile settings
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Page title
            Constraint::Length(6), // Ticket field
            Constraint::Length(6), // Confirmation field
            Constraint::Length(6), // Output directory field
            Constraint::Min(0),    // Instructions
        ])
        .split(main_chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Name field
            Constraint::Length(5), // Avatar field
            Constraint::Min(0),    // Receive button
        ])
        .split(main_chunks[1]);

    // Title
    let title_content = vec![Line::from(vec![
        Span::styled("📥 ", Style::default().fg(Color::Blue).bold()),
        Span::styled("Receive Files", Style::default().fg(Color::White).bold()),
    ])];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Blue))
        .title(" Incoming Transfer ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);
    f.render_widget(title, left_chunks[0]);

    // Ticket field
    let ticket_focused = app.receive_focused_field == 0;
    let ticket_style = if ticket_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let ticket_content = vec![
        Line::from(vec![
            Span::styled("🎫 ", Style::default().fg(Color::Blue)),
            Span::styled("Transfer Ticket:", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if ticket_focused {
                    "▶ "
                } else {
                    "  "
                },
                if ticket_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                if app.receive_ticket.is_empty() {
                    "Enter ticket from sender..."
                } else {
                    &app.receive_ticket
                },
                if app.receive_ticket.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::White).bold()
                },
            ),
        ]),
        Line::from(""),
    ];

    let ticket_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(ticket_style)
        .title(" Transfer Ticket ")
        .title_style(Style::default().fg(Color::White).bold());

    let ticket_field = Paragraph::new(ticket_content)
        .block(ticket_block)
        .alignment(Alignment::Left);
    f.render_widget(ticket_field, left_chunks[1]);

    // Confirmation field
    let confirmation_focused = app.receive_focused_field == 1;
    let confirmation_style = if confirmation_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let confirmation_content = vec![
        Line::from(vec![
            Span::styled("🔐 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Confirmation Code:",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if confirmation_focused {
                    "▶ "
                } else {
                    "  "
                },
                if confirmation_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                if app.receive_confirmation.is_empty() {
                    "Enter confirmation code..."
                } else {
                    &app.receive_confirmation
                },
                if app.receive_confirmation.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::White).bold()
                },
            ),
        ]),
        Line::from(""),
    ];

    let confirmation_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(confirmation_style)
        .title(" Confirmation Code ")
        .title_style(Style::default().fg(Color::White).bold());

    let confirmation_field = Paragraph::new(confirmation_content)
        .block(confirmation_block)
        .alignment(Alignment::Left);
    f.render_widget(confirmation_field, left_chunks[2]);

    // Output directory field
    let output_focused = app.receive_focused_field == 2;
    let output_style = if output_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let output_text = if app.receive_output_dir.is_empty() {
        if let Some(ref default_dir) = app.default_receive_dir {
            default_dir.as_str()
        } else {
            "~/Downloads/ARK-Drop"
        }
    } else {
        &app.receive_output_dir
    };

    let output_content = vec![
        Line::from(vec![
            Span::styled("📂 ", Style::default().fg(Color::Magenta)),
            Span::styled("Save Location:", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if output_focused {
                    "▶ "
                } else {
                    "  "
                },
                if output_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                output_text,
                if app.receive_output_dir.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]),
        Line::from(""),
    ];

    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(output_style)
        .title(" Output Directory ")
        .title_style(Style::default().fg(Color::White).bold());

    let output_field = Paragraph::new(output_content)
        .block(output_block)
        .alignment(Alignment::Left);
    f.render_widget(output_field, left_chunks[3]);

    // Name field
    let name_focused = app.receive_focused_field == 3;
    let name_style = if name_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let name_content = vec![
        Line::from(vec![
            Span::styled("👤 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if name_focused {
                    "▶ "
                } else {
                    "  "
                },
                if name_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                &app.receive_name,
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(""),
    ];

    let name_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(name_style)
        .title(" Your Name ")
        .title_style(Style::default().fg(Color::White).bold());

    let name_field = Paragraph::new(name_content)
        .block(name_block)
        .alignment(Alignment::Left);
    f.render_widget(name_field, right_chunks[0]);

    // Avatar field
    let avatar_focused = app.receive_focused_field == 4;
    let avatar_style = if avatar_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let avatar_text = app
        .receive_avatar_path
        .as_deref()
        .unwrap_or("No avatar selected");
    let avatar_content = vec![
        Line::from(vec![
            Span::styled("🖼️ ", Style::default().fg(Color::Magenta)),
            Span::styled(
                if avatar_focused {
                    "▶ "
                } else {
                    "  "
                },
                if avatar_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                avatar_text,
                if app.receive_avatar_path.is_some() {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray).italic()
                },
            ),
        ]),
        Line::from(""),
    ];

    let avatar_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(avatar_style)
        .title(" Avatar (Optional) ")
        .title_style(Style::default().fg(Color::White).bold());

    let avatar_field = Paragraph::new(avatar_content)
        .block(avatar_block)
        .alignment(Alignment::Left);
    f.render_widget(avatar_field, right_chunks[1]);

    // Instructions in left panel
    let can_receive =
        !app.receive_ticket.is_empty() && !app.receive_confirmation.is_empty();

    let instructions_content = if can_receive {
        vec![
            Line::from(vec![
                Span::styled("✅ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Ready to receive!",
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📥 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Click Receive button to start download",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Missing required information",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "Enter both ticket and confirmation code",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]
    };

    let instructions_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Status ")
        .title_style(Style::default().fg(Color::White).bold());

    let instructions = Paragraph::new(instructions_content)
        .block(instructions_block)
        .alignment(Alignment::Left);
    f.render_widget(instructions, left_chunks[4]);

    // Receive button
    let receive_button_focused = app.receive_focused_field == 5;

    let receive_button_style = if receive_button_focused && can_receive {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Blue)
            .bold()
    } else if receive_button_focused {
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Black)
            .bold()
    } else if can_receive {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let button_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if receive_button_focused {
                    "▶ "
                } else {
                    "  "
                },
                if receive_button_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                if can_receive {
                    "📥 Receive Files"
                } else {
                    "❌ Cannot Receive"
                },
                receive_button_style,
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Files will be saved to:",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(vec![Span::styled(
            if app.receive_output_dir.is_empty() {
                app.default_receive_dir
                    .as_deref()
                    .unwrap_or("~/Downloads/ARK-Drop")
            } else {
                &app.receive_output_dir
            },
            Style::default().fg(Color::Cyan).italic(),
        )]),
    ];

    let receive_button_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(if receive_button_focused {
            Style::default().fg(Color::Yellow)
        } else if can_receive {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(" Action ")
        .title_style(Style::default().fg(Color::White).bold());

    let receive_button = Paragraph::new(button_content)
        .block(receive_button_block)
        .alignment(Alignment::Center);
    f.render_widget(receive_button, right_chunks[2]);
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

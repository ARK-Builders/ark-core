use anyhow::Result;
use arkdrop_common::get_default_out_dir;
use ratatui::{
    Frame,
    backend::Backend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::env;

use crate::{
    App,
    components::file_browser::{BrowserMode, open_system_file_browser},
};

pub fn render_receive_page<B: Backend>(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    // If directory browser is open, render it as overlay
    if app.show_dir_browser.read().unwrap().clone() {
        if let Some(ref mut browser) =
            app.directory_browser.read().unwrap().clone()
        {
            browser.render::<B>(f, area);
        }
        return;
    }
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
    let ticket_focused = app.receiver_focused_field == 0;
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
                if app.receiver_ticket_in.is_empty() {
                    "Enter ticket from sender..."
                } else {
                    &app.receiver_ticket_in
                },
                if app.receiver_ticket_in.is_empty() {
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
    let confirmation_focused = app.receiver_focused_field == 1;
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
                if app.receiver_confirmation_in.is_empty() {
                    "Enter confirmation code..."
                } else {
                    &app.receiver_confirmation_in
                },
                if app.receiver_confirmation_in.is_empty() {
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
    let output_focused = app.receiver_focused_field == 2;
    let output_style = if output_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let output_text = if app.receiver_out_dir_in.is_empty() {
        get_default_out_dir()
            .to_string_lossy()
            .to_string()
    } else {
        app.receiver_out_dir_in.clone()
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
                if app.receiver_out_dir_in.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" browse • ", Style::default().fg(Color::Gray)),
            Span::styled("Ctrl+O", Style::default().fg(Color::Green).bold()),
            Span::styled(" system dialog", Style::default().fg(Color::Gray)),
        ]),
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
    let name_focused = app.receiver_focused_field == 3;
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
                &app.receiver_name,
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
    let avatar_focused = app.receiver_focused_field == 4;
    let avatar_style = if avatar_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let avatar_text = app
        .receiver_avatar_path
        .read()
        .unwrap()
        .clone()
        .unwrap_or("No avatar selected".to_string());

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
                if app.receiver_avatar_path.read().unwrap().is_some() {
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
    let can_receive = !app.receiver_ticket_in.is_empty()
        && !app.receiver_confirmation_in.is_empty();

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
    let receive_button_focused = app.receiver_focused_field == 5;

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
            if app.receiver_out_dir_in.is_empty() {
                get_default_out_dir()
                    .to_string_lossy()
                    .to_string()
            } else {
                app.receiver_out_dir_in.clone()
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
            app.receiver_focused_field = (app.receiver_focused_field + 1) % 6;
        }
        KeyCode::BackTab => {
            app.receiver_focused_field = if app.receiver_focused_field == 0 {
                5
            } else {
                app.receiver_focused_field - 1
            };
        }
        KeyCode::Enter => match app.receiver_focused_field {
            2 => {
                // Browse for output directory
                app.open_directory_browser();
            }
            5 => {
                app.start_receive_operation().await?;
            }
            _ => {}
        },
        KeyCode::Char(c) => match key.modifiers {
            KeyModifiers::NONE => match app.receiver_focused_field {
                0 => {
                    app.receiver_ticket_in.push(c);
                }
                1 => {
                    app.receiver_confirmation_in.push(c);
                }
                2 => {
                    app.receiver_out_dir_in.push(c);
                }
                3 => {
                    app.receiver_name.push(c);
                }
                4 => {
                    if app.receiver_avatar_path.read().unwrap().is_none() {
                        *app.receiver_avatar_path.write().unwrap() =
                            Some(String::new());
                    }
                    if let Some(avatar_path) =
                        app.receiver_avatar_path.write().unwrap().as_mut()
                    {
                        avatar_path.push(c);
                    }
                }
                _ => {}
            },
            KeyModifiers::CONTROL => match c {
                'o' => {
                    if app.receiver_focused_field == 2 {
                        match open_system_file_browser(
                            BrowserMode::SelectDirectory,
                            env::current_dir().ok(),
                        ) {
                            Ok(mut dirs) => {
                                if let Some(dir) = dirs.pop() {
                                    app.receiver_out_dir_in =
                                        dir.to_string_lossy().to_string();
                                }
                            }
                            Err(_) => {
                                // Fall back to TUI directory browser
                                app.open_directory_browser();
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        },
        KeyCode::Backspace => match app.receiver_focused_field {
            0 => {
                app.receiver_ticket_in.pop();
            }
            1 => {
                app.receiver_confirmation_in.pop();
            }
            2 => {
                app.receiver_out_dir_in.pop();
            }
            3 => {
                app.receiver_name.pop();
            }
            4 => {
                if let Some(avatar_path) =
                    app.receiver_avatar_path.write().unwrap().as_mut()
                {
                    if avatar_path.is_empty() {
                        *app.receiver_avatar_path.write().unwrap() = None;
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

pub async fn handle_dir_browser_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    if let Some(browser) = app.directory_browser.write().unwrap().as_mut() {
        match key.code {
            KeyCode::Esc => {
                app.close_directory_browser();
            }
            KeyCode::Up => {
                browser.navigate_up();
            }
            KeyCode::Down => {
                browser.navigate_down();
            }
            KeyCode::Enter => {
                browser.enter_selected();
                // For directories, always navigate into them
            }
            KeyCode::Tab => {
                // Select current directory and close
                let selected_dir = browser.select_current_directory();
                app.receiver_out_dir_in =
                    selected_dir.to_string_lossy().to_string();
                app.close_directory_browser();
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                browser.toggle_hidden();
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                browser.cycle_sort_mode();
            }
            _ => {}
        }
    }
    Ok(())
}

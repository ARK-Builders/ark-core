use crate::tui::{
    app::App,
    components::file_browser::{
        BrowserMode, FileBrowser, open_system_file_browser,
    },
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Backend,
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::{env, path::PathBuf};

pub fn render_send_page<B: Backend>(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    // If file browser is open, render it as modal
    if app.show_file_browser {
        render_file_browser_modal::<B>(f, app, area);
        return;
    }
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(60), // Left side - form
            Constraint::Percentage(40), // Right side - files list & actions
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(6), // File selection
            Constraint::Length(5), // Name field
            Constraint::Length(5), // Avatar field
            Constraint::Min(0),    // Instructions
        ])
        .split(main_chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Files list
            Constraint::Length(5), // Send button
        ])
        .split(main_chunks[1]);

    // Title
    let title_content = vec![Line::from(vec![
        Span::styled("📤 ", Style::default().fg(Color::Green).bold()),
        Span::styled("Send Files", Style::default().fg(Color::White).bold()),
    ])];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Green))
        .title(" Outgoing Transfer ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_content)
        .block(title_block)
        .alignment(Alignment::Center);
    f.render_widget(title, left_chunks[0]);

    // File input field
    let file_input_focused = app.send_focused_field == 0;
    let file_input_style = if file_input_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let file_input_content = vec![
        Line::from(vec![
            Span::styled("📁 ", Style::default().fg(Color::Blue)),
            Span::styled("File Path:", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "▶ ",
                if file_input_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                if app.send_file_input.is_empty() {
                    "/path/to/your/file.txt"
                } else {
                    &app.send_file_input
                },
                if app.send_file_input.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" add • ", Style::default().fg(Color::Gray)),
            Span::styled("Ctrl+O", Style::default().fg(Color::Green).bold()),
            Span::styled(" browse • ", Style::default().fg(Color::Gray)),
            Span::styled("Ctrl+C", Style::default().fg(Color::Red).bold()),
            Span::styled(" clear", Style::default().fg(Color::Gray)),
        ]),
    ];

    let file_input_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(file_input_style)
        .title(" Add Files ")
        .title_style(Style::default().fg(Color::White).bold());

    let file_input = Paragraph::new(file_input_content)
        .block(file_input_block)
        .alignment(Alignment::Left);
    f.render_widget(file_input, left_chunks[1]);

    // Name field
    let name_focused = app.send_focused_field == 1;
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
                &app.send_name,
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
    f.render_widget(name_field, left_chunks[2]);

    // Avatar field
    let avatar_focused = app.send_focused_field == 2;
    let avatar_style = if avatar_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let avatar_text = app
        .send_avatar_path
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
                if app.send_avatar_path.is_some() {
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
    f.render_widget(avatar_field, left_chunks[3]);

    // Files list
    let file_items: Vec<ListItem> = if app.send_files.is_empty() {
        vec![ListItem::new(vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "No files selected yet",
                    Style::default().fg(Color::DarkGray).italic(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "   Add files using the input field above",
                Style::default().fg(Color::Gray),
            )]),
        ])]
    } else {
        app.send_files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let file_name = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");
                let file_path = file
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("/");

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!("{}. ", i + 1),
                            Style::default().fg(Color::Yellow).bold(),
                        ),
                        Span::styled("📄 ", Style::default().fg(Color::Blue)),
                        Span::styled(
                            file_name,
                            Style::default().fg(Color::White).bold(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("   ", Style::default()),
                        Span::styled(
                            file_path,
                            Style::default().fg(Color::Gray).italic(),
                        ),
                    ]),
                ])
            })
            .collect()
    };

    let files_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(if app.send_files.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Blue)
        })
        .title(format!(" Selected Files ({}) ", app.send_files.len()))
        .title_style(Style::default().fg(Color::White).bold());

    let files_list = List::new(file_items).block(files_block);
    f.render_widget(files_list, right_chunks[0]);

    // Instructions in left panel
    let instructions_content = if app.send_files.is_empty() {
        vec![
            Line::from(vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Add at least one file to proceed",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "💡 Tip: ",
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(
                    "Enter full file paths or 'browse' command",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("✅ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Ready to send! ",
                    Style::default().fg(Color::Green),
                ),
                Span::styled(
                    format!("{} file(s) selected", app.send_files.len()),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🚀 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Click Send button to start transfer",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]
    };

    let instructions_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Instructions ")
        .title_style(Style::default().fg(Color::White).bold());

    let instructions = Paragraph::new(instructions_content)
        .block(instructions_block)
        .alignment(Alignment::Left);
    f.render_widget(instructions, left_chunks[4]);

    // Send button
    let send_button_focused = app.send_focused_field == 3;
    let can_send = !app.send_files.is_empty();

    let send_button_style = if send_button_focused && can_send {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .bold()
    } else if send_button_focused {
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Black)
            .bold()
    } else if can_send {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let button_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if send_button_focused {
                    "▶ "
                } else {
                    "  "
                },
                if send_button_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                if can_send {
                    "🚀 Send Files"
                } else {
                    "❌ Cannot Send"
                },
                send_button_style,
            ),
        ]),
        Line::from(""),
    ];

    let send_button_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(if send_button_focused {
            Style::default().fg(Color::Yellow)
        } else if can_send {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(" Action ")
        .title_style(Style::default().fg(Color::White).bold());

    let send_button = Paragraph::new(button_content)
        .block(send_button_block)
        .alignment(Alignment::Center);
    f.render_widget(send_button, right_chunks[1]);
}

pub async fn handle_send_page_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Tab, _) => {
            app.send_focused_field = (app.send_focused_field + 1) % 4;
        }
        (KeyCode::BackTab, _) => {
            app.send_focused_field = if app.send_focused_field == 0 {
                3
            } else {
                app.send_focused_field - 1
            };
        }
        (KeyCode::Enter, _) => {
            match app.send_focused_field {
                0 => {
                    // Add file
                    if !app.send_file_input.is_empty() {
                        if app.send_file_input == "browse" {
                            match open_system_file_browser(
                                BrowserMode::SelectFiles,
                                env::current_dir().ok(),
                            ) {
                                Ok(files) => {
                                    for file in files {
                                        app.add_file(file);
                                    }
                                }
                                Err(_) => {
                                    app.open_file_browser();
                                }
                            };
                        } else {
                            let path = PathBuf::from(&app.send_file_input);
                            if path.exists() {
                                app.add_file(path);
                                app.send_file_input.clear();
                            } else {
                                app.show_error(
                                    "File does not exist".to_string(),
                                );
                            }
                        }
                    }
                }
                3 => {
                    // Send files
                    app.start_send_operation();
                }
                _ => {}
            }
        }
        (KeyCode::Char(c), modifiers) => match modifiers {
            KeyModifiers::NONE => match app.send_focused_field {
                0 => {
                    app.send_file_input.push(c);
                }
                1 => {
                    app.send_name.push(c);
                }
                2 => {
                    if app.send_avatar_path.is_none() {
                        app.send_avatar_path = Some(String::new());
                    }
                    if let Some(ref mut avatar_path) = app.send_avatar_path {
                        avatar_path.push(c);
                    }
                }
                _ => {}
            },
            KeyModifiers::CONTROL => match c {
                'c' => match app.send_focused_field {
                    0 => {
                        app.send_file_input.clear();
                    }
                    _ => {}
                },
                'o' => {
                    if app.send_focused_field == 0 {
                        match open_system_file_browser(
                            BrowserMode::SelectFiles,
                            env::current_dir().ok(),
                        ) {
                            Ok(files) => {
                                for file in files {
                                    app.add_file(file);
                                }
                            }
                            Err(_) => {
                                // Fall back to TUI file browser
                                app.open_file_browser();
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        },
        (KeyCode::Backspace, _) => match app.send_focused_field {
            0 => {
                app.send_file_input.pop();
            }
            1 => {
                app.send_name.pop();
            }
            2 => {
                if let Some(ref mut avatar_path) = app.send_avatar_path {
                    if avatar_path.is_empty() {
                        app.send_avatar_path = None;
                    } else {
                        avatar_path.pop();
                    }
                }
            }
            _ => {}
        },
        (KeyCode::Delete, _) => {
            if app.send_focused_field == 0 && !app.send_files.is_empty() {
                // Remove last added file
                app.send_files.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_file_browser_modal<B: Backend>(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let mut b = FileBrowser::new(PathBuf::new(), BrowserMode::SelectFiles);
    b.render::<B>(f, area);
}

pub async fn handle_file_browser_input(
    app: &mut App,
    key: KeyEvent,
) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.close_file_browser();
        }
        _ => {}
    }
    Ok(())
}

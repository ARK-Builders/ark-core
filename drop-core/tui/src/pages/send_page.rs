use crate::{
    App,
    components::file_browser::{BrowserMode, open_system_file_browser},
};
use anyhow::Result;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Backend,
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{env, path::PathBuf};

pub fn render_send_page<B: Backend>(
    f: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
) {
    // If file browser is open, render it as overlay
    if app.show_file_browser.read().unwrap().clone() {
        if let Some(ref mut browser) = app.file_browser.read().unwrap().clone()
        {
            browser.render::<B>(f, area);
        }
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
    let file_input_focused = app.sender_focused_field == 0;
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
                if app.sender_file_path_in.is_empty() {
                    "/path/to/your/file.txt"
                } else {
                    &app.sender_file_path_in
                },
                if app.sender_file_path_in.is_empty() {
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
    let name_focused = app.sender_focused_field == 1;
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
                &app.sender_name_in,
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
    let avatar_focused = app.sender_focused_field == 2;
    let avatar_style = if avatar_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let avatar_text = app
        .sender_avatar_path_in
        .read()
        .unwrap()
        .clone()
        .unwrap_or(String::from("No avatar selected"));

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
                if app.sender_avatar_path_in.read().unwrap().is_some() {
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

    let mut file_items: Vec<ListItem> = Vec::new();
    let sender_files = app.sender_files_in.read().unwrap().clone();

    // Files list
    if sender_files.is_empty() {
        file_items.append(&mut vec![ListItem::new(vec![
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
        ])]);
    } else {
        let mut items: Vec<ListItem> = sender_files
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
            .collect();
        file_items.append(&mut items);
    };

    let files_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(
            if app
                .sender_files_in
                .read()
                .unwrap()
                .clone()
                .is_empty()
            {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Blue)
            },
        )
        .title(format!(
            " Selected Files ({}) ",
            app.sender_files_in.read().unwrap().clone().len()
        ))
        .title_style(Style::default().fg(Color::White).bold());

    let files_list = List::new(file_items).block(files_block);
    f.render_widget(files_list, right_chunks[0]);

    // Instructions in left panel
    let instructions_content = if app
        .sender_files_in
        .read()
        .unwrap()
        .clone()
        .is_empty()
    {
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
                    format!(
                        "{} file(s) selected",
                        app.sender_files_in.read().unwrap().clone().len()
                    ),
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
    let send_button_focused = app.sender_focused_field == 3;
    let can_send = !app
        .sender_files_in
        .read()
        .unwrap()
        .clone()
        .is_empty();

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
    app: &App,
    key: KeyEvent,
) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Tab, _) => {
            app.sender_focused_field = (app.sender_focused_field + 1) % 4;
        }
        (KeyCode::BackTab, _) => {
            app.sender_focused_field = if app.sender_focused_field == 0 {
                3
            } else {
                app.sender_focused_field - 1
            };
        }
        (KeyCode::Enter, _) => {
            match app.sender_focused_field {
                0 => {
                    // Add file
                    if !app.sender_file_path_in.is_empty() {
                        if app.sender_file_path_in == "browse" {
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
                            let path = PathBuf::from(&app.sender_file_path_in);
                            if path.exists() {
                                app.add_file(path);
                                app.sender_file_path_in.clear();
                            } else {
                                app.show_error(
                                    "File does not exist".to_string(),
                                );
                            }
                        }
                    }
                }
                3 => {
                    app.start_send_files().await?;
                }
                _ => {}
            }
        }
        (KeyCode::Char(c), modifiers) => match modifiers {
            KeyModifiers::NONE => match app.sender_focused_field {
                0 => {
                    app.sender_file_path_in.push(c);
                }
                1 => {
                    app.sender_name_in.push(c);
                }
                2 => {
                    if app.sender_avatar_path_in.read().unwrap().is_none() {
                        *app.sender_avatar_path_in.write().unwrap() =
                            Some(String::new());
                    }
                    if let Some(avatar_path) =
                        app.sender_avatar_path_in.write().unwrap().as_mut()
                    {
                        avatar_path.push(c);
                    }
                }
                _ => {}
            },
            KeyModifiers::CONTROL => match c {
                'c' => match app.sender_focused_field {
                    0 => {
                        app.sender_file_path_in.clear();
                    }
                    1 => {
                        app.sender_name_in.clear();
                    }
                    2 => {
                        *app.sender_avatar_path_in.write().unwrap() = None;
                    }
                    _ => {}
                },
                'o' => {
                    if app.sender_focused_field == 0 {
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
        (KeyCode::Backspace, _) => match app.sender_focused_field {
            0 => {
                app.sender_file_path_in.pop();
            }
            1 => {
                app.sender_name_in.pop();
            }
            2 => {
                if let Some(avatar_path) =
                    app.sender_avatar_path_in.write().unwrap().as_mut()
                {
                    if avatar_path.is_empty() {
                        *app.sender_avatar_path_in.write().unwrap() = None;
                    } else {
                        avatar_path.pop();
                    }
                }
            }
            _ => {}
        },
        (KeyCode::Delete, _) => {
            if app.sender_focused_field == 0
                && !app
                    .sender_files_in
                    .read()
                    .unwrap()
                    .clone()
                    .is_empty()
            {
                // Remove last added file
                app.sender_files_in.read().unwrap().clone().pop();
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn handle_file_browser_input(
    app: &App,
    key: KeyEvent,
) -> Result<()> {
    if let Some(browser) = app.file_browser.write().unwrap().as_mut() {
        match key.code {
            KeyCode::Esc => {
                // Add selected files to the app and close browser
                let selected = browser.get_selected_files();
                for file in selected {
                    app.add_file(file);
                }
                app.close_file_browser();
            }
            KeyCode::Up => {
                browser.navigate_up();
            }
            KeyCode::Down => {
                browser.navigate_down();
            }
            KeyCode::Enter => {
                if let Some(path) = browser.enter_selected() {
                    // File selected, add it
                    app.add_file(path);
                } else {
                    browser.toggle_selected();
                }
            }
            KeyCode::Char(' ') => {
                browser.toggle_selected();
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                browser.toggle_hidden();
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                browser.cycle_sort_mode();
            }
            KeyCode::Tab => {
                // Select current directory and close
                app.close_file_browser();
            }
            _ => {}
        }
    }
    Ok(())
}

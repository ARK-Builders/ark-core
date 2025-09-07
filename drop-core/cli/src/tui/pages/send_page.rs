use crate::tui::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::path::PathBuf;

pub fn render_send_page(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(8), // File selection
            Constraint::Length(5), // Name field
            Constraint::Length(5), // Avatar field
            Constraint::Min(0),    // Files list
            Constraint::Length(3), // Instructions
        ])
        .split(area);

    // Title
    let title = Paragraph::new("📤 Send Files")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // File input field
    let file_input_style = if app.send_focused_field == 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let file_input = Paragraph::new(vec![
        Line::from("Enter file path (or 'browse' to select):"),
        Line::from(""),
        Line::from(Span::styled(
            if app.send_file_input.is_empty() {
                "/path/to/file.txt"
            } else {
                &app.send_file_input
            },
            if app.send_file_input.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default()),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to add file, ", Style::default()),
            Span::styled(
                "Ctrl+C",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to clear", Style::default()),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Add Files")
            .border_style(file_input_style),
    );
    f.render_widget(file_input, chunks[1]);

    // Name field
    let name_style = if app.send_focused_field == 1 {
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
            &app.send_name,
            Style::default().fg(Color::White),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Sender Name")
            .border_style(name_style),
    );
    f.render_widget(name_field, chunks[2]);

    // Avatar field
    let avatar_style = if app.send_focused_field == 2 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let avatar_text = app.send_avatar_path.as_deref().unwrap_or("None");
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
    f.render_widget(avatar_field, chunks[3]);

    // Files list
    let file_items: Vec<ListItem> = app
        .send_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            ListItem::new(format!("{}. 📄 {}", i + 1, file.display()))
                .style(Style::default().fg(Color::Cyan))
        })
        .collect();

    let files_list = List::new(file_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Selected Files ({})", app.send_files.len())),
    );
    f.render_widget(files_list, chunks[4]);

    // Instructions
    let instructions = if app.send_files.is_empty() {
        "Add at least one file to proceed"
    } else {
        "Press Enter when focused on Send button to start transfer"
    };

    let send_button_style = if app.send_focused_field == 3 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let footer = Paragraph::new(vec![
        Line::from(instructions),
        Line::from(vec![
            Span::styled("[ ", Style::default()),
            Span::styled("Send Files", send_button_style),
            Span::styled(" ]", Style::default()),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[5]);
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
                            // In a real implementation, you'd open a file
                            // browser
                            app.show_error("File browser not implemented in TUI mode. Please enter full path.".to_string());
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

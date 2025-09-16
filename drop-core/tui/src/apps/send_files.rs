use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, OpenFileBrowserRequest, Page, SortMode,
};
use arkdrop_common::FileData;
use arkdropx_sender::{
    SendFilesRequest, SenderConfig, SenderFile, SenderProfile,
};
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Clone, PartialEq)]
enum TransferState {
    NoTransfer,
    OngoingTransfer,
}

pub struct SendFilesApp {
    b: Arc<dyn AppBackend>,

    menu: RwLock<ListState>,
    transfer_state: RwLock<TransferState>,

    file_in: RwLock<String>,
    selected_files_in: RwLock<Vec<PathBuf>>,
}

impl App for SendFilesApp {
    fn draw(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let transfer_state = self.transfer_state.read().unwrap().clone();

        self.update_transfer_state();
        match transfer_state {
            TransferState::OngoingTransfer => {
                self.draw_ongoing_transfer_view(f, area);
            }
            _ => {
                self.draw_new_transfer_view(f, area);
            }
        }
    }

    fn handle_control(&self, ev: &Event) {
        let transfer_state = self.transfer_state.read().unwrap().clone();

        match transfer_state {
            TransferState::OngoingTransfer => {
                self.handle_ongoing_transfer_controls(ev);
            }
            _ => {
                self.handle_new_transfer_controls(ev);
            }
        }
    }
}

impl AppFileBrowserSubscriber for SendFilesApp {
    fn on_cancel(&self) {
        self.b
            .get_navigation()
            .replace_with(Page::SendFiles);
    }

    fn on_save(&self, ev: AppFileBrowserSaveEvent) {
        self.b
            .get_navigation()
            .replace_with(Page::SendFiles);

        let mut selected_files = ev.selected_files;
        self.selected_files_in
            .write()
            .unwrap()
            .append(&mut selected_files);
    }
}

impl SendFilesApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let mut menu = ListState::default();
        menu.select(Some(0));

        Self {
            b,

            menu: RwLock::new(menu),
            transfer_state: RwLock::new(TransferState::NoTransfer),

            file_in: RwLock::new(String::new()),
            selected_files_in: RwLock::new(Vec::new()),
        }
    }

    fn update_transfer_state(&self) {
        let has_ongoing_transfer = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble()
            .is_some();

        let new_state = if has_ongoing_transfer {
            TransferState::OngoingTransfer
        } else {
            TransferState::NoTransfer
        };

        *self.transfer_state.write().unwrap() = new_state;
    }

    fn draw_ongoing_transfer_view(&self, f: &mut Frame, area: Rect) {
        let blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(5), // Header with transfer info
                Constraint::Length(8), // Transfer summary card
                Constraint::Length(6), // Action buttons
                Constraint::Min(0),    // Instructions
            ])
            .split(area);

        self.draw_ongoing_transfer_header(f, blocks[0]);
        self.draw_transfer_summary_card(f, blocks[1]);
        self.draw_ongoing_transfer_actions(f, blocks[2]);
        self.draw_ongoing_transfer_instructions(f, blocks[3]);
    }

    fn draw_new_transfer_view(&self, f: &mut Frame, area: Rect) {
        let blocks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([
                Constraint::Percentage(60), // Left side
                Constraint::Percentage(40), // Right side
            ])
            .split(area);

        let left_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(6), // File selection
                Constraint::Min(0),    // Instructions
            ])
            .split(blocks[0]);

        let right_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Files list
                Constraint::Length(5), // Send button
            ])
            .split(blocks[1]);

        self.draw_title(f, left_blocks[0]);
        self.draw_file_input(f, left_blocks[1]);
        self.draw_instructions(f, left_blocks[2]);

        self.draw_file_list(f, right_blocks[0]);
        self.draw_send_files_button(f, right_blocks[1]);
    }

    fn draw_ongoing_transfer_header(&self, f: &mut Frame, area: Rect) {
        let bubble = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble();

        let (status_text, status_color, status_icon) =
            if let Some(bubble) = bubble {
                (
                    format!(
                        "Transfer Code: {} {}",
                        bubble.get_ticket(),
                        bubble.get_confirmation()
                    ),
                    Color::Blue,
                    "🔄",
                )
            } else {
                ("Transfer in progress...".to_string(), Color::Yellow, "⏳")
            };

        let header_content = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color).bold(),
                ),
                Span::styled(
                    "Active Transfer",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📱 ", Style::default().fg(Color::Blue)),
                Span::styled(status_text, Style::default().fg(Color::Cyan)),
            ]),
        ];

        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Transfer Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let header = Paragraph::new(header_content)
            .block(header_block)
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn draw_transfer_summary_card(&self, f: &mut Frame, area: Rect) {
        let summary_content = vec![
            Line::from(vec![
                Span::styled("📊 ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Transfer Overview",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "Files are being sent to the connected device",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "View detailed progress in the transfer monitor",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "You can start a new transfer after this one completes",
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let summary_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Green))
            .title(" Summary ")
            .title_style(Style::default().fg(Color::White).bold());

        let summary = Paragraph::new(summary_content)
            .block(summary_block)
            .alignment(Alignment::Left);

        f.render_widget(summary, area);
    }

    fn draw_ongoing_transfer_actions(&self, f: &mut Frame, area: Rect) {
        let is_focused = self.menu.read().unwrap().selected() == Some(0);

        let actions_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    if is_focused {
                        "▶ "
                    } else {
                        "  "
                    },
                    if is_focused {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    "📈 View Transfer Progress",
                    if is_focused {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Blue)
                            .bold()
                    } else {
                        Style::default().fg(Color::Blue).bold()
                    },
                ),
            ]),
            Line::from(""),
        ];

        let actions_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(if is_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Blue)
            })
            .title(" Actions ")
            .title_style(Style::default().fg(Color::White).bold());

        let actions = Paragraph::new(actions_content)
            .block(actions_block)
            .alignment(Alignment::Center);

        f.render_widget(actions, area);
    }

    fn draw_ongoing_transfer_instructions(&self, f: &mut Frame, area: Rect) {
        let instructions_content = vec![
            Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    "Transfer Management",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    " - View detailed transfer progress",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                Span::styled(
                    " - Return to main menu",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "The transfer will continue in the background",
                    Style::default().fg(Color::Gray).italic(),
                ),
            ]),
        ];

        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Help ")
            .title_style(Style::default().fg(Color::White).bold());

        let instructions = Paragraph::new(instructions_content)
            .block(instructions_block)
            .alignment(Alignment::Left);

        f.render_widget(instructions, area);
    }

    fn handle_ongoing_transfer_controls(&self, ev: &Event) {
        match ev {
            Event::Key(key) => match key.code {
                KeyCode::Enter => {
                    self.b
                        .get_navigation()
                        .navigate_to(Page::SendFilesProgress);
                }
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_new_transfer_controls(&self, ev: &Event) {
        match ev {
            Event::Key(key) => match key.code {
                KeyCode::Down => {
                    self.navigate_down();
                }
                KeyCode::Up => {
                    self.navigate_up();
                }
                KeyCode::Tab => {
                    self.navigate_down();
                }
                KeyCode::BackTab => {
                    self.navigate_up();
                }
                KeyCode::Enter => {
                    if let KeyModifiers::CONTROL = key.modifiers {
                        self.send_files();
                    } else {
                        self.perform_action()
                    }
                }
                KeyCode::Backspace => {
                    match self.menu.read().unwrap().selected() {
                        Some(0) => {
                            self.file_in.write().unwrap().pop();
                        }
                        _ => {}
                    }
                }
                KeyCode::Delete => {
                    if self.menu.read().unwrap().selected() == Some(0)
                        && !self.selected_files_in.read().unwrap().is_empty()
                    {
                        // Remove last added file
                        self.selected_files_in.write().unwrap().pop();
                    }
                }
                KeyCode::Char(c) => match key.modifiers {
                    KeyModifiers::NONE => {
                        match self.menu.read().unwrap().selected() {
                            Some(0) => {
                                self.file_in.write().unwrap().push(c);
                            }
                            _ => {}
                        }
                    }
                    KeyModifiers::CONTROL => match c {
                        'c' => match self.menu.read().unwrap().selected() {
                            Some(0) => {
                                self.file_in.write().unwrap().clear();
                            }
                            _ => {}
                        },
                        'o' => {
                            self.open_file_browser();
                        }
                        _ => {}
                    },
                    _ => {}
                },
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn draw_title(&self, f: &mut Frame<'_>, area: Rect) {
        let title_content = vec![Line::from(vec![
            Span::styled("📤 ", Style::default().fg(Color::Green).bold()),
            Span::styled(
                "Send Files",
                Style::default().fg(Color::White).bold(),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Green))
            .title(" New Transfer ")
            .title_style(Style::default().fg(Color::White).bold());

        let title = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title, area);
    }

    fn draw_file_input(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.menu.read().unwrap().selected() == Some(0);

        let style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let file_in = self.file_in.read().unwrap();

        let content = vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Blue)),
                Span::styled("File Path:", Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "▶ ",
                    if is_focused {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    if file_in.is_empty() {
                        "/path/to/your/file.txt"
                    } else {
                        &file_in
                    },
                    if file_in.is_empty() {
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
                Span::styled(
                    "Ctrl+O",
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::styled(" browse • ", Style::default().fg(Color::Gray)),
                Span::styled("Ctrl+C", Style::default().fg(Color::Red).bold()),
                Span::styled(" clear", Style::default().fg(Color::Gray)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(style)
            .title(" Add Files ")
            .title_style(Style::default().fg(Color::White).bold());

        let file_input = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        f.render_widget(file_input, area);
    }

    fn draw_file_list(&self, f: &mut Frame<'_>, area: Rect) {
        let mut file_items: Vec<ListItem> = Vec::new();
        let selected_files_in = self.selected_files_in.read().unwrap().clone();

        if selected_files_in.is_empty() {
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
            let mut items: Vec<ListItem> = selected_files_in
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
                            Span::styled(
                                "📄 ",
                                Style::default().fg(Color::Blue),
                            ),
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
            .border_style(if selected_files_in.clone().is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Blue)
            })
            .title(format!(
                " Selected Files ({}) ",
                selected_files_in.clone().len()
            ))
            .title_style(Style::default().fg(Color::White).bold());

        let files_list = List::new(file_items).block(files_block);

        f.render_widget(files_list, area);
    }

    fn draw_instructions(&self, f: &mut Frame<'_>, area: Rect) {
        let selected_files_in = self.selected_files_in.read().unwrap();
        let instructions_content = if selected_files_in.is_empty() {
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
                        format!("{} file(s) selected", selected_files_in.len()),
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

        f.render_widget(instructions, area);
    }

    fn draw_send_files_button(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.menu.read().unwrap().selected() == Some(1);
        let selected_files_in = self.selected_files_in.read().unwrap();
        let has_files = !selected_files_in.is_empty();

        let send_button_style = if is_focused && has_files {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .bold()
        } else if is_focused {
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Black)
                .bold()
        } else if has_files {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let button_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    if is_focused {
                        "▶ "
                    } else {
                        "  "
                    },
                    if is_focused {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    if has_files {
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
            .border_style(if is_focused {
                Style::default().fg(Color::Yellow)
            } else if has_files {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .title(" Action ")
            .title_style(Style::default().fg(Color::White).bold());

        let send_button = Paragraph::new(button_content)
            .block(send_button_block)
            .alignment(Alignment::Center);

        f.render_widget(send_button, area);
    }

    fn navigate_down(&self) {
        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected();

        match selected {
            Some(i) => menu.select(Some((i + 1) % 2)),
            None => menu.select(Some(0)),
        }
    }

    fn navigate_up(&self) {
        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected();

        match selected {
            Some(i) => menu.select(Some((i + 1) % 2)),
            None => menu.select(Some(0)),
        }
    }

    fn add_file(&self, file: PathBuf) {
        self.selected_files_in.write().unwrap().push(file);
    }

    fn open_file_browser(&self) {
        self.b
            .get_file_browser_manager()
            .open_file_browser(OpenFileBrowserRequest {
                from: Page::SendFiles,
                mode: BrowserMode::SelectMultiFiles,
                sort: SortMode::Name,
            });
    }

    fn perform_action(&self) {
        let menu = self.menu.read().unwrap();

        match menu.selected() {
            Some(0) => {
                let mut file_in = self.file_in.write().unwrap();
                if !file_in.is_empty() {
                    if file_in.as_str() == "browse" {
                        self.open_file_browser();
                    } else {
                        let path = PathBuf::from(&file_in.clone());
                        if path.exists() {
                            self.add_file(path);
                            file_in.clear();
                        } else {
                            // TODO: info | log exception on TUI
                        }
                    }
                }
            }
            Some(1) => {
                self.send_files();
            }
            _ => {}
        }
    }

    fn send_files(&self) {
        if let Some(req) = self.make_send_files_request() {
            self.b.get_send_files_manager().send_files(req);
            // Navigate to progress view after starting transfer
            self.b
                .get_navigation()
                .navigate_to(Page::SendFilesProgress);
        }
    }

    fn make_send_files_request(&self) -> Option<SendFilesRequest> {
        let files = self.get_sender_files();

        if files.is_empty() {
            return None;
        }

        // TODO: low | use AppConfig to build the request
        Some(SendFilesRequest {
            files,
            profile: SenderProfile {
                name: "tui-sender".to_string(),
                avatar_b64: None,
            },
            config: SenderConfig::default(), // TODO: extra | get from config
        })
    }

    fn get_sender_files(&self) -> Vec<SenderFile> {
        let selected_files_in = self.selected_files_in.read().unwrap();

        if selected_files_in.is_empty() {
            return Vec::new();
        }

        return selected_files_in
            .iter()
            .filter_map(|f| {
                if let Some(name) = f.file_name() {
                    if let Ok(data) = FileData::new(f.clone()) {
                        let name = name.to_string_lossy().to_string();

                        return Some(SenderFile {
                            name,
                            data: Arc::new(data),
                        });
                    }
                }

                return None;
            })
            .collect();
    }
}

use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, OpenFileBrowserRequest, Page, SortMode,
};
use arkdropx_receiver::ReceiveFilesRequest;
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, ListState, Paragraph},
};

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Clone, PartialEq)]
enum TransferState {
    NoTransfer,
    OngoingTransfer,
    PreparingNewTransfer,
}

pub struct ReceiveFilesApp {
    b: Arc<dyn AppBackend>,

    menu: RwLock<ListState>,
    transfer_state: RwLock<TransferState>,

    ticket_in: RwLock<String>,
    confirmation_in: RwLock<String>,
    out_dir_in: RwLock<String>,
    selected_files_in: RwLock<Vec<PathBuf>>,
}

impl App for ReceiveFilesApp {
    fn draw(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let transfer_state = self.transfer_state.read().unwrap().clone();

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

impl AppFileBrowserSubscriber for ReceiveFilesApp {
    fn on_cancel(&self) {
        self.b
            .get_navigation()
            .replace_with(Page::ReceiveFiles);
    }

    fn on_save(&self, ev: AppFileBrowserSaveEvent) {
        self.b
            .get_navigation()
            .replace_with(Page::ReceiveFiles);

        let mut selected_files = ev.selected_files;
        self.selected_files_in
            .write()
            .unwrap()
            .append(&mut selected_files);
    }
}

impl ReceiveFilesApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let mut menu = ListState::default();
        menu.select(Some(0));

        Self {
            b,

            menu: RwLock::new(menu),
            transfer_state: RwLock::new(TransferState::NoTransfer),

            ticket_in: RwLock::new(String::new()),
            confirmation_in: RwLock::new(String::new()),
            out_dir_in: RwLock::new(String::new()),
            selected_files_in: RwLock::new(Vec::new()),
        }
    }

    fn update_transfer_state(&self) {
        let has_ongoing_transfer = self
            .b
            .get_receive_files_manager()
            .get_receive_files_bubble()
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
                Constraint::Percentage(50), // Left side - transfer details
                Constraint::Percentage(50), // Right side - action
            ])
            .split(area);

        let left_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Page title
                Constraint::Length(6), // Ticket field
                Constraint::Length(6), // Confirmation field
                Constraint::Length(6), // Output directory field
                Constraint::Min(0),    // Instructions
            ])
            .split(blocks[0]);

        let right_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0), // Receive button
            ])
            .split(blocks[1]);

        self.draw_title(f, left_blocks[0]);
        self.draw_ticket_field(f, left_blocks[1]);
        self.draw_confirmation_field(f, left_blocks[2]);
        self.draw_output_field(f, left_blocks[3]);
        self.draw_instructions(f, left_blocks[4]);
        self.draw_receive_button(f, right_blocks[0]);
    }

    fn draw_ongoing_transfer_header(&self, f: &mut Frame, area: Rect) {
        let header_content = vec![
            Line::from(vec![
                Span::styled("📥 ", Style::default().fg(Color::Blue).bold()),
                Span::styled(
                    "Active Transfer",
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📱 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Receiving files from sender...",
                    Style::default().fg(Color::Cyan),
                ),
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
                    "Files are being received from the connected device",
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
                        .navigate_to(Page::ReceiveFilesProgress);
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
        if let Event::Key(key) = ev {
            let menu = self.get_menu();
            let curr_selected = menu.selected().unwrap_or(0);
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            match key.code {
                KeyCode::Down | KeyCode::Tab => {
                    self.navigate_down();
                }
                KeyCode::Up | KeyCode::BackTab => {
                    self.navigate_up();
                }
                KeyCode::Enter => {
                    if has_ctrl {
                        self.receive_files();
                    } else {
                        match curr_selected {
                            2 => {
                                self.open_dir_browser();
                            }
                            3 => {
                                self.receive_files();
                            }
                            _ => {}
                        }
                    }
                }
                KeyCode::Backspace => match curr_selected {
                    0 => {
                        self.ticket_in.write().unwrap().pop();
                    }
                    1 => {
                        self.confirmation_in.write().unwrap().pop();
                    }
                    2 => {
                        self.out_dir_in.write().unwrap().pop();
                    }
                    _ => {}
                },
                KeyCode::Char(c) => {
                    if has_ctrl && (c == 'o' || c == 'O') {
                        self.open_dir_browser();
                    } else {
                        match curr_selected {
                            0 => {
                                self.ticket_in.write().unwrap().push(c);
                            }
                            1 => {
                                self.confirmation_in.write().unwrap().push(c);
                            }
                            2 => {
                                self.out_dir_in.write().unwrap().push(c);
                            }
                            _ => {}
                        }
                    }
                }
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            }
        }
    }

    fn draw_title(&self, f: &mut Frame<'_>, area: Rect) {
        // Check for ongoing transfer on each draw
        self.update_transfer_state();

        let title_content = vec![Line::from(vec![
            Span::styled("📥 ", Style::default().fg(Color::Blue).bold()),
            Span::styled(
                "Receive Files",
                Style::default().fg(Color::White).bold(),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" New Transfer ")
            .title_style(Style::default().fg(Color::White).bold());

        let title = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title, area);
    }

    fn draw_ticket_field(&self, f: &mut Frame<'_>, area: Rect) {
        let menu = self.get_menu();
        let ticket_in = self.get_ticket_in();

        let is_focused = menu.selected() == Some(0);
        let style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let ticket_content = vec![
            Line::from(vec![
                Span::styled("🎫 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Transfer Ticket:",
                    Style::default().fg(Color::White),
                ),
            ]),
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
                    if ticket_in.is_empty() {
                        "Enter ticket from sender..."
                    } else {
                        &ticket_in
                    },
                    if ticket_in.is_empty() {
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
            .border_style(style)
            .title(" Transfer Ticket ")
            .title_style(Style::default().fg(Color::White).bold());

        let ticket_field = Paragraph::new(ticket_content)
            .block(ticket_block)
            .alignment(Alignment::Left);

        f.render_widget(ticket_field, area);
    }

    fn draw_confirmation_field(&self, f: &mut Frame<'_>, area: Rect) {
        let menu = self.get_menu();
        let confirmation_in = self.get_confirmation_in();
        let curr_selected = menu.selected().unwrap_or(0);

        let confirmation_focused = curr_selected == 1;
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
                    if confirmation_in.is_empty() {
                        "Enter confirmation code..."
                    } else {
                        &confirmation_in
                    },
                    if confirmation_in.is_empty() {
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
        f.render_widget(confirmation_field, area);
    }

    fn draw_output_field(&self, f: &mut Frame<'_>, area: Rect) {
        let menu = self.get_menu();
        let out_dir_in = self.get_out_dir_in();
        let curr_selected = menu.selected().unwrap_or(0);

        let output_focused = curr_selected == 2;
        let output_style = if output_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let output_content = vec![
            Line::from(vec![
                Span::styled("📂 ", Style::default().fg(Color::Magenta)),
                Span::styled(
                    "Save Location:",
                    Style::default().fg(Color::White),
                ),
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
                    if out_dir_in.is_empty() {
                        "/path/to/save/directory"
                    } else {
                        &out_dir_in
                    },
                    if out_dir_in.is_empty() {
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
                Span::styled(
                    "Ctrl+O",
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::styled(
                    " system dialog",
                    Style::default().fg(Color::Gray),
                ),
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

        f.render_widget(output_field, area);
    }

    fn draw_instructions(&self, f: &mut Frame<'_>, area: Rect) {
        let instructions_content = if self.can_receive() {
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

        f.render_widget(instructions, area);
    }

    fn draw_receive_button(&self, f: &mut Frame<'_>, area: Rect) {
        let menu = self.get_menu();
        let out_dir_in = self.get_out_dir_in();
        let can_receive = self.can_receive();
        let curr_selected = menu.selected().unwrap_or(0);

        let receive_button_focused = curr_selected == 3;

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
                if out_dir_in.is_empty() {
                    "No directory selected"
                } else {
                    &out_dir_in
                },
                if out_dir_in.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::Cyan).italic()
                },
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

        f.render_widget(receive_button, area);
    }

    fn navigate_down(&self) {
        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected();

        match selected {
            Some(i) => menu.select(Some((i + 1) % 4)),
            None => menu.select(Some(0)),
        }
    }

    fn navigate_up(&self) {
        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected();

        match selected {
            Some(i) => menu.select(Some((i + 3) % 4)),
            None => menu.select(Some(0)),
        }
    }

    fn open_dir_browser(&self) {
        self.b
            .get_file_browser_manager()
            .open_file_browser(OpenFileBrowserRequest {
                from: Page::ReceiveFiles,
                mode: BrowserMode::SelectMultiFiles,
                sort: SortMode::Name,
            });
    }

    fn receive_files(&self) {
        if let Some(req) = self.make_receive_files_request() {
            self.b
                .get_receive_files_manager()
                .receive_files(req);
            // Navigate to progress view after starting transfer
            self.b
                .get_navigation()
                .navigate_to(Page::ReceiveFilesProgress);
        }
    }

    fn make_receive_files_request(&self) -> Option<ReceiveFilesRequest> {
        if !self.can_receive() {
            return None;
        }

        // TODO: Implement proper ReceiveFilesRequest creation
        // This should use the ticket, confirmation, and output directory
        None
    }

    fn get_menu(&self) -> ListState {
        self.menu.read().unwrap().clone()
    }

    fn get_ticket_in(&self) -> String {
        self.ticket_in.read().unwrap().clone()
    }

    fn get_confirmation_in(&self) -> String {
        self.confirmation_in.read().unwrap().clone()
    }

    fn get_out_dir_in(&self) -> String {
        self.out_dir_in.read().unwrap().clone()
    }

    fn can_receive(&self) -> bool {
        !self.get_ticket_in().is_empty()
            && !self.get_confirmation_in().is_empty()
            && !self.get_out_dir_in().is_empty()
    }
}

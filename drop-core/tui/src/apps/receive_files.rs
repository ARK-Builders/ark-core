use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, OpenFileBrowserRequest, Page, SortMode,
};
use arkdropx_receiver::{ReceiveFilesRequest, ReceiverProfile};
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
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize},
    },
};

#[derive(Clone, PartialEq)]
enum TransferState {
    NoTransfer,
    OngoingTransfer,
    PreparingNewTransfer,
}

#[derive(Clone, PartialEq)]
enum InputField {
    Ticket,
    Confirmation,
    OutputDirectory,
    ReceiveButton,
}

pub struct ReceiveFilesApp {
    b: Arc<dyn AppBackend>,

    // UI State
    menu: RwLock<ListState>,
    transfer_state: RwLock<TransferState>,
    selected_field: AtomicUsize,

    // Input fields
    ticket_in: RwLock<String>,
    confirmation_in: RwLock<String>,
    out_dir_in: RwLock<String>,
    selected_files_in: RwLock<Vec<PathBuf>>,

    // Text input state
    is_editing_field: Arc<AtomicBool>,
    current_editing_field: Arc<AtomicUsize>,
    input_buffer: Arc<RwLock<String>>,
    cursor_position: Arc<AtomicUsize>,

    // Status and feedback
    status_message: Arc<RwLock<String>>,
    is_processing: Arc<AtomicBool>,
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
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;
            let transfer_state = self.transfer_state.read().unwrap().clone();

            match transfer_state {
                TransferState::OngoingTransfer => {
                    self.handle_ongoing_transfer_controls(key.code, has_ctrl);
                }
                _ => {
                    let is_editing = self.is_editing_field();

                    if is_editing {
                        self.handle_text_input_controls(key.code, has_ctrl);
                    } else {
                        self.handle_navigation_controls(key.code, has_ctrl);
                    }
                }
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

        if let Some(selected_path) = ev.selected_files.first() {
            *self.out_dir_in.write().unwrap() =
                selected_path.to_string_lossy().to_string();
            self.set_status_message(&format!(
                "Output directory set: {}",
                selected_path.display()
            ));
        }
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
            selected_field: AtomicUsize::new(0),

            ticket_in: RwLock::new(String::new()),
            confirmation_in: RwLock::new(String::new()),
            out_dir_in: RwLock::new(String::new()),
            selected_files_in: RwLock::new(Vec::new()),

            // Text input state
            is_editing_field: Arc::new(AtomicBool::new(false)),
            current_editing_field: Arc::new(AtomicUsize::new(0)),
            input_buffer: Arc::new(RwLock::new(String::new())),
            cursor_position: Arc::new(AtomicUsize::new(0)),

            // Status and feedback
            status_message: Arc::new(RwLock::new(
                "Enter transfer details to receive files".to_string(),
            )),
            is_processing: Arc::new(AtomicBool::new(false)),
        }
    }

    fn handle_ongoing_transfer_controls(
        &self,
        key_code: KeyCode,
        has_ctrl: bool,
    ) {
        if has_ctrl {
            match key_code {
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            }
        } else {
            match key_code {
                KeyCode::Enter => {
                    self.b
                        .get_navigation()
                        .navigate_to(Page::ReceiveFilesProgress);
                }
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => {}
            }
        }
    }

    fn handle_text_input_controls(&self, key_code: KeyCode, has_ctrl: bool) {
        match key_code {
            KeyCode::Enter => {
                self.finish_editing_field();
            }
            KeyCode::Esc => {
                self.cancel_editing_field();
            }
            KeyCode::Backspace => {
                self.handle_backspace();
            }
            KeyCode::Delete => {
                self.handle_delete();
            }
            KeyCode::Left => {
                self.move_cursor_left();
            }
            KeyCode::Right => {
                self.move_cursor_right();
            }
            KeyCode::Home => {
                self.move_cursor_home();
            }
            KeyCode::End => {
                self.move_cursor_end();
            }
            KeyCode::Char(c) => {
                if has_ctrl {
                    match c {
                        'v' | 'V' => {
                            self.set_status_message("Paste not available - type or use middle mouse button if supported");
                        }
                        'a' | 'A' => self.move_cursor_home(),
                        'e' | 'E' => self.move_cursor_end(),
                        'u' | 'U' => self.clear_input(),
                        'w' | 'W' => self.delete_word_backward(),
                        'o' | 'O' => {
                            if self
                                .current_editing_field
                                .load(std::sync::atomic::Ordering::Relaxed)
                                == 2
                            {
                                self.cancel_editing_field();
                                self.open_dir_browser();
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.insert_char(c);
                }
            }
            _ => {}
        }
    }

    fn handle_navigation_controls(&self, key_code: KeyCode, has_ctrl: bool) {
        if has_ctrl {
            match key_code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.receive_files();
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    self.open_dir_browser();
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.clear_all_fields();
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.show_paste_instructions();
                }
                _ => {}
            }
        } else {
            match key_code {
                KeyCode::Up | KeyCode::BackTab => {
                    self.navigate_up();
                }
                KeyCode::Down | KeyCode::Tab => {
                    self.navigate_down();
                }
                KeyCode::Enter => {
                    self.activate_current_field();
                }
                KeyCode::Esc => {
                    if self.is_processing() {
                        self.set_status_message("Operation cancelled");
                        self.set_processing(false);
                    } else {
                        self.b.get_navigation().go_back();
                    }
                }
                _ => {}
            }
        }
    }

    fn show_paste_instructions(&self) {
        let current_field = self.get_selected_field();
        let field_name = match current_field {
            0 => "ticket",
            1 => "confirmation code",
            2 => "output directory",
            _ => "field",
        };

        self.set_status_message(&format!(
            "To paste {}: 1) Press Enter to edit, 2) Use terminal's paste (Ctrl+Shift+V or middle-click), 3) Press Enter to save",
            field_name
        ));
    }

    fn is_editing_field(&self) -> bool {
        self.is_editing_field
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn start_editing_field(&self, field_index: usize) {
        let current_value = match field_index {
            0 => self.ticket_in.read().unwrap().clone(),
            1 => self.confirmation_in.read().unwrap().clone(),
            2 => self.out_dir_in.read().unwrap().clone(),
            _ => String::new(),
        };

        *self.input_buffer.write().unwrap() = current_value.clone();
        self.cursor_position
            .store(current_value.len(), std::sync::atomic::Ordering::Relaxed);
        self.current_editing_field
            .store(field_index, std::sync::atomic::Ordering::Relaxed);
        self.is_editing_field
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let field_name = match field_index {
            0 => "ticket",
            1 => "confirmation code",
            2 => "output directory",
            _ => "field",
        };

        self.set_status_message(&format!(
            "Editing {} - Enter to save, Esc to cancel, Ctrl+Shift+V to paste from terminal",
            field_name
        ));
    }

    fn finish_editing_field(&self) {
        let input_text = self.input_buffer.read().unwrap().clone();
        let trimmed_text = input_text.trim();
        let field_index = self
            .current_editing_field
            .load(std::sync::atomic::Ordering::Relaxed);

        match field_index {
            0 => {
                *self.ticket_in.write().unwrap() = trimmed_text.to_string();
                if trimmed_text.is_empty() {
                    self.set_status_message("Ticket cleared");
                } else {
                    self.set_status_message("Ticket updated");
                }
            }
            1 => {
                *self.confirmation_in.write().unwrap() =
                    trimmed_text.to_string();
                if trimmed_text.is_empty() {
                    self.set_status_message("Confirmation code cleared");
                } else {
                    self.set_status_message("Confirmation code updated");
                }
            }
            2 => {
                *self.out_dir_in.write().unwrap() = trimmed_text.to_string();
                if trimmed_text.is_empty() {
                    self.set_status_message("Output directory cleared");
                } else {
                    self.set_status_message("Output directory updated");
                }
            }
            _ => {}
        }

        self.is_editing_field
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn cancel_editing_field(&self) {
        self.is_editing_field
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.set_status_message("Field editing cancelled");
    }

    fn insert_char(&self, c: char) {
        let mut buffer = self.input_buffer.write().unwrap();
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        buffer.insert(cursor_pos, c);
        self.cursor_position
            .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
    }

    fn handle_backspace(&self) {
        let mut buffer = self.input_buffer.write().unwrap();
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos > 0 {
            buffer.remove(cursor_pos - 1);
            self.cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn handle_delete(&self) {
        let mut buffer = self.input_buffer.write().unwrap();
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos < buffer.len() {
            buffer.remove(cursor_pos);
        }
    }

    fn move_cursor_left(&self) {
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos > 0 {
            self.cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_right(&self) {
        let buffer = self.input_buffer.read().unwrap();
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos < buffer.len() {
            self.cursor_position
                .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_home(&self) {
        self.cursor_position
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_end(&self) {
        let buffer = self.input_buffer.read().unwrap();
        self.cursor_position
            .store(buffer.len(), std::sync::atomic::Ordering::Relaxed);
    }

    fn clear_input(&self) {
        self.input_buffer.write().unwrap().clear();
        self.cursor_position
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    fn delete_word_backward(&self) {
        let mut buffer = self.input_buffer.write().unwrap();
        let cursor_pos = self
            .cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos == 0 {
            return;
        }

        let mut new_pos = cursor_pos;
        let chars: Vec<char> = buffer.chars().collect();

        // Skip whitespace backwards
        while new_pos > 0 && chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }

        // Delete word characters backwards
        while new_pos > 0 && !chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }

        buffer.drain(new_pos..cursor_pos);
        self.cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_input_fields(&self) -> Vec<InputField> {
        vec![
            InputField::Ticket,
            InputField::Confirmation,
            InputField::OutputDirectory,
            InputField::ReceiveButton,
        ]
    }

    fn get_selected_field(&self) -> usize {
        self.selected_field
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn navigate_up(&self) {
        let fields = self.get_input_fields();
        let current = self.get_selected_field();
        let new_index = if current > 0 {
            current - 1
        } else {
            fields.len() - 1
        };

        self.selected_field
            .store(new_index, std::sync::atomic::Ordering::Relaxed);
        self.menu.write().unwrap().select(Some(new_index));
    }

    fn navigate_down(&self) {
        let fields = self.get_input_fields();
        let current = self.get_selected_field();
        let new_index = if current < fields.len() - 1 {
            current + 1
        } else {
            0
        };

        self.selected_field
            .store(new_index, std::sync::atomic::Ordering::Relaxed);
        self.menu.write().unwrap().select(Some(new_index));
    }

    fn activate_current_field(&self) {
        let fields = self.get_input_fields();
        let current = self.get_selected_field();

        if let Some(field) = fields.get(current) {
            match field {
                InputField::Ticket => {
                    self.start_editing_field(0);
                }
                InputField::Confirmation => {
                    self.start_editing_field(1);
                }
                InputField::OutputDirectory => {
                    self.start_editing_field(2);
                }
                InputField::ReceiveButton => {
                    self.receive_files();
                }
            }
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

    fn clear_all_fields(&self) {
        *self.ticket_in.write().unwrap() = String::new();
        *self.confirmation_in.write().unwrap() = String::new();
        *self.out_dir_in.write().unwrap() = String::new();
        self.set_status_message("All fields cleared");
    }

    fn open_dir_browser(&self) {
        self.set_status_message("Opening directory browser...");
        self.b
            .get_file_browser_manager()
            .open_file_browser(OpenFileBrowserRequest {
                from: Page::ReceiveFiles,
                mode: BrowserMode::SelectDirectory,
                sort: SortMode::Name,
            });
    }

    fn receive_files(&self) {
        if let Some(req) = self.make_receive_files_request() {
            self.set_processing(true);
            self.set_status_message("Starting file reception...");
            self.b
                .get_receive_files_manager()
                .receive_files(req);
            self.b
                .get_navigation()
                .navigate_to(Page::ReceiveFilesProgress);
        } else {
            self.set_status_message(
                "Missing required information - check ticket and confirmation",
            );
        }
    }

    fn make_receive_files_request(&self) -> Option<ReceiveFilesRequest> {
        if !self.can_receive() {
            return None;
        }

        let config = self.b.get_config();

        return Some(ReceiveFilesRequest {
            ticket: self.get_ticket_in(),
            confirmation: self.get_confirmation_in().parse().unwrap(),
            profile: ReceiverProfile {
                name: config.get_avatar_name(),
                avatar_b64: config.get_avatar_base64(),
            },
            config: None,
        });
    }

    fn set_status_message(&self, message: &str) {
        *self.status_message.write().unwrap() = message.to_string();
    }

    fn set_processing(&self, processing: bool) {
        self.is_processing
            .store(processing, std::sync::atomic::Ordering::Relaxed);
    }

    fn is_processing(&self) -> bool {
        self.is_processing
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_status_message(&self) -> String {
        self.status_message.read().unwrap().clone()
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
        let is_focused = self.get_selected_field() == 0;
        let is_editing = self.is_editing_field()
            && self
                .current_editing_field
                .load(std::sync::atomic::Ordering::Relaxed)
                == 0;
        let ticket_in = self.get_ticket_in();

        let style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "│ (typing...)".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '│');
                }
                display
            }
        } else if ticket_in.is_empty() {
            "Enter ticket from sender...".to_string()
        } else {
            ticket_in.clone()
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
                    if is_focused || is_editing {
                        "▶ "
                    } else {
                        "  "
                    },
                    if is_focused || is_editing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    display_text,
                    if is_editing {
                        Style::default().fg(Color::White)
                    } else if ticket_in.is_empty() {
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
        let is_focused = self.get_selected_field() == 1;
        let is_editing = self.is_editing_field()
            && self
                .current_editing_field
                .load(std::sync::atomic::Ordering::Relaxed)
                == 1;
        let confirmation_in = self.get_confirmation_in();

        let confirmation_style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "│ (typing...)".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '│');
                }
                display
            }
        } else if confirmation_in.is_empty() {
            "Enter confirmation code...".to_string()
        } else {
            confirmation_in.clone()
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
                    if is_focused || is_editing {
                        "▶ "
                    } else {
                        "  "
                    },
                    if is_focused || is_editing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    display_text,
                    if is_editing {
                        Style::default().fg(Color::White)
                    } else if confirmation_in.is_empty() {
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
        let is_focused = self.get_selected_field() == 2;
        let is_editing = self.is_editing_field()
            && self
                .current_editing_field
                .load(std::sync::atomic::Ordering::Relaxed)
                == 2;
        let out_dir_in = self.get_out_dir_in();

        let output_style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "│ (typing...)".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '│');
                }
                display
            }
        } else if out_dir_in.is_empty() {
            "/path/to/save/directory".to_string()
        } else {
            out_dir_in.clone()
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
                    if is_focused || is_editing {
                        "▶ "
                    } else {
                        "  "
                    },
                    if is_focused || is_editing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    display_text,
                    if is_editing {
                        Style::default().fg(Color::White)
                    } else if out_dir_in.is_empty() {
                        Style::default().fg(Color::DarkGray).italic()
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
                Span::styled(" edit • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "Ctrl+O",
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::styled(" browse", Style::default().fg(Color::Gray)),
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
        let is_editing = self.is_editing_field();
        let can_receive = self.can_receive();
        let status_message = self.get_status_message();

        let instructions_content = if is_editing {
            vec![
                Line::from(vec![
                    Span::styled("✏️ ", Style::default().fg(Color::Green)),
                    Span::styled(
                        "Text Input Mode",
                        Style::default().fg(Color::Green).bold(),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Enter",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(
                        " - Save • ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                    Span::styled(" - Cancel", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled(
                        "Ctrl+Shift+V",
                        Style::default().fg(Color::Cyan).bold(),
                    ),
                    Span::styled(
                        " - Paste from terminal clipboard",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        "Ctrl+A",
                        Style::default().fg(Color::Cyan).bold(),
                    ),
                    Span::styled(
                        " - Home • ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        "Ctrl+E",
                        Style::default().fg(Color::Cyan).bold(),
                    ),
                    Span::styled(" - End • ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        "Ctrl+U",
                        Style::default().fg(Color::Yellow).bold(),
                    ),
                    Span::styled(" - Clear", Style::default().fg(Color::Gray)),
                ]),
            ]
        } else if can_receive {
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
                    Span::styled(
                        "Ctrl+R",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(
                        " - Start receiving • ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        "Ctrl+P",
                        Style::default().fg(Color::Cyan).bold(),
                    ),
                    Span::styled(
                        " - Paste help • ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        "Ctrl+C",
                        Style::default().fg(Color::Red).bold(),
                    ),
                    Span::styled(
                        " - Clear all",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
                    Span::styled(
                        status_message,
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
                        "Enter ticket, confirmation code, and output directory",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        "Ctrl+P",
                        Style::default().fg(Color::Cyan).bold(),
                    ),
                    Span::styled(
                        " - Show paste instructions",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
                    Span::styled(
                        status_message,
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]
        };

        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Status & Help ")
            .title_style(Style::default().fg(Color::White).bold());

        let instructions = Paragraph::new(instructions_content)
            .block(instructions_block)
            .alignment(Alignment::Left);

        f.render_widget(instructions, area);
    }

    fn draw_receive_button(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 3;
        let out_dir_in = self.get_out_dir_in();
        let can_receive = self.can_receive();

        let receive_button_style = if is_focused && can_receive {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .bold()
        } else if is_focused {
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
            .border_style(if is_focused {
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
}

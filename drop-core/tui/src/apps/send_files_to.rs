use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, ControlCapture, OpenFileBrowserRequest, Page, SortMode,
};
use arkdrop_common::FileData;
use arkdropx_sender::{
    SenderConfig, SenderFile, SenderProfile, send_files_to::SendFilesToRequest,
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
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize},
    },
};

#[derive(Clone, PartialEq)]
enum InputField {
    Ticket,
    Confirmation,
    FilePath,
    SendButton,
}

pub struct SendFilesToApp {
    b: Arc<dyn AppBackend>,

    // UI State
    menu: RwLock<ListState>,
    selected_field: AtomicUsize,

    // Connection input fields
    ticket_in: RwLock<String>,
    confirmation_in: RwLock<String>,

    // File selection
    selected_files_in: RwLock<Vec<PathBuf>>,

    // Text input state (shared for all editable fields)
    is_editing_field: Arc<AtomicBool>,
    current_editing_field: Arc<AtomicUsize>,
    field_input_buffer: Arc<RwLock<String>>,
    field_cursor_position: Arc<AtomicUsize>,

    // Status and feedback
    status_message: Arc<RwLock<String>>,
}

impl App for SendFilesToApp {
    fn draw(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        self.draw_main_view(f, area);
    }

    fn handle_control(&self, ev: &Event) -> Option<ControlCapture> {
        let is_editing = self.is_editing_field();

        if is_editing {
            self.handle_text_input_controls(ev)
        } else {
            self.handle_navigation_controls(ev)
        }
    }
}

impl AppFileBrowserSubscriber for SendFilesToApp {
    fn on_cancel(&self) {
        self.b
            .get_navigation()
            .replace_with(Page::SendFilesTo);
    }

    fn on_save(&self, ev: AppFileBrowserSaveEvent) {
        self.b
            .get_navigation()
            .replace_with(Page::SendFilesTo);

        let mut selected_files = ev.selected_files;
        let count = selected_files.len();
        self.selected_files_in
            .write()
            .unwrap()
            .append(&mut selected_files);

        self.set_status_message(&format!("Added {} file(s)", count));
    }
}

impl SendFilesToApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let mut menu = ListState::default();
        menu.select(Some(0));

        Self {
            b,

            menu: RwLock::new(menu),
            selected_field: AtomicUsize::new(0),

            ticket_in: RwLock::new(String::new()),
            confirmation_in: RwLock::new(String::new()),
            selected_files_in: RwLock::new(Vec::new()),

            is_editing_field: Arc::new(AtomicBool::new(false)),
            current_editing_field: Arc::new(AtomicUsize::new(0)),
            field_input_buffer: Arc::new(RwLock::new(String::new())),
            field_cursor_position: Arc::new(AtomicUsize::new(0)),

            status_message: Arc::new(RwLock::new(
                "Enter ticket and confirmation from receiver's QR code"
                    .to_string(),
            )),
        }
    }

    // ─── Input Handling ────────────────────────────────────────────────────

    fn handle_text_input_controls(&self, ev: &Event) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            match key.code {
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
                    if has_ctrl {
                        self.move_cursor_left_by_word();
                    } else {
                        self.move_cursor_left();
                    }
                }
                KeyCode::Right => {
                    if has_ctrl {
                        self.move_cursor_right_by_word();
                    } else {
                        self.move_cursor_right();
                    }
                }
                KeyCode::Home => {
                    self.move_cursor_home();
                }
                KeyCode::End => {
                    self.move_cursor_end();
                }
                KeyCode::Char(c) => {
                    self.insert_char(c);
                }
                _ => return None,
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }

    fn handle_navigation_controls(&self, ev: &Event) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            if has_ctrl {
                match key.code {
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        self.send_files_to();
                    }
                    KeyCode::Char('o') | KeyCode::Char('O') => {
                        self.open_file_browser();
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.clear_all();
                    }
                    _ => return None,
                }
            } else {
                match key.code {
                    KeyCode::Up | KeyCode::BackTab => {
                        self.navigate_up();
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        self.navigate_down();
                    }
                    KeyCode::Enter => {
                        self.activate_current_field();
                    }
                    KeyCode::Delete => {
                        self.remove_last_file();
                    }
                    KeyCode::Esc => {
                        self.b.get_navigation().go_back();
                    }
                    _ => return None,
                }
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }

    // ─── Field Editing ─────────────────────────────────────────────────────

    fn is_editing_field(&self) -> bool {
        self.is_editing_field
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_current_editing_field(&self) -> usize {
        self.current_editing_field
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn start_editing_field(&self, field_idx: usize) {
        let current_value = match field_idx {
            0 => self.ticket_in.read().unwrap().clone(),
            1 => self.confirmation_in.read().unwrap().clone(),
            2 => String::new(), // File path always starts empty
            _ => String::new(),
        };

        *self.field_input_buffer.write().unwrap() = current_value.clone();
        self.field_cursor_position
            .store(current_value.len(), std::sync::atomic::Ordering::Relaxed);
        self.current_editing_field
            .store(field_idx, std::sync::atomic::Ordering::Relaxed);
        self.is_editing_field
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let field_name = match field_idx {
            0 => "ticket",
            1 => "confirmation code",
            2 => "file path",
            _ => "field",
        };

        self.set_status_message(&format!(
            "Editing {} - Enter to save, Esc to cancel",
            field_name
        ));
    }

    fn finish_editing_field(&self) {
        let input_text = self.field_input_buffer.read().unwrap().clone();
        let trimmed_text = input_text.trim();
        let field_index = self.get_current_editing_field();

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
                // File path - try to add the file
                if !trimmed_text.is_empty() {
                    if trimmed_text == "browse" {
                        self.is_editing_field
                            .store(false, std::sync::atomic::Ordering::Relaxed);
                        self.open_file_browser();
                        return;
                    }
                    let path = PathBuf::from(trimmed_text);
                    if path.exists() {
                        self.add_file(path.clone());
                        self.set_status_message(&format!(
                            "Added file: {}",
                            path.display()
                        ));
                    } else {
                        self.set_status_message(&format!(
                            "File not found: {}",
                            trimmed_text
                        ));
                    }
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

    // ─── Text Cursor Operations ────────────────────────────────────────────

    fn insert_char(&self, c: char) {
        let mut buffer = self.field_input_buffer.write().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        buffer.insert(cursor_pos, c);
        self.field_cursor_position
            .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
    }

    fn handle_backspace(&self) {
        let mut buffer = self.field_input_buffer.write().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos > 0 {
            buffer.remove(cursor_pos - 1);
            self.field_cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn handle_delete(&self) {
        let mut buffer = self.field_input_buffer.write().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos < buffer.len() {
            buffer.remove(cursor_pos);
        }
    }

    fn move_cursor_left(&self) {
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos > 0 {
            self.field_cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_left_by_word(&self) {
        let buffer = self.field_input_buffer.read().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos == 0 {
            return;
        }

        let mut new_pos = cursor_pos;
        let chars: Vec<char> = buffer.chars().collect();

        while new_pos > 0 && chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }

        while new_pos > 0 && !chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }

        self.field_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_right(&self) {
        let buffer = self.field_input_buffer.read().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos < buffer.len() {
            self.field_cursor_position
                .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_right_by_word(&self) {
        let buffer = self.field_input_buffer.read().unwrap();
        let cursor_pos = self
            .field_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if buffer.is_empty() || cursor_pos >= buffer.len() {
            return;
        }

        let mut new_pos = cursor_pos;
        let chars: Vec<char> = buffer.chars().collect();
        let last_pos = chars.len();

        while new_pos < last_pos
            && chars
                .get(new_pos)
                .is_some_and(|c| c.is_whitespace())
        {
            new_pos += 1;
        }

        while new_pos < last_pos
            && chars
                .get(new_pos)
                .is_some_and(|c| !c.is_whitespace())
        {
            new_pos += 1;
        }

        self.field_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_home(&self) {
        self.field_cursor_position
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_end(&self) {
        let buffer = self.field_input_buffer.read().unwrap();
        self.field_cursor_position
            .store(buffer.len(), std::sync::atomic::Ordering::Relaxed);
    }

    // ─── Navigation ────────────────────────────────────────────────────────

    fn get_input_fields(&self) -> Vec<InputField> {
        vec![
            InputField::Ticket,
            InputField::Confirmation,
            InputField::FilePath,
            InputField::SendButton,
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
                InputField::FilePath => {
                    self.start_editing_field(2);
                }
                InputField::SendButton => {
                    self.send_files_to();
                }
            }
        }
    }

    // ─── File Operations ───────────────────────────────────────────────────

    fn add_file(&self, file: PathBuf) {
        self.selected_files_in.write().unwrap().push(file);
    }

    fn remove_last_file(&self) {
        if let Some(removed_file) =
            self.selected_files_in.write().unwrap().pop()
        {
            self.set_status_message(&format!(
                "Removed file: {}",
                removed_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        } else {
            self.set_status_message("No files to remove");
        }
    }

    fn clear_all(&self) {
        let file_count = self.selected_files_in.read().unwrap().len();
        self.selected_files_in.write().unwrap().clear();
        *self.ticket_in.write().unwrap() = String::new();
        *self.confirmation_in.write().unwrap() = String::new();
        self.set_status_message(&format!(
            "Cleared all fields and {} file(s)",
            file_count
        ));
    }

    fn open_file_browser(&self) {
        self.set_status_message("Opening file browser...");
        self.b
            .get_file_browser_manager()
            .open_file_browser(OpenFileBrowserRequest {
                from: Page::SendFilesTo,
                mode: BrowserMode::SelectMultiFiles,
                sort: SortMode::Name,
            });
    }

    // ─── Send Operation ────────────────────────────────────────────────────

    fn send_files_to(&self) {
        if let Some(req) = self.make_send_files_to_request() {
            self.set_status_message("Connecting to receiver...");
            self.b
                .get_send_files_to_manager()
                .send_files_to(req);
            self.b
                .get_navigation()
                .navigate_to(Page::SendFilesToProgress);
        } else {
            self.set_status_message(
                "Missing required information - check ticket, confirmation, and files",
            );
        }
    }

    fn make_send_files_to_request(&self) -> Option<SendFilesToRequest> {
        if !self.can_send() {
            return None;
        }

        let files = self.get_sender_files();
        if files.is_empty() {
            return None;
        }

        let config = self.b.get_config();
        let confirmation: u8 = self.get_confirmation_in().parse().ok()?;

        Some(SendFilesToRequest {
            ticket: self.get_ticket_in(),
            confirmation,
            files,
            profile: SenderProfile {
                name: config.get_avatar_name(),
                avatar_b64: config.get_avatar_base64(),
            },
            config: SenderConfig::default(),
        })
    }

    fn get_sender_files(&self) -> Vec<SenderFile> {
        let selected_files_in = self.selected_files_in.read().unwrap();

        if selected_files_in.is_empty() {
            return Vec::new();
        }

        selected_files_in
            .iter()
            .filter_map(|f| {
                if let Some(name) = f.file_name()
                    && let Ok(data) = FileData::new(f.clone())
                {
                    let name = name.to_string_lossy().to_string();
                    return Some(SenderFile {
                        name,
                        data: Arc::new(data),
                    });
                }
                None
            })
            .collect()
    }

    fn can_send(&self) -> bool {
        let ticket = self.get_ticket_in();
        let confirmation = self.get_confirmation_in();
        let has_files = !self.selected_files_in.read().unwrap().is_empty();

        !ticket.is_empty()
            && !confirmation.is_empty()
            && confirmation.parse::<u8>().is_ok()
            && has_files
    }

    // ─── Helpers ───────────────────────────────────────────────────────────

    fn set_status_message(&self, message: &str) {
        *self.status_message.write().unwrap() = message.to_string();
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

    // ─── Drawing ───────────────────────────────────────────────────────────

    fn draw_main_view(&self, f: &mut Frame, area: Rect) {
        let blocks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([
                Constraint::Percentage(55), /* Left side - connection +
                                             * files input */
                Constraint::Percentage(45), /* Right side - file list + send
                                             * button */
            ])
            .split(area);

        let left_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(5), // Ticket field
                Constraint::Length(5), // Confirmation field
                Constraint::Length(6), // File path input
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
        self.draw_ticket_field(f, left_blocks[1]);
        self.draw_confirmation_field(f, left_blocks[2]);
        self.draw_file_input(f, left_blocks[3]);
        self.draw_instructions(f, left_blocks[4]);

        self.draw_file_list(f, right_blocks[0]);
        self.draw_send_button(f, right_blocks[1]);
    }

    fn draw_title(&self, f: &mut Frame<'_>, area: Rect) {
        let title_content = vec![Line::from(vec![
            Span::styled("🔗 ", Style::default().fg(Color::Magenta).bold()),
            Span::styled(
                "Send to QR",
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                " - Connect to waiting receiver",
                Style::default().fg(Color::Gray),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Send Files To Receiver ")
            .title_style(Style::default().fg(Color::White).bold());

        let title = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title, area);
    }

    fn draw_ticket_field(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 0;
        let is_editing =
            self.is_editing_field() && self.get_current_editing_field() == 0;
        let ticket_in = self.get_ticket_in();

        let style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.field_input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .field_cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "|".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '|');
                }
                display
            }
        } else if ticket_in.is_empty() {
            "Paste ticket from receiver's QR...".to_string()
        } else {
            truncate_string(&ticket_in, 45)
        };

        let ticket_content = vec![Line::from(vec![
            Span::styled(
                if is_focused || is_editing {
                    ">"
                } else {
                    " "
                },
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(" Ticket: ", Style::default().fg(Color::White)),
            Span::styled(
                display_text,
                if is_editing {
                    Style::default().fg(Color::White)
                } else if ticket_in.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
        ])];

        let ticket_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(style)
            .title(" Connection Ticket ")
            .title_style(Style::default().fg(Color::White).bold());

        let ticket_field = Paragraph::new(ticket_content)
            .block(ticket_block)
            .alignment(Alignment::Left);

        f.render_widget(ticket_field, area);
    }

    fn draw_confirmation_field(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 1;
        let is_editing =
            self.is_editing_field() && self.get_current_editing_field() == 1;
        let confirmation_in = self.get_confirmation_in();

        let style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.field_input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .field_cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "|".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '|');
                }
                display
            }
        } else if confirmation_in.is_empty() {
            "Enter 2-digit code (0-99)...".to_string()
        } else {
            confirmation_in.clone()
        };

        let confirmation_content = vec![Line::from(vec![
            Span::styled(
                if is_focused || is_editing {
                    ">"
                } else {
                    " "
                },
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(" Confirmation: ", Style::default().fg(Color::White)),
            Span::styled(
                display_text,
                if is_editing {
                    Style::default().fg(Color::White)
                } else if confirmation_in.is_empty() {
                    Style::default().fg(Color::DarkGray).italic()
                } else {
                    Style::default().fg(Color::Green).bold()
                },
            ),
        ])];

        let confirmation_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(style)
            .title(" Confirmation Code ")
            .title_style(Style::default().fg(Color::White).bold());

        let confirmation_field = Paragraph::new(confirmation_content)
            .block(confirmation_block)
            .alignment(Alignment::Left);

        f.render_widget(confirmation_field, area);
    }

    fn draw_file_input(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 2;
        let is_editing =
            self.is_editing_field() && self.get_current_editing_field() == 2;

        let style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.field_input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .field_cursor_position
                .load(std::sync::atomic::Ordering::Relaxed);

            if buffer.is_empty() {
                "|".to_string()
            } else {
                let mut display = buffer.clone();
                if cursor_pos <= display.len() {
                    display.insert(cursor_pos, '|');
                }
                display
            }
        } else {
            "/path/to/file or 'browse'".to_string()
        };

        let content = vec![
            Line::from(vec![
                Span::styled(
                    if is_focused || is_editing {
                        ">"
                    } else {
                        " "
                    },
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(" Add file: ", Style::default().fg(Color::White)),
                Span::styled(
                    display_text,
                    if is_editing {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray).italic()
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Ctrl+O",
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(" browse | ", Style::default().fg(Color::Gray)),
                Span::styled("Del", Style::default().fg(Color::Red).bold()),
                Span::styled(" remove last", Style::default().fg(Color::Gray)),
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
        let selected_files_in = self.selected_files_in.read().unwrap().clone();

        let file_items: Vec<ListItem> = if selected_files_in.is_empty() {
            vec![ListItem::new(vec![
                Line::from(vec![Span::styled(
                    "  No files selected",
                    Style::default().fg(Color::DarkGray).italic(),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "  Add files to send to the receiver",
                    Style::default().fg(Color::Gray),
                )]),
            ])]
        } else {
            selected_files_in
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
                                "  ",
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
                                truncate_string(file_path, 35),
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
            .border_style(if selected_files_in.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Magenta)
            })
            .title(format!(" Files to Send ({}) ", selected_files_in.len()))
            .title_style(Style::default().fg(Color::White).bold());

        let files_list = List::new(file_items).block(files_block);

        f.render_widget(files_list, area);
    }

    fn draw_instructions(&self, f: &mut Frame<'_>, area: Rect) {
        let is_editing = self.is_editing_field();
        let can_send = self.can_send();
        let status_message = self.get_status_message();

        let instructions_content = if is_editing {
            vec![
                Line::from(vec![
                    Span::styled(
                        "  Editing - ",
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        "Enter",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(" save | ", Style::default().fg(Color::Gray)),
                    Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                    Span::styled(" cancel", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  ", Style::default().fg(Color::Blue)),
                    Span::styled(
                        status_message,
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]
        } else if can_send {
            vec![
                Line::from(vec![
                    Span::styled(
                        "  Ready! ",
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        "Ctrl+S",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(" send | ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        "Ctrl+C",
                        Style::default().fg(Color::Red).bold(),
                    ),
                    Span::styled(
                        " clear all",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  ", Style::default().fg(Color::Blue)),
                    Span::styled(
                        status_message,
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]
        } else {
            vec![
                Line::from(vec![Span::styled(
                    "  Enter ticket, confirmation, and add files",
                    Style::default().fg(Color::Yellow),
                )]),
                Line::from(vec![
                    Span::styled("  ", Style::default().fg(Color::Blue)),
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
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let instructions = Paragraph::new(instructions_content)
            .block(instructions_block)
            .alignment(Alignment::Left);

        f.render_widget(instructions, area);
    }

    fn draw_send_button(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 3;
        let can_send = self.can_send();

        let button_style = if is_focused && can_send {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .bold()
        } else if is_focused {
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Black)
                .bold()
        } else if can_send {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let button_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    if is_focused {
                        "> "
                    } else {
                        "  "
                    },
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    if can_send {
                        "  Send Files"
                    } else {
                        "  Cannot Send"
                    },
                    button_style,
                ),
            ]),
            Line::from(""),
        ];

        let button_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(if is_focused {
                Style::default().fg(Color::Yellow)
            } else if can_send {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .title(" Action ")
            .title_style(Style::default().fg(Color::White).bold());

        let button = Paragraph::new(button_content)
            .block(button_block)
            .alignment(Alignment::Center);

        f.render_widget(button, area);
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

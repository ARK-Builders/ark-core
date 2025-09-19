use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, ControlCapture, OpenFileBrowserRequest, Page, SortMode,
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
    ops::Deref,
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
}

#[derive(Clone, PartialEq)]
enum InputField {
    FilePath,
    SendButton,
}

pub struct SendFilesApp {
    b: Arc<dyn AppBackend>,

    // UI State
    menu: RwLock<ListState>,
    transfer_state: RwLock<TransferState>,
    selected_field: AtomicUsize,

    // Input fields
    file_in: RwLock<String>,
    selected_files_in: RwLock<Vec<PathBuf>>,

    // Text input state
    is_editing_path: Arc<AtomicBool>,
    path_input_buffer: Arc<RwLock<String>>,
    path_cursor_position: Arc<AtomicUsize>,

    // Status and feedback
    status_message: Arc<RwLock<String>>,
}

impl App for SendFilesApp {
    fn draw(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        self.refresh_transfer_state();

        match self.get_transfer_state() {
            TransferState::OngoingTransfer => {
                self.draw_ongoing_transfer_view(f, area);
            }
            _ => {
                self.draw_new_transfer_view(f, area);
            }
        }
    }

    fn handle_control(&self, ev: &Event) -> Option<ControlCapture> {
        let transfer_state = self.transfer_state.read().unwrap().clone();
        match transfer_state {
            TransferState::OngoingTransfer => {
                return self.handle_ongoing_transfer_controls(ev);
            }
            _ => {
                let is_editing = self.is_editing_path();

                if is_editing {
                    return self.handle_text_input_controls(ev);
                } else {
                    return self.handle_navigation_controls(ev);
                }
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

        self.set_status_message(&format!(
            "Added {} file(s)",
            selected_files.len()
        ));
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
            selected_field: AtomicUsize::new(0),

            file_in: RwLock::new(String::new()),
            selected_files_in: RwLock::new(Vec::new()),

            // Text input state
            is_editing_path: Arc::new(AtomicBool::new(false)),
            path_input_buffer: Arc::new(RwLock::new(String::new())),
            path_cursor_position: Arc::new(AtomicUsize::new(0)),

            // Status and feedback
            status_message: Arc::new(RwLock::new(
                "Add files to send to another device".to_string(),
            )),
        }
    }

    fn get_transfer_state(&self) -> TransferState {
        return self.transfer_state.read().unwrap().clone();
    }

    fn handle_ongoing_transfer_controls(
        &self,
        ev: &Event,
    ) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            match key.code {
                KeyCode::Enter => {
                    self.b
                        .get_navigation()
                        .navigate_to(Page::SendFilesProgress);
                }
                KeyCode::Esc => {
                    self.b.get_navigation().go_back();
                }
                _ => return None,
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }

    fn handle_text_input_controls(&self, ev: &Event) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            match key.code {
                KeyCode::Enter => {
                    if has_ctrl {
                        self.cancel_editing_path();
                        self.open_file_browser();
                    } else {
                        self.finish_editing_path();
                    }
                }
                KeyCode::Esc => {
                    self.cancel_editing_path();
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
                        self.send_files();
                    }
                    KeyCode::Char('o') | KeyCode::Char('O') => {
                        self.open_file_browser();
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.clear_selected_files();
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

    fn is_editing_path(&self) -> bool {
        self.is_editing_path
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn start_editing_path(&self) {
        let current_path = self.file_in.read().unwrap().clone();
        *self.path_input_buffer.write().unwrap() = current_path.clone();

        self.path_cursor_position
            .store(current_path.len(), std::sync::atomic::Ordering::Relaxed);

        self.is_editing_path
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.set_status_message(
            "Editing file path - Enter to add, Esc to cancel, Ctrl+O to browse",
        );
    }

    fn finish_editing_path(&self) {
        let input_text = self.path_input_buffer.read().unwrap().clone();
        let trimmed_text = input_text.trim();

        if !trimmed_text.is_empty() {
            if trimmed_text == "browse" {
                self.open_file_browser();
            } else {
                let path = PathBuf::from(trimmed_text);
                if path.exists() {
                    self.add_file(path.clone());
                    *self.file_in.write().unwrap() = String::new();
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

        self.is_editing_path
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn cancel_editing_path(&self) {
        self.is_editing_path
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.set_status_message("Path editing cancelled");
    }

    fn insert_char(&self, c: char) {
        let mut buffer = self.path_input_buffer.write().unwrap();
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        buffer.insert(cursor_pos, c);
        self.path_cursor_position
            .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
    }

    fn handle_backspace(&self) {
        let mut buffer = self.path_input_buffer.write().unwrap();
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos > 0 {
            buffer.remove(cursor_pos - 1);
            self.path_cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn handle_delete(&self) {
        let mut buffer = self.path_input_buffer.write().unwrap();
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);

        if cursor_pos < buffer.len() {
            buffer.remove(cursor_pos);
        }
    }

    fn move_cursor_left(&self) {
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos > 0 {
            self.path_cursor_position
                .store(cursor_pos - 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_left_by_word(&self) {
        let buffer = self.path_input_buffer.read().unwrap();
        let cursor_pos = self
            .path_cursor_position
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

        // Skip word characters backwards
        while new_pos > 0 && !chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }

        self.path_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_right_by_word(&self) {
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        let buffer = self.path_input_buffer.read().unwrap();
        let last_pos = buffer.len() - 1;

        if cursor_pos == last_pos {
            return;
        }

        let mut new_pos = cursor_pos;
        let chars: Vec<char> = buffer.chars().collect();

        // Skip whitespace forward
        while new_pos < last_pos && chars[new_pos + 1].is_whitespace() {
            new_pos += 1;
        }

        // Skip word characters forward
        while new_pos < last_pos && !chars[new_pos + 1].is_whitespace() {
            new_pos += 1;
        }

        self.path_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_right(&self) {
        let buffer = self.path_input_buffer.read().unwrap();
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        if cursor_pos < buffer.len() {
            self.path_cursor_position
                .store(cursor_pos + 1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn move_cursor_home(&self) {
        self.path_cursor_position
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    fn move_cursor_end(&self) {
        let buffer = self.path_input_buffer.read().unwrap();
        self.path_cursor_position
            .store(buffer.len(), std::sync::atomic::Ordering::Relaxed);
    }

    fn delete_word_backward(&self) {
        let mut buffer = self.path_input_buffer.write().unwrap();
        let cursor_pos = self
            .path_cursor_position
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
        self.path_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn delete_word_forward(&self) {
        let cursor_pos = self
            .path_cursor_position
            .load(std::sync::atomic::Ordering::Relaxed);
        let buffer = self.path_input_buffer.read().unwrap();
        let last_pos = buffer.len() - 1;

        if cursor_pos == last_pos {
            return;
        }

        let mut buffer = self.path_input_buffer.write().unwrap();
        let mut new_pos = cursor_pos;
        let chars: Vec<char> = buffer.chars().collect();

        // Skip whitespace backwards
        while new_pos < last_pos && chars[new_pos + 1].is_whitespace() {
            new_pos += 1;
        }

        // Delete word characters backwards
        while new_pos < last_pos && !chars[new_pos + 1].is_whitespace() {
            new_pos += 1;
        }

        buffer.drain(new_pos..cursor_pos);
        self.path_cursor_position
            .store(new_pos, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_input_fields(&self) -> Vec<InputField> {
        vec![InputField::FilePath, InputField::SendButton]
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
                InputField::FilePath => {
                    self.start_editing_path();
                }
                InputField::SendButton => {
                    self.send_files();
                }
            }
        }
    }

    fn refresh_transfer_state(&self) {
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

        let mut transfer_state = self.transfer_state.write().unwrap();

        if transfer_state.deref() != &new_state {
            *transfer_state = new_state;
        }
    }

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

    fn clear_selected_files(&self) {
        let count = self.selected_files_in.read().unwrap().len();
        self.selected_files_in.write().unwrap().clear();
        self.set_status_message(&format!("Cleared {} file(s)", count));
    }

    fn open_file_browser(&self) {
        self.set_status_message("Opening file browser...");
        self.b
            .get_file_browser_manager()
            .open_file_browser(OpenFileBrowserRequest {
                from: Page::SendFiles,
                mode: BrowserMode::SelectMultiFiles,
                sort: SortMode::Name,
            });
    }

    fn send_files(&self) {
        if let Some(req) = self.make_send_files_request() {
            self.set_status_message("Starting file transfer...");
            self.b.get_send_files_manager().send_files(req);
            self.b
                .get_navigation()
                .navigate_to(Page::SendFilesProgress);
        } else {
            self.set_status_message("No files selected to send");
        }
    }

    fn make_send_files_request(&self) -> Option<SendFilesRequest> {
        let files = self.get_sender_files();

        if files.is_empty() {
            return None;
        }

        let profile = self.b.get_config();

        Some(SendFilesRequest {
            files,
            profile: SenderProfile {
                name: profile.get_avatar_name(),
                avatar_b64: profile.get_avatar_base64(),
            },
            config: SenderConfig::default(),
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

    fn set_status_message(&self, message: &str) {
        *self.status_message.write().unwrap() = message.to_string();
    }

    fn get_status_message(&self) -> String {
        self.status_message.read().unwrap().clone()
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
        let is_focused = self.get_selected_field() == 0;
        let is_editing = self.is_editing_path();

        let style = if is_focused || is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let display_text = if is_editing {
            let buffer = self.path_input_buffer.read().unwrap().clone();
            let cursor_pos = self
                .path_cursor_position
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
        } else {
            let file_in = self.file_in.read().unwrap();
            if file_in.is_empty() {
                "/path/to/your/file.txt".to_string()
            } else {
                file_in.clone()
            }
        };

        let content = vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Blue)),
                Span::styled("File Path:", Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "▶ ",
                    if is_focused || is_editing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    display_text,
                    if is_editing {
                        Style::default().fg(Color::White)
                    } else if self.file_in.read().unwrap().is_empty() {
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
                Span::styled(" browse • ", Style::default().fg(Color::Gray)),
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
        let is_editing = self.is_editing_path();
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
                        " - Save path • ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                    Span::styled(" - Cancel", Style::default().fg(Color::Gray)),
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
        } else if selected_files_in.is_empty() {
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
                        "Enter full file paths or use 'browse' command",
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
                    Span::styled(
                        "Ctrl+S",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(
                        " - Send files • ",
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

    fn draw_send_files_button(&self, f: &mut Frame<'_>, area: Rect) {
        let is_focused = self.get_selected_field() == 1;
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
}

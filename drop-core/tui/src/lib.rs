use anyhow::Result;
use arkdrop_common::{Profile, create_sender_files, get_default_out_dir};
use arkdropx_receiver::{
    ReceiveFilesBubble, ReceiveFilesFile, ReceiveFilesRequest,
    ReceiveFilesSubscriber, ReceiverProfile, receive_files,
};
use arkdropx_sender::{
    SendFilesBubble, SendFilesRequest, SendFilesSubscriber, SenderConfig,
    SenderProfile, send_files,
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
            KeyEvent, KeyEventKind, KeyModifiers,
        },
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    },
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListState, Paragraph, Wrap},
};
use std::{
    collections::HashMap, fs, io, io::Write, path::PathBuf, sync::RwLock,
    time::Duration,
};
use tokio::time::Instant;
use uuid::Uuid;

use crate::{
    components::{BrowserMode, FileBrowser},
    pages::{
        handle_config_page_input, handle_main_page_input,
        handle_receive_page_input, handle_send_page_input, render_config_page,
        render_help_page, render_main_page, render_receive_page,
        render_receive_progress_page, render_send_page,
        render_send_progress_page,
    },
};

mod components;
mod pages;

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Main,
    Send,
    Receive,
    Config,
    Help,
    SendProgress,
    ReceiveProgress,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Sending,
    Receiving,
}

pub struct App {
    pub id: String,

    pub state: RwLock<AppState>,

    pub current_page: RwLock<Page>,
    pub previous_pages: RwLock<Vec<Page>>,

    pub main_menu_state: ListState,
    pub config_menu_state: ListState,

    // Send page fields
    pub sender_files: RwLock<Vec<PathBuf>>,
    pub sender_name: String,
    pub sender_avatar_path: RwLock<Option<String>>,
    pub sender_focused_field: usize,
    pub send_files_bubble: RwLock<Option<SendFilesBubble>>,
    pub sender_file_in: String,

    // Receive page fields
    pub receiver_name: String,
    pub receiver_avatar_path: RwLock<Option<String>>,
    pub receiver_focused_field: usize,
    pub receive_files_bubble: RwLock<Option<ReceiveFilesBubble>>,
    pub receive_files: RwLock<Vec<ReceiveFilesFile>>,
    pub received: RwLock<HashMap<String, u64>>,
    pub receiver_out_suffix: RwLock<String>,
    pub receiver_ticket_in: String,
    pub receiver_confirmation_in: String,
    pub receiver_out_dir_in: String,

    // Popups
    pub show_error_modal: RwLock<bool>,
    pub show_file_browser: RwLock<bool>,
    pub show_success_modal: RwLock<bool>,
    pub show_dir_browser: RwLock<bool>,

    // Progress tracking
    pub progress_message: RwLock<String>,
    pub progress_percentage: RwLock<f64>,
    pub operation_start_time: RwLock<Option<Instant>>,

    // Error/Success messages
    pub error_message: RwLock<Option<String>>,
    pub success_message: RwLock<Option<String>>,

    // Browsers
    pub file_browser: RwLock<Option<FileBrowser>>,
    pub directory_browser: RwLock<Option<FileBrowser>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut main_menu_state = ListState::default();
        main_menu_state.select(Some(0));

        let mut config_menu_state = ListState::default();
        config_menu_state.select(Some(0));

        Self {
            id: Uuid::new_v4().to_string(),
            state: RwLock::new(AppState::Idle),

            current_page: RwLock::new(Page::Main),
            previous_pages: RwLock::new(Vec::new()),

            main_menu_state,
            config_menu_state,

            sender_name: "arkdrop-sender".to_string(),
            sender_avatar_path: RwLock::new(None),
            sender_focused_field: 0,
            sender_file_in: String::new(),
            sender_files: RwLock::new(Vec::new()),
            send_files_bubble: RwLock::new(None),

            receiver_name: "arkdrop-receiver".to_string(),
            receiver_avatar_path: RwLock::new(None),
            receiver_focused_field: 0,
            receive_files_bubble: RwLock::new(None),
            receive_files: RwLock::new(Vec::new()),
            received: RwLock::new(HashMap::new()),
            receiver_out_dir_in: get_default_out_dir()
                .to_string_lossy()
                .to_string(),
            receiver_out_suffix: RwLock::new(Uuid::new_v4().to_string()),

            show_error_modal: RwLock::new(false),
            show_file_browser: RwLock::new(false),
            show_success_modal: RwLock::new(false),
            show_dir_browser: RwLock::new(false),

            progress_message: RwLock::new(String::new()),
            progress_percentage: RwLock::new(0.0),
            operation_start_time: RwLock::new(None),

            error_message: RwLock::new(None),
            success_message: RwLock::new(None),

            file_browser: RwLock::new(None),
            directory_browser: RwLock::new(None),

            receiver_ticket_in: String::new(),
            receiver_confirmation_in: String::new(),
        }
    }

    pub fn navigate_to(&self, page: Page) {
        self.previous_pages
            .write()
            .unwrap()
            .push(self.current_page.read().unwrap().clone());
        *self.current_page.write().unwrap() = page;
    }

    pub fn go_back(&self) {
        if let Some(previous) = self.previous_pages.write().unwrap().pop() {
            *self.current_page.write().unwrap() = previous;
        }
    }

    pub async fn update(&self) -> Result<()> {
        match self.state.read().unwrap().clone() {
            AppState::Sending | AppState::Receiving => {
                let pct = self.progress_percentage.read().unwrap().clone();
                if pct >= 100.0 {
                    *self.state.write().unwrap() = AppState::Idle;
                    *self.operation_start_time.write().unwrap() = None;
                    *self.show_success_modal.write().unwrap() = true;
                    *self.success_message.write().unwrap() =
                        Some(match self.current_page.read().unwrap().clone() {
                            Page::SendProgress => {
                                "Files sent successfully!".to_string()
                            }
                            Page::ReceiveProgress => {
                                "Files received successfully!".to_string()
                            }
                            _ => {
                                "Operation completed successfully!".to_string()
                            }
                        });
                    *self.current_page.write().unwrap() = Page::Main;
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn show_error(&self, message: String) {
        *self.error_message.write().unwrap() = Some(message);
        *self.show_error_modal.write().unwrap() = true;
        *self.state.write().unwrap() = AppState::Idle;
        *self.operation_start_time.write().unwrap() = None;
    }

    pub fn show_success(&self, message: String) {
        *self.success_message.write().unwrap() = Some(message);
        *self.show_success_modal.write().unwrap() = true;
        *self.state.write().unwrap() = AppState::Idle;
        *self.operation_start_time.write().unwrap() = None;
    }

    pub async fn start_send_operation(&self) -> Result<()> {
        if self.sender_files.read().unwrap().is_empty() {
            let err_str = "No files selected to send";
            self.show_error(err_str.to_string());
            return Err(anyhow::Error::msg(err_str.to_string()));
        }

        *self.state.write().unwrap() = AppState::Sending;
        *self.operation_start_time.write().unwrap() = Some(Instant::now());

        *self.progress_message.write().unwrap() =
            String::from("Preparing files for sending...");
        *self.progress_percentage.write().unwrap() = 0.0;

        let request = SendFilesRequest {
            profile: self.build_sender_profile()?,
            files: create_sender_files(
                self.sender_files.read().unwrap().clone(),
            )?,
            config: SenderConfig::balanced(),
        };
        *self.send_files_bubble.write().unwrap() =
            Some(send_files(request).await?);

        *self.current_page.write().unwrap() = Page::SendProgress;

        Ok(())
    }

    pub async fn start_receive_operation(&self) -> Result<()> {
        if self.receiver_ticket_in.is_empty()
            || self.receiver_confirmation_in.is_empty()
        {
            let err_str = "Both ticket and confirmation code are required";
            self.show_error(err_str.to_string());
            return Err(anyhow::Error::msg(err_str.to_string()));
        }

        *self.state.write().unwrap() = AppState::Receiving;
        *self.operation_start_time.write().unwrap() = Some(Instant::now());

        *self.progress_message.write().unwrap() =
            "Connecting to sender...".to_string();
        *self.progress_percentage.write().unwrap() = 0.0;

        let request = ReceiveFilesRequest {
            ticket: self.receiver_ticket_in.clone(),
            confirmation: self.receiver_confirmation_in.parse().unwrap(),
            profile: self.build_receiver_profile()?,
            config: None,
        };
        *self.receive_files_bubble.write().unwrap() =
            Some(receive_files(request).await?);

        *self.current_page.write().unwrap() = Page::ReceiveProgress;

        Ok(())
    }

    pub fn build_sender_profile(&self) -> Result<SenderProfile> {
        let mut profile = Profile::new(self.sender_name.clone(), None);

        if let Some(ref avatar_path) =
            self.sender_avatar_path.read().unwrap().clone()
        {
            profile = profile.with_avatar_file(avatar_path)?;
        }

        Ok(SenderProfile {
            name: profile.name,
            avatar_b64: profile.avatar_b64,
        })
    }

    pub fn build_receiver_profile(&self) -> Result<ReceiverProfile> {
        let mut profile = Profile::new(self.receiver_name.clone(), None);

        if let Some(ref avatar_path) =
            self.receiver_avatar_path.read().unwrap().clone()
        {
            profile = profile.with_avatar_file(avatar_path)?;
        }

        Ok(ReceiverProfile {
            name: profile.name,
            avatar_b64: profile.avatar_b64,
        })
    }

    pub fn add_file(&self, file_path: PathBuf) {
        if file_path.exists()
            && !self
                .sender_files
                .read()
                .unwrap()
                .contains(&file_path)
        {
            self.sender_files.write().unwrap().push(file_path);
        }
    }

    pub fn remove_file(&self, index: usize) {
        if index < self.sender_files.read().unwrap().len() {
            self.sender_files.write().unwrap().remove(index);
        }
    }

    pub fn clear_files(&self) {
        self.sender_files.write().unwrap().clear();
    }

    pub fn open_file_browser(&self) {
        *self.show_file_browser.write().unwrap() = true;
        if self.file_browser.read().unwrap().is_none() {
            let start_path =
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            *self.file_browser.write().unwrap() =
                Some(FileBrowser::new(start_path, BrowserMode::SelectFiles));
        }
    }

    pub fn close_file_browser(&self) {
        *self.show_file_browser.write().unwrap() = false;
    }

    pub fn open_directory_browser(&self) {
        *self.show_dir_browser.write().unwrap() = true;
        if self.directory_browser.read().unwrap().is_none() {
            let start_path = get_default_out_dir();
            *self.directory_browser.write().unwrap() = Some(FileBrowser::new(
                start_path,
                BrowserMode::SelectDirectory,
            ));
        }
    }

    pub fn close_directory_browser(&self) {
        *self.show_dir_browser.write().unwrap() = false;
    }
}

impl SendFilesSubscriber for App {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        *self.progress_message.write().unwrap() = message.clone();
    }

    fn notify_sending(&self, event: arkdropx_sender::SendFilesSendingEvent) {
        let pct = event.sent / (event.sent + event.remaining);
        *self.progress_percentage.write().unwrap() = pct as f64;
        *self.progress_message.write().unwrap() = event.name.clone();
    }

    fn notify_connecting(
        &self,
        event: arkdropx_sender::SendFilesConnectingEvent,
    ) {
        *self.progress_message.write().unwrap() = format!(
            "🔗 Connected to receiver: [{}] {}",
            event.receiver.id, event.receiver.name
        );
    }
}

impl ReceiveFilesSubscriber for App {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        *self.progress_message.write().unwrap() = message.clone();
    }

    fn notify_receiving(
        &self,
        event: arkdropx_receiver::ReceiveFilesReceivingEvent,
    ) {
        // Look up file metadata by id
        let files = match self.receive_files.read() {
            Ok(files) => files,
            Err(err) => {
                let err_str = format!("❌ Error accessing files list: {err}");
                self.show_error(err_str.to_string());
                return;
            }
        };
        let file = match files.iter().find(|f| f.id == event.id) {
            Some(file) => file,
            None => {
                let err_str =
                    format!("❌ File not found with ID: {}", event.id);
                self.show_error(err_str.to_string());
                return;
            }
        };

        // Update received byte count
        let mut recvd = self.received.write().unwrap();
        let entry = recvd.entry(event.id.clone()).or_insert(0);
        *entry += event.data.len() as u64;

        *self.progress_message.write().unwrap() = format!("{}...", file.name);
        *self.progress_percentage.write().unwrap() =
            (entry.clone() / file.len) as f64;

        let file_path = get_default_out_dir()
            .join(self.receiver_out_suffix.read().unwrap().clone())
            .join(file.name.clone());

        match fs::File::options()
            .create(true)
            .append(true)
            .open(&file_path)
        {
            Ok(mut file_stream) => {
                if let Err(e) = file_stream.write_all(&event.data) {
                    self.show_error(format!(
                        "❌ Error writing to file {}: {}",
                        file.name, e
                    ));
                }
                if let Err(e) = file_stream.flush() {
                    self.show_error(format!(
                        "❌ Error flushing file {}: {}",
                        file.name, e
                    ));
                }
            }
            Err(e) => {
                self.show_error(format!(
                    "❌ Error opening file {}: {}",
                    file.name, e
                ));
            }
        }
    }

    fn notify_connecting(
        &self,
        event: arkdropx_receiver::ReceiveFilesConnectingEvent,
    ) {
        *self.progress_message.write().unwrap() = format!(
            "🔗 Connected to sender: [{}] {}",
            event.sender.id, event.sender.name
        );
    }
}

pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();
    let res = run_tui_loop(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_tui_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::<B>(f, app))?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_key_event(app, key).await? {
                        break;
                    }
                }
            }
        }

        // Update app state if needed
        app.update().await?;
    }

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5), // Title
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Footer/Help
        ])
        .split(f.area());

    // Title
    let title_text = vec![
        Line::from(vec![
            Span::styled("  🚀 ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("ARK ", Style::default().fg(Color::Cyan).bold()),
            Span::styled("Drop", Style::default().fg(Color::Blue).bold()),
            Span::styled(
                " - File Transfer Tool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Fast • Secure • Peer-to-Peer",
            Style::default().fg(Color::Gray).italic(),
        )]),
    ];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Welcome ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_text)
        .block(title_block)
        .alignment(Alignment::Left);
    f.render_widget(title, main_chunks[0]);

    // Main content based on current page
    let page = app.current_page.read().unwrap().clone();
    match page {
        Page::Main => render_main_page(f, app, main_chunks[1]),
        Page::Send => render_send_page::<B>(f, app, main_chunks[1]),
        Page::Receive => render_receive_page::<B>(f, app, main_chunks[1]),
        Page::Config => render_config_page(f, app, main_chunks[1]),
        Page::Help => render_help_page(f, main_chunks[1]),
        Page::SendProgress => render_send_progress_page(f, app, main_chunks[1]),
        Page::ReceiveProgress => {
            render_receive_progress_page(f, app, main_chunks[1])
        }
    }

    // Footer with navigation help
    let (help_text, status_color) =
        match app.current_page.read().unwrap().clone() {
            Page::Main => (
                "↑/↓ Navigate • Enter Select • CTRL-H Help • CTRL-Q Quit",
                Color::Cyan,
            ),
            Page::Send => (
                "Tab Next Field • Enter Send • Esc Back • CTRL-Q Quit",
                Color::Green,
            ),
            Page::Receive => (
                "Tab Next Field • Enter Receive • Esc Back • CTRL-Q Quit",
                Color::Blue,
            ),
            Page::Config => (
                "↑/↓ Navigate • Enter Select • Esc Back • CTRL-Q Quit",
                Color::Yellow,
            ),
            Page::Help => ("Esc Back • CTRL-Q Quit", Color::Magenta),
            Page::SendProgress => {
                ("Transfer in progress... • CTRL-Q Quit", Color::Green)
            }
            Page::ReceiveProgress => {
                ("Transfer in progress... • CTRL-Q Quit", Color::Blue)
            }
        };

    let footer_content = vec![
        Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(help_text, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(status_color))
        .title(" Controls ")
        .title_style(Style::default().fg(Color::White).bold());

    let footer = Paragraph::new(footer_content)
        .block(footer_block)
        .alignment(Alignment::Center);
    f.render_widget(footer, main_chunks[2]);

    // Render modals/dialogs if any
    if app.show_error_modal.read().unwrap().clone() {
        render_error_modal(f, app);
    }

    if app.show_success_modal.read().unwrap().clone() {
        render_success_modal(f, app);
    }
}

async fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    let show_file_browser = app.show_file_browser.read().unwrap().clone();
    let show_dir_browser = app.show_dir_browser.read().unwrap().clone();
    let show_success_modal = app.show_success_modal.read().unwrap().clone();
    let show_error_modal = app.show_error_modal.read().unwrap().clone();

    if show_file_browser {
        pages::handle_file_browser_input(app, key).await?;
    } else if show_dir_browser {
        pages::handle_dir_browser_input(app, key).await?;
    } else if show_success_modal || show_error_modal {
        match key.code {
            KeyCode::Esc => {
                *app.show_error_modal.write().unwrap() = false;
                *app.show_success_modal.write().unwrap() = false;
            }
            _ => {}
        }
    } else {
        match (key.code, key.modifiers) {
            (
                KeyCode::Char('q') | KeyCode::Char('Q'),
                KeyModifiers::CONTROL,
            ) => {
                return Ok(true);
            }
            (KeyCode::Esc, _) => {
                if app.previous_pages.read().unwrap().len() > 0 {
                    app.go_back();
                }
            }
            _ => {
                let page = app.current_page.read().unwrap().clone();
                match &page {
                    Page::Main => handle_main_page_input(app, key).await?,
                    Page::Send => handle_send_page_input(app, key).await?,
                    Page::Receive => {
                        handle_receive_page_input(app, key).await?
                    }
                    Page::Config => handle_config_page_input(app, key).await?,
                    Page::Help => {}
                    Page::SendProgress => {}
                    Page::ReceiveProgress => {}
                }
            }
        }
    }

    Ok(false)
}

fn render_error_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let error_text = app.error_message.read().unwrap().clone().unwrap();

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⚠️  ", Style::default().fg(Color::Red).bold()),
            Span::styled(
                "Something went wrong:",
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(error_text, Style::default().fg(Color::LightRed)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(Color::Gray)),
            Span::styled("ESC", Style::default().fg(Color::White).bold()),
            Span::styled(" to dismiss", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    let block = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::THICK)
                .border_style(Style::default().fg(Color::Red))
                .title(" ❌ Error ")
                .title_style(Style::default().fg(Color::Red).bold()),
        )
        .alignment(Alignment::Left);

    f.render_widget(block, area);
}

fn render_success_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let success_text = app
        .success_message
        .read()
        .unwrap()
        .clone()
        .unwrap();

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  🎉  ", Style::default().fg(Color::Green).bold()),
            Span::styled("Success!", Style::default().fg(Color::White).bold()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(success_text, Style::default().fg(Color::LightGreen)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(Color::Gray)),
            Span::styled("ESC", Style::default().fg(Color::White).bold()),
            Span::styled(" to continue", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    let block = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::THICK)
                .border_style(Style::default().fg(Color::Green))
                .title(" ✅ Success ")
                .title_style(Style::default().fg(Color::Green).bold()),
        )
        .alignment(Alignment::Left);

    f.render_widget(block, area);
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

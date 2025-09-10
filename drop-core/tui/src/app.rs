use anyhow::{Result, anyhow};
use arkdrop_common::{FileData, Profile, TransferFile, get_default_out_dir};
use arkdropx_receiver::{
    ReceiveFilesBubble, ReceiveFilesFile, ReceiveFilesRequest,
    ReceiveFilesSubscriber, ReceiverProfile, receive_files,
};
use arkdropx_sender::{
    SendFilesBubble, SendFilesRequest, SendFilesSubscriber, SenderConfig,
    SenderFile, SenderProfile, send_files,
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
    collections::HashMap,
    fs,
    io::{self, Write},
    ops::Deref,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::time::Instant;
use uuid::Uuid;

use crate::components::FileBrowser;

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Main,
    Send,
    Receive,
    Config,
    Help,
    SendProgress,
    ReceiveProgress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppState {
    Idle,
    Sending,
    Receiving,
}

pub struct App {
    id: String,
    state: Arc<RwLock<AppState>>,

    current_page: Arc<RwLock<Page>>,
    previous_pages: Arc<RwLock<Vec<Page>>>,

    main_menu_nav: Arc<RwLock<ListState>>,
    config_menu_nav: Arc<RwLock<ListState>>,

    // Sender input
    sender_focused_field: Arc<RwLock<usize>>,
    sender_file_path_in: Arc<RwLock<String>>,
    sender_files_in: Arc<RwLock<Vec<PathBuf>>>,
    sender_name_in: Arc<RwLock<String>>,
    sender_avatar_path_in: Arc<RwLock<String>>,
    send_files_bubble: Arc<RwLock<Option<Arc<SendFilesBubble>>>>,

    // Receiver input
    receiver_focused_field: Arc<RwLock<usize>>,
    receiver_ticket_in: Arc<RwLock<String>>,
    receiver_name_in: Arc<RwLock<String>>,
    receiver_out_dir_in: Arc<RwLock<String>>,
    receiver_avatar_path_in: Arc<RwLock<String>>,
    receiver_confirmation_in: Arc<RwLock<String>>,
    receive_files_bubble: Arc<RwLock<Option<Arc<ReceiveFilesBubble>>>>,

    // Modals
    show_error_modal: Arc<RwLock<bool>>,
    error_message: Arc<RwLock<String>>,
    show_success_modal: Arc<RwLock<bool>>,
    success_message: Arc<RwLock<String>>,

    // Browsers
    dir_browser: Arc<RwLock<Option<FileBrowser>>>,
    show_dir_browser: Arc<RwLock<bool>>,
    file_browser: Arc<RwLock<Option<FileBrowser>>>,
    show_file_browser: Arc<RwLock<bool>>,

    // Transfer
    transfer_id: Arc<RwLock<String>>,
    transfer_message: Arc<RwLock<String>>,
    transfer_files: Arc<RwLock<Vec<TransferFile>>>,
    transfer_start_time: Arc<RwLock<Option<Instant>>>,
}

impl App {
    pub fn new() -> Self {
        let mut main_menu_nav = ListState::default();
        main_menu_nav.select(Some(0));

        let mut config_menu_nav = ListState::default();
        config_menu_nav.select(Some(0));

        Self {
            id: Uuid::new_v4().to_string(),
            state: Arc::new(RwLock::new(AppState::Idle)),

            current_page: Arc::new(RwLock::new(Page::Main)),
            previous_pages: Arc::new(RwLock::new(Vec::new())),

            main_menu_nav: Arc::new(RwLock::new(main_menu_nav)),
            config_menu_nav: Arc::new(RwLock::new(config_menu_nav)),

            // Sender input
            sender_focused_field: Arc::new(RwLock::new(0)),
            sender_name_in: Arc::new(RwLock::new("arkdrop-sender".to_string())),
            sender_avatar_path_in: Arc::new(RwLock::new(String::new())),
            sender_files_in: Arc::new(RwLock::new(Vec::new())),
            sender_file_path_in: Arc::new(RwLock::new(String::new())),
            send_files_bubble: Arc::new(RwLock::new(None)),

            // Receiver input
            receiver_focused_field: Arc::new(RwLock::new(0)),
            receiver_ticket_in: Arc::new(RwLock::new(String::new())),
            receiver_name_in: Arc::new(RwLock::new(
                "arkdrop-receiver".to_string(),
            )),
            receiver_out_dir_in: Arc::new(RwLock::new(String::new())),
            receiver_avatar_path_in: Arc::new(RwLock::new(String::new())),
            receiver_confirmation_in: Arc::new(RwLock::new(String::new())),
            receive_files_bubble: Arc::new(RwLock::new(None)),

            // Modals
            error_message: Arc::new(RwLock::new(String::new())),
            show_error_modal: Arc::new(RwLock::new(false)),
            success_message: Arc::new(RwLock::new(String::new())),
            show_success_modal: Arc::new(RwLock::new(false)),

            // Browsers
            dir_browser: Arc::new(RwLock::new(None)),
            show_dir_browser: Arc::new(RwLock::new(false)),
            file_browser: Arc::new(RwLock::new(None)),
            show_file_browser: Arc::new(RwLock::new(false)),

            // Utilities
            transfer_id: Arc::new(RwLock::new(Uuid::new_v4().to_string())),
            transfer_message: Arc::new(RwLock::new(String::new())),
            transfer_files: Arc::new(RwLock::new(Vec::new())),
            transfer_start_time: Arc::new(RwLock::new(None)),
        }
    }

    pub fn navigate_to(&self, page: Page) {
        let mut current_page = self.current_page.write().unwrap();
        let mut previous_pages = self.previous_pages.write().unwrap();
        previous_pages.push(current_page.clone());
        *current_page = page;
    }

    pub fn go_back(&self) {
        let mut current_page = self.current_page.write().unwrap();
        let mut previous_pages = self.previous_pages.write().unwrap();
        if let Some(previous) = previous_pages.pop() {
            *current_page = previous;
        }
    }

    pub fn update(&self) {
        match self.state.read().unwrap().deref() {
            AppState::Sending | AppState::Receiving => {
                if self.is_transfer_finished() {
                    let mut current_page = self.current_page.write().unwrap();
                    let mut previous_pages =
                        self.previous_pages.write().unwrap();

                    let message = match current_page.deref() {
                        Page::SendProgress => "Files sent successfully!",
                        Page::ReceiveProgress => "Files received successfully!",
                        _ => "Operation completed successfully!",
                    };
                    self.show_success(message.to_string());

                    *current_page = Page::Main;
                    previous_pages.clear();
                }
            }
            AppState::Idle => {
                // TODO
            }
        }
    }

    fn is_transfer_finished(&self) -> bool {
        self.transfer_files
            .read()
            .unwrap()
            .deref()
            .iter()
            .all(|f| f.get_pct() >= 100.0)
    }

    pub fn show_error(&self, message: String) {
        *self.error_message.write().unwrap() = message;
        *self.show_error_modal.write().unwrap() = true;
    }

    pub fn show_success(&self, message: String) {
        *self.success_message.write().unwrap() = message;
        *self.show_success_modal.write().unwrap() = true;
    }

    pub async fn start_send_files(&self) -> Result<()> {
        let sender_files_in = self.sender_files_in.read().unwrap();

        if sender_files_in.is_empty() {
            let err_str = "No files selected to send";
            self.show_error(err_str.to_string());
            return Err(anyhow::Error::msg(err_str.to_string()));
        }

        *self.state.write().unwrap() = AppState::Sending;
        *self.current_page.write().unwrap() = Page::SendProgress;

        self.refresh_sender_transfer_files()?;

        let request = SendFilesRequest {
            profile: self.build_sender_profile()?,
            files: self.create_sender_files(),
            config: SenderConfig::balanced(),
        };
        let bubble = Arc::new(send_files(request).await?);

        self.send_files_bubble
            .write()
            .unwrap()
            .replace(bubble.clone());
        // bubble.subscribe(Arc::new(self.clone()));

        Ok(())
    }

    fn refresh_sender_transfer_files(&self) -> Result<()> {
        let mut transfer_files: Vec<TransferFile> =
            Vec::with_capacity(sender_files.len());

        for path in sender_files.deref() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    anyhow::anyhow!("Invalid file name: {}", path.display())
                })?
                .to_string();

            let len = path
                .metadata()
                .map_err(|err| {
                    anyhow::anyhow!(
                        "Invalid file metadata: {} {err}",
                        path.display()
                    )
                })?
                .len();

            transfer_files.push(TransferFile {
                id: Uuid::new_v4().to_string(),
                name,
                path: path.clone(),
                len: 0,
                expected_len: len,
            });
        }

        *self.transfer_files.write().unwrap() = transfer_files;

        Ok(())
    }

    fn create_sender_files(&self) -> Vec<SenderFile> {
        let transfer_files = self.transfer_files.read().unwrap();
        let mut sender_files = Vec::with_capacity(transfer_files.len());

        for transfer_file in transfer_files.deref() {
            let data = FileData::new(transfer_file.path.clone()).unwrap();
            sender_files.push(SenderFile {
                name: transfer_file.name.clone(),
                data: Arc::new(data),
            });
        }

        sender_files
    }

    pub async fn start_receive_files(&self) -> Result<()> {
        let ticket = self.receiver_ticket_in.read().unwrap();
        let confirmation = self.receiver_confirmation_in.read().unwrap();

        if ticket.is_empty() || confirmation.is_empty() {
            let message = "Both ticket and confirmation code are required";
            self.show_error(message.to_string());
            return Err(anyhow::Error::msg(message.to_string()));
        }

        let confirmation = match confirmation.parse::<u8>() {
            Ok(confirmation) => confirmation,
            Err(err) => {
                let message = format!(
                    "Confirmation code '{confirmation}' is invalid: {err}"
                );
                self.show_error(message.clone());
                return Err(anyhow::Error::msg(message.clone()));
            }
        };

        let request = ReceiveFilesRequest {
            ticket: ticket.clone(),
            confirmation,
            profile: self.build_receiver_profile()?,
            config: None,
        };
        let bubble = Arc::new(receive_files(request).await?);

        *self.current_page.write().unwrap() = Page::ReceiveProgress;
        *self.receive_files_bubble.write().unwrap() = Some(bubble.clone());

        bubble.start()?;
        // bubble.subscribe(Arc::new(self.clone()));

        Ok(())
    }

    pub fn build_sender_profile(&self) -> Result<SenderProfile> {
        let mut profile = Profile::new(self.sender_name_in.clone(), None);

        if !self.sender_avatar_path_in.is_empty() {
            profile = profile.with_avatar_file(&self.sender_avatar_path_in)?;
        }

        Ok(SenderProfile {
            name: profile.name,
            avatar_b64: profile.avatar_b64,
        })
    }

    pub fn build_receiver_profile(&self) -> Result<ReceiverProfile> {
        let mut profile = Profile::new(self.receiver_name_in.clone(), None);

        if !self.receiver_avatar_path_in.is_empty() {
            profile =
                profile.with_avatar_file(&self.receiver_avatar_path_in)?;
        }

        Ok(ReceiverProfile {
            name: profile.name,
            avatar_b64: profile.avatar_b64,
        })
    }

    pub fn add_file(&self, file_path: PathBuf) {
        if file_path.exists() && !self.sender_files_in.contains(&file_path) {
            self.sender_files_in.push(file_path);
        }
    }

    pub fn remove_file(&self, index: usize) {
        if index < self.sender_files_in.len() {
            self.sender_files_in.remove(index);
        }
    }

    pub fn clear_files(&self) {
        self.sender_files_in.clear();
    }

    pub fn open_file_browser(&self) {
        self.show_file_browser = true;
        if self.file_browser.is_none() {
            let start_path =
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            self.file_browser =
                Some(FileBrowser::new(start_path, BrowserMode::SelectFiles));
        }
    }

    pub fn close_file_browser(&self) {
        self.show_file_browser = false;
    }

    pub fn open_directory_browser(&self) {
        self.show_dir_browser = true;
        if self.dir_browser.is_none() {
            let start_path = self.get_out_dir();
            self.dir_browser = Some(FileBrowser::new(
                start_path,
                BrowserMode::SelectDirectory,
            ));
        }
    }

    pub fn close_directory_browser(&self) {
        self.show_dir_browser = false;
    }

    pub fn get_out_dir(&self) -> PathBuf {
        let has_custom_out_dir = !self.receiver_out_dir_in.is_empty();
        if has_custom_out_dir {
            PathBuf::from(self.receiver_out_dir_in.clone())
        } else {
            get_default_out_dir()
        }
    }

    pub fn get_transfer_out_dir(&self) -> PathBuf {
        self.get_out_dir().join(self.transfer_id.clone())
    }

    pub fn get_current_page(&self) -> Page {
        self.current_page.read().unwrap().clone()
    }
}

impl SendFilesSubscriber for App {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        let own = self.get_own();
        own.write()
            .unwrap()
            .transfer_message
            .replace_range(0.., message.as_str());
        self.update();
    }

    fn notify_sending(&self, event: arkdropx_sender::SendFilesSendingEvent) {
        let own = self.get_own();

        for f in own.write().unwrap().transfer_files.iter_mut() {
            if f.id == event.id {
                f.fill += event.sent;
                break;
            }
        }

        own.write()
            .unwrap()
            .transfer_message
            .replace_range(0.., event.name.as_str());

        self.update();
    }

    fn notify_connecting(
        &self,
        event: arkdropx_sender::SendFilesConnectingEvent,
    ) {
        let own = self.get_own();

        own.write()
            .unwrap()
            .transfer_start_time
            .replace(Instant::now());

        own.write()
            .unwrap()
            .transfer_message
            .replace_range(
                0..,
                format!(
                    "🔗 Connected to receiver: [{}] {}",
                    event.receiver.id, event.receiver.name
                )
                .as_str(),
            );

        self.update();
    }
}

impl ReceiveFilesSubscriber for App {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.progress_message = message.clone();
    }

    fn notify_receiving(
        &self,
        event: arkdropx_receiver::ReceiveFilesReceivingEvent,
    ) {
        // Look up file metadata by id
        let files = match self.receiver_progress_files.read() {
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
        let mut recvd = self.received;
        let entry = recvd.entry(event.id.clone()).or_insert(0);
        *entry += event.data.len() as u64;

        self.progress_message = format!("{}...", file.name);
        self.progress_percentage = (entry.clone() / file.len) as f64;

        let file_path = self
            .get_transfer_out_dir()
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
        self.update();
    }

    fn notify_connecting(
        &self,
        event: arkdropx_receiver::ReceiveFilesConnectingEvent,
    ) {
        self.transfer_start_time = Some(Instant::now());
        self.progress_message = format!(
            "🔗 Connected to sender: [{}] {}",
            event.sender.id, event.sender.name
        );
        self.update();
    }
}

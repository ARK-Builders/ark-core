use anyhow::Result;
use arkdrop::Profile;
use ratatui::widgets::ListState;
use std::path::PathBuf;
use tokio::time::Instant;

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
    pub current_page: Page,
    pub state: AppState,
    pub main_menu_state: ListState,
    pub config_menu_state: ListState,
    pub previous_page: Vec<Page>,

    // Send page fields
    pub send_files: Vec<PathBuf>,
    pub send_name: String,
    pub send_avatar_path: Option<String>,
    pub send_focused_field: usize,
    pub send_file_input: String,

    // Receive page fields
    pub receive_ticket: String,
    pub receive_confirmation: String,
    pub receive_output_dir: String,
    pub receive_name: String,
    pub receive_avatar_path: Option<String>,
    pub receive_focused_field: usize,

    // Progress tracking
    pub progress_message: String,
    pub progress_percentage: f64,
    pub operation_start_time: Option<Instant>,

    // Error/Success modals
    pub show_error_modal: bool,
    pub error_message: Option<String>,
    pub show_success_modal: bool,
    pub success_message: Option<String>,

    // Configuration
    pub default_receive_dir: Option<String>,
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
            current_page: Page::Main,
            state: AppState::Idle,
            main_menu_state,
            config_menu_state,
            previous_page: Vec::new(),

            send_files: Vec::new(),
            send_name: "arkdrop-sender".to_string(),
            send_avatar_path: None,
            send_focused_field: 0,
            send_file_input: String::new(),

            receive_ticket: String::new(),
            receive_confirmation: String::new(),
            receive_output_dir: String::new(),
            receive_name: "arkdrop-receiver".to_string(),
            receive_avatar_path: None,
            receive_focused_field: 0,

            progress_message: String::new(),
            progress_percentage: 0.0,
            operation_start_time: None,

            show_error_modal: false,
            error_message: None,
            show_success_modal: false,
            success_message: None,

            default_receive_dir: None,
        }
    }

    pub fn navigate_to(&mut self, page: Page) {
        self.previous_page.push(self.current_page.clone());
        self.current_page = page;
    }

    pub fn go_back(&mut self) {
        if let Some(previous) = self.previous_page.pop() {
            self.current_page = previous;
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        // Update default receive directory
        if self.default_receive_dir.is_none() {
            if let Ok(Some(dir)) = arkdrop::get_default_receive_dir() {
                self.default_receive_dir = Some(dir);
            }
        }

        // Handle progress updates based on state
        match self.state {
            AppState::Sending | AppState::Receiving => {
                // In a real implementation, you'd update progress here
                // For now, we'll just simulate some progress
                if let Some(start_time) = self.operation_start_time {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    self.progress_percentage = (elapsed * 10.0).min(100.0);

                    if self.progress_percentage >= 100.0 {
                        self.state = AppState::Idle;
                        self.operation_start_time = None;
                        self.show_success_modal = true;
                        self.success_message = Some(match self.current_page {
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
                        self.current_page = Page::Main;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn show_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.show_error_modal = true;
        self.state = AppState::Idle;
        self.operation_start_time = None;
    }

    pub fn show_success(&mut self, message: String) {
        self.success_message = Some(message);
        self.show_success_modal = true;
        self.state = AppState::Idle;
        self.operation_start_time = None;
    }

    pub fn start_send_operation(&mut self) {
        if self.send_files.is_empty() {
            self.show_error("No files selected to send".to_string());
            return;
        }

        self.state = AppState::Sending;
        self.operation_start_time = Some(Instant::now());
        self.progress_message = "Preparing files for sending...".to_string();
        self.progress_percentage = 0.0;
        self.current_page = Page::SendProgress;
    }

    pub fn start_receive_operation(&mut self) {
        if self.receive_ticket.is_empty()
            || self.receive_confirmation.is_empty()
        {
            self.show_error(
                "Both ticket and confirmation code are required".to_string(),
            );
            return;
        }

        self.state = AppState::Receiving;
        self.operation_start_time = Some(Instant::now());
        self.progress_message = "Connecting to sender...".to_string();
        self.progress_percentage = 0.0;
        self.current_page = Page::ReceiveProgress;
    }

    pub fn build_send_profile(&self) -> Result<Profile> {
        let mut profile = Profile::new(self.send_name.clone(), None);

        if let Some(ref avatar_path) = self.send_avatar_path {
            profile = profile.with_avatar_file(avatar_path)?;
        }

        Ok(profile)
    }

    pub fn build_receive_profile(&self) -> Result<Profile> {
        let mut profile = Profile::new(self.receive_name.clone(), None);

        if let Some(ref avatar_path) = self.receive_avatar_path {
            profile = profile.with_avatar_file(avatar_path)?;
        }

        Ok(profile)
    }

    pub fn add_file(&mut self, file_path: PathBuf) {
        if file_path.exists() && !self.send_files.contains(&file_path) {
            self.send_files.push(file_path);
        }
    }

    pub fn remove_file(&mut self, index: usize) {
        if index < self.send_files.len() {
            self.send_files.remove(index);
        }
    }

    pub fn clear_files(&mut self) {
        self.send_files.clear();
    }
}

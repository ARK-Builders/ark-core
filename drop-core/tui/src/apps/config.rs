use std::{
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize},
    },
};

use crate::{
    App, AppBackend, AppFileBrowserSaveEvent, AppFileBrowserSubscriber,
    BrowserMode, SortMode,
};
use arkdrop_common::AppConfig;
use base64::{Engine as _, engine::general_purpose};
use image::ImageFormat;
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io::Cursor;

#[derive(Clone, PartialEq)]
enum ConfigField {
    AvatarName,
    AvatarFile,
    OutputDirectory,
}

impl ConfigField {
    fn title(&self) -> &'static str {
        match self {
            ConfigField::AvatarName => "Display Name",
            ConfigField::AvatarFile => "Avatar Image",
            ConfigField::OutputDirectory => "Default Output Directory",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            ConfigField::AvatarName => "👤",
            ConfigField::AvatarFile => "🖼️",
            ConfigField::OutputDirectory => "📁",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            ConfigField::AvatarName => "Your display name for file transfers",
            ConfigField::AvatarFile => "Profile picture file path",
            ConfigField::OutputDirectory => "Default folder for received files",
        }
    }
}

pub struct ConfigApp {
    b: Arc<dyn AppBackend>,

    // UI State
    menu: RwLock<ListState>,
    selected_field: AtomicUsize,

    // Configuration values (matching AppConfig structure)
    avatar_name: RwLock<Option<String>>,
    avatar_file: RwLock<Option<PathBuf>>,
    out_dir: RwLock<Option<PathBuf>>,

    // UI state for avatar preview
    avatar_base64_preview: Arc<RwLock<Option<String>>>,

    // Status and feedback
    status_message: Arc<RwLock<String>>,
    is_processing: Arc<AtomicBool>,

    // File browser integration
    awaiting_browser_result: RwLock<Option<ConfigField>>,
}

impl App for ConfigApp {
    fn draw(&self, f: &mut Frame, area: Rect) {
        let blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(5), // Title and description
                Constraint::Min(12),   // Configuration fields
                Constraint::Length(4), // Status and help
            ])
            .split(area);

        self.draw_header(f, blocks[0]);
        self.draw_config_fields(f, blocks[1]);
        self.draw_footer(f, blocks[2]);
    }

    fn handle_control(&self, ev: &Event) {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            match key.code {
                KeyCode::Up => self.navigate_up(),
                KeyCode::Down => self.navigate_down(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.activate_current_field()
                }
                KeyCode::Esc => {
                    if self.is_processing() {
                        self.set_status_message("Operation cancelled");
                        self.set_processing(false);
                    } else {
                        self.b.get_navigation().go_back();
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if has_ctrl {
                        self.save_configuration();
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if has_ctrl {
                        self.reset_to_defaults();
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if has_ctrl {
                        self.b.get_navigation().go_back();
                    }
                }
                _ => {}
            }
        }
    }
}

impl AppFileBrowserSubscriber for ConfigApp {
    fn on_save(&self, event: AppFileBrowserSaveEvent) {
        let awaiting_field = self
            .awaiting_browser_result
            .write()
            .unwrap()
            .take();

        if let Some(field) = awaiting_field {
            if let Some(selected_path) = event.selected_files.first() {
                match field {
                    ConfigField::AvatarFile => {
                        self.set_avatar_file(selected_path.clone());
                        self.process_avatar_preview(selected_path.clone());
                    }
                    ConfigField::OutputDirectory => {
                        self.set_out_dir(selected_path.clone());
                        self.set_status_message(&format!(
                            "Output directory set to: {}",
                            selected_path.display()
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    fn on_cancel(&self) {
        self.b.get_navigation().go_back();
    }
}

impl ConfigApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let config = b.get_config();

        let mut menu = ListState::default();
        menu.select(Some(0));

        let app = Self {
            b,

            menu: RwLock::new(menu),
            selected_field: AtomicUsize::new(0),

            avatar_name: RwLock::new(config.avatar_name.clone()),
            avatar_file: RwLock::new(config.avatar_file.clone()),
            out_dir: RwLock::new(config.out_dir.clone()),

            avatar_base64_preview: Arc::new(RwLock::new(None)),

            status_message: Arc::new(RwLock::new(
                "Configure your profile and transfer preferences".to_string(),
            )),
            is_processing: Arc::new(AtomicBool::new(false)),

            awaiting_browser_result: RwLock::new(None),
        };

        // Generate preview for existing avatar file
        if let Some(avatar_path) = &config.avatar_file {
            app.process_avatar_preview(avatar_path.clone());
        }

        app
    }

    fn get_config_fields(&self) -> Vec<ConfigField> {
        vec![
            ConfigField::AvatarName,
            ConfigField::AvatarFile,
            ConfigField::OutputDirectory,
        ]
    }

    fn navigate_up(&self) {
        let fields = self.get_config_fields();
        let current = self
            .selected_field
            .load(std::sync::atomic::Ordering::Relaxed);
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
        let fields = self.get_config_fields();
        let current = self
            .selected_field
            .load(std::sync::atomic::Ordering::Relaxed);
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
        let fields = self.get_config_fields();
        let current = self
            .selected_field
            .load(std::sync::atomic::Ordering::Relaxed);

        if let Some(field) = fields.get(current) {
            match field {
                ConfigField::AvatarName => {
                    self.edit_avatar_name();
                }
                ConfigField::AvatarFile => {
                    self.open_avatar_browser();
                }
                ConfigField::OutputDirectory => {
                    self.open_directory_browser();
                }
            }
        }
    }

    fn edit_avatar_name(&self) {
        // For now, we'll use a simple approach - in a real implementation,
        // you might want to implement a text input widget
        self.set_status_message("Name editing not implemented in this demo - use config file directly");
    }

    fn open_avatar_browser(&self) {
        self.set_status_message("Opening file browser for avatar selection...");
        *self.awaiting_browser_result.write().unwrap() =
            Some(ConfigField::AvatarFile);

        let file_browser_manager = self.b.get_file_browser_manager();
        file_browser_manager.open_file_browser(crate::OpenFileBrowserRequest {
            from: crate::Page::Config,
            mode: BrowserMode::SelectFile,
            sort: SortMode::Name,
        });
    }

    fn open_directory_browser(&self) {
        self.set_status_message("Opening directory browser...");
        *self.awaiting_browser_result.write().unwrap() =
            Some(ConfigField::OutputDirectory);

        let file_browser_manager = self.b.get_file_browser_manager();
        file_browser_manager.open_file_browser(crate::OpenFileBrowserRequest {
            from: crate::Page::Config,
            mode: BrowserMode::SelectDirectory,
            sort: SortMode::Name,
        });
    }

    fn process_avatar_preview(&self, path: PathBuf) {
        self.set_processing(true);
        self.set_status_message("Processing avatar preview...");

        // Process image in a separate thread to avoid blocking UI
        let path_clone = path.clone();
        let status_message = self.status_message.clone();
        let avatar_base64_preview = self.avatar_base64_preview.clone();
        let is_processing = self.is_processing.clone();

        std::thread::spawn(move || {
            match Self::convert_image_to_base64_preview(&path_clone) {
                Ok(base64_string) => {
                    *avatar_base64_preview.write().unwrap() =
                        Some(base64_string);
                    *status_message.write().unwrap() = format!(
                        "Avatar file set: {}",
                        path_clone
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    );
                }
                Err(e) => {
                    *status_message.write().unwrap() =
                        format!("Failed to process image: {}", e);
                }
            }
            is_processing.store(false, std::sync::atomic::Ordering::Relaxed);
        });
    }

    fn convert_image_to_base64_preview(
        path: &PathBuf,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Read and decode the image
        let img = image::open(path)?;

        // Resize to low resolution (64x64 pixels) for preview only
        let resized = img.resize(64, 64, image::imageops::FilterType::Lanczos3);

        // Convert to JPEG format for smaller size
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        resized.write_to(&mut cursor, ImageFormat::Jpeg)?;

        // Encode to base64
        let base64_string = general_purpose::STANDARD.encode(&buffer);

        Ok(format!("data:image/jpeg;base64,{}", base64_string))
    }

    fn save_configuration(&self) {
        self.set_processing(true);
        self.set_status_message("Saving configuration...");

        let avatar_name = self.avatar_name.read().unwrap().clone();
        let avatar_file = self.avatar_file.read().unwrap().clone();
        let out_dir = self.out_dir.read().unwrap().clone();

        let config = AppConfig {
            avatar_name,
            avatar_file,
            out_dir,
        };

        match config.save() {
            Ok(_) => {
                self.set_status_message("Configuration saved successfully!");
            }
            Err(e) => {
                self.set_status_message(&format!(
                    "Failed to save configuration: {}",
                    e
                ));
            }
        }

        self.set_processing(false);
    }

    fn reset_to_defaults(&self) {
        *self.avatar_name.write().unwrap() = None;
        *self.avatar_file.write().unwrap() = None;
        *self.out_dir.write().unwrap() = None;
        *self.avatar_base64_preview.write().unwrap() = None;

        self.set_status_message("Configuration reset to defaults");
    }

    fn set_avatar_file(&self, path: PathBuf) {
        *self.avatar_file.write().unwrap() = Some(path);
    }

    fn set_out_dir(&self, path: PathBuf) {
        *self.out_dir.write().unwrap() = Some(path);
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

    fn get_avatar_name(&self) -> Option<String> {
        self.avatar_name.read().unwrap().clone()
    }

    fn get_avatar_file(&self) -> Option<PathBuf> {
        self.avatar_file.read().unwrap().clone()
    }

    fn get_out_dir(&self) -> Option<PathBuf> {
        self.out_dir.read().unwrap().clone()
    }

    fn get_avatar_base64_preview(&self) -> Option<String> {
        self.avatar_base64_preview.read().unwrap().clone()
    }

    fn get_status_message(&self) -> String {
        self.status_message.read().unwrap().clone()
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let has_name = self.get_avatar_name().is_some();
        let has_avatar = self.get_avatar_file().is_some();
        let has_out_dir = self.get_out_dir().is_some();

        let completion_count = [has_name, has_avatar, has_out_dir]
            .iter()
            .filter(|&&x| x)
            .count();

        let header_content = vec![
            Line::from(vec![
                Span::styled("⚙️ ", Style::default().fg(Color::Blue).bold()),
                Span::styled(
                    "Configuration",
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(
                    format!(" • {}/3 configured", completion_count),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Configure your profile and transfer preferences",
                Style::default().fg(Color::Gray).italic(),
            )]),
        ];

        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Settings ")
            .title_style(Style::default().fg(Color::White).bold());

        let header = Paragraph::new(header_content)
            .block(header_block)
            .alignment(Alignment::Left);

        f.render_widget(header, area);
    }

    fn draw_config_fields(&self, f: &mut Frame, area: Rect) {
        let fields = self.get_config_fields();
        let current_selection = self
            .selected_field
            .load(std::sync::atomic::Ordering::Relaxed);

        let field_items: Vec<ListItem> = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let is_selected = index == current_selection;
                let value_text = self.get_field_value_display(field);
                let is_configured = self.is_field_configured(field);

                let status_icon = if is_configured {
                    "✅"
                } else {
                    "⚪"
                };
                let value_color = if is_configured {
                    Color::Green
                } else {
                    Color::Gray
                };

                let title_line = Line::from(vec![
                    Span::styled(
                        format!("{} ", status_icon),
                        Style::default().fg(if is_configured {
                            Color::Green
                        } else {
                            Color::Gray
                        }),
                    ),
                    Span::styled(
                        format!("{} ", field.icon()),
                        Style::default().fg(Color::Blue),
                    ),
                    Span::styled(
                        field.title(),
                        Style::default()
                            .fg(if is_selected {
                                Color::White
                            } else {
                                Color::LightBlue
                            })
                            .bold(),
                    ),
                ]);

                let value_line = Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(value_text, Style::default().fg(value_color)),
                ]);

                let description_line = Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        field.description(),
                        Style::default().fg(Color::DarkGray).italic(),
                    ),
                ]);

                ListItem::new(vec![title_line, value_line, description_line])
                    .style(if is_selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    })
            })
            .collect();

        let fields_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::White))
            .title(" Profile Settings ")
            .title_style(Style::default().fg(Color::White).bold());

        let fields_list = List::new(field_items)
            .block(fields_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::Black)
                    .bold(),
            )
            .highlight_symbol("▶ ");

        f.render_stateful_widget(
            fields_list,
            area,
            &mut self.menu.write().unwrap(),
        );
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let status_message = self.get_status_message();
        let is_processing = self.is_processing();

        let (status_icon, status_color) = if is_processing {
            ("⏳", Color::Yellow)
        } else {
            ("ℹ️", Color::Blue)
        };

        let footer_content = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(status_message, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
                Span::styled(": Edit • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "CTRL-S",
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::styled(": Save • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "CTRL-R",
                    Style::default().fg(Color::Yellow).bold(),
                ),
                Span::styled(": Reset • ", Style::default().fg(Color::Gray)),
                Span::styled("ESC", Style::default().fg(Color::Red).bold()),
                Span::styled(": Back", Style::default().fg(Color::Gray)),
            ]),
        ];

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Status & Controls ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer, area);
    }

    fn get_field_value_display(&self, field: &ConfigField) -> String {
        match field {
            ConfigField::AvatarName => {
                if let Some(name) = self.get_avatar_name() {
                    name
                } else {
                    "Not set".to_string()
                }
            }
            ConfigField::AvatarFile => {
                if let Some(path) = self.get_avatar_file() {
                    let has_preview =
                        self.get_avatar_base64_preview().is_some();
                    format!(
                        "{} {}",
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        if has_preview {
                            "(preview ready)"
                        } else {
                            "(processing...)"
                        }
                    )
                } else {
                    "No avatar file selected".to_string()
                }
            }
            ConfigField::OutputDirectory => {
                if let Some(dir) = self.get_out_dir() {
                    dir.to_string_lossy().to_string()
                } else {
                    "Use system default".to_string()
                }
            }
        }
    }

    fn is_field_configured(&self, field: &ConfigField) -> bool {
        match field {
            ConfigField::AvatarName => self.get_avatar_name().is_some(),
            ConfigField::AvatarFile => self.get_avatar_file().is_some(),
            ConfigField::OutputDirectory => self.get_out_dir().is_some(),
        }
    }
}

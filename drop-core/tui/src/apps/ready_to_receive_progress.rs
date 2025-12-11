use std::{
    collections::HashMap,
    fs,
    io::Write,
    sync::{Arc, RwLock, atomic::AtomicU32},
    time::Instant,
};

use crate::{
    App, AppBackend, ControlCapture, utilities::qr_renderer::QrCodeRenderer,
};
use arkdropx_receiver::ready_to_receive::{
    ReadyToReceiveConnectingEvent, ReadyToReceiveReceivingEvent,
    ReadyToReceiveSubscriber,
};
use crossterm::event::KeyModifiers;
use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
};
use uuid::Uuid;

#[derive(Clone)]
struct ProgressFile {
    id: String,
    name: String,
    len: u64,
    received: u64,
    last_update: Instant,
    bytes_per_second: f64,
    status: FileTransferStatus,
}

#[derive(Clone, PartialEq)]
enum FileTransferStatus {
    Waiting,
    Receiving,
    Completed,
    Error(String),
}

impl FileTransferStatus {
    fn icon(&self) -> &'static str {
        match self {
            FileTransferStatus::Waiting => "⏳",
            FileTransferStatus::Receiving => "📥",
            FileTransferStatus::Completed => "✅",
            FileTransferStatus::Error(_) => "❌",
        }
    }

    fn color(&self) -> Color {
        match self {
            FileTransferStatus::Waiting => Color::Gray,
            FileTransferStatus::Receiving => Color::Cyan,
            FileTransferStatus::Completed => Color::Green,
            FileTransferStatus::Error(_) => Color::Red,
        }
    }
}

pub struct ReadyToReceiveProgressApp {
    id: String,
    b: Arc<dyn AppBackend>,

    progress_pct: AtomicU32,
    operation_start_time: RwLock<Option<Instant>>,

    title_text: RwLock<String>,
    block_title_text: RwLock<String>,
    status_text: RwLock<String>,
    log_text: RwLock<String>,
    error_message: RwLock<Option<String>>,

    files: RwLock<HashMap<String, ProgressFile>>,
    total_transfer_speed: RwLock<f64>,
    sender_name: RwLock<String>,
    total_chunks_received: RwLock<u64>,
}

impl App for ReadyToReceiveProgressApp {
    fn draw(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        if self.has_transfer_started() {
            self.draw_receiving_mode(f, area);
        } else {
            self.draw_waiting_mode(f, area);
        }
    }

    fn handle_control(
        &self,
        ev: &ratatui::crossterm::event::Event,
    ) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            if has_ctrl {
                match key.code {
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.b.get_ready_to_receive_manager().cancel();
                        self.b.get_navigation().go_back();
                        self.reset();
                    }
                    _ => return None,
                }
            } else {
                match key.code {
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
}

impl ReadyToReceiveSubscriber for ReadyToReceiveProgressApp {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.set_log_text(message.as_str());
    }

    fn notify_receiving(&self, event: ReadyToReceiveReceivingEvent) {
        self.increment_chunk_count();
        self.update_file(&event);
        self.refresh_total_transfer_speed();
        self.refresh_overall_progress();
        self.write_file_to_fs(&event);
    }

    fn notify_connecting(&self, event: ReadyToReceiveConnectingEvent) {
        self.set_connecting_files(&event);
        self.set_now_as_operation_start_time();
        self.set_title_text("📥 Receiving Files");
        self.set_block_title_text(
            format!("Connected to {}", event.sender.name).as_str(),
        );
        self.set_status_text(
            format!("Receiving Files from {}", event.sender.name).as_str(),
        );
        *self.sender_name.write().unwrap() = event.sender.name.clone();
    }
}

impl ReadyToReceiveProgressApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            b,

            progress_pct: AtomicU32::new(0),
            operation_start_time: RwLock::new(None),

            title_text: RwLock::new("📥 Waiting for Sender".to_string()),
            block_title_text: RwLock::new("Ready to Receive".to_string()),
            status_text: RwLock::new("Waiting for connection".to_string()),
            log_text: RwLock::new("Initializing...".to_string()),
            error_message: RwLock::new(None),

            files: RwLock::new(HashMap::new()),
            total_transfer_speed: RwLock::new(0.0),
            sender_name: RwLock::new(String::new()),
            total_chunks_received: RwLock::new(0),
        }
    }

    fn reset(&self) {
        self.progress_pct
            .store(0, std::sync::atomic::Ordering::Relaxed);
        *self.operation_start_time.write().unwrap() = None;
        *self.title_text.write().unwrap() = "📥 Waiting for Sender".to_string();
        *self.block_title_text.write().unwrap() =
            "Ready to Receive".to_string();
        *self.status_text.write().unwrap() =
            "Waiting for connection".to_string();
        *self.log_text.write().unwrap() = "Initializing...".to_string();
        *self.error_message.write().unwrap() = None;
        self.files.write().unwrap().clear();
        *self.total_transfer_speed.write().unwrap() = 0.0;
        *self.sender_name.write().unwrap() = String::new();
        *self.total_chunks_received.write().unwrap() = 0;
    }

    fn has_transfer_started(&self) -> bool {
        self.get_operation_start_time().is_some()
    }

    fn set_title_text(&self, text: &str) {
        *self.title_text.write().unwrap() = text.to_string();
    }

    fn set_block_title_text(&self, text: &str) {
        *self.block_title_text.write().unwrap() = text.to_string();
    }

    fn set_status_text(&self, text: &str) {
        *self.status_text.write().unwrap() = text.to_string();
    }

    fn set_log_text(&self, text: &str) {
        *self.log_text.write().unwrap() = text.to_string();
    }

    fn set_now_as_operation_start_time(&self) {
        self.operation_start_time
            .write()
            .unwrap()
            .replace(Instant::now());
    }

    fn get_operation_start_time(&self) -> Option<Instant> {
        *self.operation_start_time.read().unwrap()
    }

    fn get_title_text(&self) -> String {
        self.title_text.read().unwrap().clone()
    }

    fn get_block_title_text(&self) -> String {
        self.block_title_text.read().unwrap().clone()
    }

    fn get_progress_pct(&self) -> f64 {
        let v = self
            .progress_pct
            .load(std::sync::atomic::Ordering::Relaxed);
        f64::from(v)
    }

    fn get_files(&self) -> Vec<ProgressFile> {
        self.files
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn get_total_transfer_speed(&self) -> f64 {
        *self.total_transfer_speed.read().unwrap()
    }

    fn increment_chunk_count(&self) {
        let mut count = self.total_chunks_received.write().unwrap();
        *count += 1;
    }

    fn update_file(&self, event: &ReadyToReceiveReceivingEvent) {
        let now = Instant::now();
        let mut files = self.files.write().unwrap();

        if let Some(file) = files.get_mut(&event.id) {
            let time_diff = now.duration_since(file.last_update).as_secs_f64();
            let bytes_received = event.data.len() as u64;

            if time_diff > 0.0 {
                file.bytes_per_second = bytes_received as f64 / time_diff;
            }

            file.received += bytes_received;
            file.last_update = now;

            if file.received >= file.len {
                file.status = FileTransferStatus::Completed;
            } else {
                file.status = FileTransferStatus::Receiving;
            }
        }
    }

    fn set_connecting_files(&self, event: &ReadyToReceiveConnectingEvent) {
        let mut files = self.files.write().unwrap();
        files.clear();

        for file in &event.files {
            files.insert(
                file.id.clone(),
                ProgressFile {
                    id: file.id.clone(),
                    name: file.name.clone(),
                    len: file.len,
                    received: 0,
                    last_update: Instant::now(),
                    bytes_per_second: 0.0,
                    status: FileTransferStatus::Waiting,
                },
            );
        }
    }

    fn refresh_total_transfer_speed(&self) {
        let files = self.files.read().unwrap();
        let total_speed: f64 = files
            .values()
            .filter(|f| f.status == FileTransferStatus::Receiving)
            .map(|f| f.bytes_per_second)
            .sum();
        *self.total_transfer_speed.write().unwrap() = total_speed;
    }

    fn refresh_overall_progress(&self) {
        let files = self.files.read().unwrap();
        let total_size: u64 = files.values().map(|f| f.len).sum();
        let total_received: u64 = files.values().map(|f| f.received).sum();

        let progress_pct = if total_size > 0 {
            ((total_received as f64 / total_size as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        self.progress_pct
            .store(progress_pct as u32, std::sync::atomic::Ordering::Relaxed);
    }

    fn write_file_to_fs(&self, event: &ReadyToReceiveReceivingEvent) {
        let out_dir = self.b.get_config().get_out_dir();
        let file_name = {
            let files = self.files.read().unwrap();
            files.get(&event.id).map(|f| f.name.clone())
        };

        if let Some(name) = file_name {
            let file_path = out_dir.join(&name);

            // Create parent directories if needed
            if let Some(parent) = file_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    self.set_file_error(
                        &event.id,
                        format!("Failed to create directory: {}", e),
                    );
                    return;
                }
            }

            let mut options = fs::OpenOptions::new();
            options.create(true).append(true);

            match options.open(&file_path) {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(&event.data) {
                        self.set_file_error(
                            &event.id,
                            format!("Failed to write data: {}", e),
                        );
                    }
                }
                Err(e) => {
                    self.set_file_error(
                        &event.id,
                        format!("Failed to open file: {}", e),
                    );
                }
            }
        }
    }

    fn set_file_error(&self, file_id: &str, error: String) {
        // Update the file's status to Error
        let mut files = self.files.write().unwrap();
        if let Some(file) = files.get_mut(file_id) {
            file.status = FileTransferStatus::Error(error.clone());
        }
        // Also set global error message for UI display
        *self.error_message.write().unwrap() = Some(error);
    }

    fn get_error_message(&self) -> Option<String> {
        self.error_message.read().unwrap().clone()
    }

    fn format_bytes(&self, bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    fn format_speed(&self, bytes_per_sec: f64) -> String {
        if bytes_per_sec == 0.0 {
            return "--".to_string();
        }
        format!("{}/s", self.format_bytes(bytes_per_sec as u64))
    }

    // ── Waiting Mode (QR Code Display) ───────────────────────────────────────

    fn draw_waiting_mode(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(15),   // QR Code
                Constraint::Length(5), // Connection info
                Constraint::Length(4), // Footer
            ])
            .split(area);

        self.draw_waiting_title(f, blocks[0]);
        self.draw_qr_code(f, blocks[1]);
        self.draw_connection_info(f, blocks[2]);
        self.draw_waiting_footer(f, blocks[3]);
    }

    fn draw_waiting_title(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let title_content = vec![Line::from(vec![
            Span::styled("📥 ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                "Ready to Receive",
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                " • Waiting for sender to connect",
                Style::default().fg(Color::Gray),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Waiting ")
            .title_style(Style::default().fg(Color::White).bold());

        let title_widget = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title_widget, area);
    }

    fn draw_qr_code(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let qr_block =
            QrCodeRenderer::create_qr_block("Scan to Send", Color::Cyan);

        if let Some(bubble) = self
            .b
            .get_ready_to_receive_manager()
            .get_ready_to_receive_bubble()
        {
            let qr_data = format!(
                "drop://send?ticket={}&confirmation={}",
                bubble.get_ticket(),
                bubble.get_confirmation()
            );

            QrCodeRenderer::render_qr_code(f, area, qr_block, &qr_data);
        } else {
            QrCodeRenderer::render_waiting(
                f,
                area,
                qr_block,
                "Preparing to receive...",
            );
        }
    }

    fn draw_connection_info(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
    ) {
        let info_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Connection Details ")
            .title_style(Style::default().fg(Color::White).bold());

        let info_content = if let Some(bubble) = self
            .b
            .get_ready_to_receive_manager()
            .get_ready_to_receive_bubble()
        {
            vec![
                Line::from(vec![
                    Span::styled("🔑 ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "Confirmation: ",
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!("{:02}", bubble.get_confirmation()),
                        Style::default().fg(Color::White).bold(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("🎫 ", Style::default().fg(Color::Blue)),
                    Span::styled("Ticket: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        truncate_string(&bubble.get_ticket(), 40),
                        Style::default().fg(Color::White),
                    ),
                ]),
            ]
        } else {
            vec![Line::from(vec![Span::styled(
                "Generating connection details...",
                Style::default().fg(Color::Yellow),
            )])]
        };

        let info_widget = Paragraph::new(info_content)
            .block(info_block)
            .alignment(Alignment::Left);

        f.render_widget(info_widget, area);
    }

    fn draw_waiting_footer(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let footer_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Ctrl+C to cancel • ESC to go back",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer_widget = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer_widget, area);
    }

    // ── Receiving Mode ───────────────────────────────────────────────────────

    fn draw_receiving_mode(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(6), // Overall progress
                Constraint::Min(8),    // Individual files list
                Constraint::Length(4), // Footer
            ])
            .split(area);

        self.draw_receiving_title(f, blocks[0]);
        self.draw_overall_progress(f, blocks[1]);
        self.draw_files_list(f, blocks[2]);
        self.draw_receiving_footer(f, blocks[3]);
    }

    fn draw_receiving_title(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
    ) {
        let progress_pct = self.get_progress_pct();
        let files = self.get_files();
        let completed_files = files
            .iter()
            .filter(|f| f.status == FileTransferStatus::Completed)
            .count();
        let total_files = files.len();

        let progress_icon = if progress_pct >= 100.0 {
            "✅"
        } else {
            match (progress_pct as u8) % 4 {
                0 => "◐",
                1 => "◓",
                2 => "◑",
                _ => "◒",
            }
        };

        let title_content = vec![Line::from(vec![
            Span::styled(
                format!("{} ", progress_icon),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                self.get_title_text(),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                format!(
                    " • {}/{} files • {:.1}%",
                    completed_files, total_files, progress_pct
                ),
                Style::default().fg(Color::Cyan),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(self.get_block_title_text())
            .title_style(Style::default().fg(Color::White).bold());

        let title_widget = Paragraph::new(title_content)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(title_widget, area);
    }

    fn draw_overall_progress(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
    ) {
        let progress_pct = self.get_progress_pct();
        let files = self.get_files();
        let total_size: u64 = files.iter().map(|f| f.len).sum();
        let total_received: u64 = files.iter().map(|f| f.received).sum();
        let transfer_speed = self.get_total_transfer_speed();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ])
            .split(area);

        let progress_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Overall Progress ")
            .title_style(Style::default().fg(Color::White).bold());

        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(
                Style::default()
                    .fg(if progress_pct >= 100.0 {
                        Color::Green
                    } else {
                        Color::Cyan
                    })
                    .bg(Color::DarkGray),
            )
            .percent(progress_pct as u16)
            .label(format!("{:.1}%", progress_pct));

        f.render_widget(progress, chunks[0]);

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Stats ")
            .title_style(Style::default().fg(Color::White).bold());

        let stats_content = vec![
            Line::from(vec![
                Span::styled("📊 ", Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!(
                        "{} / {}",
                        self.format_bytes(total_received),
                        self.format_bytes(total_size)
                    ),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    self.format_speed(transfer_speed),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let stats_widget = Paragraph::new(stats_content)
            .block(stats_block)
            .alignment(Alignment::Center);

        f.render_widget(stats_widget, chunks[1]);
    }

    fn draw_files_list(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let files = self.get_files();

        let file_items: Vec<ListItem> = files
            .iter()
            .map(|file| {
                let progress_pct = if file.len > 0 {
                    (file.received as f64 / file.len as f64) * 100.0
                } else {
                    0.0
                };

                let name_color = match &file.status {
                    FileTransferStatus::Completed => Color::Green,
                    FileTransferStatus::Error(_) => Color::Red,
                    _ => Color::White,
                };

                let status_line = Line::from(vec![
                    Span::styled(
                        format!("{} ", file.status.icon()),
                        Style::default().fg(file.status.color()),
                    ),
                    Span::styled(
                        file.name.clone(),
                        Style::default().fg(name_color),
                    ),
                    Span::styled(
                        format!("{:>6.1}%", progress_pct),
                        Style::default().fg(Color::Cyan),
                    ),
                ]);

                let detail_text = match &file.status {
                    FileTransferStatus::Receiving => format!(
                        "{} / {} • {}",
                        self.format_bytes(file.received),
                        self.format_bytes(file.len),
                        self.format_speed(file.bytes_per_second)
                    ),
                    FileTransferStatus::Completed => format!(
                        "{} / {} • Complete",
                        self.format_bytes(file.received),
                        self.format_bytes(file.len)
                    ),
                    FileTransferStatus::Error(err) => {
                        format!("Error: {}", truncate_string(err, 40))
                    }
                    FileTransferStatus::Waiting => format!(
                        "{} / {} • --",
                        self.format_bytes(file.received),
                        self.format_bytes(file.len)
                    ),
                };

                let detail_color = match &file.status {
                    FileTransferStatus::Error(_) => Color::Red,
                    _ => Color::Gray,
                };

                let detail_line = Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        detail_text,
                        Style::default().fg(detail_color),
                    ),
                ]);

                ListItem::new(vec![status_line, detail_line])
            })
            .collect();

        let files_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::White))
            .title(format!(" Files ({}) ", files.len()))
            .title_style(Style::default().fg(Color::White).bold());

        let files_list = List::new(file_items)
            .block(files_block)
            .style(Style::default().fg(Color::White));

        f.render_widget(files_list, area);
    }

    fn draw_receiving_footer(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
    ) {
        let progress_pct = self.get_progress_pct();
        let error_message = self.get_error_message();

        let (footer_text, footer_color, footer_icon) =
            if let Some(err) = error_message {
                (
                    format!(
                        "Error: {} • Press ESC to go back",
                        truncate_string(&err, 50)
                    ),
                    Color::Red,
                    "❌",
                )
            } else if progress_pct >= 100.0 {
                (
                    "All files received successfully! Press ESC to continue"
                        .to_string(),
                    Color::Green,
                    "✅",
                )
            } else {
                (
                    "Ctrl+C to cancel • ESC to go back".to_string(),
                    Color::Cyan,
                    "📥",
                )
            };

        let footer_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("{} ", footer_icon),
                    Style::default().fg(footer_color),
                ),
                Span::styled(footer_text, Style::default().fg(footer_color)),
            ]),
        ];

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(footer_color))
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer_widget = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer_widget, area);
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

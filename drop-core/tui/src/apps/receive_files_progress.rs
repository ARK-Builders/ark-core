use std::{
    collections::HashMap,
    sync::{Arc, RwLock, atomic::AtomicU32},
    time::Instant,
};

use crate::{App, AppBackend, ControlCapture};
use arkdropx_receiver::ReceiveFilesSubscriber;
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
    name: String,
    total_size: u64,
    received: u64,
    status: FileTransferStatus,
    transfer_speed: f64, // bytes per second
    last_update: Instant,
    last_chunk_size: u64, // Size of last received chunk for speed calculation
}

#[derive(Clone, PartialEq)]
enum FileTransferStatus {
    Receiving,
    Completed,
}

impl FileTransferStatus {
    fn icon(&self) -> &'static str {
        match self {
            FileTransferStatus::Receiving => "📥",
            FileTransferStatus::Completed => "✅",
        }
    }

    fn color(&self) -> Color {
        match self {
            FileTransferStatus::Receiving => Color::Blue,
            FileTransferStatus::Completed => Color::Green,
        }
    }
}

pub struct ReceiveFilesProgressApp {
    id: String,
    b: Arc<dyn AppBackend>,

    progress_pct: AtomicU32,
    operation_start_time: RwLock<Option<Instant>>,

    title_text: RwLock<String>,
    block_title_text: RwLock<String>,
    status_text: RwLock<String>,
    log_text: RwLock<String>,

    files: RwLock<HashMap<String, ProgressFile>>,
    file_metadata: RwLock<HashMap<String, FileMetadata>>, /* Store file metadata separately */
    total_transfer_speed: RwLock<f64>,
    sender_name: RwLock<String>,
    total_chunks_received: RwLock<u64>,
}

#[derive(Clone)]
struct FileMetadata {
    name: String,
    total_size: u64,
}

impl App for ReceiveFilesProgressApp {
    fn draw(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
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

        self.draw_title(f, blocks[0]);
        self.draw_overall_progress(f, blocks[1]);
        self.draw_files_list(f, blocks[2]);
        self.draw_footer(f, blocks[3]);
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
                        self.b.get_receive_files_manager().cancel();
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

impl ReceiveFilesSubscriber for ReceiveFilesProgressApp {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.set_log_text(message.as_str());
    }

    fn notify_receiving(
        &self,
        event: arkdropx_receiver::ReceiveFilesReceivingEvent,
    ) {
        let id = event.id;
        let chunk_data = event.data;
        let chunk_size = chunk_data.len() as u64;
        let now = Instant::now();

        // Increment total chunks received counter
        *self.total_chunks_received.write().unwrap() += 1;

        // Update or create file progress
        let mut files = self.files.write().unwrap();
        let file_metadata = self.file_metadata.read().unwrap();

        // Get file metadata if available
        let (file_name, total_size) =
            if let Some(metadata) = file_metadata.get(&id) {
                (metadata.name.clone(), metadata.total_size)
            } else {
                // If no metadata available, use ID as name and estimate size
                (format!("File_{}", &id[..8]), 0)
            };

        if let Some(file) = files.get_mut(&id) {
            // Update existing file progress
            let time_diff = now.duration_since(file.last_update).as_secs_f64();

            // Calculate transfer speed based on chunk size and time difference
            if time_diff > 0.0 {
                // Use exponential moving average for smoother speed calculation
                let instant_speed = chunk_size as f64 / time_diff;
                file.transfer_speed = if file.transfer_speed == 0.0 {
                    instant_speed
                } else {
                    // 70% old speed + 30% new speed for smoothing
                    file.transfer_speed * 0.7 + instant_speed * 0.3
                };
            }

            file.received += chunk_size;
            file.last_update = now;
            file.last_chunk_size = chunk_size;

            // Update status based on progress
            if total_size > 0 && file.received >= total_size {
                file.status = FileTransferStatus::Completed;
            } else {
                file.status = FileTransferStatus::Receiving;
            }
        } else {
            // Create new file entry
            files.insert(
                id.clone(),
                ProgressFile {
                    name: file_name,
                    total_size,
                    received: chunk_size,
                    status: if total_size > 0 && chunk_size >= total_size {
                        FileTransferStatus::Completed
                    } else {
                        FileTransferStatus::Receiving
                    },
                    transfer_speed: 0.0, // Will be calculated on next chunk
                    last_update: now,
                    last_chunk_size: chunk_size,
                },
            );
        }

        // Calculate total transfer speed from all active files
        let total_speed: f64 = files
            .values()
            .filter(|f| f.status == FileTransferStatus::Receiving)
            .map(|f| f.transfer_speed)
            .sum();
        *self.total_transfer_speed.write().unwrap() = total_speed;

        // Calculate overall progress
        let total_files_size: u64 = files.values().map(|f| f.total_size).sum();
        let total_received_size: u64 = files.values().map(|f| f.received).sum();

        let progress_pct = if total_files_size > 0 {
            ((total_received_size as f64 / total_files_size as f64) * 100.0)
                .min(100.0)
        } else {
            // If no total size info, show progress based on active transfers
            let completed_files = files
                .values()
                .filter(|f| f.status == FileTransferStatus::Completed)
                .count();
            let total_files = files.len();

            if total_files > 0 {
                (completed_files as f64 / total_files as f64) * 100.0
            } else {
                0.0
            }
        };

        self.progress_pct
            .store(progress_pct as u32, std::sync::atomic::Ordering::Relaxed);
    }

    fn notify_connecting(
        &self,
        event: arkdropx_receiver::ReceiveFilesConnectingEvent,
    ) {
        let sender = event.sender;
        let name = sender.name;

        self.set_now_as_operation_start_time();
        self.set_title_text("📥 Receiving Files");
        self.set_block_title_text(format!("Connected to {}", name).as_str());
        self.set_status_text(format!("Receiving Files from {}", name).as_str());
        self.set_sender_name(name.as_str());
    }
}

impl ReceiveFilesProgressApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            b,

            progress_pct: AtomicU32::new(0),
            operation_start_time: RwLock::new(None),

            title_text: RwLock::new("📥 Receiving Files".to_string()),
            block_title_text: RwLock::new("Waiting for Connection".to_string()),
            status_text: RwLock::new("Waiting for Sender".to_string()),
            log_text: RwLock::new("Initializing transfer...".to_string()),

            files: RwLock::new(HashMap::new()),
            file_metadata: RwLock::new(HashMap::new()),
            total_transfer_speed: RwLock::new(0.0),
            sender_name: RwLock::new("Unknown".to_string()),
            total_chunks_received: RwLock::new(0),
        }
    }

    fn set_title_text(&self, text: &str) {
        *self.title_text.write().unwrap() = text.to_string()
    }

    fn set_block_title_text(&self, text: &str) {
        *self.block_title_text.write().unwrap() = text.to_string()
    }

    fn set_status_text(&self, text: &str) {
        *self.status_text.write().unwrap() = text.to_string()
    }

    fn set_log_text(&self, text: &str) {
        *self.log_text.write().unwrap() = text.to_string()
    }

    fn set_sender_name(&self, name: &str) {
        *self.sender_name.write().unwrap() = name.to_string()
    }

    fn set_now_as_operation_start_time(&self) {
        self.operation_start_time
            .write()
            .unwrap()
            .replace(Instant::now());
    }

    fn get_title_text(&self) -> String {
        self.title_text.read().unwrap().clone()
    }

    fn get_block_title_text(&self) -> String {
        self.block_title_text.read().unwrap().clone()
    }

    fn get_sender_name(&self) -> String {
        self.sender_name.read().unwrap().clone()
    }

    fn get_progress_pct(&self) -> f64 {
        let v = self
            .progress_pct
            .load(std::sync::atomic::Ordering::Relaxed);
        return f64::from(v);
    }

    fn get_operation_start_time(&self) -> Option<Instant> {
        self.operation_start_time.read().unwrap().clone()
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

    fn get_total_chunks_received(&self) -> u64 {
        *self.total_chunks_received.read().unwrap()
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

    fn draw_title(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();
        let files = self.get_files();
        let completed_files = files
            .iter()
            .filter(|f| f.status == FileTransferStatus::Completed)
            .count();
        let total_files = files.len();
        let total_chunks = self.get_total_chunks_received();

        let progress_icon = if progress_pct >= 100.0 {
            "✅"
        } else {
            match total_chunks % 4 {
                0 => "◐",
                1 => "◓",
                2 => "◑",
                _ => "◒",
            }
        };

        let title_content = vec![Line::from(vec![
            Span::styled(
                format!("{} ", progress_icon),
                Style::default().fg(Color::Blue).bold(),
            ),
            Span::styled(
                self.get_title_text(),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                format!(
                    " • {}/{} files • {:.1}% • {} chunks",
                    completed_files, total_files, progress_pct, total_chunks
                ),
                Style::default().fg(Color::Cyan),
            ),
        ])];

        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
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
        let total_size: u64 = files.iter().map(|f| f.total_size).sum();
        let total_received: u64 = files.iter().map(|f| f.received).sum();
        let transfer_speed = self.get_total_transfer_speed();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70), // Progress bar
                Constraint::Percentage(30), // Stats
            ])
            .split(area);

        // Progress bar
        let progress_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Overall Progress ")
            .title_style(Style::default().fg(Color::White).bold());

        let progress_label = if total_size > 0 {
            format!(
                "{:.1}% • {} / {}",
                progress_pct,
                self.format_bytes(total_received),
                self.format_bytes(total_size)
            )
        } else {
            format!(
                "{:.1}% • {} received",
                progress_pct,
                self.format_bytes(total_received)
            )
        };

        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(
                Style::default()
                    .fg(if progress_pct >= 100.0 {
                        Color::Green
                    } else {
                        Color::Blue
                    })
                    .bg(Color::DarkGray),
            )
            .percent(progress_pct as u16)
            .label(Span::styled(
                progress_label,
                Style::default().fg(Color::White).bold(),
            ));

        f.render_widget(progress, chunks[0]);

        // Stats
        let elapsed_time = if let Some(start_time) =
            self.get_operation_start_time()
        {
            let elapsed = start_time.elapsed();
            format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
        } else {
            "00:00".to_string()
        };

        let estimated_remaining = if progress_pct > 0.0
            && progress_pct < 100.0
            && transfer_speed > 0.0
            && total_size > 0
        {
            let remaining_bytes = total_size.saturating_sub(total_received);
            let remaining_secs = remaining_bytes as f64 / transfer_speed;
            format!(
                "{}:{:02}",
                (remaining_secs as u64) / 60,
                (remaining_secs as u64) % 60
            )
        } else {
            "--:--".to_string()
        };

        let stats_content = vec![
            Line::from(vec![
                Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    self.format_speed(transfer_speed),
                    Style::default().fg(Color::Cyan).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("⏱️ ", Style::default().fg(Color::Yellow)),
                Span::styled(elapsed_time, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("⏰ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    estimated_remaining,
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ];

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Stats ")
            .title_style(Style::default().fg(Color::White).bold());

        let stats = Paragraph::new(stats_content)
            .block(stats_block)
            .alignment(Alignment::Left);

        f.render_widget(stats, chunks[1]);
    }

    fn draw_files_list(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let files = self.get_files();

        if files.is_empty() {
            let empty_content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("📁 ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        "Waiting for files...",
                        Style::default().fg(Color::Gray).italic(),
                    ),
                ]),
                Line::from(""),
            ];

            let empty_block = Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(Style::default().fg(Color::Gray))
                .title(" Files ")
                .title_style(Style::default().fg(Color::White).bold());

            let empty_widget = Paragraph::new(empty_content)
                .block(empty_block)
                .alignment(Alignment::Center);

            f.render_widget(empty_widget, area);
            return;
        }

        let file_items: Vec<ListItem> = files
            .iter()
            .map(|file| {
                let progress_pct = if file.total_size > 0 {
                    (file.received as f64 / file.total_size as f64) * 100.0
                } else {
                    // For files without known total size, show as receiving if
                    // chunks are coming
                    if file.status == FileTransferStatus::Receiving {
                        50.0 // Show partial progress
                    } else if file.status == FileTransferStatus::Completed {
                        100.0
                    } else {
                        0.0
                    }
                };

                // Create a mini progress bar using Unicode blocks
                let progress_width = 20.0;
                let filled_width =
                    ((progress_pct / 100.0) * progress_width as f64) as f64;
                let progress_bar = format!(
                    "{}{}",
                    "█".repeat(filled_width as usize),
                    "░".repeat((progress_width - filled_width) as usize)
                );

                let file_name = if file.name.len() > 25 {
                    format!("{}...", &file.name[..22])
                } else {
                    file.name.clone()
                };

                let status_line = Line::from(vec![
                    Span::styled(
                        format!("{} ", file.status.icon()),
                        Style::default().fg(file.status.color()),
                    ),
                    Span::styled(
                        format!("{:<25}", file_name),
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(
                        format!(" {} ", progress_bar),
                        Style::default().fg(if progress_pct >= 100.0 {
                            Color::Green
                        } else {
                            Color::Blue
                        }),
                    ),
                    Span::styled(
                        format!("{:>6.1}%", progress_pct),
                        Style::default().fg(Color::Cyan),
                    ),
                ]);

                let detail_text = if file.total_size > 0 {
                    format!(
                        "{} / {} • {}",
                        self.format_bytes(file.received),
                        self.format_bytes(file.total_size),
                        if file.status == FileTransferStatus::Receiving {
                            self.format_speed(file.transfer_speed)
                        } else {
                            match file.status {
                                FileTransferStatus::Completed => {
                                    "Complete".to_string()
                                }
                                _ => "--".to_string(),
                            }
                        }
                    )
                } else {
                    format!(
                        "{} received • {}",
                        self.format_bytes(file.received),
                        if file.status == FileTransferStatus::Receiving {
                            self.format_speed(file.transfer_speed)
                        } else {
                            match file.status {
                                FileTransferStatus::Completed => {
                                    "Complete".to_string()
                                }
                                _ => "--".to_string(),
                            }
                        }
                    )
                };

                let detail_line = Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(detail_text, Style::default().fg(Color::Gray)),
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

    fn draw_footer(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();
        let sender_name = self.get_sender_name();
        let total_chunks = self.get_total_chunks_received();

        let (footer_text, footer_color, footer_icon) = if progress_pct >= 100.0
        {
            (
                "All files received successfully! Press ESC to continue"
                    .to_string(),
                Color::Green,
                "✅",
            )
        } else if let Some(_bubble) = self
            .b
            .get_receive_files_manager()
            .get_receive_files_bubble()
        {
            (
                format!(
                    "Receiving from {} • {} chunks • Press ESC to cancel",
                    sender_name, total_chunks
                ),
                Color::Blue,
                "📥",
            )
        } else {
            (
                "Preparing to receive... Press ESC to cancel".to_string(),
                Color::Yellow,
                "⏳",
            )
        };

        let footer_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("{} ", footer_icon),
                    Style::default().fg(footer_color),
                ),
                Span::styled(footer_text, Style::default().fg(Color::White)),
            ]),
        ];

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(footer_color))
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer, area);
    }
}

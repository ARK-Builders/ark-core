use std::{
    sync::{Arc, RwLock, atomic::AtomicU32},
    time::Instant,
};

use crate::{
    App, AppBackend, ControlCapture, utilities::clipboard::copy_to_clipboard,
};
use arkdropx_sender::SendFilesSubscriber;
use crossterm::event::KeyModifiers;
use qrcode::QrCode;
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
    total_size: u64,
    sent: u64,
    status: FileTransferStatus,
    transfer_speed: f64, // bytes per second
    last_update: Instant,
}

#[derive(Clone, PartialEq)]
enum FileTransferStatus {
    Transferring,
    Completed,
}

impl FileTransferStatus {
    fn icon(&self) -> &'static str {
        match self {
            FileTransferStatus::Transferring => "📤",
            FileTransferStatus::Completed => "✅",
        }
    }

    fn color(&self) -> Color {
        match self {
            FileTransferStatus::Transferring => Color::Blue,
            FileTransferStatus::Completed => Color::Green,
        }
    }
}

pub struct SendFilesProgressApp {
    id: String,
    b: Arc<dyn AppBackend>,

    progress_pct: AtomicU32,
    operation_start_time: RwLock<Option<Instant>>,

    title_text: RwLock<String>,
    block_title_text: RwLock<String>,

    status_text: RwLock<String>,

    log_text: RwLock<String>, // TODO: info | display log text on UI

    files: RwLock<Vec<ProgressFile>>,
    total_transfer_speed: RwLock<f64>,

    // Copy feedback for T/Y clipboard shortcuts
    copy_feedback: RwLock<Option<(String, Instant)>>,
}

impl App for SendFilesProgressApp {
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
        self.draw_main_content(f, blocks[2]);
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
                        self.b.get_send_files_manager().cancel();
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
                    // T/Y copy shortcuts - only in waiting mode (before transfer starts)
                    KeyCode::Char('t') | KeyCode::Char('T')
                        if !self.has_transfer_started() =>
                    {
                        self.copy_ticket_to_clipboard();
                    }
                    KeyCode::Char('y') | KeyCode::Char('Y')
                        if !self.has_transfer_started() =>
                    {
                        self.copy_confirmation_to_clipboard();
                    }
                    _ => return None,
                }
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }
}

impl SendFilesSubscriber for SendFilesProgressApp {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.set_log_text(message.as_str());
    }

    fn notify_sending(&self, event: arkdropx_sender::SendFilesSendingEvent) {
        let id = event.id;
        let name = event.name;
        let remaining = event.remaining;
        let sent = event.sent;
        let total_size = sent + remaining;
        let now = Instant::now();

        // Try to find and update existing file or add new file
        let mut files = self.files.write().unwrap();
        if let Some(file) = files.iter_mut().find(|f| f.id == id) {
            // Calculate transfer speed
            let time_diff = now.duration_since(file.last_update).as_secs_f64();
            let bytes_diff = sent.saturating_sub(file.sent);

            if time_diff > 0.0 && bytes_diff > 0 {
                file.transfer_speed = bytes_diff as f64 / time_diff;
            }

            file.sent = sent;
            file.status = if remaining == 0 {
                FileTransferStatus::Completed
            } else {
                FileTransferStatus::Transferring
            };
            file.last_update = now;
        } else {
            files.push(ProgressFile {
                id: id.clone(),
                name: name.clone(),
                total_size,
                sent,
                status: if remaining == 0 {
                    FileTransferStatus::Completed
                } else {
                    FileTransferStatus::Transferring
                },
                transfer_speed: 0.0,
                last_update: now,
            });
        }

        // Calculate total transfer speed
        let total_speed: f64 = files
            .iter()
            .filter(|f| f.status == FileTransferStatus::Transferring)
            .map(|f| f.transfer_speed)
            .sum();
        *self.total_transfer_speed.write().unwrap() = total_speed;

        // Recalculate total progress
        let total_files_size: u64 = files.iter().map(|f| f.total_size).sum();
        let total_sent_size: u64 = files.iter().map(|f| f.sent).sum();
        let progress_pct = if total_files_size > 0 {
            ((total_sent_size as f64 / total_files_size as f64) * 100.0)
                .min(100.0)
        } else {
            0.0
        };

        self.progress_pct
            .store(progress_pct as u32, std::sync::atomic::Ordering::Relaxed);
    }

    fn notify_connecting(
        &self,
        event: arkdropx_sender::SendFilesConnectingEvent,
    ) {
        let receiver = event.receiver;
        let name = receiver.name;

        self.set_now_as_operation_start_time();
        self.set_title_text("📤 Sending Files");
        self.set_block_title_text(format!("Connected to {name}").as_str());
        self.set_status_text(format!("Sending Files to {name}").as_str());
    }
}

impl SendFilesProgressApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            b,

            progress_pct: AtomicU32::new(0),
            operation_start_time: RwLock::new(None),

            title_text: RwLock::new("📤 Sending Files".to_string()),
            block_title_text: RwLock::new("Waiting for Connection".to_string()),

            status_text: RwLock::new("Waiting for Peer".to_string()),

            log_text: RwLock::new("Initializing transfer...".to_string()),

            files: RwLock::new(Vec::new()),
            total_transfer_speed: RwLock::new(0.0),

            copy_feedback: RwLock::new(None),
        }
    }

    fn has_transfer_started(&self) -> bool {
        self.get_operation_start_time().is_some()
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

    fn get_progress_pct(&self) -> f64 {
        let v = self
            .progress_pct
            .load(std::sync::atomic::Ordering::Relaxed);
        f64::from(v)
    }

    fn get_operation_start_time(&self) -> Option<Instant> {
        *self.operation_start_time.read().unwrap()
    }

    fn get_files(&self) -> Vec<ProgressFile> {
        self.files.read().unwrap().clone()
    }

    fn get_total_transfer_speed(&self) -> f64 {
        *self.total_transfer_speed.read().unwrap()
    }

    fn copy_ticket_to_clipboard(&self) {
        if let Some(bubble) = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble()
        {
            match copy_to_clipboard(&bubble.get_ticket()) {
                Ok(_) => self.set_copy_feedback("✓ Ticket copied!"),
                Err(e) => self.set_copy_feedback(&format!("✗ {}", e)),
            }
        }
    }

    fn copy_confirmation_to_clipboard(&self) {
        if let Some(bubble) = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble()
        {
            let conf = format!("{:02}", bubble.get_confirmation());
            match copy_to_clipboard(&conf) {
                Ok(_) => self.set_copy_feedback("✓ Code copied!"),
                Err(e) => self.set_copy_feedback(&format!("✗ {}", e)),
            }
        }
    }

    fn set_copy_feedback(&self, message: &str) {
        *self.copy_feedback.write().unwrap() =
            Some((message.to_string(), Instant::now()));
    }

    fn get_copy_feedback(&self) -> Option<String> {
        let feedback = self.copy_feedback.read().unwrap();
        if let Some((msg, time)) = feedback.as_ref()
            && time.elapsed().as_secs() < 2
        {
            return Some(msg.clone());
        }
        None
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
                Style::default().fg(Color::Blue).bold(),
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
        let total_sent: u64 = files.iter().map(|f| f.sent).sum();
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
                format!(
                    "{:.1}% • {} / {}",
                    progress_pct,
                    self.format_bytes(total_sent),
                    self.format_bytes(total_size)
                ),
                Style::default().fg(Color::White).bold(),
            ));

        f.render_widget(progress, chunks[0]);

        // Stats
        let elapsed_time = if let Some(start_time) =
            self.get_operation_start_time()
            && progress_pct < 100.0
        {
            let elapsed = start_time.elapsed();
            format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
        } else {
            "00:00".to_string()
        };

        let estimated_remaining = if progress_pct > 0.0
            && progress_pct < 100.0
            && transfer_speed > 0.0
        {
            let remaining_bytes = total_size.saturating_sub(total_sent);
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

    fn draw_main_content(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        if self.has_transfer_started() {
            self.draw_files_list(f, area);
        } else {
            self.draw_qr_code(f, area);
        }
    }

    fn draw_files_list(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let files = self.get_files();

        if files.is_empty() {
            let empty_content = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("📁 ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        "No files to transfer",
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
                    (file.sent as f64 / file.total_size as f64) * 100.0
                } else {
                    0.0
                };

                // Create a mini progress bar using Unicode blocks
                let progress_width = 20.0;
                let filled_width = (progress_pct / 100.0) * progress_width;
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

                let detail_line = Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        format!(
                            "{} / {} • {}",
                            self.format_bytes(file.sent),
                            self.format_bytes(file.total_size),
                            if file.status == FileTransferStatus::Transferring {
                                self.format_speed(file.transfer_speed)
                            } else {
                                match file.status {
                                    FileTransferStatus::Completed => {
                                        "Complete".to_string()
                                    }
                                    _ => "--".to_string(),
                                }
                            }
                        ),
                        Style::default().fg(Color::Gray),
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

    fn draw_qr_code(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let qr_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Scan to Connect ")
            .title_style(Style::default().fg(Color::White).bold());

        if let Some(bubble) = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble()
        {
            let qr_data = format!(
                "drop://receive?ticket={}&confirmation={}",
                bubble.get_ticket(),
                bubble.get_confirmation()
            );

            let qr_code = match QrCode::new(&qr_data) {
                Ok(code) => code,
                Err(_) => {
                    self.draw_qr_error(f, area, qr_block);
                    return;
                }
            };

            let qr_matrix = qr_code
                .render::<char>()
                .quiet_zone(false)
                .module_dimensions(1, 1)
                .build();

            // Split QR code into lines for display
            let qr_lines: Vec<Line> = qr_matrix
                .lines()
                .map(|line| {
                    Line::from(vec![Span::styled(
                        line.replace('█', "██").replace(' ', "  "), // Make blocks wider for better visibility
                        Style::default().fg(Color::White).bg(Color::Black),
                    )])
                })
                .collect();

            let qr_widget = Paragraph::new(qr_lines)
                .block(qr_block)
                .alignment(Alignment::Center);

            f.render_widget(qr_widget, area);
        } else {
            self.draw_qr_waiting(f, area, qr_block);
        }
    }

    fn draw_qr_error(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
        block: Block,
    ) {
        let error_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("❌ ", Style::default().fg(Color::Red)),
                Span::styled(
                    "Failed to generate QR code",
                    Style::default().fg(Color::Red).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Please check connection details",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let error_widget = Paragraph::new(error_content)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(error_widget, area);
    }

    fn draw_qr_waiting(
        &self,
        f: &mut Frame,
        area: ratatui::prelude::Rect,
        block: Block,
    ) {
        let waiting_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Preparing connection...",
                    Style::default().fg(Color::Yellow).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "QR code will appear when ready",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let waiting_widget = Paragraph::new(waiting_content)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(waiting_widget, area);
    }

    fn draw_footer(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();
        let copy_feedback = self.get_copy_feedback();

        let (footer_lines, footer_color) = if progress_pct >= 100.0 {
            (
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("✅ ", Style::default().fg(Color::Green)),
                        Span::styled(
                            "All files transferred successfully! Press ESC to continue",
                            Style::default().fg(Color::White),
                        ),
                    ]),
                ],
                Color::Green,
            )
        } else if let Some(bubble) = self
            .b
            .get_send_files_manager()
            .get_send_files_bubble()
        {
            // Show ticket and confirmation with copy hints when in waiting mode
            let mut lines = vec![Line::from(vec![
                Span::styled("🔑 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Confirmation: ",
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{:02}", bubble.get_confirmation()),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(
                    " [Y to copy]",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("  •  ", Style::default().fg(Color::Gray)),
                Span::styled("Ticket: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "(full)",
                    Style::default().fg(Color::DarkGray).italic(),
                ),
                Span::styled(
                    " [T to copy]",
                    Style::default().fg(Color::DarkGray),
                ),
            ])];

            // Show copy feedback if available
            if let Some(feedback) = copy_feedback {
                let feedback_color = if feedback.starts_with('✓') {
                    Color::Green
                } else {
                    Color::Red
                };
                lines.push(Line::from(vec![Span::styled(
                    feedback,
                    Style::default().fg(feedback_color).bold(),
                )]));
            } else {
                lines.push(Line::from(""));
            }

            (lines, Color::Blue)
        } else {
            (
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            "Preparing transfer... Press ESC to cancel",
                            Style::default().fg(Color::White),
                        ),
                    ]),
                ],
                Color::Yellow,
            )
        };

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(footer_color))
            .title(" Status ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer = Paragraph::new(footer_lines)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer, area);
    }

    fn reset(&self) {
        *self.operation_start_time.write().unwrap() = None;
        *self.files.write().unwrap() = Vec::new();
        *self.copy_feedback.write().unwrap() = None;
    }
}

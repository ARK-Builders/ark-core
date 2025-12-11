use std::{
    sync::{Arc, RwLock, atomic::AtomicU32},
    time::Instant,
};

use crate::{App, AppBackend, ControlCapture};
use arkdropx_sender::send_files_to::{
    SendFilesToConnectingEvent, SendFilesToSendingEvent, SendFilesToSubscriber,
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
    total_size: u64,
    sent: u64,
    status: FileTransferStatus,
    transfer_speed: f64,
    last_update: Instant,
}

#[derive(Clone, PartialEq)]
enum FileTransferStatus {
    Waiting,
    Transferring,
    Completed,
}

impl FileTransferStatus {
    fn icon(&self) -> &'static str {
        match self {
            FileTransferStatus::Waiting => "⏳",
            FileTransferStatus::Transferring => "📤",
            FileTransferStatus::Completed => "✅",
        }
    }

    fn color(&self) -> Color {
        match self {
            FileTransferStatus::Waiting => Color::Gray,
            FileTransferStatus::Transferring => Color::Blue,
            FileTransferStatus::Completed => Color::Green,
        }
    }
}

pub struct SendFilesToProgressApp {
    id: String,
    b: Arc<dyn AppBackend>,

    progress_pct: AtomicU32,
    operation_start_time: RwLock<Option<Instant>>,

    title_text: RwLock<String>,
    block_title_text: RwLock<String>,
    status_text: RwLock<String>,
    log_text: RwLock<String>,

    files: RwLock<Vec<ProgressFile>>,
    total_transfer_speed: RwLock<f64>,
    receiver_name: RwLock<String>,
}

impl App for SendFilesToProgressApp {
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
                        self.b.get_send_files_to_manager().cancel();
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

impl SendFilesToSubscriber for SendFilesToProgressApp {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.set_log_text(message.as_str());
    }

    fn notify_sending(&self, event: SendFilesToSendingEvent) {
        let id = event.id;
        let name = event.name;
        let remaining = event.remaining;
        let sent = event.sent;
        let total_size = sent + remaining;
        let now = Instant::now();

        let mut files = self.files.write().unwrap();
        if let Some(file) = files.iter_mut().find(|f| f.id == id) {
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

        let total_speed: f64 = files
            .iter()
            .filter(|f| f.status == FileTransferStatus::Transferring)
            .map(|f| f.transfer_speed)
            .sum();
        *self.total_transfer_speed.write().unwrap() = total_speed;

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

    fn notify_connecting(&self, event: SendFilesToConnectingEvent) {
        let receiver = event.receiver;
        let name = receiver.name.clone();

        *self.receiver_name.write().unwrap() = name.clone();
        self.set_now_as_operation_start_time();
        self.set_title_text("📤 Sending Files");
        self.set_block_title_text(format!("Connected to {name}").as_str());
        self.set_status_text(format!("Sending Files to {name}").as_str());
    }
}

impl SendFilesToProgressApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            b,

            progress_pct: AtomicU32::new(0),
            operation_start_time: RwLock::new(None),

            title_text: RwLock::new("📤 Sending Files".to_string()),
            block_title_text: RwLock::new("Connecting to Receiver".to_string()),
            status_text: RwLock::new("Establishing Connection".to_string()),
            log_text: RwLock::new("Initializing transfer...".to_string()),

            files: RwLock::new(Vec::new()),
            total_transfer_speed: RwLock::new(0.0),
            receiver_name: RwLock::new(String::new()),
        }
    }

    fn reset(&self) {
        self.progress_pct
            .store(0, std::sync::atomic::Ordering::Relaxed);
        *self.operation_start_time.write().unwrap() = None;
        *self.title_text.write().unwrap() = "📤 Sending Files".to_string();
        *self.block_title_text.write().unwrap() =
            "Connecting to Receiver".to_string();
        *self.status_text.write().unwrap() =
            "Establishing Connection".to_string();
        *self.log_text.write().unwrap() =
            "Initializing transfer...".to_string();
        self.files.write().unwrap().clear();
        *self.total_transfer_speed.write().unwrap() = 0.0;
        *self.receiver_name.write().unwrap() = String::new();
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
        self.files.read().unwrap().clone()
    }

    fn get_total_transfer_speed(&self) -> f64 {
        *self.total_transfer_speed.read().unwrap()
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
                Style::default().fg(Color::Magenta).bold(),
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
            .border_style(Style::default().fg(Color::Magenta))
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
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ])
            .split(area);

        let progress_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Overall Progress ")
            .title_style(Style::default().fg(Color::White).bold());

        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(
                Style::default()
                    .fg(if progress_pct >= 100.0 {
                        Color::Green
                    } else {
                        Color::Magenta
                    })
                    .bg(Color::DarkGray),
            )
            .percent(progress_pct as u16)
            .label(format!("{:.1}%", progress_pct));

        f.render_widget(progress, chunks[0]);

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Stats ")
            .title_style(Style::default().fg(Color::White).bold());

        let stats_content = vec![
            Line::from(vec![
                Span::styled("📊 ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!(
                        "{} / {}",
                        self.format_bytes(total_sent),
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
                let progress_pct = if file.total_size > 0 {
                    (file.sent as f64 / file.total_size as f64) * 100.0
                } else {
                    0.0
                };

                let status_line = Line::from(vec![
                    Span::styled(
                        format!("{} ", file.status.icon()),
                        Style::default().fg(file.status.color()),
                    ),
                    Span::styled(
                        file.name.clone(),
                        Style::default().fg(
                            if file.status == FileTransferStatus::Completed {
                                Color::Green
                            } else {
                                Color::White
                            },
                        ),
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

    fn draw_footer(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let progress_pct = self.get_progress_pct();

        let (footer_text, footer_color, footer_icon) = if progress_pct >= 100.0
        {
            (
                "All files sent successfully! Press ESC to continue"
                    .to_string(),
                Color::Green,
                "✅",
            )
        } else {
            (
                "Ctrl+C to cancel • ESC to go back".to_string(),
                Color::Magenta,
                "📤",
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

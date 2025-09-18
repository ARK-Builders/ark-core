use std::sync::{Arc, RwLock, atomic::AtomicUsize};

use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{App, AppBackend, ControlCapture, Page};

#[derive(Clone, PartialEq)]
enum MenuItem {
    SendFiles,
    ReceiveFiles,
    Config,
    Help,
}

pub struct HomeApp {
    b: Arc<dyn AppBackend>,

    // UI State
    menu: RwLock<ListState>,
    selected_item: AtomicUsize,

    // Status and feedback
    status_message: Arc<RwLock<String>>,
}

impl App for HomeApp {
    fn draw(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let blocks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([
                Constraint::Percentage(50), // Left side - welcome & info
                Constraint::Percentage(50), // Right side - menu
            ])
            .split(area);

        let left_blocks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Welcome message
                Constraint::Length(6), // Features
                Constraint::Min(0),    // Status info
            ])
            .split(blocks[0]);

        self.draw_welcome(f, left_blocks[0]);
        self.draw_features_overview(f, left_blocks[1]);
        self.draw_status_info(f, left_blocks[2]);
        self.draw_main_menu(f, blocks[1]);
    }

    fn handle_control(&self, ev: &Event) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            if has_ctrl {
                match key.code {
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        self.navigate_to_page(Page::Help);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        self.navigate_to_page(Page::SendFiles);
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.navigate_to_page(Page::ReceiveFiles);
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.navigate_to_page(Page::Config);
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
                        self.activate_current_item();
                    }
                    KeyCode::Char('1') => {
                        self.select_item(0);
                        self.activate_current_item();
                    }
                    KeyCode::Char('2') => {
                        self.select_item(1);
                        self.activate_current_item();
                    }
                    KeyCode::Char('3') => {
                        self.select_item(2);
                        self.activate_current_item();
                    }
                    KeyCode::Char('4') => {
                        self.select_item(3);
                        self.activate_current_item();
                    }
                    KeyCode::Esc => {
                        self.set_status_message(
                            "Press Ctrl+Q to quit application",
                        );
                    }
                    _ => return None,
                }
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }
}

impl HomeApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let mut menu = ListState::default();
        menu.select(Some(0));

        Self {
            b,
            menu: RwLock::new(menu),
            selected_item: AtomicUsize::new(0),
            status_message: Arc::new(RwLock::new(
                "Welcome to ARK Drop - Select an option to get started"
                    .to_string(),
            )),
        }
    }

    fn get_menu_items(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::SendFiles,
            MenuItem::ReceiveFiles,
            MenuItem::Config,
            MenuItem::Help,
        ]
    }

    fn get_selected_item(&self) -> usize {
        self.selected_item
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn navigate_up(&self) {
        let items = self.get_menu_items();
        let current = self.get_selected_item();
        let new_index = if current > 0 {
            current - 1
        } else {
            items.len() - 1
        };

        self.select_item(new_index);
    }

    fn navigate_down(&self) {
        let items = self.get_menu_items();
        let current = self.get_selected_item();
        let new_index = if current < items.len() - 1 {
            current + 1
        } else {
            0
        };

        self.select_item(new_index);
    }

    fn select_item(&self, index: usize) {
        let items = self.get_menu_items();
        if index < items.len() {
            self.selected_item
                .store(index, std::sync::atomic::Ordering::Relaxed);
            self.menu.write().unwrap().select(Some(index));

            // Update status message based on selection
            let item_name = match items.get(index) {
                Some(MenuItem::SendFiles) => "Send files to another device",
                Some(MenuItem::ReceiveFiles) => {
                    "Receive files from another device"
                }
                Some(MenuItem::Config) => {
                    "Configure your profile and preferences"
                }
                Some(MenuItem::Help) => "View help and keyboard shortcuts",
                None => "Unknown option",
            };
            self.set_status_message(item_name);
        }
    }

    fn activate_current_item(&self) {
        let items = self.get_menu_items();
        let current = self.get_selected_item();

        if let Some(item) = items.get(current) {
            match item {
                MenuItem::SendFiles => {
                    self.navigate_to_page(Page::SendFiles);
                }
                MenuItem::ReceiveFiles => {
                    self.navigate_to_page(Page::ReceiveFiles);
                }
                MenuItem::Config => {
                    self.navigate_to_page(Page::Config);
                }
                MenuItem::Help => {
                    self.navigate_to_page(Page::Help);
                }
            }
        }
    }

    fn navigate_to_page(&self, page: Page) {
        self.set_status_message(&format!("Navigating to {:?}...", page));
        self.b.get_navigation().navigate_to(page);
    }

    fn set_status_message(&self, message: &str) {
        *self.status_message.write().unwrap() = message.to_string();
    }

    fn get_status_message(&self) -> String {
        self.status_message.read().unwrap().clone()
    }

    fn draw_main_menu(&self, f: &mut Frame<'_>, area: Rect) {
        let menu_items = vec![
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        "📤 ",
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(
                        "Send Files",
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(" (1)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        "Share files with others",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]),
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        "📥 ",
                        Style::default().fg(Color::Blue).bold(),
                    ),
                    Span::styled(
                        "Receive Files",
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(" (2)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        "Download files from sender",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]),
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        "⚙️ ",
                        Style::default().fg(Color::Yellow).bold(),
                    ),
                    Span::styled(
                        "Configuration",
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(" (3)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        "Adjust settings and preferences",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]),
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        "❓ ",
                        Style::default().fg(Color::Magenta).bold(),
                    ),
                    Span::styled(
                        "Help",
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(" (4)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        "View help and keyboard shortcuts",
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ]),
        ];

        let menu_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::White))
            .title(" Main Menu ")
            .title_style(Style::default().fg(Color::White).bold());

        let menu = List::new(menu_items)
            .block(menu_block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        f.render_stateful_widget(menu, area, &mut self.menu.write().unwrap());
    }

    fn draw_welcome(&self, f: &mut Frame<'_>, area: Rect) {
        let config = self.b.get_config();
        let greeting_text = match config.avatar_name {
            Some(name) => format!("👋 Hi, {name}! "),
            None => "👋 ".to_string(),
        };

        let welcome_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(greeting_text, Style::default().fg(Color::Yellow)),
                Span::styled("Welcome to ", Style::default().fg(Color::White)),
                Span::styled(
                    "ARK Drop",
                    Style::default().fg(Color::Cyan).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🔒 ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Secure peer-to-peer file transfer",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Fast and reliable connections",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(""),
        ];

        let welcome_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" About ")
            .title_style(Style::default().fg(Color::White).bold());

        let welcome = Paragraph::new(welcome_content)
            .block(welcome_block)
            .alignment(Alignment::Left);

        f.render_widget(welcome, area);
    }

    fn draw_features_overview(&self, f: &mut Frame<'_>, area: Rect) {
        let features_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "No file size limits",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Green)),
                Span::styled(
                    "End-to-end encryption",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Works across networks",
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let features_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Green))
            .title(" Features ")
            .title_style(Style::default().fg(Color::White).bold());

        let features = Paragraph::new(features_content)
            .block(features_block)
            .alignment(Alignment::Left);

        f.render_widget(features, area);
    }

    fn draw_status_info(&self, f: &mut Frame<'_>, area: Rect) {
        let status_message = self.get_status_message();

        let status_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("🟢 ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Ready to transfer files",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("ℹ️ ", Style::default().fg(Color::Blue)),
                Span::styled(
                    status_message,
                    Style::default().fg(Color::Gray).italic(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Ctrl+Q", Style::default().fg(Color::Red).bold()),
                Span::styled(" - Quit • ", Style::default().fg(Color::Gray)),
                Span::styled("1-4", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    " - Quick select",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ];

        let status_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Status & Controls ")
            .title_style(Style::default().fg(Color::White).bold());

        let status = Paragraph::new(status_content)
            .block(status_block)
            .alignment(Alignment::Left);

        f.render_widget(status, area);
    }
}

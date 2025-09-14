use std::sync::{Arc, RwLock};

use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{App, AppBackend, Page};

pub struct HomeApp {
    b: Arc<dyn AppBackend>,

    menu: RwLock<ListState>,
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

        draw_welcome(f, left_blocks[0]);
        draw_features_overview(f, left_blocks[1]);
        draw_status_info(f, left_blocks[2]);
        self.draw_main_menu(f, blocks[1])
    }

    fn handle_control(&self, ev: &Event) {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;
            match key.code {
                KeyCode::Up => {
                    let selected =
                        self.menu.write().unwrap().selected().unwrap_or(0);
                    if selected > 0 {
                        self.menu
                            .write()
                            .unwrap()
                            .select(Some(selected - 1));
                    } else {
                        self.menu.write().unwrap().select(Some(3));
                    }
                }
                KeyCode::Down => {
                    let selected =
                        self.menu.write().unwrap().selected().unwrap_or(0);
                    if selected < 3 {
                        self.menu
                            .write()
                            .unwrap()
                            .select(Some(selected + 1));
                    } else {
                        self.menu.write().unwrap().select(Some(0));
                    }
                }
                KeyCode::Enter => {
                    let nav = self.b.get_navigation();

                    match self.menu.write().unwrap().selected() {
                        Some(0) => nav.navigate_to(Page::SendFiles),
                        Some(1) => nav.navigate_to(Page::ReceiveFiles),
                        Some(2) => nav.navigate_to(Page::Config),
                        Some(3) => nav.navigate_to(Page::Help),
                        _ => {}
                    }
                }
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    if has_ctrl {
                        self.b.get_navigation().navigate_to(Page::Help);
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if has_ctrl {
                        self.b
                            .get_navigation()
                            .navigate_to(Page::SendFiles);
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if has_ctrl {
                        self.b
                            .get_navigation()
                            .navigate_to(Page::ReceiveFiles);
                    }
                }
                _ => {}
            }
        }
    }
}

fn draw_welcome(f: &mut Frame<'_>, area: Rect) {
    let welcome_content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("👋 ", Style::default().fg(Color::Yellow)),
            Span::styled("Welcome to ", Style::default().fg(Color::White)),
            Span::styled("ARK Drop", Style::default().fg(Color::Cyan).bold()),
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

fn draw_features_overview(f: &mut Frame<'_>, area: Rect) {
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

fn draw_status_info(f: &mut Frame<'_>, area: Rect) {
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
        Line::from(vec![Span::styled(
            "Get started by choosing an option →",
            Style::default().fg(Color::Gray).italic(),
        )]),
    ];

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Gray))
        .title(" Status ")
        .title_style(Style::default().fg(Color::White).bold());

    let status = Paragraph::new(status_content)
        .block(status_block)
        .alignment(Alignment::Left);

    f.render_widget(status, area);
}

impl HomeApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let mut menu = ListState::default();
        menu.select(Some(0));

        Self {
            b,
            menu: RwLock::new(menu),
        }
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
                    Span::styled("� ", Style::default().fg(Color::Blue).bold()),
                    Span::styled(
                        "Receive Files",
                        Style::default().fg(Color::White).bold(),
                    ),
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
}

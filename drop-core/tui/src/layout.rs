use std::sync::{Arc, RwLock, atomic::AtomicBool};

use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{App, AppNavigation, Page};

#[derive(Clone)]
pub struct LayoutChild {
    pub page: Option<Page>,
    pub app: Arc<dyn App>,
    pub is_active: bool,
    pub z_index: i32,
    pub control_index: i32,
}

pub struct LayoutApp {
    children: RwLock<Vec<LayoutChild>>,

    current_page: RwLock<Page>,
    previous_pages: RwLock<Vec<Page>>,

    is_finished: AtomicBool,
}

impl App for LayoutApp {
    fn draw(&self, f: &mut Frame, area: Rect) {
        let blocks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(5), // Title
                Constraint::Min(0),    // Main content
                Constraint::Length(4), // Footer/Help
            ])
            .split(area);

        let children = self.get_active_children_sort_by_z_index();

        draw_title(f, blocks[0]);
        draw_content(f, blocks[1], children);
        self.draw_footer(f, blocks[2]);
    }

    fn handle_control(&self, ev: &Event) {
        let children = self.get_active_children_sort_by_control_index();

        self.handle_global_event(ev);

        children
            .iter()
            .for_each(|c| c.app.handle_control(ev));
    }
}

impl AppNavigation for LayoutApp {
    fn navigate_to(&self, page: Page) {
        let current_page = self.current_page.read().unwrap().clone();
        let mut updated_current_page = false;
        let mut updated_previous_pages = false;
        let mut children = self.children.write().unwrap();

        for child in children.iter_mut() {
            if let Some(child_page) = &child.page {
                if child_page == &page {
                    child.is_active = true;
                    *self.current_page.write().unwrap() = page.clone();
                    updated_current_page = true;
                } else if child_page == &current_page {
                    child.is_active = false;
                    self.previous_pages
                        .write()
                        .unwrap()
                        .push(current_page.clone());
                    updated_previous_pages = true;
                }
            }

            if updated_current_page && updated_previous_pages {
                break;
            }
        }
    }

    fn replace_with(&self, page: Page) {
        let current_page = self.current_page.read().unwrap().clone();
        let mut updated_current_page = false;
        let mut updated_previous_pages = false;
        let mut children = self.children.write().unwrap();

        for child in children.iter_mut() {
            if let Some(child_page) = &child.page {
                if child_page == &page {
                    child.is_active = true;
                    *self.current_page.write().unwrap() = page.clone();
                    updated_current_page = true;
                } else if child_page == &current_page {
                    child.is_active = false;
                    updated_previous_pages = true;
                }
            }

            if updated_current_page && updated_previous_pages {
                break;
            }
        }
    }

    fn navigate_fresh_to(&self, page: Page) {
        let current_page = self.current_page.read().unwrap().clone();
        let mut updated_current_page = false;
        let mut updated_previous_pages = false;
        let mut children = self.children.write().unwrap();

        for child in children.iter_mut() {
            if let Some(child_page) = &child.page {
                if child_page == &page {
                    child.is_active = true;
                    *self.current_page.write().unwrap() = page.clone();
                    updated_current_page = true;
                } else if child_page == &current_page {
                    child.is_active = false;
                    self.previous_pages.write().unwrap().clear();
                    updated_previous_pages = true;
                }
            }

            if updated_current_page && updated_previous_pages {
                break;
            }
        }
    }

    fn go_back(&self) {
        let current_page = self.current_page.read().unwrap().clone();
        let last_page = self.previous_pages.write().unwrap().pop();
        let mut updated_current_page = false;
        let mut updated_previous_pages = false;
        let mut children = self.children.write().unwrap();

        match last_page {
            Some(page) => {
                for child in children.iter_mut() {
                    if let Some(child_page) = &child.page {
                        if child_page == &page {
                            child.is_active = true;
                            *self.current_page.write().unwrap() = page.clone();
                            updated_current_page = true;
                        } else if child_page == &current_page {
                            child.is_active = false;
                            updated_previous_pages = true;
                        }
                    }

                    if updated_current_page && updated_previous_pages {
                        break;
                    }
                }
            }
            None => {}
        }
    }
}

impl LayoutApp {
    pub fn new() -> Self {
        Self {
            children: RwLock::new(Vec::new()),

            current_page: RwLock::new(Page::Home),
            previous_pages: RwLock::new(Vec::new()),

            is_finished: AtomicBool::new(false),
        }
    }

    pub fn add_child(&self, c: LayoutChild) {
        self.children.write().unwrap().push(c);
    }

    fn get_active_children(&self) -> Vec<LayoutChild> {
        self.children
            .read()
            .unwrap()
            .clone()
            .into_iter()
            .filter_map(|c| {
                if c.is_active {
                    return Some(c);
                }
                return None;
            })
            .collect()
    }

    fn get_active_children_sort_by_z_index(&self) -> Vec<LayoutChild> {
        let mut children = self.get_active_children();
        children.sort_by(|a, b| a.z_index.cmp(&b.z_index));
        return children;
    }

    fn get_active_children_sort_by_control_index(&self) -> Vec<LayoutChild> {
        let mut children = self.get_active_children();
        children.sort_by(|a, b| a.control_index.cmp(&b.z_index));
        return children;
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn handle_global_event(&self, ev: &Event) {
        match ev {
            Event::Key(key) => match key.code {
                KeyCode::Char(c) => {
                    let pressed_quit = c == 'q'
                        || c == 'Q' && key.modifiers == KeyModifiers::CONTROL;
                    if pressed_quit {
                        self.finish();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn finish(&self) {
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let current_page = self.current_page.read().unwrap().clone();

        let (help_text, status_color) = match current_page {
            Page::Home => (
                "↑/↓ Navigate • Enter/Space Select • CTRL-S Send • CTRL-R Receive • CTRL-H Help • CTRL-Q Quit",
                Color::Cyan,
            ),
            Page::SendFiles => (
                "Tab Next Field • Enter Send • Esc Back • CTRL-Q Quit",
                Color::Green,
            ),
            Page::ReceiveFiles => (
                "↑/↓ Navigate • Tab Next Field • CTRL-Enter Receive • Esc Back • CTRL-Q Quit",
                Color::Blue,
            ),
            Page::Config => (
                "↑/↓ Navigate • Enter/Space Select • Esc Back • CTRL-Q Quit",
                Color::Yellow,
            ),
            Page::Help => ("Esc Back • CTRL-Q Quit", Color::Magenta),
            Page::SendFilesProgress => {
                // TODO: info | set dynamic messages according to the transfer
                // real-time progress/state
                ("Transfer in progress... • CTRL-Q Quit", Color::Green)
            }
            Page::ReceiveFilesProgress => {
                // TODO: info | set dynamic messages according to the transfer
                // real-time progress/state
                ("Transfer in progress... • CTRL-Q Quit", Color::Blue)
            }
            Page::FileBrowser => (
                "↑/↓ Navigate • Enter Go • Space Select • ESC|CTRL-S Save • CTRL-H Hidden • CTRL-J Sort • CTRL-C Cancel • CTRL-Q Quit",
                Color::Blue,
            ),
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

        f.render_widget(footer, area);
    }
}

fn draw_title(f: &mut Frame, area: Rect) {
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

    f.render_widget(title, area);
}

fn draw_content(f: &mut Frame, area: Rect, children: Vec<LayoutChild>) {
    children.iter().for_each(|c| c.app.draw(f, area));
}

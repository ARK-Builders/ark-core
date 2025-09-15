use std::{
    env,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    App, AppFileBrowser, AppFileBrowserManager, AppFileBrowserSubscriber,
    AppNavigation, OpenFileBrowserRequest, Page,
    utilities::helper_footer::{HelperFooterControl, create_helper_footer},
};

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

    file_browser: RwLock<Option<Arc<dyn AppFileBrowser>>>,
    file_browser_subs: RwLock<Vec<(Page, Arc<dyn AppFileBrowserSubscriber>)>>,
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

impl AppFileBrowserManager for LayoutApp {
    fn open_file_browser(&self, req: OpenFileBrowserRequest) {
        if let Some(fb) = self.get_file_browser() {
            let curr_dir = env::current_dir().unwrap();
            let sub = self.get_file_browser_sub(&req.from);

            fb.clear_selection();
            fb.set_mode(req.mode);
            fb.set_sort(req.sort);
            fb.set_current_path(curr_dir);

            if let Some(sub) = sub {
                fb.set_subscriber(sub);
            }

            self.navigate_to(Page::FileBrowser);
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

            file_browser: RwLock::new(None),
            file_browser_subs: RwLock::new(Vec::new()),
        }
    }

    fn get_file_browser_sub(
        &self,
        page: &Page,
    ) -> Option<Arc<dyn AppFileBrowserSubscriber>> {
        self.file_browser_subs
            .read()
            .unwrap()
            .iter()
            .find_map(|(p, s)| {
                if p == page {
                    return Some(s.clone());
                }
                return None;
            })
    }

    pub fn set_file_browser(&self, fb: Arc<dyn AppFileBrowser>) {
        self.file_browser.write().unwrap().replace(fb);
    }

    pub fn file_browser_subscribe(
        &self,
        page: Page,
        sub: Arc<dyn AppFileBrowserSubscriber>,
    ) {
        self.file_browser_subs
            .write()
            .unwrap()
            .push((page, sub));
    }

    fn get_file_browser(&self) -> Option<Arc<dyn AppFileBrowser>> {
        return self.file_browser.read().unwrap().clone();
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

        let footer = match current_page {
            Page::Home => create_helper_footer(vec![
                HelperFooterControl::new("↑/↓", "Navigate"),
                HelperFooterControl::new("Enter/Space", "Interact"),
                HelperFooterControl::new("CTRL-S", "Send"),
                HelperFooterControl::new("CTRL-R", "Receive"),
                HelperFooterControl::new("CTRL-H", "Help"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
            Page::SendFiles => create_helper_footer(vec![
                HelperFooterControl::new("↑/↓", "Navigate"),
                HelperFooterControl::new("Enter/Space", "Interact"),
                HelperFooterControl::new("CTRL-Enter", "Send"),
                HelperFooterControl::new("ESC", "Back"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
            Page::ReceiveFiles => create_helper_footer(vec![
                HelperFooterControl::new("↑/↓", "Navigate"),
                HelperFooterControl::new("Enter/Space", "Interact"),
                HelperFooterControl::new("CTRL-Enter", "Receive"),
                HelperFooterControl::new("ESC", "Back"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
            Page::Config => create_helper_footer(vec![
                HelperFooterControl::new("↑/↓", "Navigate"),
                HelperFooterControl::new("Enter/Space", "Interact"),
                HelperFooterControl::new("ESC", "Back"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
            Page::Help => create_helper_footer(vec![
                HelperFooterControl::new("ESC", "Back"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
            Page::SendFilesProgress => {
                // TODO: info | set dynamic messages according to the transfer
                // real-time progress/state
                create_helper_footer(vec![
                    HelperFooterControl::new("ESC", "Back"),
                    HelperFooterControl::new("CTRL-Q", "Quit"),
                ])
            }
            Page::ReceiveFilesProgress => {
                // TODO: info | set dynamic messages according to the transfer
                // real-time progress/state
                create_helper_footer(vec![
                    HelperFooterControl::new("ESC", "Back"),
                    HelperFooterControl::new("CTRL-Q", "Quit"),
                ])
            }
            Page::FileBrowser => create_helper_footer(vec![
                HelperFooterControl::new("↑/↓", "Navigate"),
                HelperFooterControl::new("Enter", "Into"),
                HelperFooterControl::new("Space", "Select"),
                HelperFooterControl::new("ESC/CTRL-S", "Save"),
                HelperFooterControl::new("CTRL-H", "Hidden"),
                HelperFooterControl::new("CTRL-J", "Sort"),
                HelperFooterControl::new("CTRL-K", "Reset"),
                HelperFooterControl::new("CTRL-C", "Cancel"),
                HelperFooterControl::new("CTRL-Q", "Quit"),
            ]),
        };

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

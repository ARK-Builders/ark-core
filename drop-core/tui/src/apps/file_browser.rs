use ratatui::{
    Frame,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{
    env,
    fs::{self, DirEntry},
    ops::Deref,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::SystemTime,
};

use crate::{
    App, AppBackend, AppFileBrowser, AppFileBrowserSaveEvent,
    AppFileBrowserSubscriber, BrowserMode, ControlCapture, SortMode,
};

#[derive(Clone, Debug, PartialEq)]
struct FileItem {
    name: String,
    path: PathBuf,
    size: Option<u64>,
    modified: Option<SystemTime>,
    is_hidden: bool,
    is_selected: bool,
    is_directory: bool,
}

pub struct FileBrowserApp {
    b: Arc<dyn AppBackend>,

    menu: RwLock<ListState>,

    current_path: RwLock<PathBuf>,
    items: RwLock<Vec<FileItem>>,
    sort: RwLock<SortMode>,

    mode: RwLock<BrowserMode>,
    selected_files_in: RwLock<Vec<PathBuf>>,

    has_hidden_items: AtomicBool,
    enforced_extensions: RwLock<Vec<String>>,

    // TODO: extra | implement dynamic filter based on user's input
    filter_in: RwLock<String>,

    sub: RwLock<Option<Arc<dyn AppFileBrowserSubscriber>>>,
}

impl App for FileBrowserApp {
    fn draw(&self, f: &mut Frame, area: Rect) {
        let blocks = self.get_layout_blocks(area);

        self.refresh();
        self.draw_header(f, blocks[0]);
        self.draw_file_list(f, blocks[1]);
        self.draw_footer_with_help(f, blocks[2]);
    }

    fn handle_control(&self, ev: &Event) -> Option<ControlCapture> {
        if let Event::Key(key) = ev {
            let has_ctrl = key.modifiers == KeyModifiers::CONTROL;

            match key.code {
                KeyCode::Up => {
                    self.go_up();
                }
                KeyCode::Down => {
                    self.go_down();
                }
                KeyCode::Enter => self.enter_current_menu_item(),
                KeyCode::Char(' ') => self.select_current_menu_item(),
                KeyCode::Esc => {
                    self.on_save();
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if has_ctrl {
                        self.on_save()
                    }
                }
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    if has_ctrl {
                        self.toggle_hidden()
                    }
                }
                KeyCode::Char('j') | KeyCode::Char('J') => {
                    if has_ctrl {
                        self.cycle_sort_mode()
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if has_ctrl {
                        self.b.get_navigation().go_back();
                    }
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
                    if has_ctrl {
                        self.reset();
                    }
                }
                _ => return None,
            }

            return Some(ControlCapture::new(ev));
        }

        None
    }
}

impl AppFileBrowser for FileBrowserApp {
    fn set_subscriber(&self, sub: Arc<dyn AppFileBrowserSubscriber>) {
        self.sub.write().unwrap().replace(sub);
    }

    fn pop_subscriber(&self) {
        self.sub.write().unwrap().take();
    }

    fn get_selected_files(&self) -> Vec<PathBuf> {
        self.selected_files_in.read().unwrap().clone()
    }

    fn select_file(&self, path: PathBuf) {
        self.selected_files_in.write().unwrap().push(path);
    }

    fn deselect_file(&self, file: PathBuf) {
        let mut selected_files = self.selected_files_in.write().unwrap();
        let selected_file_index =
            selected_files.iter().position(|f| *f == file);

        if let Some(selected_file_index) = selected_file_index {
            selected_files.remove(selected_file_index);
        }
    }

    fn set_mode(&self, mode: BrowserMode) {
        *self.mode.write().unwrap() = mode;
    }

    fn set_sort(&self, sort: SortMode) {
        *self.sort.write().unwrap() = sort;
    }

    fn set_current_path(&self, path: PathBuf) {
        if path.exists() {
            *self.current_path.write().unwrap() = path;
        } else {
            // TODO: info | log exception on TUI
        }
    }

    fn clear_selection(&self) {
        self.selected_files_in.write().unwrap().clear();
        for item in self.items.write().unwrap().iter_mut() {
            item.is_selected = false;
        }
    }
}

impl FileBrowserApp {
    pub fn new(b: Arc<dyn AppBackend>) -> Self {
        let default_start_path =
            env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        Self {
            b,

            menu: RwLock::new(ListState::default()),

            current_path: RwLock::new(default_start_path),
            items: RwLock::new(Vec::new()),
            sort: RwLock::new(SortMode::Name),

            mode: RwLock::new(BrowserMode::SelectMultiFiles),
            selected_files_in: RwLock::new(Vec::new()),

            has_hidden_items: AtomicBool::new(false),
            enforced_extensions: RwLock::new(Vec::new()),

            filter_in: RwLock::new(String::new()),

            sub: RwLock::new(None),
        }
    }

    fn refresh(&self) {
        self.refresh_items();
        self.refresh_menu();
    }

    fn refresh_menu(&self) {
        let items = self.items.read().unwrap();
        let mut menu = self.menu.write().unwrap();

        if items.is_empty() {
            menu.select(None);
        } else if menu.selected().is_none() {
            menu.select(Some(0));
        }
    }

    fn sort_items(&self, items: &mut Vec<FileItem>) {
        items.sort_by(|a, b| {
            // Always put directories first
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    // Then sort by the selected criteria
                    match self.sort.read().unwrap().deref() {
                        SortMode::Name => {
                            a.name.to_lowercase().cmp(&b.name.to_lowercase())
                        }
                        SortMode::Size => {
                            let a_size = a.size.unwrap_or(0);
                            let b_size = b.size.unwrap_or(0);
                            b_size.cmp(&a_size) // Descending order
                        }
                        SortMode::Modified => match (a.modified, b.modified) {
                            (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => a.name.cmp(&b.name),
                        },
                        SortMode::Type => {
                            let a_ext = a.path.extension().unwrap_or_default();
                            let b_ext = b.path.extension().unwrap_or_default();
                            a_ext.cmp(b_ext)
                        }
                    }
                }
            }
        });
    }

    fn go_up(&self) {
        let items = self.items.read().unwrap();

        if items.is_empty() {
            return;
        }

        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected().unwrap_or(0);
        let last_index = items.len() - 1;

        if selected > 0 {
            menu.select(Some(selected - 1));
        } else {
            menu.select(Some(last_index));
        }
    }

    fn go_down(&self) {
        let items = self.items.read().unwrap();

        if items.is_empty() {
            return;
        }

        let mut menu = self.menu.write().unwrap();
        let selected = menu.selected().unwrap_or(0);
        let last_index = items.len() - 1;

        if selected < last_index {
            menu.select(Some(selected + 1));
        } else {
            menu.select(Some(0));
        }
    }

    fn toggle_hidden(&self) {
        let current = self
            .has_hidden_items
            .load(std::sync::atomic::Ordering::Relaxed);
        self.has_hidden_items
            .store(!current, std::sync::atomic::Ordering::Relaxed);
    }

    fn cycle_sort_mode(&self) {
        let mut sort_by = self.sort.write().unwrap();

        *sort_by = match sort_by.deref() {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Modified,
            SortMode::Modified => SortMode::Type,
            SortMode::Type => SortMode::Name,
        };
    }

    fn get_menu(&self) -> ListState {
        self.menu.read().unwrap().clone()
    }

    fn enter_current_menu_item(&self) {
        let menu = self.get_menu();
        let items = self.get_items();

        if let Some(current_index) = menu.selected()
            && let Some(item) = items.get(current_index)
            && item.is_directory
        {
            self.enter_item_path(item);
        }
    }

    fn enter_item_path(&self, item: &FileItem) {
        *self.current_path.write().unwrap() = item.path.clone();

        self.menu.write().unwrap().select(Some(0));
    }

    fn select_current_menu_item(&self) {
        let mode = self.get_mode();
        let menu = self.get_menu();

        if let Some(item_idx) = menu.selected()
            && let Some(item) = self.items.write().unwrap().get_mut(item_idx)
        {
            match mode {
                BrowserMode::SelectFile => {
                    self.select_file(item);
                    self.on_save();
                }
                BrowserMode::SelectDirectory => {
                    self.select_dir(item);
                    self.on_save();
                }
                BrowserMode::SelectMultiFiles => {
                    self.select_file(item);
                }
            }
        }
    }

    fn reset(&self) {
        self.filter_in.write().unwrap().clear();
        self.selected_files_in.write().unwrap().clear();

        for item in self.items.write().unwrap().iter_mut() {
            item.is_selected = false;
        }
    }

    fn get_current_path(&self) -> PathBuf {
        self.current_path.read().unwrap().clone()
    }

    fn is_extension_valid(&self, name: &String) -> bool {
        let enforced_extensions = self.enforced_extensions.read().unwrap();
        if enforced_extensions.is_empty() {
            return true;
        }
        enforced_extensions
            .iter()
            .any(|ee| name.ends_with(&format!(".{ee}")))
    }

    fn is_hidden_valid(&self, is_hidden: bool) -> bool {
        self.has_hidden_items
            .load(std::sync::atomic::Ordering::Relaxed)
            && is_hidden
    }

    fn refresh_items(&self) {
        let mut items = self.items.write().unwrap();
        let current_path = self.current_path.read().unwrap();

        items.clear();

        // Add parent directory entry if not at root
        if let Some(parent) = current_path.parent() {
            items.push(FileItem {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_directory: true,
                is_hidden: false,
                size: None,
                modified: None,
                is_selected: false,
            });
        }

        // Add directory contents
        if let Ok(entries) = fs::read_dir(current_path.deref()) {
            let mut dir_items: Vec<FileItem> = entries
                .filter_map(|entry| {
                    match entry {
                        Ok(entry) => self.transform_to_item(entry),
                        Err(_) => {
                            // TODO: info | log exception on TUI
                            None
                        }
                    }
                })
                .collect();

            // Sort based on current sort mode
            self.sort_items(&mut dir_items);
            items.extend(dir_items);
        }
    }

    fn transform_to_item(&self, entry: DirEntry) -> Option<FileItem> {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_directory = path.is_dir();
        let is_hidden = name.starts_with('.');
        let is_hidden_valid = self.is_hidden_valid(is_hidden);
        let is_extension_valid = self.is_extension_valid(&name);
        let is_valid = is_hidden_valid && is_extension_valid;

        if is_valid {
            return None;
        }

        let (size, modified) = if let Ok(metadata) = entry.metadata() {
            let size = if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            };
            let modified = metadata.modified().ok();
            (size, modified)
        } else {
            (None, None)
        };

        let is_selected = self
            .selected_files_in
            .read()
            .unwrap()
            .contains(&path);

        Some(FileItem {
            name,
            path,
            is_directory,
            is_hidden,
            size,
            modified,
            is_selected,
        })
    }

    fn on_save(&self) {
        let selected_files = self.get_selected_files();

        if selected_files.is_empty() {
            return;
        }

        if let Some(sub) = self.get_sub() {
            sub.on_save(AppFileBrowserSaveEvent { selected_files });
        }

        self.b.get_navigation().go_back();
    }

    fn select_file(&self, item: &mut FileItem) {
        if item.is_directory {
            return;
        }

        let mut selected_files_in = self.selected_files_in.write().unwrap();

        if item.is_selected {
            selected_files_in.retain(|p| p != &item.path);
            item.is_selected = false;
        } else {
            if !selected_files_in.contains(&item.path) {
                selected_files_in.push(item.path.clone());
            }
            item.is_selected = true;
        }
    }

    fn select_dir(&self, item: &mut FileItem) {
        if !item.is_directory {
            return;
        }

        self.select_item(item);
    }

    fn select_item(&self, item: &mut FileItem) {
        let mut selected_files_in = self.selected_files_in.write().unwrap();

        if item.is_selected {
            selected_files_in.retain(|p| p != &item.path);
            item.is_selected = false;
        } else {
            if !selected_files_in.contains(&item.path) {
                selected_files_in.push(item.path.clone());
            }
            item.is_selected = true;
        }
    }

    fn has_hidden_items(&self) -> bool {
        self.has_hidden_items
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_sort(&self) -> SortMode {
        self.sort.read().unwrap().clone()
    }

    fn get_mode(&self) -> BrowserMode {
        self.mode.read().unwrap().clone()
    }

    fn draw_header(&self, f: &mut Frame, block: Rect) {
        let current_path = self.get_current_path();
        let show_hidden = self.has_hidden_items();
        let sort = self.get_sort();
        let selected_files = self.get_selected_files();
        let mode = self.get_mode();

        // Header with current path and controls
        let header_content = vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Current Path: ",
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(
                    format!("{}", current_path.display()),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Sort: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:?}", sort),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(" • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if show_hidden {
                        "Hidden: On"
                    } else {
                        "Hidden: Off"
                    },
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    if matches!(mode, BrowserMode::SelectMultiFiles) {
                        format!(" • Selected: {}", selected_files.len())
                    } else {
                        String::new()
                    },
                    Style::default().fg(Color::Green),
                ),
            ]),
        ];

        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Blue))
            .title(match mode {
                BrowserMode::SelectFile => " File Browser - Select a File ",
                BrowserMode::SelectMultiFiles => {
                    " File Browser - Select Multiple Files "
                }
                BrowserMode::SelectDirectory => {
                    " Directory Browser - Select Folder "
                }
            })
            .title_style(Style::default().fg(Color::White).bold());

        let header = Paragraph::new(header_content)
            .block(header_block)
            .alignment(Alignment::Left);

        f.render_widget(header, block);
    }

    fn get_items(&self) -> Vec<FileItem> {
        self.items.read().unwrap().clone()
    }

    fn get_list_items(&self) -> Vec<ListItem<'static>> {
        self.get_items()
            .iter()
            .map(|item| transform_into_list_item(item.clone()))
            .collect()
    }

    fn draw_file_list(&self, f: &mut Frame, block: Rect) {
        let list_items = self.get_list_items();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::White));

        let list = List::new(list_items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, block, &mut self.menu.write().unwrap());
    }

    fn draw_footer_with_help(&self, f: &mut Frame, block: Rect) {
        let footer_content = vec![Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                ": Enter Directory • ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled("Space", Style::default().fg(Color::Green).bold()),
            Span::styled(
                ": Select/Deselect • ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled("CTRL-H", Style::default().fg(Color::Yellow).bold()),
            Span::styled(": Hidden • ", Style::default().fg(Color::Gray)),
            Span::styled("CTRL-J", Style::default().fg(Color::Magenta).bold()),
            Span::styled(": Sort • ", Style::default().fg(Color::Gray)),
            Span::styled("CTRL-C", Style::default().fg(Color::Magenta).bold()),
            Span::styled(": Cancel • ", Style::default().fg(Color::Gray)),
            Span::styled("CTRL-S", Style::default().fg(Color::Red).bold()),
            Span::styled(": Save", Style::default().fg(Color::Gray)),
        ])];

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Controls ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);

        f.render_widget(footer, block);
    }

    fn get_layout_blocks(&self, area: Rect) -> Rc<[Rect]> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header with path and controls
                Constraint::Min(0),    // File list
                Constraint::Length(3), // Footer with help
            ])
            .split(area)
    }

    fn get_sub(&self) -> Option<Arc<dyn AppFileBrowserSubscriber>> {
        self.sub.read().unwrap().clone()
    }
}

fn transform_into_list_item(item: FileItem) -> ListItem<'static> {
    let (icon, color) = if item.name == ".." {
        ("⬆️ ", Color::Yellow)
    } else if item.is_directory {
        ("📁 ", Color::Cyan)
    } else {
        match item.path.extension().and_then(|s| s.to_str()) {
            Some("txt") | Some("md") | Some("rs") => ("📝 ", Color::Green),
            Some("jpg") | Some("png") | Some("gif") => ("🖼️ ", Color::Magenta),
            Some("mp3") | Some("wav") | Some("flac") => ("🎵 ", Color::Blue),
            Some("mp4") | Some("avi") | Some("mkv") => ("🎬 ", Color::Red),
            Some("zip") | Some("tar") | Some("gz") => ("📦 ", Color::Yellow),
            _ => ("📄 ", Color::White),
        }
    };

    let size_str = if let Some(size) = item.size {
        format_file_size(size)
    } else if item.is_directory && item.name != ".." {
        "<DIR>".to_string()
    } else {
        String::new()
    };

    let selection_indicator = if item.is_selected {
        "✓ "
    } else {
        "  "
    };

    let style = if item.is_selected {
        Style::default().fg(Color::Green).bold()
    } else if item.is_hidden {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(color)
    };

    let line = if size_str.is_empty() {
        format!("{}{}{}", selection_indicator, icon, item.name)
    } else {
        format!(
            "{}{}{} ({})",
            selection_indicator, icon, item.name, size_str
        )
    };

    ListItem::new(line).style(style)
}

fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size_f = size as f64;
    let mut unit_idx = 0;

    while size_f >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size_f, UNITS[unit_idx])
    }
}

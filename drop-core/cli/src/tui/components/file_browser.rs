// Placeholder for future file browser component
// This would allow users to visually browse and select files in TUI mode

use ratatui::{
    Frame,
    backend::Backend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use std::path::{Path, PathBuf};

pub struct FileBrowser {
    pub current_path: PathBuf,
    pub items: Vec<FileItem>,
    pub state: ListState,
}

#[derive(Clone)]
pub struct FileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
}

impl FileBrowser {
    pub fn new(path: PathBuf) -> Self {
        let mut browser = Self {
            current_path: path.clone(),
            items: Vec::new(),
            state: ListState::default(),
        };
        browser.refresh();
        browser.state.select(Some(0));
        browser
    }

    pub fn refresh(&mut self) {
        self.items.clear();

        // Add parent directory entry if not at root
        if let Some(parent) = self.current_path.parent() {
            self.items.push(FileItem {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_directory: true,
            });
        }

        // Add directory contents
        if let Ok(entries) = std::fs::read_dir(&self.current_path) {
            let mut items: Vec<FileItem> = entries
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_directory = path.is_dir();

                    Some(FileItem {
                        name,
                        path,
                        is_directory,
                    })
                })
                .collect();

            // Sort: directories first, then files
            items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            });

            self.items.extend(items);
        }
    }

    pub fn render<B: Backend>(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let icon = if item.is_directory {
                    "📁"
                } else {
                    "📄"
                };
                let style = if item.is_directory {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(format!("{} {}", icon, item.name)).style(style)
            })
            .collect();

        let title = format!("Browse: {}", self.current_path.display());
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("→ ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    pub fn navigate_up(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        if selected > 0 {
            self.state.select(Some(selected - 1));
        } else if !self.items.is_empty() {
            self.state.select(Some(self.items.len() - 1));
        }
    }

    pub fn navigate_down(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        if selected < self.items.len().saturating_sub(1) {
            self.state.select(Some(selected + 1));
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn enter_selected(&mut self) -> Option<PathBuf> {
        if let Some(selected) = self.state.selected() {
            if let Some(item) = self.items.get(selected) {
                if item.is_directory {
                    self.current_path = item.path.clone();
                    self.refresh();
                    self.state.select(Some(0));
                    None
                } else {
                    Some(item.path.clone())
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_current_path(&self) -> &Path {
        &self.current_path
    }

    pub fn get_selected_item(&self) -> Option<&FileItem> {
        if let Some(selected) = self.state.selected() {
            self.items.get(selected)
        } else {
            None
        }
    }
}

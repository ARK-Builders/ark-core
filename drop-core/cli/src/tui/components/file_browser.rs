use ratatui::{
    Frame,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{
    env,
    fs::{self},
    path::{Path, PathBuf},
    time::SystemTime,
};

pub struct FileBrowser {
    pub current_path: PathBuf,
    pub items: Vec<FileItem>,
    pub state: ListState,
    pub selected_files: Vec<PathBuf>,
    pub mode: BrowserMode,
    pub show_hidden: bool,
    pub sort_by: SortMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserMode {
    SelectFiles,     // Multiple file selection for sending
    SelectDirectory, // Single directory selection for receive path
}

#[derive(Clone, Debug, PartialEq)]
pub enum SortMode {
    Name,
    Size,
    Modified,
    Type,
}

#[derive(Clone, Debug)]
pub struct FileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
    pub is_hidden: bool,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub is_selected: bool,
}

impl FileBrowser {
    pub fn new(path: PathBuf, mode: BrowserMode) -> Self {
        let start_path = if path.exists() {
            path
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        };

        let mut browser = Self {
            current_path: start_path,
            items: Vec::new(),
            state: ListState::default(),
            selected_files: Vec::new(),
            mode,
            show_hidden: false,
            sort_by: SortMode::Name,
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
                is_hidden: false,
                size: None,
                modified: None,
                is_selected: false,
            });
        }

        // Add directory contents
        if let Ok(entries) = fs::read_dir(&self.current_path) {
            let mut items: Vec<FileItem> = entries
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_directory = path.is_dir();
                    let is_hidden = name.starts_with('.');

                    // Skip hidden files if not showing them
                    if is_hidden && !self.show_hidden {
                        return None;
                    }

                    let (size, modified) =
                        if let Ok(metadata) = entry.metadata() {
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

                    let is_selected = self.selected_files.contains(&path);

                    Some(FileItem {
                        name,
                        path,
                        is_directory,
                        is_hidden,
                        size,
                        modified,
                        is_selected,
                    })
                })
                .collect();

            // Sort based on current sort mode
            self.sort_items(&mut items);
            self.items.extend(items);
        }

        // Ensure we have a valid selection
        if !self.items.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
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
                    match self.sort_by {
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
                            a_ext.cmp(&b_ext)
                        }
                    }
                }
            }
        });
    }

    pub fn render<B: Backend>(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header with path and controls
                Constraint::Min(0),    // File list
                Constraint::Length(3), // Footer with help
            ])
            .split(area);

        // Header with current path and controls
        let header_content = vec![
            Line::from(vec![
                Span::styled("📁 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "Current Path: ",
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(
                    format!("{}", self.current_path.display()),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Sort: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:?}", self.sort_by),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(" • ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if self.show_hidden {
                        "Hidden: On"
                    } else {
                        "Hidden: Off"
                    },
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    if matches!(self.mode, BrowserMode::SelectFiles) {
                        format!(" • Selected: {}", self.selected_files.len())
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
            .title(match self.mode {
                BrowserMode::SelectFiles => " File Browser - Select Files ",
                BrowserMode::SelectDirectory => {
                    " Directory Browser - Select Folder "
                }
            })
            .title_style(Style::default().fg(Color::White).bold());

        let header = Paragraph::new(header_content)
            .block(header_block)
            .alignment(Alignment::Left);
        f.render_widget(header, chunks[0]);

        // File list
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let (icon, color) = if item.name == ".." {
                    ("⬆️ ", Color::Yellow)
                } else if item.is_directory {
                    ("📁 ", Color::Cyan)
                } else {
                    match item.path.extension().and_then(|s| s.to_str()) {
                        Some("txt") | Some("md") | Some("rs") => {
                            ("📝 ", Color::Green)
                        }
                        Some("jpg") | Some("png") | Some("gif") => {
                            ("🖼️ ", Color::Magenta)
                        }
                        Some("mp3") | Some("wav") | Some("flac") => {
                            ("🎵 ", Color::Blue)
                        }
                        Some("mp4") | Some("avi") | Some("mkv") => {
                            ("🎬 ", Color::Red)
                        }
                        Some("zip") | Some("tar") | Some("gz") => {
                            ("📦 ", Color::Yellow)
                        }
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
            })
            .collect();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::White));

        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, chunks[1], &mut self.state);

        // Footer with controls
        let footer_content = match self.mode {
            BrowserMode::SelectFiles => vec![Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    ": Select/Navigate • ",
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("Space", Style::default().fg(Color::Green).bold()),
                Span::styled(": Toggle • ", Style::default().fg(Color::Gray)),
                Span::styled("H", Style::default().fg(Color::Yellow).bold()),
                Span::styled(": Hidden • ", Style::default().fg(Color::Gray)),
                Span::styled("S", Style::default().fg(Color::Magenta).bold()),
                Span::styled(": Sort • ", Style::default().fg(Color::Gray)),
                Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                Span::styled(": Done", Style::default().fg(Color::Gray)),
            ])],
            BrowserMode::SelectDirectory => vec![Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    ": Navigate/Select • ",
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("Tab", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    ": Select Current • ",
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("H", Style::default().fg(Color::Yellow).bold()),
                Span::styled(": Hidden • ", Style::default().fg(Color::Gray)),
                Span::styled("Esc", Style::default().fg(Color::Red).bold()),
                Span::styled(": Cancel", Style::default().fg(Color::Gray)),
            ])],
        };

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Gray))
            .title(" Controls ")
            .title_style(Style::default().fg(Color::White).bold());

        let footer = Paragraph::new(footer_content)
            .block(footer_block)
            .alignment(Alignment::Center);
        f.render_widget(footer, chunks[2]);
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

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }

    pub fn cycle_sort_mode(&mut self) {
        self.sort_by = match self.sort_by {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Modified,
            SortMode::Modified => SortMode::Type,
            SortMode::Type => SortMode::Name,
        };
        self.refresh();
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
                    // For file selection mode, return the file path
                    match self.mode {
                        BrowserMode::SelectFiles => Some(item.path.clone()),
                        BrowserMode::SelectDirectory => None, /* Can't select files in directory mode */
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn toggle_selected(&mut self) -> bool {
        if let Some(selected_idx) = self.state.selected() {
            if let Some(item) = self.items.get_mut(selected_idx) {
                // Only allow file selection in SelectFiles mode
                if matches!(self.mode, BrowserMode::SelectFiles)
                    && !item.is_directory
                {
                    if item.is_selected {
                        // Remove from selection
                        self.selected_files.retain(|p| p != &item.path);
                        item.is_selected = false;
                    } else {
                        // Add to selection
                        if !self.selected_files.contains(&item.path) {
                            self.selected_files.push(item.path.clone());
                        }
                        item.is_selected = true;
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn select_current_directory(&self) -> PathBuf {
        self.current_path.clone()
    }

    pub fn get_selected_files(&self) -> Vec<PathBuf> {
        self.selected_files.clone()
    }

    pub fn clear_selection(&mut self) {
        self.selected_files.clear();
        for item in &mut self.items {
            item.is_selected = false;
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

    pub fn go_to_home(&mut self) {
        if let Ok(home) = env::var("HOME") {
            self.current_path = PathBuf::from(home);
        } else if let Ok(userprofile) = env::var("USERPROFILE") {
            self.current_path = PathBuf::from(userprofile);
        } else {
            self.current_path = PathBuf::from("/");
        }
        self.refresh();
        self.state.select(Some(0));
    }

    pub fn go_to_root(&mut self) {
        self.current_path = PathBuf::from("/");
        self.refresh();
        self.state.select(Some(0));
    }
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

// System file browser integration
pub fn open_system_file_browser(
    mode: BrowserMode,
    current_path: Option<PathBuf>,
) -> Result<Vec<PathBuf>, String> {
    let default_path = current_path.unwrap_or_else(|| {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    });

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        match mode {
            BrowserMode::SelectFiles => {
                // Use PowerShell with Windows Forms for file selection
                let script = format!(
                    r#"
                    Add-Type -AssemblyName System.Windows.Forms
                    $openFileDialog = New-Object System.Windows.Forms.OpenFileDialog
                    $openFileDialog.InitialDirectory = '{}'
                    $openFileDialog.Multiselect = $true
                    $openFileDialog.Title = 'Select Files to Send'
                    $openFileDialog.Filter = 'All Files (*.*)|*.*'
                    if ($openFileDialog.ShowDialog() -eq 'OK') {{
                        $openFileDialog.FileNames | ForEach-Object {{ Write-Output $_ }}
                    }}
                    "#,
                    default_path.display()
                );

                match Command::new("powershell")
                    .args(&["-Command", &script])
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let files: Vec<PathBuf> =
                                String::from_utf8_lossy(&output.stdout)
                                    .lines()
                                    .filter(|line| !line.trim().is_empty())
                                    .map(PathBuf::from)
                                    .collect();
                            Ok(files)
                        } else {
                            Err("File selection cancelled or failed"
                                .to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to open file dialog: {}", e)),
                }
            }
            BrowserMode::SelectDirectory => {
                let script = format!(
                    r#"
                    Add-Type -AssemblyName System.Windows.Forms
                    $folderBrowserDialog = New-Object System.Windows.Forms.FolderBrowserDialog
                    $folderBrowserDialog.SelectedPath = '{}'
                    $folderBrowserDialog.Description = 'Select Directory for Received Files'
                    if ($folderBrowserDialog.ShowDialog() -eq 'OK') {{
                        Write-Output $folderBrowserDialog.SelectedPath
                    }}
                    "#,
                    default_path.display()
                );

                match Command::new("powershell")
                    .args(&["-Command", &script])
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let path = String::from_utf8_lossy(&output.stdout)
                                .trim()
                                .to_string();
                            if !path.is_empty() {
                                Ok(vec![PathBuf::from(path)])
                            } else {
                                Err("Directory selection cancelled".to_string())
                            }
                        } else {
                            Err("Directory selection cancelled or failed"
                                .to_string())
                        }
                    }
                    Err(e) => {
                        Err(format!("Failed to open directory dialog: {}", e))
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        match mode {
            BrowserMode::SelectFiles => {
                let mut cmd = Command::new("osascript");
                cmd.args(&[
                    "-e",
                    &format!(
                        r#"tell application "Finder" to set theFiles to (choose file with prompt "Select files to send:" with multiple selections allowed default location alias POSIX file "{}")
set filePaths to {{}}
repeat with aFile in theFiles
    set end of filePaths to POSIX path of aFile
end repeat
set AppleScript's text item delimiters to "
"
set filePathsText to filePaths as text
set AppleScript's text item delimiters to ""
return filePathsText"#,
                        default_path.display()
                    ),
                ]);

                match cmd.output() {
                    Ok(output) => {
                        if output.status.success() {
                            let files: Vec<PathBuf> =
                                String::from_utf8_lossy(&output.stdout)
                                    .lines()
                                    .filter(|line| !line.trim().is_empty())
                                    .map(|line| PathBuf::from(line.trim()))
                                    .collect();
                            Ok(files)
                        } else {
                            Err("File selection cancelled".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to open file dialog: {}", e)),
                }
            }
            BrowserMode::SelectDirectory => {
                let mut cmd = Command::new("osascript");
                cmd.args(&[
                    "-e",
                    &format!(
                        r#"tell application "Finder" to set theFolder to (choose folder with prompt "Select directory for received files:" default location alias POSIX file "{}")
return POSIX path of theFolder"#,
                        default_path.display()
                    ),
                ]);

                match cmd.output() {
                    Ok(output) => {
                        if output.status.success() {
                            let path = String::from_utf8_lossy(&output.stdout)
                                .trim()
                                .to_string();
                            if !path.is_empty() {
                                Ok(vec![PathBuf::from(path)])
                            } else {
                                Err("Directory selection cancelled".to_string())
                            }
                        } else {
                            Err("Directory selection cancelled".to_string())
                        }
                    }
                    Err(e) => {
                        Err(format!("Failed to open directory dialog: {}", e))
                    }
                }
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        use std::process::Command;

        let zenity_path_config =
            format!("--filename={}/", default_path.display());
        let kdialog_path_config = format!("{}", default_path.display());
        let file_managers = [
            (
                "zenity",
                match mode {
                    BrowserMode::SelectFiles => vec![
                        "--file-selection",
                        "--multiple",
                        "--title=Select files to send",
                        &zenity_path_config,
                    ],
                    BrowserMode::SelectDirectory => vec![
                        "--file-selection",
                        "--directory",
                        "--title=Select directory for received files",
                        &zenity_path_config,
                    ],
                },
            ),
            (
                "kdialog",
                match mode {
                    BrowserMode::SelectFiles => vec![
                        "--getopenfilename",
                        &kdialog_path_config,
                        "--multiple",
                        "--title",
                        "Select files to send",
                    ],
                    BrowserMode::SelectDirectory => vec![
                        "--getexistingdirectory",
                        &kdialog_path_config,
                        "--title",
                        "Select directory for received files",
                    ],
                },
            ),
        ];

        for (manager, args) in &file_managers {
            if let Ok(output) = Command::new(manager).args(args).output() {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .to_string();
                    if !result.is_empty() {
                        return Ok(result
                            .split('|')
                            .map(PathBuf::from)
                            .collect());
                    }
                }
            }
        }

        Err("No suitable file manager found. Please install zenity or kdialog, or use the built-in TUI browser.".to_string())
    }
}

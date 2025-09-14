mod backend;
mod layout;
mod pages;

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use arkdrop_common::AppConfig;
use arkdropx_sender::{SendFilesBubble, SendFilesRequest};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, poll},
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    },
    layout::Rect,
};

use crate::{
    backend::MainAppBackend,
    layout::{LayoutApp, LayoutChild},
    pages::{
        file_browser::FileBrowserApp, help::HelpApp, home::HomeApp,
        send_files::SendFilesApp,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Home,
    Help,
    Config,
    SendFiles,
    FileBrowser,
    ReceiveFiles,
    SendFilesProgress,
    ReceiveFilesProgress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserMode {
    SelectFile,
    SelectDirectory,
    SelectMultiFiles,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SortMode {
    Name,
    Type,
    Size,
    Modified,
}

pub struct AppFileBrowserSaveEvent {
    pub selected_files: Vec<PathBuf>,
}

pub struct OpenFileBrowserRequest {
    pub from: Page,

    pub mode: BrowserMode,
    pub sort: SortMode,
}

pub trait App: Send + Sync {
    fn draw(&self, f: &mut Frame, area: Rect);
    fn handle_control(&self, ev: &Event);
}

pub trait AppNavigation: Send + Sync {
    fn navigate_to(&self, page: Page);
    fn replace_with(&self, page: Page);
    fn navigate_fresh_to(&self, page: Page);
    fn go_back(&self);
}

pub trait AppSendFilesManager: Send + Sync {
    fn send_files(&self, req: SendFilesRequest);
    fn get_send_files_bubble(&self) -> Option<Arc<SendFilesBubble>>;
}

pub trait AppFileBrowserManager: Send + Sync {
    fn open_file_browser(&self, req: OpenFileBrowserRequest);
}

pub trait AppBackend: Send + Sync {
    fn get_send_files_manager(&self) -> Arc<dyn AppSendFilesManager>;
    fn get_file_browser_manager(&self) -> Arc<dyn AppFileBrowserManager>;

    fn get_config(&self) -> AppConfig;
    fn get_navigation(&self) -> Arc<dyn AppNavigation>;
}

pub trait AppFileBrowserSubscriber: Send + Sync {
    fn on_cancel(&self);
    fn on_save(&self, ev: AppFileBrowserSaveEvent);
}

pub trait AppFileBrowser: Send + Sync {
    fn get_selected_files(&self) -> Vec<PathBuf>;

    fn select_file(&self, file: PathBuf);
    fn deselect_file(&self, file: PathBuf);

    fn set_subscriber(&self, sub: Arc<dyn AppFileBrowserSubscriber>);
    fn pop_subscriber(&self);

    fn set_mode(&self, mode: BrowserMode);
    fn set_sort(&self, sort: SortMode);

    fn set_current_path(&self, path: PathBuf);
    fn clear_selection(&self);
}

pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let b = Arc::new(MainAppBackend::new());
    let layout = Arc::new(LayoutApp::new());

    let home = Arc::new(HomeApp::new(b.clone()));

    let send_files = Arc::new(SendFilesApp::new(b.clone()));
    let file_browser = Arc::new(FileBrowserApp::new(b.clone()));

    let help = Arc::new(HelpApp::new(b.clone()));

    b.set_navigation(layout.clone());
    b.set_file_browser(file_browser.clone());
    b.file_browser_subscribe(Page::SendFiles, send_files.clone());

    // TODO: low | b.set_send_files_manager
    // TODO: low | b.set_file_browser_manager

    layout.add_child(LayoutChild {
        page: Some(Page::Home),
        app: home,
        is_active: true,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::SendFiles),
        app: send_files,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::FileBrowser),
        app: file_browser,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::Help),
        app: help,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    loop {
        terminal.draw(|f| {
            let area = f.area();
            layout.draw(f, area)
        })?;

        if poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            layout.handle_control(&ev);
        }

        if layout.is_finished() {
            break;
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(())
}

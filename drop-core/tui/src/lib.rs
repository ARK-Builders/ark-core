mod apps;
mod backend;
mod layout;
mod receive_files_manager;
mod send_files_manager;
mod utilities;

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use arkdrop_common::AppConfig;
use arkdropx_receiver::{ReceiveFilesBubble, ReceiveFilesRequest};
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
    apps::{
        config::ConfigApp, file_browser::FileBrowserApp, help::HelpApp,
        home::HomeApp, receive_files::ReceiveFilesApp,
        receive_files_progress::ReceiveFilesProgressApp,
        send_files::SendFilesApp, send_files_progress::SendFilesProgressApp,
    },
    backend::MainAppBackend,
    layout::{LayoutApp, LayoutChild},
    receive_files_manager::MainAppReceiveFilesManager,
    send_files_manager::MainAppSendFilesManager,
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

pub struct ControlCapture {
    pub ev: Event,
}

impl ControlCapture {
    pub fn new(ev: &Event) -> Self {
        return Self { ev: ev.clone() };
    }
}

pub trait App: Send + Sync {
    fn draw(&self, f: &mut Frame, area: Rect);
    fn handle_control(&self, ev: &Event) -> Option<ControlCapture>;
}

pub trait AppNavigation: Send + Sync {
    fn navigate_to(&self, page: Page);
    fn replace_with(&self, page: Page);
    fn navigate_fresh_to(&self, page: Page);
    fn go_back(&self);
}

pub trait AppSendFilesManager: Send + Sync {
    fn cancel(&self);
    fn send_files(&self, req: SendFilesRequest);
    fn get_send_files_bubble(&self) -> Option<Arc<SendFilesBubble>>;
}

pub trait AppReceiveFilesManager: Send + Sync {
    fn cancel(&self);
    fn receive_files(&self, req: ReceiveFilesRequest);
    fn get_receive_files_bubble(&self) -> Option<Arc<ReceiveFilesBubble>>;
}

pub trait AppFileBrowserManager: Send + Sync {
    fn open_file_browser(&self, req: OpenFileBrowserRequest);
}

pub trait AppBackend: Send + Sync {
    fn get_send_files_manager(&self) -> Arc<dyn AppSendFilesManager>;
    fn get_receive_files_manager(&self) -> Arc<dyn AppReceiveFilesManager>;
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
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let backend = Arc::new(MainAppBackend::new());
    let layout = Arc::new(LayoutApp::new());

    let file_browser = Arc::new(FileBrowserApp::new(backend.clone()));
    let help = Arc::new(HelpApp::new(backend.clone()));
    let home = Arc::new(HomeApp::new(backend.clone()));
    let config = Arc::new(ConfigApp::new(backend.clone()));

    let send_files = Arc::new(SendFilesApp::new(backend.clone()));
    let receive_files = Arc::new(ReceiveFilesApp::new(backend.clone()));

    let send_files_progress =
        Arc::new(SendFilesProgressApp::new(backend.clone()));
    let receive_files_progress =
        Arc::new(ReceiveFilesProgressApp::new(backend.clone()));

    let send_files_manager = Arc::new(MainAppSendFilesManager::new());
    let receive_files_manager = Arc::new(MainAppReceiveFilesManager::new());

    layout.set_file_browser(file_browser.clone());
    layout.file_browser_subscribe(Page::SendFiles, send_files.clone());
    layout.file_browser_subscribe(Page::Config, config.clone());

    backend.set_navigation(layout.clone());
    backend.set_file_browser_manager(layout.clone());
    backend.set_send_files_manager(send_files_manager.clone());
    backend.set_receive_files_manager(receive_files_manager.clone());

    send_files_manager.set_send_files_subscriber(send_files_progress.clone());
    receive_files_manager
        .set_receive_files_subscriber(receive_files_progress.clone());

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

    layout.add_child(LayoutChild {
        page: Some(Page::SendFilesProgress),
        app: send_files_progress,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::ReceiveFiles),
        app: receive_files,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::ReceiveFilesProgress),
        app: receive_files_progress,
        is_active: false,
        z_index: 0,
        control_index: 0,
    });

    layout.add_child(LayoutChild {
        page: Some(Page::Config),
        app: config,
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

        let should_finish = layout.is_finished();
        if should_finish {
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

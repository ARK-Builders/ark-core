use std::sync::{Arc, RwLock, atomic::AtomicBool};

use arkdrop_common::AppConfig;
use arkdropx_sender::{SendFilesBubble, SendFilesSubscriber};

use crate::{
    AppBackend, AppFileBrowser, AppFileBrowserManager,
    AppFileBrowserSubscriber, AppNavigation, AppSendFilesManager, Page,
};

pub struct MainAppBackend {
    is_shutdown: AtomicBool,

    send_files_manager: RwLock<Option<Arc<dyn AppSendFilesManager>>>,
    file_browser_manager: RwLock<Option<Arc<dyn AppFileBrowserManager>>>,

    navigation: RwLock<Option<Arc<dyn AppNavigation>>>,

    file_browser: RwLock<Option<Arc<dyn AppFileBrowser>>>,
    file_browser_subs: RwLock<Vec<(Page, Arc<dyn AppFileBrowserSubscriber>)>>,

    send_files_bub: RwLock<Option<Arc<SendFilesBubble>>>,
    send_files_sub: RwLock<Option<Arc<dyn SendFilesSubscriber>>>,
}

impl AppBackend for MainAppBackend {
    fn shutdown(&self) {
        self.is_shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_send_files_manager(&self) -> Arc<dyn AppSendFilesManager> {
        self.send_files_manager
            .read()
            .unwrap()
            .clone()
            .unwrap()
    }

    fn get_file_browser_manager(&self) -> Arc<dyn AppFileBrowserManager> {
        self.file_browser_manager
            .read()
            .unwrap()
            .clone()
            .unwrap()
    }

    fn get_config(&self) -> AppConfig {
        AppConfig::load().unwrap_or(AppConfig::default())
    }

    fn get_navigation(&self) -> Arc<dyn AppNavigation> {
        self.navigation.read().unwrap().clone().unwrap()
    }
}

impl MainAppBackend {
    pub fn new() -> Self {
        Self {
            is_shutdown: AtomicBool::new(false),

            send_files_manager: RwLock::new(None),
            file_browser_manager: RwLock::new(None),

            navigation: RwLock::new(None),

            file_browser: RwLock::new(None),
            file_browser_subs: RwLock::new(Vec::new()),

            send_files_bub: RwLock::new(None),
            send_files_sub: RwLock::new(None),
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_send_files_manager(
        &self,
        manager: Arc<dyn AppSendFilesManager>,
    ) {
        self.send_files_manager
            .write()
            .unwrap()
            .replace(manager);
    }

    pub fn set_file_browser_manager(
        &self,
        manager: Arc<dyn AppFileBrowserManager>,
    ) {
        self.file_browser_manager
            .write()
            .unwrap()
            .replace(manager);
    }

    pub fn set_send_files_subscriber(&self, sub: Arc<dyn SendFilesSubscriber>) {
        self.send_files_sub.write().unwrap().replace(sub);
    }

    pub fn set_navigation(&self, nav: Arc<dyn AppNavigation>) {
        self.navigation.write().unwrap().replace(nav);
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
}

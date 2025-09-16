use std::sync::{Arc, RwLock, atomic::AtomicBool};

use arkdrop_common::AppConfig;
use arkdropx_sender::{SendFilesBubble, SendFilesSubscriber};

use crate::{
    AppBackend, AppFileBrowser, AppFileBrowserManager,
    AppFileBrowserSubscriber, AppNavigation, AppReceiveFilesManager,
    AppSendFilesManager, Page,
};

pub struct MainAppBackend {
    is_shutdown: AtomicBool,

    send_files_manager: RwLock<Option<Arc<dyn AppSendFilesManager>>>,
    receive_files_manager: RwLock<Option<Arc<dyn AppReceiveFilesManager>>>,
    file_browser_manager: RwLock<Option<Arc<dyn AppFileBrowserManager>>>,

    navigation: RwLock<Option<Arc<dyn AppNavigation>>>,
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

    fn get_receive_files_manager(&self) -> Arc<dyn AppReceiveFilesManager> {
        self.receive_files_manager
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
            receive_files_manager: RwLock::new(None),
            file_browser_manager: RwLock::new(None),

            navigation: RwLock::new(None),
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

    pub fn set_receive_files_manager(
        &self,
        manager: Arc<dyn AppReceiveFilesManager>,
    ) {
        self.receive_files_manager
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

    pub fn set_navigation(&self, nav: Arc<dyn AppNavigation>) {
        self.navigation.write().unwrap().replace(nav);
    }
}

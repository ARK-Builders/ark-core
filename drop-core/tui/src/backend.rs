use std::sync::{Arc, RwLock};

use arkdrop_common::AppConfig;

use crate::{
    AppBackend, AppFileBrowserManager, AppNavigation, AppReadyToReceiveManager,
    AppReceiveFilesManager, AppSendFilesManager, AppSendFilesToManager,
};

pub struct MainAppBackend {
    send_files_manager: RwLock<Option<Arc<dyn AppSendFilesManager>>>,
    receive_files_manager: RwLock<Option<Arc<dyn AppReceiveFilesManager>>>,
    file_browser_manager: RwLock<Option<Arc<dyn AppFileBrowserManager>>>,
    send_files_to_manager: RwLock<Option<Arc<dyn AppSendFilesToManager>>>,
    ready_to_receive_manager: RwLock<Option<Arc<dyn AppReadyToReceiveManager>>>,

    navigation: RwLock<Option<Arc<dyn AppNavigation>>>,
}

impl AppBackend for MainAppBackend {
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

    fn get_send_files_to_manager(&self) -> Arc<dyn AppSendFilesToManager> {
        self.send_files_to_manager
            .read()
            .unwrap()
            .clone()
            .unwrap()
    }

    fn get_ready_to_receive_manager(
        &self,
    ) -> Arc<dyn AppReadyToReceiveManager> {
        self.ready_to_receive_manager
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
            send_files_manager: RwLock::new(None),
            receive_files_manager: RwLock::new(None),
            file_browser_manager: RwLock::new(None),
            send_files_to_manager: RwLock::new(None),
            ready_to_receive_manager: RwLock::new(None),

            navigation: RwLock::new(None),
        }
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

    pub fn set_send_files_to_manager(
        &self,
        manager: Arc<dyn AppSendFilesToManager>,
    ) {
        self.send_files_to_manager
            .write()
            .unwrap()
            .replace(manager);
    }

    pub fn set_ready_to_receive_manager(
        &self,
        manager: Arc<dyn AppReadyToReceiveManager>,
    ) {
        self.ready_to_receive_manager
            .write()
            .unwrap()
            .replace(manager);
    }

    pub fn set_navigation(&self, nav: Arc<dyn AppNavigation>) {
        self.navigation.write().unwrap().replace(nav);
    }
}

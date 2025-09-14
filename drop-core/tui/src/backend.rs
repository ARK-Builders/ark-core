use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};

use arkdrop_common::AppConfig;
use arkdropx_sender::{SendFilesBubble, SendFilesRequest, send_files};

use crate::{
    AppBackend, AppFileBrowser, AppFileBrowserSubscriber, AppNavigation,
    OpenFileBrowserRequest, Page,
};

pub struct MainAppBackend {
    navigation: RwLock<Option<Arc<dyn AppNavigation>>>,

    file_browser: RwLock<Option<Arc<dyn AppFileBrowser>>>,
    file_browser_subs: RwLock<Vec<(Page, Arc<dyn AppFileBrowserSubscriber>)>>,

    send_files_bub: RwLock<Option<SendFilesBubble>>,
}

impl MainAppBackend {
    pub fn new() -> Self {
        Self {
            navigation: RwLock::new(None),

            file_browser: RwLock::new(None),
            file_browser_subs: RwLock::new(Vec::new()),

            send_files_bub: RwLock::new(None),
        }
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

impl AppBackend for MainAppBackend {
    fn send_files(&self, req: SendFilesRequest) {
        todo!()
    }

    fn open_file_browser(&self, req: OpenFileBrowserRequest) {
        if let Some(fb) = self.file_browser.read().unwrap().deref() {
            for (subscriber_page, sub) in
                self.file_browser_subs.read().unwrap().deref()
            {
                if subscriber_page == &req.from {
                    let nav = self.get_navigation();

                    fb.clear_selection();
                    fb.set_subscriber(sub.clone());

                    fb.set_mode(req.mode);
                    fb.set_sort(req.sort);

                    nav.navigate_to(Page::FileBrowser);
                    break;
                }
            }
        }
    }

    fn get_config(&self) -> AppConfig {
        AppConfig::load().unwrap_or(AppConfig::default())
    }

    fn get_navigation(&self) -> Arc<dyn AppNavigation> {
        self.navigation.read().unwrap().clone().unwrap()
    }
}

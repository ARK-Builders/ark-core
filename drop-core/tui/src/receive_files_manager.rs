use std::sync::{Arc, RwLock};

use arkdropx_receiver::{
    ReceiveFilesBubble, ReceiveFilesSubscriber, receive_files,
};

use crate::AppReceiveFilesManager;

pub struct MainAppReceiveFilesManager {
    receive_files_bubble: Arc<RwLock<Option<Arc<ReceiveFilesBubble>>>>,
    receive_files_sub: RwLock<Option<Arc<dyn ReceiveFilesSubscriber>>>,
}

impl AppReceiveFilesManager for MainAppReceiveFilesManager {
    fn receive_files(&self, req: arkdropx_receiver::ReceiveFilesRequest) {
        let receive_files_bub = self.receive_files_bubble.clone();
        tokio::spawn(async move {
            let bubble = receive_files(req).await;
            match bubble {
                Ok(bub) => receive_files_bub
                    .write()
                    .unwrap()
                    .replace(Arc::new(bub)),
                Err(_) => todo!(),
            }
        });
    }

    fn get_receive_files_bubble(
        &self,
    ) -> Option<std::sync::Arc<arkdropx_receiver::ReceiveFilesBubble>> {
        let receive_files_bubble = self.receive_files_bubble.read().unwrap();
        return receive_files_bubble.clone();
    }
}

impl MainAppReceiveFilesManager {
    pub fn new() -> Self {
        Self {
            receive_files_bubble: Arc::new(RwLock::new(None)),
            receive_files_sub: RwLock::new(None),
        }
    }

    pub fn set_receive_files_subscriber(
        &self,
        sub: Arc<dyn ReceiveFilesSubscriber>,
    ) {
        self.receive_files_sub.write().unwrap().replace(sub);
    }
}

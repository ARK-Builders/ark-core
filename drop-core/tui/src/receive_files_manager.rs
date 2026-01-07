use std::sync::{Arc, RwLock};

use arkdropx_receiver::{
    ReceiveFilesBubble, ReceiveFilesSubscriber, receive_files,
};

use crate::AppReceiveFilesManager;

pub struct MainAppReceiveFilesManager {
    bubble: Arc<RwLock<Option<Arc<ReceiveFilesBubble>>>>,
    sub: Arc<RwLock<Option<Arc<dyn ReceiveFilesSubscriber>>>>,
}

impl AppReceiveFilesManager for MainAppReceiveFilesManager {
    fn cancel(&self) {
        let taken_bubble = self.bubble.write().unwrap().take();

        if let Some(bub) = &taken_bubble {
            bub.cancel();
        }
    }

    fn receive_files(&self, req: arkdropx_receiver::ReceiveFilesRequest) {
        let curr_sub = self.sub.clone();
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let bubble = receive_files(req).await;
            match bubble {
                Ok(bub) => {
                    let bub = Arc::new(bub);

                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        bub.subscribe(sub);
                    }

                    let _ = bub.start();

                    curr_bubble.write().unwrap().replace(bub);
                }
                Err(_) => todo!(),
            }
        });
    }

    fn get_receive_files_bubble(
        &self,
    ) -> Option<std::sync::Arc<arkdropx_receiver::ReceiveFilesBubble>> {
        let bubble = self.bubble.read().unwrap();
        bubble.clone()
    }
}

impl MainAppReceiveFilesManager {
    pub fn new() -> Self {
        Self {
            bubble: Arc::new(RwLock::new(None)),
            sub: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_receive_files_subscriber(
        &self,
        sub: Arc<dyn ReceiveFilesSubscriber>,
    ) {
        self.sub.write().unwrap().replace(sub);
    }
}

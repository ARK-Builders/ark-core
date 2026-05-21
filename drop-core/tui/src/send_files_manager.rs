use std::sync::{Arc, RwLock};

use arkdropx_sender::{SendFilesBubble, SendFilesSubscriber, send_files};

use crate::AppSendFilesManager;

pub struct MainAppSendFilesManager {
    bubble: Arc<RwLock<Option<Arc<SendFilesBubble>>>>,
    sub: Arc<RwLock<Option<Arc<dyn SendFilesSubscriber>>>>,
}

impl AppSendFilesManager for MainAppSendFilesManager {
    fn cancel(&self) {
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let taken_bubble = curr_bubble.write().unwrap().take();
            if let Some(bub) = &taken_bubble {
                let _ = bub.cancel().await;
            }
        });
    }

    fn send_files(&self, req: arkdropx_sender::SendFilesRequest) {
        let curr_sub = self.sub.clone();
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let bubble = send_files(req).await;
            match bubble {
                Ok(bub) => {
                    let bub = Arc::new(bub);

                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        bub.subscribe(sub);
                    }

                    curr_bubble.write().unwrap().replace(bub)
                }
                Err(_) => todo!(),
            }
        });
    }

    fn get_send_files_bubble(
        &self,
    ) -> Option<std::sync::Arc<arkdropx_sender::SendFilesBubble>> {
        let send_files_bubble = self.bubble.read().unwrap();
        send_files_bubble.clone()
    }
}

impl MainAppSendFilesManager {
    pub fn new() -> Self {
        Self {
            bubble: Arc::new(RwLock::new(None)),
            sub: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_send_files_subscriber(&self, sub: Arc<dyn SendFilesSubscriber>) {
        self.sub.write().unwrap().replace(sub);
    }
}

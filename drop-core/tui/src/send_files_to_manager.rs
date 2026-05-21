use std::sync::{Arc, RwLock};

use arkdropx_sender::send_files_to::{
    SendFilesToBubble, SendFilesToRequest, SendFilesToSubscriber, send_files_to,
};

use crate::AppSendFilesToManager;

pub struct MainAppSendFilesToManager {
    bubble: Arc<RwLock<Option<Arc<SendFilesToBubble>>>>,
    sub: Arc<RwLock<Option<Arc<dyn SendFilesToSubscriber>>>>,
}

impl AppSendFilesToManager for MainAppSendFilesToManager {
    fn cancel(&self) {
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let taken_bubble = curr_bubble.write().unwrap().take();
            if let Some(bub) = &taken_bubble {
                let _ = bub.cancel().await;
            }
        });
    }

    fn send_files_to(&self, req: SendFilesToRequest) {
        let curr_sub = self.sub.clone();
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let bubble = send_files_to(req).await;
            match bubble {
                Ok(bub) => {
                    let bub = Arc::new(bub);

                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        bub.subscribe(sub.clone());

                        // Start the transfer after subscribing
                        if let Err(e) = bub.start() {
                            sub.log(format!(
                                "[ERROR] Failed to start transfer: {}",
                                e
                            ));
                        }
                    }

                    curr_bubble.write().unwrap().replace(bub);
                }
                Err(e) => {
                    // Log error to subscriber if available
                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        sub.log(format!("[ERROR] Failed to connect: {}", e));
                    }
                }
            }
        });
    }

    fn get_send_files_to_bubble(&self) -> Option<Arc<SendFilesToBubble>> {
        let bubble = self.bubble.read().unwrap();
        bubble.clone()
    }
}

impl Default for MainAppSendFilesToManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MainAppSendFilesToManager {
    pub fn new() -> Self {
        Self {
            bubble: Arc::new(RwLock::new(None)),
            sub: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_send_files_to_subscriber(
        &self,
        sub: Arc<dyn SendFilesToSubscriber>,
    ) {
        self.sub.write().unwrap().replace(sub);
    }
}

use std::sync::{Arc, RwLock};

use arkdropx_receiver::ready_to_receive::{
    ReadyToReceiveBubble, ReadyToReceiveRequest, ReadyToReceiveSubscriber,
    ready_to_receive,
};

use crate::AppReadyToReceiveManager;

pub struct MainAppReadyToReceiveManager {
    bubble: Arc<RwLock<Option<Arc<ReadyToReceiveBubble>>>>,
    sub: Arc<RwLock<Option<Arc<dyn ReadyToReceiveSubscriber>>>>,
}

impl AppReadyToReceiveManager for MainAppReadyToReceiveManager {
    fn cancel(&self) {
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let taken_bubble = curr_bubble.write().unwrap().take();
            if let Some(bub) = &taken_bubble {
                let _ = bub.cancel().await;
            }
        });
    }

    fn ready_to_receive(&self, req: ReadyToReceiveRequest) {
        let curr_sub = self.sub.clone();
        let curr_bubble = self.bubble.clone();

        tokio::spawn(async move {
            let bubble = ready_to_receive(req).await;
            match bubble {
                Ok(bub) => {
                    let bub = Arc::new(bub);

                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        bub.subscribe(sub);
                    }

                    // No explicit start needed - the bubble starts waiting
                    // immediately
                    curr_bubble.write().unwrap().replace(bub);
                }
                Err(e) => {
                    // Log error to subscriber if available
                    if let Some(sub) = curr_sub.read().unwrap().clone() {
                        sub.log(format!("[ERROR] Failed to start: {}", e));
                    }
                }
            }
        });
    }

    fn get_ready_to_receive_bubble(&self) -> Option<Arc<ReadyToReceiveBubble>> {
        let bubble = self.bubble.read().unwrap();
        bubble.clone()
    }
}

impl Default for MainAppReadyToReceiveManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MainAppReadyToReceiveManager {
    pub fn new() -> Self {
        Self {
            bubble: Arc::new(RwLock::new(None)),
            sub: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_ready_to_receive_subscriber(
        &self,
        sub: Arc<dyn ReadyToReceiveSubscriber>,
    ) {
        self.sub.write().unwrap().replace(sub);
    }
}

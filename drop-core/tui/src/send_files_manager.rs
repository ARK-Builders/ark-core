use std::sync::{Arc, RwLock};

use arkdropx_sender::{SendFilesBubble, SendFilesSubscriber, send_files};

use crate::AppSendFilesManager;

pub struct MainAppSendFilesManager {
    send_files_bubble: Arc<RwLock<Option<Arc<SendFilesBubble>>>>,
    send_files_sub: RwLock<Option<Arc<dyn SendFilesSubscriber>>>,
}

impl AppSendFilesManager for MainAppSendFilesManager {
    fn send_files(&self, req: arkdropx_sender::SendFilesRequest) {
        let send_files_bub = self.send_files_bubble.clone();
        tokio::spawn(async move {
            let bubble = send_files(req).await;
            match bubble {
                Ok(bub) => send_files_bub
                    .write()
                    .unwrap()
                    .replace(Arc::new(bub)),
                Err(_) => todo!(),
            }
        });
    }

    fn get_send_files_bubble(
        &self,
    ) -> Option<std::sync::Arc<arkdropx_sender::SendFilesBubble>> {
        let send_files_bubble = self.send_files_bubble.read().unwrap();
        return send_files_bubble.clone();
    }
}

impl MainAppSendFilesManager {
    pub fn new() -> Self {
        Self {
            send_files_bubble: Arc::new(RwLock::new(None)),
            send_files_sub: RwLock::new(None),
        }
    }

    pub fn set_send_files_subscriber(&self, sub: Arc<dyn SendFilesSubscriber>) {
        self.send_files_sub.write().unwrap().replace(sub);
    }
}

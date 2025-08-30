use std::sync::Arc;

use super::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
use crate::DropError;

pub struct SendFilesRequest {
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
    pub config: Option<SenderConfig>,
}

pub struct SendFilesBubble {
    inner: dropx_sender::SendFilesBubble,
    _runtime: tokio::runtime::Runtime,
}
impl SendFilesBubble {
    pub fn get_ticket(&self) -> String {
        return self.inner.get_ticket();
    }

    pub fn get_confirmation(&self) -> u8 {
        return self.inner.get_confirmation();
    }

    pub async fn cancel(&self) -> Result<(), DropError> {
        return self
            .inner
            .cancel()
            .await
            .map_err(|e| DropError::TODO(e.to_string()));
    }

    pub fn is_finished(&self) -> bool {
        return self.inner.is_finished();
    }

    pub fn is_connected(&self) -> bool {
        return self.inner.is_connected();
    }

    pub fn get_created_at(&self) -> String {
        return self.inner.get_created_at();
    }

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let adapted_subscriber =
            SendFilesSubscriberAdapter { inner: subscriber };
        return self.inner.subscribe(Arc::new(adapted_subscriber));
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let adapted_subscriber =
            SendFilesSubscriberAdapter { inner: subscriber };
        return self
            .inner
            .unsubscribe(Arc::new(adapted_subscriber));
    }
}

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    fn notify_sending(&self, event: SendFilesSendingEvent);
    fn notify_connecting(&self, event: SendFilesConnectingEvent);
}

pub struct SendFilesSendingEvent {
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

pub struct SendFilesConnectingEvent {
    pub receiver: SendFilesProfile,
}

pub struct SendFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

struct SendFilesSubscriberAdapter {
    inner: Arc<dyn SendFilesSubscriber>,
}
impl dropx_sender::SendFilesSubscriber for SendFilesSubscriberAdapter {
    fn get_id(&self) -> String {
        return self.inner.get_id();
    }

    fn log(&self, message: String) {
        return self.inner.log(message);
    }

    fn notify_sending(&self, event: dropx_sender::SendFilesSendingEvent) {
        return self.inner.notify_sending(SendFilesSendingEvent {
            name: event.name,
            sent: event.sent,
            remaining: event.remaining,
        });
    }

    fn notify_connecting(&self, event: dropx_sender::SendFilesConnectingEvent) {
        return self
            .inner
            .notify_connecting(SendFilesConnectingEvent {
                receiver: SendFilesProfile {
                    id: event.receiver.id,
                    name: event.receiver.name,
                    avatar_b64: event.receiver.avatar_b64,
                },
            });
    }
}

pub async fn send_files(
    request: SendFilesRequest,
) -> Result<Arc<SendFilesBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            return dropx_sender::send_files(adapted_request).await;
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    return Ok(Arc::new(SendFilesBubble {
        inner: bubble,
        _runtime: runtime,
    }));
}

fn create_adapted_request(
    request: SendFilesRequest,
) -> dropx_sender::SendFilesRequest {
    let profile = dropx_sender::SenderProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let files = request
        .files
        .into_iter()
        .map(|f| {
            let data = SenderFileDataAdapter { inner: f.data };
            return dropx_sender::SenderFile {
                name: f.name,
                data: Arc::new(data),
            };
        })
        .collect();
    let config = match request.config {
        Some(config) => dropx_sender::SenderConfig {
            chunk_size: config.chunk_size,
            parallel_streams: config.parallel_streams,
        },
        None => dropx_sender::SenderConfig::default(),
    };
    return dropx_sender::SendFilesRequest {
        profile,
        files,
        config,
    };
}

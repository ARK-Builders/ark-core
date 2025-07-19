use std::sync::Arc;

use super::{SenderFile, SenderFileDataAdapter, SenderProfile};
use crate::DropError;

pub struct SendFilesRequest {
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
}

pub struct SendFilesBubble {
    inner: sender::SendFilesBubble,
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

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) -> () {
        let adapted_subscriber = SendFilesSubscriberAdapter { inner: subscriber };
        return self.inner.subscribe(Arc::new(adapted_subscriber));
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) -> () {
        let adapted_subscriber = SendFilesSubscriberAdapter { inner: subscriber };
        return self.inner.unsubscribe(Arc::new(adapted_subscriber));
    }
}

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
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
    pub avatar_b64: Option<String>
}

struct SendFilesSubscriberAdapter {
    inner: Arc<dyn SendFilesSubscriber>,
}
impl sender::SendFilesSubscriber for SendFilesSubscriberAdapter {
    fn get_id(&self) -> String {
        return self.inner.get_id();
    }

    fn notify_sending(&self, event: sender::SendFilesSendingEvent) {
        return self.inner.notify_sending(SendFilesSendingEvent {
            name: event.name,
            sent: event.sent,
            remaining: event.remaining,
        });
    }

    fn notify_connecting(&self, event: sender::SendFilesConnectingEvent) {
        return self.inner.notify_connecting(SendFilesConnectingEvent {
            receiver: SendFilesProfile {
                id: event.receiver.id,
                name: event.receiver.name,
                avatar_b64: event.receiver.avatar_b64
            },
        });
    }
}

pub async fn send_files(request: SendFilesRequest) -> Result<Arc<SendFilesBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            return sender::send_files(adapted_request).await;
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    return Ok(Arc::new(SendFilesBubble {
        inner: bubble,
        _runtime: runtime,
    }));
}

fn create_adapted_request(request: SendFilesRequest) -> sender::SendFilesRequest {
    let profile = sender::SenderProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let files = request
        .files
        .into_iter()
        .map(|f| {
            let data = SenderFileDataAdapter { inner: f.data };
            return sender::SenderFile {
                name: f.name,
                data: Arc::new(data),
            };
        })
        .collect();
    return sender::SendFilesRequest { profile, files };
}

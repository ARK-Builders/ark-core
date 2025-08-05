use std::sync::Arc;

use crate::DropError;

use super::{ReceiverConfig, ReceiverProfile};

pub struct ReceiveFilesRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: ReceiverProfile,
    pub config: Option<ReceiverConfig>,
}

pub struct ReceiveFilesBubble {
    inner: dropx_receiver::ReceiveFilesBubble,
    runtime: tokio::runtime::Runtime,
}
impl ReceiveFilesBubble {
    pub fn start(&self) -> Result<(), DropError> {
        return self
            .runtime
            .block_on(async {
                return self.inner.start();
            })
            .map_err(|e| DropError::TODO(e.to_string()));
    }

    pub fn cancel(&self) {
        return self.inner.cancel();
    }

    pub fn is_finished(&self) -> bool {
        return self.inner.is_finished();
    }

    pub fn is_cancelled(&self) -> bool {
        return self.inner.is_cancelled();
    }

    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let adapted_subscriber =
            ReceiveFilesSubscriberAdapter { inner: subscriber };
        return self.inner.subscribe(Arc::new(adapted_subscriber));
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let adapted_subscriber =
            ReceiveFilesSubscriberAdapter { inner: subscriber };
        return self
            .inner
            .unsubscribe(Arc::new(adapted_subscriber));
    }
}

pub trait ReceiveFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent);
    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent);
}

pub struct ReceiveFilesReceivingEvent {
    pub id: String,
    pub data: Vec<u8>,
}

pub struct ReceiveFilesConnectingEvent {
    pub sender: ReceiveFilesProfile,
    pub files: Vec<ReceiveFilesFile>,
}

pub struct ReceiveFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct ReceiveFilesFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

struct ReceiveFilesSubscriberAdapter {
    inner: Arc<dyn ReceiveFilesSubscriber>,
}
impl dropx_receiver::ReceiveFilesSubscriber for ReceiveFilesSubscriberAdapter {
    fn get_id(&self) -> String {
        return self.inner.get_id();
    }

    fn notify_receiving(
        &self,
        event: dropx_receiver::ReceiveFilesReceivingEvent,
    ) {
        return self
            .inner
            .notify_receiving(ReceiveFilesReceivingEvent {
                id: event.id,
                data: event.data,
            });
    }

    fn notify_connecting(
        &self,
        event: dropx_receiver::ReceiveFilesConnectingEvent,
    ) {
        return self
            .inner
            .notify_connecting(ReceiveFilesConnectingEvent {
                sender: ReceiveFilesProfile {
                    id: event.sender.id,
                    name: event.sender.name,
                    avatar_b64: event.sender.avatar_b64,
                },
                files: event
                    .files
                    .iter()
                    .map(|f| ReceiveFilesFile {
                        id: f.id.clone(),
                        name: f.name.clone(),
                        len: f.len,
                    })
                    .collect(),
            });
    }
}

pub async fn receive_files(
    request: ReceiveFilesRequest,
) -> Result<Arc<ReceiveFilesBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            return dropx_receiver::receive_files(adapted_request).await;
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    return Ok(Arc::new(ReceiveFilesBubble {
        inner: bubble,
        runtime,
    }));
}

fn create_adapted_request(
    request: ReceiveFilesRequest,
) -> dropx_receiver::ReceiveFilesRequest {
    let profile = dropx_receiver::ReceiverProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let config = match request.config {
        Some(config) => dropx_receiver::ReceiverConfig {
            decompression_enabled: config.decompression_enabled,
            buffer_size: config.buffer_size,
            max_concurrent_streams: config.max_concurrent_streams,
        },
        None => dropx_receiver::ReceiverConfig::balanced(),
    };
    return dropx_receiver::ReceiveFilesRequest {
        profile,
        config,
        ticket: request.ticket,
        confirmation: request.confirmation,
    };
}

use std::sync::Arc;

use crate::DropError;

use super::{ReceiverConfig, ReceiverProfile};

/// Request to start a receive session.
///
/// Provide the ticket and confirmation obtained from the sender, the receiver's
/// profile, and optional tuning parameters. If `config` is None, defaults from
/// the lower-level transport will be used.
pub struct ReceiveFilesRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: ReceiverProfile,
    pub config: Option<ReceiverConfig>,
}

/// Handle to a receive session ("bubble").
///
/// Wraps `dropx_receiver::ReceiveFilesBubble` and holds a dedicated Tokio
/// runtime used to drive the session and synchronous `start()`.
pub struct ReceiveFilesBubble {
    inner: dropx_receiver::ReceiveFilesBubble,
    runtime: tokio::runtime::Runtime,
}
impl ReceiveFilesBubble {
    /// Start the session and begin receiving data.
    ///
    /// This method blocks on the internal runtime until setup finishes or an
    /// error is returned. On success, subscribers will receive chunks/events.
    pub fn start(&self) -> Result<(), DropError> {
        self
            .runtime
            .block_on(async {
                self.inner.start()
            })
            .map_err(|e| DropError::TODO(e.to_string()))
    }

    /// Cancel the session. No further progress will occur.
    pub fn cancel(&self) {
        self.inner.cancel()
    }

    /// True when the session has completed (successfully or not).
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// True if the session has been explicitly canceled.
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// Register an observer for logs, chunk payloads, and connection events.
    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let adapted_subscriber =
            ReceiveFilesSubscriberAdapter { inner: subscriber };
        self.inner.subscribe(Arc::new(adapted_subscriber))
    }

    /// Unregister a previously subscribed observer.
    ///
    /// Identity is determined by the subscriber's `get_id()`.
    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let adapted_subscriber =
            ReceiveFilesSubscriberAdapter { inner: subscriber };
        self
            .inner
            .unsubscribe(Arc::new(adapted_subscriber))
    }
}

/// Observer for receive-side logs and events.
///
/// Implementers should provide a stable `get_id()` used for
/// subscribe/unsubscribe identity. `log()` calls are only emitted in debug
/// builds.
pub trait ReceiveFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    /// Emitted for each received chunk of a specific file id.
    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent);
    /// Emitted on connection and when file manifest is known.
    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent);
}

/// A streamed chunk of data for a specific file.
pub struct ReceiveFilesReceivingEvent {
    /// File id this chunk belongs to.
    pub id: String,
    /// Raw bytes of the chunk.
    pub data: Vec<u8>,
}

/// Connection information and file manifest received from the sender.
pub struct ReceiveFilesConnectingEvent {
    pub sender: ReceiveFilesProfile,
    pub files: Vec<ReceiveFilesFile>,
}

/// Sender identity preview available to the receiver.
pub struct ReceiveFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// Information about a single file to be received.
pub struct ReceiveFilesFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

/// Adapter bridging this crate's subscriber trait to the lower-level one.
struct ReceiveFilesSubscriberAdapter {
    inner: Arc<dyn ReceiveFilesSubscriber>,
}
impl dropx_receiver::ReceiveFilesSubscriber for ReceiveFilesSubscriberAdapter {
    fn get_id(&self) -> String {
        self.inner.get_id()
    }

    fn log(&self, message: String) {
        #[cfg(debug_assertions)]
        return self.inner.log(message);
    }

    fn notify_receiving(
        &self,
        event: dropx_receiver::ReceiveFilesReceivingEvent,
    ) {
        self
            .inner
            .notify_receiving(ReceiveFilesReceivingEvent {
                id: event.id,
                data: event.data,
            })
    }

    fn notify_connecting(
        &self,
        event: dropx_receiver::ReceiveFilesConnectingEvent,
    ) {
        self
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
            })
    }
}

/// Start a new receive session and return its bubble.
///
/// Internally creates a dedicated Tokio runtime to drive async operations and
/// performs the initial handshake on that runtime. The caller owns the returned
/// bubble and should retain it for the session lifetime.
pub async fn receive_files(
    request: ReceiveFilesRequest,
) -> Result<Arc<ReceiveFilesBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            dropx_receiver::receive_files(adapted_request).await
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    Ok(Arc::new(ReceiveFilesBubble {
        inner: bubble,
        runtime,
    }))
}

/// Convert the high-level request into the dropx_receiver request format.
///
/// - Copies metadata and session params.
/// - Uses provided config if any, otherwise passes None and relies on defaults.
fn create_adapted_request(
    request: ReceiveFilesRequest,
) -> dropx_receiver::ReceiveFilesRequest {
    let profile = dropx_receiver::ReceiverProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let config = request
        .config
        .map(|c| dropx_receiver::ReceiverConfig {
            chunk_size: c.chunk_size,
            parallel_streams: c.parallel_streams,
        });
    dropx_receiver::ReceiveFilesRequest {
        profile,
        ticket: request.ticket,
        confirmation: request.confirmation,
        config,
    }
}

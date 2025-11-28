use std::sync::Arc;

use super::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
use crate::DropError;

/// Request to start a send session.
///
/// Provide sender metadata, the list of files, and optional tuning parameters.
/// If `config` is None, defaults from the lower-level transport will be used.
pub struct SendFilesRequest {
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
    pub config: Option<SenderConfig>,
}

/// Handle to a running send session ("bubble").
///
/// This type wraps the underlying `arkdropx_sender::SendFilesBubble` and owns a
/// dedicated Tokio runtime to drive async tasks. It exposes status accessors,
/// subscription hooks, and cancellation.
pub struct SendFilesBubble {
    inner: arkdropx_sender::SendFilesBubble,
    _runtime: tokio::runtime::Runtime,
}
impl SendFilesBubble {
    /// Returns the ticket that the receiver must provide to connect.
    pub fn get_ticket(&self) -> String {
        self.inner.get_ticket()
    }

    /// Returns the short confirmation code required during pairing.
    pub fn get_confirmation(&self) -> u8 {
        self.inner.get_confirmation()
    }

    /// Cancel the session asynchronously.
    ///
    /// Errors are mapped into `DropError`. After cancellation, `is_finished()`
    /// will eventually become true.
    pub async fn cancel(&self) -> Result<(), DropError> {
        return self
            .inner
            .cancel()
            .await
            .map_err(|e| DropError::TODO(e.to_string()));
    }

    /// True once all files are sent or the session has been canceled.
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// True once a receiver has connected and handshake has completed.
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// ISO-8601 timestamp for when the session was created.
    pub fn get_created_at(&self) -> String {
        self.inner.get_created_at()
    }

    /// Register an observer for logs and progress/connect events.
    ///
    /// The subscriber is adapted and passed to the underlying transport.
    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let adapted_subscriber =
            SendFilesSubscriberAdapter { inner: subscriber };
        self.inner.subscribe(Arc::new(adapted_subscriber))
    }

    /// Unregister a previously subscribed observer.
    ///
    /// Identity is determined by the subscriber's `get_id()`.
    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let adapted_subscriber =
            SendFilesSubscriberAdapter { inner: subscriber };
        self.inner
            .unsubscribe(Arc::new(adapted_subscriber))
    }
}

/// Observer for send-side logs and events.
///
/// Implementers should provide a stable `get_id()` used for
/// subscribe/unsubscribe identity. `log()` calls are only emitted in debug
/// builds.
pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    /// Periodic progress update while sending a file.
    fn notify_sending(&self, event: SendFilesSendingEvent);
    /// Emitted when attempting to connect to the receiver.
    fn notify_connecting(&self, event: SendFilesConnectingEvent);
}

/// Progress information for a single file being sent.
pub struct SendFilesSendingEvent {
    pub id: String,
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

/// Connection information about the receiver.
pub struct SendFilesConnectingEvent {
    pub receiver: SendFilesProfile,
}

/// Receiver identity preview available to the sender.
pub struct SendFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// Adapter bridging this crate's subscriber trait to the lower-level one.
struct SendFilesSubscriberAdapter {
    inner: Arc<dyn SendFilesSubscriber>,
}
impl arkdropx_sender::SendFilesSubscriber for SendFilesSubscriberAdapter {
    fn get_id(&self) -> String {
        self.inner.get_id()
    }

    fn log(&self, message: String) {
        #[cfg(debug_assertions)]
        return self.inner.log(message.clone());
    }

    fn notify_sending(&self, event: arkdropx_sender::SendFilesSendingEvent) {
        return self.inner.notify_sending(SendFilesSendingEvent {
            id: event.id,
            name: event.name,
            sent: event.sent,
            remaining: event.remaining,
        })
    }

    fn notify_connecting(
        &self,
        event: arkdropx_sender::SendFilesConnectingEvent,
    ) {
        return self
            .inner
            .notify_connecting(SendFilesConnectingEvent {
                receiver: SendFilesProfile {
                    id: event.receiver.id,
                    name: event.receiver.name,
                    avatar_b64: event.receiver.avatar_b64,
                },
            })
    }
}

/// Start a new send session and return its bubble.
///
/// Internally creates a dedicated Tokio runtime to drive async operations.
/// The caller owns the returned bubble and should retain it for the session
/// lifetime. Errors are mapped into `DropError`.
pub async fn send_files(
    request: SendFilesRequest,
) -> Result<Arc<SendFilesBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            return arkdropx_sender::send_files(adapted_request).await;
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    Ok(Arc::new(SendFilesBubble {
        inner: bubble,
        _runtime: runtime,
    }))
}

/// Convert the high-level request into the arkdropx_sender request format.
///
/// - Copies metadata and files.
/// - Wraps `SenderFileData` with an adapter implementing the dropx trait.
/// - Supplies default config if none was provided.
fn create_adapted_request(
    request: SendFilesRequest,
) -> arkdropx_sender::SendFilesRequest {
    let profile = arkdropx_sender::SenderProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let files = request
        .files
        .into_iter()
        .map(|f| {
            let data = SenderFileDataAdapter { inner: f.data };
            return arkdropx_sender::SenderFile {
                name: f.name,
                data: Arc::new(data),
            }
        })
        .collect();
    let config = match request.config {
        Some(config) => arkdropx_sender::SenderConfig {
            chunk_size: config.chunk_size,
            parallel_streams: config.parallel_streams,
        },
        None => arkdropx_sender::SenderConfig::default(),
    };
    return arkdropx_sender::SendFilesRequest {
        profile,
        files,
        config,
    }
}

//! Binding adapter for the send-files-to (QR-to-receive) flow.
//!
//! In this mode, the **receiver** creates a session and displays a QR code.
//! The **sender** scans that QR, connects, and pushes files.

use std::sync::Arc;

use super::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
use crate::DropError;

/// Request to start a send-to session.
///
/// Provide the ticket and confirmation obtained from the receiver's QR code,
/// the sender's profile, the files to send, and optional tuning parameters.
/// If `config` is None, defaults from the lower-level transport will be used.
pub struct SendFilesToRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
    pub config: Option<SenderConfig>,
}

/// Handle to a send-to session ("bubble").
///
/// Wraps `arkdropx_sender::send_files_to::SendFilesToBubble` and holds a
/// dedicated Tokio runtime used to drive the session.
pub struct SendFilesToBubble {
    inner: arkdropx_sender::send_files_to::SendFilesToBubble,
    _runtime: tokio::runtime::Runtime,
}
impl SendFilesToBubble {
    /// Start the transfer.
    ///
    /// This method initiates the handshake and begins sending files.
    /// Returns an error if the session has already been started.
    pub fn start(&self) -> Result<(), DropError> {
        self.inner
            .start()
            .map_err(|e| DropError::TODO(e.to_string()))
    }

    /// Cancel the session. No further progress will occur.
    pub async fn cancel(&self) -> Result<(), DropError> {
        self.inner
            .cancel()
            .await
            .map_err(|e| DropError::TODO(e.to_string()))
    }

    /// True when the session has completed (successfully or not).
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// Register an observer for logs and progress/connect events.
    ///
    /// The subscriber is adapted and passed to the underlying transport.
    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesToSubscriber>) {
        let adapted_subscriber =
            SendFilesToSubscriberAdapter { inner: subscriber };
        self.inner.subscribe(Arc::new(adapted_subscriber))
    }

    /// Unregister a previously subscribed observer.
    ///
    /// Identity is determined by the subscriber's `get_id()`.
    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesToSubscriber>) {
        let adapted_subscriber =
            SendFilesToSubscriberAdapter { inner: subscriber };
        self.inner.unsubscribe(Arc::new(adapted_subscriber))
    }
}

/// Observer for send-to-side logs and events.
///
/// Implementers should provide a stable `get_id()` used for
/// subscribe/unsubscribe identity. `log()` calls are only emitted in debug
/// builds.
pub trait SendFilesToSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    /// Periodic progress update while sending a file.
    fn notify_sending(&self, event: SendFilesToSendingEvent);
    /// Emitted when the receiver connection is established.
    fn notify_connecting(&self, event: SendFilesToConnectingEvent);
}

/// Progress information for a single file being sent.
pub struct SendFilesToSendingEvent {
    pub id: String,
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

/// Connection information about the receiver.
pub struct SendFilesToConnectingEvent {
    pub receiver: SendFilesToReceiverProfile,
}

/// Receiver identity preview available to the sender.
pub struct SendFilesToReceiverProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// Adapter bridging this crate's subscriber trait to the lower-level one.
struct SendFilesToSubscriberAdapter {
    inner: Arc<dyn SendFilesToSubscriber>,
}
impl arkdropx_sender::send_files_to::SendFilesToSubscriber
    for SendFilesToSubscriberAdapter
{
    fn get_id(&self) -> String {
        self.inner.get_id()
    }

    fn log(&self, message: String) {
        #[cfg(debug_assertions)]
        return self.inner.log(message.clone());
    }

    fn notify_sending(
        &self,
        event: arkdropx_sender::send_files_to::SendFilesToSendingEvent,
    ) {
        self.inner.notify_sending(SendFilesToSendingEvent {
            id: event.id,
            name: event.name,
            sent: event.sent,
            remaining: event.remaining,
        })
    }

    fn notify_connecting(
        &self,
        event: arkdropx_sender::send_files_to::SendFilesToConnectingEvent,
    ) {
        self.inner
            .notify_connecting(SendFilesToConnectingEvent {
                receiver: SendFilesToReceiverProfile {
                    id: event.receiver.id,
                    name: event.receiver.name,
                    avatar_b64: event.receiver.avatar_b64,
                },
            })
    }
}

/// Start a new send-to session and return its bubble.
///
/// Internally creates a dedicated Tokio runtime to drive async operations.
/// The caller owns the returned bubble and should retain it for the session
/// lifetime. Errors are mapped into `DropError`.
pub async fn send_files_to(
    request: SendFilesToRequest,
) -> Result<Arc<SendFilesToBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            arkdropx_sender::send_files_to::send_files_to(adapted_request).await
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    Ok(Arc::new(SendFilesToBubble {
        inner: bubble,
        _runtime: runtime,
    }))
}

/// Convert the high-level request into the arkdropx_sender request format.
fn create_adapted_request(
    request: SendFilesToRequest,
) -> arkdropx_sender::send_files_to::SendFilesToRequest {
    let profile = arkdropx_sender::SenderProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let files = request
        .files
        .into_iter()
        .map(|f| {
            let data = SenderFileDataAdapter { inner: f.data };
            arkdropx_sender::SenderFile {
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
    arkdropx_sender::send_files_to::SendFilesToRequest {
        ticket: request.ticket,
        confirmation: request.confirmation,
        profile,
        files,
        config,
    }
}

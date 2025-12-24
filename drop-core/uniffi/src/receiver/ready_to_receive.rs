//! Binding adapter for the ready-to-receive (QR-to-receive) flow.
//!
//! In this mode, the **receiver** creates a session and displays a QR code.
//! The **sender** scans that QR, connects, and pushes files. The receiver
//! waits and receives chunks as they arrive.

use std::sync::Arc;

use super::{ReceiverConfig, ReceiverProfile};
use crate::DropError;

/// Request to start waiting for a sender.
///
/// Provide the receiver's profile and optional tuning parameters.
/// If `config` is None, defaults from the lower-level transport will be used.
pub struct ReadyToReceiveRequest {
    pub profile: ReceiverProfile,
    pub config: Option<ReceiverConfig>,
}

/// Handle to a ready-to-receive session ("bubble").
///
/// Wraps `arkdropx_receiver::ready_to_receive::ReadyToReceiveBubble` and holds
/// a dedicated Tokio runtime used to drive the session.
pub struct ReadyToReceiveBubble {
    inner: arkdropx_receiver::ready_to_receive::ReadyToReceiveBubble,
    _runtime: tokio::runtime::Runtime,
}
impl ReadyToReceiveBubble {
    /// Returns the ticket that the sender must provide to connect.
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
        self.inner
            .cancel()
            .await
            .map_err(|e| DropError::TODO(e.to_string()))
    }

    /// True once the session has completed (all files received or canceled).
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// True once a sender has connected and handshake has completed.
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// ISO-8601 timestamp for when the session was created.
    pub fn get_created_at(&self) -> String {
        self.inner.get_created_at()
    }

    /// Register an observer for logs, chunk payloads, and connection events.
    ///
    /// The subscriber is adapted and passed to the underlying transport.
    pub fn subscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let adapted_subscriber =
            ReadyToReceiveSubscriberAdapter { inner: subscriber };
        self.inner.subscribe(Arc::new(adapted_subscriber))
    }

    /// Unregister a previously subscribed observer.
    ///
    /// Identity is determined by the subscriber's `get_id()`.
    pub fn unsubscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let adapted_subscriber =
            ReadyToReceiveSubscriberAdapter { inner: subscriber };
        self.inner.unsubscribe(Arc::new(adapted_subscriber))
    }
}

/// Observer for ready-to-receive logs and events.
///
/// Implementers should provide a stable `get_id()` used for
/// subscribe/unsubscribe identity. `log()` calls are only emitted in debug
/// builds.
pub trait ReadyToReceiveSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    /// Emitted for each received chunk of a specific file id.
    fn notify_receiving(&self, event: ReadyToReceiveReceivingEvent);
    /// Emitted on connection and when file manifest is known.
    fn notify_connecting(&self, event: ReadyToReceiveConnectingEvent);
}

/// A streamed chunk of data for a specific file.
pub struct ReadyToReceiveReceivingEvent {
    /// File id this chunk belongs to.
    pub id: String,
    /// Raw bytes of the chunk.
    pub data: Vec<u8>,
}

/// Connection information and file manifest received from the sender.
pub struct ReadyToReceiveConnectingEvent {
    pub sender: ReadyToReceiveSenderProfile,
    pub files: Vec<ReadyToReceiveFile>,
}

/// Sender identity preview available to the receiver.
pub struct ReadyToReceiveSenderProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// Information about a single file to be received.
pub struct ReadyToReceiveFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

/// Adapter bridging this crate's subscriber trait to the lower-level one.
struct ReadyToReceiveSubscriberAdapter {
    inner: Arc<dyn ReadyToReceiveSubscriber>,
}
impl arkdropx_receiver::ready_to_receive::ReadyToReceiveSubscriber
    for ReadyToReceiveSubscriberAdapter
{
    fn get_id(&self) -> String {
        self.inner.get_id()
    }

    fn log(&self, message: String) {
        #[cfg(debug_assertions)]
        return self.inner.log(message.clone());
    }

    fn notify_receiving(
        &self,
        event: arkdropx_receiver::ready_to_receive::ReadyToReceiveReceivingEvent,
    ) {
        self.inner
            .notify_receiving(ReadyToReceiveReceivingEvent {
                id: event.id,
                data: event.data,
            })
    }

    fn notify_connecting(
        &self,
        event: arkdropx_receiver::ready_to_receive::ReadyToReceiveConnectingEvent,
    ) {
        self.inner
            .notify_connecting(ReadyToReceiveConnectingEvent {
                sender: ReadyToReceiveSenderProfile {
                    id: event.sender.id,
                    name: event.sender.name,
                    avatar_b64: event.sender.avatar_b64,
                },
                files: event
                    .files
                    .iter()
                    .map(|f| ReadyToReceiveFile {
                        id: f.id.clone(),
                        name: f.name.clone(),
                        len: f.len,
                    })
                    .collect(),
            })
    }
}

/// Start waiting for a sender and return a bubble.
///
/// Internally creates a dedicated Tokio runtime to drive async operations.
/// The caller owns the returned bubble and should retain it for the session
/// lifetime. Display the ticket and confirmation code (e.g., as QR) for the
/// sender to scan.
pub async fn ready_to_receive(
    request: ReadyToReceiveRequest,
) -> Result<Arc<ReadyToReceiveBubble>, DropError> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| DropError::TODO(e.to_string()))?;
    let bubble = runtime
        .block_on(async {
            let adapted_request = create_adapted_request(request);
            arkdropx_receiver::ready_to_receive::ready_to_receive(
                adapted_request,
            )
            .await
        })
        .map_err(|e| DropError::TODO(e.to_string()))?;
    Ok(Arc::new(ReadyToReceiveBubble {
        inner: bubble,
        _runtime: runtime,
    }))
}

/// Convert the high-level request into the arkdropx_receiver request format.
fn create_adapted_request(
    request: ReadyToReceiveRequest,
) -> arkdropx_receiver::ready_to_receive::ReadyToReceiveRequest {
    let profile = arkdropx_receiver::ReceiverProfile {
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
    };
    let config = match request.config {
        Some(config) => arkdropx_receiver::ready_to_receive::ReadyToReceiveConfig {
            chunk_size: config.chunk_size,
            parallel_streams: config.parallel_streams,
        },
        None => arkdropx_receiver::ready_to_receive::ReadyToReceiveConfig::default(),
    };
    arkdropx_receiver::ready_to_receive::ReadyToReceiveRequest { profile, config }
}

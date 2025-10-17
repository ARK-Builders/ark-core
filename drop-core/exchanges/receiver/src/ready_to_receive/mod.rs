//! High-level ready-to-receive operation.
//!
//! This module contains the user-facing entry point `ready_to_receive` and the
//! `ReadyToReceiveBubble` handle returned to the caller. The bubble exposes the
//! ticket and confirmation code, supports cancellation, status queries, and
//! observer subscription for logging and chunk arrivals.

mod handler;

use anyhow::Result;
use chrono::{DateTime, Utc};
use drop_entities::Profile;
use handler::ReadyToReceiveHandler;
use iroh::{Endpoint, Watcher, protocol::Router};
use iroh_base::ticket::NodeTicket;
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

use super::ReceiverProfile;

pub use handler::{
    ReadyToReceiveConnectingEvent, ReadyToReceiveFile,
    ReadyToReceiveReceivingEvent, ReadyToReceiveSenderProfile,
    ReadyToReceiveSubscriber,
};

/// All inputs required to start waiting for a sender.
///
/// Construct this and pass it to [`ready_to_receive`].
pub struct ReadyToReceiveRequest {
    /// Receiver profile data shown to the sender during handshake.
    pub profile: ReceiverProfile,
    /// Preferred receive configuration. Actual values may be negotiated.
    pub config: ReadyToReceiveConfig,
}

/// Tunable settings for waiting to receive files.
///
/// Similar to `ReceiverConfig` but used in the ready-to-receive flow.
#[derive(Clone, Debug)]
pub struct ReadyToReceiveConfig {
    /// Target chunk size in bytes for incoming file projections.
    pub chunk_size: u64,
    /// Number of unidirectional streams to process concurrently.
    pub parallel_streams: u64,
}

impl Default for ReadyToReceiveConfig {
    /// Returns the balanced preset:
    /// - 512 KiB chunks
    /// - 4 parallel streams
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 512, // 512KB chunks
            parallel_streams: 4,    // 4 parallel streams
        }
    }
}

impl ReadyToReceiveConfig {
    /// Preset optimized for higher bandwidth and modern hardware:
    /// - 512 KiB chunks
    /// - 8 parallel streams
    pub fn high_performance() -> Self {
        Self {
            chunk_size: 1024 * 512, // 512KB chunks
            parallel_streams: 8,    // 8 parallel streams
        }
    }

    /// Alias of `Default::default()` returning a balanced configuration.
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Preset tuned for constrained or lossy networks:
    /// - 64 KiB chunks
    /// - 2 parallel streams
    pub fn low_bandwidth() -> Self {
        Self {
            chunk_size: 1024 * 64, // 64KB chunks
            parallel_streams: 2,   // 2 parallel streams
        }
    }
}

/// A waiting receive session.
///
/// Returned by [`ready_to_receive`]. It exposes the ticket and a numeric
/// confirmation code the sender must present to connect. You can subscribe to
/// progress updates, cancel the waiting, and poll the connection state.
pub struct ReadyToReceiveBubble {
    ticket: String,
    confirmation: u8,
    router: Router,
    handler: Arc<ReadyToReceiveHandler>,
    created_at: DateTime<Utc>,
}

impl ReadyToReceiveBubble {
    /// Create a new bubble. Internal use only.
    pub fn new(
        ticket: String,
        confirmation: u8,
        router: Router,
        handler: Arc<ReadyToReceiveHandler>,
    ) -> Self {
        Self {
            ticket,
            confirmation,
            router,
            handler,
            created_at: Utc::now(),
        }
    }

    /// Returns the iroh node ticket used by the sender to dial this receiver.
    pub fn get_ticket(&self) -> String {
        self.ticket.clone()
    }

    /// Returns the confirmation code (0–99) that the sender must echo during
    /// the acceptance flow. Meant to prevent accidental connections.
    pub fn get_confirmation(&self) -> u8 {
        self.confirmation
    }

    /// Asynchronously cancels the waiting, shutting down the router and
    /// preventing any new connections.
    pub async fn cancel(&self) -> Result<()> {
        self.handler
            .log("cancel: Initiating receive wait cancellation".to_string());
        let result = self
            .router
            .shutdown()
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()));

        match &result {
            Ok(_) => {
                self.handler.log(
                    "cancel: Receive wait cancelled successfully".to_string(),
                );
            }
            Err(e) => {
                self.handler
                    .log(format!("cancel: Error during cancellation: {e}"));
            }
        }

        result
    }

    /// Returns true when the router has been shut down or the handler has
    /// finished receiving. If finished, it ensures the router is shut down.
    pub fn is_finished(&self) -> bool {
        let router = self.router.clone();
        let is_router_shutdown = router.is_shutdown();
        let is_handler_finished = self.handler.is_finished();
        let is_finished = is_router_shutdown || is_handler_finished;

        self.handler.log(format!("is_finished: Router shutdown: {is_router_shutdown}, Handler finished: {is_handler_finished}, Overall finished: {is_finished}"));

        if is_finished {
            self.handler.log(
                "is_finished: Transfer is finished, ensuring router shutdown"
                    .to_string(),
            );

            tokio::spawn(async move {
                let _ = router.shutdown().await;
            });
        }

        is_finished
    }

    /// Returns true if a sender has connected and been accepted (i.e.,
    /// the handler has consumed the single allowed connection).
    pub fn is_connected(&self) -> bool {
        let finished = self.is_finished();
        if finished {
            self.handler.log(
                "is_connected: Transfer is finished, returning false"
                    .to_string(),
            );
            return false;
        }

        let consumed = self.handler.is_consumed();
        self.handler
            .log(format!("is_connected: Handler consumed: {consumed}"));

        consumed
    }

    /// Returns the RFC3339 timestamp marking when this bubble was created.
    pub fn get_created_at(&self) -> String {
        self.created_at.to_rfc3339()
    }

    /// Register a subscriber to receive logs and chunk notifications.
    ///
    /// Subscribers must be `Send + Sync`. Duplicate IDs will replace previous
    /// subscribers with the same ID.
    pub fn subscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.handler.log(format!(
            "subscribe: Subscribing new subscriber with ID: {subscriber_id}"
        ));
        self.handler.subscribe(subscriber);
    }

    /// Remove a previously registered subscriber.
    pub fn unsubscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.handler.log(format!(
            "unsubscribe: Unsubscribing subscriber with ID: {subscriber_id}"
        ));
        self.handler.unsubscribe(subscriber);
    }
}

/// Starts waiting for a sender and returns a [`ReadyToReceiveBubble`] handle.
///
/// The function:
/// - Builds an iroh endpoint with discovery enabled.
/// - Generates a random human-check confirmation code (0–99).
/// - Spawns a protocol router that accepts exactly one sender matching the
///   confirmation code.
/// - Returns the ticket and handle used to monitor or cancel the waiting.
///
/// Errors if the endpoint fails to bind or the router cannot be spawned.
///
/// Example:
/// ```rust no_run
/// use std::sync::Arc;
/// use dropx_receiver::{
///     ready_to_receive::*, ReceiverProfile,
/// };
///
/// struct Logger;
/// impl ReadyToReceiveSubscriber for Logger {
///     fn get_id(&self) -> String { "logger".into() }
///     fn log(&self, msg: String) { println!("[log] {msg}"); }
///     fn notify_receiving(&self, e: ReadyToReceiveReceivingEvent) {
///         println!("chunk for {}: {} bytes", e.id, e.data.len());
///     }
///     fn notify_connecting(&self, e: ReadyToReceiveConnectingEvent) {
///         println!("sender: {}, files: {}", e.sender.name, e.files.len());
///     }
/// }
///
/// # async fn run() -> anyhow::Result<()> {
/// let bubble = ready_to_receive(ReadyToReceiveRequest {
///     profile: ReceiverProfile { name: "Receiver".into(), avatar_b64: None },
///     config: ReadyToReceiveConfig::balanced(),
/// }).await?;
///
/// bubble.subscribe(Arc::new(Logger));
/// println!("Ticket: {}", bubble.get_ticket());
/// println!("Confirmation: {}", bubble.get_confirmation());
///
/// // ... wait for sender connection and file reception ...
/// # Ok(())
/// # }
/// ```
pub async fn ready_to_receive(
    request: ReadyToReceiveRequest,
) -> Result<ReadyToReceiveBubble> {
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: request.profile.name.clone(),
        avatar_b64: request.profile.avatar_b64.clone(),
    };

    let handler =
        Arc::new(ReadyToReceiveHandler::new(profile, request.config.clone()));

    handler.log(
        "ready_to_receive: Starting receive wait initialization".to_string(),
    );
    handler.log(format!(
        "ready_to_receive: Chunk size configuration: {} bytes",
        request.config.chunk_size
    ));

    handler.log(
        "ready_to_receive: Creating endpoint builder with discovery_n0"
            .to_string(),
    );
    let endpoint_builder = Endpoint::builder().discovery_n0();

    handler.log("ready_to_receive: Binding endpoint".to_string());
    let endpoint = endpoint_builder.bind().await?;
    handler.log("ready_to_receive: Endpoint bound successfully".to_string());

    handler.log("ready_to_receive: Initializing node address".to_string());
    let node_addr = endpoint.node_addr().initialized().await;
    handler.log(format!(
        "ready_to_receive: Node address initialized: {node_addr:?}"
    ));

    handler.log(
        "ready_to_receive: Generating random confirmation code".to_string(),
    );
    let confirmation: u8 = rand::rng().random_range(0..=99);
    handler.log(format!(
        "ready_to_receive: Generated confirmation code: {confirmation}"
    ));

    handler.log("ready_to_receive: Creating router with handler".to_string());
    let router = Router::builder(endpoint)
        .accept([confirmation], handler.clone())
        .spawn();
    handler.log(
        "ready_to_receive: Router created and spawned successfully".to_string(),
    );

    let ticket = NodeTicket::new(node_addr).to_string();
    handler.log(format!("ready_to_receive: Generated ticket: {ticket}"));
    handler.log(
        "ready_to_receive: Receive wait initialization completed successfully"
            .to_string(),
    );

    Ok(ReadyToReceiveBubble::new(
        ticket,
        confirmation,
        router,
        handler,
    ))
}

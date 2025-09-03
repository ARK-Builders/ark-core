//! High-level send operation.
//!
//! This module contains the user-facing entry point `send_files` and the
//! `SendFilesBubble` handle returned to the caller. The bubble exposes the
//! ticket and confirmation code, supports cancellation, status queries, and
//! observer subscription for logging and progress updates.

mod handler;

use crate::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
use anyhow::Result;
use arkdrop_entities::{File, Profile};
use chrono::{DateTime, Utc};
use handler::SendFilesHandler;
use iroh::{Endpoint, Watcher, protocol::Router};
use iroh_base::ticket::NodeTicket;
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

pub use handler::{
    SendFilesConnectingEvent, SendFilesSendingEvent, SendFilesSubscriber,
};

/// All inputs required to start a file transfer.
///
/// Construct this and pass it to [`send_files`].
pub struct SendFilesRequest {
    /// Sender profile data shown to the receiver during handshake.
    pub profile: SenderProfile,
    /// Files to transfer. Each file must provide a `SenderFileData` source.
    pub files: Vec<SenderFile>,
    /// Preferred transfer configuration. Actual values may be negotiated.
    pub config: SenderConfig,
}

/// A running file transfer session.
///
/// Returned by [`send_files`]. It exposes the ticket and a numeric confirmation
/// code the receiver must present to connect. You can subscribe to progress
/// updates, cancel the transfer, and poll the connection state.
pub struct SendFilesBubble {
    ticket: String,
    confirmation: u8,
    router: Router,
    handler: Arc<SendFilesHandler>,
    created_at: DateTime<Utc>,
}
impl SendFilesBubble {
    /// Create a new bubble. Internal use only.
    pub fn new(
        ticket: String,
        confirmation: u8,
        router: Router,
        handler: Arc<SendFilesHandler>,
    ) -> Self {
        Self {
            ticket,
            confirmation,
            router,
            handler,
            created_at: Utc::now(),
        }
    }

    /// Returns the iroh node ticket used by the receiver to dial this sender.
    pub fn get_ticket(&self) -> String {
        self.ticket.clone()
    }

    /// Returns the confirmation code (0–99) that the receiver must echo during
    /// the acceptance flow. Meant to prevent accidental connections.
    pub fn get_confirmation(&self) -> u8 {
        self.confirmation
    }

    /// Asynchronously cancels the transfer, shutting down the router and
    /// preventing any new connections.
    pub async fn cancel(&self) -> Result<()> {
        self.handler
            .log("cancel: Initiating file transfer cancellation".to_string());
        let result = self
            .router
            .shutdown()
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()));

        match &result {
            Ok(_) => {
                self.handler.log(
                    "cancel: File transfer cancelled successfully".to_string(),
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
    /// finished sending. If finished, it ensures the router is shut down.
    pub fn is_finished(&self) -> bool {
        let router = self.router.clone();
        let router_shutdown = router.is_shutdown();
        let handler_finished = self.handler.is_finished();
        let is_finished = router_shutdown || handler_finished;

        self.handler.log(format!("is_finished: Router shutdown: {router_shutdown}, Handler finished: {handler_finished}, Overall finished: {is_finished}"));

        self.handler.log(
            "is_finished: Transfer is finished, ensuring router shutdown"
                .to_string(),
        );

        tokio::spawn(async move {
            let _ = router.shutdown().await;
        });

        is_finished
    }

    /// Returns true if a receiver has connected and been accepted (i.e.,
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

    /// Register a subscriber to receive logs and progress notifications.
    ///
    /// Subscribers must be `Send + Sync`. Duplicate IDs will replace previous
    /// subscribers with the same ID.
    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.handler.log(format!(
            "subscribe: Subscribing new subscriber with ID: {subscriber_id}"
        ));
        self.handler.subscribe(subscriber);
    }

    /// Remove a previously registered subscriber.
    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.handler.log(format!(
            "unsubscribe: Unsubscribing subscriber with ID: {subscriber_id}"
        ));
        self.handler.unsubscribe(subscriber);
    }
}

/// Starts a new file transfer and returns a [`SendFilesBubble`] handle.
///
/// The function:
/// - Builds an iroh endpoint with discovery enabled.
/// - Generates a random human-check confirmation code (0–99).
/// - Spawns a protocol router that accepts exactly one receiver matching the
///   confirmation code.
/// - Returns the ticket and handle used to monitor or cancel the transfer.
///
/// Errors if the endpoint fails to bind or the router cannot be spawned.
pub async fn send_files(request: SendFilesRequest) -> Result<SendFilesBubble> {
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: request.profile.name.clone(),
        avatar_b64: request.profile.avatar_b64.clone(),
    };

    let files: Vec<File> = request
        .files
        .into_iter()
        .map(|f| {
            let data = SenderFileDataAdapter { inner: f.data };
            File {
                id: Uuid::new_v4().to_string(),
                name: f.name,
                data: Arc::new(data),
            }
        })
        .collect();

    let files_len = files.len();
    let handler = Arc::new(SendFilesHandler::new(
        profile,
        files,
        request.config.clone(),
    ));

    handler.log(format!(
        "send_files: Starting file transfer initialization with {files_len} files"
    ));
    handler.log(format!(
        "send_files: Chunk size configuration: {} bytes",
        request.config.chunk_size
    ));

    handler.log(
        "send_files: Creating endpoint builder with discovery_n0".to_string(),
    );
    let endpoint_builder = Endpoint::builder().discovery_n0();

    handler.log("send_files: Binding endpoint".to_string());
    let endpoint = endpoint_builder.bind().await?;
    handler.log("send_files: Endpoint bound successfully".to_string());

    handler.log("send_files: Initializing node address".to_string());
    let node_addr = endpoint.node_addr().initialized().await;
    handler.log(format!(
        "send_files: Node address initialized: {node_addr:?}"
    ));

    handler.log("send_files: Generating random confirmation code".to_string());
    let confirmation: u8 = rand::rng().random_range(0..=99);
    handler.log(format!(
        "send_files: Generated confirmation code: {confirmation}"
    ));

    handler.log("send_files: Creating router with handler".to_string());
    let router = Router::builder(endpoint)
        .accept([confirmation], handler.clone())
        .spawn();
    handler
        .log("send_files: Router created and spawned successfully".to_string());

    let ticket = NodeTicket::new(node_addr).to_string();
    handler.log(format!("send_files: Generated ticket: {ticket}"));
    handler.log(
        "send_files: File transfer initialization completed successfully"
            .to_string(),
    );

    Ok(SendFilesBubble::new(ticket, confirmation, router, handler))
}

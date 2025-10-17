//! Internal protocol handler for waiting to receive files.
//!
//! This module implements `iroh::protocol::ProtocolHandler` to accept a single
//! sender, exchange handshakes, negotiate configuration, and receive file data
//! using unidirectional streams. It provides an observer API via
//! `ReadyToReceiveSubscriber` to report logs, connection metadata, and per-file
//! chunk arrivals.

use anyhow::Result;
use drop_entities::Profile;
use dropx_common::{
    handshake::{
        HandshakeConfig, HandshakeProfile, NegotiatedConfig, ReceiverHandshake,
        SenderHandshake,
    },
    projection::FileProjection,
};
use futures::Future;
use iroh::{
    endpoint::{Connection, RecvStream, SendStream, VarInt},
    protocol::ProtocolHandler,
};
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock, atomic::AtomicBool},
};
use tokio::task::JoinSet;

use super::ReadyToReceiveConfig;

/// Observer interface for transfer logs and progress.
///
/// Implementors must be thread-safe (`Send + Sync`) since notifications are
/// dispatched from async tasks.
pub trait ReadyToReceiveSubscriber: Send + Sync {
    /// A stable unique identifier for this subscriber (used as a map key).
    fn get_id(&self) -> String;

    /// Receives diagnostic log lines from the transfer pipeline.
    fn log(&self, message: String);

    /// Receives chunk data for each file being received.
    ///
    /// Multiple events can arrive out of order across files.
    fn notify_receiving(&self, event: ReadyToReceiveReceivingEvent);

    /// Notified when a sender connects and completes the handshake.
    fn notify_connecting(&self, event: ReadyToReceiveConnectingEvent);
}

/// Per-chunk receiving event.
///
/// Contains the file ID and raw chunk data.
#[derive(Clone)]
pub struct ReadyToReceiveReceivingEvent {
    pub id: String,
    pub data: Vec<u8>,
}

/// Connection event carrying the sender's profile and files list as reported
/// during handshake.
pub struct ReadyToReceiveConnectingEvent {
    pub sender: ReadyToReceiveSenderProfile,
    pub files: Vec<ReadyToReceiveFile>,
}

/// Sender profile details surfaced to subscribers.
#[derive(Clone)]
pub struct ReadyToReceiveSenderProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// File information provided by sender during handshake.
#[derive(Clone)]
pub struct ReadyToReceiveFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

/// Protocol handler responsible for accepting a single sender and receiving
/// data.
///
/// A `ReadyToReceiveHandler`:
/// - Enforces single-consumption of the incoming connection.
/// - Performs JSON-based handshake exchange.
/// - Negotiates chunking and concurrency parameters.
/// - Receives files over unidirectional streams.
/// - Emits events to registered subscribers.
pub struct ReadyToReceiveHandler {
    is_consumed: AtomicBool,
    is_finished: Arc<AtomicBool>,
    profile: Profile,
    config: ReadyToReceiveConfig,
    subscribers:
        Arc<RwLock<HashMap<String, Arc<dyn ReadyToReceiveSubscriber>>>>,
}
impl Debug for ReadyToReceiveHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadyToReceiveHandler")
            .field("is_consumed", &self.is_consumed)
            .field("is_finished", &self.is_finished)
            .field("profile", &self.profile)
            .field("config", &self.config)
            .finish()
    }
}
impl ReadyToReceiveHandler {
    /// Constructs a new handler for the given profile and configuration.
    pub fn new(profile: Profile, config: ReadyToReceiveConfig) -> Self {
        Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            config,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns true if a connection has already been accepted.
    ///
    /// This handler accepts at most one sender for a bubble.
    pub fn is_consumed(&self) -> bool {
        let consumed = self
            .is_consumed
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_consumed check: {consumed}"));
        consumed
    }

    /// Returns true if the transfer has finished or the handler has been shut
    /// down.
    pub fn is_finished(&self) -> bool {
        let finished = self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_finished check: {finished}"));
        finished
    }

    /// Broadcasts a log message to all subscribers.
    pub fn log(&self, message: String) {
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, subscriber)| {
                subscriber.log(message.clone());
            });
    }

    /// Registers a new subscriber or replaces an existing one with the same
    /// ID.
    pub fn subscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "Subscribing new subscriber with ID: {subscriber_id}"
        ));

        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber_id.clone(), subscriber);

        self.log(format!(
            "Subscriber {} successfully subscribed. Total subscribers: {}",
            subscriber_id,
            self.subscribers.read().unwrap().len()
        ));
    }

    /// Unregisters a subscriber by its ID.
    pub fn unsubscribe(&self, subscriber: Arc<dyn ReadyToReceiveSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!("Unsubscribing subscriber with ID: {subscriber_id}"));

        let removed = self
            .subscribers
            .write()
            .unwrap()
            .remove(&subscriber_id);

        if removed.is_some() {
            self.log(format!("Subscriber {subscriber_id} successfully unsubscribed. Remaining subscribers: {}", self.subscribers.read().unwrap().len()));
        } else {
            self.log(format!(
                "Subscriber {subscriber_id} was not found during unsubscribe operation"
            ));
        }
    }
}
impl ProtocolHandler for ReadyToReceiveHandler {
    fn on_connecting(
        &self,
        connecting: iroh::endpoint::Connecting,
    ) -> impl Future<
        Output = std::result::Result<Connection, iroh::protocol::AcceptError>,
    > + Send {
        self.log("on_connecting: New connection attempt received".to_string());

        let is_consumed = self
            .is_consumed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Relaxed,
            )
            .unwrap_or(true);

        async move {
            if is_consumed {
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            let connection = connecting.await?;
            Ok(connection)
        }
    }

    fn shutdown(&self) -> impl Future<Output = ()> + Send {
        self.log("shutdown: Initiating handler shutdown".to_string());
        let is_finished = self.is_finished.clone();

        async move {
            is_finished.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn accept(
        &self,
        connection: Connection,
    ) -> impl Future<
        Output = std::result::Result<(), iroh::protocol::AcceptError>,
    > + Send {
        self.log("accept: Creating carrier for file reception".to_string());

        let carrier = Carrier {
            is_finished: self.is_finished.clone(),
            config: self.config.clone(),
            negotiated_config: None,
            profile: self.profile.clone(),
            connection,
            subscribers: self.subscribers.clone(),
        };

        async move {
            let mut carrier = carrier;
            if (carrier.greet().await).is_err() {
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            if (carrier.receive_files().await).is_err() {
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            carrier.finish();
            Ok(())
        }
    }
}

/// Helper that performs handshake, configuration negotiation, and streaming.
///
/// Not exposed publicly; used internally by `ReadyToReceiveHandler`.
struct Carrier {
    is_finished: Arc<AtomicBool>,
    config: ReadyToReceiveConfig,
    negotiated_config: Option<NegotiatedConfig>,
    profile: Profile,
    connection: Connection,
    subscribers:
        Arc<RwLock<HashMap<String, Arc<dyn ReadyToReceiveSubscriber>>>>,
}
impl Carrier {
    /// Performs the bidirectional handshake exchange and notifies subscribers
    /// about the sender identity and files.
    async fn greet(&mut self) -> Result<()> {
        let mut bi = self.connection.accept_bi().await?;

        self.receive_handshake(&mut bi).await?;
        self.send_handshake(&mut bi).await?;

        bi.0.stopped().await?;

        self.log("greet: Handshake completed successfully".to_string());
        Ok(())
    }

    /// Receives the sender handshake and computes the negotiated
    /// configuration.
    async fn receive_handshake(
        &mut self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        let mut header = [0u8; 4];
        bi.1.read_exact(&mut header).await?;
        let len = u32::from_be_bytes(header);

        let mut buffer = vec![0u8; len as usize];
        bi.1.read_exact(&mut buffer).await?;

        let handshake: SenderHandshake = serde_json::from_slice(&buffer)?;

        // Negotiate configuration
        let receiver_config = HandshakeConfig {
            chunk_size: self.config.chunk_size,
            parallel_streams: self.config.parallel_streams,
        };

        self.negotiated_config = Some(NegotiatedConfig::negotiate(
            &handshake.config,
            &receiver_config,
        ));

        // Prepare data structures
        let profile = ReadyToReceiveSenderProfile {
            id: handshake.profile.id,
            name: handshake.profile.name,
            avatar_b64: handshake.profile.avatar_b64,
        };

        let files: Vec<ReadyToReceiveFile> = handshake
            .files
            .into_iter()
            .map(|f| ReadyToReceiveFile {
                id: f.id,
                name: f.name,
                len: f.len,
            })
            .collect();

        // Notify subscribers
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(ReadyToReceiveConnectingEvent {
                    sender: profile.clone(),
                    files: files.clone(),
                });
            });

        Ok(())
    }

    /// Sends the receiver's profile and preferred configuration.
    async fn send_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        let handshake = ReceiverHandshake {
            profile: HandshakeProfile {
                id: self.profile.id.clone(),
                name: self.profile.name.clone(),
                avatar_b64: self.profile.avatar_b64.clone(),
            },
            config: HandshakeConfig {
                chunk_size: self.config.chunk_size,
                parallel_streams: self.config.parallel_streams,
            },
        };

        // Pre-allocate vector with estimated capacity
        let mut buffer = Vec::with_capacity(256);
        serde_json::to_writer(&mut buffer, &handshake)?;

        let len_bytes = (buffer.len() as u32).to_be_bytes();

        // Single write operation
        let mut combined = Vec::with_capacity(4 + buffer.len());
        combined.extend_from_slice(&len_bytes);
        combined.extend_from_slice(&buffer);

        bi.0.write_all(&combined).await?;
        Ok(())
    }

    /// Receives all files using unidirectional streams and the negotiated
    /// settings.
    async fn receive_files(&self) -> Result<()> {
        let mut join_set = JoinSet::new();

        // Use negotiated configuration or fallback to defaults
        let (chunk_size, parallel_streams) =
            if let Some(config) = &self.negotiated_config {
                (config.chunk_size, config.parallel_streams)
            } else {
                (self.config.chunk_size, self.config.parallel_streams)
            };

        let expected_close = iroh::endpoint::ConnectionError::ApplicationClosed(
            iroh::endpoint::ApplicationClose {
                error_code: VarInt::from_u32(200),
                reason: "finished".into(),
            },
        );

        'files_iterator: loop {
            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();

            join_set.spawn(async move {
                Self::receive_single_file(chunk_size, connection, subscribers)
                    .await
            });

            // Limit concurrent streams to negotiated number
            while join_set.len() >= parallel_streams as usize {
                if let Some(result) = join_set.join_next().await
                    && let Err(err) = result?
                {
                    // Check for expected close
                    if let Some(connection_err) =
                        err.downcast_ref::<iroh::endpoint::ConnectionError>()
                        && connection_err == &expected_close
                    {
                        break 'files_iterator;
                    }
                    self.log(format!("receive_files: Stream failed: {err}"));
                    return Err(err);
                }
            }
        }

        // Wait for all remaining streams to complete
        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result? {
                if let Some(connection_err) =
                    err.downcast_ref::<iroh::endpoint::ConnectionError>()
                    && connection_err == &expected_close
                {
                    continue;
                }
                self.log(format!("receive_single_file: Stream failed: {err}"));
                return Err(err);
            }
        }

        self.log("receive_files: All files received successfully".to_string());
        Ok(())
    }

    /// Receives a single file in JSON-framed chunks:
    /// - 4-byte big-endian length header
    /// - JSON payload containing `FileProjection { id, data }`
    async fn receive_single_file(
        chunk_size: u64,
        connection: Connection,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReadyToReceiveSubscriber>>>,
        >,
    ) -> Result<()> {
        let mut uni = connection.accept_uni().await?;

        let mut buffer =
            Vec::with_capacity((chunk_size + 256 * 1024).try_into().unwrap());

        loop {
            buffer.clear();

            let len =
                match Self::read_serialized_projection_len(&mut uni).await? {
                    Some(l) => l,
                    None => break, // Stream finished
                };

            buffer.resize(len, 0);

            uni.read_exact(&mut buffer).await?;

            let projection: FileProjection = serde_json::from_slice(&buffer)?;

            // Notify subscribers about received chunk
            let event = ReadyToReceiveReceivingEvent {
                id: projection.id,
                data: projection.data,
            };

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(_, s)| {
                    s.notify_receiving(event.clone());
                });
        }

        Ok(())
    }

    /// Read a 4-byte big-endian length prefix from a unidirectional stream.
    ///
    /// Returns:
    /// - `Ok(Some(len))` when a length was read,
    /// - `Ok(None)` if the stream has finished normally,
    /// - `Err(e)` for I/O errors.
    async fn read_serialized_projection_len(
        uni: &mut RecvStream,
    ) -> Result<Option<usize>> {
        let mut header = [0u8; 4];

        match uni.read_exact(&mut header).await {
            Ok(()) => {
                let len = u32::from_be_bytes(header) as usize;
                Ok(Some(len))
            }
            Err(e) => {
                use iroh::endpoint::ReadExactError;
                match e {
                    ReadExactError::FinishedEarly(_) => Ok(None),
                    ReadExactError::ReadError(io_error) => Err(io_error.into()),
                }
            }
        }
    }

    /// Marks the handler as finished and closes the connection with a code and
    /// reason.
    fn finish(&self) {
        self.log("finish: Starting transfer finish process".to_string());

        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.log("finish: Transfer finished flag set to true".to_string());

        self.log("finish: Connection closed".to_string());
        self.connection
            .close(VarInt::from_u32(200), "finished".as_bytes());

        self.log("finish: Transfer process completed successfully".to_string());
    }

    /// Internal logger that prefixes subscriber IDs.
    fn log(&self, message: String) {
        self.subscribers.read().unwrap().iter().for_each(
            |(_id, subscriber)| {
                subscriber.log(message.clone());
            },
        );
    }
}

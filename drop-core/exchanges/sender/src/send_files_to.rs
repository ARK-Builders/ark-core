//! Send files to a waiting receiver.
//!
//! This module provides the `send_files_to` function which connects to a
//! receiver's ticket (from ready_to_receive) and sends files. This is the
//! complement to the receiver's ready_to_receive flow.

use crate::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
use anyhow::Result;
use drop_entities::{File, Profile};
use dropx_common::{
    handshake::{
        HandshakeConfig, HandshakeFile, HandshakeProfile, NegotiatedConfig,
        ReceiverHandshake, SenderHandshake,
    },
    projection::FileProjection,
};
use iroh::{
    Endpoint,
    endpoint::{Connection, RecvStream, SendStream, VarInt},
};
use iroh_base::ticket::NodeTicket;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, atomic::AtomicBool},
};
use tokio::task::JoinSet;
use uuid::Uuid;

/// All inputs required to send files to a waiting receiver.
///
/// Construct this and pass it to [`send_files_to`].
pub struct SendFilesToRequest {
    /// Receiver's ticket (obtained from their QR code or directly).
    pub ticket: String,
    /// Receiver's confirmation code (0–99).
    pub confirmation: u8,
    /// Sender profile data shown to the receiver during handshake.
    pub profile: SenderProfile,
    /// Files to transfer. Each file must provide a `SenderFileData` source.
    pub files: Vec<SenderFile>,
    /// Preferred transfer configuration. Actual values may be negotiated.
    pub config: SenderConfig,
}

/// A running send-to-receiver session.
///
/// Returned by [`send_files_to`]. You can subscribe to progress updates and
/// poll the connection state.
pub struct SendFilesToBubble {
    endpoint: Endpoint,
    connection: Connection,
    profile: Profile,
    files: Vec<File>,
    config: SenderConfig,
    is_running: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesToSubscriber>>>>,
}

impl SendFilesToBubble {
    /// Create a new bubble. Internal use only.
    pub fn new(
        endpoint: Endpoint,
        connection: Connection,
        profile: Profile,
        files: Vec<File>,
        config: SenderConfig,
    ) -> Self {
        Self {
            endpoint,
            connection,
            profile,
            files,
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            is_finished: Arc::new(AtomicBool::new(false)),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the send-to-receiver transfer asynchronously.
    ///
    /// - Performs handshake, then begins sending file data.
    /// - Returns an error if the bubble has already been started.
    /// - Progress is published to subscribers.
    pub fn start(&self) -> Result<()> {
        self.log("start: Checking if transfer can be started".to_string());

        let is_running = self
            .is_running
            .load(std::sync::atomic::Ordering::Acquire);

        if is_running {
            self.log(
                "start: Cannot start transfer, already running".to_string(),
            );
            return Err(anyhow::Error::msg("Already running."));
        }

        self.log("start: Setting running flag".to_string());
        self.is_running
            .store(true, std::sync::atomic::Ordering::Release);

        self.log("start: Creating carrier for file sending".to_string());
        let carrier = Carrier {
            profile: self.profile.clone(),
            config: self.config.clone(),
            negotiated_config: None,
            connection: self.connection.clone(),
            files: self.files.clone(),
            is_finished: self.is_finished.clone(),
            subscribers: self.subscribers.clone(),
        };

        self.log("start: Spawning async task for file sending".to_string());
        let endpoint = self.endpoint.clone();
        tokio::spawn(async move {
            let mut carrier = carrier;
            if let Err(e) = carrier.greet().await {
                carrier.log(format!("start: Handshake failed: {e}"));
                carrier.finish(&endpoint).await;
                return;
            }

            let result = carrier.send_files().await;
            if let Err(e) = result {
                carrier.log(format!("start: File sending failed: {e}"));
            } else {
                carrier.log(
                    "start: File sending completed successfully".to_string(),
                );
            }

            carrier.finish(&endpoint).await;
        });

        Ok(())
    }

    /// Returns `true` when the session has completed cleanup.
    pub fn is_finished(&self) -> bool {
        let finished = self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_finished check: {finished}"));
        finished
    }

    /// Register a subscriber to receive log and progress events.
    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesToSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "subscribe: Subscribing new subscriber with ID: {subscriber_id}"
        ));

        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber_id.clone(), subscriber);

        self.log(format!("subscribe: Subscriber {subscriber_id} successfully subscribed. Total subscribers: {}", self.subscribers.read().unwrap().len()));
    }

    /// Remove a previously registered subscriber.
    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesToSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "unsubscribe: Unsubscribing subscriber with ID: {subscriber_id}"
        ));

        let removed = self
            .subscribers
            .write()
            .unwrap()
            .remove(&subscriber_id);

        if removed.is_some() {
            self.log(format!("unsubscribe: Subscriber {subscriber_id} successfully unsubscribed. Remaining subscribers: {}", self.subscribers.read().unwrap().len()));
        } else {
            self.log(format!("unsubscribe: Subscriber {subscriber_id} was not found during unsubscribe operation"));
        }
    }

    fn log(&self, message: String) {
        self.subscribers.read().unwrap().iter().for_each(
            |(_id, subscriber)| {
                subscriber.log(message.clone());
            },
        );
    }
}

/// Subscriber interface for observing send-to-receiver transfer.
pub trait SendFilesToSubscriber: Send + Sync {
    /// Stable identifier for this subscriber (used as a map key).
    fn get_id(&self) -> String;
    /// Receive diagnostic log messages.
    fn log(&self, message: String);
    /// Receive progress updates for each file being sent.
    fn notify_sending(&self, event: SendFilesToSendingEvent);
    /// Notified when receiver connection is established.
    fn notify_connecting(&self, event: SendFilesToConnectingEvent);
}

/// Per-file progress event.
#[derive(Clone)]
pub struct SendFilesToSendingEvent {
    pub id: String,
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

/// Connection event carrying the receiver's profile.
pub struct SendFilesToConnectingEvent {
    pub receiver: SendFilesToReceiverProfile,
}

/// Receiver profile details surfaced to subscribers.
#[derive(Clone)]
pub struct SendFilesToReceiverProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

/// Helper that performs handshake, configuration negotiation, and streaming.
struct Carrier {
    profile: Profile,
    config: SenderConfig,
    negotiated_config: Option<NegotiatedConfig>,
    connection: Connection,
    files: Vec<File>,
    is_finished: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesToSubscriber>>>>,
}

impl Carrier {
    /// Performs the bidirectional handshake exchange.
    async fn greet(&mut self) -> Result<()> {
        let mut bi = self.connection.open_bi().await?;

        self.send_handshake(&mut bi).await?;
        self.receive_handshake(&mut bi).await?;

        bi.0.finish()?;
        bi.1.stop(VarInt::from_u32(0))?;

        self.log("greet: Handshake completed successfully".to_string());
        Ok(())
    }

    /// Sends the sender's profile, file list, and preferred configuration.
    async fn send_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        let handshake = SenderHandshake {
            profile: HandshakeProfile {
                id: self.profile.id.clone(),
                name: self.profile.name.clone(),
                avatar_b64: self.profile.avatar_b64.clone(),
            },
            files: self
                .files
                .iter()
                .map(|f| HandshakeFile {
                    id: f.id.clone(),
                    name: f.name.clone(),
                    len: f.data.len(),
                })
                .collect(),
            config: HandshakeConfig {
                chunk_size: self.config.chunk_size,
                parallel_streams: self.config.parallel_streams,
            },
        };

        let mut buffer = Vec::with_capacity(512);
        serde_json::to_writer(&mut buffer, &handshake)?;

        let len_bytes = (buffer.len() as u32).to_be_bytes();

        let mut combined = Vec::with_capacity(4 + buffer.len());
        combined.extend_from_slice(&len_bytes);
        combined.extend_from_slice(&buffer);

        bi.0.write_all(&combined).await?;
        Ok(())
    }

    /// Receives the receiver handshake and computes the negotiated
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

        let handshake: ReceiverHandshake = serde_json::from_slice(&buffer)?;

        // Negotiate configuration
        let sender_config = HandshakeConfig {
            chunk_size: self.config.chunk_size,
            parallel_streams: self.config.parallel_streams,
        };

        self.negotiated_config = Some(NegotiatedConfig::negotiate(
            &sender_config,
            &handshake.config,
        ));

        // Notify subscribers
        let profile = SendFilesToReceiverProfile {
            id: handshake.profile.id,
            name: handshake.profile.name,
            avatar_b64: handshake.profile.avatar_b64,
        };

        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(SendFilesToConnectingEvent {
                    receiver: profile.clone(),
                });
            });

        Ok(())
    }

    /// Streams all files using unidirectional streams.
    async fn send_files(&self) -> Result<()> {
        let mut join_set = JoinSet::new();

        let (chunk_size, parallel_streams) =
            if let Some(config) = &self.negotiated_config {
                (config.chunk_size, config.parallel_streams)
            } else {
                (self.config.chunk_size, self.config.parallel_streams)
            };

        for file in self.files.clone() {
            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();

            join_set.spawn(async move {
                Self::send_single_file(
                    &file,
                    chunk_size,
                    connection,
                    subscribers,
                )
                .await
            });

            if join_set.len() >= parallel_streams as usize
                && let Some(result) = join_set.join_next().await
                && let Err(err) = result?
            {
                self.log(format!("send_files: Stream failed: {err}"));
                return Err(err);
            }
        }

        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result? {
                self.log(format!("send_single_file: Stream failed: {err}"));
                return Err(err);
            }
        }

        self.log("send_files: All files transferred successfully".to_string());
        Ok(())
    }

    /// Streams a single file in JSON-framed chunks.
    async fn send_single_file(
        file: &File,
        chunk_size: u64,
        connection: Connection,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn SendFilesToSubscriber>>>,
        >,
    ) -> Result<()> {
        let total_len = file.data.len();
        let mut sent = 0u64;
        let mut remaining = total_len;
        let mut chunk_buffer =
            Vec::with_capacity((chunk_size + 1024).try_into().unwrap());

        let mut uni = connection.open_uni().await?;

        Self::notify_progress(file, sent, remaining, subscribers.clone());

        loop {
            chunk_buffer.clear();

            let chunk_data = file.data.read_chunk(chunk_size);
            if chunk_data.is_empty() {
                break;
            }
            let projection = FileProjection {
                id: file.id.clone(),
                data: chunk_data,
            };

            serde_json::to_writer(&mut chunk_buffer, &projection)?;
            let len_bytes = (chunk_buffer.len() as u32).to_be_bytes();

            uni.write_all(&len_bytes).await?;
            uni.write_all(&chunk_buffer).await?;

            let data_len = projection.data.len() as u64;
            sent += data_len;
            remaining = remaining.saturating_sub(data_len);

            Self::notify_progress(file, sent, remaining, subscribers.clone());
        }

        uni.finish()?;
        uni.stopped().await?;

        Ok(())
    }

    /// Marks the transfer as finished and closes the connection and endpoint.
    async fn finish(&self, endpoint: &Endpoint) {
        self.log("finish: Starting transfer finish process".to_string());

        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.log("finish: Transfer finished flag set to true".to_string());

        self.log("finish: Closing connection".to_string());
        self.connection
            .close(VarInt::from_u32(200), "finished".as_bytes());

        self.log("finish: Closing endpoint".to_string());
        endpoint.close().await;

        self.log("finish: Transfer process completed successfully".to_string());
    }

    fn log(&self, message: String) {
        self.subscribers.read().unwrap().iter().for_each(
            |(_id, subscriber)| {
                subscriber.log(message.clone());
            },
        );
    }

    fn notify_progress(
        file: &File,
        sent: u64,
        remaining: u64,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn SendFilesToSubscriber>>>,
        >,
    ) {
        let event = SendFilesToSendingEvent {
            id: file.id.clone(),
            name: file.name.clone(),
            sent,
            remaining,
        };

        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_sending(event.clone());
            });
    }
}

/// Connects to a waiting receiver and sends files.
///
/// This function:
/// - Parses the provided receiver `ticket`,
/// - Creates and binds a new iroh `Endpoint`,
/// - Connects to the receiver using the confirmation token,
/// - Returns a `SendFilesToBubble` that you can `start()` and subscribe to for
///   events.
///
/// Example:
/// ```rust no_run
/// use std::sync::Arc;
/// use dropx_sender::{
///     send_files_to::*, SenderProfile, SenderConfig, SenderFile,
/// };
///
/// struct Logger;
/// impl SendFilesToSubscriber for Logger {
///     fn get_id(&self) -> String { "logger".into() }
///     fn log(&self, msg: String) { println!("[log] {msg}"); }
///     fn notify_sending(&self, e: SendFilesToSendingEvent) {
///         println!("sent {}/{} for {}", e.sent, e.sent + e.remaining, e.name);
///     }
///     fn notify_connecting(&self, e: SendFilesToConnectingEvent) {
///         println!("connected to receiver: {}", e.receiver.name);
///     }
/// }
///
/// # async fn run() -> anyhow::Result<()> {
/// let bubble = send_files_to(SendFilesToRequest {
///     ticket: "<receiver-ticket>".into(),
///     confirmation: 42,
///     profile: SenderProfile { name: "Sender".into(), avatar_b64: None },
///     files: vec![/* ... */],
///     config: SenderConfig::balanced(),
/// }).await?;
///
/// bubble.subscribe(Arc::new(Logger));
/// bubble.start()?;
///
/// // ... await completion ...
/// # Ok(())
/// # }
/// ```
pub async fn send_files_to(
    request: SendFilesToRequest,
) -> Result<SendFilesToBubble> {
    let ticket: NodeTicket = request.ticket.parse()?;

    let endpoint_builder = Endpoint::builder().discovery_n0();
    let endpoint = endpoint_builder.bind().await?;
    let connection = endpoint
        .connect(ticket, &[request.confirmation])
        .await?;

    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: request.profile.name,
        avatar_b64: request.profile.avatar_b64,
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

    Ok(SendFilesToBubble::new(
        endpoint,
        connection,
        profile,
        files,
        request.config,
    ))
}

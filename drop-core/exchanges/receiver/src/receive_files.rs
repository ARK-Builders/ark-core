use anyhow::Result;
use drop_entities::Profile;
use dropx_common::{
    handshake::{
        HandshakeConfig, HandshakeProfile, NegotiatedConfig, ReceiverHandshake,
        SenderHandshake,
    },
    projection::FileProjection,
};
use iroh::{
    Endpoint,
    endpoint::{
        ApplicationClose, Connection, ConnectionError, RecvStream, SendStream,
        VarInt,
    },
};
use iroh_base::ticket::NodeTicket;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::Duration,
};
use tokio::{task::JoinSet, time::sleep};

use uuid::Uuid;

use super::{ReceiverConfig, ReceiverProfile};

pub struct ReceiveFilesRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: ReceiverProfile,
    pub config: Option<ReceiverConfig>,
}

pub struct ReceiveFilesBubble {
    profile: Profile,
    config: ReceiverConfig,
    endpoint: Endpoint,
    connection: Connection,
    is_running: Arc<AtomicBool>,
    is_consumed: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
}
impl ReceiveFilesBubble {
    pub fn new(
        profile: Profile,
        config: ReceiverConfig,
        endpoint: Endpoint,
        connection: Connection,
    ) -> Self {
        Self {
            profile,
            config,
            endpoint,
            connection,
            is_running: Arc::new(AtomicBool::new(false)),
            is_consumed: Arc::new(AtomicBool::new(false)),
            is_finished: Arc::new(AtomicBool::new(false)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start(&self) -> Result<()> {
        self.log("start: Checking if transfer can be started".to_string());

        // Acquiring, so we can check if the transfer has already started before
        let is_consumed = self
            .is_consumed
            .load(std::sync::atomic::Ordering::Acquire);

        if is_consumed {
            self.log(format!("start: Cannot start transfer, it has already started - consumed: {}", 
                is_consumed));
            return Err(anyhow::Error::msg(
                "Already running or has run or finished.",
            ));
        }

        self.log("start: Setting running and consumed flags".to_string());
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.is_consumed
            .store(true, std::sync::atomic::Ordering::Release);

        self.log("start: Creating carrier for file reception".to_string());
        let carrier = Carrier {
            profile: self.profile.clone(),
            config: self.config.clone(),
            negotiated_config: None,
            endpoint: self.endpoint.clone(),
            connection: self.connection.clone(),
            is_running: self.is_running.clone(),
            is_finished: self.is_finished.clone(),
            is_cancelled: self.is_cancelled.clone(),
            subscribers: self.subscribers.clone(),
        };

        self.log("start: Spawning async task for file reception".to_string());
        tokio::spawn(async move {
            let mut carrier = carrier;
            if let Err(e) = carrier.greet().await {
                carrier.log(format!("start: Handshake failed: {}", e));
                return;
            }

            let result = carrier.receive_files().await;
            if let Err(e) = result {
                carrier.log(format!("start: File reception failed: {}", e));
            } else {
                carrier.log(
                    "start: File reception completed successfully".to_string(),
                );
            }

            carrier.finish().await;
            carrier
                .is_running
                .store(false, std::sync::atomic::Ordering::Relaxed);
        });

        Ok(())
    }

    pub fn cancel(&self) {
        self.log("cancel: Checking if transfer can be cancelled".to_string());

        if !self.is_running() || self.is_finished() {
            self.log(format!(
                "cancel: Cannot cancel - not running: {} or finished: {}",
                !self.is_running(),
                self.is_finished()
            ));
            return;
        }

        self.log("cancel: Setting cancelled flag to true".to_string());
        self.is_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.log("cancel: File reception cancellation requested".to_string());
    }

    fn is_running(&self) -> bool {
        let running = self
            .is_running
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_running check: {}", running));
        running
    }

    pub fn is_finished(&self) -> bool {
        let finished = self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_finished check: {}", finished));
        finished
    }

    pub fn is_cancelled(&self) -> bool {
        let cancelled = self
            .is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_cancelled check: {}", cancelled));
        cancelled
    }

    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "subscribe: Subscribing new subscriber with ID: {}",
            subscriber_id
        ));

        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber_id.clone(), subscriber);

        self.log(format!("subscribe: Subscriber {} successfully subscribed. Total subscribers: {}", 
            subscriber_id, self.subscribers.read().unwrap().len()));
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "unsubscribe: Unsubscribing subscriber with ID: {}",
            subscriber_id
        ));

        let removed = self
            .subscribers
            .write()
            .unwrap()
            .remove(&subscriber_id);

        if removed.is_some() {
            self.log(format!("unsubscribe: Subscriber {} successfully unsubscribed. Remaining subscribers: {}", 
                subscriber_id, self.subscribers.read().unwrap().len()));
        } else {
            self.log(format!("unsubscribe: Subscriber {} was not found during unsubscribe operation", subscriber_id));
        }
    }

    fn log(&self, message: String) {
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, subscriber)| {
                subscriber.log(format!("[{}] {}", id, message));
            });
    }
}

struct Carrier {
    profile: Profile,
    config: ReceiverConfig,
    negotiated_config: Option<NegotiatedConfig>,
    endpoint: Endpoint,
    connection: Connection,
    is_running: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
}
impl Carrier {
    async fn greet(&mut self) -> Result<()> {
        let mut bi = self.connection.open_bi().await?;

        self.send_handshake(&mut bi).await?;
        self.receive_handshake(&mut bi).await?;

        bi.0.finish()?;
        bi.1.stop(VarInt::from_u32(0))?;

        self.log("greet: Handshake completed successfully".to_string());
        Ok(())
    }

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

        let mut buffer = Vec::with_capacity(256);
        serde_json::to_writer(&mut buffer, &handshake)?;

        let len_bytes = (buffer.len() as u32).to_be_bytes();

        let mut combined = Vec::with_capacity(4 + buffer.len());
        combined.extend_from_slice(&len_bytes);
        combined.extend_from_slice(&buffer);

        bi.0.write_all(&combined).await?;
        Ok(())
    }

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

        // Prepare data structures once
        let profile = ReceiveFilesProfile {
            id: handshake.profile.id,
            name: handshake.profile.name,
            avatar_b64: handshake.profile.avatar_b64,
        };

        let files: Vec<ReceiveFilesFile> = handshake
            .files
            .into_iter()
            .map(|f| ReceiveFilesFile {
                id: f.id,
                len: f.len,
                name: f.name,
            })
            .collect();

        let event = ReceiveFilesConnectingEvent {
            sender: profile,
            files,
        };

        // Notify all subscribers
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(event.clone());
            });

        Ok(())
    }

    async fn receive_files(&self) -> Result<()> {
        let (chunk_size, parallel_streams) =
            if let Some(config) = &self.negotiated_config {
                (config.chunk_size, config.parallel_streams)
            } else {
                (self.config.chunk_size, self.config.parallel_streams)
            };

        let expected_close =
            ConnectionError::ApplicationClosed(ApplicationClose {
                error_code: VarInt::from_u32(200),
                reason: "finished".into(),
            });

        let mut join_set = JoinSet::new();

        'files_iterator: loop {
            if self.is_cancelled() {
                self.connection
                    .close(VarInt::from_u32(0), b"cancelled");
                return Err(anyhow::Error::msg(
                    "Receive files has been cancelled.",
                ));
            }

            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();

            join_set.spawn(async move {
                Self::process_single_file(chunk_size, connection, subscribers)
                    .await
            });

            // Clean up completed tasks periodically
            while join_set.len() >= parallel_streams as usize {
                if let Some(result) = join_set.join_next().await {
                    if let Err(err) = result? {
                        // Downcast anyhow::Error to ConnectionError
                        if let Some(connection_err) =
                            err.downcast_ref::<ConnectionError>()
                        {
                            if connection_err == &expected_close {
                                break 'files_iterator;
                            }
                        }
                        return Err(err);
                    }
                }
            }
        }

        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result? {
                // Downcast anyhow::Error to ConnectionError
                if let Some(connection_err) =
                    err.downcast_ref::<ConnectionError>()
                {
                    if connection_err == &expected_close {
                        continue;
                    }
                }
                return Err(err);
            }
        }

        return Ok(());
    }

    async fn process_single_file(
        chunk_size: u64,
        connection: Connection,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>,
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
            let event = ReceiveFilesReceivingEvent {
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

        sleep(Duration::from_secs(1)).await;
        uni.stop(VarInt::from_u32(0))?;

        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        let cancelled = self
            .is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_cancelled check: {}", cancelled));
        cancelled
    }

    fn log(&self, message: String) {
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, subscriber)| {
                subscriber.log(format!("[{}] {}", id, message));
            });
    }

    async fn finish(&self) {
        self.log("finish: Starting transfer finish process".to_string());

        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.log("finish: Closing connection".to_string());
        self.connection
            .close(VarInt::from_u32(200), "finished".as_bytes());

        self.log("finish: Closing endpoint".to_string());
        self.endpoint.close().await;

        self.log("finish: Transfer finished flag set to true".to_string());
        self.log("finish: Transfer process completed successfully".to_string());
    }

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
}

pub trait ReceiveFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent);
    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent);
}

#[derive(Clone)]
pub struct ReceiveFilesReceivingEvent {
    pub id: String,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct ReceiveFilesConnectingEvent {
    pub sender: ReceiveFilesProfile,
    pub files: Vec<ReceiveFilesFile>,
}

#[derive(Clone)]
pub struct ReceiveFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

#[derive(Clone)]
pub struct ReceiveFilesFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

pub async fn receive_files(
    request: ReceiveFilesRequest,
) -> Result<ReceiveFilesBubble> {
    let ticket: NodeTicket = request.ticket.parse()?;

    let endpoint_builder = Endpoint::builder().discovery_n0();

    let endpoint = endpoint_builder.bind().await?;
    let connection = endpoint
        .connect(ticket, &[request.confirmation])
        .await?;

    let config = request.config.unwrap_or_default();

    Ok(ReceiveFilesBubble::new(
        Profile {
            id: Uuid::new_v4().to_string(),
            name: request.profile.name,
            avatar_b64: request.profile.avatar_b64,
        },
        config,
        endpoint,
        connection,
    ))
}

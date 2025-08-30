use anyhow::Result;
use drop_entities::{File, Profile};
use dropx_common::{
    handshake::{
        HandshakeConfig, HandshakeFile, HandshakeProfile, NegotiatedConfig,
        ReceiverHandshake, SenderHandshake,
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
use tokio::{sync::Semaphore, task::JoinSet};

use super::SenderConfig;

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
    fn notify_sending(&self, event: SendFilesSendingEvent);
    fn notify_connecting(&self, event: SendFilesConnectingEvent);
}

#[derive(Clone)]
pub struct SendFilesSendingEvent {
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

pub struct SendFilesConnectingEvent {
    pub receiver: SendFilesProfile,
}

#[derive(Clone)]
pub struct SendFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct SendFilesHandler {
    is_consumed: AtomicBool,
    is_finished: Arc<AtomicBool>,
    profile: Profile,
    files: Vec<File>,
    config: SenderConfig,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
}
impl Debug for SendFilesHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendFilesHandler")
            .field("is_consumed", &self.is_consumed)
            .field("is_finished", &self.is_finished)
            .field("profile", &self.profile)
            .field("files", &self.files)
            .field("config", &self.config)
            .finish()
    }
}
impl SendFilesHandler {
    pub fn new(
        profile: Profile,
        files: Vec<File>,
        config: SenderConfig,
    ) -> Self {
        return Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            files: files.clone(),
            config,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        };
    }

    pub fn is_consumed(&self) -> bool {
        let consumed = self
            .is_consumed
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_consumed check: {}", consumed));
        consumed
    }

    pub fn is_finished(&self) -> bool {
        let finished = self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_finished check: {}", finished));
        finished
    }

    pub fn log(&self, message: String) {
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, subscriber)| {
                subscriber.log(format!("[{}] {}", id, message));
            });
    }

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "Subscribing new subscriber with ID: {}",
            subscriber_id
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

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "Unsubscribing subscriber with ID: {}",
            subscriber_id
        ));

        let removed = self
            .subscribers
            .write()
            .unwrap()
            .remove(&subscriber_id);

        if removed.is_some() {
            self.log(format!("Subscriber {} successfully unsubscribed. Remaining subscribers: {}", subscriber_id, self.subscribers.read().unwrap().len()));
        } else {
            self.log(format!(
                "Subscriber {} was not found during unsubscribe operation",
                subscriber_id
            ));
        }
    }
}
impl ProtocolHandler for SendFilesHandler {
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
        self.log("accept: Creating carrier for file transfer".to_string());

        let carrier = Carrier {
            is_finished: self.is_finished.clone(),
            config: self.config.clone(),
            negotiated_config: None,
            profile: self.profile.clone(),
            connection,
            files: self.files.clone(),
            subscribers: self.subscribers.clone(),
        };

        async move {
            let mut carrier = carrier;
            if let Err(_) = carrier.greet().await {
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            if let Err(_) = carrier.send_files().await {
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            carrier.finish();
            Ok(())
        }
    }
}

struct Carrier {
    is_finished: Arc<AtomicBool>,
    config: SenderConfig,
    negotiated_config: Option<NegotiatedConfig>,
    profile: Profile,
    connection: Connection,
    files: Vec<File>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
}
impl Carrier {
    async fn greet(&mut self) -> Result<()> {
        let mut bi = self.connection.accept_bi().await?;

        self.send_handshake(&mut bi).await?;
        self.receive_handshake(&mut bi).await?;

        bi.0.stopped().await?;

        self.log("greet: Handshake completed successfully".to_string());
        Ok(())
    }

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

        // Pre-allocate vector with estimated capacity
        let mut buffer = Vec::with_capacity(512);
        serde_json::to_writer(&mut buffer, &handshake)?;

        let len_bytes = (buffer.len() as u32).to_be_bytes();

        // Single write operation
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
        let profile = SendFilesProfile {
            id: handshake.profile.id,
            name: handshake.profile.name,
            avatar_b64: handshake.profile.avatar_b64,
        };

        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(SendFilesConnectingEvent {
                    receiver: profile.clone(),
                });
            });

        Ok(())
    }

    async fn send_files(&self) -> Result<()> {
        let mut join_set = JoinSet::new();

        // Use negotiated configuration or fallback to defaults
        let (chunk_size, parallel_streams) =
            if let Some(config) = &self.negotiated_config {
                (config.chunk_size, config.parallel_streams)
            } else {
                (self.config.chunk_size, self.config.parallel_streams)
            };

        let semaphore = Arc::new(Semaphore::new(parallel_streams as usize));

        for file in self.files.clone() {
            let sem = semaphore.clone();
            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();
            join_set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                return Self::send_single_file(
                    &file,
                    chunk_size,
                    connection,
                    subscribers,
                )
                .await;
            });
        }

        // Wait for all remaining streams to complete and update final progress
        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result? {
                self.log(format!("send_single_file: Stream failed: {}", err));
                return Err(err);
            }
        }

        self.log("send_files: All files transferred successfully".to_string());
        return Ok(());
    }

    async fn send_single_file(
        file: &File,
        chunk_size: u64,
        connection: Connection,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    ) -> Result<()> {
        let total_len = file.data.len() as u64;
        let mut sent = 0u64;
        let mut remaining = total_len;
        let mut chunk_buffer =
            Vec::with_capacity((chunk_size + 1024).try_into().unwrap());

        let mut uni = connection.open_uni().await?;

        Self::notify_progress(&file.name, sent, remaining, subscribers.clone());

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

            // Write header + data
            uni.write_all(&len_bytes).await?;
            uni.write_all(&chunk_buffer).await?;

            let data_len = projection.data.len() as u64;
            sent += data_len;
            remaining = remaining.saturating_sub(data_len);

            Self::notify_progress(
                &file.name,
                sent,
                remaining,
                subscribers.clone(),
            );
        }

        uni.finish()?;
        uni.stopped().await?;

        return Ok(());
    }

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

    fn log(&self, message: String) {
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, subscriber)| {
                subscriber.log(format!("[{}] {}", id, message));
            });
    }

    fn notify_progress(
        name: &str,
        sent: u64,
        remaining: u64,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    ) {
        let event = SendFilesSendingEvent {
            name: name.to_string(),
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

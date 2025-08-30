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
    cmp::min,
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock, atomic::AtomicBool},
};
use tokio::{
    task::JoinSet,
    time::{Duration, timeout},
};

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
            .field("config_buffer_size", &self.config.buffer_size)
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

    #[inline(always)]
    pub fn log(&self, message: String) {
        // Only log important messages to reduce overhead
        if message.contains("error")
            || message.contains("failed")
            || message.contains("completed")
        {
            self.subscribers.read().unwrap().iter().for_each(
                |(id, subscriber)| {
                    subscriber.log(format!("[{}] {}", id, message));
                },
            );
        }
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
                buffer_size: self.config.buffer_size,
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
            buffer_size: self.config.buffer_size,
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
        for file in &self.files {
            self.send_single_file(file).await?;
        }
        self.log("send_files: All files transferred successfully".to_string());
        Ok(())
    }

    async fn send_single_file(&self, file: &File) -> Result<()> {
        let mut sent = 0u64;
        let total_len = file.data.len() as u64;
        let mut remaining = total_len;

        // Initial progress notification
        self.notify_progress(&file.name, sent, remaining);

        // Use negotiated configuration or fallback to defaults
        let (chunk_size, buffer_size, parallel_streams) =
            if let Some(config) = &self.negotiated_config {
                (
                    config.chunk_size,
                    config.buffer_size,
                    config.parallel_streams,
                )
            } else {
                (
                    self.config.chunk_size,
                    self.config.buffer_size,
                    self.config.parallel_streams,
                )
            };

        let mut join_set = JoinSet::new();
        let chunks_per_stream = (buffer_size / chunk_size).max(1) as usize;
        let mut batch_buffer = Vec::with_capacity(chunks_per_stream);

        // Read and send data in batches to avoid loading entire file into
        // memory
        while remaining > 0 {
            // Fill up a batch of chunks
            batch_buffer.clear();
            let mut batch_size = 0u64;

            for _ in 0..chunks_per_stream {
                if remaining == 0 {
                    break;
                }

                let current_chunk_size = min(chunk_size, remaining);
                let chunk_data = file.data.read_chunk(current_chunk_size);

                if chunk_data.is_empty() {
                    if remaining > 0 {
                        self.log(format!("send_single_file: Unexpected end of file. Expected {} more bytes", remaining));
                        return Err(anyhow::Error::msg(
                            "Unexpected end of file",
                        ));
                    }
                    break;
                }

                let projection = FileProjection {
                    id: file.id.clone(),
                    data: chunk_data,
                };

                let bytes_read = projection.data.len() as u64;
                batch_size += bytes_read;
                remaining = remaining.saturating_sub(bytes_read);
                batch_buffer.push(projection);
            }

            // Send the batch if we have chunks
            if !batch_buffer.is_empty() {
                let connection = self.connection.clone();
                let file_name = file.name.clone();
                let subscribers = self.subscribers.clone();
                let stream_chunks = batch_buffer.clone();
                let batch_bytes = batch_size;

                join_set.spawn(async move {
                    // Add timeout to prevent hanging streams
                    let result = timeout(
                        Duration::from_secs(30),
                        Self::send_stream_chunks(
                            chunk_size,
                            connection,
                            stream_chunks,
                            file_name,
                            subscribers,
                        ),
                    )
                    .await;

                    match result {
                        Ok(stream_result) => stream_result.map(|_| batch_bytes),
                        Err(_) => {
                            Err(anyhow::Error::msg("Stream send timeout"))
                        }
                    }
                });

                // Limit concurrent streams to negotiated number
                if join_set.len() >= parallel_streams as usize {
                    if let Some(result) = join_set.join_next().await {
                        match result? {
                            Ok(transmitted_bytes) => {
                                sent += transmitted_bytes;
                                self.notify_progress(
                                    &file.name,
                                    sent,
                                    total_len.saturating_sub(sent),
                                );
                            }
                            Err(e) => {
                                self.log(format!(
                                    "send_single_file: Stream failed: {}",
                                    e
                                ));
                                return Err(e);
                            }
                        }
                    }
                }
            }
        }

        // Wait for all remaining streams to complete and update final progress
        while let Some(result) = join_set.join_next().await {
            match result? {
                Ok(transmitted_bytes) => {
                    sent += transmitted_bytes;
                    self.notify_progress(
                        &file.name,
                        sent,
                        total_len.saturating_sub(sent),
                    );
                }
                Err(e) => {
                    self.log(format!("send_single_file: Stream failed: {}", e));
                    return Err(e);
                }
            }
        }

        // Final progress notification to ensure 100% completion
        self.notify_progress(&file.name, total_len, 0);

        Ok(())
    }

    async fn send_stream_chunks(
        chunk_size: u64,
        connection: Connection,
        chunks: Vec<FileProjection>,
        _file_name: String,
        _subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>,
        >,
    ) -> Result<u64> {
        let mut uni = connection.open_uni().await?;
        let mut total_sent = 0u64;

        // Pre-allocate buffer for serialization
        let mut buffer =
            Vec::with_capacity((chunk_size + 256 * 1024).try_into().unwrap());

        for chunk in chunks {
            let data_len = chunk.data.len() as u64;

            // Serialize chunk
            buffer.clear();
            serde_json::to_writer(&mut buffer, &chunk)?;

            let len_bytes = (buffer.len() as u32).to_be_bytes();

            // Write header + data
            uni.write_all(&len_bytes).await?;
            uni.write_all(&buffer).await?;

            total_sent += data_len;
        }

        uni.stopped().await?;

        Ok(total_sent)
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

    #[inline(always)]
    fn log(&self, message: String) {
        // Only log important messages to reduce overhead
        if message.contains("error")
            || message.contains("failed")
            || message.contains("completed")
        {
            self.subscribers.read().unwrap().iter().for_each(
                |(id, subscriber)| {
                    subscriber.log(format!("[{}] {}", id, message));
                },
            );
        }
    }

    #[inline(always)]
    fn notify_progress(&self, name: &str, sent: u64, remaining: u64) {
        let event = SendFilesSendingEvent {
            name: name.to_string(),
            sent,
            remaining,
        };

        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_sending(event.clone());
            });
    }
}

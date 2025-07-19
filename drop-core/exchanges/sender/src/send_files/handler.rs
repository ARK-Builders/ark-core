use anyhow::{Ok, Result};
use common::{
    FileProjection, HandshakeFile, HandshakeProfile, ReceiverHandshake,
    SenderHandshake,
};
use entities::{File, Profile};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream, VarInt},
    protocol::ProtocolHandler,
};
use std::{
    collections::HashMap,
    fmt::Debug,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock, atomic::AtomicBool},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn notify_sending(&self, event: SendFilesSendingEvent);
    fn notify_connecting(&self, event: SendFilesConnectingEvent);
}

pub struct SendFilesSendingEvent {
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
}

pub struct SendFilesConnectingEvent {
    pub receiver: SendFilesProfile,
}

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
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    chunk_size: usize,
    max_concurrent_streams: usize,
}
impl Debug for SendFilesHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendFilesHandler")
            .field("is_consumed", &self.is_consumed)
            .field("is_finished", &self.is_finished)
            .field("profile", &self.profile)
            .field("files", &self.files)
            .field("chunk_size", &self.chunk_size)
            .field("max_concurrent_streams", &self.max_concurrent_streams)
            .finish()
    }
}
impl SendFilesHandler {
    pub fn new(profile: Profile, files: Vec<File>) -> Self {
        Self::with_config(profile, files, 1024 * 1024, 8) // 1MB chunks, 8 concurrent streams
    }

    pub fn with_config(
        profile: Profile,
        files: Vec<File>,
        chunk_size: usize,
        max_concurrent_streams: usize,
    ) -> Self {
        Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            files,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            chunk_size,
            max_concurrent_streams,
        }
    }

    pub fn is_consumed(&self) -> bool {
        return self
            .is_consumed
            .load(std::sync::atomic::Ordering::Acquire);
    }

    pub fn is_finished(&self) -> bool {
        return self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed);
    }

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber.get_id(), subscriber);
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .remove(&subscriber.get_id());
    }
}
impl ProtocolHandler for SendFilesHandler {
    fn on_connecting(
        &self,
        connecting: iroh::endpoint::Connecting,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<iroh::endpoint::Connection>>
                + Send
                + 'static,
        >,
    > {
        let is_consumed = self
            .is_consumed
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Relaxed,
            )
            .unwrap_or(true);
        if is_consumed {
            return Box::pin(async {
                return Err(anyhow::anyhow!(
                    "Connection has already been consumed."
                ));
            });
        }
        return Box::pin(async { return Ok(connecting.await?) });
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        return Box::pin(async {});
    }

    fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let carrier = Carrier {
            is_finished: self.is_finished.clone(),
            chunk_size: self.chunk_size,
            max_concurrent_streams: self.max_concurrent_streams,
            profile: self.profile.clone(),
            connection,
            files: self.files.clone(),
            subscribers: self.subscribers.clone(),
        };
        return Box::pin(async move {
            carrier.greet().await?;
            carrier.send_files().await?;
            carrier.finish();
            return Ok(());
        });
    }
}

struct Carrier {
    is_finished: Arc<AtomicBool>,
    chunk_size: usize,
    max_concurrent_streams: usize,
    profile: Profile,
    connection: Connection,
    files: Vec<File>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
}
impl Carrier {
    async fn greet(&self) -> Result<()> {
        let mut bi = self.connection.accept_bi().await?;
        self.send_handshake(&mut bi).await?;
        self.receive_handshake(&mut bi).await?;
        bi.0.finish()?;
        bi.0.stopped().await?;
        return Ok(());
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
        };
        // Optimization: Use bincode for faster serialization
        let serialized_handshake = bincode::serialize(&handshake)
            .unwrap_or_else(|_| serde_json::to_vec(&handshake).unwrap());
        let serialized_handshake_len = serialized_handshake.len() as u32;
        let serialized_handshake_header =
            serialized_handshake_len.to_be_bytes();
        bi.0.write_all(&serialized_handshake_header)
            .await?;
        bi.0.write_all(&serialized_handshake).await?;
        return Ok(());
    }

    async fn receive_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        let mut serialized_handshake_header = [0u8; 4];
        bi.1.read_exact(&mut serialized_handshake_header)
            .await?;
        let serialized_handshake_len =
            u32::from_be_bytes(serialized_handshake_header);
        let mut serialized_handshake =
            vec![0u8; serialized_handshake_len as usize];
        bi.1.read_exact(&mut serialized_handshake).await?;
        // Try bincode first, fallback to JSON
        let handshake: ReceiverHandshake =
            bincode::deserialize(&serialized_handshake).unwrap_or_else(|_| {
                serde_json::from_slice(&serialized_handshake).unwrap()
            });
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(SendFilesConnectingEvent {
                    receiver: SendFilesProfile {
                        id: handshake.profile.id.clone(),
                        name: handshake.profile.name.clone(),
                        avatar_b64: handshake.profile.avatar_b64.clone(),
                    },
                });
            });
        return Ok(());
    }

    async fn send_files(&self) -> Result<()> {
        use futures::stream::{FuturesUnordered, StreamExt};
        use tokio::sync::Semaphore;
        // Optimization: Use semaphore to control concurrent streams
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_streams));
        let mut tasks = FuturesUnordered::new();
        for file in &self.files {
            let file_clone = file.clone();
            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();
            let chunk_size = self.chunk_size;
            let semaphore = semaphore.clone();

            let task = async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::send_single_file(
                    file_clone,
                    connection,
                    subscribers,
                    chunk_size,
                )
                .await
            };

            tasks.push(task);
        }
        // Process all files concurrently
        while let Some(result) = tasks.next().await {
            result?;
        }
        self.connection
            .close(VarInt::from_u32(200), String::from("Finished.").as_bytes());
        return Ok(());
    }

    async fn send_single_file(
        file: File,
        connection: Connection,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
        chunk_size: usize,
    ) -> Result<()> {
        let mut sent = 0u64;
        let total_len = file.data.len();
        let mut remaining = total_len;
        // Initial progress notification
        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_sending(SendFilesSendingEvent {
                    name: file.name.clone(),
                    sent,
                    remaining,
                });
            });
        // Optimization: Pre-allocate buffer to avoid repeated allocations
        let mut buffer = Vec::with_capacity(chunk_size);
        loop {
            buffer.clear();
            // Read chunk from file data
            for _ in 0..chunk_size {
                if let Some(byte) = file.data.read() {
                    buffer.push(byte);
                } else {
                    break;
                }
            }
            if buffer.is_empty() {
                break;
            }
            let projection = FileProjection {
                id: file.id.clone(),
                data: buffer.clone(),
            };
            // Optimization: Use bincode for faster serialization
            let serialized_projection = bincode::serialize(&projection)
                .unwrap_or_else(|_| serde_json::to_vec(&projection).unwrap());
            let mut uni = connection.open_uni().await?;
            let serialized_projection_len = serialized_projection.len() as u32;
            let serialized_projection_header =
                serialized_projection_len.to_be_bytes();
            // Optimization: Write header and data in single operation when possible
            let mut write_buffer =
                Vec::with_capacity(4 + serialized_projection.len());
            write_buffer.extend_from_slice(&serialized_projection_header);
            write_buffer.extend_from_slice(&serialized_projection);
            uni.write_all(&write_buffer).await?;
            uni.finish()?;
            let chunk_len = buffer.len() as u64;
            sent += chunk_len;
            remaining = remaining.saturating_sub(chunk_len);
            // Progress notification
            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(_, s)| {
                    s.notify_sending(SendFilesSendingEvent {
                        name: file.name.clone(),
                        sent,
                        remaining,
                    });
                });
            uni.stopped().await?;
        }
        return Ok(());
    }

    fn finish(&self) {
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

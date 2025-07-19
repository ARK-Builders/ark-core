use anyhow::{Ok, Result};
use common::{
    FileProjection, HandshakeProfile, ReceiverHandshake, SenderHandshake,
};
use entities::Profile;
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
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::ReceiverProfile;

pub struct ReceiveFilesRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: ReceiverProfile,
}

pub struct ReceiveFilesBubble {
    profile: Profile,
    endpoint: Endpoint,
    connection: Connection,
    is_running: Arc<AtomicBool>,
    is_consumed: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
    // Optimization: Buffer pool for reusing memory
    buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>,
    // Optimization: Concurrent stream processing
    max_concurrent_streams: usize,
}
impl ReceiveFilesBubble {
    pub fn new(
        profile: Profile,
        endpoint: Endpoint,
        connection: Connection,
    ) -> Self {
        Self::with_config(profile, endpoint, connection, 8) // 8 concurrent streams
    }

    pub fn with_config(
        profile: Profile,
        endpoint: Endpoint,
        connection: Connection,
        max_concurrent_streams: usize,
    ) -> Self {
        Self {
            profile,
            endpoint,
            connection,
            is_running: Arc::new(AtomicBool::new(false)),
            is_consumed: Arc::new(AtomicBool::new(false)),
            is_finished: Arc::new(AtomicBool::new(false)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            buffer_pool: Arc::new(RwLock::new(Vec::new())),
            max_concurrent_streams,
        }
    }

    pub fn start(&self) -> Result<()> {
        if self.is_running() || self.is_consumed() || self.is_finished() {
            return Err(anyhow::Error::msg(
                "Already running or has run or finished.",
            ));
        }
        self.is_running
            .store(true, std::sync::atomic::Ordering::Release);
        self.is_consumed
            .store(true, std::sync::atomic::Ordering::Release);

        let carrier = Carrier {
            profile: self.profile.clone(),
            endpoint: self.endpoint.clone(),
            connection: self.connection.clone(),
            is_running: self.is_running.clone(),
            is_finished: self.is_finished.clone(),
            is_cancelled: self.is_cancelled.clone(),
            subscribers: self.subscribers.clone(),
            buffer_pool: self.buffer_pool.clone(),
            max_concurrent_streams: self.max_concurrent_streams,
        };
        tokio::spawn(async move {
            let result = async {
                carrier.greet().await?;
                carrier.receive_files().await
            }
            .await;
            if result.is_ok() {
                carrier
                    .is_finished
                    .store(true, std::sync::atomic::Ordering::Release);
            }
            carrier.endpoint.close().await;
            carrier
                .is_running
                .store(false, std::sync::atomic::Ordering::Release);
            return ();
        });

        return Ok(());
    }

    pub fn cancel(&self) {
        if !self.is_running() || self.is_finished() {
            return ();
        }
        return self
            .is_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn is_running(&self) -> bool {
        return self
            .is_running
            .load(std::sync::atomic::Ordering::Acquire);
    }

    fn is_consumed(&self) -> bool {
        return self
            .is_consumed
            .load(std::sync::atomic::Ordering::Acquire);
    }

    pub fn is_finished(&self) -> bool {
        return self
            .is_finished
            .load(std::sync::atomic::Ordering::Acquire);
    }

    pub fn is_cancelled(&self) -> bool {
        return self
            .is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
    }

    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber.get_id(), subscriber);
        return ();
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .remove(&subscriber.get_id());
        return ();
    }
}

struct Carrier {
    profile: Profile,
    endpoint: Endpoint,
    connection: Connection,
    is_running: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
    buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>,
    max_concurrent_streams: usize,
}
impl Carrier {
    async fn greet(&self) -> Result<()> {
        let mut bi = self.connection.open_bi().await?;
        self.receive_handshake(&mut bi).await?;
        self.send_handshake(&mut bi).await?;
        bi.0.finish()?;
        bi.0.stopped().await?;
        return Ok(());
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
        // Optimization: Get buffer from pool or allocate new one
        let mut serialized_handshake =
            self.get_buffer(serialized_handshake_len as usize);
        serialized_handshake.resize(serialized_handshake_len as usize, 0);
        bi.1.read_exact(&mut serialized_handshake).await?;
        // Try bincode first, fallback to JSON
        let handshake: SenderHandshake =
            bincode::deserialize(&serialized_handshake).unwrap_or_else(|_| {
                serde_json::from_slice(&serialized_handshake).unwrap()
            });
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_connecting(ReceiveFilesConnectingEvent {
                    sender: ReceiveFilesProfile {
                        id: handshake.profile.id.clone(),
                        name: handshake.profile.name.clone(),
                        avatar_b64: handshake.profile.avatar_b64.clone(),
                    },
                    files: handshake
                        .files
                        .iter()
                        .map(|f| ReceiveFilesFile {
                            id: f.id.clone(),
                            len: f.len,
                            name: f.name.clone(),
                        })
                        .collect(),
                });
            });
        // Return buffer to pool
        self.return_buffer(serialized_handshake);
        return Ok(());
    }

    async fn receive_files(&self) -> Result<()> {
        use futures::stream::{FuturesUnordered, StreamExt};
        use tokio::sync::Semaphore;

        // Optimization: Use semaphore to control concurrent stream processing
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_streams));
        let mut tasks = FuturesUnordered::new();

        loop {
            if self.is_cancelled() {
                self.connection.close(
                    VarInt::from_u32(0),
                    String::from("Receive files has been cancelled.")
                        .as_bytes(),
                );
                return Err(anyhow::Error::msg(
                    "Receive files has been cancelled.",
                ));
            }
            let uni_result = self.connection.accept_uni().await;
            if uni_result.is_err() {
                let err = uni_result.unwrap_err();
                if err.eq(&ConnectionError::ApplicationClosed(
                    ApplicationClose {
                        error_code: VarInt::from_u32(200),
                        reason: String::from("Finished.").into(),
                    },
                )) {
                    break;
                }
                return Err(anyhow::Error::msg(
                    "Connection unexpectedly closed.",
                ));
            }
            let uni = uni_result.unwrap();
            let subscribers = self.subscribers.clone();
            let buffer_pool = self.buffer_pool.clone();
            let semaphore = semaphore.clone();

            let task = async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::process_stream(uni, subscribers, buffer_pool).await
            };

            tasks.push(task);

            // Process completed tasks to avoid unbounded growth
            while let Ok(Some(result)) = tokio::time::timeout(
                std::time::Duration::from_millis(1),
                tasks.next(),
            )
            .await
            {
                result?;
            }
        }

        // Wait for remaining tasks to complete
        while let Some(result) = tasks.next().await {
            result?;
        }

        Ok(())
    }

    async fn process_stream(
        mut uni: RecvStream,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>,
        >,
        buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>,
    ) -> Result<()> {
        let projection = Self::read_projection(&mut uni, &buffer_pool).await?;
        if let Some(projection) = projection {
            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(_, s)| {
                    s.notify_receiving(ReceiveFilesReceivingEvent {
                        id: projection.id.clone(),
                        data: projection.data.clone(),
                    });
                });
        }
        return Ok(());
    }

    fn is_cancelled(&self) -> bool {
        self.is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn read_projection(
        uni: &mut RecvStream,
        buffer_pool: &Arc<RwLock<Vec<Vec<u8>>>>,
    ) -> Result<Option<FileProjection>> {
        // Read header
        let mut header = [0u8; 4];
        let bytes_read = uni.read(&mut header).await?;
        if bytes_read.is_none() || bytes_read.unwrap() != 4 {
            return Ok(None);
        }

        let serialized_projection_len = u32::from_be_bytes(header);

        // Get buffer from pool
        let mut buffer = Self::get_buffer_from_pool(
            buffer_pool,
            serialized_projection_len as usize,
        );
        buffer.resize(serialized_projection_len as usize, 0);

        uni.read_exact(&mut buffer).await?;

        // Try bincode first, fallback to JSON
        let projection: FileProjection = bincode::deserialize(&buffer)
            .unwrap_or_else(|_| serde_json::from_slice(&buffer).unwrap());

        // Return buffer to pool
        Self::return_buffer_to_pool(buffer_pool, buffer);

        Ok(Some(projection))
    }

    fn get_buffer(&self, size: usize) -> Vec<u8> {
        Self::get_buffer_from_pool(&self.buffer_pool, size)
    }

    fn return_buffer(&self, buffer: Vec<u8>) {
        Self::return_buffer_to_pool(&self.buffer_pool, buffer);
    }

    fn get_buffer_from_pool(
        pool: &Arc<RwLock<Vec<Vec<u8>>>>,
        min_size: usize,
    ) -> Vec<u8> {
        if let Ok(mut pool) = pool.write() {
            // Find a suitable buffer from the pool
            if let Some(pos) = pool
                .iter()
                .position(|buf| buf.capacity() >= min_size)
            {
                let mut buffer = pool.swap_remove(pos);
                buffer.clear();
                return buffer;
            }
        }
        // Allocate new buffer with some extra capacity to reduce future allocations
        Vec::with_capacity(min_size.max(4096))
    }

    fn return_buffer_to_pool(
        pool: &Arc<RwLock<Vec<Vec<u8>>>>,
        mut buffer: Vec<u8>,
    ) {
        buffer.clear();
        if let Ok(mut pool) = pool.write() {
            // Limit pool size to prevent excessive memory usage
            if pool.len() < 32 {
                pool.push(buffer);
            }
        }
    }
}

pub trait ReceiveFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent);
    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent);
}

pub struct ReceiveFilesReceivingEvent {
    pub id: String,
    pub data: Vec<u8>,
}

pub struct ReceiveFilesConnectingEvent {
    pub sender: ReceiveFilesProfile,
    pub files: Vec<ReceiveFilesFile>,
}

pub struct ReceiveFilesProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct ReceiveFilesFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

pub async fn receive_files(
    request: ReceiveFilesRequest,
) -> Result<ReceiveFilesBubble> {
    let ticket: NodeTicket = request.ticket.parse()?;
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;
    let connection = endpoint
        .connect(ticket, &[request.confirmation])
        .await?;
    return Ok(ReceiveFilesBubble::new(
        Profile {
            id: Uuid::new_v4().to_string(),
            name: request.profile.name,
            avatar_b64: request.profile.avatar_b64,
        },
        endpoint,
        connection,
    ));
}

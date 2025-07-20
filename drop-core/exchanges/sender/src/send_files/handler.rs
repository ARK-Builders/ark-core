use anyhow::{Ok, Result};
use drop_entities::{File, Profile};
use dropx_common::{
    FileProjection, HandshakeFile, HandshakeProfile, ReceiverHandshake,
    SenderHandshake,
};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream, VarInt},
    protocol::ProtocolHandler,
};
use std::{
    collections::HashMap,
    fmt::Debug,
    pin::Pin,
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::Instant,
};
use tokio::sync::Semaphore;

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
}
impl Debug for SendFilesHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendFilesHandler")
            .field("is_consumed", &self.is_consumed)
            .field("is_finished", &self.is_finished)
            .field("profile", &self.profile)
            .field("files", &self.files)
            .finish()
    }
}
impl SendFilesHandler {
    pub fn new(profile: Profile, files: Vec<File>) -> Self {
        return Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            files,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        };
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
        return ();
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .remove(&subscriber.get_id());
        return ();
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
        return Box::pin(async {
            return Ok(connecting.await?);
        });
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
            // Increased chunk size for better throughput
            limiter: 65536, // 64KB chunks instead of 1KB
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
    limiter: u32,
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
        let serialized_handshake = serde_json::to_vec(&handshake).unwrap();
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
        let handshake: ReceiverHandshake =
            serde_json::from_slice(&serialized_handshake)?;
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
        // Use parallel file sending with controlled concurrency
        let semaphore = Arc::new(Semaphore::new(4)); // Max 4 concurrent file transfers
        let mut handles = Vec::new();

        for file in &self.files {
            let file = file.clone();
            let connection = self.connection.clone();
            let subscribers = self.subscribers.clone();
            let limiter = self.limiter;
            let semaphore = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::send_single_file(file, connection, subscribers, limiter)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all files to complete
        for handle in handles {
            handle.await??;
        }

        self.connection
            .close(VarInt::from_u32(200), String::from("Finished.").as_bytes());
        return Ok(());
    }

    async fn send_single_file(
        file: File,
        connection: Connection,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
        limiter: u32,
    ) -> Result<()> {
        let mut sent = 0;
        let mut remaining = file.data.len();
        let mut last_notification = Instant::now();
        const NOTIFICATION_INTERVAL_MS: u64 = 100; // Throttle notifications to every 100ms

        // Initial notification
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

        // Use buffered chunks for better performance
        let mut chunk_buffer = Vec::with_capacity(limiter as usize * 8); // Buffer multiple chunks
        let mut chunks_in_buffer = 0;
        const MAX_CHUNKS_IN_BUFFER: usize = 8;

        loop {
            // Fill buffer with chunks
            while chunks_in_buffer < MAX_CHUNKS_IN_BUFFER {
                let projection = Self::read_next_projection(&file, limiter);
                if projection.is_none() {
                    break;
                }
                let projection = projection.unwrap();
                chunk_buffer.push(projection);
                chunks_in_buffer += 1;
            }

            if chunk_buffer.is_empty() {
                break;
            }

            // Send all buffered chunks in parallel
            let mut chunk_handles = Vec::new();
            for projection in chunk_buffer.drain(..) {
                let connection = connection.clone();
                let handle = tokio::spawn(async move {
                    let mut uni = connection.open_uni().await?;
                    let serialized_projection =
                        serde_json::to_vec(&projection)?;
                    let serialized_projection_len =
                        serialized_projection.len() as u16;
                    let serialized_projection_header =
                        serialized_projection_len.to_be_bytes();

                    uni.write_all(&serialized_projection_header)
                        .await?;
                    uni.write_all(&serialized_projection).await?;
                    uni.finish()?;
                    uni.stopped().await?;

                    Ok::<u64>(projection.data.len() as u64)
                });
                chunk_handles.push(handle);
            }

            // Wait for all chunks to complete and update progress
            for handle in chunk_handles {
                let chunk_size = handle.await??;
                sent += chunk_size;
                if remaining >= chunk_size {
                    remaining -= chunk_size;
                } else {
                    remaining = 0;
                }
            }

            chunks_in_buffer = 0;

            // Throttled progress notifications
            if last_notification.elapsed().as_millis()
                >= NOTIFICATION_INTERVAL_MS as u128
            {
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
                last_notification = Instant::now();
            }
        }

        // Final notification
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

        Ok(())
    }

    fn read_next_projection(
        file: &File,
        limiter: u32,
    ) -> Option<FileProjection> {
        let mut data = Vec::with_capacity(limiter as usize);
        for _ in 0..limiter {
            let b = file.data.read();
            if b.is_none() {
                break;
            }
            data.push(b.unwrap());
        }
        if data.is_empty() {
            return None;
        }
        return Some(FileProjection {
            id: file.id.clone(),
            data,
        });
    }

    fn finish(&self) -> () {
        return self
            .is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

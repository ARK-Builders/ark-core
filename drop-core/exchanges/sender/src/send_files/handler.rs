use anyhow::Result;
use drop_entities::{File, Profile};
use dropx_common::{
    FileProjection, HandshakeFile, HandshakeProfile, ReceiverHandshake,
    SenderHandshake,
};
use flate2::{Compression, write::GzEncoder};
use futures::{Future, future::join_all};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream, VarInt},
    protocol::ProtocolHandler,
};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::Write,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64},
    },
    time::Instant,
};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

use super::SenderConfig;

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn notify_sending(&self, event: SendFilesSendingEvent);
    fn notify_connecting(&self, event: SendFilesConnectingEvent);
}

pub struct SendFilesSendingEvent {
    pub name: String,
    pub sent: u64,
    pub remaining: u64,
    pub throughput_mbps: f64,
    pub compression_ratio: f64,
    pub active_streams: u32,
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
    config: SenderConfig,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    bytes_sent: Arc<AtomicU64>,
    bytes_compressed: Arc<AtomicU64>,
    start_time: Arc<RwLock<Option<Instant>>>,
    active_streams: Arc<AtomicU32>,
}

impl Debug for SendFilesHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendFilesHandler")
            .field("is_consumed", &self.is_consumed)
            .field("is_finished", &self.is_finished)
            .field("profile", &self.profile)
            .field("files", &self.files)
            .field("config_chunk_size", &self.config.chunk_size)
            .field(
                "active_streams",
                &self
                    .active_streams
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish()
    }
}

impl SendFilesHandler {
    pub fn new(
        profile: Profile,
        files: Vec<File>,
        config: SenderConfig,
    ) -> Self {
        info!("Creating send files handler with {} files", files.len());
        Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            files,
            config,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_compressed: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(RwLock::new(None)),
            active_streams: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn is_consumed(&self) -> bool {
        self.is_consumed
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
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

    pub fn get_performance_metrics(&self) -> String {
        let bytes_sent = self
            .bytes_sent
            .load(std::sync::atomic::Ordering::Relaxed);
        let bytes_compressed = self
            .bytes_compressed
            .load(std::sync::atomic::Ordering::Relaxed);
        let active_streams = self
            .active_streams
            .load(std::sync::atomic::Ordering::Relaxed);

        let compression_ratio = if bytes_sent > 0 {
            bytes_compressed as f64 / bytes_sent as f64
        } else {
            1.0
        };

        let throughput = {
            let start_time_guard = self.start_time.read().unwrap();
            if let Some(start) = *start_time_guard {
                let elapsed = start.elapsed();
                if elapsed.as_secs() > 0 {
                    (bytes_sent as f64 / (1024.0 * 1024.0))
                        / elapsed.as_secs_f64()
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        format!(
            "Bytes sent: {}, Compression ratio: {:.2}, Throughput: {:.2} MB/s, Active streams: {}",
            bytes_sent, compression_ratio, throughput, active_streams
        )
    }
}

impl ProtocolHandler for SendFilesHandler {
    fn on_connecting(
        &self,
        connecting: iroh::endpoint::Connecting,
    ) -> impl Future<
        Output = std::result::Result<Connection, iroh::protocol::AcceptError>,
    > + Send {
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

            info!("Accepting connection");
            return Ok(connecting.await?);
        }
    }

    fn shutdown(&self) -> impl Future<Output = ()> + Send {
        info!("Shutting down handler");
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
        let carrier = Carrier {
            is_finished: self.is_finished.clone(),
            config: self.config.clone(),
            profile: self.profile.clone(),
            connection,
            files: self.files.clone(),
            subscribers: self.subscribers.clone(),
            bytes_sent: self.bytes_sent.clone(),
            bytes_compressed: self.bytes_compressed.clone(),
            start_time: self.start_time.clone(),
            active_streams: self.active_streams.clone(),
        };

        async move {
            info!("Starting file transfer process");
            carrier
                .greet()
                .await
                .map_err(|e| iroh::protocol::AcceptError::NotAllowed {})?;
            carrier
                .send_files()
                .await
                .map_err(|e| iroh::protocol::AcceptError::NotAllowed {})?;
            carrier.finish();
            info!("File transfer completed");
            Ok(())
        }
    }
}

struct Carrier {
    is_finished: Arc<AtomicBool>,
    config: SenderConfig,
    profile: Profile,
    connection: Connection,
    files: Vec<File>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    bytes_sent: Arc<AtomicU64>,
    bytes_compressed: Arc<AtomicU64>,
    start_time: Arc<RwLock<Option<Instant>>>,
    active_streams: Arc<AtomicU32>,
}

impl Carrier {
    async fn greet(&self) -> Result<()> {
        let mut bi = self.connection.accept_bi().await?;
        debug!("Starting handshake process");
        self.send_handshake(&mut bi).await?;
        self.receive_handshake(&mut bi).await?;
        bi.0.finish()?;
        bi.0.stopped().await?;
        info!("Handshake completed successfully");
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
        };

        let serialized_handshake = serde_json::to_vec(&handshake)?;
        let serialized_handshake_len = serialized_handshake.len() as u32;
        let serialized_handshake_header =
            serialized_handshake_len.to_be_bytes();

        bi.0.write_all(&serialized_handshake_header)
            .await?;
        bi.0.write_all(&serialized_handshake).await?;

        debug!("Sent handshake with {} files", handshake.files.len());
        Ok(())
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

        debug!(
            "Received handshake from receiver: {}",
            handshake.profile.name
        );

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

        Ok(())
    }

    async fn send_files(&self) -> Result<()> {
        info!("Starting file transfer for {} files", self.files.len());

        // Record start time
        {
            let mut start_time = self.start_time.write().unwrap();
            *start_time = Some(Instant::now());
        }

        for file in &self.files {
            let connection = self.connection.clone();
            let config = self.config.clone();
            let file_clone = file.clone();
            let subscribers = self.subscribers.clone();
            let bytes_sent = self.bytes_sent.clone();
            let bytes_compressed = self.bytes_compressed.clone();
            let start_time = self.start_time.clone();
            let active_streams = self.active_streams.clone();

            Self::send_single_file(
                    connection,
                    config,
                    file_clone,
                    subscribers,
                    bytes_sent,
                    bytes_compressed,
                    start_time,
                    active_streams.clone(),
                )
                .await?;
        }

        // Close connection with success code
        // self.connection.close(
        //     VarInt::from_u32(200),
        //     String::from("Transfer finished.").as_bytes(),
        // );

        info!("All files transferred successfully");
        Ok(())
    }

    async fn send_single_file(
        connection: Connection,
        config: SenderConfig,
        file: File,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
        bytes_sent: Arc<AtomicU64>,
        bytes_compressed: Arc<AtomicU64>,
        start_time: Arc<RwLock<Option<Instant>>>,
        active_streams: Arc<AtomicU32>,
    ) -> Result<()> {
        debug!("Starting transfer for file: {}", file.name);

        let mut sent = 0u64;
        let total_size = file.data.len();
        let mut remaining = total_size;

        // Initial progress notification
        Self::notify_progress(
            &subscribers,
            &file.name,
            sent,
            remaining,
            0.0,
            1.0,
            &start_time,
            &active_streams,
        )
        .await;

        // Open unidirectional stream for this file
        let mut uni = connection.open_uni().await?;

        loop {
            let projection = Self::read_next_projection(&file, &config);
            if projection.is_none() {
                break;
            }

            let projection = projection.unwrap();
            let original_size = projection.data.len() as u64;

            // Apply compression if enabled
            let (final_data, compression_ratio) = if config.compression_enabled
            {
                Self::compress_data(&projection.data)?
            } else {
                (projection.data.clone(), 1.0)
            };

            let compressed_size = final_data.len() as u64;

            // Create projection with compressed data
            let projection = FileProjection {
                id: projection.id,
                data: final_data,
            };

            // Serialize and send projection with larger buffer
            let serialized_projection = serde_json::to_vec(&projection)?;
            let serialized_projection_len = serialized_projection.len() as u32;
            let serialized_projection_header =
                serialized_projection_len.to_be_bytes();

            uni.write_all(&serialized_projection_header)
                .await?;
            uni.write_all(&serialized_projection).await?;

            // Update counters
            sent += original_size;
            remaining = if remaining >= original_size {
                remaining - original_size
            } else {
                0
            };

            bytes_sent
                .fetch_add(original_size, std::sync::atomic::Ordering::Relaxed);
            bytes_compressed.fetch_add(
                compressed_size,
                std::sync::atomic::Ordering::Relaxed,
            );

            // Calculate and notify progress with compression info
            Self::notify_progress(
                &subscribers,
                &file.name,
                sent,
                remaining,
                0.0,
                compression_ratio,
                &start_time,
                &active_streams,
            )
            .await;
        }

        uni.stopped().await?;

        debug!(
            "Completed transfer for file: {} ({} bytes)",
            file.name, sent
        );
        Ok(())
    }

    async fn notify_progress(
        subscribers: &Arc<
            RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>,
        >,
        file_name: &str,
        sent: u64,
        remaining: u64,
        _throughput_mbps: f64,
        compression_ratio: f64,
        start_time: &Arc<RwLock<Option<Instant>>>,
        active_streams: &Arc<AtomicU32>,
    ) {
        let actual_throughput = {
            let start_time_guard = start_time.read().unwrap();
            if let Some(start) = *start_time_guard {
                let elapsed = start.elapsed();
                if elapsed.as_secs() > 0 {
                    (sent as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        let current_active_streams =
            active_streams.load(std::sync::atomic::Ordering::Relaxed);

        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_sending(SendFilesSendingEvent {
                    name: file_name.to_string(),
                    sent,
                    remaining,
                    throughput_mbps: actual_throughput,
                    compression_ratio,
                    active_streams: current_active_streams,
                });
            });
    }

    fn read_next_projection(
        file: &File,
        config: &SenderConfig,
    ) -> Option<FileProjection> {
        let buffer = file.data.read_chunk(config.chunk_size);

        if buffer.is_empty() {
            return None;
        }

        Some(FileProjection {
            id: file.id.clone(),
            data: buffer.clone(),
        })
    }

    fn compress_data(data: &[u8]) -> Result<(Vec<u8>, f64)> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;

        let compression_ratio = if data.len() > 0 {
            compressed.len() as f64 / data.len() as f64
        } else {
            1.0
        };

        Ok((compressed, compression_ratio))
    }

    fn finish(&self) {
        info!("Finishing transfer");
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

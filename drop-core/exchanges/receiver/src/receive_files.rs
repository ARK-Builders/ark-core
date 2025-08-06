use anyhow::Result;
use drop_entities::Profile;
use dropx_common::{
    FileProjection, HandshakeProfile, ReceiverHandshake, SenderHandshake,
};
use flate2::read::GzDecoder;
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
    io::Read,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64},
    },
    time::Instant,
};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{ReceiverConfig, ReceiverProfile};

pub struct ReceiveFilesRequest {
    pub ticket: String,
    pub confirmation: u8,
    pub profile: ReceiverProfile,
    pub config: ReceiverConfig,
}

pub struct ReceiveFilesBubble {
    profile: Profile,
    endpoint: Endpoint,
    connection: Connection,
    config: ReceiverConfig,
    is_running: Arc<AtomicBool>,
    is_consumed: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
    bytes_received: Arc<AtomicU64>,
    bytes_decompressed: Arc<AtomicU64>,
    start_time: Arc<RwLock<Option<Instant>>>,
    active_streams: Arc<AtomicU32>,
}

impl ReceiveFilesBubble {
    pub fn new(
        profile: Profile,
        endpoint: Endpoint,
        connection: Connection,
        config: ReceiverConfig,
    ) -> Self {
        Self {
            profile,
            endpoint,
            connection,
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            is_consumed: Arc::new(AtomicBool::new(false)),
            is_finished: Arc::new(AtomicBool::new(false)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            bytes_received: Arc::new(AtomicU64::new(0)),
            bytes_decompressed: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(RwLock::new(None)),
            active_streams: Arc::new(AtomicU32::new(0)),
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

        // Record start time
        {
            let mut start_time = self.start_time.write().unwrap();
            *start_time = Some(Instant::now());
        }

        let carrier = Carrier {
            profile: self.profile.clone(),
            endpoint: self.endpoint.clone(),
            connection: self.connection.clone(),
            config: self.config.clone(),
            is_running: self.is_running.clone(),
            is_finished: self.is_finished.clone(),
            is_cancelled: self.is_cancelled.clone(),
            subscribers: self.subscribers.clone(),
            bytes_received: self.bytes_received.clone(),
            bytes_decompressed: self.bytes_decompressed.clone(),
            start_time: self.start_time.clone(),
            active_streams: self.active_streams.clone(),
        };

        tokio::spawn(async move {
            info!("Starting file reception");
            if let Err(e) = carrier.greet().await {
                error!("Handshake failed: {}", e);
                return;
            }

            let result = carrier.receive_files().await;
            if result.is_ok() {
                carrier
                    .is_finished
                    .store(true, std::sync::atomic::Ordering::Release);
                info!("File reception completed successfully");
            } else {
                error!("File reception failed: {:?}", result);
            }

            carrier.endpoint.close().await;
            carrier
                .is_running
                .store(false, std::sync::atomic::Ordering::Release);
        });

        Ok(())
    }

    pub fn cancel(&self) {
        if !self.is_running() || self.is_finished() {
            return;
        }
        info!("Cancelling file reception");
        self.is_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn is_running(&self) -> bool {
        self.is_running
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn is_consumed(&self) -> bool {
        self.is_consumed
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber.get_id(), subscriber);
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        self.subscribers
            .write()
            .unwrap()
            .remove(&subscriber.get_id());
    }

    pub fn get_performance_metrics(&self) -> String {
        let bytes_received = self
            .bytes_received
            .load(std::sync::atomic::Ordering::Relaxed);
        let bytes_decompressed = self
            .bytes_decompressed
            .load(std::sync::atomic::Ordering::Relaxed);
        let active_streams = self
            .active_streams
            .load(std::sync::atomic::Ordering::Relaxed);

        let decompression_ratio = if bytes_received > 0 {
            bytes_decompressed as f64 / bytes_received as f64
        } else {
            1.0
        };

        let throughput = {
            let start_time_guard = self.start_time.read().unwrap();
            if let Some(start) = *start_time_guard {
                let elapsed = start.elapsed();
                if elapsed.as_secs() > 0 {
                    (bytes_received as f64 / (1024.0 * 1024.0))
                        / elapsed.as_secs_f64()
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        format!(
            "Bytes received: {}, Decompression ratio: {:.2}, Throughput: {:.2} MB/s, Active streams: {}",
            bytes_received, decompression_ratio, throughput, active_streams
        )
    }
}

struct Carrier {
    profile: Profile,
    endpoint: Endpoint,
    connection: Connection,
    config: ReceiverConfig,
    is_running: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
    bytes_received: Arc<AtomicU64>,
    bytes_decompressed: Arc<AtomicU64>,
    start_time: Arc<RwLock<Option<Instant>>>,
    active_streams: Arc<AtomicU32>,
}

impl Carrier {
    async fn greet(&self) -> Result<()> {
        debug!("Starting handshake process");
        let mut bi = self.connection.open_bi().await?;
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
        let handshake = ReceiverHandshake {
            profile: HandshakeProfile {
                id: self.profile.id.clone(),
                name: self.profile.name.clone(),
                avatar_b64: self.profile.avatar_b64.clone(),
            },
        };
        let serialized_handshake = serde_json::to_vec(&handshake)?;
        let serialized_handshake_len = serialized_handshake.len() as u32;
        let serialized_handshake_header =
            serialized_handshake_len.to_be_bytes();

        bi.0.write_all(&serialized_handshake_header)
            .await?;
        bi.0.write_all(&serialized_handshake).await?;

        debug!("Sent receiver handshake");
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

        let handshake: SenderHandshake =
            serde_json::from_slice(&serialized_handshake)?;

        debug!("Received handshake from sender: {}", handshake.profile.name);

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

        Ok(())
    }

    async fn receive_files(&self) -> Result<()> {
        info!("Starting file reception");

        // Create semaphore for concurrent stream processing
        let semaphore = Arc::new(Semaphore::new(
            self.config.max_concurrent_streams as usize,
        ));
        let mut stream_tasks = Vec::new();

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
                        reason: String::from("Transfer finished.").into(),
                    },
                )) {
                    info!("Sender completed transfer");
                    break;
                }
                error!("Connection unexpectedly closed: {:?}", err);
                return Err(anyhow::Error::msg(
                    "Connection unexpectedly closed.",
                ));
            }

            let uni = uni_result.unwrap();
            let sem = semaphore.clone();
            let config = self.config.clone();
            let subscribers = self.subscribers.clone();
            let bytes_received = self.bytes_received.clone();
            let bytes_decompressed = self.bytes_decompressed.clone();
            let start_time = self.start_time.clone();
            let active_streams = self.active_streams.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                active_streams
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let result = Self::process_stream(
                    uni,
                    config,
                    subscribers,
                    bytes_received,
                    bytes_decompressed,
                    start_time,
                    active_streams.clone(),
                )
                .await;

                active_streams
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                result
            });

            stream_tasks.push(task);

            // Limit the number of concurrent tasks to prevent memory issues
            if stream_tasks.len()
                >= (self.config.max_concurrent_streams * 2) as usize
            {
                // Wait for some tasks to complete
                let selected = futures::future::select_all(stream_tasks).await;
                match selected.0 {
                    Ok(Ok(())) => {
                        debug!("Stream processing completed successfully")
                    }
                    Ok(Err(e)) => warn!("Stream processing error: {}", e),
                    Err(e) => warn!("Task join error: {}", e),
                }
                stream_tasks = selected.2;
            }
        }

        // Wait for all remaining tasks to complete
        for task in stream_tasks {
            match task.await {
                Ok(Ok(())) => continue,
                Ok(Err(e)) => warn!("Stream processing error: {}", e),
                Err(e) => warn!("Task join error: {}", e),
            }
        }

        info!("All streams processed successfully");
        Ok(())
    }

    async fn process_stream(
        mut uni: RecvStream,
        config: ReceiverConfig,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>,
        >,
        bytes_received: Arc<AtomicU64>,
        bytes_decompressed: Arc<AtomicU64>,
        start_time: Arc<RwLock<Option<Instant>>>,
        active_streams: Arc<AtomicU32>,
    ) -> Result<()> {
        let projection = Self::read_next_projection(&mut uni, &config).await?;
        if projection.is_none() {
            return Ok(());
        }

        let projection = projection.unwrap();
        let received_size = projection.data.len() as u64;

        // Apply decompression if enabled
        let (final_data, decompression_ratio) = if config.decompression_enabled
        {
            Self::decompress_data(&projection.data)?
        } else {
            (projection.data.clone(), 1.0)
        };

        let decompressed_size = final_data.len() as u64;

        // Update counters
        bytes_received
            .fetch_add(received_size, std::sync::atomic::Ordering::Relaxed);
        bytes_decompressed
            .fetch_add(decompressed_size, std::sync::atomic::Ordering::Relaxed);

        // Notify subscribers with event
        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(_, s)| {
                s.notify_receiving(ReceiveFilesReceivingEvent {
                    id: projection.id.clone(),
                    data: final_data.clone(),
                    received_bytes: received_size,
                    decompressed_bytes: decompressed_size,
                    decompression_ratio,
                    throughput_mbps: Self::calculate_throughput(
                        &start_time,
                        bytes_received
                            .load(std::sync::atomic::Ordering::Relaxed),
                    ),
                    active_streams: active_streams
                        .load(std::sync::atomic::Ordering::Relaxed),
                });
            });

        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn read_next_projection(
        uni: &mut RecvStream,
        config: &ReceiverConfig,
    ) -> Result<Option<FileProjection>> {
        let serialized_projection_len =
            Self::read_serialized_projection_len(uni).await?;
        if serialized_projection_len.is_none() {
            return Ok(None);
        }

        let len = serialized_projection_len.unwrap();
        let mut serialized_projection = Vec::with_capacity(len);
        serialized_projection.resize(len, 0);

        // Use larger buffer for reading
        let mut buffer = vec![0u8; config.buffer_size.min(len as u64) as usize];
        let mut total_read = 0;

        while total_read < len {
            let to_read = (len - total_read).min(buffer.len());
            let read = uni.read(&mut buffer[..to_read]).await?;

            if let Some(bytes_read) = read {
                if bytes_read == 0 {
                    break;
                }
                serialized_projection[total_read..total_read + bytes_read]
                    .copy_from_slice(&buffer[..bytes_read]);
                total_read += bytes_read;
            } else {
                break;
            }
        }

        if total_read != len {
            return Err(anyhow::Error::msg(
                "Incomplete projection data received",
            ));
        }

        let projection: FileProjection =
            serde_json::from_slice(&serialized_projection)?;
        Ok(Some(projection))
    }

    async fn read_serialized_projection_len(
        uni: &mut RecvStream,
    ) -> Result<Option<usize>> {
        let mut serialized_projection_header = [0u8; 4];
        let read = uni
            .read(&mut serialized_projection_header)
            .await?;

        if read.is_none() {
            return Ok(None);
        }

        if read.unwrap() != 4 {
            return Err(anyhow::Error::msg(
                "Invalid data chunk length header.",
            ));
        }

        let serialized_projection_len =
            u32::from_be_bytes(serialized_projection_header);
        Ok(Some(serialized_projection_len as usize))
    }

    fn decompress_data(data: &[u8]) -> Result<(Vec<u8>, f64)> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        let decompression_ratio = if data.len() > 0 {
            decompressed.len() as f64 / data.len() as f64
        } else {
            1.0
        };

        Ok((decompressed, decompression_ratio))
    }

    fn calculate_throughput(
        start_time: &Arc<RwLock<Option<Instant>>>,
        bytes_received: u64,
    ) -> f64 {
        let start_time_guard = start_time.read().unwrap();
        if let Some(start) = *start_time_guard {
            let elapsed = start.elapsed();
            if elapsed.as_secs() > 0 {
                (bytes_received as f64 / (1024.0 * 1024.0))
                    / elapsed.as_secs_f64()
            } else {
                0.0
            }
        } else {
            0.0
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
    pub received_bytes: u64,
    pub decompressed_bytes: u64,
    pub decompression_ratio: f64,
    pub throughput_mbps: f64,
    pub active_streams: u32,
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
    info!(
        "Starting file reception with config: max_streams={}, buffer_size={}, decompression={}",
        request.config.max_concurrent_streams,
        request.config.buffer_size,
        request.config.decompression_enabled
    );

    let ticket: NodeTicket = request.ticket.parse()?;

    let endpoint_builder = Endpoint::builder().discovery_n0();

    let endpoint = endpoint_builder.bind().await?;
    let connection = endpoint
        .connect(ticket, &[request.confirmation])
        .await?;

    Ok(ReceiveFilesBubble::new(
        Profile {
            id: Uuid::new_v4().to_string(),
            name: request.profile.name,
            avatar_b64: request.profile.avatar_b64,
        },
        endpoint,
        connection,
        request.config,
    ))
}

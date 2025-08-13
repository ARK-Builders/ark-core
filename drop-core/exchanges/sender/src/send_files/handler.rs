use anyhow::Result;
use drop_entities::{File, Profile};
use dropx_common::{
    handshake::{
        HandshakeFile, HandshakeProfile, ReceiverHandshake, SenderHandshake,
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

use super::SenderConfig;

pub trait SendFilesSubscriber: Send + Sync {
    fn get_id(&self) -> String;
    fn log(&self, message: String);
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
            .load(std::sync::atomic::Ordering::Acquire);
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

        let subscribers = self.subscribers.clone();

        async move {
            if is_consumed {
                // Log to subscribers before returning error
                subscribers
                    .read()
                    .unwrap()
                    .iter()
                    .for_each(|(id, subscriber)| {
                        subscriber.log(format!("[{}] on_connecting: Connection rejected - handler already consumed", id));
                    });
                return Err(iroh::protocol::AcceptError::NotAllowed {});
            }

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!("[{}] on_connecting: Attempting to establish connection", id));
                });

            let connection = connecting.await;

            match &connection {
                Ok(_) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] on_connecting: Connection successfully established", id));
                        });
                }
                Err(e) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] on_connecting: Connection failed with error: {}", id, e));
                        });
                }
            }

            Ok(connection?)
        }
    }

    fn shutdown(&self) -> impl Future<Output = ()> + Send {
        self.log("shutdown: Initiating handler shutdown".to_string());
        let is_finished = self.is_finished.clone();
        let subscribers = self.subscribers.clone();

        async move {
            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!(
                        "[{}] shutdown: Setting finished flag to true",
                        id
                    ));
                });

            is_finished.store(true, std::sync::atomic::Ordering::Relaxed);

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!(
                        "[{}] shutdown: Handler shutdown completed",
                        id
                    ));
                });
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
            profile: self.profile.clone(),
            connection,
            files: self.files.clone(),
            subscribers: self.subscribers.clone(),
        };

        let subscribers = self.subscribers.clone();

        async move {
            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!(
                        "[{}] accept: Starting file transfer process",
                        id
                    ));
                });

            let greet_result = carrier.greet().await;
            match &greet_result {
                Ok(_) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] accept: Greeting phase completed successfully", id));
                        });
                }
                Err(e) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] accept: Greeting phase failed with error: {}", id, e));
                        });
                    return Err(iroh::protocol::AcceptError::NotAllowed {});
                }
            }

            let send_result = carrier.send_files().await;
            match &send_result {
                Ok(_) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] accept: File sending phase completed successfully", id));
                        });
                }
                Err(e) => {
                    subscribers
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|(id, subscriber)| {
                            subscriber.log(format!("[{}] accept: File sending phase failed with error: {}", id, e));
                        });
                    return Err(iroh::protocol::AcceptError::NotAllowed {});
                }
            }

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!(
                        "[{}] accept: Finishing transfer process",
                        id
                    ));
                });

            carrier.finish();

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, subscriber)| {
                    subscriber.log(format!(
                        "[{}] accept: File transfer completed successfully",
                        id
                    ));
                });

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
}
impl Carrier {
    async fn greet(&self) -> Result<()> {
        self.log("greet: Starting greeting process".to_string());

        self.log("greet: Accepting bidirectional stream".to_string());
        let mut bi = match self.connection.accept_bi().await {
            Ok(bi) => {
                self.log(
                    "greet: Bidirectional stream accepted successfully"
                        .to_string(),
                );
                bi
            }
            Err(e) => {
                self.log(format!(
                    "greet: Failed to accept bidirectional stream: {}",
                    e
                ));
                return Err(e.into());
            }
        };

        self.log("greet: Starting handshake process".to_string());

        if let Err(e) = self.send_handshake(&mut bi).await {
            self.log(format!("greet: Sender handshake failed: {}", e));
            return Err(e);
        }
        self.log("greet: Sender handshake completed successfully".to_string());

        if let Err(e) = self.receive_handshake(&mut bi).await {
            self.log(format!("greet: Receiver handshake failed: {}", e));
            return Err(e);
        }
        self.log(
            "greet: Receiver handshake completed successfully".to_string(),
        );

        self.log("greet: Finishing send stream".to_string());
        bi.0.finish()?;

        self.log("greet: Waiting for send stream to stop".to_string());
        bi.0.stopped().await?;

        // self.log("greet: Stopping receive stream".to_string());
        // bi.1.stop(VarInt::from_u32(0))?;

        self.log("greet: Handshake completed successfully".to_string());
        Ok(())
    }

    async fn send_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        self.log("send_handshake: Creating sender handshake".to_string());

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

        self.log(format!(
            "send_handshake: Handshake created - Profile: {} ({}), Files: {}",
            handshake.profile.name,
            handshake.profile.id,
            handshake.files.len()
        ));

        for (index, file) in handshake.files.iter().enumerate() {
            self.log(format!(
                "send_handshake: File {}: {} ({} bytes)",
                index + 1,
                file.name,
                file.len
            ));
        }

        self.log("send_handshake: Serializing handshake to JSON".to_string());
        let serialized_handshake = serde_json::to_vec(&handshake)?;
        let serialized_handshake_len = serialized_handshake.len() as u32;
        let serialized_handshake_header =
            serialized_handshake_len.to_be_bytes();

        self.log(format!(
            "send_handshake: Serialized handshake size: {} bytes",
            serialized_handshake_len
        ));

        self.log("send_handshake: Writing handshake header".to_string());
        bi.0.write_all(&serialized_handshake_header)
            .await?;

        self.log("send_handshake: Writing handshake payload".to_string());
        bi.0.write_all(&serialized_handshake).await?;

        self.log(format!(
            "send_handshake: Successfully sent handshake with {} files",
            handshake.files.len()
        ));
        Ok(())
    }

    async fn receive_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        self.log(
            "receive_handshake: Reading handshake header from receiver"
                .to_string(),
        );

        let mut serialized_handshake_header = [0u8; 4];
        bi.1.read_exact(&mut serialized_handshake_header)
            .await?;
        let serialized_handshake_len =
            u32::from_be_bytes(serialized_handshake_header);

        self.log(format!(
            "receive_handshake: Expected handshake size: {} bytes",
            serialized_handshake_len
        ));

        self.log(
            "receive_handshake: Reading handshake payload from receiver"
                .to_string(),
        );
        let mut serialized_handshake =
            vec![0u8; serialized_handshake_len as usize];
        bi.1.read_exact(&mut serialized_handshake).await?;

        self.log(format!(
            "receive_handshake: Successfully read {} bytes of handshake data",
            serialized_handshake.len()
        ));

        self.log(
            "receive_handshake: Deserializing handshake from JSON".to_string(),
        );
        let handshake: ReceiverHandshake =
            serde_json::from_slice(&serialized_handshake)?;

        self.log(format!(
            "receive_handshake: Received handshake from receiver - Name: {}, ID: {}",
            handshake.profile.name, handshake.profile.id
        ));

        self.log(
            "receive_handshake: Notifying subscribers about connecting event"
                .to_string(),
        );
        self.subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, s)| {
                self.log(format!("receive_handshake: Notifying subscriber {} about connection", id));
                s.notify_connecting(SendFilesConnectingEvent {
                    receiver: SendFilesProfile {
                        id: handshake.profile.id.clone(),
                        name: handshake.profile.name.clone(),
                        avatar_b64: handshake.profile.avatar_b64.clone(),
                    },
                });
            });

        self.log(
            "receive_handshake: Handshake exchange completed successfully"
                .to_string(),
        );
        Ok(())
    }

    async fn send_files(&self) -> Result<()> {
        self.log(format!(
            "send_files: Starting file transfer for {} files",
            self.files.len()
        ));

        for (file_index, file) in self.files.iter().enumerate() {
            self.log(format!(
                "send_files: Processing file {} of {}: {} ({} bytes)",
                file_index + 1,
                self.files.len(),
                file.name,
                file.data.len()
            ));

            let connection = self.connection.clone();
            let config = self.config.clone();
            let file_clone = file.clone();
            let subscribers = self.subscribers.clone();

            let send_result = Self::send_single_file(
                connection,
                config,
                file_clone,
                subscribers,
            )
            .await;
            match &send_result {
                Ok(_) => {
                    self.log(format!(
                        "send_files: File {} ({}) transferred successfully",
                        file_index + 1,
                        file.name
                    ));
                }
                Err(e) => {
                    self.log(format!(
                        "send_files: File {} ({}) transfer failed: {}",
                        file_index + 1,
                        file.name,
                        e
                    ));
                    return send_result;
                }
            }
        }

        self.log("send_files: All files transferred successfully".to_string());
        Ok(())
    }

    async fn send_single_file(
        connection: Connection,
        config: SenderConfig,
        file: File,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    ) -> Result<()> {
        // Helper function to log to subscribers
        let log =
            |message: String| {
                subscribers.read().unwrap().iter().for_each(
                    |(id, subscriber)| {
                        subscriber.log(format!("[{}] {}", id, message));
                    },
                );
            };

        log(format!(
            "send_single_file: Starting transfer for file: {} ({} bytes)",
            file.name,
            file.data.len()
        ));

        let mut sent = 0u64;
        let total_len = file.data.len();
        let mut remaining = total_len;
        let mut chunk_count = 0u64;

        log(format!(
            "send_single_file: File {} - Total size: {} bytes, Buffer size: {} bytes",
            file.name, total_len, config.buffer_size
        ));

        // Initial progress notification
        log(format!(
            "send_single_file: Sending initial progress notification for file {}",
            file.name
        ));
        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, s)| {
                log(format!("send_single_file: Notifying subscriber {} about initial progress", id));
                s.notify_sending(SendFilesSendingEvent {
                    name: file.name.clone(),
                    sent,
                    remaining,
                });
            });

        log(format!(
            "send_single_file: Starting chunk transfer loop for file {}",
            file.name
        ));

        loop {
            log(format!(
                "send_single_file: Reading next projection for file {} (chunk {})",
                file.name,
                chunk_count + 1
            ));
            let projection = Self::read_next_projection(&file, &config);
            if projection.is_none() {
                log(format!(
                    "send_single_file: No more data to read for file {} - transfer complete",
                    file.name
                ));
                break;
            }

            let projection = projection.unwrap();
            let data_len = projection.data.len() as u64;
            chunk_count += 1;

            log(format!(
                "send_single_file: Read chunk {} for file {} - size: {} bytes",
                chunk_count, file.name, data_len
            ));

            // Open unidirectional stream for this chunk
            log(format!(
                "send_single_file: Opening unidirectional stream for chunk {} of file {}",
                chunk_count, file.name
            ));
            let mut uni = connection.open_uni().await?;
            log(format!(
                "send_single_file: Successfully opened stream for chunk {} of file {}",
                chunk_count, file.name
            ));

            let projection = FileProjection {
                id: projection.id,
                data: projection.data,
            };

            // Serialize and send projection with larger buffer
            log(format!(
                "send_single_file: Serializing projection for chunk {} of file {}",
                chunk_count, file.name
            ));
            let serialized_projection = serde_json::to_vec(&projection)?;
            let serialized_projection_len = serialized_projection.len() as u32;
            let serialized_projection_header =
                serialized_projection_len.to_be_bytes();

            log(format!(
                "send_single_file: Serialized projection size: {} bytes for chunk {} of file {}",
                serialized_projection_len, chunk_count, file.name
            ));

            log(format!(
                "send_single_file: Writing projection header for chunk {} of file {}",
                chunk_count, file.name
            ));
            uni.write_all(&serialized_projection_header)
                .await?;

            log(format!(
                "send_single_file: Writing projection data for chunk {} of file {}",
                chunk_count, file.name
            ));
            uni.write_all(&serialized_projection).await?;

            // Properly finish the stream to signal end of data
            log(format!(
                "send_single_file: Finishing stream for chunk {} of file {}",
                chunk_count, file.name
            ));
            uni.finish()?;

            // Wait for the stream to be acknowledged as stopped
            log(format!(
                "send_single_file: Waiting for stream to stop for chunk {} of file {}",
                chunk_count, file.name
            ));
            uni.stopped().await?;
            log(format!(
                "send_single_file: Stream stopped for chunk {} of file {}",
                chunk_count, file.name
            ));

            // Update counters
            sent += data_len;
            remaining = if remaining >= data_len {
                remaining - data_len
            } else {
                0
            };

            let progress = if total_len > 0 {
                (sent as f64 / total_len as f64) * 100.0
            } else {
                100.0
            };

            log(format!(
                "send_single_file: Progress for file {}: {:.1}% ({}/{} bytes, chunk {})",
                file.name, progress, sent, total_len, chunk_count
            ));

            subscribers
                .read()
                .unwrap()
                .iter()
                .for_each(|(id, s)| {
                    log(format!("send_single_file: Notifying subscriber {} about progress update", id));
                    s.notify_sending(SendFilesSendingEvent {
                        name: file.name.clone(),
                        sent,
                        remaining,
                    });
                });
        }

        log(format!(
            "send_single_file: Completed transfer for file: {} ({} bytes in {} chunks)",
            file.name, sent, chunk_count
        ));
        Ok(())
    }

    fn read_next_projection(
        file: &File,
        config: &SenderConfig,
    ) -> Option<FileProjection> {
        let buffer = file.data.read_chunk(config.buffer_size);

        if buffer.is_empty() {
            return None;
        }

        Some(FileProjection {
            id: file.id.clone(),
            data: buffer,
        })
    }

    fn finish(&self) {
        self.log("finish: Starting transfer finish process".to_string());

        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.log("finish: Transfer finished flag set to true".to_string());
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
}

use anyhow::Result;
use drop_entities::{File, Profile};
use dropx_common::{
    FileProjection, HandshakeFile, HandshakeProfile, ReceiverHandshake,
    SenderHandshake,
};
use futures::Future;
use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    protocol::ProtocolHandler,
};
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock, atomic::AtomicBool},
};
use tracing::{debug, info};

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
        info!("Creating send files handler with {} files", files.len());
        Self {
            is_consumed: AtomicBool::new(false),
            is_finished: Arc::new(AtomicBool::new(false)),
            profile,
            files,
            config,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
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
        };

        async move {
            info!("Starting file transfer process");
            carrier
                .greet()
                .await
                // TODO: handle error
                .map_err(|_e| iroh::protocol::AcceptError::NotAllowed {})?;
            carrier
                .send_files()
                .await
                // TODO: handle error
                .map_err(|_e| iroh::protocol::AcceptError::NotAllowed {})?;
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

        for file in &self.files {
            let connection = self.connection.clone();
            let config = self.config.clone();
            let file_clone = file.clone();
            let subscribers = self.subscribers.clone();

            Self::send_single_file(connection, config, file_clone, subscribers)
                .await?;
        }

        info!("All files transferred successfully");
        Ok(())
    }

    async fn send_single_file(
        connection: Connection,
        config: SenderConfig,
        file: File,
        subscribers: Arc<RwLock<HashMap<String, Arc<dyn SendFilesSubscriber>>>>,
    ) -> Result<()> {
        debug!("Starting transfer for file: {}", file.name);

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

        loop {
            let projection = Self::read_next_projection(&file, &config);
            if projection.is_none() {
                break;
            }

            let projection = projection.unwrap();
            let data_len = projection.data.len() as u64;

            // Open unidirectional stream for this chunk
            let mut uni = connection.open_uni().await?;

            let projection = FileProjection {
                id: projection.id,
                data: projection.data,
            };

            // Serialize and send projection with larger buffer
            let serialized_projection = serde_json::to_vec(&projection)?;
            let serialized_projection_len = serialized_projection.len() as u32;
            let serialized_projection_header =
                serialized_projection_len.to_be_bytes();

            uni.write_all(&serialized_projection_header)
                .await?;
            uni.write_all(&serialized_projection).await?;
            uni.finish()?;

            // Update counters
            sent += data_len;
            remaining = if remaining >= data_len {
                remaining - data_len
            } else {
                0
            };

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

        debug!(
            "Completed transfer for file: {} ({} bytes)",
            file.name, sent
        );
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
        info!("Finishing transfer");
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

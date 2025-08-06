use anyhow::Result;
use drop_entities::Profile;
use dropx_common::{
    FileProjection, HandshakeProfile, ReceiverHandshake, SenderHandshake,
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
};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::ReceiverProfile;

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
}
impl ReceiveFilesBubble {
    pub fn new(
        profile: Profile,
        endpoint: Endpoint,
        connection: Connection,
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

            // Close connection with success code
            carrier.connection.close(
                VarInt::from_u32(200),
                String::from("Transfer finished.").as_bytes(),
            );

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
}

struct Carrier {
    profile: Profile,
    endpoint: Endpoint,
    connection: Connection,
    is_running: Arc<AtomicBool>,
    is_finished: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
    subscribers: Arc<RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>>,
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
            let subscribers = self.subscribers.clone();

            Self::process_stream(uni, subscribers).await?;
        }

        info!("All streams processed successfully");
        Ok(())
    }

    async fn process_stream(
        mut uni: RecvStream,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>,
        >,
    ) -> Result<()> {
        let projection = Self::read_next_projection(&mut uni).await?;
        if projection.is_none() {
            return Ok(());
        }

        let projection = projection.unwrap();

        // Notify subscribers with event
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

        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn read_next_projection(
        uni: &mut RecvStream,
    ) -> Result<Option<FileProjection>> {
        let serialized_projection_len =
            Self::read_serialized_projection_len(uni).await?;
        if serialized_projection_len.is_none() {
            return Ok(None);
        }

        let len = serialized_projection_len.unwrap();
        let mut serialized_projection = vec![0u8; len];
        uni.read_exact(&mut serialized_projection).await?;

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
    ))
}

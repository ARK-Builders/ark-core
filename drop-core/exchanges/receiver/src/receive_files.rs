use anyhow::Result;
use drop_entities::Profile;
use dropx_common::{
    handshake::{HandshakeProfile, ReceiverHandshake, SenderHandshake},
    projection::FileProjection,
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
        self.log("start: Checking if transfer can be started".to_string());

        if self.is_running() || self.is_consumed() || self.is_finished() {
            self.log(format!("start: Cannot start transfer - running: {}, consumed: {}, finished: {}", 
                self.is_running(), self.is_consumed(), self.is_finished()));
            return Err(anyhow::Error::msg(
                "Already running or has run or finished.",
            ));
        }

        self.log("start: Setting running and consumed flags".to_string());
        self.is_running
            .store(true, std::sync::atomic::Ordering::Release);
        self.is_consumed
            .store(true, std::sync::atomic::Ordering::Release);

        self.log("start: Creating carrier for file reception".to_string());
        let carrier = Carrier {
            profile: self.profile.clone(),
            endpoint: self.endpoint.clone(),
            connection: self.connection.clone(),
            is_running: self.is_running.clone(),
            is_finished: self.is_finished.clone(),
            is_cancelled: self.is_cancelled.clone(),
            subscribers: self.subscribers.clone(),
        };

        self.log("start: Spawning async task for file reception".to_string());
        tokio::spawn(async move {
            carrier.log(
                "start: File reception task started".to_string(),
            );

            carrier.log(
                "start: Beginning handshake process".to_string(),
            );
            if let Err(e) = carrier.greet().await {
                carrier.log(format!(
                    "start: Handshake failed: {}",
                    e
                ));
                return;
            }
            carrier.log("start: Handshake completed successfully, starting file reception".to_string());

            let result = carrier.receive_files().await;
            match &result {
                Ok(_) => {
                    carrier.log(
                        "start: File reception completed successfully"
                            .to_string(),
                    );
                    carrier
                        .is_finished
                        .store(true, std::sync::atomic::Ordering::Release);
                }
                Err(e) => {
                    carrier.log(format!(
                        "start: File reception failed: {}",
                        e
                    ));
                }
            }

            carrier.log(
                "start: Closing connection with success code".to_string(),
            );
            // Close connection with success code
            carrier.connection.close(
                VarInt::from_u32(200),
                String::from("Transfer finished.").as_bytes(),
            );

            carrier.log("start: Closing endpoint".to_string());
            carrier.endpoint.close().await;

            carrier.log(
                "start: Setting running flag to false".to_string(),
            );
            carrier
                .is_running
                .store(false, std::sync::atomic::Ordering::Release);

            carrier.log(
                "start: File reception task completed".to_string(),
            );
        });

        self.log("start: File reception started successfully".to_string());
        Ok(())
    }

    pub fn cancel(&self) {
        self.log("cancel: Checking if transfer can be cancelled".to_string());

        if !self.is_running() || self.is_finished() {
            self.log(format!(
                "cancel: Cannot cancel - not running: {} or finished: {}",
                !self.is_running(),
                self.is_finished()
            ));
            return;
        }

        self.log("cancel: Setting cancelled flag to true".to_string());
        self.is_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.log("cancel: File reception cancellation requested".to_string());
    }

    fn is_running(&self) -> bool {
        let running = self
            .is_running
            .load(std::sync::atomic::Ordering::Acquire);
        self.log(format!("is_running check: {}", running));
        running
    }

    fn is_consumed(&self) -> bool {
        let consumed = self
            .is_consumed
            .load(std::sync::atomic::Ordering::Acquire);
        self.log(format!("is_consumed check: {}", consumed));
        consumed
    }

    pub fn is_finished(&self) -> bool {
        let finished = self
            .is_finished
            .load(std::sync::atomic::Ordering::Acquire);
        self.log(format!("is_finished check: {}", finished));
        finished
    }

    pub fn is_cancelled(&self) -> bool {
        let cancelled = self
            .is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_cancelled check: {}", cancelled));
        cancelled
    }

    pub fn subscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "subscribe: Subscribing new subscriber with ID: {}",
            subscriber_id
        ));

        self.subscribers
            .write()
            .unwrap()
            .insert(subscriber_id.clone(), subscriber);

        self.log(format!("subscribe: Subscriber {} successfully subscribed. Total subscribers: {}", 
            subscriber_id, self.subscribers.read().unwrap().len()));
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn ReceiveFilesSubscriber>) {
        let subscriber_id = subscriber.get_id();
        self.log(format!(
            "unsubscribe: Unsubscribing subscriber with ID: {}",
            subscriber_id
        ));

        let removed = self
            .subscribers
            .write()
            .unwrap()
            .remove(&subscriber_id);

        if removed.is_some() {
            self.log(format!("unsubscribe: Subscriber {} successfully unsubscribed. Remaining subscribers: {}", 
                subscriber_id, self.subscribers.read().unwrap().len()));
        } else {
            self.log(format!("unsubscribe: Subscriber {} was not found during unsubscribe operation", subscriber_id));
        }
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
        self.log(
            "greet: Starting handshake process".to_string(),
        );

        self.log(
            "greet: Opening bidirectional stream".to_string(),
        );
        let mut bi = match self.connection.open_bi().await {
            Ok(bi) => {
                self.log(
                    "greet: Bidirectional stream opened successfully"
                        .to_string(),
                );
                bi
            }
            Err(e) => {
                self.log(format!(
                    "greet: Failed to open bidirectional stream: {}",
                    e
                ));
                return Err(e.into());
            }
        };

        self.log(
            "greet: Sending receiver handshake".to_string(),
        );
        if let Err(e) = self.send_handshake(&mut bi).await {
            self.log(format!(
                "greet: Receiver handshake failed: {}",
                e
            ));
            return Err(e);
        }
        self.log(
            "greet: Receiver handshake sent successfully".to_string(),
        );

        self.log(
            "greet: Receiving sender handshake".to_string(),
        );
        if let Err(e) = self.receive_handshake(&mut bi).await {
            self.log(format!(
                "greet: Sender handshake reception failed: {}",
                e
            ));
            return Err(e);
        }
        self.log(
            "greet: Sender handshake received successfully".to_string(),
        );

        self.log(
            "greet: Finishing send stream".to_string(),
        );
        bi.0.finish()?;

        self.log(
            "greet: Stopping receive stream".to_string(),
        );
        bi.1.stop(VarInt::from_u32(0))?;

        self.log(
            "greet: Waiting for send stream to stop".to_string(),
        );
        bi.0.stopped().await?;

        self.log(
            "greet: Handshake completed successfully".to_string(),
        );
        Ok(())
    }

    async fn send_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        self.log(
            "send_handshake: Creating receiver handshake".to_string(),
        );

        let handshake = ReceiverHandshake {
            profile: HandshakeProfile {
                id: self.profile.id.clone(),
                name: self.profile.name.clone(),
                avatar_b64: self.profile.avatar_b64.clone(),
            },
        };

        self.log(format!(
            "send_handshake: Handshake created - Profile: {} ({})",
            handshake.profile.name, handshake.profile.id
        ));

        self.log(
            "send_handshake: Serializing handshake to JSON".to_string(),
        );
        let serialized_handshake = serde_json::to_vec(&handshake)?;
        let serialized_handshake_len = serialized_handshake.len() as u32;
        let serialized_handshake_header =
            serialized_handshake_len.to_be_bytes();

        self.log(format!(
            "send_handshake: Serialized handshake size: {} bytes",
            serialized_handshake_len
        ));

        self.log(
            "send_handshake: Writing handshake header".to_string(),
        );
        bi.0.write_all(&serialized_handshake_header)
            .await?;

        self.log(
            "send_handshake: Writing handshake payload".to_string(),
        );
        bi.0.write_all(&serialized_handshake).await?;

        self.log(
            "send_handshake: Receiver handshake sent successfully".to_string(),
        );
        Ok(())
    }

    async fn receive_handshake(
        &self,
        bi: &mut (SendStream, RecvStream),
    ) -> Result<()> {
        self.log(
            "receive_handshake: Reading handshake header from sender"
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
            "receive_handshake: Reading handshake payload from sender"
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
        let handshake: SenderHandshake =
            serde_json::from_slice(&serialized_handshake)?;

        self.log(format!(
            "receive_handshake: Received handshake from sender - Name: {}, ID: {}, Files: {}",
            handshake.profile.name, handshake.profile.id, handshake.files.len()
        ));

        for (index, file) in handshake.files.iter().enumerate() {
            self.log(format!(
                "receive_handshake: File {}: {} ({} bytes)",
                index + 1,
                file.name,
                file.len
            ));
        }

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

        self.log(
            "receive_handshake: Handshake exchange completed successfully"
                .to_string(),
        );
        Ok(())
    }

    async fn receive_files(&self) -> Result<()> {
        self.log(
            "receive_files: Starting file reception loop".to_string(),
        );
        let mut stream_count = 0u64;

        loop {
            if self.is_cancelled() {
                self.log(
                    "receive_files: Cancellation detected, closing connection"
                        .to_string(),
                );
                self.connection.close(
                    VarInt::from_u32(0),
                    String::from("Receive files has been cancelled.")
                        .as_bytes(),
                );
                return Err(anyhow::Error::msg(
                    "Receive files has been cancelled.",
                ));
            }

            self.log(format!("receive_files: Waiting for unidirectional stream {} from sender", stream_count + 1));
            let uni_result = self.connection.accept_uni().await;

            match uni_result {
                Ok(uni) => {
                    stream_count += 1;
                    self.log(format!(
                        "receive_files: Accepted unidirectional stream {}",
                        stream_count
                    ));

                    let subscribers = self.subscribers.clone();

                    let process_result =
                        Self::process_stream(uni, subscribers).await;
                    match &process_result {
                        Ok(_) => {
                            self.log(format!("receive_files: Stream {} processed successfully", stream_count));
                        }
                        Err(e) => {
                            self.log(format!("receive_files: Stream {} processing failed: {}", stream_count, e));
                            return process_result;
                        }
                    }
                }
                Err(err) => {
                    if err.eq(&ConnectionError::ApplicationClosed(
                        ApplicationClose {
                            error_code: VarInt::from_u32(200),
                            reason: String::from("Transfer finished.").into(),
                        },
                    )) {
                        self.log("receive_files: Sender completed transfer with success code".to_string());
                        break;
                    }
                    self.log(format!(
                        "receive_files: Connection unexpectedly closed: {:?}",
                        err
                    ));
                    return Err(anyhow::Error::msg(
                        "Connection unexpectedly closed.",
                    ));
                }
            }
        }

        self.log(format!(
            "receive_files: All {} streams processed successfully",
            stream_count
        ));
        Ok(())
    }

    async fn process_stream(
        mut uni: RecvStream,
        subscribers: Arc<
            RwLock<HashMap<String, Arc<dyn ReceiveFilesSubscriber>>>,
        >,
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

        log(
            "process_stream: Starting stream processing".to_string(),
        );

        log(
            "process_stream: Reading projection data from stream".to_string(),
        );
        let projection_result = Self::read_next_projection(&mut uni).await;

        let projection = match projection_result {
            Ok(Some(proj)) => {
                log(format!(
                    "process_stream: Successfully read projection - File ID: {}, Data size: {} bytes",
                    proj.id,
                    proj.data.len()
                ));
                proj
            }
            Ok(None) => {
                log("process_stream: No projection data found in stream (empty stream)".to_string());
                return Ok(());
            }
            Err(e) => {
                log(format!(
                    "process_stream: Failed to read projection: {}",
                    e
                ));
                return Err(e);
            }
        };

        log(format!(
            "process_stream: Notifying {} subscribers about received data",
            subscribers.read().unwrap().len()
        ));

        // Notify subscribers with event
        subscribers
            .read()
            .unwrap()
            .iter()
            .for_each(|(id, s)| {
                log(format!("process_stream: Notifying subscriber {} about {} bytes received for file {}", 
                    id, projection.data.len(), projection.id));
                s.notify_receiving(ReceiveFilesReceivingEvent {
                    id: projection.id.clone(),
                    data: projection.data.clone(),
                });
            });

        log("process_stream: Stopping unidirectional stream to signal completion".to_string());
        uni.stop(VarInt::from_u32(0))?;

        log(
            "process_stream: Stream processing completed successfully"
                .to_string(),
        );
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        let cancelled = self
            .is_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.log(format!("is_cancelled check: {}", cancelled));
        cancelled
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
    fn log(&self, message: String);
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

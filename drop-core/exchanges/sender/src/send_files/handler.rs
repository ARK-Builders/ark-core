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
    pin::Pin,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

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
            limiter: 1024, // TODO: FLEXIBILIZE CHUNK LIMITER
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
                    },
                });
            });
        return Ok(());
    }

    async fn send_files(&self) -> Result<()> {
        for file in &self.files {
            let mut sent = 0;
            let mut remaining = file.data.len();
            self.subscribers
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
                let projection = self.read_next_projection(file);
                if projection.is_none() {
                    break;
                }
                let projection = projection.unwrap();
                let projection_data_len = projection.data.len() as u64;
                let mut uni = self.connection.open_uni().await?;
                let serialized_projection =
                    serde_json::to_vec(&projection).unwrap();
                let serialized_projection_len =
                    serialized_projection.len() as u16;
                let serialized_projection_header =
                    serialized_projection_len.to_be_bytes();
                uni.write_all(&serialized_projection_header)
                    .await?;
                uni.write_all(&serialized_projection).await?;
                uni.finish()?;
                sent += projection_data_len;
                if remaining >= projection_data_len {
                    remaining -= projection_data_len
                } else {
                    remaining = 0
                }
                self.subscribers
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
        }
        self.connection
            .close(VarInt::from_u32(200), String::from("Finished.").as_bytes());
        return Ok(());
    }

    fn read_next_projection(&self, file: &File) -> Option<FileProjection> {
        let mut data = Vec::new();
        for _ in 0..self.limiter {
            let b = file.data.read();
            if b.is_none() {
                break;
            }
            data.push(b.unwrap());
        }
        if data.len() == 0 {
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

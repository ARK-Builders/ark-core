use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeProfile {
    pub id: String,
    pub name: String,
    pub avatar_b64: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeFile {
    pub id: String,
    pub name: String,
    pub len: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeConfig {
    pub chunk_size: u64,
    pub parallel_streams: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SenderHandshake {
    pub profile: HandshakeProfile,
    pub files: Vec<HandshakeFile>,
    pub config: HandshakeConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiverHandshake {
    pub profile: HandshakeProfile,
    pub config: HandshakeConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiatedConfig {
    pub chunk_size: u64,
    pub parallel_streams: u64,
}

impl NegotiatedConfig {
    pub fn negotiate(
        sender_config: &HandshakeConfig,
        receiver_config: &HandshakeConfig,
    ) -> Self {
        Self {
            chunk_size: sender_config
                .chunk_size
                .min(receiver_config.chunk_size),
            parallel_streams: sender_config
                .parallel_streams
                .min(receiver_config.parallel_streams),
        }
    }
}

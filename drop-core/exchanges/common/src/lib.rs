use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SenderHandshake {
    pub profile: HandshakeProfile,
    pub files: Vec<HandshakeFile>,
}

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
pub struct ReceiverHandshake {
    pub profile: HandshakeProfile,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProjection {
    pub id: String,
    pub data: Vec<u8>,
}

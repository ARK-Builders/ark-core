mod receive_files;

pub use receive_files::*;

pub struct ReceiverProfile {
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct ReceiverConfig {
    pub decompression_enabled: bool,
    pub buffer_size: u64,
}

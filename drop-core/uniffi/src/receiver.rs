mod receive_files;

pub use receive_files::*;

pub struct ReceiverProfile {
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct ReceiverConfig {
    pub buffer_size: u64,
    pub chunk_size: u64,
    pub parallel_streams: u64,
}

mod send_files;

use std::sync::Arc;

pub use send_files::*;

pub struct SenderProfile {
    pub name: String,
    pub avatar_b64: Option<String>,
}

pub struct SenderFile {
    pub name: String,
    pub data: Arc<dyn SenderFileData>,
}

pub trait SenderFileData: Send + Sync {
    fn len(&self) -> u64;
    fn read(&self) -> Option<u8>;
    fn read_chunk(&self, size: u64) -> Vec<u8>;
}
struct SenderFileDataAdapter {
    inner: Arc<dyn SenderFileData>,
}
impl dropx_sender::SenderFileData for SenderFileDataAdapter {
    fn len(&self) -> u64 {
        return self.inner.len();
    }

    fn read(&self) -> Option<u8> {
        return self.inner.read();
    }

    fn read_chunk(&self, size: u64) -> Vec<u8> {
        return self.inner.read_chunk(size);
    }
}

pub struct SenderConfig {
    pub compression_enabled: bool,
    pub chunk_size: u64,
    pub buffer_size: u64,
    pub max_concurrent_streams: u32,
}
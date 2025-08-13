mod send_files;

use drop_entities::Data;
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
impl Data for SenderFileDataAdapter {
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

#[derive(Clone)]
pub struct SenderConfig {
    pub buffer_size: u64,
}
impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            buffer_size: 32768, // 32KB buffer for optimal balance
        }
    }
}
impl SenderConfig {
    pub fn high_performance() -> Self {
        Self {
            buffer_size: 8388608, // 8MB buffer
        }
    }

    pub fn balanced() -> Self {
        Self::default()
    }

    pub fn low_bandwidth() -> Self {
        Self {
            buffer_size: 131072, // 128KB buffer
        }
    }
}

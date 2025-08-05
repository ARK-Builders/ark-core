mod receive_files;

use std::{
    io::{Bytes, Read},
    sync::{RwLock, atomic::AtomicBool},
};

pub use receive_files::*;

pub struct ReceiverProfile {
    pub name: String,
    pub avatar_b64: Option<String>,
}

#[derive(Debug)]
pub struct ReceiverFile {
    pub id: String,
    pub name: String,
    pub data: ReceiverFileData,
}

#[derive(Debug)]
pub struct ReceiverFileData {
    is_finished: AtomicBool,
    path: std::path::PathBuf,
    reader: RwLock<Option<Bytes<std::fs::File>>>,
}
impl ReceiverFileData {
    pub fn new(path: std::path::PathBuf) -> Self {
        return Self {
            is_finished: AtomicBool::new(false),
            path,
            reader: RwLock::new(None),
        };
    }

    pub fn len(&self) -> u64 {
        let file = std::fs::File::open(&self.path).unwrap();
        return file.bytes().count() as u64;
    }

    pub fn read(&self) -> Option<u8> {
        if self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        if self.reader.read().unwrap().is_none() {
            let file = std::fs::File::open(&self.path).unwrap();
            self.reader.write().unwrap().replace(file.bytes());
        }
        let next = self
            .reader
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .next();
        if next.is_some() {
            let read_result = next.unwrap();
            if read_result.is_ok() {
                return Some(read_result.unwrap());
            }
        }
        self.reader.write().unwrap().as_mut().take();
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
        return None;
    }
}

#[derive(Clone)]
pub struct ReceiverConfig {
    pub decompression_enabled: bool,
    pub buffer_size: u64,
    pub max_concurrent_streams: u32,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            decompression_enabled: true,
            buffer_size: 2097152, // 2MB buffer
            max_concurrent_streams: 8,
        }
    }
}

impl ReceiverConfig {
    pub fn high_performance() -> Self {
        Self {
            decompression_enabled: false, // Skip decompression for speed
            buffer_size: 8388608,         // 8MB buffer
            max_concurrent_streams: 16,
        }
    }

    pub fn balanced() -> Self {
        Self::default()
    }

    pub fn low_bandwidth() -> Self {
        Self {
            decompression_enabled: true,
            buffer_size: 131072, // 128KB buffer
            max_concurrent_streams: 2,
        }
    }
}
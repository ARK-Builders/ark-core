use serde::{Deserialize, Serialize};
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{fs, num::TryFromIntError};

use crc32fast::Hasher;
use log;

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ResourceId {
    pub data_size: u64,
    pub crc32: u32,
}

impl ResourceId {
    pub fn store(self: Self) -> String {
        format!("{}-{}", self.data_size, self.crc32)
    }

    //todo: Option
    pub fn load(encoded: &str) -> Self {
        let mut parts = encoded.split('-');
        let data_size: u64 = parts
            .next()
            .unwrap()
            .parse()
            .expect("not a u64 number");
        let crc32: u32 = parts
            .next()
            .unwrap()
            .parse()
            .expect("not a u32 number");

        ResourceId { data_size, crc32 }
    }

    pub fn compute<P: AsRef<Path>>(data_size: u64, file_path: P) -> Self {
        log::trace!(
            "[compute] file {} with size {} mb",
            file_path.as_ref().display(),
            data_size / MEGABYTE
        );

        let source = fs::OpenOptions::new()
            .read(true)
            .open(file_path.as_ref())
            .expect(&format!(
                "Failed to read from {}",
                file_path.as_ref().display()
            ));

        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, source);

        ResourceId::compute_reader(data_size, &mut reader).expect(&format!(
            "Failed to read from {}",
            file_path.as_ref().display()
        ))
    }
    pub fn compute_bytes(bytes: &[u8]) -> Self {
        let data_size = bytes.len().try_into().unwrap();
        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, bytes);
        ResourceId::compute_reader(data_size, &mut reader)
            .expect(&format!("Failed to read from raw bytes",))
    }

    pub fn compute_reader<R: Read>(
        data_size: u64,
        reader: &mut BufReader<R>,
    ) -> Result<Self, anyhow::Error> {
        assert!(reader.buffer().is_empty());

        log::trace!(
            "Calculating hash of raw bytes (given size is {} megabytes)",
            data_size / MEGABYTE
        );

        let mut hasher = Hasher::new();
        let mut bytes_read: u32 = 0;
        loop {
            let bytes_read_iteration: usize = reader
                .fill_buf()
                .expect(&format!("Failed to read from the reader",))
                .len();
            if bytes_read_iteration == 0 {
                break;
            }
            hasher.update(reader.buffer());
            reader.consume(bytes_read_iteration);
            bytes_read += u32::try_from(bytes_read_iteration)
                .expect(&format!("Failed to read from the reader",))
        }

        let crc32: u32 = hasher.finalize().into();
        log::trace!("[compute] {} bytes has been read", bytes_read);
        log::trace!("[compute] checksum: {:#02x}", crc32);
        assert_eq!(
            bytes_read,
            (data_size.try_into() as Result<u32, TryFromIntError>).unwrap()
        );

        Ok(ResourceId { data_size, crc32 })
    }
}

const KILOBYTE: u64 = 1024;
const MEGABYTE: u64 = 1024 * KILOBYTE;
const BUFFER_CAPACITY: usize = 512 * KILOBYTE as usize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_id_test() {
        let file_path = Path::new("./tests/lena.jpg");
        let data_size = fs::metadata(file_path)
            .expect(&format!(
                "Could not open image test file_path.{}",
                file_path.display()
            ))
            .len();

        let id1 = ResourceId::compute(data_size.try_into().unwrap(), file_path);
        assert_eq!(id1.crc32, 0x342a3d4a);

        let raw_bytes = fs::read(file_path).unwrap();
        let id2 = ResourceId::compute_bytes(raw_bytes.as_slice());
        assert_eq!(id2.crc32, 0x342a3d4a);
    }
}

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{fs, num::TryFromIntError};

use crc32fast::Hasher;
use log;

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct ResourceId {
    pub file_size: u64,
    pub crc32: u32,
}

impl ResourceId {
    pub fn compute<P: AsRef<Path>>(file_size: u64, file_path: P) -> Self {
        log::trace!(
            "Calculating hash of {} (given size is {} megabytes)",
            file_path.as_ref().display(),
            file_size / MEGABYTE
        );

        let source = fs::OpenOptions::new()
            .read(true)
            .open(file_path.as_ref())
            .expect(&format!(
                "Failed to read from {}",
                file_path.as_ref().display()
            ));

        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, source);
        assert!(reader.buffer().is_empty());

        let mut hasher = Hasher::new();
        let mut bytes_read: u32 = 0;
        loop {
            let bytes_read_iteration: usize = reader
                .fill_buf()
                .expect(&format!(
                    "Failed to read from {}",
                    file_path.as_ref().display()
                ))
                .len();
            if bytes_read_iteration == 0 {
                break;
            }
            hasher.update(reader.buffer());
            reader.consume(bytes_read_iteration);
            bytes_read += u32::try_from(bytes_read_iteration).expect(&format!(
                "Failed to read from {}",
                file_path.as_ref().display()
            ))
        }

        let crc32: u32 = hasher.finalize().into();
        log::trace!("{} bytes has been read", bytes_read);
        log::trace!("checksum: {:#02x}", crc32);
        assert_eq!(
            bytes_read,
            (file_size.try_into() as Result<u32, TryFromIntError>).unwrap()
        );

        ResourceId { file_size, crc32 }
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
        let file_size = fs::metadata(file_path)
            .expect(&format!(
                "Could not open image test file_path.{}",
                file_path.display()
            ))
            .len();

        let id = ResourceId::compute(file_size.try_into().unwrap(), file_path);
        assert_eq!(id.crc32, 0x342a3d4a);
    }
}

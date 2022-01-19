pub const TAG_STORAGE_FILENAME: &str = ".ark-tags";

pub mod resource_id {
    use crc32fast::Hasher;
    use log::trace;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::path::Path;

    const KILOBYTE: usize = 1024;
    const MEGABYTE: usize = 1024 * KILOBYTE;
    const BUFFER_CAPACITY: usize = 512 * KILOBYTE;

    pub fn compute_id<P: AsRef<Path>>(file_size: usize, file_path: P) -> i64 {
        trace!(
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
        let mut bytes_read: i64 = 0;
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
            bytes_read += i64::try_from(bytes_read_iteration).expect(&format!(
                "Failed to read from {}",
                file_path.as_ref().display()
            ))
        }

        let checksum: i64 = hasher.finalize().into();
        trace!("{} bytes has been read", bytes_read);
        trace!("checksum: {:#02x}", checksum);
        assert!(bytes_read == file_size.try_into().unwrap());

        return checksum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::metadata;
    use std::path::Path;

    #[test]
    fn compute_id_test() {
        let file_path = Path::new("./tests/lena.jpg");
        let file_size = metadata(file_path)
            .expect(&format!(
                "Could not open image test file_path.{}",
                file_path.display()
            ))
            .len();

        let checksum =
            resource_id::compute_id(file_size.try_into().unwrap(), file_path);
        assert_eq!(checksum, 0x342a3d4a);
    }
}

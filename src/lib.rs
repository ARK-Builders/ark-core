pub mod resource_id {
    use std::fs;
    use std::path::Path;
    use std::io::{BufReader, BufRead};
    use crc32fast::Hasher;
    use log::trace;
    pub fn compute_id<P: AsRef<Path>>(
        file_size: usize,
        file_path: P) -> i64 {
        const KILOBYTE: usize = 1024;
        const MEGABYTE: usize = 1024 * KILOBYTE;
        const BUFFER_CAPACITY: usize = 512 * KILOBYTE;
        trace!("Calculating hash of {} (given size is {} megabytes)", file_path.as_ref().display(), file_size / MEGABYTE);
        let source = fs::OpenOptions::new()
            .read(true)
            .open(file_path.as_ref())
            .expect(&format!("Failed to read from {}", file_path.as_ref().display()));
        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, source);
        assert!(reader.buffer().is_empty());
        let mut hasher = Hasher::new();
        let mut bytes_read: i64 = 0;
        loop {
            let bytes_read_iteration: usize = reader
                .fill_buf()
                .expect(&format!("Failed to read from {}", file_path.as_ref().display()))
                .len();
            if bytes_read_iteration == 0 {
                break;
            }
            hasher.update(reader.buffer());
            reader.consume(bytes_read_iteration);
            bytes_read += i64::try_from(bytes_read_iteration)
                .expect(&format!("Failed to read from {}", file_path.as_ref().display()))
        }
        let checksum: i64  = hasher.finalize().into();
        trace!("{} bytes has been read", bytes_read);
        trace!("checksum: {:#02x}", checksum);
        assert!(bytes_read == file_size.try_into().unwrap());
        return checksum;
    }
}

#[cfg(target_os="android")]
#[allow(non_snake_case)]
pub mod android {
    use super::resource_id;

    use jni::JNIEnv;
    use jni::objects::{JString, JClass};
    use jni::sys::{jlong};

    use std::path::Path;
    use log::{Level, trace};
    
    extern crate android_logger;
    use android_logger::Config;

    #[no_mangle]
    pub unsafe extern fn Java_space_taran_arknavigator_mvp_model_repo_index_ResourceIdKt_computeIdNative(
        env: JNIEnv,
        _: JClass,
        jni_size: i64,
        jni_file_name: JString) -> jlong {
        android_logger::init_once(
            Config::default().with_min_level(Level::Trace));
        let file_size: usize = usize::try_from(jni_size)
            .expect(&format!("Failed to parse input size"));
        trace!("Received size: {}", file_size);
        let file_name: String = env
            .get_string(jni_file_name)
            .expect("Failed to parse input file name")
            .into();
        let file_path: &Path = Path::new(&file_name);
        trace!("Received filename: {}", file_path.display());
        return resource_id::compute_id(file_size, file_path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::metadata;
    use std::path::Path;
    use super::*;

    #[test]
    fn compute_id_test() {
        let file_path = Path::new("./tests/lena.jpg");
        let file_size = metadata(file_path)
            .expect(&format!("Could not open image test file_path.{}", file_path.display()))
            .len();
        let checksum = resource_id::compute_id(file_size.try_into().unwrap(), file_path);
        assert_eq!(checksum, 0x342a3d4a);
    }
}

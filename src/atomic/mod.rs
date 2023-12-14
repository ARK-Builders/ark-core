mod file;

use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Result, Write};

pub use file::AtomicFile;

pub fn modify(
    atomic_file: &AtomicFile,
    mut operator: impl FnMut(&[u8]) -> Vec<u8>,
) -> Result<()> {
    let mut buf = vec![];
    loop {
        let latest = atomic_file.load()?;
        buf.clear();
        if let Some(mut file) = latest.open()? {
            file.read_to_end(&mut buf)?;
        }
        let data = operator(&buf);
        let tmp = atomic_file.make_temp()?;
        (&tmp).write_all(&data)?;
        (&tmp).flush()?;
        match atomic_file.compare_and_swap(&latest, tmp) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                continue
            }
            Err(err) => return Err(err),
        }
    }
}

pub fn modify_json<T: Serialize + DeserializeOwned>(
    atomic_file: &AtomicFile,
    mut operator: impl FnMut(&mut Option<T>),
) -> Result<()> {
    loop {
        let latest = atomic_file.load()?;
        let mut val = None;
        if let Some(file) = latest.open()? {
            val = Some(serde_json::from_reader(std::io::BufReader::new(file))?);
        }
        operator(&mut val);
        let tmp = atomic_file.make_temp()?;
        let mut writer = std::io::BufWriter::new(&tmp);
        serde_json::to_writer(&mut writer, &val)?;
        writer.flush()?;
        drop(writer);
        match atomic_file.compare_and_swap(&latest, tmp) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                continue
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempdir::TempDir;

    #[test]
    fn failed_to_write_simultaneously() {
        let dir = TempDir::new("writing_test").unwrap();
        let root = dir.path();
        let shared_file = std::sync::Arc::new(AtomicFile::new(root).unwrap());
        let mut handles = Vec::with_capacity(5);
        for i in 0..5 {
            let file = shared_file.clone();
            let handle = std::thread::spawn(move || {
                let temp = file.make_temp().unwrap();
                let current = file.load().unwrap();
                let content = format!("Content from thread {i}!");
                (&temp).write_all(content.as_bytes()).unwrap();
                // In case slow computer ensure each thread are running in the same time
                std::thread::sleep(std::time::Duration::from_millis(300));
                file.compare_and_swap(&current, temp)
            });
            handles.push(handle);
        }
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        // Ensure only one thread has succeed to write
        let success = results.iter().fold(0, |mut acc, r| {
            if r.is_ok() {
                acc += 1;
            }
            acc
        });
        assert_eq!(success, 1);
    }

    #[test]
    fn multiple_writes_detected() {
        let dir = TempDir::new("simultaneous_writes").unwrap();
        let root = dir.path();
        let shared_file = std::sync::Arc::new(AtomicFile::new(root).unwrap());
        let thread_number = 10;
        assert!(thread_number > 3);
        // Need to have less than 255 thread to store thread number as byte directly
        assert!(thread_number < 256);
        let mut handles = Vec::with_capacity(thread_number);
        for i in 0..thread_number {
            let file = shared_file.clone();
            let handle = std::thread::spawn(move || {
                modify(&file, |data| {
                    let mut data = data.to_vec();
                    data.push(i.try_into().unwrap());
                    data
                })
            });
            handles.push(handle);
        }
        handles.into_iter().for_each(|handle| {
            handle.join().unwrap().unwrap();
        });
        // Last content
        let last_file = shared_file.load().unwrap();
        let last_content = last_file.read_content().unwrap();
        for i in 0..thread_number {
            let as_byte = i.try_into().unwrap();
            assert!(last_content.contains(&as_byte));
        }
    }
}

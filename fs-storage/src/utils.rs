use data_error::{ArklibError, Result};
use serde::Serialize;
use std::io::Write;
use std::{collections::BTreeMap, fs::File, path::Path, time::SystemTime};

/// Parses version 2 `FileStorage` format and returns the data as a BTreeMap
///
/// Version 2 `FileStorage` format represents data as a BTreeMap in plaintext.
///
/// For example:
/// ```text
/// version: 2
/// key1:1
/// key2:2
/// key3:3
/// ```
pub fn read_version_2_fs<K, V>(path: &Path) -> Result<BTreeMap<K, V>>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr,
{
    // First check if the file starts with "version: 2"
    let file_content = std::fs::read_to_string(path)?;
    if !file_content.starts_with("version: 2") {
        return Err(data_error::ArklibError::Parse);
    }

    // Parse the file content into a BTreeMap
    let mut data = BTreeMap::new();
    for line in file_content.lines().skip(1) {
        let mut parts = line.split(':');
        let key = parts
            .next()
            .unwrap()
            .parse()
            .map_err(|_| data_error::ArklibError::Parse)?;
        let value = parts
            .next()
            .unwrap()
            .parse()
            .map_err(|_| data_error::ArklibError::Parse)?;

        data.insert(key, value);
    }

    Ok(data)
}

/// Writes a serializable value to a file.
///
/// This function takes a path, a serializable value, and a timestamp. It writes the value to the specified
/// file in a pretty JSON format. The function ensures that the file is flushed and synced after writing.
pub fn write_json_file<T: Serialize>(
    path: &Path,
    value: &T,
    time: SystemTime,
) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.flush()?;

    file.set_modified(time)?;
    file.sync_all()?;

    Ok(())
}

/// Extracts a key of type K from the given file path.
///
/// The function can include or exclude the file extension based on the `include_extension` parameter.
/// It returns a Result containing the parsed key or an error if the extraction or parsing fails.
pub fn extract_key_from_file_path<K>(
    label: &str,
    path: &Path,
    include_extension: bool,
) -> Result<K>
where
    K: std::str::FromStr,
{
    match include_extension {
        true => path.file_name(), // ("tmp/foo.txt").file_name() -> ("foo.txt")
        false => path.file_stem(), // ("tmp/foo.txt").file_stem() -> ("foo")
    }
    .ok_or_else(|| {
        ArklibError::Storage(
            label.to_owned(),
            "Failed to extract file stem from filename".to_owned(),
        )
    })?
    .to_str()
    .ok_or_else(|| {
        ArklibError::Storage(
            label.to_owned(),
            "Failed to convert file stem to string".to_owned(),
        )
    })?
    .parse::<K>()
    .map_err(|_| {
        ArklibError::Storage(
            label.to_owned(),
            "Failed to parse key from filename".to_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tempdir::TempDir;

    /// Test reading a legacy version 2 `FileStorage` file
    #[test]
    fn test_read_legacy_fs() {
        let temp_dir = TempDir::new("ark-rust")
            .expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("test_read_legacy_fs");
        let file_content = r#"version: 2
key1:1
key2:2
key3:3
"#;
        let mut file = std::fs::File::create(&file_path)
            .expect("Failed to create test file");
        file.write_all(file_content.as_bytes())
            .expect("Failed to write to test file");

        // Read the file and check the data
        let data: BTreeMap<String, i32> = read_version_2_fs(&file_path)
            .expect("Failed to read version 2 file storage");
        assert_eq!(data.len(), 3);
        assert_eq!(data.get("key1"), Some(&1));
        assert_eq!(data.get("key2"), Some(&2));
        assert_eq!(data.get("key3"), Some(&3));
    }

    /// Test writing a JSON file
    #[test]
    fn test_write_json_file() {
        let temp_dir = TempDir::new("ark-rust")
            .expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("test_write_json_file.json");
        let value = json!({"key": "value"});

        write_json_file(&file_path, &value, SystemTime::now())
            .expect("Failed to write JSON file");

        let written_content = std::fs::read_to_string(&file_path)
            .expect("Failed to read written JSON file");
        let expected_content = serde_json::to_string_pretty(&value)
            .expect("Failed to serialize JSON value");
        assert_eq!(written_content, expected_content);
    }

    /// Test extracting a key from a file path
    #[test]
    fn test_extract_key_from_file_path() {
        let path_with_extension = Path::new("tmp/foo.txt");
        let path_without_extension = Path::new("tmp/foo");

        let key_with_extension: String =
            extract_key_from_file_path("test", path_with_extension, true)
                .expect("Failed to extract key with extension");
        assert_eq!(key_with_extension, "foo.txt");

        let key_without_extension: String =
            extract_key_from_file_path("test", path_without_extension, false)
                .expect("Failed to extract key without extension");
        assert_eq!(key_without_extension, "foo");
    }
}

#[cfg(test)]
mod tests {
    use ark_rust::storage::file_storage::FileStorage;
    use std::collections::HashMap;
    use std::fs::remove_file;
    use tempfile::TempDir;

    #[test]
    fn test_file_storage_integration() {
        let temp_dir =
            TempDir::new().expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("test_storage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);
        
        let mut data_to_write = HashMap::new();
        data_to_write.insert("key1".to_string(), "value1".to_string());
        data_to_write.insert("key2".to_string(), "value2".to_string());

        file_storage
            .write_to_disk(&data_to_write)
            .expect("Failed to write data to disk");

        let mut data_read: HashMap<String, String> = HashMap::new();

        file_storage
            .read_from_disk(|data: HashMap<String, String>| {
                data_read = data;
            })
            .expect("Failed to write data to disk");

        assert_eq!(data_to_write, data_read);

        remove_file(&storage_path).expect("Failed to remove temporary file");
    }
}

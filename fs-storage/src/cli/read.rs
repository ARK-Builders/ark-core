use std::env;
use fs_storage::file_storage::FileStorage; // Import FileStorage
use std::collections::HashMap;
use std::str::FromStr;
use data_error::ArklibError;
use std::fmt::Debug;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <path_to_storage> <key>", args[0]);
        return;
    }
    let storage_path = &args[1];
    let key = &args[2];

    let file_storage = FileStorage::new("Storage".to_string(), storage_path);

    match file_storage.read_file::<String, String>(|value_by_id| {
        if let Some(value) = value_by_id.get(key) {
            println!("Value for key '{}': {:?}", key, value);
        } else {
            println!("Key '{}' not found in storage.", key);
        }
    }) {
        Ok(_) => {}
        Err(e) => {
            println!("Error reading storage file: {:?}", e);
        }
    }
}
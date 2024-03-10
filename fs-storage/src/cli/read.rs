use std::env;
use std::path::Path;
use fs_storage::file_storage::FileStorage;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <path_to_storage> <key>", args[0]);
        return;
    }
    let storage_path = &args[1];
    let key = &args[2];

    println!("Storage Path: {}", storage_path);
    println!("Key: {}", key);

    if !Path::new(storage_path).exists() {
        println!("Error: Storage file does not exist.");
        return;
    }
    
    let file_storage = FileStorage::new("our_label".to_string(), Path::new(storage_path));
    match file_storage.read_file::<String, String>(|value_by_id| {
        println!("ruun3");
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

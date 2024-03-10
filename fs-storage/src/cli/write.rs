use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use fs_storage::file_storage::FileStorage;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <path_to_storage>", args[0]);
        return;
    }

    let storage_path = &args[1];
    if fs::metadata(storage_path).is_ok() {
        println!("Storage already exists at {}", storage_path);
        return;
    }

    if let Err(err) = fs::create_dir_all(storage_path) {
        println!("Error creating storage directory: {}", err);
        return;
    }
    println!("Storage directory created successfully at {}", storage_path);

    let mut kv_pairs: HashMap<String, String> = HashMap::new();
    loop {
        println!("Enter a key-value pair (key=value), or enter 'done' to finish:");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let input = input.trim();

        if input.eq_ignore_ascii_case("done") {
            break;
        }

        let pair: Vec<&str> = input.splitn(2, '=').collect();
        if pair.len() != 2 {
            println!("Invalid input, key-value pair must be in the format 'key=value'");
            continue;
        }

        let key = pair[0].trim().to_string();
        let value = pair[1].trim().to_string();

        kv_pairs.insert(key, value);
    }

    println!("Key-Value Pairs:");
    for (key, value) in &kv_pairs {
        println!("{}: {}", key, value);
    }

    if let Err(err) = write_to_file(kv_pairs, storage_path) {
        println!("Error writing to file: {}", err);
    }
}

fn write_to_file(kv_pairs: HashMap<String, String>, storage_path: &str) -> io::Result<()> {
    let mut storage_file = PathBuf::from(storage_path);
    storage_file.push("storage.txt");

    let mut storage = FileStorage::new("our_label".to_string(), &storage_file);
    match storage.write_file(&kv_pairs) {
        Ok(_) => Ok(()),
        Err(err) => {
            let io_err = io::Error::new(io::ErrorKind::Other, format!("ArklibError: {:?}", err));
            Err(io_err)
        }
    }
}

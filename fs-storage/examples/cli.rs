use fs_storage::file_storage::FileStorage;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage:");
        println!("    cargo run -- write <path> <json_file>");
        println!("    cargo run -- read <path> <key1,key2,...>");
        return;
    }

    let command = &args[1];
    let path = &args[2];

    match command.as_str() {
        "read" => {
            let keys = if args.len() > 3 {
                args[3]
                    .split(',')
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            } else {
                vec![]
            };
            let mut fs = FileStorage::new("cli".to_string(), Path::new(path));
            let map: BTreeMap<String, String> = fs.read_file().unwrap();
            if keys.is_empty() {
                for (key, value) in map {
                    println!("{}: {}", key, value);
                }
            } else {
                for key in &keys {
                    if let Some(value) = map.get(key) {
                        println!("{}: {}", key, value);
                    } else {
                        println!("Key '{}' not found", key);
                    }
                }
            }
        }
        "write" => {
            if args.len() < 4 {
                println!("Usage: cargo run -- write <path> <json_file>");
                return;
            }

            let json_file = &args[3];
            let json_contents = fs::read_to_string(json_file)
                .expect("Failed to read JSON file");
            let json_value: Value =
                serde_json::from_str(&json_contents).expect("Invalid JSON");

            let mut kv_pairs = BTreeMap::new();
            if let Value::Object(object) = json_value {
                for (key, value) in object {
                    if let Value::String(value_str) = value {
                        kv_pairs.insert(key, value_str);
                    } else {
                        println!(
                            "Warning: Skipping non-string value for key '{}'",
                            key
                        );
                    }
                }
            } else {
                println!("JSON value is not an object");
                return;
            }

            let mut fs = FileStorage::new("cli".to_string(), Path::new(path));
            fs.write_file(&kv_pairs).unwrap();
        }
        _ => eprintln!("Invalid command. Use 'read' or 'write'."),
    }
}

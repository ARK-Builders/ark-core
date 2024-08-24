use anyhow::{Context, Result};
use fs_storage::{
    base_storage::BaseStorage, file_storage::FileStorage,
    folder_storage::FolderStorage,
};
use serde_json::Value;
use std::{env, fs, path::Path};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage:");
        println!(" cargo run --example cli [file|folder] write <path> [JSON_FILE_PATH | KEY_VALUE_PAIRS]");
        println!(" cargo run --example cli [file|folder] read <path> <key1,key2,...>");
        return Ok(());
    }

    let storage_type = &args[1];
    let command = &args[2];
    let path = &args[3];
    match storage_type.as_str() {
        "file" => match command.as_str() {
            "read" => file_read_command(&args, path),
            "write" => file_write_command(&args, path),
            _ => {
                eprintln!("Invalid command. Use 'read' or 'write'.");
                Ok(())
            }
        },
        "folder" => match command.as_str() {
            "read" => folder_read_command(&args, path),
            "write" => folder_write_command(&args, path),
            _ => {
                eprintln!("Invalid command. Use 'read' or 'write'.");
                Ok(())
            }
        },
        _ => {
            eprintln!("Invalid storage. Use 'file' or 'folder'.");
            Ok(())
        }
    }
}

fn file_read_command(args: &[String], path: &str) -> Result<()> {
    let keys = if args.len() > 3 {
        args[4]
            .split(',')
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    } else {
        vec![]
    };

    let mut fs: FileStorage<String, String> =
        FileStorage::new("cli".to_string(), Path::new(path))
            .context("Failed to create FileStorage")?;

    let map = fs
        .read_fs()
        .expect("No Data is present on this path");
    if keys.is_empty() {
        for (key, value) in map {
            println!("{}: {}", key, value);
        }
    }
    for key in &keys {
        if let Some(value) = map.get(key) {
            println!("{}: {}", key, value);
        } else {
            eprintln!("Key '{}' not found", key);
        }
    }

    Ok(())
}

fn file_write_command(args: &[String], path: &str) -> Result<()> {
    if args.len() < 4 {
        println!("Usage: cargo run --example cli file write <path> [JSON_FILE_PATH | KEY_VALUE_PAIRS]");
        return Ok(());
    }

    let content = &args[4];
    // Check if the content is a JSON file path
    let content_json = Path::new(content)
        .extension()
        .map_or(false, |ext| ext == "json");

    let mut fs: FileStorage<String, String> =
        FileStorage::new("cli".to_string(), Path::new(path))
            .context("Failed to create FileStorage")?;
    if content_json {
        let content =
            fs::read_to_string(content).context("Failed to read JSON file")?;
        let json: Value =
            serde_json::from_str(&content).context("Failed to parse JSON")?;
        if let Value::Object(object) = json {
            for (key, value) in object {
                if let Value::String(value_str) = value {
                    fs.set(key, value_str);
                } else {
                    println!(
                        "Warning: Skipping non-string value for key '{}'",
                        key
                    );
                }
            }
        } else {
            println!("JSON value is not an object");
            return Ok(());
        }
    } else {
        let pairs = content.split(',');
        for pair in pairs {
            let kv: Vec<&str> = pair.split(':').collect();
            if kv.len() == 2 {
                fs.set(kv[0].to_string(), kv[1].to_string());
            }
        }
    }
    fs.write_fs().expect("Failed to write to file");
    Ok(())
}

fn folder_read_command(args: &[String], path: &str) -> Result<()> {
    let keys = if args.len() > 3 {
        args[4]
            .split(',')
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    } else {
        vec![]
    };

    let mut fs: FolderStorage<String, String> =
        FolderStorage::new("cli".to_string(), Path::new(path))
            .context("Failed to create FolderStorage")?;

    let map = fs
        .read_fs()
        .expect("No Data is present on this path");
    if keys.is_empty() {
        for (key, value) in map {
            println!("{}: {}", key, value);
        }
    }
    for key in &keys {
        if let Some(value) = map.get(key) {
            println!("{}: {}", key, value);
        } else {
            eprintln!("Key '{}' not found", key);
        }
    }

    Ok(())
}

fn folder_write_command(args: &[String], path: &str) -> Result<()> {
    if args.len() < 4 {
        println!("Usage: cargo run --example cli folder write <path> [JSON_FILE_PATH | KEY_VALUE_PAIRS]");
        return Ok(());
    }

    let content = &args[4];
    // Check if the content is a JSON file path
    let content_json = Path::new(content)
        .extension()
        .map_or(false, |ext| ext == "json");

    let mut fs: FolderStorage<String, String> =
        FolderStorage::new("cli".to_string(), Path::new(path))
            .context("Failed to create FolderStorage")?;
    if content_json {
        let content =
            fs::read_to_string(content).context("Failed to read JSON file")?;
        let json: Value =
            serde_json::from_str(&content).context("Failed to parse JSON")?;
        if let Value::Object(object) = json {
            for (key, value) in object {
                if let Value::String(value_str) = value {
                    fs.set(key, value_str);
                } else {
                    println!(
                        "Warning: Skipping non-string value for key '{}'",
                        key
                    );
                }
            }
        } else {
            println!("JSON value is not an object");
            return Ok(());
        }
    } else {
        let pairs = content.split(',');
        for pair in pairs {
            let kv: Vec<&str> = pair.split(':').collect();
            if kv.len() == 2 {
                fs.set(kv[0].to_string(), kv[1].to_string());
            }
        }
    }
    fs.write_fs().expect("Failed to write to folder");
    Ok(())
}

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <path_to_storage> <key>", args[0]);
        return;
    }
    let storage_path = &args[1];
    let key = &args[2];
    let file = match File::open(storage_path) {
        Ok(file) => file,
        Err(err) => {
            println!("Error opening storage file: {}", err);
            return;
        }
    };

    let reader = BufReader::new(file);
    let mut found = false;
    for line in reader.lines() {
        if let Ok(line) = line {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 && parts[0] == key {
                println!("Value for key '{}': {}", key, parts[1]);
                found = true;
                break;
            }
        }
    }
    if !found {
        println!("Key '{}' not found in storage.", key);
    }
}

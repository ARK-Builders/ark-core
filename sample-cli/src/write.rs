use std::env;
use std::fs;
use std::io::{self, Write};

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
    if let Err(err) = fs::create_dir(storage_path) {
        println!("Error creating storage directory: {}", err);
        return;
    }
    println!("Storage created successfully at {}", storage_path);

    let mut storage_type: String = String::new();
    let mut kv_pairs: Vec<(String, String)> = Vec::new();

    println!("Please specify the storage type:");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut storage_type).expect("Failed to read line");
    let storage_type = storage_type.trim();

    loop {
        println!("Enter a key-value pair (key=value), or enter 'done' to finish:");
        io::stdout().flush().unwrap();

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

        kv_pairs.push((key, value));
    }

    println!("Storage Type: {}", storage_type);
    println!("Key-Value Pairs:");
    for (key, value) in kv_pairs {
        println!("{}: {}", key, value);
    }
}

fn echo(s: &str, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;

    f.write_all(s.as_bytes())
}
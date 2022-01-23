use std::env::current_dir;
use std::fs::{canonicalize, copy, create_dir_all, metadata, File};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use home::home_dir;
use walkdir::WalkDir;

use arklib::resource_id;
use arklib::TAG_STORAGE_FILENAME;

#[derive(Parser, Debug)]
#[clap(name = "ark-cli")]
#[clap(about = "Manage ARK tag storages and indexes", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Backup {
        #[clap(parse(from_os_str))]
        roots_cfg: Option<PathBuf>,
    },

    Collisions {
        #[clap(parse(from_os_str))]
        target_dir: Option<PathBuf>,
    },
}

const ARK_HOME: &str = ".ark";

const ROOTS_CFG_FILENAME: &str = "roots";
const HOME_BACKUPS_DIRNAME: &str = "backups";

fn main() {
    let args = Cli::parse();

    match &args.command {
        Command::Backup { roots_cfg } => {
            let timestamp = timestamp().as_secs();
            let backup_dir = home_dir()
                .expect("Couldn't retrieve home directory!")
                .join(&ARK_HOME)
                .join(&HOME_BACKUPS_DIRNAME)
                .join(&timestamp.to_string());

            if backup_dir.is_dir() {
                println!("Wait at least 1 second, please!");
                std::process::exit(0)
            }

            println!("Preparing backup:");
            let roots = if let Some(path) = roots_cfg {
                println!(
                    "\tRoots config provided explicitly:\n\t\t{}",
                    path.display()
                );
                let config = File::open(&path).expect("File doesn't exist!");

                parse_roots(config)
            } else {
                let roots_cfg = home_roots_cfg();

                if let Ok(config) = File::open(&roots_cfg) {
                    println!(
                        "\tRoots config was found automatically:\n\t\t{}",
                        roots_cfg.display()
                    );

                    parse_roots(config)
                } else {
                    println!("\tRoots config wasn't found.");

                    println!("Looking for a folder containing tag storage:");
                    let path = canonicalize(
                        current_dir().expect("Can't open current directory!"),
                    )
                    .expect("Couldn't canonicalize working directory!");

                    let result = path.ancestors().find(|path| {
                        println!("\t{}", path.display());
                        tag_storage_exists(path)
                    });

                    if let Some(root) = result {
                        println!("Root folder found:\n\t{}", root.display());
                        vec![root.to_path_buf()]
                    } else {
                        println!("Root folder wasn't found.");
                        vec![]
                    }
                }
            };

            let (valid, invalid): (Vec<PathBuf>, Vec<PathBuf>) = roots
                .into_iter()
                .partition(|root| tag_storage_exists(&root));

            if !invalid.is_empty() {
                println!("These folders don't contain tag storages:");
                invalid
                    .into_iter()
                    .for_each(|root| println!("\t{}", root.display()));
            }

            if valid.is_empty() {
                println!("Nothing to backup. Bye!");
                std::process::exit(0)
            }

            create_dir_all(&backup_dir)
                .expect("Couldn't create backup directory!");

            let mut roots_cfg_backup =
                File::create(&backup_dir.join(&ROOTS_CFG_FILENAME))
                    .expect("Couldn't backup roots config!");

            valid.iter().for_each(|root| {
                writeln!(roots_cfg_backup, "{}", root.display())
                    .expect("Couldn't write to roots config backup!")
            });

            println!("Performing backups:");
            valid
                .into_iter()
                .enumerate()
                .for_each(|(i, root)| {
                    println!("\tRoot {}", root.display());
                    let storage_backup = backup_dir.join(&i.to_string());
                    let result =
                        copy(root.join(&TAG_STORAGE_FILENAME), storage_backup);
                    if let Err(e) = result {
                        println!("\t\tFailed to copy tag storage!\n\t\t{}", e);
                    }
                });

            println!("Backup created:\n\t{}", backup_dir.display());
        }

        Command::Collisions { target_dir } => {
            use std::collections::HashMap;

            let dir_path = if let Some(path) = target_dir {
                path.clone()
            } else {
                current_dir()
                    .expect("Can't open current directory!")
                    .clone()
            };

            println!(
                "Calculating IDs of all files by path:\n\t{}",
                dir_path.display()
            );

            let mut index = HashMap::<u32, usize>::new();
            for entry in WalkDir::new(dir_path).into_iter() {
                let entry2 = entry.expect("whatever").clone();
                let path = entry2.path();
                if !path.is_dir() {
                    let size = metadata(&path).expect("whatever").len();
                    let id = resource_id::compute_id(size, path);

                    let count = index.get_mut(&id);
                    if let Some(nonempty) = count {
                        *nonempty += 1;
                    } else {
                        index.insert(id, 1);
                    }
                }
            }

            for (key, count) in index.into_iter() {
                if count > 1 {
                    println!("{}: {} times", key, count);
                }
            }
        }
    }
}

fn home_roots_cfg() -> PathBuf {
    return home_dir()
        .expect("Couldn't retrieve home directory!")
        .join(&ARK_HOME)
        .join(&ROOTS_CFG_FILENAME);
}

fn tag_storage_exists(path: &Path) -> bool {
    return File::open(path.join(&TAG_STORAGE_FILENAME)).is_ok();
}

fn parse_roots(config: File) -> Vec<PathBuf> {
    return BufReader::new(config)
        .lines()
        .filter_map(|line| match line {
            Ok(path) => Some(PathBuf::from(path)),
            Err(msg) => {
                println!("{:?}", msg);
                None
            }
        })
        .collect();
}

fn timestamp() -> Duration {
    let start = SystemTime::now();
    return start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards!");
}

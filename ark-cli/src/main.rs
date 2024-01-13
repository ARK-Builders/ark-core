use std::env::current_dir;
use std::fs::{canonicalize, create_dir_all, metadata, File};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Display, Path, PathBuf};
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arklib::app_id;
use arklib::id::ResourceId;
use arklib::index::ResourceIndex;
use arklib::pdf::PDFQuality;
use arklib::{
    modify, AtomicFile, APP_ID_FILE, ARK_FOLDER, FAVORITES_FILE,
    METADATA_STORAGE_FOLDER, PREVIEWS_STORAGE_FOLDER,
    PROPERTIES_STORAGE_FOLDER, SCORE_STORAGE_FILE, STATS_FOLDER,
    TAG_STORAGE_FILE, THUMBNAILS_STORAGE_FOLDER,
};
use clap::{Parser, Subcommand};
use fs_extra::dir::{self, CopyOptions};
use home::home_dir;
use std::io::{Result, Write};
use url::Url;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(name = "ark-cli")]
#[clap(about = "Manage ARK tag storages and indexes", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug)]
enum InsertContent {
    Values(Vec<(String, String)>),
    String(String),
}

impl FromStr for InsertContent {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let pairs: Vec<&str> = s.split(',').collect();

        if pairs.len() == 1 {
            let key_value: Vec<&str> = pairs[0].split(':').collect();
            if key_value.len() == 2 {
                let key = key_value[0].trim().to_string();
                let value = key_value[1].trim().to_string();
                return Ok(InsertContent::Values(vec![(key, value)]));
            } else {
                return Ok(InsertContent::String(s.to_string()));
            }
        }

        let mut values = Vec::new();

        for pair in pairs {
            let key_value: Vec<&str> = pair.split(':').collect();
            if key_value.len() == 2 {
                let key = key_value[0].trim().to_string();
                let value = key_value[1].trim().to_string();
                values.push((key, value));
            } else {
                return Err("Invalid key-value pair format");
            }
        }

        Ok(InsertContent::Values(values))
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    Backup {
        #[clap(parse(from_os_str))]
        roots_cfg: Option<PathBuf>,
    },

    Collisions {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,
    },

    Monitor {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,
        interval: Option<u64>,
    },

    Render {
        #[clap(parse(from_os_str))]
        path: Option<PathBuf>,
        quality: Option<String>,
    },

    #[clap(subcommand)]
    Link(Link),

    #[clap(subcommand)]
    File(FileCommand),
}

#[derive(Subcommand, Debug)]
enum FileCommand {
    Append {
        #[clap(parse(from_os_str))]
        storage: Option<PathBuf>,

        content: Option<InsertContent>,
    },

    Insert {
        #[clap(parse(from_os_str))]
        storage: Option<PathBuf>,

        content: Option<InsertContent>,
    },

    Read {
        #[clap(parse(from_os_str))]
        storage: Option<PathBuf>,
    },

    List {
        #[clap(parse(from_os_str))]
        storage: Option<PathBuf>,

        #[clap(short, long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
enum Link {
    Create {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,

        url: Option<String>,
        title: Option<String>,
        desc: Option<String>,
    },

    Load {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,

        #[clap(parse(from_os_str))]
        file_path: Option<PathBuf>,

        id: Option<ResourceId>,
    },
}

const ARK_CONFIG: &str = ".config/ark";
const ARK_BACKUPS_PATH: &str = ".ark-backups";
const ROOTS_CFG_FILENAME: &str = "roots";

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Cli::parse();

    let app_id_dir = home_dir().expect("Couldn't retrieve home directory!");
    let ark_dir = app_id_dir.join(".ark");
    if !ark_dir.exists() {
        std::fs::create_dir(&ark_dir).unwrap();
    }
    println!("Loading app id at {}...", ark_dir.display());
    let _ = app_id::load(ark_dir).map_err(|e| {
        println!("Couldn't load app id: {}", e);
        std::process::exit(1);
    });

    match &args.command {
        Command::Backup { roots_cfg } => {
            let timestamp = timestamp().as_secs();
            let backup_dir = home_dir()
                .expect("Couldn't retrieve home directory!")
                .join(&ARK_BACKUPS_PATH)
                .join(&timestamp.to_string());

            if backup_dir.is_dir() {
                println!("Wait at least 1 second, please!");
                std::process::exit(0)
            }

            println!("Preparing backup:");
            let roots = discover_roots(roots_cfg);

            let (valid, invalid): (Vec<PathBuf>, Vec<PathBuf>) = roots
                .into_iter()
                .partition(|root| storages_exists(&root));

            if !invalid.is_empty() {
                println!("These folders don't contain any storages:");
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

                    let mut options = CopyOptions::new();
                    options.overwrite = true;
                    options.copy_inside = true;

                    let result = dir::copy(
                        root.join(&arklib::ARK_FOLDER),
                        storage_backup,
                        &options,
                    );

                    if let Err(e) = result {
                        println!("\t\tFailed to copy storages!\n\t\t{}", e);
                    }
                });

            println!("Backup created:\n\t{}", backup_dir.display());
        }

        Command::Collisions { root_dir } => monitor_index(&root_dir, None),

        Command::Monitor { root_dir, interval } => {
            let millis = interval.unwrap_or(1000);
            monitor_index(&root_dir, Some(millis))
        }

        Command::Render { path, quality } => {
            let filepath = path.to_owned().unwrap();
            let quality = match quality.to_owned().unwrap().as_str() {
                "high" => PDFQuality::High,
                "medium" => PDFQuality::Medium,
                "low" => PDFQuality::Low,
                _ => panic!("unknown render option"),
            };
            let buf = File::open(&filepath).unwrap();
            let dest_path = filepath.with_file_name(
                filepath
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned()
                    + ".png",
            );
            let img = arklib::pdf::render_preview_page(buf, quality);
            img.save(PathBuf::from(dest_path)).unwrap();
        }

        Command::Link(link) => match &link {
            Link::Create {
                root_dir,
                url,
                title,
                desc,
            } => {
                let root = provide_root(root_dir);

                let url = Url::parse(url.as_deref().unwrap());
                let link: arklib::link::Link = arklib::link::Link::new(
                    url.unwrap(),
                    title.to_owned().unwrap(),
                    desc.to_owned(),
                );

                let future = link.save(&root, true);

                println!("Saving link...");

                match future.await {
                    Ok(_) => {
                        println!("Link saved successfully!");
                        match provide_index(&root).store() {
                            Ok(_) => println!("Index stored successfully!"),
                            Err(e) => println!("Error: {}", e),
                        }
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }

            Link::Load {
                root_dir,
                file_path,
                id,
            } => {
                let root = provide_root(root_dir);

                let path_from_index = id.map(|id| {
                    let index = provide_index(&root);
                    index.id2path[&id].as_path().to_path_buf()
                });
                let path_from_user = file_path;

                let path = match (path_from_user, path_from_index) {
                    (Some(path), Some(path2)) => {
                        if path.canonicalize().unwrap() != path2 {
                            println!("Path {:?} was requested.", path);
                            println!(
                                "But id {} maps to path {:?}",
                                id.unwrap(),
                                path2
                            );
                            panic!()
                        } else {
                            path.to_path_buf()
                        }
                    }
                    (Some(path), None) => path.to_path_buf(),
                    (None, Some(path)) => path,
                    (None, None) => {
                        println!("Provide a path or id for request.");
                        panic!()
                    }
                };

                let link = arklib::link::Link::load(root, path);
                println!("Link data:\n{:?}", link.unwrap());
            }
        },

        Command::File(file) => match &file {
            FileCommand::Append { storage, content } => {
                let root = provide_root(&None);
                let storage = storage.as_ref().unwrap_or(&root);
                let file_path = get_storage_from_path(storage)
                    .expect("Could not find storage folder");

                let atomic_file = arklib::AtomicFile::new(file_path).unwrap();

                if let Some(content) = content {
                    match content {
                        InsertContent::String(content) => {
                            modify(&atomic_file, |current| {
                                let mut combined_vec: Vec<u8> =
                                    current.to_vec();
                                combined_vec
                                    .extend_from_slice(content.as_bytes());
                                combined_vec
                            })
                            .expect("Could not append string")
                        }
                        InsertContent::Values(values) => {
                            append_json(&atomic_file, values.to_vec())
                                .expect("Could not append json");
                        }
                    }
                } else {
                    println!("Provide content to insert");
                }
            }

            FileCommand::Insert { storage, content } => {
                let root = provide_root(&None);
                let storage = storage.as_ref().unwrap_or(&root);
                let file_path = get_storage_from_path(storage)
                    .expect("Could not find storage folder");

                let atomic_file = arklib::AtomicFile::new(file_path).unwrap();

                if let Some(content) = content {
                    match content {
                        InsertContent::String(content) => {
                            modify(&atomic_file, |_| {
                                content.as_bytes().to_vec()
                            })
                            .expect("Could not insert string");
                        }
                        InsertContent::Values(values) => {
                            modify_json(
                                &atomic_file,
                                |current: &mut Option<serde_json::Value>| {
                                    let mut new = serde_json::Map::new();
                                    for (key, value) in values {
                                        new.insert(
                                            key.clone(),
                                            serde_json::Value::String(
                                                value.clone(),
                                            ),
                                        );
                                    }
                                    *current =
                                        Some(serde_json::Value::Object(new));
                                },
                            )
                            .expect("Could not insert json");
                        }
                    }
                } else {
                    println!("Provide content to insert");
                }
            }

            FileCommand::Read { storage } => {
                let root = provide_root(&None);
                let storage = storage.as_ref().unwrap_or(&root);
                let file_path = get_storage_from_path(storage)
                    .expect("Could not find storage folder");

                let atomic_file = arklib::AtomicFile::new(&file_path).unwrap();

                if let Some(file) = format_file(&atomic_file, true) {
                    println!("{}", file);
                } else {
                    println!(
                        "FILE: {} is not a valid atomic file",
                        file_path.display()
                    );
                }
            }

            FileCommand::List { storage, all } => {
                let root = provide_root(&None);
                let storage = storage.as_ref().unwrap_or(&root);
                let file_path = get_storage_from_path(storage)
                    .expect("Could not find storage folder");

                if !all {
                    let file = AtomicFile::new(&file_path).unwrap();
                    if let Some(file) = format_file(&file, false) {
                        println!("{}", file);
                    } else {
                        println!(
                            "FILE: {} is not a valid atomic file",
                            file_path.display()
                        );
                    }
                } else {
                    let files: Vec<AtomicFile> = WalkDir::new(file_path)
                        .into_iter()
                        .filter_entry(|e| e.file_type().is_dir())
                        .filter_map(|v| v.ok())
                        .filter_map(|e| match AtomicFile::new(e.path()) {
                            Ok(file) => Some(file),
                            Err(_) => None,
                        })
                        .collect();

                    for file in files {
                        if let Some(file) = format_file(&file, false) {
                            println!("{}", file);
                        }
                    }
                }
            }
        },
    }
}

pub fn append_json(
    atomic_file: &AtomicFile,
    data: Vec<(String, String)>,
) -> Result<()> {
    modify_json(&atomic_file, |current: &mut Option<serde_json::Value>| {
        let current_data = match current {
            Some(current) => {
                if let Ok(value) = serde_json::to_value(current) {
                    match value {
                        serde_json::Value::Object(map) => Some(map),
                        _ => None,
                    }
                } else {
                    None
                }
            }

            None => None,
        };
        let mut new = serde_json::Map::new();

        if let None = current_data {
            for (key, value) in &data {
                new.insert(
                    key.clone(),
                    serde_json::Value::String(value.clone()),
                );
            }
            *current = Some(serde_json::Value::Object(new));
        } else if let Some(values) = current_data {
            for (key, value) in &values {
                new.insert(key.clone(), value.clone());
            }

            for (key, value) in &data {
                new.insert(
                    key.clone(),
                    serde_json::Value::String(value.clone()),
                );
            }
            *current = Some(serde_json::Value::Object(new));
        }
    })
}

fn get_storage_from_path(storage: &PathBuf) -> Option<PathBuf> {
    if storage.exists() {
        Some(storage.clone())
    } else {
        match storage
            .clone()
            .into_os_string()
            .into_string()
            .unwrap()
            .to_lowercase()
            .as_str()
        {
            "favorites" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(FAVORITES_FILE),
            ),
            "device" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(DEVICE_ID),
            ),
            "tage" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(TAG_STORAGE_FILE),
            ),
            "score" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(SCORE_STORAGE_FILE),
            ),
            "stats" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(STATS_FOLDER),
            ),
            "properties" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(PROPERTIES_STORAGE_FOLDER),
            ),
            "metadata" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(METADATA_STORAGE_FOLDER),
            ),
            "previews" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(PREVIEWS_STORAGE_FOLDER),
            ),
            "thumbnails" => Some(
                provide_root(&None)
                    .join(ARK_FOLDER)
                    .join(THUMBNAILS_STORAGE_FOLDER),
            ),

            _ => None,
        }
    }
}

fn format_file(file: &AtomicFile, show_content: bool) -> Option<String> {
    let current = file.load().ok()?;

    if current.version == 0 {
        return None;
    }

    let mut split = current
        .path
        .file_name()
        .expect("Not a file")
        .to_str()
        .unwrap()
        .split("_");

    let mut output = format!(
        "{}: [{} - {}]",
        current.version,
        split.next().unwrap(),
        split.next().unwrap()
    );

    if show_content {
        let data = current.read_to_string().ok()?;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            output.push_str(&format!(
                "\n\n{}",
                serde_json::to_string_pretty(&json).unwrap()
            ));
        } else {
            output.push_str(&format!("\n\n{}", data));
        }
    }

    Some(output)
}

fn discover_roots(roots_cfg: &Option<PathBuf>) -> Vec<PathBuf> {
    if let Some(path) = roots_cfg {
        println!(
            "\tRoots config provided explicitly:\n\t\t{}",
            path.display()
        );
        let config = File::open(&path).expect("File doesn't exist!");

        parse_roots(config)
    } else {
        if let Ok(config) = File::open(&ARK_CONFIG) {
            println!(
                "\tRoots config was found automatically:\n\t\t{}",
                &ARK_CONFIG
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
                storages_exists(path)
            });

            if let Some(root) = result {
                println!("Root folder found:\n\t{}", root.display());
                vec![root.to_path_buf()]
            } else {
                println!("Root folder wasn't found.");
                vec![]
            }
        }
    }
}

fn provide_root(root_dir: &Option<PathBuf>) -> PathBuf {
    if let Some(path) = root_dir {
        path.clone()
    } else {
        current_dir()
            .expect("Can't open current directory!")
            .clone()
    }
}

// Read-only structure
fn provide_index(root_dir: &PathBuf) -> ResourceIndex {
    let rwlock =
        arklib::provide_index(root_dir).expect("Failed to retrieve index");
    let index = &*rwlock.read().unwrap();
    index.clone()
}

fn monitor_index(root_dir: &Option<PathBuf>, interval: Option<u64>) {
    let dir_path = provide_root(root_dir);

    println!("Building index of folder {}", dir_path.display());
    let start = Instant::now();
    let dir_path = provide_root(root_dir);
    let result = arklib::provide_index(dir_path);
    let duration = start.elapsed();

    match result {
        Ok(rwlock) => {
            println!("Build succeeded in {:?}\n", duration);

            if let Some(millis) = interval {
                let mut index = rwlock.write().unwrap();
                loop {
                    let pause = Duration::from_millis(millis);
                    thread::sleep(pause);

                    let start = Instant::now();
                    match index.update_all() {
                        Err(msg) => println!("Oops! {}", msg),
                        Ok(diff) => {
                            index.store().expect("Could not store index");
                            let duration = start.elapsed();
                            println!("Updating succeeded in {:?}\n", duration);

                            if !diff.deleted.is_empty() {
                                println!("Deleted: {:?}", diff.deleted);
                            }
                            if !diff.added.is_empty() {
                                println!("Added: {:?}", diff.added);
                            }
                        }
                    }
                }
            } else {
                let index = rwlock.read().unwrap();

                println!("Here are {} entries in the index", index.size());

                for (key, count) in index.collisions.iter() {
                    println!("Id {:?} calculated {} times", key, count);
                }
            }
        }
        Err(err) => println!("Failure: {:?}", err),
    }
}

fn storages_exists(path: &Path) -> bool {
    let meta = metadata(path.join(&arklib::ARK_FOLDER));
    if let Ok(meta) = meta {
        return meta.is_dir();
    }

    false
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

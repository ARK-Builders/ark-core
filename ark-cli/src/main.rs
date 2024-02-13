use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use arklib::id::ResourceId;
use arklib::pdf::PDFQuality;
use arklib::{app_id, provide_index, INDEX_PATH};
use arklib::{
    modify, AtomicFile, APP_ID_FILE, ARK_FOLDER, FAVORITES_FILE,
    METADATA_STORAGE_FOLDER, PREVIEWS_STORAGE_FOLDER,
    PROPERTIES_STORAGE_FOLDER, SCORE_STORAGE_FILE, STATS_FOLDER,
    TAG_STORAGE_FILE, THUMBNAILS_STORAGE_FOLDER,
};

use chrono::prelude::DateTime;
use chrono::Utc;

use clap::{Parser, Subcommand};

use fs_extra::dir::{self, CopyOptions};

use home::home_dir;

use crate::parsers::Format;
use crate::storage::Storage;
use storage::StorageType;
use util::{
    discover_roots, monitor_index, provide_root, read_storage_value,
    storages_exists, timestamp, translate_storage,
};

mod commands;
mod parsers;
mod storage;
mod util;

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

    List {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,

        #[clap(long, short)]
        entry: Option<String>,

        #[clap(long, short)]
        timestamp: Option<bool>,

        #[clap(long, short = 'g')]
        tags: Option<bool>,

        #[clap(long, short)]
        scores: Option<bool>,
    },

    #[clap(subcommand)]
    Link(Link),

    #[clap(subcommand)]
    File(FileCommand),

    #[clap(subcommand)]
    Storage(StorageCommand),
}

#[derive(Subcommand, Debug)]
enum StorageCommand {
    List {
        #[clap(parse(from_os_str))]
        root_dir: Option<PathBuf>,

        storage: Option<String>,

        #[clap(short, long)]
        versions: Option<bool>,

        #[clap(short, long)]
        type_: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum FileCommand {
    Append {
        #[clap(parse(from_os_str))]
        root_dir: PathBuf,

        storage: String,

        id: String,

        content: String,

        #[clap(short, long)]
        format: Option<String>,

        #[clap(short, long)]
        type_: Option<String>,
    },

    Insert {
        #[clap(parse(from_os_str))]
        root_dir: PathBuf,

        storage: String,

        id: String,

        content: String,

        #[clap(short, long)]
        format: Option<String>,

        #[clap(short, long)]
        type_: Option<String>,
    },

    Read {
        #[clap(parse(from_os_str))]
        root_dir: PathBuf,

        storage: String,

        id: String,

        #[clap(short, long)]
        type_: Option<String>,
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

enum EntryOutput {
    Id,
    Path,
    Both,
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
        Command::List {
            entry,
            root_dir,
            timestamp,
            tags,
            scores,
        } => {
            let root = provide_root(root_dir);

            let entry_output: EntryOutput = match entry {
                Some(entry) => match entry.to_lowercase().as_str() {
                    "id" => EntryOutput::Id,
                    "path" => EntryOutput::Path,
                    "both" => EntryOutput::Both,
                    _ => panic!("unknown entry option"),
                },
                None => EntryOutput::Id,
            };

            let index = provide_index(&root).expect("could not provide index");

            let resource_index = index.read().unwrap();

            for (path, resource) in resource_index.path2id.iter() {
                let tags_list = read_storage_value(
                    &root,
                    "tags",
                    &resource.id.to_string(),
                    &None,
                )
                .unwrap_or("".to_string());

                let scores_list = read_storage_value(
                    &root,
                    "scores",
                    &resource.id.to_string(),
                    &None,
                )
                .unwrap_or("0".to_string());

                let mut output: String = match entry_output {
                    EntryOutput::Id => resource.id.to_string(),
                    EntryOutput::Path => path.display().to_string(),
                    EntryOutput::Both => {
                        format!("{}@{}", resource.id, path.display())
                    }
                };

                let datetime = DateTime::<Utc>::from(resource.modified);

                if timestamp.unwrap_or(false) {
                    let timestamp_str = datetime
                        .format("%Y-%m-%d %H:%M:%S.%f")
                        .to_string();
                    output.push_str(&format!(
                        " last modified on {}",
                        timestamp_str
                    ));
                }

                if tags.unwrap_or(false) {
                    output.push_str(&format!(" with tags {}", tags_list));
                }

                if scores.unwrap_or(false) {
                    output.push_str(&format!(" with score {}", scores_list));
                }

                println!("{}", output);
            }
        }

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
                let url = url.as_ref().expect("ERROR: Url was not provided");
                let title = title
                    .as_ref()
                    .expect("ERROR: Title was not provided");

                println!("Saving link...");

                match commands::link::create_link(
                    &root,
                    url,
                    title,
                    desc.to_owned(),
                )
                .await
                {
                    Ok(_) => {
                        println!("Link saved successfully!");
                    }
                    Err(e) => println!("ERROR: {}", e),
                }
            }

            Link::Load {
                root_dir,
                file_path,
                id,
            } => {
                let root = provide_root(root_dir);
                let link = commands::link::load_link(&root, file_path, id);

                match link {
                    Ok(link) => {
                        println!("Link data:\n{:?}", link);
                    }
                    Err(e) => println!("ERROR: {}", e),
                }
            }
        },
        Command::File(file) => match &file {
            FileCommand::Append {
                root_dir,
                storage,
                id,
                content,
                format,
                type_,
            } => {
                let (file_path, storage_type) =
                    translate_storage(&Some(root_dir.to_owned()), storage)
                        .expect("ERROR: Could not find storage folder");

                let storage_type = storage_type.unwrap_or(match type_ {
                    Some(type_) => match type_.to_lowercase().as_str() {
                        "file" => StorageType::File,
                        "folder" => StorageType::Folder,
                        _ => panic!("unknown storage type"),
                    },
                    None => StorageType::File,
                });

                let format =
                    parsers::get_format(&format).unwrap_or(Format::Raw);

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                let resource_id = ResourceId::from_str(id)
                    .expect("ERROR: Could not parse id");

                storage
                    .append(resource_id, content, format)
                    .unwrap();
            }

            FileCommand::Insert {
                root_dir,
                storage,
                id,
                content,
                format,
                type_,
            } => {
                let (file_path, storage_type) =
                    translate_storage(&Some(root_dir.to_owned()), storage)
                        .expect("ERROR: Could not find storage folder");

                let storage_type = storage_type.unwrap_or(match type_ {
                    Some(type_) => match type_.to_lowercase().as_str() {
                        "file" => StorageType::File,
                        "folder" => StorageType::Folder,
                        _ => panic!("unknown storage type"),
                    },
                    None => StorageType::File,
                });

                let format =
                    parsers::get_format(&format).unwrap_or(Format::Raw);

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                let resource_id = ResourceId::from_str(id)
                    .expect("ERROR: Could not parse id");

                storage
                    .insert(resource_id, content, format)
                    .unwrap();
            }

            FileCommand::Read {
                root_dir,
                storage,
                id,
                type_,
            } => {
                let (file_path, storage_type) =
                    translate_storage(&Some(root_dir.to_owned()), storage)
                        .expect("ERROR: Could not find storage folder");

                let storage_type = storage_type.unwrap_or(match type_ {
                    Some(type_) => match type_.to_lowercase().as_str() {
                        "file" => StorageType::File,
                        "folder" => StorageType::Folder,
                        _ => panic!("unknown storage type"),
                    },
                    None => StorageType::File,
                });

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                let resource_id = ResourceId::from_str(id)
                    .expect("ERROR: Could not parse id");

                let output = storage.read(resource_id);

                match output {
                    Ok(output) => println!("{}", output),
                    Err(e) => println!("ERROR: {}", e),
                }
            }
        },
        Command::Storage(cmd) => match &cmd {
            StorageCommand::List {
                root_dir,
                storage,
                type_,
                versions,
            } => {
                let storage = storage
                    .as_ref()
                    .expect("ERROR: Storage was not provided");

                let versions = versions.unwrap_or(false);

                let (file_path, storage_type) =
                    translate_storage(root_dir, storage)
                        .expect("ERROR: Could not find storage folder");

                let storage_type = storage_type.unwrap_or(match type_ {
                    Some(type_) => match type_.to_lowercase().as_str() {
                        "file" => StorageType::File,
                        "folder" => StorageType::Folder,
                        _ => panic!("unknown storage type"),
                    },
                    None => StorageType::File,
                });

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                storage
                    .load()
                    .expect("ERROR: Could not load storage");

                let output = storage
                    .list(versions)
                    .expect("ERROR: Could not list storage content");

                println!("{}", output);
            }
        },
    }
}

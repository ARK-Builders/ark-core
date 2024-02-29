use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use arklib::id::ResourceId;
use arklib::pdf::PDFQuality;
use arklib::{app_id, provide_index};

use chrono::prelude::DateTime;
use chrono::Utc;

use clap::Parser;

use fs_extra::dir::{self, CopyOptions};

use home::home_dir;

use crate::models::cli::{Command, FileCommand, Link, StorageCommand};
use crate::models::entry::EntryOutput;
use crate::models::format::Format;
use crate::models::sort::Sort;
use crate::models::storage::{Storage, StorageType};

use util::{
    discover_roots, monitor_index, provide_root, read_storage_value,
    storages_exists, timestamp, translate_storage,
};

mod commands;
mod models;
mod util;

const ARK_CONFIG: &str = ".config/ark";
const ARK_BACKUPS_PATH: &str = ".ark-backups";
const ROOTS_CFG_FILENAME: &str = "roots";

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = models::cli::Cli::parse();

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
            entry_id,
            entry_path,

            root_dir,
            modified,
            tags,
            scores,
            sort,
            filter,
        } => {
            let root = provide_root(root_dir);

            let entry_output = match (entry, entry_id, entry_path) {
                (Some(e), false, false) => e,
                (None, true, false) => &EntryOutput::Id,
                (None, false, true) => &EntryOutput::Path,
                (None, true, true) => &EntryOutput::Both,
                (None, false, false) => &EntryOutput::Id, // default mode
                _ => panic!(
                    "incompatible entry output options, please choose only one"
                ),
            };

            let index = provide_index(&root).expect("could not provide index");

            let resource_index = index.read().expect("could not read index");

            let mut resources = resource_index
                .path2id
                .iter()
                .map(|(path, resource)| {
                    let tags_list = read_storage_value(
                        &root,
                        "tags",
                        &resource.id.to_string(),
                        &None,
                    )
                    .unwrap_or("NO_TAGS".to_string());

                    let scores_list = read_storage_value(
                        &root,
                        "scores",
                        &resource.id.to_string(),
                        &None,
                    )
                    .unwrap_or("NO_SCORE".to_string());

                    let datetime = DateTime::<Utc>::from(resource.modified);

                    (path, resource, tags_list, scores_list, datetime)
                })
                .collect::<Vec<_>>();

            match sort {
                Some(Sort::Asc) => resources
                    .sort_by(|(_, _, _, _, a), (_, _, _, _, b)| a.cmp(b)),

                Some(Sort::Desc) => resources
                    .sort_by(|(_, _, _, _, a), (_, _, _, _, b)| b.cmp(a)),
                None => (),
            };

            if let Some(filter) = filter {
                resources = resources
                    .into_iter()
                    .filter(|(_, _, tags_list, _, _)| {
                        tags_list
                            .split(',')
                            .any(|tag| tag.trim() == filter)
                    })
                    .collect();
            }

            for (path, resource, tags_list, scores_list, datetime) in resources
            {
                let mut output = String::new();

                let entry_str = match entry_output {
                    EntryOutput::Id => resource.id.to_string(),
                    EntryOutput::Path => path.display().to_string(),
                    EntryOutput::Both => {
                        format!("{}@{}", resource.id, path.display())
                    }
                };

                output.push_str(&entry_str);

                if *modified {
                    let timestamp_str = datetime
                        .format("%Y-%m-%d %H:%M:%S.%f")
                        .to_string();
                    output.push_str(&format!(
                        " last modified on {}",
                        timestamp_str
                    ));
                }

                if *tags {
                    output.push_str(&format!(" with tags {}", tags_list));
                }

                if *scores {
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
                    Some(t) => *t,
                    None => StorageType::File,
                });

                let format = format.unwrap_or(Format::Raw);

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                let resource_id = ResourceId::from_str(id)
                    .expect("ERROR: Could not parse id");

                storage
                    .append(resource_id, content, format)
                    .expect("ERROR: Could not append content to storage");
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
                    Some(t) => *t,
                    None => StorageType::File,
                });

                let format = format.unwrap_or(Format::Raw);

                let mut storage = Storage::new(file_path, storage_type)
                    .expect("ERROR: Could not create storage");

                let resource_id = ResourceId::from_str(id)
                    .expect("ERROR: Could not parse id");

                storage
                    .insert(resource_id, content, format)
                    .expect("ERROR: Could not insert content to storage");
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
                    Some(t) => *t,
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
                    Some(t) => *t,
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

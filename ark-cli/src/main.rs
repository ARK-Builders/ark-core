use std::env::current_dir;
use std::fs::{canonicalize, copy, create_dir_all, File};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arklib::id::ResourceId;
use arklib::index::ResourceIndex;
use arklib::pdf::PDFQuality;
use clap::{Parser, Subcommand};
use home::home_dir;
use url::Url;

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

    #[clap(subcommand)]
    Link(Link),
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

const ARK_HOME: &str = ".ark";

const ROOTS_CFG_FILENAME: &str = "roots";
const HOME_BACKUPS_DIRNAME: &str = "backups";

fn main() {
    env_logger::init();

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
            let roots = discover_roots(roots_cfg);

            let (valid, invalid): (Vec<PathBuf>, Vec<PathBuf>) = roots
                .into_iter()
                .partition(|root| storages_exists(&root));

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
                    let result = copy(
                        root.join(&arklib::STORAGES_FOLDER),
                        storage_backup,
                    );
                    if let Err(e) = result {
                        println!("\t\tFailed to copy tag storage!\n\t\t{}", e);
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
                let mut link: arklib::link::Link = arklib::link::Link::new(
                    url.unwrap(),
                    title.to_owned().unwrap(),
                    desc.to_owned(),
                );

                let timestamp = timestamp().as_secs();
                let path = Path::join(
                    &root,
                    format!("{}.link", &timestamp.to_string()),
                );
                link.write_to_path_sync(root, path.clone(), true)
                    .unwrap();
                println!("Link saved successfully: {:?}", path.display())
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
    }
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
                    match index.update() {
                        Err(msg) => println!("Oops! {}", msg),
                        Ok(diff) => {
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

fn home_roots_cfg() -> PathBuf {
    return home_dir()
        .expect("Couldn't retrieve home directory!")
        .join(&ARK_HOME)
        .join(&ROOTS_CFG_FILENAME);
}

fn storages_exists(path: &Path) -> bool {
    return File::open(path.join(&arklib::STORAGES_FOLDER)).is_ok();
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

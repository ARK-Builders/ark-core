use std::env::current_dir;
use std::fs::{canonicalize, copy, create_dir_all, File};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        path: Option<PathBuf>,
        title: Option<String>,
        desc: Option<String>,
        url: Option<String>,
    },
    Load {
        #[clap(parse(from_os_str))]
        file_path: Option<PathBuf>,
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
                    let result = copy(
                        root.join(&arklib::TAG_STORAGE_FILENAME),
                        storage_backup,
                    );
                    if let Err(e) = result {
                        println!("\t\tFailed to copy tag storage!\n\t\t{}", e);
                    }
                });

            println!("Backup created:\n\t{}", backup_dir.display());
        }

        Command::Collisions { root_dir } => build_index(&root_dir, None),
        Command::Monitor { root_dir, interval } => {
            let millis = interval.unwrap_or(1000);
            build_index(&root_dir, Some(millis))
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
                title,
                desc,
                url,
                path,
            } => {
                let _url = Url::parse(url.as_deref().unwrap());
                let mut _link: arklib::link::Link = arklib::link::Link::new(
                    title.to_owned().unwrap(),
                    desc.to_owned().unwrap(),
                    _url.unwrap(),
                );
                let file_path = Path::join(
                    path.to_owned().unwrap().as_path(),
                    format!("{}.link", _link.format_name()),
                );
                _link.write_to_path_sync(file_path.clone(), true);
                println!(
                    "Link saved successfully: {:?}",
                    file_path.clone().display()
                )
            }
            Link::Load { file_path } => {
                let link = arklib::link::Link::load_json(
                    file_path.to_owned().unwrap().as_path(),
                );
                println!("Link data:\n{}", link.unwrap());
            }
        },
    }
}

fn build_index(root_dir: &Option<PathBuf>, interval: Option<u64>) {
    let dir_path = if let Some(path) = root_dir {
        path.clone()
    } else {
        current_dir()
            .expect("Can't open current directory!")
            .clone()
    };

    println!("Building index of folder {}", dir_path.display());
    let start = Instant::now();
    let result = arklib::provide_index(dir_path);
    let duration = start.elapsed();

    match result {
        Ok(rwlock) => {
            println!("Success, took {:?}\n", duration);

            if let Some(millis) = interval {
                let mut index = rwlock.write().unwrap();
                loop {
                    let pause = Duration::from_millis(millis);
                    thread::sleep(pause);

                    match index.update() {
                        Err(msg) => println!("Oops! {}", msg),
                        Ok(diff) => {
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

fn tag_storage_exists(path: &Path) -> bool {
    return File::open(path.join(&arklib::TAG_STORAGE_FILENAME)).is_ok();
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

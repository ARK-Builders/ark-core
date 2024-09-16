use std::path::PathBuf;

use futures::{pin_mut, StreamExt};

use fs_index::{watch_index, WatchEvent};

use crate::{AppError, DateTime, ResourceId, Utc};

#[derive(Clone, Debug, clap::Args)]
#[clap(
    name = "watch",
    about = "Watch the ark managed folder for changes and update the index accordingly"
)]
pub struct Watch {
    #[clap(
        help = "Path to the directory to watch for changes",
        default_value = ".",
        value_parser
    )]
    path: PathBuf,
}

impl Watch {
    pub async fn run(&self) -> Result<(), AppError> {
        let stream = watch_index::<_, ResourceId>(&self.path);
        pin_mut!(stream);

        while let Some(value) = stream.next().await {
            match value {
                WatchEvent::UpdatedOne(update) => {
                    println!("Index updated with a single file change");

                    let added = update.added();
                    let removed = update.removed();
                    for file in added {
                        let time_stamped_path = file.1.iter().next().unwrap();
                        let file_path = time_stamped_path.item();
                        let last_modified = time_stamped_path.last_modified();
                        let last_modified: DateTime<Utc> = last_modified.into();
                        println!(
                            "\tAdded file: {:?} (last modified: {})",
                            file_path,
                            last_modified.format("%d/%m/%Y %T")
                        );
                    }
                    for file in removed {
                        println!("\tRemoved file with hash: {:?}", file);
                    }
                }
                WatchEvent::UpdatedAll(update) => {
                    println!("Index fully updated");

                    let added = update.added();
                    let removed = update.removed();

                    for file in added {
                        let time_stamped_path = file.1.iter().next().unwrap();
                        let file_path = time_stamped_path.item();
                        let last_modified = time_stamped_path.last_modified();
                        println!(
                            "\tAdded file: {:?} (last modified: {:?})",
                            file_path, last_modified
                        );
                    }
                    for file in removed {
                        println!("\tRemoved file with hash: {:?}", file);
                    }
                }
            }
        }

        Ok(())
    }
}

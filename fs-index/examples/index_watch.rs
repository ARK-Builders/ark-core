use std::path::Path;

use anyhow::Result;
use futures::{pin_mut, StreamExt};

use dev_hash::Blake3;
use fs_index::{watch_index, WatchEvent};

/// A simple example of using `watch_index` to monitor a directory for file
/// changes. This asynchronously listens for updates and prints the paths of
/// changed files.
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Change this to the path of the directory you want to watch
    let root = Path::new("test-assets");

    let stream = watch_index::<_, Blake3>(root);

    pin_mut!(stream); // needed for iteration

    while let Some(value) = stream.next().await {
        match value {
            WatchEvent::UpdatedOne(update) => {
                println!("Updated file: {:?}", update);
            }
            WatchEvent::UpdatedAll(update) => {
                println!("Updated all: {:?}", update);
            }
        }
    }

    Ok(())
}

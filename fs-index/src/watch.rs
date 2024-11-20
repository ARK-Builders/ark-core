use std::{fs, path::Path, thread, time::Duration};

use async_stream::stream;
use futures::Stream;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::new_debouncer;
use tokio::sync::mpsc;

use data_resource::ResourceId;
use fs_storage::ARK_FOLDER;

use crate::{IndexUpdate, ResourceIndex};

/// Represents the different kinds of events that can occur when watching the
/// resource index.
#[derive(Debug)]
pub enum WatchEvent<Id: ResourceId> {
    /// Represents an update to a single resource.
    UpdatedOne(IndexUpdate<Id>),
    /// Represents an update to all resources.
    UpdatedAll(IndexUpdate<Id>),
}

/// Watches for file system changes and emits events related to the
/// [ResourceIndex].
///
/// This function sets up a file watcher that monitors a specified root path for
/// changes to files. It sends events such as file creations, modifications,
/// renames, and deletions through an asynchronous stream. The function uses a
/// debouncer to ensure that multiple rapid events are collapsed into a single
/// event.
pub fn watch_index<P: AsRef<Path>, Id: ResourceId + 'static>(
    root_path: P,
) -> impl Stream<Item = WatchEvent<Id>> {
    log::debug!(
        "Attempting to watch index at root path: {:?}",
        root_path.as_ref()
    );

    let root_path = fs::canonicalize(root_path.as_ref()).unwrap();
    let mut index: ResourceIndex<Id> =
        ResourceIndex::build(&root_path).unwrap();
    index.store().unwrap();

    let (tx, mut rx) = mpsc::channel(100);
    let ark_folder = root_path.join(ARK_FOLDER);

    // We need to spawn a new thread to run the blocking file system watcher
    thread::spawn(move || {
        // Setup the synchronous channel (notify debouncer expects this)
        let (sync_tx, sync_rx) = std::sync::mpsc::channel();

        let mut debouncer =
            new_debouncer(Duration::from_secs(2), None, sync_tx).unwrap();
        let watcher = debouncer.watcher();
        watcher
            .watch(&root_path, RecursiveMode::Recursive)
            .unwrap();
        log::info!("Started debouncer file system watcher for {:?}", root_path);

        while let Ok(events) = sync_rx.recv() {
            let events = match events {
                Ok(evts) => evts,
                Err(errs) => {
                    for err in errs {
                        log::error!("Error receiving event: {:?}", err);
                    }
                    continue;
                }
            };

            // Send events to the async channel
            for event in events {
                log::trace!("Received event: {:?}", event);

                // If the event is a change in the .ark folder, ignore it
                if event
                    .paths
                    .iter()
                    .any(|p| p.starts_with(&ark_folder))
                {
                    continue;
                }

                let event_kind = event.event.kind;
                // We only care for:
                // - file modifications
                // - file renames
                // - file creations
                // - file deletions
                match event_kind {
                    notify::EventKind::Modify(
                        notify::event::ModifyKind::Data(_),
                    )
                    | notify::EventKind::Modify(
                        notify::event::ModifyKind::Name(_),
                    )
                    // On macOS, we noticed that force deleting a file
                    // triggers a metadata change event for some reason
                    | notify::EventKind::Modify(
                        notify::event::ModifyKind::Metadata(notify::event::MetadataKind::Any),
                    )
                    | notify::EventKind::Create(
                        notify::event::CreateKind::File,
                    )
                    | notify::EventKind::Remove(
                        notify::event::RemoveKind::File,
                    ) => {}
                    _ => continue,
                }

                let watch_event: WatchEvent<Id> = if event.need_rescan() {
                    log::info!("Detected rescan event: {:?}", event);
                    match index.update_all() {
                        Ok(update_result) => {
                            WatchEvent::UpdatedAll(update_result)
                        }
                        Err(e) => {
                            log::error!("Failed to update all: {:?}", e);
                            continue;
                        }
                    }
                } else {
                    // Update the index for the specific file
                    let file = event
                        .paths
                        .first()
                        .expect("Failed to get file path from event");

                    let relative_path = match file.strip_prefix(&root_path) {
                        Ok(path) => path,
                        Err(e) => {
                            log::error!("Failed to get relative path: {:?}", e);
                            continue;
                        }
                    };

                    match index.update_one(relative_path) {
                        Ok(update_result) => {
                            WatchEvent::UpdatedOne(update_result)
                        }
                        Err(e) => {
                            log::error!("Failed to update one: {:?}", e);
                            continue;
                        }
                    }
                };

                if let Err(e) = index.store() {
                    log::error!("Failed to store index: {:?}", e);
                }

                // Use blocking send to the async channel because we are in a
                // separate thread
                if tx.blocking_send(watch_event).is_err() {
                    log::error!("Failed to send event to async channel");
                    break;
                }
            }
        }
    });

    // Create an async stream that reads from the receiver
    stream! {
        while let Some(event) = rx.recv().await {
            yield event;
        }
    }
}

#![deny(clippy::all)]
#[macro_use]
extern crate lazy_static;
extern crate canonical_path;

pub mod errors;
pub use errors::{ArklibError, Result};

pub mod id;
pub mod index;

pub mod link;
pub mod pdf;

mod atomic;
mod storage;
mod util;

pub use atomic::{modify, modify_json, AtomicFile};

use index::ResourceIndex;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use canonical_path::CanonicalPathBuf;
use std::sync::Once;

use crate::id::app_id;

pub static INIT: Once = Once::new();

pub const ARK_FOLDER: &str = ".ark";

// must not be lost (user data)
pub const STATS_FOLDER: &str = "stats";
pub const FAVORITES_FILE: &str = "favorites";
pub const DEVICE_ID: &str = "device";

// User-defined data
pub const TAG_STORAGE_FILE: &str = "user/tags";
pub const SCORE_STORAGE_FILE: &str = "user/scores";
pub const PROPERTIES_STORAGE_FOLDER: &str = "user/properties";

// Generated data
pub const INDEX_PATH: &str = "index";
pub const METADATA_STORAGE_FOLDER: &str = "cache/metadata";
pub const PREVIEWS_STORAGE_FOLDER: &str = "cache/previews";
pub const THUMBNAILS_STORAGE_FOLDER: &str = "cache/thumbnails";

pub const APP_ID_FILE: &str = "app_id";

pub type ResourceIndexLock = Arc<RwLock<ResourceIndex>>;

lazy_static! {
    pub static ref REGISTRAR: RwLock<HashMap<CanonicalPathBuf, ResourceIndexLock>> =
        RwLock::new(HashMap::new());
}
lazy_static! {
    pub static ref APP_ID_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
}

pub fn initialize() {
    INIT.call_once(|| {
        log::info!("Initializing arklib");
        app_id::load("./").unwrap();
    });
}

pub fn provide_index<P: AsRef<Path>>(
    root_path: P,
) -> Result<Arc<RwLock<ResourceIndex>>> {
    let root_path = CanonicalPathBuf::canonicalize(root_path)?;

    {
        let registrar = REGISTRAR.read().unwrap();

        if let Some(index) = registrar.get(&root_path) {
            log::info!("Index has been registered before");
            return Ok(index.clone());
        }
    }

    log::info!("Index has not been registered before");
    match ResourceIndex::provide(&root_path) {
        Ok(index) => {
            let mut registrar = REGISTRAR.write().unwrap();
            let arc = Arc::new(RwLock::new(index));
            registrar.insert(root_path, arc.clone());

            log::info!("Index was registered");
            Ok(arc)
        }
        Err(e) => Err(e),
    }
}

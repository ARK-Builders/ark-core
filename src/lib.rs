#[macro_use]
extern crate lazy_static;
extern crate canonical_path;
pub mod errors;
pub use errors::{ArklibError, Result};
pub mod id;
pub mod link;
pub mod pdf;

pub mod index;
mod meta;

use index::ResourceIndex;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use canonical_path::CanonicalPathBuf;

use log;

pub const ARK_FOLDER: &str = ".ark";

// must not be lost (user data)
pub const STATS_FOLDER: &str = "stats";
pub const FAVORITES_FILE: &str = "favorites";

// User-defined data
pub const TAG_STORAGE_FILE: &str = "user/tags";
pub const SCORE_STORAGE_FILE: &str = "user/scores";
pub const PROPERTIES_STORAGE_FOLDER: &str = "user/properties";

// Generated data
pub const INDEX_PATH: &str = "index";
pub const METADATA_STORAGE_FOLDER: &str = "cache/metadata";
pub const PREVIEWS_STORAGE_FOLDER: &str = "cache/previews";
pub const THUMBNAILS_STORAGE_FOLDER: &str = "cache/thumbnails";

pub type ResourceIndexLock = Arc<RwLock<ResourceIndex>>;

lazy_static! {
    pub static ref REGISTRAR: RwLock<HashMap<CanonicalPathBuf, ResourceIndexLock>> =
        RwLock::new(HashMap::new());
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

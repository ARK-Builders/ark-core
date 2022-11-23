#[macro_use]
extern crate lazy_static;
extern crate canonical_path;
pub mod id;
pub mod link;
pub mod pdf;

mod index;
mod meta;

use index::ResourceIndex;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use canonical_path::CanonicalPathBuf;

use anyhow::Error;
use log;

pub const STORAGES_FOLDER: &str = ".ark";

// allowed to be lost (cache)
pub const INDEX_PATH: &str = "index";
pub const PREVIEWS_PATH: &str = "previews";

// must not be lost (user data)
pub const METADATA_PATH: &str = "meta";
pub const TAGS_PATH: &str = "tags";

pub type ResourceIndexLock = Arc<RwLock<ResourceIndex>>;

lazy_static! {
    pub static ref REGISTRAR: RwLock<HashMap<CanonicalPathBuf, ResourceIndexLock>> =
        RwLock::new(HashMap::new());
}

pub fn provide_index<P: AsRef<Path>>(
    root_path: P,
) -> Result<Arc<RwLock<ResourceIndex>>, Error> {
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

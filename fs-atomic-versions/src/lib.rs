#![deny(clippy::all)]
#[macro_use]
extern crate lazy_static;
extern crate canonical_path;

pub mod errors;
pub use errors::{ArklibError, Result};

pub mod app_id;
mod atomic;

pub use atomic::{modify, modify_json, AtomicFile};

use index::ResourceIndex;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use canonical_path::CanonicalPathBuf;
use std::sync::Once;

pub static INIT: Once = Once::new();

pub const ARK_FOLDER: &str = ".ark";

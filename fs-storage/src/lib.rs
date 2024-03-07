pub mod file_storage;
pub mod meta;
pub mod prop;

pub const ARK_FOLDER: &str = ".ark";

// Should not be lost if possible
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

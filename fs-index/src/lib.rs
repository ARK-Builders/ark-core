mod index;
mod serde;
mod utils;
#[cfg(feature = "watch")]
mod watch;

pub use index::{IndexUpdate, ResourceIndex};
pub use utils::load_or_build_index;
#[cfg(feature = "watch")]
pub use watch::{watch_index, WatchEvent};

#[cfg(test)]
mod tests;

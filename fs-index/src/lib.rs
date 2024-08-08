mod index;
mod serde;
mod utils;

pub use utils::load_or_build_index;

pub use index::ResourceIndex;

#[cfg(test)]
mod tests;

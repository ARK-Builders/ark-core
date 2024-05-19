use std::path::PathBuf;

use crate::{monitor_index, AppError};

#[derive(Clone, Debug, clap::Args)]
#[clap(
    name = "collisions",
    about = "Find collisions in the ark managed folder"
)]
pub struct Collisions {
    #[clap(value_parser, help = "Path to the root directory")]
    root_dir: Option<PathBuf>,
}

impl Collisions {
    pub fn run(&self) -> Result<(), AppError> {
        monitor_index(&self.root_dir, None)
    }
}

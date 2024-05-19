use std::path::PathBuf;

use crate::{monitor_index, AppError};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "monitor", about = "Monitor the ark managed folder")]
pub struct Monitor {
    #[clap(value_parser, help = "Path to the root directory")]
    root_dir: Option<PathBuf>,
    #[clap(help = "Interval to check for changes")]
    interval: Option<u64>,
}

impl Monitor {
    pub fn run(&self) -> Result<(), AppError> {
        let millis = self.interval.unwrap_or(1000);
        monitor_index(&self.root_dir, Some(millis))
    }
}

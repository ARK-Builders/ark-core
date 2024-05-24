use std::path::PathBuf;

use crate::{monitor_index, AppError};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "monitor", about = "Monitor the ark managed folder")]
pub struct Monitor {
    #[clap(value_parser, help = "Path to the root directory")]
    root_dir: Option<PathBuf>,
    // FIXME: help message should specify what metric the interval is in
    #[clap(help = "Interval to check for changes")]
    interval: Option<u64>,
}

impl Monitor {
    pub fn run(&self) -> Result<(), AppError> {
        // FIXME: 1000 should be the default value in clap configuration
        //        so users know
        let millis = self.interval.unwrap_or(1000);
        monitor_index(&self.root_dir, Some(millis))
    }
}

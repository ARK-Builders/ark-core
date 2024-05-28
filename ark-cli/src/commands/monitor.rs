use std::path::PathBuf;

use crate::{monitor_index, AppError};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "monitor", about = "Monitor the ark managed folder")]
pub struct Monitor {
    #[clap(value_parser, help = "Path to the root directory")]
    root_dir: Option<PathBuf>,
    #[clap(
        default_value = "1000",
        help = "Interval to check for changes in milliseconds"
    )]
    interval: Option<u64>,
}

impl Monitor {
    pub fn run(&self) -> Result<(), AppError> {
        // SAFETY: interval is always Some since it has a default value in clap
        let millis = self.interval.unwrap();
        monitor_index(&self.root_dir, Some(millis))
    }
}

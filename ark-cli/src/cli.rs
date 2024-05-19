use crate::commands::Commands;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "ark-cli")]
#[clap(about = "Manage ARK tag storages and indexes", long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

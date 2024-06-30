use crate::commands::Commands;

use clap::{builder::styling::AnsiColor, Parser};

#[derive(Parser, Debug)]
#[clap(name = "ark-cli")]
#[clap(about = "Manage ARK tag storages and indexes", styles=styles())]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

pub fn styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Yellow.on_default())
        .literal(AnsiColor::Green.on_default())
}

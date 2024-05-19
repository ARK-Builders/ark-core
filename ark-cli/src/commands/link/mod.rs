use clap::Subcommand;

pub mod create;
mod load;
mod utils;

/// Available commands for the `link` subcommand
#[derive(Subcommand, Debug)]
pub enum Link {
    Create(create::Create),
    Load(load::Load),
}

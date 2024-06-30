use clap::Subcommand;

mod list;

/// Available commands for the `storage` subcommand
#[derive(Subcommand, Debug)]
pub enum Storage {
    List(list::List),
}

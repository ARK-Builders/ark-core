use clap::Subcommand;

mod list;

// FIXME: We should use new `fs-storage` crate to handle storage operations

/// Available commands for the `storage` subcommand
#[derive(Subcommand, Debug)]
pub enum Storage {
    List(list::List),
}

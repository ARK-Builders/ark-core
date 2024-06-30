use clap::Subcommand;

mod append;
mod insert;
mod read;
mod utils;

/// Available commands for the `file` subcommand
#[derive(Subcommand, Debug)]
pub enum File {
    Append(append::Append),
    Insert(insert::Insert),
    Read(read::Read),
}

pub use utils::{file_append, file_insert, format_file, format_line};

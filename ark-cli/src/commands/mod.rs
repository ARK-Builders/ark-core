use clap::Subcommand;

mod backup;
mod collisions;
pub mod file;
pub mod link;
mod list;
mod monitor;
mod render;
pub mod storage;

pub use file::{file_append, file_insert, format_file, format_line};

#[derive(Debug, Subcommand)]
pub enum Commands {
    Backup(backup::Backup),
    Collisions(collisions::Collisions),
    Monitor(monitor::Monitor),
    Render(render::Render),
    List(list::List),
    #[command(about = "Manage links")]
    Link {
        #[clap(subcommand)]
        subcommand: link::Link,
    },
    #[command(about = "Manage files")]
    File {
        #[clap(subcommand)]
        subcommand: file::File,
    },
    #[command(about = "Manage storage")]
    Storage {
        #[clap(subcommand)]
        subcommand: storage::Storage,
    },
}

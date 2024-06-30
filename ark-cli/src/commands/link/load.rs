use std::path::PathBuf;

use crate::{
    commands::link::utils::load_link, provide_root, AppError, ResourceId,
};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "create", about = "Create a new link")]
pub struct Load {
    #[clap(value_parser, help = "Root directory of the ark managed folder")]
    root_dir: Option<PathBuf>,
    #[clap(value_parser, help = "Path to the file to load")]
    file_path: Option<PathBuf>,
    #[clap(help = "ID of the resource to load")]
    id: Option<ResourceId>,
}

impl Load {
    pub fn run(&self) -> Result<(), AppError> {
        let root = provide_root(&self.root_dir)?;
        let link = load_link(&root, &self.file_path, &self.id)?;
        println!("Link data:\n{:?}", link);

        Ok(())
    }
}

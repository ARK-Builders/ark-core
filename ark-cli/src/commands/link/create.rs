use std::path::PathBuf;

use crate::{commands::link::utils::create_link, provide_root, AppError};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "create", about = "Create a new link")]
pub struct Create {
    #[clap(value_parser, help = "Root directory of the ark managed folder")]
    root_dir: Option<PathBuf>,
    #[clap(help = "URL of the link")]
    url: Option<String>,
    #[clap(help = "Title of the link")]
    title: Option<String>,
    #[clap(help = "Description of the link")]
    desc: Option<String>,
}

impl Create {
    pub async fn run(&self) -> Result<(), AppError> {
        let root = provide_root(&self.root_dir)?;
        let url = self.url.as_ref().ok_or_else(|| {
            AppError::LinkCreationError("Url was not provided".to_owned())
        })?;
        let title = self.title.as_ref().ok_or_else(|| {
            AppError::LinkCreationError("Title was not provided".to_owned())
        })?;

        println!("Saving link...");

        match create_link(&root, url, title, self.desc.to_owned()).await {
            Ok(_) => {
                println!("Link saved successfully!");
            }
            Err(e) => println!("{}", e),
        }

        Ok(())
    }
}

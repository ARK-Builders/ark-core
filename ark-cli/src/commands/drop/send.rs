use std::path::PathBuf;

use drop_core::IrohInstance;

use crate::AppError;

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "send", about = "Send files and generate a shareable ticket")]
pub struct Send {
    #[clap(value_parser, help = "List of file paths to send")]
    files: Vec<PathBuf>,
}

impl Send {
    pub async fn run(&self) -> Result<(), AppError> {
        let instance = IrohInstance::new().await?;
        let ticket = instance.send_files(self.files.clone()).await?;
        println!("Share this ticket to receive the files:\n{}", ticket);
        Ok(())
    }
}

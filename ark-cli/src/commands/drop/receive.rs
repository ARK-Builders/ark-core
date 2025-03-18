use std::path::PathBuf;

use drop_core::IrohInstance;

use anyhow::Result;
use drop_core::{FileTransfer, FileTransferHandle};
use std::sync::{mpsc::channel, Arc};

use crate::AppError;

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "receive", about = "Receive files using a ticket")]
pub struct Receive {
    #[clap(value_parser, help = "Ticket string")]
    ticket: String,
}

impl Receive {
    pub async fn run(&self) -> Result<(), AppError> {
        let instance = IrohInstance::new().await?;

        let (tx, _rx) = channel::<Vec<FileTransfer>>();
        let handle = Arc::new(FileTransferHandle(tx));

        let files = instance
            .receive_files(self.ticket.clone(), handle)
            .await?;

        let outpath = if let Some(path) = dirs::download_dir() {
            path
        } else {
            PathBuf::from("/storage/emulated/0/Download/")
        };

        for (name, hash) in files.iter() {
            let content = instance
                .get_node()
                .0
                .blobs()
                .read_to_bytes(*hash)
                .await
                .expect("Failed to read blob");
            let file_path = outpath.join(name);
            std::fs::write(&file_path, content).expect("Failed to write file");
        }

        println!("Files saved to {:?}", outpath);
        Ok(())
    }
}

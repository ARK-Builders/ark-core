use std::path::PathBuf;

use anyhow::Result;
use drop_cli::run_receive_files;

use crate::AppError;

#[derive(Clone, Debug, clap::Args)]
#[clap(
    name = "receive",
    about = "Receive files using a ticket and a confirmation byte"
)]
pub struct Receive {
    #[clap(value_parser, help = "Output directory")]
    output: PathBuf,
    #[clap(value_parser, help = "Ticket string")]
    ticket: String,
    #[clap(value_parser, help = "Confirmation byte")]
    confirmation: u8,
}

impl Receive {
    pub async fn run(&self) -> Result<(), AppError> {
        let mut args: Vec<String> = Vec::with_capacity(3);
        args.push(
            self.output
                .as_os_str()
                .to_str()
                .map_or(
                    Err(AppError::DropError(String::from(
                        "Unknown file path.",
                    ))),
                    |s| Ok(s),
                )?
                .to_string(),
        );
        args.push(self.ticket.clone());
        args.push(self.confirmation.to_string());
        return run_receive_files(args)
            .await
            .map_err(|e| AppError::DropError(e.to_string()));
    }
}

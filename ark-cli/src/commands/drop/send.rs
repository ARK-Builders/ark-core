use std::path::PathBuf;

use drop_cli::run_send_files;

use crate::AppError;

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "send", about = "Send files and generate a shareable ticket")]
pub struct Send {
    #[clap(value_parser, help = "List of file paths to send")]
    files: Vec<PathBuf>,
}

impl Send {
    pub async fn run(&self) -> Result<(), AppError> {
        let mut args: Vec<String> = Vec::with_capacity(self.files.len());
        for f in &self.files {
            let path = f.as_os_str().to_str().map_or(
                Err(AppError::DropError(String::from("Unknown file path."))),
                |s| Ok(s),
            )?;
            args.push(path.to_string());
        }
        return run_send_files(args)
            .await
            .map_err(|e| AppError::DropError(e.to_string()));
    }
}

use anyhow::Result;
use arkdrop_cli::run_cli;

#[tokio::main]
async fn main() -> Result<()> {
    run_cli().await
}

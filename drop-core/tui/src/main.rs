use anyhow::Result;
use arkdrop_tui::run_tui;

#[tokio::main]
pub async fn main() -> Result<()> {
    run_tui().await
}

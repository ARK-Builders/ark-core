use anyhow::Result;
use arkdrop_cli::{build_cli, run_cli};
use arkdrop_tui::run_tui;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = build_cli();
    let matches = cli.get_matches();

    if !matches.args_present() && matches.subcommand().is_none() {
        return run_tui();
    }

    return run_cli().await;
}

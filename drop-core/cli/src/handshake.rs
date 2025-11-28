use anyhow::{Context, Result};
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Prompt user for peer's credentials (async version using tokio)
pub async fn prompt_for_credentials() -> Result<(String, u8)> {
    print!("\nEnter peer's ticket: ");
    std::io::stdout().flush()?;

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    let mut ticket = String::new();
    reader.read_line(&mut ticket).await?;
    let ticket = ticket.trim().to_string();

    if ticket.is_empty() {
        anyhow::bail!("Ticket cannot be empty");
    }

    print!("Enter peer's confirmation code: ");
    std::io::stdout().flush()?;

    let mut confirmation = String::new();
    reader.read_line(&mut confirmation).await?;
    let confirmation = confirmation
        .trim()
        .parse::<u8>()
        .context("Invalid confirmation code - must be a number 0-255")?;

    Ok((ticket, confirmation))
}

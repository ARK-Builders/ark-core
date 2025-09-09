use anyhow::Result;
use arkdrop_tui::{App, run_app};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    },
};
use std::io;

#[tokio::main]
pub async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

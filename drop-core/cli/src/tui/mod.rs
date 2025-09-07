use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
        KeyEvent, KeyEventKind,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::io;
use tokio::time::Duration;

mod app;
mod components;
mod pages;

pub use app::{App, Page};
use pages::*;

pub async fn run_tui() -> Result<()> {
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

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_key_event(app, key).await? {
                        break;
                    }
                }
            }
        }

        // Update app state if needed
        app.update().await?;
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer/Help
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("🚀 ARK Drop - File Transfer Tool")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Main content based on current page
    match app.current_page {
        Page::Main => render_main_page(f, app, chunks[1]),
        Page::Send => render_send_page(f, app, chunks[1]),
        Page::Receive => render_receive_page(f, app, chunks[1]),
        Page::Config => render_config_page(f, app, chunks[1]),
        Page::Help => render_help_page(f, app, chunks[1]),
        Page::SendProgress => render_send_progress_page(f, app, chunks[1]),
        Page::ReceiveProgress => {
            render_receive_progress_page(f, app, chunks[1])
        }
    }

    // Footer with navigation help
    let help_text = match app.current_page {
        Page::Main => "↑/↓: Navigate • Enter: Select • Q: Quit • H: Help",
        Page::Send => "Tab: Next field • Enter: Send • Esc: Back • Q: Quit",
        Page::Receive => {
            "Tab: Next field • Enter: Receive • Esc: Back • Q: Quit"
        }
        Page::Config => "↑/↓: Navigate • Enter: Select • Esc: Back • Q: Quit",
        Page::Help => "Esc: Back • Q: Quit",
        Page::SendProgress => "Q: Quit",
        Page::ReceiveProgress => "Q: Quit",
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Render modals/dialogs if any
    if app.show_error_modal {
        render_error_modal(f, app);
    }

    if app.show_success_modal {
        render_success_modal(f, app);
    }
}

async fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true), // Quit
        KeyCode::Esc => {
            if app.show_error_modal || app.show_success_modal {
                app.show_error_modal = false;
                app.show_success_modal = false;
            } else {
                app.go_back();
            }
        }
        KeyCode::Char('h') | KeyCode::Char('H') => {
            if matches!(app.current_page, Page::Main) {
                app.current_page = Page::Help;
            }
        }
        _ => match app.current_page {
            Page::Main => handle_main_page_input(app, key).await?,
            Page::Send => handle_send_page_input(app, key).await?,
            Page::Receive => handle_receive_page_input(app, key).await?,
            Page::Config => handle_config_page_input(app, key).await?,
            _ => {}
        },
    }

    Ok(false)
}

fn render_error_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let error_text = app
        .error_message
        .as_deref()
        .unwrap_or("An error occurred");
    let block = Paragraph::new(error_text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("❌ Error")
                .style(Style::default().fg(Color::Red)),
        );

    f.render_widget(block, area);
}

fn render_success_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let success_text = app
        .success_message
        .as_deref()
        .unwrap_or("Operation completed successfully");
    let block = Paragraph::new(success_text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("✅ Success")
                .style(Style::default().fg(Color::Green)),
        );

    f.render_widget(block, area);
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

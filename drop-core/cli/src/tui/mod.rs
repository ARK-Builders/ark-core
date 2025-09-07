use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
        KeyEvent, KeyEventKind, KeyModifiers,
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
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
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
        terminal.draw(|f| ui::<B>(f, app))?;

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

fn ui<B: Backend>(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5), // Title
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Footer/Help
        ])
        .split(f.area());

    // Title
    let title_text = vec![
        Line::from(vec![
            Span::styled("  🚀 ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("ARK ", Style::default().fg(Color::Cyan).bold()),
            Span::styled("Drop", Style::default().fg(Color::Blue).bold()),
            Span::styled(
                " - File Transfer Tool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Fast • Secure • Peer-to-Peer",
            Style::default().fg(Color::Gray).italic(),
        )]),
    ];

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Welcome ")
        .title_style(Style::default().fg(Color::White).bold());

    let title = Paragraph::new(title_text)
        .block(title_block)
        .alignment(Alignment::Left);
    f.render_widget(title, main_chunks[0]);

    // Main content based on current page
    match app.current_page {
        Page::Main => render_main_page(f, app, main_chunks[1]),
        Page::Send => render_send_page::<B>(f, app, main_chunks[1]),
        Page::Receive => render_receive_page::<B>(f, app, main_chunks[1]),
        Page::Config => render_config_page(f, app, main_chunks[1]),
        Page::Help => render_help_page(f, app, main_chunks[1]),
        Page::SendProgress => render_send_progress_page(f, app, main_chunks[1]),
        Page::ReceiveProgress => {
            render_receive_progress_page(f, app, main_chunks[1])
        }
    }

    // Footer with navigation help
    let (help_text, status_color) = match app.current_page {
        Page::Main => {
            ("↑/↓ Navigate • Enter Select • H Help • Q Quit", Color::Cyan)
        }
        Page::Send => (
            "Tab Next Field • Enter Send • Esc Back • Q Quit",
            Color::Green,
        ),
        Page::Receive => (
            "Tab Next Field • Enter Receive • Esc Back • Q Quit",
            Color::Blue,
        ),
        Page::Config => (
            "↑/↓ Navigate • Enter Select • Esc Back • Q Quit",
            Color::Yellow,
        ),
        Page::Help => ("Esc Back • Q Quit", Color::Magenta),
        Page::SendProgress => {
            ("Transfer in progress... • Q Quit", Color::Green)
        }
        Page::ReceiveProgress => {
            ("Transfer in progress... • Q Quit", Color::Blue)
        }
    };

    let footer_content = vec![
        Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(help_text, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(status_color))
        .title(" Controls ")
        .title_style(Style::default().fg(Color::White).bold());

    let footer = Paragraph::new(footer_content)
        .block(footer_block)
        .alignment(Alignment::Center);
    f.render_widget(footer, main_chunks[2]);

    // Render modals/dialogs if any
    if app.show_error_modal {
        render_error_modal(f, app);
    }

    if app.show_success_modal {
        render_success_modal(f, app);
    }
}

async fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q') | KeyCode::Char('Q'), KeyModifiers::CONTROL) => {
            return Ok(true);
        }
        (KeyCode::Esc, _) => {
            if app.show_error_modal || app.show_success_modal {
                app.show_error_modal = false;
                app.show_success_modal = false;
            } else {
                app.go_back();
            }
        }
        (KeyCode::Char('h') | KeyCode::Char('H'), KeyModifiers::CONTROL) => {
            app.current_page = Page::Help;
        }
        _ => {
            // Handle browser inputs first
            if app.show_file_browser {
                pages::handle_file_browser_input(app, key).await?;
            } else if app.show_directory_browser {
                pages::handle_directory_browser_input(app, key).await?;
            } else {
                match app.current_page {
                    Page::Main => match key.code {
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            app.current_page = Page::Help;
                        }
                        _ => handle_main_page_input(app, key).await?,
                    },
                    Page::Send => handle_send_page_input(app, key).await?,
                    Page::Receive => {
                        handle_receive_page_input(app, key).await?
                    }
                    Page::Config => handle_config_page_input(app, key).await?,
                    _ => {}
                }
            }
        }
    }

    Ok(false)
}

fn render_error_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let error_text = app
        .error_message
        .as_deref()
        .unwrap_or("An unexpected error occurred");

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⚠️  ", Style::default().fg(Color::Red).bold()),
            Span::styled(
                "Something went wrong:",
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(error_text, Style::default().fg(Color::LightRed)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(Color::Gray)),
            Span::styled("ESC", Style::default().fg(Color::White).bold()),
            Span::styled(" to dismiss", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    let block = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::THICK)
                .border_style(Style::default().fg(Color::Red))
                .title(" ❌ Error ")
                .title_style(Style::default().fg(Color::Red).bold()),
        )
        .alignment(Alignment::Left);

    f.render_widget(block, area);
}

fn render_success_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let success_text = app
        .success_message
        .as_deref()
        .unwrap_or("Operation completed successfully!");

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  🎉  ", Style::default().fg(Color::Green).bold()),
            Span::styled("Success!", Style::default().fg(Color::White).bold()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(success_text, Style::default().fg(Color::LightGreen)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(Color::Gray)),
            Span::styled("ESC", Style::default().fg(Color::White).bold()),
            Span::styled(" to continue", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    let block = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::THICK)
                .border_style(Style::default().fg(Color::Green))
                .title(" ✅ Success ")
                .title_style(Style::default().fg(Color::Green).bold()),
        )
        .alignment(Alignment::Left);

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

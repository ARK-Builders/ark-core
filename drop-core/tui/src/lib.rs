use anyhow::{Result, anyhow};
use arkdrop_common::{FileData, Profile, TransferFile, get_default_out_dir};
use arkdropx_receiver::{
    ReceiveFilesBubble, ReceiveFilesFile, ReceiveFilesRequest,
    ReceiveFilesSubscriber, ReceiverProfile, receive_files,
};
use arkdropx_sender::{
    SendFilesBubble, SendFilesRequest, SendFilesSubscriber, SenderConfig,
    SenderFile, SenderProfile, send_files,
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
            KeyEvent, KeyEventKind, KeyModifiers,
        },
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    },
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListState, Paragraph, Wrap},
};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::time::Instant;
use uuid::Uuid;

use crate::{
    app::{App, Page},
    components::{BrowserMode, FileBrowser},
    pages::{
        handle_config_page_input, handle_main_page_input,
        handle_receive_page_input, handle_send_page_input, render_config_page,
        render_help_page, render_main_page, render_receive_page,
        render_receive_progress_page, render_send_page,
        render_send_progress_page,
    },
};

mod app;
mod components;
mod pages;

pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();
    let res = run_tui_loop(&mut terminal, &mut app).await;

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

async fn run_tui_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &App,
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

        // Update app state
        app.update();
    }

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame, app: &App) {
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
    match app.get_current_page() {
        Page::Main => render_main_page(f, app, main_chunks[1]),
        Page::Send => render_send_page::<B>(f, app, main_chunks[1]),
        Page::Receive => render_receive_page::<B>(f, app, main_chunks[1]),
        Page::Config => render_config_page(f, app, main_chunks[1]),
        Page::Help => render_help_page(f, main_chunks[1]),
        Page::SendProgress => render_send_progress_page(f, app, main_chunks[1]),
        Page::ReceiveProgress => {
            render_receive_progress_page(f, app, main_chunks[1])
        }
    }

    // Footer with navigation help
    let (help_text, status_color) = match app.current_page.clone() {
        Page::Main => (
            "↑/↓ Navigate • Enter Select • CTRL-H Help • CTRL-Q Quit",
            Color::Cyan,
        ),
        Page::Send => (
            "Tab Next Field • Enter Send • Esc Back • CTRL-Q Quit",
            Color::Green,
        ),
        Page::Receive => (
            "Tab Next Field • Enter Receive • Esc Back • CTRL-Q Quit",
            Color::Blue,
        ),
        Page::Config => (
            "↑/↓ Navigate • Enter Select • Esc Back • CTRL-Q Quit",
            Color::Yellow,
        ),
        Page::Help => ("Esc Back • CTRL-Q Quit", Color::Magenta),
        Page::SendProgress => {
            ("Transfer in progress... • CTRL-Q Quit", Color::Green)
        }
        Page::ReceiveProgress => {
            ("Transfer in progress... • CTRL-Q Quit", Color::Blue)
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
    if app.show_error_modal.clone() {
        render_error_modal(f, app);
    }

    if app.show_success_modal.clone() {
        render_success_modal(f, app);
    }
}

async fn handle_key_event(app: &App, key: KeyEvent) -> Result<bool> {
    let show_file_browser = app.show_file_browser.clone();
    let show_dir_browser = app.show_dir_browser.clone();
    let show_success_modal = app.show_success_modal.clone();
    let show_error_modal = app.show_error_modal.clone();

    if show_file_browser {
        pages::handle_file_browser_input(app, key).await?;
    } else if show_dir_browser {
        pages::handle_dir_browser_input(app, key).await?;
    } else if show_success_modal || show_error_modal {
        match key.code {
            KeyCode::Esc => {
                app.show_error_modal = false;
                app.show_success_modal = false;
            }
            _ => {}
        }
    } else {
        match (key.code, key.modifiers) {
            (
                KeyCode::Char('q') | KeyCode::Char('Q'),
                KeyModifiers::CONTROL,
            ) => {
                return Ok(true);
            }
            (KeyCode::Esc, _) => {
                if app.previous_pages.len() > 0 {
                    app.go_back();
                }
            }
            _ => {
                let page = app.current_page.clone();
                match &page {
                    Page::Main => handle_main_page_input(app, key).await?,
                    Page::Send => handle_send_page_input(app, key).await?,
                    Page::Receive => {
                        handle_receive_page_input(app, key).await?
                    }
                    Page::Config => handle_config_page_input(app, key).await?,
                    Page::Help => {}
                    Page::SendProgress => {}
                    Page::ReceiveProgress => {}
                }
            }
        }
    }

    Ok(false)
}

fn render_error_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    f.render_widget(Clear, area);

    let error_text = app.error_message.clone().unwrap();

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
        .read()
        .unwrap()
        .clone()
        .unwrap();

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

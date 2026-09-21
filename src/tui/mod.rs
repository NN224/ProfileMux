pub mod app;
pub mod events;
pub mod keymap;
pub mod render;
pub mod sizes;

use std::io::stdout;
use std::panic;
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;

/// Restores the terminal to cooked mode and the primary screen.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
}

/// RAII guard ensuring terminal restoration even on early returns or errors.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Main entry point for the ProfileMux terminal user interface.
pub fn run() -> anyhow::Result<()> {
    // Install panic hook that cleans up alternate screen before printing panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen, crossterm::cursor::Hide)
        .context("failed to enter alternate screen")?;

    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;
    terminal.clear().context("failed to clear terminal")?;

    let mut app = App::new();

    while !app.should_quit {
        app.drain_scan_results();

        terminal
            .draw(|frame| {
                render::render(frame, &app);
            })
            .context("failed to draw terminal frame")?;

        if event::poll(Duration::from_millis(100)).context("event poll failed")? {
            match event::read().context("event read failed")? {
                Event::Key(key) => {
                    events::handle_key(&mut app, key);
                }
                Event::Resize(_, _) => {
                    // Terminal will naturally re-query dimensions on next draw
                }
                _ => {}
            }
        }
    }

    Ok(())
}

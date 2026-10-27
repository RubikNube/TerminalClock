mod app;
mod config;
mod export;
mod timing;
mod ui;

use std::{
    env,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use chrono::Local;
use crossterm::event::{self, Event};

use crate::{app::App, config::Config};

const USAGE: &str = "TerminalClock — clock, countdown timer, and stopwatch\n\nUsage: terminal-clock [--config PATH]\n\n  --config PATH  Load this TOML file instead of the default user config\n  -h, --help     Show this help\n  -V, --version  Show version\n\nDefault config: $XDG_CONFIG_HOME/terminal-clock/config.toml\n                or ~/.config/terminal-clock/config.toml\n\nIn the TUI: h/l tabs, j/k lists, ? help, q quit, Ctrl+C exit.\n";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("terminal-clock: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let mut explicit = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                print!("{USAGE}");
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!("terminal-clock {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--config") if explicit.is_none() => {
                explicit = Some(PathBuf::from(
                    args.next().ok_or("--config requires a path")?,
                ));
            }
            _ => {
                return Err(format!(
                    "Unknown or repeated argument {:?}. Use --help for usage.",
                    arg
                ));
            }
        }
    }
    let mut app = App::new(Config::load(explicit.as_deref())?)?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("An interactive terminal is required".into());
    }
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            ratatui::restore();
            return Err(error.to_string());
        }
    };
    let result = event_loop(&mut terminal, &mut app);
    let cleanup = ratatui::try_restore();
    result.and(cleanup).map_err(|e| e.to_string())
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        let now = Instant::now();
        update_timer(app, now)?;
        terminal.draw(|frame| ui::render(frame, app, now, Local::now()))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Account for time spent blocked in poll before handling an action.
                    let now = Instant::now();
                    update_timer(app, now)?;
                    app.handle_key(key, now);
                    if let Some(action) = app.export_request.take() {
                        export::handle(terminal, app, action)?;
                    }
                }
                Event::Resize(_, _) => {} // The next draw recomputes the layout.
                _ => {}
            }
        }
        if app.quit {
            return Ok(());
        }
    }
}

fn update_timer(app: &mut App, now: Instant) -> io::Result<()> {
    app.timer.tick(now);
    if app.config.timer.bell && app.timer.bell_due(now, app.config.timer.repeat_bell) {
        let mut stdout = io::stdout().lock();
        stdout.write_all(b"\x07")?;
        stdout.flush()?;
    }
    Ok(())
}

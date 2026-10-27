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

const USAGE: &str = "TerminalClock — clock, countdown timer, stopwatch, and alarm\n\nUsage: terminal-clock [--config PATH]\n\n  --config PATH  Load this TOML file instead of the default user config\n  -h, --help     Show this help\n  -V, --version  Show version\n\nDefault config: $XDG_CONFIG_HOME/terminal-clock/config.toml\n                or ~/.config/terminal-clock/config.toml\n\nIn the TUI: h/l tabs, j/k lists, ? help, q quit, Ctrl+C exit.\n";

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
        update_timers(app, now, Local::now())?;
        terminal.draw(|frame| ui::render(frame, app, now, Local::now()))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Account for time spent blocked in poll before handling an action.
                    let now = Instant::now();
                    update_timers(app, now, Local::now())?;
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

fn update_timers(app: &mut App, now: Instant, local: chrono::DateTime<Local>) -> io::Result<()> {
    update_timers_to(app, now, local, &mut io::stdout().lock())
}

fn update_timers_to(
    app: &mut App,
    now: Instant,
    local: chrono::DateTime<Local>,
    output: &mut impl Write,
) -> io::Result<()> {
    app.timer.tick(now);
    app.alarm.tick(local.with_timezone(&chrono::Utc));
    let timer_bell = app.config.timer.bell && app.timer.bell_due(now, app.config.timer.repeat_bell);
    let alarm_bell = app.config.alarm.bell && app.alarm.bell_due(now, app.config.alarm.repeat_bell);
    if timer_bell || alarm_bell {
        output.write_all(b"\x07")?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simultaneous_completions_emit_one_bell_and_dismiss_together() {
        let mut app = App::new(Config::default()).unwrap();
        let start = Instant::now();
        let wall = "2024-01-01T06:59:59Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap()
            .with_timezone(&Local);
        app.alarm.target = Some((wall + chrono::Duration::seconds(1)).with_timezone(&chrono::Utc));
        app.timer.duration = Duration::from_secs(1);
        app.timer.toggle(start);
        app.help = true;
        app.editor = Some(app::Editor {
            text: "00:05:00".into(),
            error: None,
            selected_field: 1,
        });
        let mut output = Vec::new();
        let due = start + Duration::from_secs(1);
        let local_due = wall + chrono::Duration::seconds(1);
        update_timers_to(&mut app, due, local_due, &mut output).unwrap();
        assert_eq!(output, b"\x07");
        assert!(app.alarm.notice && app.timer.notice);
        update_timers_to(&mut app, due, local_due, &mut output).unwrap();
        assert_eq!(output, b"\x07");
        app.editor = None;
        app.handle_key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
            due,
        );
        update_timers_to(
            &mut app,
            due + Duration::from_secs(10),
            local_due + chrono::Duration::seconds(10),
            &mut output,
        )
        .unwrap();
        assert_eq!(output, b"\x07");
    }

    #[test]
    fn silent_alarm_still_displays_notice() {
        let mut app = App::new(Config::parse("[alarm]\nbell = false").unwrap()).unwrap();
        let local = Local::now();
        app.alarm.target = Some(local.with_timezone(&chrono::Utc));
        let mut output = Vec::new();
        update_timers_to(&mut app, Instant::now(), local, &mut output).unwrap();
        assert!(app.alarm.notice);
        assert!(output.is_empty());
    }
}

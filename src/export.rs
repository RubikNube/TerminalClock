use crate::{
    app::App,
    config::Action,
    timing::{Lap, format_duration},
};
use std::{
    env,
    io::{self, Write},
    process::{Command, Stdio},
};

fn format_laps(laps: &[Lap]) -> String {
    let mut text = String::from("| Lap | Total | Split |\n| ---: | ---: | ---: |\n");
    for (i, lap) in laps.iter().enumerate() {
        text.push_str(&format!(
            "| {} | {} | {} |\n",
            i + 1,
            format_duration(lap.total, true, false),
            format_duration(lap.split, true, false)
        ));
    }
    text
}

pub fn handle(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    action: Action,
) -> io::Result<()> {
    if app.stopwatch.laps.is_empty() {
        app.feedback = Some("No laps to export".into());
        return Ok(());
    }
    let text = format_laps(&app.stopwatch.laps);
    let result = if action == Action::CopyLaps {
        copy(&text)
    } else {
        edit(terminal, &text)?
    };
    app.feedback = Some(match result {
        Ok(()) => if action == Action::CopyLaps {
            "Lap times copied"
        } else {
            "Returned from editor"
        }
        .into(),
        Err(e) => e,
    });
    Ok(())
}

fn copy(text: &str) -> Result<(), String> {
    let mut candidates = Vec::new();
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        candidates.push(("wl-copy", vec![]));
    }
    if env::var_os("DISPLAY").is_some() {
        candidates.extend([
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ]);
    }
    for (program, args) in candidates {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let written = child.stdin.take().unwrap().write_all(text.as_bytes());
        if child.wait().is_ok_and(|s| s.success()) && written.is_ok() {
            return Ok(());
        }
    }
    Err("Clipboard unavailable: install wl-clipboard (Wayland), xclip or xsel (X11) and check your display session".into())
}

fn edit(terminal: &mut ratatui::DefaultTerminal, text: &str) -> io::Result<Result<(), String>> {
    let mut file = match tempfile::Builder::new().suffix(".md").tempfile() {
        Ok(f) => f,
        Err(e) => return Ok(Err(e.to_string())),
    };
    if let Err(e) = file.write_all(text.as_bytes()) {
        return Ok(Err(e.to_string()));
    }
    let configured = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|key| env::var(key).ok())
        .find(|s| !s.trim().is_empty());
    let commands = if let Some(value) = configured {
        match shlex::split(&value) {
            Some(parts) if !parts.is_empty() => vec![parts],
            _ => return Ok(Err("Invalid editor command quoting".into())),
        }
    } else {
        ["nvim", "vim", "vi"]
            .into_iter()
            .map(|s| vec![s.to_owned()])
            .collect()
    };
    ratatui::try_restore()?;
    let mut outcome =
        Err("No editor available: set VISUAL or EDITOR, or install nvim/vim/vi".into());
    for parts in commands {
        match Command::new(&parts[0])
            .args(&parts[1..])
            .arg(file.path())
            .status()
        {
            Ok(status) => {
                outcome = if status.success() {
                    Ok(())
                } else {
                    Err(format!("Editor exited with {status}"))
                };
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                outcome = Err(format!("Cannot open editor: {e}"));
                break;
            }
        }
    }
    *terminal = ratatui::try_init()?;
    terminal.clear()?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn exports_all_laps_in_recording_order() {
        let laps = [
            Lap {
                total: Duration::from_millis(1230),
                split: Duration::from_millis(1230),
            },
            Lap {
                total: Duration::from_millis(2450),
                split: Duration::from_millis(1220),
            },
        ];
        assert_eq!(
            format_laps(&laps),
            "| Lap | Total | Split |\n| ---: | ---: | ---: |\n| 1 | 00:00:01.23 | 00:00:01.23 |\n| 2 | 00:00:02.45 | 00:00:01.22 |\n"
        );
    }
}

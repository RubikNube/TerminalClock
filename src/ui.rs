use std::time::Instant;

use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};

use crate::{
    app::App,
    config::{Action, color},
    timing::format_duration,
};

const DIGITS: [[&str; 5]; 11] = [
    ["███", "█ █", "█ █", "█ █", "███"],
    [" █ ", "██ ", " █ ", " █ ", "███"],
    ["███", "  █", "███", "█  ", "███"],
    ["███", "  █", "███", "  █", "███"],
    ["█ █", "█ █", "███", "  █", "  █"],
    ["███", "█  ", "███", "  █", "███"],
    ["███", "█  ", "███", "█ █", "███"],
    ["███", "  █", "  █", "  █", "  █"],
    ["███", "█ █", "███", "█ █", "███"],
    ["███", "█ █", "███", "  █", "███"],
    ["   ", " █ ", "   ", " █ ", "   "],
];

fn accent(app: &App) -> Style {
    Style::default().fg(color(&app.config.theme.accent).unwrap())
}

fn error(app: &App) -> Style {
    Style::default().fg(color(&app.config.theme.error).unwrap())
}

pub fn render(frame: &mut Frame, app: &App, now: Instant, local: DateTime<Local>) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(
            Style::default()
                .fg(color(&app.config.theme.foreground).unwrap())
                .bg(color(&app.config.theme.background).unwrap()),
        ),
        area,
    );
    if area.width < 40 || area.height < 16 {
        let notice = if app.timer.notice {
            "\nTIMER FINISHED"
        } else {
            ""
        };
        let text = format!(
            "TerminalClock\nResize to at least 40 × 16{notice}\n{} quit · Ctrl+C exit",
            app.bindings.label(Action::Quit)
        );
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
        return;
    }
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(if app.timer.notice { 3 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);
    let titles = [
        format!("{} Clock", app.bindings.label(Action::Clock)),
        format!("{} Timer", app.bindings.label(Action::Timer)),
        format!("{} Stopwatch", app.bindings.label(Action::Stopwatch)),
    ];
    frame.render_widget(
        Tabs::new(titles)
            .select(app.tab)
            .highlight_style(accent(app).add_modifier(Modifier::BOLD))
            .block(Block::bordered().title(" TerminalClock ")),
        chunks[0],
    );
    if app.timer.notice {
        frame.render_widget(
            Paragraph::new(format!(
                "Timer finished · {} dismiss",
                app.bindings.label(Action::Acknowledge)
            ))
            .alignment(Alignment::Center)
            .style(accent(app).add_modifier(Modifier::BOLD))
            .block(Block::bordered()),
            chunks[1],
        );
    }
    if app.help {
        render_help(frame, app, chunks[2]);
    } else {
        match app.tab {
            0 => render_clock(frame, app, local, chunks[2]),
            1 => render_timer(frame, app, now, chunks[2]),
            _ => render_stopwatch(frame, app, now, chunks[2]),
        }
    }
    let hints = if app.editor.is_some() {
        "h/l field · j/k -/+ · Enter save · Esc cancel · Ctrl+C exit".into()
    } else if app.help {
        format!(
            "Esc / {} close · {}/{} scroll · {} quit",
            app.bindings.label(Action::Help),
            app.bindings.label(Action::ListDown),
            app.bindings.label(Action::ListUp),
            app.bindings.label(Action::Quit)
        )
    } else {
        let mut text = format!(
            "{} / {} tabs",
            app.bindings.label(Action::PreviousTab),
            app.bindings.label(Action::NextTab)
        );
        if app.tab != 0 {
            text.push_str(&format!(
                " · {} start/pause · {} reset",
                app.bindings.label(Action::Toggle),
                app.bindings.label(Action::Reset)
            ));
            let (action, label) = if app.tab == 1 {
                (Action::EditTimer, "edit")
            } else {
                (Action::Lap, "lap")
            };
            text.push_str(&format!(" · {} {label}", app.bindings.label(action)));
        }
        text.push_str(&format!(
            " · {} help · {} quit",
            app.bindings.label(Action::Help),
            app.bindings.label(Action::Quit)
        ));
        text
    };
    frame.render_widget(
        Paragraph::new(hints)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::TOP)),
        chunks[3],
    );
    if app.editor.is_some() {
        render_editor(frame, app, chunks[2]);
    }
}

fn render_clock(frame: &mut Frame, app: &App, local: DateTime<Local>, area: Rect) {
    let twelve = app.config.clock.format == "12h";
    let time = local
        .format(if twelve { "%I:%M:%S" } else { "%H:%M:%S" })
        .to_string();
    let mut lines = Vec::new();
    if twelve {
        lines.push(local.format("%p").to_string());
    }
    if app.config.clock.show_date {
        lines.push(local.format("%A, %d %B %Y").to_string());
    }
    if app.config.clock.show_timezone {
        lines.push(format!("Local time · UTC{}", local.format("%:z")));
    }
    render_digits(
        frame,
        app,
        &time,
        lines,
        app.config.clock.large_digits,
        area,
    );
}

fn render_timer(frame: &mut Frame, app: &App, now: Instant, area: Rect) {
    let status = if app.timer.finished {
        "FINISHED"
    } else if app.timer.running() {
        "RUNNING"
    } else if app.timer.editable(now) {
        "READY"
    } else {
        "PAUSED"
    };
    let mut lines = vec![
        status.into(),
        format!(
            "Duration {}",
            format_duration(app.timer.duration, false, false)
        ),
    ];
    if let Some(feedback) = &app.feedback {
        lines.push(feedback.clone());
    }
    render_digits(
        frame,
        app,
        &format_duration(app.timer.remaining(now), false, true),
        lines,
        true,
        area,
    );
}

fn render_digits(
    frame: &mut Frame,
    app: &App,
    time: &str,
    details: Vec<String>,
    large: bool,
    area: Rect,
) {
    let width = time.chars().count() * 4 - 1;
    let available = area.height.saturating_sub(details.len() as u16 + 1);
    let scale = if large && area.width as usize >= width * 2 && available >= 10 {
        2
    } else if large && area.width as usize >= width && available >= 5 {
        1
    } else {
        0
    };
    let mut lines: Vec<Line> = Vec::new();
    if scale == 0 {
        lines.push(Line::styled(
            time.to_owned(),
            accent(app).add_modifier(Modifier::BOLD),
        ));
    } else {
        for (row, _) in DIGITS[0].iter().enumerate() {
            let glyph = time
                .chars()
                .map(|c| {
                    let index = c.to_digit(10).map_or(10, |n| n as usize);
                    DIGITS[index][row]
                        .chars()
                        .flat_map(|c| std::iter::repeat_n(c, scale))
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join(&" ".repeat(scale));
            for _ in 0..scale {
                lines.push(Line::styled(glyph.clone(), accent(app)));
            }
        }
    }
    lines.push(Line::default());
    lines.extend(details.into_iter().map(Line::from));
    let height = (lines.len() as u16).min(area.height);
    let centered = Rect {
        y: area.y + area.height.saturating_sub(height) / 2,
        height,
        ..area
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines)).alignment(Alignment::Center),
        centered,
    );
}

fn render_stopwatch(frame: &mut Frame, app: &App, now: Instant, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    let status = if app.stopwatch.elapsed.running() {
        "RUNNING"
    } else {
        "STOPPED"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format_duration(app.stopwatch.elapsed.value(now), true, false),
                accent(app).add_modifier(Modifier::BOLD),
            ),
            Line::from(status),
        ])
        .alignment(Alignment::Center),
        chunks[0],
    );
    if app.stopwatch.laps.is_empty() {
        frame.render_widget(
            Paragraph::new(format!(
                "No laps yet · {} record a lap while running",
                app.bindings.label(Action::Lap)
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(" Laps ")),
            chunks[1],
        );
        return;
    }
    let items: Vec<_> = app
        .stopwatch
        .laps
        .iter()
        .enumerate()
        .rev()
        .map(|(index, lap)| {
            ListItem::new(format!(
                "{:>3}  {}  +{}",
                index + 1,
                format_duration(lap.total, true, false),
                format_duration(lap.split, true, false)
            ))
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(app.stopwatch.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Laps · total / split "))
            .highlight_style(accent(app).add_modifier(Modifier::BOLD))
            .highlight_symbol("› "),
        chunks[1],
        &mut state,
    );
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let pairs: &[(Action, &str)] = &[
        (Action::PreviousTab, "Previous tab"),
        (Action::NextTab, "Next tab"),
        (Action::Clock, "Clock"),
        (Action::Timer, "Timer"),
        (Action::Stopwatch, "Stopwatch"),
        (Action::Toggle, "Start / pause"),
        (Action::Reset, "Reset"),
        (Action::EditTimer, "Edit duration"),
        (Action::Lap, "Record lap"),
        (Action::ListDown, "Older lap"),
        (Action::ListUp, "Newer lap"),
        (Action::ListFirst, "Newest lap"),
        (Action::ListLast, "Oldest lap"),
        (Action::Acknowledge, "Dismiss alert"),
        (Action::Help, "Help"),
        (Action::Quit, "Quit"),
    ];
    let visible: Vec<_> = pairs
        .iter()
        .filter(|(a, _)| a.contexts() & (1 << app.tab) != 0)
        .collect();
    let compact = area.height < visible.len() as u16 + 2;
    let lines: Vec<_> = if compact {
        visible
            .iter()
            .map(|(a, label)| Line::from(format!("{} {label}", app.bindings.label(*a))))
            .collect()
    } else {
        visible
            .iter()
            .map(|(a, label)| {
                Line::from(vec![
                    Span::styled(format!("{:20}", app.bindings.label(*a)), accent(app)),
                    Span::raw(*label),
                ])
            })
            .collect()
    };
    let mut state = ListState::default().with_selected(Some(app.help_selected));
    frame.render_stateful_widget(
        List::new(lines.into_iter().map(ListItem::new))
            .block(Block::bordered().title(" Help · current tab "))
            .highlight_style(accent(app).add_modifier(Modifier::BOLD)),
        area,
        &mut state,
    );
}

fn render_editor(frame: &mut Frame, app: &App, area: Rect) {
    let editor = app.editor.as_ref().unwrap();
    let width = area.width.min(64);
    let height = area.height.min(7);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let base = Style::default()
        .fg(color(&app.config.theme.foreground).unwrap())
        .bg(color(&app.config.theme.background).unwrap());
    let mut duration: Vec<Span> = editor
        .text
        .chars()
        .enumerate()
        .map(|(index, c)| {
            let style = if index / 3 == editor.selected_field && index % 3 < 2 {
                accent(app).add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                accent(app)
            };
            Span::styled(c.to_string(), style)
        })
        .collect();
    duration.push(Span::styled("▏", accent(app)));
    let lines = vec![
        Line::from("h/l field · j/k decrease/increase"),
        Line::from(duration),
        Line::from("Digits/: type · Backspace delete"),
        Line::styled(editor.error.clone().unwrap_or_default(), error(app)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(base)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(" Edit countdown ")),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::{Terminal, backend::TestBackend};

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, app, Instant::now(), Local::now()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn layouts_alerts_and_custom_hints() {
        let mut app =
            App::new(Config::parse("[keys]\nquit = ['x']\nhelp = ['F']").unwrap()).unwrap();
        assert!(screen(&app, 80, 24).contains("x quit"));
        assert!(screen(&app, 80, 24).contains("F help"));
        app.timer.notice = true;
        for tab in 0..3 {
            app.tab = tab;
            assert!(screen(&app, 80, 24).contains("Timer finished"));
            screen(&app, 40, 16);
        }
        assert!(screen(&app, 20, 8).contains("TIMER FINISHED"));
        for (w, h) in [(0, 0), (1, 1), (39, 15), (40, 16), (120, 40)] {
            screen(&app, w, h);
        }
        app.help = true;
        assert!(screen(&app, 80, 30).contains("Quit"));
        app.handle_key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('G'),
                crossterm::event::KeyModifiers::NONE,
            ),
            Instant::now(),
        );
        assert!(screen(&app, 40, 16).contains("Quit"));
    }

    #[test]
    fn editor_validation_error_is_visible() {
        let mut app = App::new(Config::default()).unwrap();
        app.tab = 1;
        app.editor = Some(crate::app::Editor {
            text: "00:00:00".into(),
            error: Some("Invalid duration".into()),
            selected_field: 1,
        });
        assert!(screen(&app, 80, 24).contains("Invalid duration"));
    }

    #[test]
    fn editor_highlights_only_selected_field_and_shows_vim_hints() {
        let mut app = App::new(Config::default()).unwrap();
        app.tab = 1;
        for field in 0..3 {
            app.editor = Some(crate::app::Editor {
                text: "00:05:00".into(),
                error: None,
                selected_field: field,
            });
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| render(frame, &app, Instant::now(), Local::now()))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let (x, y) = (0..24)
                .find_map(|y| {
                    (0..73).find_map(|x| {
                        let value: String = (x..x + 8)
                            .map(|column| buffer[(column, y)].symbol())
                            .collect();
                        (value == "00:05:00").then_some((x, y))
                    })
                })
                .expect("editor duration is visible");
            for offset in 0..8 {
                let selected = offset / 3 == field as u16 && offset % 3 < 2;
                assert_eq!(
                    buffer[(x + offset, y)]
                        .modifier
                        .contains(Modifier::REVERSED),
                    selected
                );
            }
        }
        assert!(screen(&app, 40, 16).contains("h/l field"));
        assert!(screen(&app, 40, 16).contains("j/k decrease/increase"));
    }

    #[test]
    fn clock_preferences_control_format_and_details() {
        let app = App::new(Config::parse("[clock]\nformat = '12h'\nlarge_digits = false\nshow_date = false\nshow_timezone = false").unwrap()).unwrap();
        let local = DateTime::from_timestamp(1700000000, 0)
            .unwrap()
            .with_timezone(&Local);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| render(frame, &app, Instant::now(), local))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains(&local.format("%I:%M:%S").to_string()));
        assert!(text.contains(&local.format("%p").to_string()));
        assert!(!text.contains("UTC"));
        assert!(!text.contains(&local.format("%A").to_string()));
    }
}

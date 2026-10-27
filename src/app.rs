use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    config::{Action, Bindings, Config, Key},
    timing::{Stopwatch, Timer, format_duration, parse_duration, parse_duration_fields},
};

pub struct Editor {
    pub text: String,
    pub error: Option<String>,
    /// Hours, minutes, seconds; minutes are selected when the editor opens.
    pub selected_field: usize,
}

impl Editor {
    fn adjust(&mut self, increase: bool) {
        match parse_duration_fields(&self.text) {
            Ok(mut fields) => {
                let maximum = if self.selected_field == 0 { 99 } else { 59 };
                let value = &mut fields[self.selected_field];
                *value = if increase {
                    (*value + 1) % (maximum + 1)
                } else {
                    (*value + maximum) % (maximum + 1)
                };
                self.text = format!("{:02}:{:02}:{:02}", fields[0], fields[1], fields[2]);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

pub struct App {
    pub config: Config,
    pub bindings: Bindings,
    pub tab: usize,
    pub timer: Timer,
    pub stopwatch: Stopwatch,
    pub editor: Option<Editor>,
    pub help: bool,
    pub help_selected: usize,
    pub quit: bool,
    pub export_request: Option<Action>,
    pub feedback: Option<String>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        let bindings = config.bindings()?;
        let duration = parse_duration(&config.timer.default_duration)?;
        Ok(Self {
            config,
            bindings,
            tab: 0,
            timer: Timer::new(duration),
            stopwatch: Stopwatch::default(),
            editor: None,
            help: false,
            help_selected: 0,
            quit: false,
            export_request: None,
            feedback: None,
        })
    }

    pub fn handle_key(&mut self, event: KeyEvent, now: Instant) {
        let adjustment_repeat = self.editor.is_some()
            && event.kind == KeyEventKind::Repeat
            && matches!(event.code, KeyCode::Char('j' | 'k'));
        if event.kind != KeyEventKind::Press && !adjustment_repeat {
            return;
        }
        let Some(key) = Key::from_event(event) else {
            return;
        };
        if key.is_interrupt() {
            self.quit = true;
            return;
        }
        if let Some(editor) = &mut self.editor {
            if event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                return;
            }
            match event.code {
                KeyCode::Char('h') => {
                    editor.selected_field = editor.selected_field.saturating_sub(1)
                }
                KeyCode::Char('l') => editor.selected_field = (editor.selected_field + 1).min(2),
                KeyCode::Char('j') => editor.adjust(false),
                KeyCode::Char('k') => editor.adjust(true),
                KeyCode::Esc => self.editor = None,
                KeyCode::Enter => match parse_duration(&editor.text) {
                    Ok(duration) => {
                        self.timer.duration = duration;
                        self.timer.reset();
                        self.editor = None;
                    }
                    Err(error) => editor.error = Some(error),
                },
                KeyCode::Backspace => {
                    editor.text.pop();
                    editor.error = None;
                }
                KeyCode::Char(c) if (c.is_ascii_digit() || c == ':') && editor.text.len() < 8 => {
                    editor.text.push(c);
                    editor.error = None;
                }
                _ => {}
            }
            return;
        }
        if self.help {
            let last = Action::ALL
                .iter()
                .filter(|a| a.contexts() & (1 << self.tab) != 0)
                .count()
                .saturating_sub(1);
            if event.code == KeyCode::Esc || self.bindings.matches(Action::Help, key) {
                self.help = false;
            } else if self.bindings.matches(Action::Quit, key) {
                self.quit = true;
            } else if self.bindings.matches(Action::Acknowledge, key) {
                self.timer.notice = false;
            } else if self.bindings.matches(Action::ListDown, key) {
                self.help_selected = self.help_selected.saturating_add(1).min(last);
            } else if self.bindings.matches(Action::ListUp, key) {
                self.help_selected = self.help_selected.saturating_sub(1);
            } else if self.bindings.matches(Action::ListFirst, key) {
                self.help_selected = 0;
            } else if self.bindings.matches(Action::ListLast, key) {
                self.help_selected = last;
            }
            return;
        }
        self.feedback = None;
        let Some(action) = self.bindings.action(key, self.tab) else {
            return;
        };
        match action {
            Action::PreviousTab => self.tab = (self.tab + 2) % 3,
            Action::NextTab => self.tab = (self.tab + 1) % 3,
            Action::Clock => self.tab = 0,
            Action::Timer => self.tab = 1,
            Action::Stopwatch => self.tab = 2,
            Action::Toggle if self.tab == 1 => self.timer.toggle(now),
            Action::Toggle => self.stopwatch.elapsed.toggle(now),
            Action::Reset if self.tab == 1 => self.timer.reset(),
            Action::Reset => self.stopwatch.reset(),
            Action::EditTimer => {
                if self.timer.editable(now) {
                    self.editor = Some(Editor {
                        text: format_duration(self.timer.duration, false, false),
                        error: None,
                        selected_field: 1,
                    });
                } else {
                    self.feedback = Some("Reset the timer before editing its duration".into());
                }
            }
            Action::EditLaps | Action::CopyLaps => self.export_request = Some(action),
            Action::Lap => self.stopwatch.lap(now),
            Action::ListDown => {
                self.stopwatch.selected =
                    (self.stopwatch.selected + 1).min(self.stopwatch.laps.len().saturating_sub(1))
            }
            Action::ListUp => self.stopwatch.selected = self.stopwatch.selected.saturating_sub(1),
            Action::ListFirst => self.stopwatch.selected = 0,
            Action::ListLast => {
                self.stopwatch.selected = self.stopwatch.laps.len().saturating_sub(1)
            }
            Action::Acknowledge => self.timer.notice = false,
            Action::Help => {
                self.help = true;
                self.help_selected = 0;
            }
            Action::Quit => self.quit = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn press(app: &mut App, code: KeyCode, now: Instant) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), now);
    }

    fn open_editor() -> (App, Instant) {
        let mut app = App::new(Config::default()).unwrap();
        let now = Instant::now();
        press(&mut app, KeyCode::Char('2'), now);
        press(&mut app, KeyCode::Char('e'), now);
        (app, now)
    }

    #[test]
    fn editor_field_navigation_saves_and_cancels_without_switching_tabs() {
        let (mut app, now) = open_editor();
        assert_eq!(app.editor.as_ref().unwrap().selected_field, 1);
        for c in "hhk".chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        assert_eq!(app.editor.as_ref().unwrap().selected_field, 0);
        assert_eq!(app.editor.as_ref().unwrap().text, "01:05:00");
        for c in "ljllk".chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        assert_eq!(app.tab, 1);
        assert_eq!(app.editor.as_ref().unwrap().selected_field, 2);
        assert_eq!(app.editor.as_ref().unwrap().text, "01:04:01");
        assert_eq!(app.timer.duration, Duration::from_secs(300));
        press(&mut app, KeyCode::Enter, now);
        assert_eq!(app.timer.duration, Duration::from_secs(3841));
        assert!(app.editor.is_none());
        press(&mut app, KeyCode::Char('e'), now);
        assert_eq!(app.editor.as_ref().unwrap().selected_field, 1);
        press(&mut app, KeyCode::Char('k'), now);
        press(&mut app, KeyCode::Esc, now);
        assert!(app.editor.is_none());
        assert_eq!(app.timer.duration, Duration::from_secs(3841));
    }

    #[test]
    fn editor_adjustments_wrap_without_changing_other_fields() {
        for (field, initial, incremented) in [
            (0, "12:34:56", "13:34:56"),
            (1, "12:34:56", "12:35:56"),
            (2, "12:34:56", "12:34:57"),
        ] {
            let mut editor = Editor {
                text: initial.into(),
                error: None,
                selected_field: field,
            };
            editor.adjust(true);
            assert_eq!(editor.text, incremented);
            editor.adjust(false);
            assert_eq!(editor.text, initial);
        }
        for (field, wrapped_up, wrapped_down) in [
            (0, "00:59:59", "99:00:00"),
            (1, "99:00:59", "00:59:00"),
            (2, "99:59:00", "00:00:59"),
        ] {
            for (value, increase, expected) in [
                ("99:59:59", true, wrapped_up),
                ("00:00:00", false, wrapped_down),
            ] {
                let mut editor = Editor {
                    text: value.into(),
                    error: None,
                    selected_field: field,
                };
                editor.adjust(increase);
                assert_eq!(editor.text, expected);
                assert!(editor.error.is_none());
                editor.adjust(!increase);
                assert_eq!(editor.text, value);
            }
        }
    }

    #[test]
    fn editor_adjustment_repeats_but_ignores_releases_and_control_chords() {
        let (mut app, now) = open_editor();
        for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
            app.handle_key(
                KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, kind),
                now,
            );
        }
        assert_eq!(app.editor.as_ref().unwrap().text, "00:07:00");
        app.handle_key(
            KeyEvent::new_with_kind(
                KeyCode::Char('k'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ),
            now,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            now,
        );
        assert_eq!(app.editor.as_ref().unwrap().text, "00:07:00");
        app.handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Repeat),
            now,
        );
        assert_eq!(app.editor.as_ref().unwrap().text, "00:06:00");
    }

    #[test]
    fn editor_adjustments_validate_text_and_recover_from_zero() {
        let (mut app, now) = open_editor();
        for malformed in ["", "00:05", "00:60:00", "00:00:60", "aa:bb:cc"] {
            app.editor.as_mut().unwrap().text = malformed.into();
            for c in ['j', 'k'] {
                press(&mut app, KeyCode::Char(c), now);
                assert_eq!(app.editor.as_ref().unwrap().text, malformed);
                assert!(app.editor.as_ref().unwrap().error.is_some());
            }
            press(&mut app, KeyCode::Char('h'), now);
            assert_eq!(app.editor.as_ref().unwrap().selected_field, 0);
            press(&mut app, KeyCode::Char('l'), now);
            assert_eq!(app.editor.as_ref().unwrap().selected_field, 1);
        }
        // Direct typing continues to work after a failed adjustment.
        for _ in 0..8 {
            press(&mut app, KeyCode::Backspace, now);
        }
        for c in "00:01:00".chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        press(&mut app, KeyCode::Char('j'), now);
        assert_eq!(app.editor.as_ref().unwrap().text, "00:00:00");
        press(&mut app, KeyCode::Enter, now);
        assert!(app.editor.as_ref().unwrap().error.is_some());
        press(&mut app, KeyCode::Char('k'), now);
        assert!(app.editor.as_ref().unwrap().error.is_none());
        press(&mut app, KeyCode::Enter, now);
        assert!(app.editor.is_none());
        assert_eq!(app.timer.duration, Duration::from_secs(60));
    }

    #[test]
    fn remapped_navigation_and_editor_isolation() {
        let mut app =
            App::new(Config::parse("[keys]\nnext_tab = ['n']\nquit = ['x']").unwrap()).unwrap();
        let now = Instant::now();
        press(&mut app, KeyCode::Char('l'), now);
        assert_eq!(app.tab, 0);
        press(&mut app, KeyCode::Char('n'), now);
        press(&mut app, KeyCode::Char('e'), now);
        press(&mut app, KeyCode::Char('x'), now);
        press(&mut app, KeyCode::Char('3'), now);
        assert_eq!(app.tab, 1);
        assert!(!app.quit);
        for _ in 0..8 {
            press(&mut app, KeyCode::Backspace, now);
        }
        for c in "00:00:00".chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        press(&mut app, KeyCode::Enter, now);
        assert!(app.editor.as_ref().unwrap().error.is_some());
        press(&mut app, KeyCode::Backspace, now);
        press(&mut app, KeyCode::Char('2'), now);
        press(&mut app, KeyCode::Enter, now);
        assert!(app.editor.is_none());
        assert_eq!(app.timer.duration, Duration::from_secs(2));
        press(&mut app, KeyCode::Char('x'), now);
        assert!(app.quit);
    }

    #[test]
    fn timing_continues_across_tabs_and_banner_acknowledges() {
        let mut app = App::new(Config::default()).unwrap();
        let now = Instant::now();
        app.timer.duration = Duration::from_secs(1);
        press(&mut app, KeyCode::Char('2'), now);
        press(&mut app, KeyCode::Char(' '), now);
        press(&mut app, KeyCode::Char('h'), now);
        assert!(app.timer.tick(now + Duration::from_secs(2)));
        assert!(app.timer.notice);
        press(&mut app, KeyCode::Enter, now + Duration::from_secs(2));
        assert!(!app.timer.notice);
        press(&mut app, KeyCode::Char('3'), now);
        press(&mut app, KeyCode::Char(' '), now);
        for offset in 1..=3 {
            press(
                &mut app,
                KeyCode::Char('a'),
                now + Duration::from_secs(offset),
            );
        }
        press(&mut app, KeyCode::Char('G'), now);
        assert_eq!(app.stopwatch.selected, 2);
        press(&mut app, KeyCode::Char('k'), now);
        assert_eq!(app.stopwatch.selected, 1);
        press(&mut app, KeyCode::Char('g'), now);
        assert_eq!(app.stopwatch.selected, 0);
    }

    #[test]
    fn interrupt_works_in_editor_and_help() {
        for help in [false, true] {
            let mut app = App::new(Config::default()).unwrap();
            app.help = help;
            if !help {
                app.editor = Some(Editor {
                    text: String::new(),
                    error: None,
                    selected_field: 1,
                });
            }
            app.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Instant::now(),
            );
            assert!(app.quit);
        }
    }

    #[test]
    fn help_navigation_clamps_and_returns_from_last_item() {
        let mut app = App::new(Config::default()).unwrap();
        let now = Instant::now();
        press(&mut app, KeyCode::Char('3'), now);
        press(&mut app, KeyCode::Char('?'), now);
        app.timer.notice = true;
        press(&mut app, KeyCode::Enter, now);
        assert!(!app.timer.notice);
        assert!(app.help);
        press(&mut app, KeyCode::Char('G'), now);
        let last = app.help_selected;
        press(&mut app, KeyCode::Char('j'), now);
        assert_eq!(app.help_selected, last);
        press(&mut app, KeyCode::Char('k'), now);
        assert_eq!(app.help_selected, last - 1);
        press(&mut app, KeyCode::Esc, now);
        assert!(!app.help);
    }
}

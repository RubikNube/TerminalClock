use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::Deserialize;

use crate::timing::parse_duration;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    PreviousTab,
    NextTab,
    Clock,
    Timer,
    Stopwatch,
    ListDown,
    ListUp,
    ListFirst,
    ListLast,
    Toggle,
    Reset,
    EditTimer,
    Lap,
    Acknowledge,
    Help,
    Quit,
}

impl Action {
    pub const ALL: [Self; 16] = [
        Self::PreviousTab,
        Self::NextTab,
        Self::Clock,
        Self::Timer,
        Self::Stopwatch,
        Self::ListDown,
        Self::ListUp,
        Self::ListFirst,
        Self::ListLast,
        Self::Toggle,
        Self::Reset,
        Self::EditTimer,
        Self::Lap,
        Self::Acknowledge,
        Self::Help,
        Self::Quit,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::PreviousTab => "previous_tab",
            Self::NextTab => "next_tab",
            Self::Clock => "clock",
            Self::Timer => "timer",
            Self::Stopwatch => "stopwatch",
            Self::ListDown => "list_down",
            Self::ListUp => "list_up",
            Self::ListFirst => "list_first",
            Self::ListLast => "list_last",
            Self::Toggle => "toggle",
            Self::Reset => "reset",
            Self::EditTimer => "edit_timer",
            Self::Lap => "lap",
            Self::Acknowledge => "acknowledge",
            Self::Help => "help",
            Self::Quit => "quit",
        }
    }

    pub fn contexts(self) -> u8 {
        match self {
            Self::EditTimer => 2,
            Self::Lap | Self::ListDown | Self::ListUp | Self::ListFirst | Self::ListLast => 4,
            Self::Toggle | Self::Reset => 6,
            _ => 7,
        }
    }

    fn defaults(self) -> &'static [&'static str] {
        match self {
            Self::PreviousTab => &["h", "BackTab"],
            Self::NextTab => &["l", "Tab"],
            Self::Clock => &["1"],
            Self::Timer => &["2"],
            Self::Stopwatch => &["3"],
            Self::ListDown => &["j", "Down"],
            Self::ListUp => &["k", "Up"],
            Self::ListFirst => &["g"],
            Self::ListLast => &["G"],
            Self::Toggle => &["Space"],
            Self::Reset => &["r"],
            Self::EditTimer => &["e"],
            Self::Lap => &["a"],
            Self::Acknowledge => &["Enter"],
            Self::Help => &["?"],
            Self::Quit => &["q"],
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub clock: ClockConfig,
    pub timer: TimerConfig,
    pub theme: Theme,
    pub keys: BTreeMap<Action, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockConfig {
    pub format: String,
    pub show_date: bool,
    pub show_timezone: bool,
    pub large_digits: bool,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            format: "24h".into(),
            show_date: true,
            show_timezone: true,
            large_digits: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimerConfig {
    pub default_duration: String,
    pub bell: bool,
    pub repeat_bell: bool,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            default_duration: "00:05:00".into(),
            bell: true,
            repeat_bell: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub foreground: String,
    pub background: String,
    pub accent: String,
    pub error: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: "white".into(),
            background: "default".into(),
            accent: "cyan".into(),
            error: "red".into(),
        }
    }
}

pub fn color(value: &str) -> Result<Color, String> {
    Ok(match value {
        "default" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" => Color::Gray,
        "dark_gray" => Color::DarkGray,
        "light_red" => Color::LightRed,
        "light_green" => Color::LightGreen,
        "light_yellow" => Color::LightYellow,
        "light_blue" => Color::LightBlue,
        "light_magenta" => Color::LightMagenta,
        "light_cyan" => Color::LightCyan,
        "white" => Color::White,
        hex if hex.len() == 7
            && hex.starts_with('#')
            && hex[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            let rgb = u32::from_str_radix(&hex[1..], 16).map_err(|e| e.to_string())?;
            Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
        }
        _ => {
            return Err(format!(
                "Invalid color {value:?}; use an ANSI name, default, or #RRGGBB"
            ));
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Key {
    code: KeyCode,
    control: bool,
}

impl Key {
    pub fn parse(value: &str) -> Result<Self, String> {
        let code = match value {
            "Enter" => KeyCode::Enter,
            "Tab" => KeyCode::Tab,
            "BackTab" => KeyCode::BackTab,
            "Esc" => KeyCode::Esc,
            "Space" => KeyCode::Char(' '),
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "Backspace" => KeyCode::Backspace,
            s if s.starts_with("Ctrl+") => {
                let letter = &s[5..];
                if letter.len() == 1 && letter.as_bytes()[0].is_ascii_alphabetic() {
                    return Ok(Self {
                        code: KeyCode::Char(letter.to_ascii_lowercase().chars().next().unwrap()),
                        control: true,
                    }
                    .normalized());
                }
                return Err(format!(
                    "Invalid key {value:?}; Ctrl+ requires one ASCII letter"
                ));
            }
            s if s.chars().count() == 1 && !s.chars().next().unwrap().is_control() => {
                KeyCode::Char(s.chars().next().unwrap())
            }
            _ => return Err(format!("Invalid key {value:?}")),
        };
        Ok(Self {
            code,
            control: false,
        })
    }

    pub fn from_event(event: KeyEvent) -> Option<Self> {
        if event.modifiers.intersects(
            KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
        ) {
            return None;
        }
        let control = event.modifiers.contains(KeyModifiers::CONTROL);
        let code = match event.code {
            KeyCode::Char(c) if control => KeyCode::Char(c.to_ascii_lowercase()),
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::SHIFT) => {
                KeyCode::Char(c.to_ascii_uppercase())
            }
            KeyCode::Tab if event.modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
            code => code,
        };
        Some(Self { code, control }.normalized())
    }

    fn normalized(self) -> Self {
        // Legacy terminal protocols encode Ctrl+I as Tab and Ctrl+M as Enter.
        // Treat these aliases alike even if an enhanced protocol reports modifiers.
        match (self.code, self.control) {
            (KeyCode::Char('i'), true) => Self {
                code: KeyCode::Tab,
                control: false,
            },
            (KeyCode::Char('m'), true) => Self {
                code: KeyCode::Enter,
                control: false,
            },
            _ => self,
        }
    }

    pub fn is_interrupt(self) -> bool {
        self == Self {
            code: KeyCode::Char('c'),
            control: true,
        }
    }
}

pub struct Bindings {
    entries: Vec<(Action, Vec<Key>, Vec<String>)>,
}

impl Bindings {
    fn build(overrides: &BTreeMap<Action, Vec<String>>) -> Result<Self, String> {
        let mut entries: Vec<(Action, Vec<Key>, Vec<String>)> = Vec::new();
        for action in Action::ALL {
            let names = overrides
                .get(&action)
                .cloned()
                .unwrap_or_else(|| action.defaults().iter().map(|s| (*s).into()).collect());
            if names.is_empty() {
                return Err(format!(
                    "keys.{} requires at least one binding",
                    action.name()
                ));
            }
            let mut keys = Vec::new();
            for name in &names {
                let key = Key::parse(name)?;
                if key.is_interrupt() || keys.contains(&key) {
                    return Err(format!(
                        "Reserved or duplicate key {name:?} for {}",
                        action.name()
                    ));
                }
                for (other, previous, _) in &entries {
                    if action.contexts() & other.contexts() != 0 && previous.contains(&key) {
                        return Err(format!(
                            "Key {name:?} conflicts between {} and {}",
                            other.name(),
                            action.name()
                        ));
                    }
                }
                keys.push(key);
            }
            entries.push((action, keys, names));
        }
        Ok(Self { entries })
    }

    pub fn action(&self, key: Key, tab: usize) -> Option<Action> {
        self.entries
            .iter()
            .find(|(action, keys, _)| action.contexts() & (1 << tab) != 0 && keys.contains(&key))
            .map(|(action, _, _)| *action)
    }

    pub fn matches(&self, action: Action, key: Key) -> bool {
        self.entries
            .iter()
            .any(|(a, keys, _)| *a == action && keys.contains(&key))
    }

    pub fn label(&self, action: Action) -> String {
        self.entries
            .iter()
            .find(|(a, _, _)| *a == action)
            .unwrap()
            .2
            .join("/")
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if !matches!(self.clock.format.as_str(), "12h" | "24h") {
            return Err("clock.format must be 12h or 24h".into());
        }
        parse_duration(&self.timer.default_duration)?;
        for value in [
            &self.theme.foreground,
            &self.theme.background,
            &self.theme.accent,
            &self.theme.error,
        ] {
            color(value)?;
        }
        self.bindings()?;
        Ok(())
    }

    pub fn bindings(&self) -> Result<Bindings, String> {
        Bindings::build(&self.keys)
    }

    pub fn load(explicit: Option<&Path>) -> Result<Self, String> {
        let default = default_path(
            env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            env::var_os("HOME").map(PathBuf::from),
        );
        Self::load_path(explicit, default.as_deref())
    }

    fn load_path(explicit: Option<&Path>, default: Option<&Path>) -> Result<Self, String> {
        let Some(path) = explicit.or(default) else {
            return Self::parse("");
        };
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if explicit.is_none() && e.kind() == io::ErrorKind::NotFound => Self::parse(""),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

fn default_path(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    // XDG paths must be absolute; ignore empty/relative values per the specification.
    xdg.filter(|p| p.is_absolute())
        .or_else(|| home.map(|p| p.join(".config")))
        .map(|p| p.join("terminal-clock/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_partial_overrides_and_example() {
        let config = Config::parse("[clock]\nformat = '12h'\n[keys]\nquit = ['x']").unwrap();
        assert!(config.clock.show_date);
        assert_eq!(config.timer.default_duration, "00:05:00");
        assert!(config.timer.repeat_bell);
        assert!(
            !Config::parse("[timer]\nrepeat_bell = false")
                .unwrap()
                .timer
                .repeat_bell
        );
        assert_eq!(config.bindings().unwrap().label(Action::Quit), "x");
        Config::parse(include_str!("../config.example.toml")).unwrap();
    }

    #[test]
    fn rejects_bad_settings() {
        for text in [
            "unknown = 1",
            "[clock]\nunknown = true",
            "[clock]\nformat = 'bad'",
            "[timer]\ndefault_duration = '00:00:00'",
            "[timer]\nrepeat_bell = 1",
            "[theme]\naccent = 'orange'",
            "[keys]\nquit = []",
            "[keys]\nquit = ['h']",
            "[keys]\nquit = ['x', 'x']",
            "[keys]\nquit = ['Ctrl+c']",
            "[keys]\nunknown = ['x']",
            "[keys]\nquit = ['Ctrl+xx']",
            "[keys",
        ] {
            assert!(Config::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn bindings_are_context_aware_and_normalized() {
        let c = Config::parse("[keys]\nedit_timer = ['a']").unwrap();
        let b = c.bindings().unwrap();
        let key = Key::parse("a").unwrap();
        assert_eq!(b.action(key, 1), Some(Action::EditTimer));
        assert_eq!(b.action(key, 2), Some(Action::Lap));
        assert_eq!(
            Key::from_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::SHIFT)),
            Some(Key::parse("G").unwrap())
        );
        assert_eq!(
            Key::from_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(Key::parse("BackTab").unwrap())
        );
        assert_eq!(Key::parse("Ctrl+X"), Key::parse("Ctrl+x"));
        assert_eq!(Key::parse("Ctrl+i"), Key::parse("Tab"));
        assert_eq!(Key::parse("Ctrl+m"), Key::parse("Enter"));
        assert!(Config::parse("[keys]\nquit = ['Ctrl+i']").is_err());
        assert!(Config::parse("[keys]\nquit = ['Ctrl+m']").is_err());
        assert!(Key::from_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)).is_none());
        assert_eq!(color("#12aBff").unwrap(), Color::Rgb(18, 171, 255));
        assert!(color("#zzzzzz").is_err());
    }

    #[test]
    fn config_paths_and_missing_file_policy() {
        assert_eq!(
            default_path(Some("/config".into()), Some("/home/user".into())),
            Some("/config/terminal-clock/config.toml".into())
        );
        assert_eq!(
            default_path(Some("relative".into()), Some("/home/user".into())),
            Some("/home/user/.config/terminal-clock/config.toml".into())
        );
        let missing = Path::new("/tmp/terminal-clock-does-not-exist/config.toml");
        assert!(Config::load_path(None, Some(missing)).is_ok());
        assert!(Config::load_path(Some(missing), None).is_err());
        assert!(Config::load_path(Some(Path::new(".")), None).is_err());
        assert!(Config::load_path(Some(Path::new("config.example.toml")), Some(missing)).is_ok());
    }
}

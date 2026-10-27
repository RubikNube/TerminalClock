# TerminalClock

A Rust terminal clock, countdown timer, and stopwatch with TOML configuration
and customizable Vim-style keyboard navigation. Linux is the primary supported
platform.

## Run

### Download a release (Linux x86_64)

Download the `.tar.gz` archive and `SHA256SUMS` from the same
[GitHub release](https://github.com/RubikNube/TerminalClock/releases). The binary
requires Linux x86_64 with glibc 2.35 or newer (Ubuntu 22.04 or newer, for example).
From the download directory, verify and extract the archive, replacing `v0.1.0`
with the version you downloaded:

```sh
sha256sum --check SHA256SUMS
tar -xzf terminal-clock-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
mkdir -p ~/.local/bin
install -m 755 terminal-clock-v0.1.0-x86_64-unknown-linux-gnu/terminal-clock ~/.local/bin/terminal-clock
```

Ensure `~/.local/bin` is on your `PATH`, then run `terminal-clock`. The archive
also includes this README, the license, and `config.example.toml`.

### Build from source

Install a current stable Rust toolchain, then run from this repository:

```sh
cargo run --release
```

To install the executable:

```sh
cargo install --path . --locked
terminal-clock
```

Use an interactive terminal with Unicode support. A window of at least 40 columns
by 16 rows supports the compact interface; 80 × 24 or larger gives the clock room
for large digital digits. Smaller windows show a resize prompt while timing
continues. `terminal-clock --help` lists command-line options.

## Features

- **Clock:** large digital local time with date and UTC offset. Supports 12-hour
  and 24-hour formats, and a compact text display.
- **Timer:** one countdown with start, pause, resume, reset, and an editable
  duration. Completion shows a persistent notice on every tab and optionally
  rings the terminal bell every second until dismissed. Whether the bell sounds
  depends on your terminal. Set `timer.repeat_bell = false` for a single chime.
- **Stopwatch:** elapsed time to hundredths, with cumulative and split lap times.
  Laps appear newest first and can be scrolled.

Both timing tools continue across tab changes, help, and terminal resizing. They
measure elapsed time independently of rendering and wall-clock adjustments.
Exiting discards sessions; settings load at startup and are not rewritten.

## Controls

| Action | Default keys |
| --- | --- |
| Previous / next tab | `h` / `l`, Shift+Tab / Tab |
| Clock / Timer / Stopwatch | `1` / `2` / `3` |
| Start, pause, resume | Space (Timer or Stopwatch) |
| Stop and reset | `r` (Timer or Stopwatch) |
| Edit countdown duration | `e` (Timer, when ready or finished) |
| Record lap | `a` (while Stopwatch runs) |
| Older / newer lap | `j` / `k`, Down / Up |
| Newest / oldest lap | `g` / `G` |
| Dismiss completion notice | Enter |
| Open / close help | `?`; Escape also closes help |
| Quit | `q`; Ctrl+C always exits |

This is Vim-style navigation without Normal/Insert modes or a colon command
prompt. The footer and help display your configured bindings. Help shows actions
for the current tab; use the list navigation keys to browse long help screens.

In the duration editor, `h`/`l` select hours, minutes, or seconds (minutes are
selected initially). `k` increases the selected field and `j` decreases it; hold
either key to repeat. Each field wraps at its limits: increasing 59 minutes or
seconds returns to 0, and increasing 99 hours returns to 0. Decreasing 0 wraps to
the field's maximum, without carrying or borrowing between fields. The selected
field is highlighted.

Direct text entry is also available: Backspace removes the last character;
digits and `:` append characters. Enter saves and Escape cancels. Use a complete
`HH:MM:SS` value to adjust fields. Incomplete or malformed text stays unchanged
with an inline error. Adjustments can pass through `00:00:00`, but saving requires
`00:00:01` through `99:59:59`. Other shortcuts are suspended; Ctrl+C still exits.
Editor controls are fixed and independent of configured navigation bindings.
Reset a paused timer before changing its duration. Resetting the stopwatch also
clears its laps. Starting a finished timer starts its full duration again.

## Configuration

The app looks for `$XDG_CONFIG_HOME/terminal-clock/config.toml` when
`XDG_CONFIG_HOME` is an absolute path, otherwise
`~/.config/terminal-clock/config.toml`. A missing default file uses built-in
defaults. Pass an explicit file to use it instead of the default:

```sh
cargo run --release -- --config config.example.toml
terminal-clock --config /path/to/config.toml
```

Copy [config.example.toml](config.example.toml) to your preferred location for a
fully commented reference. Every field is optional. For example:

```toml
[clock]
format = "12h"
show_date = true
show_timezone = false
large_digits = true

[timer]
default_duration = "00:25:00"
bell = false
repeat_bell = true # When bell is enabled, repeat until dismissed.

[theme]
accent = "#80cbc4"
background = "default"

[keys]
quit = ["x", "Ctrl+q"]
```

Theme fields are `foreground`, `background`, `accent`, and `error`. Colors accept
`default`, `#RRGGBB`, or these ANSI names: `black`, `red`, `green`, `yellow`,
`blue`, `magenta`, `cyan`, `gray`, `dark_gray`, `light_red`, `light_green`,
`light_yellow`, `light_blue`, `light_magenta`, `light_cyan`, `white`.

Bindings accept case-sensitive single characters, `Ctrl+<ASCII letter>`, or
`Enter`, `Tab`, `BackTab`, `Esc`, `Space`, `Up`, `Down`, `Left`, `Right`, `Home`,
`End`, `Backspace`. Use `BackTab` for Shift+Tab. Each configured action replaces
its entire key array; omitted actions keep defaults. Every action requires at
least one binding. Ctrl+C is reserved; editor controls remain fixed.
Ctrl+I and Ctrl+M are treated as aliases of Tab and Enter, including when checking
for binding conflicts, because traditional terminals cannot distinguish them.

The example lists all action names. Duplicate keys are rejected within an action
and between actions active on the same tab. Timer-only and stopwatch-only actions
can share keys. Remapping a key to another action may therefore require remapping
its original action too. Terminal or shell settings may intercept some Ctrl keys.

Malformed TOML, unknown fields/actions, invalid values, unreadable files, binding
conflicts, and missing explicit config files produce errors before the TUI starts.
There is no configuration hot reload or session persistence.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

GitHub Actions runs these checks on Ubuntu with stable Rust for pushes and pull
requests. The Verification workflow also supports manual runs.

Tests cover configuration, key dispatch, timing transitions using controlled
timestamps, laps, duration validation, and rendering at different terminal sizes.

## Releasing

Update the package version in `Cargo.toml`, run `cargo check` to update
`Cargo.lock`, and commit both files with the release changes. Push the commit,
then create and push a matching stable version tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The Release workflow accepts only `vX.Y.Z` tags matching the package version.
It runs formatting, Clippy, and tests, builds on Ubuntu 22.04 with stable Rust,
and verifies the archive and packaged executable before automatically publishing
a GitHub release with generated notes, a Linux x86_64 archive, and `SHA256SUMS`.
No additional repository secrets are required. An existing release causes
publication to fail; reruns do not overwrite published assets.

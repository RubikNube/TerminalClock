use std::process::Command;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_terminal-clock"))
}

#[test]
fn help_and_version_work_without_a_terminal() {
    let output = command().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--config PATH"));
    let output = command().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn config_and_argument_errors_do_not_enter_raw_mode() {
    for args in [
        vec!["--config"],
        vec!["--unknown"],
        vec!["--config", "tests/missing/config.toml"],
        vec![
            "--config",
            "config.example.toml",
            "--config",
            "config.example.toml",
        ],
    ] {
        let output = command().args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("terminal-clock:"));
        assert!(!output.stdout.contains(&0x1b));
        assert!(!output.stderr.contains(&0x1b));
    }
}

#[test]
fn redirected_io_is_rejected_cleanly_after_valid_config() {
    let output = command()
        .args(["--config", "config.example.toml"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal is required"));
    assert!(!output.stdout.contains(&0x1b));
}

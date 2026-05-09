//! cosmic-color-picker: the GUI app.
//!
//! D0 architecture: the overlay code lives in the `cosmic-color-pickerd`
//! daemon binary now. The GUI talks to the daemon when one is running
//! (via the IPC socket); when no daemon is reachable it falls back to
//! spawning `cosmic-color-pickerd` as a one-shot subprocess. D1 extends
//! the daemon to be long-running with proper IPC; D2 wires the GUI's
//! Pick button through that IPC instead of subprocess spawn.

mod app;
mod autostart;
mod color;
mod config;
mod i18n;
mod ipc;

use std::env;
use std::process::{Command, ExitCode};

#[derive(Debug, Default)]
struct CliFlags {
    pick: bool,
    background: bool,
}

fn parse_args() -> Result<CliFlags, ExitCode> {
    let mut flags = CliFlags::default();
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--pick" => flags.pick = true,
            "--background" => flags.background = true,
            "-h" | "--help" => {
                print_help();
                return Err(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("cosmic-color-picker {}", env!("CARGO_PKG_VERSION"));
                return Err(ExitCode::SUCCESS);
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(flags)
}

fn print_help() {
    println!("Usage: cosmic-color-picker [--pick | --background]");
    println!();
    println!("  (no flags)    Open the application window.");
    println!("  --pick        Trigger the picker overlay and copy the result.");
    println!("  --background  Start the GUI hidden (used by autostart, deprecated in D2+).");
}

fn main() -> ExitCode {
    let flags = match parse_args() {
        Ok(f) => f,
        Err(code) => return code,
    };

    if flags.pick {
        return run_pick();
    }

    run_app(flags.background)
}

/// `--pick` path. Try the running daemon's IPC socket first; fall back to
/// spawning `cosmic-color-pickerd` directly if no daemon is reachable.
/// Either way the daemon owns clipboard + notification delivery.
fn run_pick() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => return spawn_daemon_oneshot(),
    };
    if runtime.block_on(ipc::try_send_pick()) {
        return ExitCode::SUCCESS;
    }
    spawn_daemon_oneshot()
}

fn spawn_daemon_oneshot() -> ExitCode {
    match Command::new("cosmic-color-pickerd").status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("cosmic-color-picker: failed to launch cosmic-color-pickerd: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_app(background: bool) -> ExitCode {
    if !background {
        if let Ok(rt) = tokio::runtime::Runtime::new()
            && rt.block_on(ipc::try_send_show())
        {
            return ExitCode::SUCCESS;
        }
    }

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default()
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(420.0)
                .min_height(360.0),
        )
        .size(cosmic::iced::Size::new(560.0, 680.0));

    let flags = app::Flags { background };
    match cosmic::app::run::<app::AppModel>(settings, flags) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cosmic-color-picker: application failed: {e}");
            ExitCode::from(1)
        }
    }
}

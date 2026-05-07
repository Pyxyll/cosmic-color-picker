//! Color picker for COSMIC.
//!
//! - `cosmic-color-picker` (no args): launches the libcosmic application
//!   window. Once D-Bus single-instance is wired up (M4), a second invocation
//!   focuses the running window instead of starting a new one.
//! - `cosmic-color-picker --pick`: triggers the picker overlay. Today this
//!   captures and runs the overlay in-process; M4 will redirect it through
//!   D-Bus to the running daemon so the result lands in the app's history.
//! - `cosmic-color-picker --background`: launches the daemon without showing
//!   the window (for autostart). M5 wires this into the autostart toggle.

mod app;
mod config;
mod i18n;
mod overlay;

use std::env;
use std::process::ExitCode;

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
    println!("  --background  Start the daemon without showing the window.");
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

/// `--pick` path: capture + overlay, copy hex to clipboard, fire notification.
/// Once M4 lands, this will instead D-Bus into the running app and the daemon
/// owns the post-pick UI.
fn run_pick() -> ExitCode {
    match overlay::pick_color() {
        Ok(Some(hex)) => {
            deliver(&hex);
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("color picker: overlay failed: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_app(background: bool) -> ExitCode {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default()
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(420.0)
                .min_height(360.0),
        )
        .size(cosmic::iced::Size::new(520.0, 540.0));

    let flags = app::Flags { background };
    match cosmic::app::run::<app::AppModel>(settings, flags) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("color picker: application failed: {e}");
            ExitCode::from(1)
        }
    }
}

fn deliver(hex: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    println!("{hex}");

    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn()
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(hex.as_bytes());
        drop(stdin);
        let _ = child.wait();
    }

    let _ = Command::new("notify-send")
        .args([
            "--app-name",
            "Color Picker",
            "--icon",
            "color-select-symbolic",
            "--expire-time",
            "3000",
            hex,
            "Copied to clipboard",
        ])
        .status();
}

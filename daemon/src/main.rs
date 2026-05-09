//! cosmic-color-pickerd: headless color-picker daemon.
//!
//! D0 only ships the one-shot picker subset (matches v0.1 behaviour): run
//! it, the overlay opens, the picked hex is printed on stdout, copied to
//! the clipboard and shown in a notification, then the process exits.
//!
//! D1 adds the long-running daemon mode (IPC socket + history persistence).
//!
//! `--quiet` skips the clipboard + notification side-effects and only
//! prints the hex on stdout — useful when the GUI invokes us as a
//! subprocess and wants to handle delivery itself.

mod capture;
mod font;
mod overlay;

use std::env;
use std::io::Write;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let quiet = env::args().any(|a| a == "--quiet" || a == "-q");

    match overlay::pick_color() {
        Ok(Some(hex)) => {
            println!("{hex}");
            if !quiet {
                deliver(&hex);
            }
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cosmic-color-pickerd: {e}");
            ExitCode::from(1)
        }
    }
}

fn deliver(hex: &str) {
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

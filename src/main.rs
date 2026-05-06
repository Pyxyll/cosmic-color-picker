//! Native Wayland color picker for COSMIC.
//!
//! Approach: grab a screenshot via `grim`, open a fullscreen layer-shell
//! overlay showing that capture, follow the cursor with a magnifier lens,
//! and on click copy the hex of the picked pixel to the clipboard.
//!
//! This is the MVP — milestone 1 just opens an overlay and proves the
//! plumbing works. Capture, magnifier, and pick logic land in subsequent
//! milestones.

mod capture;
mod font;
mod overlay;

use std::process::ExitCode;

fn main() -> ExitCode {
    let img = match capture::screenshot() {
        Ok(img) => img,
        Err(e) => {
            eprintln!("color picker: screen capture failed: {e}");
            return ExitCode::from(1);
        }
    };

    match overlay::run(img) {
        Ok(Some(hex)) => {
            deliver(&hex);
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS, // user cancelled
        Err(e) => {
            eprintln!("color picker: overlay failed: {e}");
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

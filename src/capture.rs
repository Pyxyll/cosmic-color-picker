//! Screen capture via the `grim` CLI tool.
//!
//! Phase 1 wraps grim because it already works on COSMIC and frees us from
//! implementing `ext-image-copy-capture-v1` directly. Phase 2 (later) can
//! replace this with a native protocol implementation for live frame capture.

use std::io;
use std::process::Command;

pub fn screenshot() -> io::Result<image::RgbaImage> {
    let out = Command::new("grim").args(["-t", "png", "-"]).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "grim exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let img = image::load_from_memory(&out.stdout)
        .map_err(|e| io::Error::other(format!("decode: {e}")))?;
    Ok(img.to_rgba8())
}

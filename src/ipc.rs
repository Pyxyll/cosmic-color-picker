//! Lightweight IPC: a Unix socket the running app listens on so that
//! `cosmic-color-picker --pick` (fired from the user's hotkey) lands inside
//! the existing process instead of spawning a fresh one. The picked color
//! flows into the running app's history.
//!
//! Falls back transparently to the in-process overlay when no app is
//! running, so the hotkey works either way.

use std::path::PathBuf;

/// Where the running app's listening socket lives. Uses XDG_RUNTIME_DIR so
/// it lives under /run/user/<uid> on most setups, which is wiped on logout
/// and avoids stale sockets piling up in /tmp.
pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    runtime.join("cosmic-color-picker.sock")
}

/// Attempt to connect to the running app's socket and request a pick.
/// Returns `true` on successful delivery (the daemon will run the overlay
/// and store the result), `false` if no daemon is reachable.
pub async fn try_send_pick() -> bool {
    use tokio::io::AsyncWriteExt;
    let Ok(mut stream) = tokio::net::UnixStream::connect(socket_path()).await else {
        return false;
    };
    stream.write_all(b"p").await.is_ok()
}

/// Best-effort cleanup of any socket file left behind by a prior crash.
/// Called by the app before binding so its `bind` doesn't fail with EADDRINUSE.
pub fn clean_stale_socket() {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&path);
}

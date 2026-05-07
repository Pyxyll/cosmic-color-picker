//! Persistent configuration stored via cosmic-config.
//!
//! M0: skeleton only. Fields land progressively across milestones — history
//! list (M3), autostart toggle (M5), preferred hotkey (M6), default format.

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Reserved. Will be populated in later milestones.
    placeholder: String,
}

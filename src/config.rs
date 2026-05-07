//! Persistent configuration stored via cosmic-config.

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Recent picks as hex strings (`#RRGGBB`), newest first. Capped at the
    /// limit defined in `app.rs`. Stored as strings rather than packed ints
    /// so the on-disk config file stays human-readable and editable.
    pub history: Vec<String>,
}

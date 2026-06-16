// Package israce reports if the Go race detector is enabled.

// Enabled reports if the race detector is enabled.
#[cfg(not(feature = "race"))]
pub const ENABLED: bool = false;

// Enabled reports if the race detector is enabled.
#[cfg(feature = "race")]
pub const ENABLED: bool = true;

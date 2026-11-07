//! Approval binding key provisioning for the Tauri bridge.
//!
//! The implementation (and its tests) lives in `g9_app::keyfile` so that the
//! bridge and the command-line driver (`apps/cli`) provision the key from the
//! same `<app data>/binding.key` file with the same rules (`0600`, 32 bytes,
//! atomic creation, never logged). This module only re-exports it.

pub use g9_app::keyfile::*;

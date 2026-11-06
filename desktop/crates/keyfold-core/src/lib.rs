//! Keyfold core. Contract: secret values live only in the OS credential store and
//! are never part of any return value meant for display, logs or agents.

pub mod l10n;
mod keyfold;
pub mod redactor;
pub mod registry;
mod share;
pub mod store;

pub use keyfold::{project_for_dir, Config, Keyfold, KeyfoldError, SlotInfo, GENERATE_DEFAULT, GENERATE_MAX, GENERATE_MIN, PROJECT_FILE};

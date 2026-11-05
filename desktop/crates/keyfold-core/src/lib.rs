//! Keyfold core. Contract: secret values live only in the OS credential store and
//! are never part of any return value meant for display, logs or agents.

pub mod l10n;
mod keyfold;
pub mod redactor;
pub mod registry;
pub mod store;

pub use keyfold::{Config, Keyfold, KeyfoldError, SlotInfo};

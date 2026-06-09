//! Hardware compatibility checks and installer configuration for Sirius.

pub mod check;
pub mod config;
pub mod facts;
pub mod probes;
pub mod report;

pub use check::{Check, Status};
pub use config::SiriusConfig;
pub use facts::SystemFacts;
pub use report::{is_blocked, run_all_checks};

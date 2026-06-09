//! Hardware compatibility checks and installer configuration for Sirius.

pub mod check;
pub mod facts;
pub mod probes;
pub mod report;

pub use check::{Check, Status};
pub use facts::SystemFacts;
pub use report::{is_blocked, run_all_checks};

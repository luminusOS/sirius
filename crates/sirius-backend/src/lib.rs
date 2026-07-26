//! Operating-system integrations and the privileged installation boundary.
//!
//! GTK and Relm4 must not be used in this crate.

pub mod distro;
pub mod install;
pub mod network;
pub mod runner;
pub mod spawn;
pub mod storage;
pub mod system;

pub use sirius_core::{InstallRequest, Progress};

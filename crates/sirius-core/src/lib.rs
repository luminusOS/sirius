//! Pure domain models and process protocol shared across Sirius.
//!
//! This crate performs no hardware discovery, disk mutation, process spawning,
//! or graphical work. Those responsibilities belong to `sirius-backend` and
//! `sirius-app`.

pub mod distro;
pub mod install;
pub mod protocol;

pub use distro::{Bento, BootcConfig, Branding, DiskConfig, DistroDescriptor};
pub use install::{
    InstallConfig, InstallType, MountAssignment, PartitionOperation, PartitionPlan, PartitionRef,
    UserAccount, validate_encryption_passphrase,
};
pub use protocol::{InstallRequest, Progress};

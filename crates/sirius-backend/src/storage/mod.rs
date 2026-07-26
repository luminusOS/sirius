//! Storage facade.
//!
//! Discovery is read-only and safe for the app process. Applying a plan is
//! destructive and is called only from the privileged runner.

mod apply;
mod scan;

use serde::{Deserialize, Serialize};

pub use apply::apply_partition_plan;
pub use scan::scan_disks;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskSnapshot {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub table_type: String,
    pub read_only: bool,
    pub in_use: bool,
    pub partitions: Vec<PartitionSnapshot>,
    pub free_regions: Vec<FreeRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartitionSnapshot {
    pub path: String,
    pub start_bytes: u64,
    pub size_bytes: u64,
    pub filesystem: String,
    pub label: String,
    pub mountpoints: Vec<String>,
    pub gpt_type: String,
    pub part_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FreeRegion {
    pub offset_bytes: u64,
    pub size_bytes: u64,
}

pub fn format_size(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.0} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

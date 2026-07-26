//! Stable messages exchanged across the unprivileged/privileged boundary.

use crate::PartitionPlan;
use serde::{Deserialize, Serialize};

/// User choices sent to the privileged installer.
///
/// Distribution-owned image and partition-layout settings are deliberately
/// absent: the privileged side reads those from its root-owned descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallRequest {
    pub target_disk: String,
    pub encrypt: bool,
    pub tpm: bool,
    pub encryption_key: String,
    pub locale: String,
    pub keyboard: String,
    pub timezone: String,
    pub hostname: String,
    pub username: String,
    pub full_name: String,
    pub partition_plan: Option<PartitionPlan>,
}

/// Progress reported by the privileged runner to the app as JSON lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Progress {
    Step { fraction: f64, message: String },
    Log { line: String },
    Finished,
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_json_remains_wire_compatible() {
        assert_eq!(
            serde_json::to_string(&Progress::Step {
                fraction: 0.5,
                message: "partitioning".into(),
            })
            .unwrap(),
            r#"{"Step":{"fraction":0.5,"message":"partitioning"}}"#
        );
        assert_eq!(
            serde_json::to_string(&Progress::Log {
                line: "formatting".into(),
            })
            .unwrap(),
            r#"{"Log":{"line":"formatting"}}"#
        );
        assert_eq!(
            serde_json::to_string(&Progress::Finished).unwrap(),
            r#""Finished""#
        );
        assert_eq!(
            serde_json::to_string(&Progress::Error {
                message: "failed".into(),
            })
            .unwrap(),
            r#"{"Error":{"message":"failed"}}"#
        );
    }

    #[test]
    fn request_json_keeps_existing_field_names() {
        let request = InstallRequest {
            target_disk: "/dev/vdb".into(),
            encrypt: false,
            tpm: false,
            encryption_key: String::new(),
            locale: "en_US".into(),
            keyboard: "us".into(),
            timezone: "UTC".into(),
            hostname: "localhost".into(),
            username: "demo".into(),
            full_name: "Demo".into(),
            partition_plan: None,
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["target_disk"], "/dev/vdb");
        assert_eq!(value["partition_plan"], serde_json::Value::Null);
        assert!(value.get("image").is_none());
        assert!(value.get("repart_dir").is_none());
    }
}

use serde::Serialize;

/// Outcome of a single hardware compatibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

/// A single compatibility check result shown on the diagnostics page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub id: String,
    pub label: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    pub fn new(id: &str, label: &str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            status,
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_new_builds_expected_struct() {
        let c = Check::new("uefi", "UEFI firmware", Status::Pass, "found");
        assert_eq!(c.id, "uefi");
        assert_eq!(c.status, Status::Pass);
        assert_eq!(c.detail, "found");
    }
}

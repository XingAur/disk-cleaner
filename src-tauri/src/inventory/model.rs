use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Protection {
    Safe,
    Review,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub source: String,
    pub category: String,
    pub evidence: String,
    pub impact: String,
    pub protection: Protection,
    pub protection_reason: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub is_dir: bool,
    pub modified_ms: u64,
    pub permanent_allowed: bool,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub path: PathBuf,
    pub source: String,
    pub category: String,
    pub evidence: String,
    pub impact: String,
    pub protection: Protection,
    pub children: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub path: PathBuf,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    #[serde(default)]
    pub registrations: Vec<Registration>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub current: String,
    pub completed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub scan_id: String,
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
    pub drives: Vec<super::platform::Drive>,
    pub recycle_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteMode {
    Recycle,
    Permanent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRequest {
    pub scan_id: String,
    pub ids: Vec<String>,
    pub reviewed_ids: Vec<String>,
    pub mode: DeleteMode,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub recycled_bytes: u64,
    pub deleted_bytes: u64,
    pub completed_ids: Vec<String>,
    pub errors: Vec<String>,
}

//! BootKeeper core: shared domain model.
//!
//! Everything in this crate (CLI, helper, GUI, MCP) works on these types.
//! Keep this file platform-independent.

use serde::{Deserialize, Serialize};

/// Startup mechanism categories. v1 = three of them; more later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// HKCU/HKLM ...\Run and RunOnce
    RegistryRun,
    /// User / common startup folders
    StartupFolder,
    /// Task Scheduler
    ScheduledTask,
    /// Windows services set to auto-start
    Service,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::RegistryRun,
        Category::StartupFolder,
        Category::ScheduledTask,
        Category::Service,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Category::RegistryRun => "registry_run",
            Category::StartupFolder => "startup_folder",
            Category::ScheduledTask => "scheduled_task",
            Category::Service => "service",
        }
    }
}

/// Risk level decided by software rules (never by the AI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
}

/// Signature status, verified via WinVerifyTrust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signature {
    /// File has no signature
    None,
    /// Signature present and chain verifies
    Valid,
    /// Signature present but verification failed
    Invalid,
    /// Not applicable (e.g. task with no executable)
    Unknown,
}

/// One startup item as shown to GUI/AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    /// Stable ID: category + location + name, so AI can reference it.
    pub id: String,
    pub category: Category,
    /// Display name (registry value name, task name, file name)
    pub name: String,
    /// Full command line or executable path
    pub command: String,
    /// Where it lives (registry key, folder path, task path)
    pub location: String,
    /// Signature info (None = not yet verified)
    pub signature: Signature,
    /// Publisher name when signed
    pub publisher: Option<String>,
    /// Risk decided by rules engine
    pub risk: Risk,
    /// Whether the item is currently active (enabled)
    pub enabled: bool,
}

impl StartupItem {
    pub fn build_id(category: Category, location: &str, name: &str) -> String {
        format!("{}:{}:{}", category.as_str(), location, name)
    }
}

/// A captured snapshot of one item, before/after a write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub item_id: String,
    /// Original value name before disable (rename) operations
    pub original_name: String,
    pub current_name: String,
    pub command: String,
    pub location: String,
    pub category: Category,
    /// True when this snapshot represents a removed item
    pub was_removed: bool,
}

/// One snapshot: the state delta of one write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    /// ISO-8601 timestamp of the operation
    pub created_at: String,
    pub operation: String,
    pub entries: Vec<SnapshotEntry>,
}

/// Everything an enumerator returns before rule-engine enrichment.
#[derive(Debug, Clone)]
pub struct RawEntry {
    pub category: Category,
    pub name: String,
    pub command: String,
    pub location: String,
}

impl RawEntry {
    pub fn new(category: Category, location: &str, name: &str, command: &str) -> Self {
        Self {
            category,
            name: name.to_string(),
            command: command.to_string(),
            location: location.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_id_is_stable_and_unique() {
        let a = StartupItem::build_id(Category::RegistryRun, "HKCU\\Run", "Foo");
        let b = StartupItem::build_id(Category::RegistryRun, "HKCU\\Run", "Foo");
        let c = StartupItem::build_id(Category::RegistryRun, "HKLM\\Run", "Foo");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn risk_orders_low_med_high() {
        assert!(Risk::Low < Risk::Medium);
        assert!(Risk::Medium < Risk::High);
    }
}

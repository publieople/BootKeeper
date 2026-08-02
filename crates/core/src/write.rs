//! Write-operation protocol types — platform-independent.
//!
//! These are the JSON contract between CLI and helper. The actual mutation
//! logic is Windows-only (see windows/ops.rs).

use crate::model::{Category, SnapshotEntry};
use serde::{Deserialize, Serialize};

/// A mutation request from CLI/AI. The helper re-verifies everything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteOp {
    /// Rename value/file so it no longer autostarts (Foo -> Foo.disabled).
    Disable { item_id: String },
    /// Rename back (Foo.disabled -> Foo).
    Enable { item_id: String },
    /// Remove entry entirely (after snapshot backup).
    Remove { item_id: String },
    /// Add a new entry.
    Add {
        category: Category,
        name: String,
        command: String,
        #[serde(default)]
        location: String,
    },
    /// Restore an item from a snapshot.
    Restore {
        snapshot_id: String,
        item_id: String,
    },
}

/// Result of a mutation: what happened + snapshot for undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResult {
    pub ok: bool,
    pub message: String,
    pub snapshot_entry: Option<SnapshotEntry>,
}

/// Parse an item id ("category:location:name") into parts.
pub fn parse_item_id(id: &str) -> Option<(Category, String, String)> {
    let mut it = id.splitn(3, ':');
    let cat = match it.next()? {
        "registry_run" => Category::RegistryRun,
        "startup_folder" => Category::StartupFolder,
        "scheduled_task" => Category::ScheduledTask,
        _ => return None,
    };
    let location = it.next()?.to_string();
    let name = it.next()?.to_string();
    Some((cat, location, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_item_id_roundtrip() {
        let (cat, loc, name) =
            parse_item_id("registry_run:HKCU\\...\\Run:OneDrive").unwrap();
        assert_eq!(cat, Category::RegistryRun);
        assert_eq!(loc, "HKCU\\...\\Run");
        assert_eq!(name, "OneDrive");
    }

    #[test]
    fn parse_item_id_rejects_bad() {
        assert!(parse_item_id("bogus").is_none());
        assert!(parse_item_id("nope:onlytwo").is_none());
    }

    #[test]
    fn writeop_json_roundtrips() {
        let op = WriteOp::Disable {
            item_id: "registry_run:HKCU\\...\\Run:Foo".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: WriteOp = serde_json::from_str(&json).unwrap();
        match back {
            WriteOp::Disable { item_id } => assert!(item_id.contains("Foo")),
            _ => panic!("wrong variant"),
        }
    }
}

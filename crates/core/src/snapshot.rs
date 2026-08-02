//! Snapshot store: JSON files on disk with 7-day retention.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::model::{Snapshot, SnapshotEntry};
use crate::Error;

const RETENTION_DAYS: i64 = 7;

/// A snapshot store rooted at `dir`. Snapshot files are self-contained JSON.
pub struct SnapshotStore {
    dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Create a new snapshot from a list of entries.
    pub fn create(&self, operation: &str, entries: Vec<SnapshotEntry>) -> Result<Snapshot, Error> {
        let now = Utc::now();
        let snap = Snapshot {
            id: Uuid::new_v4().to_string(),
            created_at: now.to_rfc3339(),
            operation: operation.to_string(),
            entries,
        };
        let path = self.dir.join(format!("snapshot-{}.json", snap.id));
        let bytes = serde_json::to_vec_pretty(&snap)?;
        fs::create_dir_all(&self.dir)?;
        fs::write(&path, bytes)?;
        Ok(snap)
    }

    /// List snapshots, newest first.
    pub fn list(&self) -> Result<Vec<Snapshot>, Error> {
        let mut out = Vec::new();
        if !self.dir.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("snapshot-") || !name.ends_with(".json") {
                continue;
            }
            let bytes = fs::read(entry.path())?;
            if let Ok(snap) = serde_json::from_slice::<Snapshot>(&bytes) {
                out.push(snap);
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    /// Remove snapshots older than `days` (default 7). Call after each write.
    pub fn prune_old(&self) -> Result<usize, Error> {
        let cutoff = Utc::now() - Duration::days(RETENTION_DAYS);
        let mut removed = 0;
        for snap in self.list()? {
            let ts: DateTime<Utc> = DateTime::parse_from_rfc3339(&snap.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or(Utc::now());
            if ts < cutoff {
                let path = self.dir.join(format!("snapshot-{}.json", snap.id));
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    /// Load a snapshot by id.
    pub fn get(&self, id: &str) -> Result<Snapshot, Error> {
        let path = self.dir.join(format!("snapshot-{id}.json"));
        let bytes = fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Helper to build a SnapshotEntry from a current item state.
pub fn entry_from_parts(
    item_id: &str,
    original_name: &str,
    current_name: &str,
    command: &str,
    location: &str,
    category: crate::model::Category,
) -> SnapshotEntry {
    SnapshotEntry {
        item_id: item_id.to_string(),
        original_name: original_name.to_string(),
        current_name: current_name.to_string(),
        command: command.to_string(),
        location: location.to_string(),
        category,
        was_removed: false,
    }
}

/// Compact snapshot metadata for listing (no entries blob).
pub fn summarize(snap: &Snapshot) -> serde_json::Value {
    json!({
        "id": snap.id,
        "created_at": snap.created_at,
        "operation": snap.operation,
        "entry_count": snap.entries.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bk-test-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn create_and_list_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let store = SnapshotStore::new(dir.clone());
        let entries = vec![entry_from_parts(
            "registry_run:HKCU\\Run:Foo",
            "Foo",
            "Foo.disabled",
            "cmd /c foo.exe",
            "HKCU\\Run",
            Category::RegistryRun,
        )];
        let snap = store.create("disable", entries).unwrap();
        assert_eq!(snap.entries.len(), 1);

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, snap.id);

        let got = store.get(&snap.id).unwrap();
        assert_eq!(got.operation, "disable");
        assert_eq!(got.entries[0].original_name, "Foo");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn prune_removes_old_but_keeps_fresh() {
        let dir = tmp_dir("prune");
        let store = SnapshotStore::new(dir.clone());
        let fresh = store.create("disable", vec![]).unwrap();

        // Craft an old snapshot file directly.
        let old_id = "old-snap-1";
        let old_path = dir.join(format!("snapshot-{old_id}.json"));
        let old_snap = Snapshot {
            id: old_id.to_string(),
            created_at: (Utc::now() - Duration::days(30)).to_rfc3339(),
            operation: "disable".to_string(),
            entries: vec![],
        };
        fs::write(&old_path, serde_json::to_vec(&old_snap).unwrap()).unwrap();

        let removed = store.prune_old().unwrap();
        assert_eq!(removed, 1);
        assert!(!old_path.exists());
        assert!(store.get(&fresh.id).is_ok());
        fs::remove_dir_all(dir).ok();
    }
}

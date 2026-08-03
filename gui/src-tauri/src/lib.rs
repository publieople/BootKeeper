// BootKeeper GUI Rust side.
//
// Read: `list_items` enumerates via core. Write: goes through the shared
// elevated-helper launcher (confirmation dialog is the helper's job).
// Snapshots are read via the SnapshotStore.

use bootkeeper_core::windows::launcher;
use bootkeeper_core::SnapshotStore;

#[tauri::command]
fn list_items() -> Vec<serde_json::Value> {
    #[cfg(windows)]
    {
        use bootkeeper_core::model::Signature;
        use bootkeeper_core::windows::signature::verify_file_signature;
        use bootkeeper_core::{enrich, windows};

        let verifier = |cmd: &str| {
            let path = cmd.trim_matches('"').split_whitespace().next().unwrap_or("");
            if path.is_empty() {
                Signature::Unknown
            } else {
                verify_file_signature(path)
            }
        };
        windows::enumerate_all()
            .iter()
            .map(|r| serde_json::to_value(enrich(r, &verifier)).unwrap_or_default())
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Write action: disable / enable / remove → shared elevated helper.
/// The helper pops the confirmation dialog; without a user "Yes" nothing runs.
#[tauri::command]
async fn run_write_action(action: String, id: String) -> Result<serde_json::Value, String> {
    let op = match action.as_str() {
        "disable" => bootkeeper_core::WriteOp::Disable { item_id: id },
        "enable" => bootkeeper_core::WriteOp::Enable { item_id: id },
        "remove" => bootkeeper_core::WriteOp::Remove { item_id: id },
        other => return Err(format!("unknown write action: {other}")),
    };
    let result = launcher::run_helper(op).map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

/// Restore an item from a snapshot entry.
#[tauri::command]
async fn restore_item(snapshot_id: String, item_id: String) -> Result<serde_json::Value, String> {
    let store = SnapshotStore::new(launcher::snapshot_dir());
    let snap = store.get(&snapshot_id).map_err(|e| e.to_string())?;
    let entry = if item_id.is_empty() {
        // Restore the snapshot's first entry (v1: single-entry snapshots).
        snap.entries
            .first()
            .ok_or_else(|| format!("snapshot {snapshot_id} is empty"))?
            .clone()
    } else {
        snap.entries
            .iter()
            .find(|e| e.item_id == item_id)
            .ok_or_else(|| format!("item {item_id} not in snapshot {snapshot_id}"))?
            .clone()
    };
    #[cfg(windows)]
    {
        let result = launcher::run_restore(entry, &snapshot_id, launcher::snapshot_dir())
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (entry, snapshot_id);
        Err("restore requires Windows (bootkeeper-helper)".into())
    }
}

/// List snapshot metadata (id, time, operation, entry count).
#[tauri::command]
fn list_snapshots() -> Vec<serde_json::Value> {
    let store = SnapshotStore::new(launcher::snapshot_dir());
    match store.list() {
        Ok(snaps) => snaps
            .iter()
            .map(bootkeeper_core::snapshot::summarize)
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_items,
            run_write_action,
            restore_item,
            list_snapshots
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

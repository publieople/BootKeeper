//! Windows write operations.
//!
//! These are the ONLY functions that mutate startup state. They run inside
//! the elevated helper process AFTER the confirmation dialog. Protocol types
//! (WriteOp/WriteResult/parse_item_id) live in crate::write (platform-free).

use crate::model::{Category, SnapshotEntry};
use crate::write::{parse_item_id, WriteResult};
use crate::Error;

const DISABLED_SUFFIX: &str = ".disabled";

/// Look up a raw entry by id (Windows-only real enumeration).
#[cfg(windows)]
pub fn find_raw_by_id(id: &str) -> Option<crate::model::RawEntry> {
    let (cat, location, name) = parse_item_id(id)?;
    crate::windows::enumerate_all()
        .into_iter()
        .find(|r| r.category == cat && r.location == location && r.name == name)
}

/// Disable: rename value/file (registry, startup folder) or set task state.
#[cfg(windows)]
pub fn disable(item_id: &str) -> Result<WriteResult, Error> {
    let (cat, _loc, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    match cat {
        Category::ScheduledTask => {
            let entry = set_task_enabled(item_id, false)?;
            Ok(WriteResult {
                ok: true,
                message: format!("disabled {item_id}"),
                snapshot_entry: Some(entry),
            })
        }
        _ => {
            if name.ends_with(DISABLED_SUFFIX) {
                return Ok(WriteResult {
                    ok: true,
                    message: "already disabled".into(),
                    snapshot_entry: None,
                });
            }
            let new_name = format!("{name}{DISABLED_SUFFIX}");
            let entry = rename_entry(item_id, &name, &new_name)?;
            Ok(WriteResult {
                ok: true,
                message: format!("disabled {item_id}"),
                snapshot_entry: Some(entry),
            })
        }
    }
}

/// Enable: rename back (registry, startup folder) or set task state.
#[cfg(windows)]
pub fn enable(item_id: &str) -> Result<WriteResult, Error> {
    let (cat, _loc, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    match cat {
        Category::ScheduledTask => {
            let entry = set_task_enabled(item_id, true)?;
            Ok(WriteResult {
                ok: true,
                message: format!("enabled {item_id}"),
                snapshot_entry: Some(entry),
            })
        }
        _ => {
            if !name.ends_with(DISABLED_SUFFIX) {
                return Ok(WriteResult {
                    ok: true,
                    message: "already enabled".into(),
                    snapshot_entry: None,
                });
            }
            let orig = name.trim_end_matches(DISABLED_SUFFIX);
            let entry = rename_entry(item_id, &name, orig)?;
            Ok(WriteResult {
                ok: true,
                message: format!("enabled {item_id}"),
                snapshot_entry: Some(entry),
            })
        }
    }
}

/// Remove an entry entirely (after snapshot backup).
#[cfg(windows)]
pub fn remove(item_id: &str) -> Result<WriteResult, Error> {
    let (cat, location, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    match cat {
        Category::RegistryRun => remove_registry_value(&location, &name, item_id),
        Category::StartupFolder => remove_file(&location, &name, item_id),
        Category::ScheduledTask => remove_task(&location, &name, item_id),
    }
}

// ---------- rename (disable/enable for registry + startup folder) ----------

#[cfg(windows)]
fn rename_entry(item_id: &str, old_name: &str, new_name: &str) -> Result<SnapshotEntry, Error> {
    let (cat, location, _) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    match cat {
        Category::RegistryRun => rename_registry_value(&location, old_name, new_name, item_id),
        Category::StartupFolder => rename_file(&location, old_name, new_name, item_id),
        Category::ScheduledTask => Err(Error::Msg(
            "rename not supported for scheduled tasks (use task state)".into(),
        )),
    }
}

#[cfg(windows)]
fn rename_registry_value(
    key_path: &str,
    old_name: &str,
    new_name: &str,
    item_id: &str,
) -> Result<SnapshotEntry, Error> {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    let (hive, rest) = parse_hive_and_key(key_path)?;
    let key = hive.open_subkey_with_flags(rest, KEY_READ | KEY_WRITE)?;
    let value: winreg::RegValue = key.get_raw_value(old_name)?;
    key.set_raw_value(new_name, &value)?;
    key.delete_value(old_name)?;

    Ok(SnapshotEntry {
        item_id: item_id.to_string(),
        original_name: old_name.to_string(),
        current_name: new_name.to_string(),
        command: crate::windows::registry::decode_reg_value_bytes(value.vtype, &value.bytes),
        location: key_path.to_string(),
        category: Category::RegistryRun,
        was_removed: false,
    })
}

#[cfg(windows)]
fn rename_file(
    folder_label: &str,
    old_name: &str,
    new_name: &str,
    item_id: &str,
) -> Result<SnapshotEntry, Error> {
    let dir = crate::windows::startup_folder::startup_folder_path(folder_label)
        .ok_or_else(|| Error::Msg(format!("cannot resolve startup folder: {folder_label}")))?;
    let from = std::path::Path::new(&dir).join(old_name);
    let to = std::path::Path::new(&dir).join(new_name);
    std::fs::rename(&from, &to)?;
    Ok(SnapshotEntry {
        item_id: item_id.to_string(),
        original_name: old_name.to_string(),
        current_name: new_name.to_string(),
        command: to.to_string_lossy().to_string(),
        location: folder_label.to_string(),
        category: Category::StartupFolder,
        was_removed: false,
    })
}

// ---------- remove ----------

#[cfg(windows)]
fn remove_registry_value(
    key_path: &str,
    name: &str,
    item_id: &str,
) -> Result<WriteResult, Error> {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    let (hive, rest) = parse_hive_and_key(key_path)?;
    let key = hive.open_subkey_with_flags(rest, KEY_READ | KEY_WRITE)?;
    let value: winreg::RegValue = key.get_raw_value(name)?;
    key.delete_value(name)?;
    Ok(WriteResult {
        ok: true,
        message: format!("removed {item_id}"),
        snapshot_entry: Some(SnapshotEntry {
            item_id: item_id.to_string(),
            original_name: name.to_string(),
            current_name: name.to_string(),
            command: crate::windows::registry::decode_reg_value_bytes(value.vtype, &value.bytes),
            location: key_path.to_string(),
            category: Category::RegistryRun,
            was_removed: true,
        }),
    })
}

#[cfg(windows)]
fn remove_file(folder_label: &str, name: &str, item_id: &str) -> Result<WriteResult, Error> {
    let dir = crate::windows::startup_folder::startup_folder_path(folder_label)
        .ok_or_else(|| Error::Msg(format!("cannot resolve startup folder: {folder_label}")))?;
    let path = std::path::Path::new(&dir).join(name);
    std::fs::remove_file(&path)?;
    Ok(WriteResult {
        ok: true,
        message: format!("removed {item_id}"),
        snapshot_entry: Some(SnapshotEntry {
            item_id: item_id.to_string(),
            original_name: name.to_string(),
            current_name: name.to_string(),
            command: path.to_string_lossy().to_string(),
            location: folder_label.to_string(),
            category: Category::StartupFolder,
            was_removed: true,
        }),
    })
}

#[cfg(windows)]
fn remove_task(task_path: &str, name: &str, item_id: &str) -> Result<WriteResult, Error> {
    let folder = task_parent_folder(task_path);
    let (service, root) = connect_task_service()?;
    let _ = service;
    let folder = unsafe { root.GetFolder(&windows::core::BSTR::from(&folder)) }
            .map_err(|e| Error::Msg(e.to_string()))?;
    unsafe {
        folder
            .DeleteTask(&windows::core::BSTR::from(name), 0)
            .map_err(|e| Error::Msg(e.to_string()))?;
    }
    Ok(WriteResult {
        ok: true,
        message: format!("removed {item_id}"),
        snapshot_entry: Some(SnapshotEntry {
            item_id: item_id.to_string(),
            original_name: name.to_string(),
            current_name: name.to_string(),
            command: String::new(),
            location: task_path.to_string(),
            category: Category::ScheduledTask,
            was_removed: true,
        }),
    })
}

// ---------- task state (disable/enable for scheduled tasks) ----------

#[cfg(windows)]
fn set_task_enabled(item_id: &str, enabled: bool) -> Result<SnapshotEntry, Error> {
    let (cat, location, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    debug_assert_eq!(cat, Category::ScheduledTask);
    let folder = task_parent_folder(&location);
    let (_service, root) = connect_task_service()?;
    let folder = unsafe { root.GetFolder(&windows::core::BSTR::from(&folder)) }
        .map_err(|e| Error::Msg(e.to_string()))?;
    let task = unsafe { folder.GetTask(&windows::core::BSTR::from(&name)) }
        .map_err(|e| Error::Msg(e.to_string()))?;
    unsafe {
        task.SetEnabled(windows::Win32::Foundation::VARIANT_BOOL(if enabled { -1 } else { 0 }))
            .map_err(|e| Error::Msg(e.to_string()))?;
    }
    Ok(SnapshotEntry {
        item_id: item_id.to_string(),
        original_name: name.to_string(),
        current_name: name.to_string(),
        command: String::new(),
        location: location.clone(),
        category: Category::ScheduledTask,
        was_removed: false,
    })
}

// ---------- shared helpers ----------

#[cfg(windows)]
fn parse_hive_and_key(
    key_path: &str,
) -> Result<(winreg::RegKey, &str), Error> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let (hive, rest) = if let Some(r) = key_path.strip_prefix("HKCU") {
        (HKEY_CURRENT_USER, r.trim_start_matches('\\'))
    } else if let Some(r) = key_path.strip_prefix("HKLM") {
        (HKEY_LOCAL_MACHINE, r.trim_start_matches('\\'))
    } else {
        return Err(Error::Msg(format!("bad hive in key path: {key_path}")));
    };
    Ok((RegKey::predef(hive), rest))
}

/// Connect to Task Scheduler COM service and return the root folder.
#[cfg(windows)]
fn connect_task_service() -> Result<
    (
        windows::Win32::System::TaskScheduler::ITaskService,
        windows::Win32::System::TaskScheduler::ITaskFolder,
    ),
    Error,
> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::System::TaskScheduler::{CLSID_CTaskScheduler, ITaskFolder, ITaskService};
    use windows::Win32::System::Variant::VARIANT;

    let service: ITaskService = unsafe {
        CoCreateInstance(&CLSID_CTaskScheduler, None, CLSCTX_INPROC_SERVER)
    }
    .map_err(|e| Error::Msg(e.to_string()))?;
    let none = VARIANT::default();
    unsafe {
        service
            .Connect(&none, &none, &none, &none)
            .map_err(|e| Error::Msg(e.to_string()))?;
    }
    let root: ITaskFolder = unsafe { service.GetFolder(&windows::core::BSTR::from("\\")) }
        .map_err(|e| Error::Msg(e.to_string()))?;
    Ok((service, root))
}

/// Given a task path like "\\Microsoft\\Windows\\Foo", return the parent
/// folder "\\Microsoft\\Windows".
#[cfg(windows)]
fn task_parent_folder(task_path: &str) -> String {
    let t = task_path.trim_end_matches('\\');
    match t.rfind('\\') {
        Some(idx) if idx > 0 => t[..idx].to_string(),
        _ => "\\".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_suffix_logic() {
        assert!(!DISABLED_SUFFIX.is_empty());
        assert_eq!("Foo.disabled".trim_end_matches(DISABLED_SUFFIX), "Foo");
    }

    #[test]
    fn task_parent_folder_paths() {
        assert_eq!(task_parent_folder("\\Microsoft\\Windows\\Foo"), "\\Microsoft\\Windows");
        assert_eq!(task_parent_folder("\\Foo"), "\\");
        assert_eq!(task_parent_folder("\\"), "\\");
    }
}

/// Restore an item from a snapshot entry (undo a disable/enable/remove).
///
/// - was_removed=true  → recreate the registry value / file.
/// - else              → rename current_name back to original_name
///   (undoes disable) or forward (undoes enable).
#[cfg(windows)]
pub fn restore(snap: &crate::model::SnapshotEntry) -> Result<WriteResult, Error> {
    match snap.category {
        Category::RegistryRun => restore_registry(snap),
        Category::StartupFolder => restore_file(snap),
        Category::ScheduledTask => restore_task(snap),
    }
}

#[cfg(windows)]
fn restore_registry(snap: &crate::model::SnapshotEntry) -> Result<WriteResult, Error> {
    use winreg::enums::{KEY_READ, KEY_WRITE};
    use winreg::RegValue;

    let (hive, rest) = parse_hive_and_key(&snap.location)?;
    let key = hive.open_subkey_with_flags(rest, KEY_READ | KEY_WRITE)?;

    if snap.was_removed {
        // Recreate the deleted value with its original command.
        key.set_raw_value(&snap.original_name, &RegValue {
            bytes: std::borrow::Cow::Owned(snap.command.as_bytes().to_vec()),
            vtype: winreg::enums::REG_SZ,
        })?;
    } else {
        // Rename current_name -> original_name.
        if let Ok(value) = key.get_raw_value(&snap.current_name) {
            key.set_raw_value(&snap.original_name, &value)?;
            key.delete_value(&snap.current_name)?;
        }
    }
    Ok(WriteResult {
        ok: true,
        message: format!("restored {}", snap.item_id),
        snapshot_entry: None,
    })
}

#[cfg(windows)]
fn restore_file(snap: &crate::model::SnapshotEntry) -> Result<WriteResult, Error> {
    let dir = crate::windows::startup_folder::startup_folder_path(&snap.location)
        .ok_or_else(|| Error::Msg(format!("cannot resolve startup folder: {}", snap.location)))?;
    let dir = std::path::Path::new(&dir);

    if snap.was_removed {
        // Recreate an empty file with the original name. Content is lost,
        // but the startup entry is restored (best effort).
        let target = dir.join(&snap.original_name);
        std::fs::write(&target, b"")?;
    } else {
        let from = dir.join(&snap.current_name);
        let to = dir.join(&snap.original_name);
        if from.exists() {
            std::fs::rename(&from, &to)?;
        }
    }
    Ok(WriteResult {
        ok: true,
        message: format!("restored {}", snap.item_id),
        snapshot_entry: None,
    })
}

#[cfg(windows)]
fn restore_task(snap: &crate::model::SnapshotEntry) -> Result<WriteResult, Error> {
    // Task removal is destructive and the full task XML is not stored in the
    // snapshot (v1). Recreate only when the task still exists: re-enable it.
    let folder = task_parent_folder(&snap.location);
    let (_service, root) = connect_task_service()?;
    let folder = unsafe { root.GetFolder(&windows::core::BSTR::from(&folder)) }
        .map_err(|e| Error::Msg(e.to_string()))?;
    let task = unsafe { folder.GetTask(&windows::core::BSTR::from(&snap.original_name)) }
        .map_err(|e| Error::Msg(e.to_string()))?;
    unsafe {
        task.SetEnabled(windows::Win32::Foundation::VARIANT_BOOL(-1))
            .map_err(|e| Error::Msg(e.to_string()))?;
    }
    Ok(WriteResult {
        ok: true,
        message: format!("restored {}", snap.item_id),
        snapshot_entry: None,
    })
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    #[test]
    fn task_parent_folder_paths_restore() {
        assert_eq!(task_parent_folder("\\Microsoft\\Windows\\Foo"), "\\Microsoft\\Windows");
        assert_eq!(task_parent_folder("\\Foo"), "\\");
    }
}

/// Add a startup entry.
///
/// - RegistryRun: write a REG_SZ value under the Run key.
/// - StartupFolder: copy the target file into the startup folder.
/// - ScheduledTask: v1 not supported (task creation via COM is heavy; the
///   CLI/GUI surface exposes Add only for registry and folder).
#[cfg(windows)]
pub fn add(op: &crate::write::WriteOpAdd) -> Result<WriteResult, Error> {
    match op.category {
        Category::RegistryRun => add_registry(op),
        Category::StartupFolder => add_file(op),
        Category::ScheduledTask => Err(Error::Msg(
            "adding scheduled tasks is not implemented in v1".into(),
        )),
    }
}

#[cfg(windows)]
fn add_registry(op: &crate::write::WriteOpAdd) -> Result<WriteResult, Error> {
    use winreg::enums::{KEY_READ, KEY_WRITE, REG_SZ};
    use winreg::RegValue;

    let (hive, rest) = parse_hive_and_key(&op.location)?;
    let key = hive.open_subkey_with_flags(rest, KEY_READ | KEY_WRITE)?;

    key.set_raw_value(
        &op.name,
        &RegValue {
            bytes: std::borrow::Cow::Owned(
                op.command
                    .encode_utf16()
                    .flat_map(|u| u.to_le_bytes())
                    .collect(),
            ),
            vtype: REG_SZ,
        },
    )?;

    Ok(WriteResult {
        ok: true,
        message: format!("added {} to {}", op.name, op.location),
        snapshot_entry: None,
    })
}

#[cfg(windows)]
fn add_file(op: &crate::write::WriteOpAdd) -> Result<WriteResult, Error> {
    let dir = crate::windows::startup_folder::startup_folder_path(&op.location)
        .ok_or_else(|| Error::Msg(format!("cannot resolve startup folder: {}", op.location)))?;
    let src = std::path::Path::new(&op.command);
    if !src.exists() {
        return Err(Error::Msg(format!("source file not found: {}", op.command)));
    }
    let target = std::path::Path::new(&dir).join(&op.name);
    std::fs::copy(src, &target)?;
    Ok(WriteResult {
        ok: true,
        message: format!("added {} to {}", op.name, op.location),
        snapshot_entry: None,
    })
}

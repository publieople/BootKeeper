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

/// Disable: rename so the entry no longer autostarts.
#[cfg(windows)]
pub fn disable(item_id: &str) -> Result<WriteResult, Error> {
    let (_cat, _loc, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
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

/// Enable: rename back to the original name.
#[cfg(windows)]
pub fn enable(item_id: &str) -> Result<WriteResult, Error> {
    let (_cat, _loc, name) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
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

/// The shared rename path for registry values and startup-folder files.
#[cfg(windows)]
fn rename_entry(item_id: &str, old_name: &str, new_name: &str) -> Result<SnapshotEntry, Error> {
    let (cat, location, _) = parse_item_id(item_id)
        .ok_or_else(|| Error::Msg(format!("bad item id: {item_id}")))?;
    match cat {
        Category::RegistryRun => rename_registry_value(&location, old_name, new_name, item_id),
        Category::StartupFolder => rename_file(&location, old_name, new_name, item_id),
        Category::ScheduledTask => Err(Error::Msg(
            "rename not supported for scheduled tasks (use task state enable/disable)".into(),
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
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    // key_path looks like "HKCU\...\Run" — parse hive + actual subkey.
    let (hive, rest) = if let Some(r) = key_path.strip_prefix("HKCU") {
        (HKEY_CURRENT_USER, r.trim_start_matches('\\'))
    } else if let Some(r) = key_path.strip_prefix("HKLM") {
        (HKEY_LOCAL_MACHINE, r.trim_start_matches('\\'))
    } else {
        return Err(Error::Msg(format!("bad hive in key path: {key_path}")));
    };
    // Skip the literal "...\" segment if present.
    let rest = rest.strip_prefix("...\\").unwrap_or(rest);

    let key = RegKey::predef(hive).open_subkey_with_flags(rest, KEY_READ | KEY_WRITE)?;
    let value: winreg::RegValue = key.get_raw_value(old_name)?;
    key.set_raw_value(new_name, &value)?;
    key.delete_value(old_name)?;

    Ok(SnapshotEntry {
        item_id: item_id.to_string(),
        original_name: old_name.to_string(),
        current_name: new_name.to_string(),
        command: String::from_utf8_lossy(&value.bytes).to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_suffix_logic() {
        assert!(!"Foo".ends_with(DISABLED_SUFFIX));
        assert!("Foo.disabled".ends_with(DISABLED_SUFFIX));
        assert_eq!("Foo.disabled".trim_end_matches(DISABLED_SUFFIX), "Foo");
    }
}

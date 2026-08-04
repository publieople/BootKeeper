//! Read the Windows Task Manager "StartupApproved" registry keys.
//!
//! When a user disables a startup entry via Task Manager (or Settings →
//! Startup), Windows writes a binary value to:
//!   HKCU/HKLM\...\Explorer\StartupApproved\{Run,StartupFolder}
//!
//! The value name matches the registry value name (for Run) or file path
//! (for StartupFolder). The first 4 bytes of the binary data encode the
//! state: 0x01 = enabled, 0x02/0x03 = disabled.
//!
//! BootKeeper must check this in addition to the .disabled suffix rename
//! so that Task-Manager-disabled entries show up correctly.

use crate::model::{Category, RawEntry};

#[cfg(windows)]
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ},
    RegKey,
};

/// Returns true if Windows Task Manager has this entry marked disabled.
/// `hive` is either "HKCU" or "HKLM".
/// `category_subkey` is "Run" for registry run, "StartupFolder" for folders.
/// `value_name` is the value name (or file path in the folder).
pub fn is_approved_disabled(hive: &str, category_subkey: &str, value_name: &str) -> bool {
    #[cfg(not(windows))]
    {
        let _ = (hive, category_subkey, value_name);
        false
    }
    #[cfg(windows)]
    {
        let reg_hive = match hive {
            "HKCU" => HKEY_CURRENT_USER,
            "HKLM" => HKEY_LOCAL_MACHINE,
            _ => return false,
        };
        let key_path = format!(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\{category_subkey}"
        );
        let key = match RegKey::predef(reg_hive).open_subkey_with_flags(&key_path, KEY_READ) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let raw: winreg::RegValue = match key.get_raw_value(value_name) {
            Ok(v) => v,
            Err(_) => return false,
        };

        if raw.bytes.len() >= 4 {
            let state = u32::from_le_bytes([raw.bytes[0], raw.bytes[1], raw.bytes[2], raw.bytes[3]]);
            // Bit 0 = 1 means enabled, 0 = disabled.
            (state & 1) == 0
        } else {
            false
        }
    }
}

/// Check whether a RawEntry is disabled by Windows Task Manager.
/// Handles both registry Run entries and startup folder entries.
pub fn is_entry_approved_disabled(raw: &RawEntry) -> bool {
    match raw.category {
        Category::RegistryRun => {
            let hive = if raw.location.starts_with("HKCU") {
                "HKCU"
            } else if raw.location.starts_with("HKLM") {
                "HKLM"
            } else {
                return false;
            };
            let subkey = raw
                .location
                .split('\\')
                .last()
                .unwrap_or("Run");
            is_approved_disabled(hive, subkey, &raw.name)
        }
        Category::StartupFolder => {
            let hive = if raw.location.contains("common") {
                "HKLM"
            } else {
                "HKCU"
            };
            is_approved_disabled(hive, "StartupFolder", &raw.command)
        }
        _ => false,
    }
}

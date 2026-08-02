//! Registry Run / RunOnce enumeration (HKCU + HKLM).

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use crate::model::{Category, RawEntry};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUNONCE_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce";

/// Enumerate Run and RunOnce values under HKCU and HKLM.
pub fn enumerate_registry_run() -> Vec<RawEntry> {
    let mut out = Vec::new();
    let hives = [
        ("HKCU", HKEY_CURRENT_USER),
        ("HKLM", HKEY_LOCAL_MACHINE),
    ];

    for (hive_name, hive) in hives {
        for (subkey_name, subkey_path) in [("Run", RUN_KEY), ("RunOnce", RUNONCE_KEY)] {
            let key_path = format!("{hive_name}\\...\\{subkey_name}");
            match RegKey::predef(hive).open_subkey_with_flags(subkey_path, KEY_READ) {
                Ok(key) => {
                    for (name, value) in key.enum_values().flatten() {
                        // Run values are strings (REG_SZ / REG_EXPAND_SZ).
                        let winreg::RegValue { bytes, .. } = value;
                        let cmd = String::from_utf8_lossy(&bytes).to_string();
                        out.push(RawEntry::new(
                            Category::RegistryRun,
                            &key_path,
                            &name,
                            &cmd,
                        ));
                    }
                }
                Err(_) => continue, // key doesn't exist — normal on many systems
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    // Windows-only tests would require a live registry; skip on CI/non-Windows.
    #[test]
    fn compiles() {}
}

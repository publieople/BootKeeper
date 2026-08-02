//! Registry Run / RunOnce enumeration (HKCU + HKLM).

use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use crate::model::{Category, RawEntry};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUNONCE_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce";

/// Decode a registry value's raw bytes into a String.
///
/// REG_SZ / REG_EXPAND_SZ values are stored as UTF-16LE with a trailing null.
/// Other types fall back to a lossy UTF-8 decode (paths and commands are
/// almost always REG_SZ, so this is the correct path for us).
pub fn decode_reg_value_bytes(vtype: RegType, bytes: &[u8]) -> String {
    match vtype {
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
            // UTF-16LE, strip trailing null unit(s).
            let mut units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            while units.last() == Some(&0) {
                units.pop();
            }
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).to_string(),
    }
}

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
                        let cmd = decode_reg_value_bytes(value.vtype, &value.bytes);
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
    use super::*;

    /// "C:\Users\甲\App.exe" encoded as UTF-16LE with trailing null.
    fn utf16le(s: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for unit in s.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn decode_reg_sz_utf16() {
        let cmd = r"C:\Users\甲\App.exe --flag";
        let raw = utf16le(cmd);
        let decoded = decode_reg_value_bytes(RegType::REG_SZ, &raw);
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn decode_strips_trailing_null() {
        let mut raw = utf16le("C:\\foo.exe");
        raw.extend_from_slice(&0u16.to_le_bytes()); // extra null
        raw.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(decode_reg_value_bytes(RegType::REG_SZ, &raw), "C:\\foo.exe");
    }

    #[test]
    fn decode_expand_sz() {
        let cmd = "%ProgramFiles%\\App\\app.exe";
        let raw = utf16le(cmd);
        assert_eq!(decode_reg_value_bytes(RegType::REG_EXPAND_SZ, &raw), cmd);
    }

    #[test]
    fn decode_non_string_falls_back() {
        let raw = b"plain ascii bytes";
        assert_eq!(decode_reg_value_bytes(RegType::REG_BINARY, raw), "plain ascii bytes");
    }
}

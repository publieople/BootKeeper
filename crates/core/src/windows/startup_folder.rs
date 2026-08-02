//! Startup folder enumeration (user + common).

use std::path::PathBuf;

use crate::model::{Category, RawEntry};

/// Known folder CSIDL values for startup locations.
const CSIDL_STARTUP: i32 = 0x07; // user startup
const CSIDL_COMMON_STARTUP: i32 = 0x18; // all-users startup

/// Fetch a known-folder path via SHGetFolderPathW.
#[cfg(windows)]
fn known_folder_path(csidl: i32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::UI::Shell::SHGetFolderPathW;

    let mut buf = [0u16; 260];
    // CSIDL_FLAG_CREATE = 0x8000 — create the folder if missing.
    let hr = unsafe { SHGetFolderPathW(None, csidl | 0x8000, None, 0, &mut buf) };
    if hr.is_ok() {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let os = OsString::from_wide(&buf[..len]);
        Some(os.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Enumerate .lnk/.exe/.cmd/.bat files in startup folders.
#[cfg(windows)]
pub fn enumerate_startup_folders() -> Vec<RawEntry> {
    let mut out = Vec::new();
    for (label, csidl) in [
        ("user_startup", CSIDL_STARTUP),
        ("common_startup", CSIDL_COMMON_STARTUP),
    ] {
        let Some(dir) = known_folder_path(csidl) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let ext = PathBuf::from(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if matches!(ext.as_str(), "lnk" | "exe" | "cmd" | "bat") {
                let full = entry.path().to_string_lossy().to_string();
                out.push(RawEntry::new(Category::StartupFolder, label, &name, &full));
            }
        }
    }
    out
}

#[cfg(not(windows))]
pub fn enumerate_startup_folders() -> Vec<RawEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}

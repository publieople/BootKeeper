//! Elevated helper launcher — shared by CLI and GUI.
//!
//! The write path is always: build a request file → ShellExecuteW "runas"
//! (UAC) → helper shows confirmation → executes → writes result JSON.
//! This module owns that protocol so CLI and GUI don't each reimplement it.

use std::path::PathBuf;

use crate::write::{WriteOp, WriteResult};
use crate::Error;

/// Where request/result temp files go (default: BOOTKEEPER_DATA/tmp).
pub fn tmp_dir() -> PathBuf {
    std::env::var_os("BOOTKEEPER_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(windows)]
            let base = std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."));
            #[cfg(not(windows))]
            let base = PathBuf::from(".");
            base.join("BootKeeper")
        })
        .join("tmp")
}

/// Default snapshot store dir (BOOTKEEPER_DATA/snapshots).
pub fn snapshot_dir() -> PathBuf {
    std::env::var_os("BOOTKEEPER_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(windows)]
            let base = std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."));
            #[cfg(not(windows))]
            let base = PathBuf::from(".");
            base.join("BootKeeper")
        })
        .join("snapshots")
}

/// Run a write operation through the elevated helper (confirmation dialog).
/// Blocks until the helper finishes. Non-Windows: returns an error.
pub fn run_helper(operation: WriteOp) -> Result<WriteResult, Error> {
    run_helper_with_snapshot_dir(operation, snapshot_dir())
}

/// Same as [`run_helper`] but with an explicit snapshot dir (tests/GUI).
pub fn run_helper_with_snapshot_dir(
    operation: WriteOp,
    snap_dir: PathBuf,
) -> Result<WriteResult, Error> {
    #[cfg(not(windows))]
    {
        let _ = (operation, snap_dir);
        Err(Error::Msg(
            "write operations require Windows (bootkeeper-helper)".into(),
        ))
    }

    #[cfg(windows)]
    {
        use serde_json::json;

        let dir = tmp_dir();
        std::fs::create_dir_all(&dir)?;
        let pid = std::process::id();
        let req_path = dir.join(format!("op-{pid}.json"));
        let res_path = dir.join(format!("result-{pid}.json"));

        let request = json!({
            "operation": operation,
            "snapshot_dir": snap_dir.to_string_lossy(),
        });
        std::fs::write(&req_path, serde_json::to_vec_pretty(&request)?)?;

        launch_helper(&req_path, &res_path)?;

        // Poll for the result file.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if res_path.exists() {
                break;
            }
            if std::time::Instant::now() > deadline {
                let _ = std::fs::remove_file(&req_path);
                return Err(Error::Msg(
                    "timed out waiting for helper (user may not have confirmed)".into(),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }

        let bytes = std::fs::read(&res_path)?;
        let _ = std::fs::remove_file(&req_path);
        let _ = std::fs::remove_file(&res_path);
        let result: WriteResult = serde_json::from_slice(&bytes)?;
        Ok(result)
    }
}

/// Restore from a snapshot entry (special request shape).
#[cfg(windows)]
pub fn run_restore(
    entry: crate::SnapshotEntry,
    snapshot_id: &str,
    snap_dir: PathBuf,
) -> Result<WriteResult, Error> {
    use serde_json::json;

    let dir = tmp_dir();
    std::fs::create_dir_all(&dir)?;
    let pid = std::process::id();
    let req_path = dir.join(format!("restore-{pid}.json"));
    let res_path = dir.join(format!("result-{pid}.json"));

    let request = json!({
        "restore": entry,
        "snapshot_id": snapshot_id,
        "snapshot_dir": snap_dir.to_string_lossy(),
    });
    std::fs::write(&req_path, serde_json::to_vec_pretty(&request)?)?;

    launch_helper(&req_path, &res_path)?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        if res_path.exists() {
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = std::fs::remove_file(&req_path);
            return Err(Error::Msg(
                "timed out waiting for helper (user may not have confirmed)".into(),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    let bytes = std::fs::read(&res_path)?;
    let _ = std::fs::remove_file(&req_path);
    let _ = std::fs::remove_file(&res_path);
    let result: WriteResult = serde_json::from_slice(&bytes)?;
    Ok(result)
}

/// Launch the helper elevated via ShellExecuteW "runas" (UAC prompt).
/// The helper path is resolved relative to the current executable:
/// bootkeeper.exe / bootkeeper-gui.exe sit next to bootkeeper-helper.exe.
#[cfg(windows)]
fn launch_helper(req_path: &std::path::Path, res_path: &std::path::Path) -> Result<(), Error> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let helper = helper_path()?;

    let verb = HSTRING::from("runas");
    let file = HSTRING::from(helper.to_string_lossy().as_ref());
    let params = HSTRING::from(format!(
        "\"{}\" \"{}\"",
        req_path.to_string_lossy(),
        res_path.to_string_lossy()
    ));
    let directory = HSTRING::from(
        helper
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    );

    let hinst = unsafe { ShellExecuteW(None, &verb, &file, &params, &directory, SW_HIDE) };
    if hinst.0 as isize <= 32 {
        return Err(Error::Msg(format!(
            "failed to launch helper (ShellExecuteW runas code {})",
            hinst.0 as isize
        )));
    }
    Ok(())
}

/// Locate bootkeeper-helper.exe next to the current executable.
#[cfg(windows)]
pub fn helper_path() -> Result<PathBuf, Error> {
    // Allow override for dev layouts.
    if let Ok(p) = std::env::var("BOOTKEEPER_HELPER") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Ok(p);
        }
    }
    let exe = std::env::current_exe().map_err(|e| Error::Msg(e.to_string()))?;
    let dir = exe.parent().ok_or_else(|| Error::Msg("no exe dir".into()))?;
    let candidate = dir.join("bootkeeper-helper.exe");
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(Error::Msg(format!(
            "bootkeeper-helper.exe not found next to {} (set BOOTKEEPER_HELPER to override)",
            exe.to_string_lossy()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_dir_respects_env() {
        std::env::set_var("BOOTKEEPER_DATA", "C:\\fake");
        let d = tmp_dir();
        assert!(d.to_string_lossy().ends_with("tmp"));
        std::env::remove_var("BOOTKEEPER_DATA");
    }

    #[test]
    fn non_windows_run_helper_errors() {
        // On Linux this always errors; on Windows it would need a live helper.
        #[cfg(not(windows))]
        {
            let r = run_helper(WriteOp::Disable { item_id: "x".into() });
            assert!(r.is_err());
        }
        #[cfg(windows)]
        {
            let _ = WriteOp::Disable {
                item_id: "x".into(),
            };
        }
    }
}

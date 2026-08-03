//! Windows-only integration tests. These run on real Windows runners (CI)
//! and exercise the actual registry / Task Scheduler / startup folders.

#![cfg(windows)]

use bootkeeper_core::windows::{
    enumerate_all, enumerate_registry_run, enumerate_scheduled_tasks, enumerate_startup_folders,
};
use bootkeeper_core::{enrich, model::Signature};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ};
use winreg::{RegKey, RegValue};

/// Create a throwaway HKCU Run value, disable it via the real ops path, verify
/// the rename to .disabled, then restore/cleanup.
#[test]
fn disable_rename_roundtrip_on_real_registry() {
    const TEST_NAME: &str = "BootKeeperCITest";
    // CI runners may not have HKCU\...\Run yet — create it (no-op if present).
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .expect("create HKCU Run")
        .0;

    // 1. Write a test value (UTF-16LE REG_SZ with a unicode path to catch
    //    encoding regressions too).
    let command = "C:\\Program Files\\BootKeeper\\测试\\app.exe --flag";
    let bytes: Vec<u8> = command
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    run_key
        .set_raw_value(
            TEST_NAME,
            &RegValue { bytes: bytes.into(), vtype: REG_SZ },
        )
        .expect("write test value");

    // 2. Locate the item via the enumerator (id is built from its fields).
    let raw = enumerate_registry_run()
        .into_iter()
        .find(|e| e.name == TEST_NAME)
        .expect("enumerator sees test value");

    // 3. Disable (rename -> .disabled).
    let id = bootkeeper_core::model::StartupItem::build_id(
            raw.category, &raw.location, &raw.name,
        );
        let res = bootkeeper_core::windows::ops::disable(&id).expect("disable succeeds");
    assert!(res.ok, "disable should report ok: {:?}", res.message);

    // 4. Verify renamed.
    let disabled = enumerate_registry_run()
        .into_iter()
        .find(|e| e.name == format!("{TEST_NAME}.disabled"));
    assert!(disabled.is_some(), "disabled value should be enumerated");

    // 5. Cleanup: delete both possible names.
    let _ = run_key.delete_value(&format!("{TEST_NAME}.disabled"));
    let _ = run_key.delete_value(TEST_NAME);
}

/// Verify the launcher data root resolves to a cross-privilege safe location
/// (ProgramData on Windows) — the elevated helper must share it.
#[test]
fn launcher_data_root_is_shared() {
    let root = bootkeeper_core::windows::launcher::data_root();
    let s = root.to_string_lossy().to_lowercase();
    // Elevated helper has a different %APPDATA% (admin profile); ProgramData
    // is the shared, cross-privilege location.
    assert!(
        s.contains("programdata") || std::env::var("BOOTKEEPER_DATA").is_ok(),
        "data root should be ProgramData (or BOOTKEEPER_DATA override): {s}"
    );
}

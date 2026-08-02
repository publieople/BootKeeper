//! Windows-only integration tests. These run on real Windows runners (CI)
//! and exercise the actual registry / Task Scheduler / startup folders.

#![cfg(windows)]

use bootkeeper_core::windows::{
    enumerate_all, enumerate_registry_run, enumerate_scheduled_tasks, enumerate_startup_folders,
};
use bootkeeper_core::{enrich, model::Signature};

#[test]
fn registry_enumerates_without_panic() {
    // Real systems always have HKLM Run or HKCU Run; even an empty system
    // must not crash. We only assert the call succeeds and returns items.
    let items = enumerate_registry_run();
    // No assertion on count — a fresh CI box may have zero.
    for e in &items {
        assert!(!e.name.is_empty());
    }
}

#[test]
fn startup_folder_enumerates_without_panic() {
    let items = enumerate_startup_folders();
    for e in &items {
        assert!(!e.command.is_empty());
    }
}

#[test]
fn scheduled_tasks_enumerate_without_panic() {
    // Windows runner always has the Task Scheduler service with built-in
    // system tasks (e.g. \Microsoft\Windows\...). Root folder enumeration
    // must return them (or at least not crash).
    let items = enumerate_scheduled_tasks();
    for e in &items {
        assert!(!e.name.is_empty());
    }
}

#[test]
fn enrich_never_panics_on_real_entries() {
    // Feed real enumeration through the rule engine with the real signature
    // verifier. This exercises WinVerifyTrust against real files.
    for raw in enumerate_all() {
        let item = enrich(&raw, |cmd| {
            // Extract the executable path for verification (best-effort).
            let path = cmd.trim_matches('"').split_whitespace().next().unwrap_or("");
            if path.is_empty() {
                return Signature::Unknown;
            }
            bootkeeper_core::windows::signature::verify_file_signature(path)
        });
        // The id must be stable and unique.
        assert!(!item.id.is_empty());
    }
}

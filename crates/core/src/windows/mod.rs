//! Windows startup enumeration.
//!
//! Everything here is Windows-only and gated behind #[cfg(windows)] so the
//! crate still compiles (and unit-tests run) on non-Windows hosts.

pub mod launcher;
#[cfg(windows)]
pub mod ops;
#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
pub mod scheduled_task;
#[cfg(windows)]
pub mod signature;
#[cfg(windows)]
pub mod startup_folder;

pub use launcher::{run_helper, run_helper_with_snapshot_dir, snapshot_dir, tmp_dir};
#[cfg(windows)]
pub use ops::{disable, enable, remove, restore};
#[cfg(windows)]
pub use registry::enumerate_registry_run;
#[cfg(windows)]
pub use scheduled_task::enumerate_scheduled_tasks;
#[cfg(windows)]
pub use startup_folder::enumerate_startup_folders;

/// Enumerate all supported startup categories into raw entries.
/// Order: registry, startup folders, scheduled tasks.
#[cfg(windows)]
pub fn enumerate_all() -> Vec<crate::model::RawEntry> {
    let mut out = Vec::new();
    out.extend(enumerate_registry_run());
    out.extend(enumerate_startup_folders());
    out.extend(enumerate_scheduled_tasks());
    out
}

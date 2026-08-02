//! bootkeeper CLI — read chain (M1) + write chain (M2).
//!
//! Read commands output JSON directly. Write commands build a request file,
//! launch the elevated helper (ShellExecuteW runas), wait for the result
//! file, and print the outcome. The helper shows the confirmation dialog;
//! without a user "Yes" nothing is executed.

use std::path::PathBuf;

use bootkeeper_core::model::{Category, Signature};
use bootkeeper_core::{enrich, rules, snapshot::SnapshotStore, StartupItem};
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "bootkeeper", version, about = "Windows autostart manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List startup items, optionally filtered by category.
    List {
        /// registry_run | startup_folder | scheduled_task
        #[arg(long)]
        category: Option<String>,
    },
    /// Show one item by id.
    Get { id: String },
    /// AI-friendly analysis: items with risk + reasons (rules decide risk).
    Analyze {
        #[arg(long)]
        category: Option<String>,
    },
    /// Disable a startup item (renames to .disabled). Requires elevation + confirmation.
    Disable { id: String },
    /// Enable a startup item (renames back). Requires elevation + confirmation.
    Enable { id: String },
    /// Remove a startup item (after snapshot backup). Requires elevation + confirmation.
    Remove { id: String },
    /// Restore an item from a snapshot. Requires elevation + confirmation.
    Restore { snapshot_id: String, item_id: String },
    /// Snapshot store operations.
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
}

#[derive(Subcommand)]
enum SnapshotAction {
    /// List snapshots (metadata only).
    List,
    /// Show full snapshot content.
    Show { id: String },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::List { category } => cmd_list(category.as_deref()),
        Commands::Get { id } => cmd_get(&id),
        Commands::Analyze { category } => cmd_analyze(category.as_deref()),
        Commands::Disable { id } => cmd_write(&id, "disable", None),
        Commands::Enable { id } => cmd_write(&id, "enable", None),
        Commands::Remove { id } => cmd_write(&id, "remove", None),
        Commands::Restore { snapshot_id, item_id } => {
            cmd_write(&item_id, "restore", Some(&snapshot_id))
        }
        Commands::Snapshot { action } => cmd_snapshot(action),
    };
    match result {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

/// Where snapshots live. M2 will allow overriding via env/config.
fn data_root() -> PathBuf {
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
}

fn snapshot_dir() -> PathBuf {
    data_root().join("snapshots")
}

fn category_from_str(s: &str) -> Option<Category> {
    match s {
        "registry_run" => Some(Category::RegistryRun),
        "startup_folder" => Some(Category::StartupFolder),
        "scheduled_task" => Some(Category::ScheduledTask),
        _ => None,
    }
}

/// Signature verifier: real on Windows, Unknown elsewhere (for tests).
fn verifier() -> impl Fn(&str) -> Signature {
    #[cfg(windows)]
    {
        |cmd| {
            let path = cmd
                .trim_matches('"')
                .split_whitespace()
                .next()
                .unwrap_or("");
            if path.is_empty() {
                Signature::Unknown
            } else {
                bootkeeper_core::windows::signature::verify_file_signature(path)
            }
        }
    }
    #[cfg(not(windows))]
    {
        |_| Signature::Unknown
    }
}

fn all_items() -> Vec<StartupItem> {
    #[cfg(windows)]
    let raws = bootkeeper_core::windows::enumerate_all();
    #[cfg(not(windows))]
    let raws: Vec<bootkeeper_core::RawEntry> = Vec::new();

    let v = verifier();
    raws.iter().map(|r| enrich(r, &v)).collect()
}

fn cmd_list(category: Option<&str>) -> Result<String, String> {
    let cat = match category {
        Some(s) => Some(category_from_str(s).ok_or_else(|| format!("unknown category: {s}"))?),
        None => None,
    };
    let items: Vec<StartupItem> = all_items()
        .into_iter()
        .filter(|i| cat.is_none_or(|c| i.category == c))
        .collect();
    serde_json::to_string_pretty(&items).map_err(|e| e.to_string())
}

fn cmd_get(id: &str) -> Result<String, String> {
    let items = all_items();
    let item = items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("item not found: {id}"))?;
    serde_json::to_string_pretty(item).map_err(|e| e.to_string())
}

fn cmd_analyze(category: Option<&str>) -> Result<String, String> {
    let cat = match category {
        Some(s) => Some(category_from_str(s).ok_or_else(|| format!("unknown category: {s}"))?),
        None => None,
    };
    #[cfg(windows)]
    let raws = bootkeeper_core::windows::enumerate_all();
    #[cfg(not(windows))]
    let raws: Vec<bootkeeper_core::RawEntry> = Vec::new();

    let v = verifier();
    let analyzed: Vec<serde_json::Value> = raws
        .iter()
        .filter(|r| cat.is_none_or(|c| r.category == c))
        .map(|r| {
            let sig = v(&r.command);
            let rule = rules::evaluate(r, sig, None);
            json!({
                "id": StartupItem::build_id(r.category, &r.location, &r.name),
                "category": r.category.as_str(),
                "name": r.name,
                "command": r.command,
                "location": r.location,
                "signature": format!("{:?}", sig).to_lowercase(),
                "risk": format!("{:?}", rule.risk).to_lowercase(),
                "reasons": rule.reasons,
            })
        })
        .collect();
    serde_json::to_string_pretty(&analyzed).map_err(|e| e.to_string())
}

/// Write chain: build request, launch elevated helper, wait, read result.
fn cmd_write(id: &str, op_name: &str, restore_snapshot_id: Option<&str>) -> Result<String, String> {
    #[cfg(not(windows))]
    {
        let _ = (id, op_name, restore_snapshot_id);
        return Err("write commands are Windows-only (bootkeeper-helper)".into());
    }

    #[cfg(windows)]
    {

        let dir = data_root().join("tmp");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let req_path = dir.join(format!("{op_name}-{}.json", std::process::id()));
        let res_path = dir.join(format!("result-{}.json", std::process::id()));

        let op = match op_name {
            "disable" => bootkeeper_core::WriteOp::Disable {
                item_id: id.to_string(),
            },
            "enable" => bootkeeper_core::WriteOp::Enable {
                item_id: id.to_string(),
            },
            "remove" => bootkeeper_core::WriteOp::Remove {
                item_id: id.to_string(),
            },
            "restore" => {
                let snap_id = restore_snapshot_id
                    .ok_or_else(|| "restore requires --snapshot-id".to_string())?;
                let store = bootkeeper_core::SnapshotStore::new(snapshot_dir());
                let snap = store.get(snap_id).map_err(|e| e.to_string())?;
                let entry = snap
                    .entries
                    .iter()
                    .find(|e| e.item_id == id)
                    .ok_or_else(|| format!("item {id} not in snapshot {snap_id}"))?
                    .clone();
                // restore 走独立路径：直接调 helper 传 snapshot entry
                return cmd_restore(entry, snap_id);
            }
            _ => return Err(format!("unknown write op: {op_name}")),
        };
        let request = json!({
            "operation": op,
            "snapshot_dir": snapshot_dir().to_string_lossy(),
        });
        std::fs::write(&req_path, serde_json::to_vec_pretty(&request).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

        // Launch the helper elevated via ShellExecuteW runas.
        launch_helper_elevated(&req_path, &res_path)?;

        // Wait for the result file.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if res_path.exists() {
                break;
            }
            if std::time::Instant::now() > deadline {
                return Err("timed out waiting for helper (user may not have confirmed)".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }

        let bytes = std::fs::read(&res_path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&req_path);
        let _ = std::fs::remove_file(&res_path);
        let result: bootkeeper_core::WriteResult =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

/// Launch the helper elevated via ShellExecuteW "runas" (UAC prompt).
/// Blocks until the elevated process exits.
#[cfg(windows)]
fn launch_helper_elevated(
    req_path: &std::path::Path,
    res_path: &std::path::Path,
) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::Win32::UI::Shell::ShellExecuteW;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or_else(|| "no exe dir".to_string())?;
    let helper = dir.join("bootkeeper-helper.exe");

    let verb = HSTRING::from("runas");
    let file = HSTRING::from(helper.to_string_lossy().as_ref());
    let params = HSTRING::from(format!(
        "\"{}\" \"{}\"",
        req_path.to_string_lossy(),
        res_path.to_string_lossy()
    ));
    let directory = HSTRING::from(dir.to_string_lossy().as_ref());

    let hinst = unsafe {
        ShellExecuteW(
            None,
            &verb,
            &file,
            &params,
            &directory,
            SW_HIDE,
        )
    };
    // HINSTANCE <= 32 means an error (see ShellExecute docs).
    if hinst.0 as isize <= 32 {
        return Err(format!(
            "failed to launch helper (ShellExecuteW runas code {})",
            hinst.0 as isize
        ));
    }

    // Wait for the helper to finish writing the result. Poll the file
    // instead of the process handle — simpler and avoids handle juggling.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !res_path.exists() {
        if std::time::Instant::now() > deadline {
            return Err("timed out waiting for helper".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Ok(())
}

/// Restore chain: build a request with the snapshot entry, launch helper.
fn cmd_restore(entry: bootkeeper_core::SnapshotEntry, snap_id: &str) -> Result<String, String> {
    #[cfg(not(windows))]
    {
        let _ = (entry, snap_id);
        return Err("restore is Windows-only (bootkeeper-helper)".into());
    }

    #[cfg(windows)]
    {
        let dir = data_root().join("tmp");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let req_path = dir.join(format!("restore-{}.json", std::process::id()));
        let res_path = dir.join(format!("result-{}.json", std::process::id()));

        let request = json!({
            "restore": entry,
            "snapshot_id": snap_id,
            "snapshot_dir": snapshot_dir().to_string_lossy(),
        });
        std::fs::write(
            &req_path,
            serde_json::to_vec_pretty(&request).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        launch_helper_elevated(&req_path, &res_path)?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if res_path.exists() {
                break;
            }
            if std::time::Instant::now() > deadline {
                return Err("timed out waiting for helper".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }

        let bytes = std::fs::read(&res_path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&req_path);
        let _ = std::fs::remove_file(&res_path);
        let result: bootkeeper_core::WriteResult =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

fn cmd_snapshot(action: SnapshotAction) -> Result<String, String> {
    let store = SnapshotStore::new(snapshot_dir());
    match action {
        SnapshotAction::List => {
            let snaps = store.list().map_err(|e| e.to_string())?;
            let meta: Vec<serde_json::Value> = snaps
                .iter()
                .map(bootkeeper_core::snapshot::summarize)
                .collect();
            serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())
        }
        SnapshotAction::Show { id } => {
            let snap = store.get(&id).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_parse() {
        assert_eq!(
            category_from_str("registry_run"),
            Some(Category::RegistryRun)
        );
        assert_eq!(category_from_str("bogus"), None);
    }

    #[test]
    fn list_outputs_valid_json_on_any_platform() {
        let out = cmd_list(None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
    }

    #[test]
    fn analyze_outputs_expected_fields() {
        let out = cmd_analyze(None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        for item in v.as_array().unwrap() {
            assert!(item.get("id").is_some());
            assert!(item.get("risk").is_some());
            assert!(item.get("reasons").is_some());
        }
    }

    #[test]
    fn get_missing_id_errors() {
        assert!(cmd_get("definitely:missing:id").is_err());
    }
}

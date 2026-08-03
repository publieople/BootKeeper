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
    /// Add a startup entry. Requires elevation + confirmation.
    Add {
        /// registry_run | startup_folder
        category: String,
        name: String,
        command: String,
        /// For registry_run: HKCU\...\Run or HKLM\...\Run. For startup_folder: user_startup.
        #[arg(long, default_value = "HKCU\\...\\Run")]
        location: String,
    },
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
        Commands::Add { category, name, command, location } => {
            cmd_add(&category, &name, &command, &location)
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

/// Where snapshots live. Uses the shared launcher data root (ProgramData on
/// Windows so elevated helper and non-elevated CLI/GUI agree on paths).
fn data_root() -> PathBuf {
    bootkeeper_core::windows::launcher::data_root()
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
            return cmd_restore(entry, snap_id);
        }
        _ => return Err(format!("unknown write op: {op_name}")),
    };

    #[cfg(windows)]
    {
        let result = bootkeeper_core::windows::run_helper(op).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        Err("write commands are Windows-only (bootkeeper-helper)".into())
    }
}

/// Restore chain: shared launcher with a snapshot entry.
fn cmd_restore(entry: bootkeeper_core::SnapshotEntry, snap_id: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        let result = bootkeeper_core::windows::launcher::run_restore(
            entry,
            snap_id,
            snapshot_dir(),
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (entry, snap_id);
        Err("restore is Windows-only (bootkeeper-helper)".into())
    }
}

/// Add chain: build WriteOp::Add, run through helper.
fn cmd_add(category: &str, name: &str, command: &str, location: &str) -> Result<String, String> {
    let cat = match category {
        "registry_run" => bootkeeper_core::Category::RegistryRun,
        "startup_folder" => bootkeeper_core::Category::StartupFolder,
        "scheduled_task" => bootkeeper_core::Category::ScheduledTask,
        _ => return Err(format!("unknown category: {category}")),
    };
    let op = bootkeeper_core::WriteOp::Add(bootkeeper_core::WriteOpAdd {
        category: cat,
        name: name.to_string(),
        command: command.to_string(),
        location: location.to_string(),
    });
    #[cfg(windows)]
    {
        let result = bootkeeper_core::windows::run_helper(op).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        Err("write commands are Windows-only (bootkeeper-helper)".into())
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

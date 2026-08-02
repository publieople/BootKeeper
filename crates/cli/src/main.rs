//! bootkeeper CLI — read chain (M1).
//!
//! All commands output JSON: AI agents parse this directly. Write commands
//! arrive in M2 with the helper confirmation flow.

use std::path::PathBuf;

use bootkeeper_core::model::{Category, Signature};
use bootkeeper_core::{enrich, rules, snapshot::SnapshotStore, StartupItem};
use clap::{Parser, Subcommand};

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
    Get {
        id: String,
    },
    /// AI-friendly analysis: items with risk + reasons (rules decide risk).
    Analyze {
        #[arg(long)]
        category: Option<String>,
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
fn snapshot_dir() -> PathBuf {
    std::env::var_os("BOOTKEEPER_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(windows)]
            let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
            #[cfg(not(windows))]
            let base = PathBuf::from(".");
            base.join("BootKeeper").join("snapshots")
        })
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
            let path = cmd.trim_matches('"').split_whitespace().next().unwrap_or("");
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
    let cat = category.map(|s| category_from_str(s).ok_or_else(|| format!("unknown category: {s}")));
    let cat = match cat {
        Some(Ok(c)) => Some(c),
        Some(Err(e)) => return Err(e),
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
            serde_json::json!({
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

fn cmd_snapshot(action: SnapshotAction) -> Result<String, String> {
    let store = SnapshotStore::new(snapshot_dir());
    match action {
        SnapshotAction::List => {
            let snaps = store.list().map_err(|e| e.to_string())?;
            let meta: Vec<serde_json::Value> =
                snaps.iter().map(bootkeeper_core::snapshot::summarize).collect();
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
        assert_eq!(category_from_str("registry_run"), Some(Category::RegistryRun));
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

//! BootKeeper MCP server.
//!
//! Exposes BootKeeper's CLI surface as MCP tools so protocol-standard agents
//! (Claude Desktop, Cursor, Codex, etc.) can manage startup items. All write
//! tools go through the elevated helper — the user always sees the
//! confirmation dialog; the AI cannot bypass it.

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// MCP server state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BootKeeperServer {
    tool_router: ToolRouter<Self>,
}

impl BootKeeperServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for BootKeeperServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BootKeeperServer {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CategoryFilter {
    /// registry_run | startup_folder | scheduled_task
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct IdParam {
    /// Item id (category:location:name), from list_items output.
    pub id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AddParam {
    /// registry_run | startup_folder (scheduled_task not supported in v1)
    pub category: String,
    /// Entry name (value name / filename)
    pub name: String,
    /// Command (registry) or source file path (startup folder)
    pub command: String,
    /// For registry_run: HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run or HKLM\...\Run. Default HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run.
    #[serde(default = "default_location")]
    pub location: String,
}

fn default_location() -> String {
    "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run".into()
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RestoreParam {
    /// Snapshot id, from list_snapshots output.
    pub snapshot_id: String,
    /// Item id inside the snapshot (optional; first entry if empty).
    #[serde(default)]
    pub item_id: Option<String>,
}

#[tool_router(router = tool_router)]
impl BootKeeperServer {
    /// List startup items (registry Run, startup folders, scheduled tasks).
    #[tool(name = "list_items", description = "List Windows startup items. Returns JSON array of items with id, category, name, command, location, signature, risk, enabled.")]
    pub async fn list_items(&self, p: Parameters<CategoryFilter>) -> String {
        let cat = p.0.category;
        #[cfg(windows)]
        {
            use bootkeeper_core::model::Signature;
            use bootkeeper_core::windows::signature::verify_file_signature;
            use bootkeeper_core::{enrich, windows};

            let verifier = |cmd: &str| {
                let path = cmd.trim_matches('"').split_whitespace().next().unwrap_or("");
                if path.is_empty() {
                    Signature::Unknown
                } else {
                    verify_file_signature(path)
                }
            };
            let items: Vec<serde_json::Value> = windows::enumerate_all()
                .iter()
                .filter(|r| cat.as_deref().is_none_or(|c| r.category.as_str() == c))
                .map(|r| serde_json::to_value(enrich(r, verifier)).unwrap_or_default())
                .collect();
            serde_json::to_string(&items).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
        #[cfg(not(windows))]
        {
            let _ = cat;
            "[]".to_string()
        }
    }

    /// Analyze startup items: risk + reasons per item (software-ruled).
    #[tool(name = "analyze_items", description = "Analyze startup items with rule-engine risk and reasons. Returns JSON array with id, name, risk, reasons.")]
    pub async fn analyze_items(&self, p: Parameters<CategoryFilter>) -> String {
        let cat = p.0.category;
        #[cfg(windows)]
        {
            use bootkeeper_core::model::Signature;
            use bootkeeper_core::rules;
            use bootkeeper_core::windows::signature::verify_file_signature;

            let verifier = |cmd: &str| {
                let path = cmd.trim_matches('"').split_whitespace().next().unwrap_or("");
                if path.is_empty() {
                    Signature::Unknown
                } else {
                    verify_file_signature(path)
                }
            };
            let out: Vec<serde_json::Value> = bootkeeper_core::windows::enumerate_all()
                .iter()
                .filter(|r| cat.as_deref().is_none_or(|c| r.category.as_str() == c))
                .map(|r| {
                    let sig = verifier(&r.command);
                    let rule = rules::evaluate(r, sig, None);
                    serde_json::json!({
                        "id": bootkeeper_core::StartupItem::build_id(r.category, &r.location, &r.name),
                        "name": r.name,
                        "category": r.category.as_str(),
                        "command": r.command,
                        "signature": format!("{:?}", sig).to_lowercase(),
                        "risk": format!("{:?}", rule.risk).to_lowercase(),
                        "reasons": rule.reasons,
                    })
                })
                .collect();
            serde_json::to_string(&out).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
        #[cfg(not(windows))]
        {
            let _ = cat;
            "[]".to_string()
        }
    }

    /// Disable a startup item (renames to .disabled). Shows a confirmation dialog.
    #[tool(name = "disable_item", description = "Disable a startup item (rename to .disabled). Launches an elevated helper that shows a UAC + confirmation dialog; the user MUST approve, otherwise nothing happens.")]
    pub async fn disable_item(&self, p: Parameters<IdParam>) -> String {
        run_write(bootkeeper_core::WriteOp::Disable { item_id: p.0.id }).await
    }

    /// Enable a startup item (renames back). Shows a confirmation dialog.
    #[tool(name = "enable_item", description = "Enable a startup item (rename back from .disabled). Launches an elevated helper that shows a UAC + confirmation dialog; the user MUST approve.")]
    pub async fn enable_item(&self, p: Parameters<IdParam>) -> String {
        run_write(bootkeeper_core::WriteOp::Enable { item_id: p.0.id }).await
    }

    /// Remove a startup item (snapshot created first). Shows a confirmation dialog.
    #[tool(name = "remove_item", description = "Remove a startup item entirely. A snapshot is created first so it can be restored. Launches an elevated helper with a UAC + confirmation dialog; the user MUST approve.")]
    pub async fn remove_item(&self, p: Parameters<IdParam>) -> String {
        run_write(bootkeeper_core::WriteOp::Remove { item_id: p.0.id }).await
    }

    /// Add a startup entry (registry or startup folder). Shows a confirmation dialog.
    #[tool(name = "add_item", description = "Add a new startup entry (registry Run value or startup folder file). Launches an elevated helper with a UAC + confirmation dialog; the user MUST approve.")]
    pub async fn add_item(&self, p: Parameters<AddParam>) -> String {
        run_add(p.0).await
    }

    /// List snapshots (undo history, kept 7 days).
    #[tool(name = "list_snapshots", description = "List snapshot metadata (id, time, operation, entry count) for undo history.")]
    pub async fn list_snapshots(&self) -> String {
        let store = bootkeeper_core::SnapshotStore::new(
            bootkeeper_core::windows::launcher::snapshot_dir(),
        );
        match store.list() {
            Ok(snaps) => {
                let meta: Vec<serde_json::Value> = snaps
                    .iter()
                    .map(bootkeeper_core::snapshot::summarize)
                    .collect();
                serde_json::to_string(&meta).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
            }
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        }
    }

    /// Restore an item from a snapshot. Shows a confirmation dialog.
    #[tool(name = "restore_item", description = "Restore an item from a snapshot (undo a disable/enable/remove). Launches an elevated helper with a UAC + confirmation dialog; the user MUST approve.")]
    pub async fn restore_item(&self, p: Parameters<RestoreParam>) -> String {
        let store = bootkeeper_core::SnapshotStore::new(
            bootkeeper_core::windows::launcher::snapshot_dir(),
        );
        let snap = match store.get(&p.0.snapshot_id) {
            Ok(s) => s,
            Err(e) => return format!("{{\"error\":\"{e}\"}}"),
        };
        let entry = match &p.0.item_id {
            Some(id) => match snap.entries.iter().find(|e| &e.item_id == id) {
                Some(e) => e.clone(),
                None => {
                    return format!("{{\"error\":\"item {id} not in snapshot\"}}");
                }
            },
            None => match snap.entries.first() {
                Some(e) => e.clone(),
                None => return "{\"error\":\"snapshot is empty\"}".into(),
            },
        };
        #[cfg(windows)]
        {
            match bootkeeper_core::windows::launcher::run_restore(
                entry,
                &p.0.snapshot_id,
                bootkeeper_core::windows::launcher::snapshot_dir(),
            ) {
                Ok(r) => serde_json::to_string(&r).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                Err(e) => format!("{{\"error\":\"{e}\"}}"),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = entry;
            "{\"error\":\"restore requires Windows\"}".into()
        }
    }
}

/// Shared add path through the elevated helper.
async fn run_add(p: AddParam) -> String {
    let cat = match p.category.as_str() {
        "registry_run" => bootkeeper_core::Category::RegistryRun,
        "startup_folder" => bootkeeper_core::Category::StartupFolder,
        "scheduled_task" => bootkeeper_core::Category::ScheduledTask,
        other => return format!("{{\"error\":\"unknown category: {other}\"}}"),
    };
    let op = bootkeeper_core::WriteOp::Add(bootkeeper_core::WriteOpAdd {
        category: cat,
        name: p.name,
        command: p.command,
        location: p.location,
    });
    run_write(op).await
}

/// Shared write path through the elevated helper.
async fn run_write(op: bootkeeper_core::WriteOp) -> String {
    #[cfg(windows)]
    {
        match bootkeeper_core::windows::run_helper(op) {
            Ok(r) => serde_json::to_string(&r).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        "{\"error\":\"write operations require Windows\"}".into()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = BootKeeperServer::new();
    let running = server.serve(rmcp::transport::io::stdio()).await?;
    // Keep the service alive until the connection closes (stdio EOF).
    running.waiting().await?;
    Ok(())
}

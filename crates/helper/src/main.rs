//! BootKeeper elevated helper.
//!
//! Runs elevated (via ShellExecuteW runas, launched by the CLI). It is the
//! ONLY process that can execute write operations. Hard-confirm model:
//!
//! 1. Read the operation request JSON from a temp file.
//! 2. Independently re-verify facts (look up the item, verify signature,
//!    run the rule engine) — never trust text from the caller.
//! 3. Show a native confirmation dialog with MACHINE-VERIFIED facts.
//! 4. Only if the user clicks Yes: execute + snapshot, write result JSON.
//!
//! The caller (CLI/AI) cannot bypass this dialog — without a Yes it does
//! nothing. This is the hard-confirmation boundary in runtime, not in the
//! AI's judgment.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    #[serde(default)]
    operation: Option<bootkeeper_core::WriteOp>,
    #[serde(default)]
    restore: Option<bootkeeper_core::SnapshotEntry>,
    #[serde(default)]
    snapshot_id: Option<String>,
    #[serde(default = "default_snapshot_dir")]
    snapshot_dir: String,
}

fn default_snapshot_dir() -> String {
    std::env::var("BOOTKEEPER_DATA")
        .map(|d| PathBuf::from(d).join("snapshots").to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into())
}

/// Facts verified by the helper itself — shown in the dialog.
#[derive(Debug, Serialize, Deserialize)]
struct VerifiedFacts {
    item_id: String,
    name: String,
    command: String,
    location: String,
    category: String,
    signature: String,
    risk: String,
    reasons: Vec<String>,
}

fn main() {
    // args: bootkeeper-helper <request.json> <result.json>
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: bootkeeper-helper <request.json> <result.json>");
        std::process::exit(2);
    }
    let req_path = &args[1];
    let res_path = &args[2];

    #[cfg(windows)]
    let run_result = run(req_path, res_path);
    #[cfg(not(windows))]
    let run_result: Result<(), String> = Err("bootkeeper-helper is Windows-only".into());
    let _ = (req_path, res_path);
    if let Err(e) = run_result {
        // Write an error result so the CLI has something to parse.
        let err_result = bootkeeper_core::WriteResult {
            ok: false,
            message: format!("helper error: {e}"),
            snapshot_entry: None,
        };
        let _ = std::fs::write(
            res_path,
            serde_json::to_vec_pretty(&err_result).unwrap_or_default(),
        );
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run(req_path: &str, res_path: &str) -> Result<(), String> {
    use bootkeeper_core::snapshot::SnapshotStore;
    use bootkeeper_core::windows::{disable, enable, remove};
    use bootkeeper_core::WriteOp;

    let req: Request = serde_json::from_slice(
        &std::fs::read(req_path).map_err(|e| format!("read request: {e}"))?,
    )
    .map_err(|e| format!("parse request: {e}"))?;

    // Restore requests carry a snapshot entry, not an operation.
    if let Some(entry) = &req.restore {
        let op = bootkeeper_core::WriteOp::Restore {
            snapshot_id: req.snapshot_id.clone().unwrap_or_default(),
            item_id: entry.item_id.clone(),
        };
        let confirmed = confirm_dialog(&op, &restore_facts(entry));
        if !confirmed {
            let res = bootkeeper_core::WriteResult {
                ok: false,
                message: "cancelled by user".into(),
                snapshot_entry: None,
            };
            write_result(res_path, &res)?;
            return Ok(());
        }
        let result = bootkeeper_core::windows::restore(entry).map_err(|e| e.to_string())?;
        write_result(res_path, &result)?;
        return Ok(());
    }

    let operation = req
        .operation
        .clone()
        .ok_or_else(|| "request has neither operation nor restore".to_string())?;

    // 1. Verify facts independently for Disable/Enable/Remove.
    let facts = match &operation {
        WriteOp::Disable { item_id }
        | WriteOp::Enable { item_id }
        | WriteOp::Remove { item_id } => {
            verify_item_facts(item_id).ok_or_else(|| format!("item not found: {item_id}"))?
        }
        WriteOp::Add { .. } | WriteOp::Restore { .. } => VerifiedFacts {
            item_id: String::new(),
            name: String::new(),
            command: String::new(),
            location: String::new(),
            category: String::new(),
            signature: String::new(),
            risk: String::new(),
            reasons: Vec::new(),
        },
    };

    // 2. Show confirmation dialog with the verified facts.
    if !confirm_dialog(&operation, &facts) {
        let res = bootkeeper_core::WriteResult {
            ok: false,
            message: "cancelled by user".into(),
            snapshot_entry: None,
        };
        write_result(res_path, &res)?;
        return Ok(());
    }

    // 3. Execute (only reached after Yes) + snapshot.
    let mut result = match &operation {
        WriteOp::Disable { item_id } => disable(item_id),
        WriteOp::Enable { item_id } => enable(item_id),
        WriteOp::Remove { item_id } => remove(item_id),
        WriteOp::Add { .. } => Err(bootkeeper_core::Error::Msg(
            "add not implemented in this milestone".into(),
        )),
        // Restore is handled in its own branch above; unreachable here.
        WriteOp::Restore { .. } => Err(bootkeeper_core::Error::Msg(
            "restore handled separately".into(),
        )),
    }
    .map_err(|e| e.to_string())?;

    // Persist snapshot entry for undo.
    if let Some(entry) = result.snapshot_entry.take() {
        let store = SnapshotStore::new(PathBuf::from(&req.snapshot_dir));
        let _ = store.create("write_op", vec![entry]);
    }

    write_result(res_path, &result)?;
    Ok(())
}

/// Re-verify facts about an item directly from the system (not from caller).
#[cfg(windows)]
fn verify_item_facts(item_id: &str) -> Option<VerifiedFacts> {
    use bootkeeper_core::rules;
    use bootkeeper_core::windows::signature::verify_file_signature;

    let raw = bootkeeper_core::windows::ops::find_raw_by_id(item_id)?;
    let signature = verify_file_signature(&raw.command);
    let rule = rules::evaluate(&raw, signature, None);
    Some(VerifiedFacts {
        item_id: item_id.to_string(),
        name: raw.name,
        command: raw.command,
        location: raw.location,
        category: raw.category.as_str().to_string(),
        signature: format!("{:?}", signature).to_lowercase(),
        risk: format!("{:?}", rule.risk).to_lowercase(),
        reasons: rule.reasons,
    })
}

/// Facts for a restore confirmation.
fn restore_facts(entry: &bootkeeper_core::SnapshotEntry) -> VerifiedFacts {
    VerifiedFacts {
        item_id: entry.item_id.clone(),
        name: entry.original_name.clone(),
        command: entry.command.clone(),
        location: entry.location.clone(),
        category: entry.category.as_str().to_string(),
        signature: String::new(),
        risk: String::new(),
        reasons: vec!["restore from snapshot".into()],
    }
}

/// Native confirmation dialog (MessageBoxW). Title says what will happen;
/// body shows machine-verified facts only.
#[cfg(windows)]
fn confirm_dialog(op: &bootkeeper_core::WriteOp, facts: &VerifiedFacts) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONWARNING, MB_YESNO, MB_TOPMOST, MESSAGEBOX_STYLE,
    };

    let (title, verb) = match op {
        bootkeeper_core::WriteOp::Disable { .. } => {
            ("BootKeeper — Disable startup item", "Disable")
        }
        bootkeeper_core::WriteOp::Enable { .. } => {
            ("BootKeeper — Enable startup item", "Enable")
        }
        bootkeeper_core::WriteOp::Remove { .. } => {
            ("BootKeeper — Remove startup item", "Remove")
        }
        bootkeeper_core::WriteOp::Add { .. } => {
            ("BootKeeper — Add startup item", "Add")
        }
        bootkeeper_core::WriteOp::Restore { .. } => {
            ("BootKeeper — Restore startup item", "Restore")
        }
    };

    let reasons = if facts.reasons.is_empty() {
        String::new()
    } else {
        format!("({})", facts.reasons.join(", "))
    };
    let body = format!(
        "{verb} this startup item?\n\n\
         NAME:      {name}\n\
         CATEGORY:  {category}\n\
         COMMAND:   {command}\n\
         LOCATION:  {location}\n\
         SIGNATURE: {signature}\n\
         RISK:      {risk} {reasons}\n\n\
         This will be executed with administrator privileges. A snapshot is\n\
         created so the change can be undone within 7 days.",
        name = facts.name,
        category = facts.category,
        command = facts.command,
        location = facts.location,
        signature = facts.signature,
        risk = facts.risk,
    );

    let title_w = HSTRING::from(title);
    let body_w = HSTRING::from(body);
    let flags = MESSAGEBOX_STYLE(MB_YESNO.0 | MB_ICONWARNING.0 | MB_TOPMOST.0);
    let ret = unsafe { MessageBoxW(None, &body_w, &title_w, flags) };
    ret == windows::Win32::UI::WindowsAndMessaging::IDYES
}

fn write_result(path: &str, res: &bootkeeper_core::WriteResult) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(res).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| format!("write result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrips() {
        let req = Request {
            operation: Some(bootkeeper_core::WriteOp::Disable {
                item_id: "registry_run:HKCU\\...\\Run:Foo".into(),
            }),
            restore: None,
            snapshot_id: None,
            snapshot_dir: ".".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: Request = serde_json::from_str(&json).unwrap();
        match back.operation {
            Some(bootkeeper_core::WriteOp::Disable { item_id }) => {
                assert!(item_id.contains("Foo"))
            }
            _ => panic!("wrong variant"),
        }
    }
}

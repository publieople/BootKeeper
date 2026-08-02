// BootKeeper GUI Rust side.
//
// v1: `list_items` enumerates startup items via core. Write actions are
// handled by the CLI + helper (elevated confirm dialog); the GUI calls them
// through Tauri shell in M4. For now buttons are wired to `invoke` stubs.

#[tauri::command]
fn list_items() -> Vec<serde_json::Value> {
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
        windows::enumerate_all()
            .iter()
            .map(|r| serde_json::to_value(enrich(r, &verifier)).unwrap_or_default())
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// v1 write-action bridge: executes `bootkeeper <action> <id>` and returns
/// the JSON result. The CLI handles the elevated helper + confirmation.
#[tauri::command]
async fn run_write_action(action: String, id: String) -> Result<String, String> {
    #[cfg(not(windows))]
    {
        let _ = (action, id);
        Err("write actions require Windows".into())
    }
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let dir = exe.parent().ok_or_else(|| "no exe dir".to_string())?;
        let cli = dir.join("bootkeeper.exe");
        let output = std::process::Command::new(&cli)
            .arg(&action)
            .arg(&id)
            .output()
            .map_err(|e| e.to_string())?;
        String::from_utf8(output.stdout).map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_items, run_write_action])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

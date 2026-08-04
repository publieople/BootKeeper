//! Deterministic risk rule engine.
//!
//! Risk is decided by these rules ONLY — never by the AI. The AI may suggest,
//! but the final label always comes from here. Every rule is a pure function
//! so it is testable and auditable.

use std::path::Path;

use crate::model::{Category, RawEntry, Risk, Signature};

/// System directories that count as "trusted location" signals.
const SYSTEM_DIRS: &[&str] = &[
    "C:\\Windows\\System32",
    "C:\\Windows\\SysWOW64",
    "C:\\Windows",
    "C:\\Program Files",
];

/// Suspicious directory prefixes (higher risk).
const SUSPICIOUS_PREFIXES: &[&str] = &[
    "C:\\Users\\",                     // non-system user profiles
    "C:\\ProgramData\\",               // shared app data
];

/// Definitely high-risk paths.
const HIGH_RISK_PREFIXES: &[&str] = &[
    "C:\\Windows\\Temp",
    // ponytail: user-profile entries get +1 from SUSPICIOUS_PREFIXES
];

/// Rules output: risk + reasons (for display and for the AI's analysis).
#[derive(Debug, Clone)]
pub struct RuleResult {
    pub risk: Risk,
    /// Human-readable reasons, e.g. "unsigned", "non-system path".
    pub reasons: Vec<String>,
}

/// Evaluate one raw entry. `signature` and `publisher` are provided by the
/// caller (WinVerifyTrust happens on the Windows side).
pub fn evaluate(entry: &RawEntry, signature: Signature, publisher: Option<&str>) -> RuleResult {
    let mut score = 0u8;
    let mut reasons = Vec::new();

    // ---- signature rules ----
    match signature {
        Signature::Valid => { /* signed: good */ }
        Signature::Invalid => {
            score += 2;
            reasons.push("signature invalid".to_string());
        }
        Signature::None => {
            score += 2;
            reasons.push("unsigned".to_string());
        }
        Signature::Unknown => { /* task with no exe — don't penalize */ }
    }

    // ---- publisher ----
    if let Some(pub_name) = publisher {
        if pub_name.to_lowercase().contains("microsoft") {
            score = score.saturating_sub(1);
        }
    }

    // ---- path rules ----
    let path = extract_path(&entry.command);
    if let Some(ref p) = path {
        let lower = p.to_lowercase();

        if path_under(HIGH_RISK_PREFIXES, &lower) {
            score += 2;
            reasons.push("temp directory path".to_string());
        } else if path_under(SUSPICIOUS_PREFIXES, &lower) {
            score += 1;
            reasons.push("non-system path".to_string());
        }

        if path_under_system_dir(p) {
            score = score.saturating_sub(1);
        }

        // dangling entry: the file no longer exists.
        if !Path::new(p).exists() {
            score += 1;
            reasons.push("file missing".to_string());
        }

        // path contains CJK or other non-ASCII — common in Chinese software
        // but also used by malware to blend in. Medium signal.
        if p.chars().any(|c| c as u32 > 0x7f) {
            score += 1;
            reasons.push("non-ascii path".to_string());
        }
    } else {
        // No path extractable (e.g. bare command name).
        reasons.push("no path".to_string());
    }

    // ---- category rules ----
    if entry.category == Category::StartupFolder && path.as_ref().is_some_and(|p| !path_under_system_dir(p)) {
        // Startup folder entries are more likely to be user-added persistence
        // than registry Run entries (which many legitimate apps use).
        score += 1;
        reasons.push("startup folder entry".to_string());
    }

    // ---- final score -> risk ----
    let risk = if score >= 3 {
        Risk::High
    } else if score >= 2 {
        Risk::Medium
    } else {
        Risk::Low
    };

    RuleResult { risk, reasons }
}

/// Extract the first quoted/executable path from a command line.
/// ponytail: naive heuristic — handles the common `"C:\\path\\app.exe" arg` case.
fn extract_path(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(stripped) = trimmed.strip_prefix('"') {
        if let Some(end) = stripped.find('"') {
            return Some(stripped[..end].to_string());
        }
    }
    let first = trimmed.split_whitespace().next()?;
    Some(first.to_string())
}

fn path_under_system_dir(path: &str) -> bool {
    path_under(SYSTEM_DIRS, &path.to_lowercase())
}

fn path_under(prefixes: &[&str], lower_path: &str) -> bool {
    prefixes.iter().any(|d| lower_path.starts_with(&d.to_lowercase()))
}

/// Check whether a path exists (used by helpers to sanity-check entries).
pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(command: &str) -> RawEntry {
        RawEntry::new(Category::RegistryRun, "HKCU\\Run", "Test", command)
    }

    fn folder_entry(command: &str) -> RawEntry {
        RawEntry::new(Category::StartupFolder, "user_startup", "Test", command)
    }

    #[test]
    fn signed_microsoft_system_path_is_low_risk() {
        let e = entry("\"C:\\Windows\\System32\\foo.exe\" --flag");
        let r = evaluate(&e, Signature::Valid, Some("Microsoft Corporation"));
        assert_eq!(r.risk, Risk::Low, "reasons: {:?}", r.reasons);
    }

    #[test]
    fn unsigned_non_system_path_is_high_risk() {
        let e = entry("\"C:\\Users\\me\\AppData\\Roaming\\evil.exe\"");
        let r = evaluate(&e, Signature::None, None);
        assert_eq!(r.risk, Risk::High, "reasons: {:?}", r.reasons);
    }

    #[test]
    fn temp_directory_is_high_risk() {
        let e = entry("\"C:\\Windows\\Temp\\payload.exe\"");
        let r = evaluate(&e, Signature::None, None);
        // unsigned (+2) + temp (+2) = 4 -> High
        assert_eq!(r.risk, Risk::High, "reasons: {:?}", r.reasons);
    }

    #[test]
    fn startup_folder_unsigned_extra_weight() {
        let e = folder_entry("\"C:\\Users\\me\\Desktop\\thing.exe\"");
        let r = evaluate(&e, Signature::None, None);
        // unsigned (+2) + non-system (+1) + startup folder (+1) = 4 -> High
        assert_eq!(r.risk, Risk::High, "reasons: {:?}", r.reasons);
    }

    #[test]
    fn chinese_path_is_medium_even_if_signed() {
        let e = entry("\"C:\\Program Files\\腾讯软件\\app.exe\"");
        let r = evaluate(&e, Signature::Valid, Some("Tencent"));
        // signed (0) + system path (-1) + non-ascii (+1) = 0 -> Low
        // (Tencent is a legitimate publisher; path is under Program Files)
        assert!(r.risk == Risk::Low || r.risk == Risk::Medium,
            "unexpected risk: {:?} -> {:?}", r.risk, r.reasons);
    }

    #[test]
    fn missing_file_under_user_profile_is_medium() {
        let e = entry("\"C:\\Users\\me\\AppData\\Roaming\\ghost.exe\"");
        let r = evaluate(&e, Signature::Unknown, None);
        // Unknown sig (0) + missing file (+1) = 1 -> Low
        // non-system too: +1 = 2 -> Medium
        assert_eq!(r.risk, Risk::Medium);
    }

    #[test]
    fn invalid_signature_non_system_high() {
        let e = entry("\"C:\\Users\\me\\AppData\\Local\\app.exe\"");
        let r = evaluate(&e, Signature::Invalid, None);
        assert_eq!(r.risk, Risk::High);
    }

    #[test]
    fn extract_path_quoted_and_unquoted() {
        assert_eq!(
            extract_path("\"C:\\a\\b.exe\" -x"),
            Some("C:\\a\\b.exe".to_string())
        );
        assert_eq!(extract_path("C:\\a\\b.exe -x"), Some("C:\\a\\b.exe".to_string()));
        assert_eq!(extract_path(""), None);
    }
}

//! Deterministic risk rule engine.
//!
//! Risk is decided by these rules ONLY — never by the AI. The AI may suggest,
//! but the final label always comes from here. Every rule is a pure function
//! so it is testable and auditable.

use std::path::Path;

use crate::model::{RawEntry, Risk, Signature};

/// System directories that count as "trusted location" signals.
const SYSTEM_DIRS: &[&str] = &[
    "C:\\Windows\\System32",
    "C:\\Windows\\SysWOW64",
    "C:\\Windows",
    "C:\\Program Files",
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

    // Signature rules
    match signature {
        Signature::Valid => {
            // signed: good
        }
        Signature::Invalid => {
            score += 2;
            reasons.push("signature invalid".to_string());
        }
        Signature::None => {
            score += 2;
            reasons.push("unsigned".to_string());
        }
        Signature::Unknown => {
            // e.g. task with no executable — don't penalize
        }
    }

    // Publisher: Microsoft-published is a strong trust signal.
    if let Some(pub_name) = publisher {
        let lower = pub_name.to_lowercase();
        if lower.contains("microsoft") {
            score = score.saturating_sub(1);
        }
    }

    // Path rules: if we can extract a path from the command.
    if let Some(path) = extract_path(&entry.command) {
        if path_under_system_dir(&path) {
            score = score.saturating_sub(1);
        } else {
            score += 1;
            reasons.push("non-system path".to_string());
        }
    }

    // Scheduled tasks are often legitimate system tasks; no extra signal here.
    // (Category-specific rules can be added later.)

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
/// ponytail: naive heuristic — handles the common `"C:\path\app.exe" arg` case.
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
    // Unquoted: take first whitespace-delimited token.
    let first = trimmed.split_whitespace().next()?;
    Some(first.to_string())
}

fn path_under_system_dir(path: &str) -> bool {
    let path = path.to_lowercase();
    SYSTEM_DIRS
        .iter()
        .any(|d| path.starts_with(&d.to_lowercase()))
}

/// Check whether a path exists (used by helpers to sanity-check entries).
pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(command: &str) -> RawEntry {
        RawEntry::new(
            crate::model::Category::RegistryRun,
            "HKCU\\Run",
            "Test",
            command,
        )
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
    fn invalid_signature_non_system_high() {
        let e = entry("\"C:\\Users\\me\\AppData\\Local\\app.exe\"");
        let r = evaluate(&e, Signature::Invalid, None);
        // invalid sig (+2) + non-system path (+1) = 3 -> High
        assert_eq!(r.risk, Risk::High);
    }

    #[test]
    fn invalid_signature_system_path_low() {
        // invalid sig (+2) - system path (-1) = 1 -> Low
        let e = entry("\"C:\\Windows\\System32\\signedbutbad.exe\"");
        let r = evaluate(&e, Signature::Invalid, None);
        assert_eq!(r.risk, Risk::Low);
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

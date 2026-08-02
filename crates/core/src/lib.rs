//! BootKeeper core: enumeration, rule engine, snapshots.
//!
//! Platform-independent logic (model, rules, snapshots) lives at the top
//! level; Windows-only enumeration lives in `windows/`.

pub mod model;
pub mod rules;
pub mod snapshot;
pub mod windows;
pub mod write;

use thiserror::Error;

pub use model::{Category, RawEntry, Risk, Signature, StartupItem};
pub use rules::{evaluate, RuleResult};
pub use snapshot::SnapshotStore;
pub use write::{parse_item_id, WriteOp, WriteResult};
pub use model::SnapshotEntry;

/// Unified error type for the core crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

/// Build a fully-enriched StartupItem from a RawEntry.
///
/// `sig_verifier` is injected so tests can fake it; on Windows the caller
/// passes `windows::signature::verify_file_signature`.
pub fn enrich(
    raw: &RawEntry,
    sig_verifier: impl Fn(&str) -> Signature,
) -> StartupItem {
    let signature = sig_verifier(&raw.command);
    let rule = rules::evaluate(raw, signature, None);
    StartupItem {
        id: StartupItem::build_id(raw.category, &raw.location, &raw.name),
        category: raw.category,
        name: raw.name.clone(),
        command: raw.command.clone(),
        location: raw.location.clone(),
        signature,
        publisher: None,
        risk: rule.risk,
        enabled: !raw.name.ends_with(".disabled"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enrich_marks_disabled_names() {
        let raw = RawEntry::new(Category::RegistryRun, "HKCU\\Run", "Foo.disabled", "x");
        let item = enrich(&raw, |_| Signature::Valid);
        assert!(!item.enabled);
        assert_eq!(item.risk, Risk::Low);
    }

    #[test]
    fn enrich_marks_unsigned_high_risk() {
        let raw = RawEntry::new(Category::RegistryRun, "HKCU\\Run", "Foo", "C:\\evil\\x.exe");
        let item = enrich(&raw, |_| Signature::None);
        assert_eq!(item.risk, Risk::High);
    }
}

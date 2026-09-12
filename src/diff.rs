use crate::baseline::{Baseline, FileRecord};

#[derive(Debug, Clone)]
pub enum Change {
    New(String),
    Deleted(String),
    Modified { path: String, content_changed: bool, mode_changed: bool, owner_changed: bool },
}

/// Compares a stored baseline against a freshly-built one. Order is
/// deterministic (both `BTreeMap`s are sorted by path already).
pub fn diff(old: &Baseline, new: &Baseline) -> Vec<Change> {
    let mut changes = Vec::new();

    for (path, old_record) in &old.files {
        match new.files.get(path) {
            None => changes.push(Change::Deleted(path.clone())),
            Some(new_record) => {
                if let Some(change) = compare(path, old_record, new_record) {
                    changes.push(change);
                }
            }
        }
    }

    for path in new.files.keys() {
        if !old.files.contains_key(path) {
            changes.push(Change::New(path.clone()));
        }
    }

    changes
}

fn compare(path: &str, old: &FileRecord, new: &FileRecord) -> Option<Change> {
    let content_changed = old.sha256 != new.sha256 || old.size != new.size;
    let mode_changed = old.mode != new.mode;
    let owner_changed = old.uid != new.uid || old.gid != new.gid;

    if content_changed || mode_changed || owner_changed {
        Some(Change::Modified { path: path.to_string(), content_changed, mode_changed, owner_changed })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn record(hash: &str, mode: u32, uid: u32, gid: u32, size: u64) -> FileRecord {
        FileRecord { sha256: hash.to_string(), mode, uid, gid, size }
    }

    fn baseline(files: Vec<(&str, FileRecord)>) -> Baseline {
        Baseline { files: files.into_iter().map(|(p, r)| (p.to_string(), r)).collect::<BTreeMap<_, _>>() }
    }

    #[test]
    fn detects_new_file() {
        let old = baseline(vec![]);
        let new = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::New(p) if p == "/etc/foo"));
    }

    #[test]
    fn detects_deleted_file() {
        let old = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let new = baseline(vec![]);
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Deleted(p) if p == "/etc/foo"));
    }

    #[test]
    fn detects_content_change() {
        let old = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let new = baseline(vec![("/etc/foo", record("xyz", 0o644, 0, 0, 12))]);
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            Change::Modified { content_changed, mode_changed, owner_changed, .. } => {
                assert!(content_changed);
                assert!(!mode_changed);
                assert!(!owner_changed);
            }
            other => panic!("expected Modified, got {other:?}"),
        }
    }

    #[test]
    fn detects_permission_change_with_same_content() {
        let old = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let new = baseline(vec![("/etc/foo", record("abc", 0o777, 0, 0, 10))]);
        let changes = diff(&old, &new);
        match &changes[0] {
            Change::Modified { content_changed, mode_changed, .. } => {
                assert!(!content_changed);
                assert!(mode_changed);
            }
            other => panic!("expected Modified, got {other:?}"),
        }
    }

    #[test]
    fn detects_ownership_change() {
        let old = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let new = baseline(vec![("/etc/foo", record("abc", 0o644, 1000, 1000, 10))]);
        let changes = diff(&old, &new);
        match &changes[0] {
            Change::Modified { owner_changed, .. } => assert!(*owner_changed),
            other => panic!("expected Modified, got {other:?}"),
        }
    }

    #[test]
    fn no_changes_produces_empty_diff() {
        let old = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        let new = baseline(vec![("/etc/foo", record("abc", 0o644, 0, 0, 10))]);
        assert!(diff(&old, &new).is_empty());
    }
}

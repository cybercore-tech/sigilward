//! Walks a set of configured paths, hashes and stats every file, and
//! (de)serializes the result as the on-disk baseline. Uses `walkdir` for
//! directory traversal (no reason to hand-roll symlink-safe recursive
//! walking — that's a solved problem) and hand-rolls the actual integrity
//! model (what gets hashed, what metadata matters, how drift is defined),
//! which is the part specific to this tool.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::config::WatchEntry;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    pub sha256: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Baseline {
    /// Absolute path -> record. BTreeMap for deterministic, sorted
    /// serialization (stable diffs in the baseline file across commits).
    pub files: BTreeMap<String, FileRecord>,
    /// Paths that exist but couldn't be read during this walk (e.g.
    /// permission denied). Kept separate from `files` so `diff` can report
    /// "couldn't verify" instead of mistaking them for deletions. Omitted
    /// from the on-disk baseline when empty, so existing baselines load
    /// unchanged.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unreadable: BTreeSet<String>,
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn record_for(path: &Path) -> std::io::Result<FileRecord> {
    let meta = std::fs::symlink_metadata(path)?;
    // Symlinks are recorded by their own metadata (not followed) — a
    // symlink being repointed is itself a change worth catching, and
    // following it risks walking outside the intended watched tree.
    if meta.file_type().is_symlink() {
        return Ok(FileRecord { sha256: "symlink".to_string(), mode: meta.mode(), uid: meta.uid(), gid: meta.gid(), size: 0 });
    }
    Ok(FileRecord { sha256: hash_file(path)?, mode: meta.mode(), uid: meta.uid(), gid: meta.gid(), size: meta.len() })
}

/// Builds a fresh `Baseline` by walking every configured watch entry right
/// now. Files that can't be read (permission denied, disappeared mid-walk)
/// are skipped with a warning to stderr rather than aborting the whole run
/// — one unreadable file shouldn't block auditing everything else.
pub fn build(entries: &[WatchEntry]) -> Baseline {
    let mut baseline = Baseline::default();

    for entry in entries {
        let root = crate::config::expand_home(&entry.path);
        if !root.exists() {
            eprintln!("sigilward: warning: watched path does not exist, skipping: {}", root.display());
            continue;
        }

        let walker = if entry.recursive {
            walkdir::WalkDir::new(&root)
        } else {
            // max_depth(0) would mean "only the root entry itself," which
            // is the directory (skipped below since only files/symlinks
            // are recorded) — depth 1 is root's immediate children, which
            // is what "non-recursive" actually means here.
            walkdir::WalkDir::new(&root).max_depth(1)
        };

        for entry_result in walker {
            let dir_entry = match entry_result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("sigilward: warning: walk error: {e}");
                    if let Some(path) = e.path() {
                        if path.exists() {
                            baseline.unreadable.insert(path.to_string_lossy().into_owned());
                        }
                    }
                    continue;
                }
            };
            if dir_entry.file_type().is_dir() {
                continue; // only files (and symlinks) get recorded, not directory entries themselves
            }
            let path = dir_entry.path();
            match record_for(path) {
                Ok(record) => {
                    baseline.files.insert(path.to_string_lossy().into_owned(), record);
                }
                Err(e) => {
                    eprintln!("sigilward: warning: could not read {}: {}", path.display(), e);
                    // Gone mid-walk is a genuine absence; anything else
                    // (permission denied, I/O error) means it's still there
                    // but unverified.
                    if e.kind() != std::io::ErrorKind::NotFound {
                        baseline.unreadable.insert(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    baseline
}

pub fn save(baseline: &Baseline, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(baseline)?;
    std::fs::write(path, json)
}

pub fn load(path: &Path) -> std::io::Result<Baseline> {
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn hashes_identical_content_identically() {
        let dir = std::env::temp_dir().join(format!("sigilward-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.txt");
        std::fs::write(&path, b"hello world").unwrap();

        let r1 = record_for(&path).unwrap();
        let r2 = record_for(&path).unwrap();
        assert_eq!(r1.sha256, r2.sha256);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn different_content_hashes_differently() {
        let dir = std::env::temp_dir().join(format!("sigilward-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.txt");

        std::fs::write(&path, b"version one").unwrap();
        let r1 = record_for(&path).unwrap();

        let mut f = std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
        f.write_all(b"version two, different content").unwrap();
        drop(f);
        let r2 = record_for(&path).unwrap();

        assert_ne!(r1.sha256, r2.sha256);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_walks_recursive_directory() {
        let dir = std::env::temp_dir().join(format!("sigilward-test3-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("top.txt"), b"top").unwrap();
        std::fs::write(dir.join("sub/nested.txt"), b"nested").unwrap();

        let entries = vec![WatchEntry { path: dir.to_string_lossy().into_owned(), recursive: true }];
        let baseline = build(&entries);

        assert_eq!(baseline.files.len(), 2);
        assert!(baseline.files.keys().any(|k| k.ends_with("top.txt")));
        assert!(baseline.files.keys().any(|k| k.ends_with("nested.txt")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_recursive_skips_subdirectories() {
        let dir = std::env::temp_dir().join(format!("sigilward-test4-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("top.txt"), b"top").unwrap();
        std::fs::write(dir.join("sub/nested.txt"), b"nested").unwrap();

        let entries = vec![WatchEntry { path: dir.to_string_lossy().into_owned(), recursive: false }];
        let baseline = build(&entries);

        assert_eq!(baseline.files.len(), 1);
        assert!(baseline.files.keys().any(|k| k.ends_with("top.txt")));

        std::fs::remove_dir_all(&dir).ok();
    }
}

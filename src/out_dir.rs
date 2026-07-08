//! Per-run hygiene for allie's `--out` command directories (AL-117).
//!
//! `allie run|report|release|verify` each write into a caller-chosen `--out`
//! directory. Before this module existed every write site only ever called
//! `fs::create_dir_all` and never cleaned it, so a rerun into a dirty
//! directory silently kept whatever an older run — or an older, since-retired
//! code path — had written there. Dogfood R5: a `remediation/` stage the
//! pipeline no longer emits outlived the code that wrote it and sat next to
//! a fresh evidence packet, so the packet directory no longer described
//! exactly one run.
//!
//! The fix never deletes a directory it cannot account for: each managed
//! out-dir gets a small `allie-run-manifest.json` listing every file the run
//! left behind. The next run into that directory deletes exactly the files
//! the manifest says the previous run owned (then prunes any directories
//! that fall empty as a result) before writing fresh output. A directory
//! that already has content and no manifest is refused outright — the
//! caller has to point `--out` at a fresh directory, the same refusal
//! `workbench start` already uses for its own job directory
//! (`ensure_new_workbench_dir` in `src/workbench.rs`).

use crate::{AllieError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MANIFEST_FILE_NAME: &str = "allie-run-manifest.json";
const MANIFEST_SCHEMA: &str = "allie.run-manifest.v0";

#[derive(Debug, Serialize, Deserialize)]
struct RunManifest {
    schema: String,
    command: String,
    written_at: String,
    /// Paths relative to the out-dir, forward-slash separated, sorted.
    files: Vec<String>,
}

/// Make `out_dir` ready to receive a fresh run's output.
///
/// - Missing directory: created.
/// - Empty directory: left as-is.
/// - Directory holding a manifest from a prior allie `command` run: every
///   file the manifest lists is removed, then any directory that falls
///   empty as a result is pruned (the out-dir itself is never removed).
/// - Non-empty directory with no manifest: refused. allie has no way to
///   tell which of those files are safe to remove, so it does not guess.
pub(crate) fn prepare_out_dir(out_dir: &Path, command: &str) -> Result<()> {
    if !out_dir.exists() {
        return create_dir_all(out_dir);
    }

    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    if manifest_path.exists() {
        let manifest: RunManifest = crate::read_json_file(&manifest_path)?;
        for relative in &manifest.files {
            remove_file_if_present(&out_dir.join(relative))?;
        }
        remove_file_if_present(&manifest_path)?;
        prune_empty_dirs(out_dir, out_dir)?;
        return create_dir_all(out_dir);
    }

    if dir_is_empty(out_dir)? {
        return create_dir_all(out_dir);
    }

    Err(AllieError::InvalidManifest(format!(
        "{command} output directory {} already has files in it that are not from an allie {command} run (no {MANIFEST_FILE_NAME} found); choose a new --out directory, or remove its contents, and rerun",
        out_dir.display()
    )))
}

/// Record every file now present under `out_dir` so the next
/// [`prepare_out_dir`] call for the same directory knows exactly what this
/// `command` run is safe to clean up. Call this once, after every write for
/// the run has completed.
pub(crate) fn finalize_out_dir_manifest(out_dir: &Path, command: &str) -> Result<()> {
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let mut files = Vec::new();
    collect_files(out_dir, out_dir, &manifest_path, &mut files)?;
    files.sort();
    let manifest = RunManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        command: command.to_string(),
        written_at: crate::now_utc().to_rfc3339(),
        files,
    };
    crate::write_json_pretty(&manifest_path, &manifest)
}

fn create_dir_all(out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir).map_err(|source| AllieError::Io {
        context: format!("create output directory {}", out_dir.display()),
        source,
    })
}

fn dir_is_empty(dir: &Path) -> Result<bool> {
    let mut entries = fs::read_dir(dir).map_err(|source| AllieError::Io {
        context: format!("read directory {}", dir.display()),
        source,
    })?;
    Ok(entries.next().is_none())
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(AllieError::Io {
            context: format!("remove stale file {}", path.display()),
            source,
        }),
    }
}

fn collect_files(
    root: &Path,
    dir: &Path,
    manifest_path: &Path,
    out: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|source| AllieError::Io {
        context: format!("read directory {}", dir.display()),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AllieError::Io {
            context: format!("read directory entry under {}", dir.display()),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| AllieError::Io {
            context: format!("read file type of {}", path.display()),
            source,
        })?;
        if file_type.is_dir() {
            collect_files(root, &path, manifest_path, out)?;
        } else if file_type.is_file() && path != manifest_path {
            out.push(crate::path_relative_to(root, &path));
        }
    }
    Ok(())
}

fn prune_empty_dirs(root: &Path, dir: &Path) -> Result<()> {
    let children: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|source| AllieError::Io {
            context: format!("read directory {}", dir.display()),
            source,
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    for child in children {
        if child.is_dir() {
            prune_empty_dirs(root, &child)?;
            if child != root && dir_is_empty(&child)? {
                fs::remove_dir(&child).map_err(|source| AllieError::Io {
                    context: format!("remove empty directory {}", child.display()),
                    source,
                })?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

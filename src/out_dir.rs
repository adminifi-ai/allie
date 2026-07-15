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
//! Ownership model: an `allie-run-manifest.json` in a directory marks the
//! whole directory as owned by one allie command's runs. [`prepare_out_dir`]
//! claims the directory by writing the manifest with `phase: "in_progress"`
//! immediately, before the run produces any output; [`finalize_out_dir_manifest`]
//! rewrites it (atomically) with `phase: "complete"` and the full list of
//! files the run left behind. A rerun into a manifested directory removes
//! everything in it — manifested files, stragglers dropped in afterwards,
//! and partial output stranded by a crash between prepare and finalize —
//! then re-claims it, so the directory always describes exactly one run and
//! a crashed run never wedges the directory into a refuse-forever state.
//! Anything a user drops into a managed out-dir is therefore absorbed and
//! removed on the next rerun: managed out-dirs belong to allie, not scratch
//! space.
//!
//! Safety boundaries, in order of enforcement:
//! - A non-empty directory with NO manifest is refused outright with an
//!   actionable error — allie cannot tell which files are safe to remove,
//!   so it does not guess (the same refusal `workbench start` uses for its
//!   job directory; see `ensure_new_workbench_dir` in `src/workbench.rs`).
//! - A manifest with an unknown schema, a different command, or any file
//!   entry that could escape the out-dir (absolute path or a non-normal
//!   component such as `..`) rejects the whole manifest: hard error,
//!   nothing deleted.
//! - Deletion never uses manifest entries as paths at all. The cleanup walk
//!   is rooted at the out-dir via `read_dir`, uses lstat-based file types,
//!   and treats symlinks as leaf entries — it removes the link itself and
//!   never recurses through or deletes anything on the other side — so
//!   every removed path is physically inside the out-dir by construction.
//! - Allie never `rm -rf`s a directory it cannot account for.

use crate::model::PublicationClass;
use crate::{AllieError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path};

const MANIFEST_FILE_NAME: &str = "allie-run-manifest.json";
const MANIFEST_SCHEMA: &str = "allie.run-manifest.v0";
const PHASE_IN_PROGRESS: &str = "in_progress";
const PHASE_COMPLETE: &str = "complete";

#[derive(Debug, Serialize, Deserialize)]
struct RunManifest {
    schema: String,
    command: String,
    #[serde(default)]
    publication_class: PublicationClass,
    /// "in_progress" from prepare until the run's last write; "complete"
    /// once finalize has recorded the run's files. Additive to v0: absent in
    /// manifests written before the field existed, which are treated as
    /// complete.
    #[serde(default = "default_phase")]
    phase: String,
    written_at: String,
    /// Paths relative to the out-dir, sorted. Evidence of what the run
    /// wrote — never used as deletion targets (see the module doc's safety
    /// boundaries).
    files: Vec<String>,
}

fn default_phase() -> String {
    PHASE_COMPLETE.to_string()
}

/// Make `out_dir` ready to receive a fresh run's output and claim it with an
/// in-progress manifest.
///
/// - Missing directory: created, claimed.
/// - Empty directory: claimed.
/// - Directory holding a valid manifest for the same `command` (any phase):
///   all contents removed, then re-claimed.
/// - Non-empty directory with no manifest, or with a manifest that fails
///   validation (unknown schema, other command, escaping entries): refused
///   with nothing deleted.
pub(crate) fn prepare_out_dir(out_dir: &Path, command: &str) -> Result<()> {
    create_dir_all(out_dir)?;

    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    if manifest_path.exists() {
        let manifest: RunManifest = crate::read_json_file(&manifest_path)?;
        validate_manifest(out_dir, command, &manifest)?;
        clean_dir_contents(out_dir)?;
    } else if !dir_is_empty(out_dir)? {
        return Err(AllieError::InvalidManifest(format!(
            "{command} output directory {} already has files in it that are not from an allie {command} run (no {MANIFEST_FILE_NAME} found); choose a new --out directory, or remove its contents, and rerun",
            out_dir.display()
        )));
    }

    write_manifest(out_dir, command, PHASE_IN_PROGRESS, Vec::new())
}

/// Record every file now present under `out_dir` and mark the run complete.
/// Call this once, after every write for the run has finished.
pub(crate) fn finalize_out_dir_manifest(out_dir: &Path, command: &str) -> Result<()> {
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let mut files = Vec::new();
    collect_files(out_dir, out_dir, &manifest_path, &mut files)?;
    files.sort();
    write_manifest(out_dir, command, PHASE_COMPLETE, files)
}

fn validate_manifest(out_dir: &Path, command: &str, manifest: &RunManifest) -> Result<()> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "{} in {} has unknown schema {} (expected {MANIFEST_SCHEMA}); refusing to clean a directory allie cannot account for — choose a new --out directory",
            MANIFEST_FILE_NAME,
            out_dir.display(),
            manifest.schema
        )));
    }
    if manifest.command != command {
        return Err(AllieError::InvalidManifest(format!(
            "output directory {} belongs to `allie {}` (per {}), not `allie {command}`; choose a new --out directory instead of mixing command outputs",
            out_dir.display(),
            manifest.command,
            MANIFEST_FILE_NAME
        )));
    }
    for entry in &manifest.files {
        let path = Path::new(entry);
        let escapes = path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)));
        if escapes {
            return Err(AllieError::InvalidManifest(format!(
                "{} in {} lists {entry}, which could escape the output directory; refusing to clean — remove the tampered manifest or choose a new --out directory",
                MANIFEST_FILE_NAME,
                out_dir.display()
            )));
        }
    }
    Ok(())
}

fn write_manifest(out_dir: &Path, command: &str, phase: &str, files: Vec<String>) -> Result<()> {
    let manifest = RunManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        command: command.to_string(),
        publication_class: match command {
            "publication" => PublicationClass::PublicSummary,
            _ => PublicationClass::SensitiveLocal,
        },
        phase: phase.to_string(),
        written_at: crate::now_utc().to_rfc3339(),
        files,
    };
    let path = out_dir.join(MANIFEST_FILE_NAME);
    let json = serde_json::to_string_pretty(&manifest).map_err(|source| AllieError::Json {
        context: format!("serialize run manifest {}", path.display()),
        source,
    })?;
    // Atomic so a crash mid-write can never leave a corrupt manifest that
    // would wedge every later prepare into a parse error.
    crate::write_string_atomic(&path, &(json + "\n"))
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

/// Remove everything inside `dir` (not `dir` itself). File types come from
/// `DirEntry::file_type`, which does not traverse symlinks: a symlink is
/// removed as a leaf via `remove_file`, never recursed into, so nothing
/// outside the walk root can be touched.
fn clean_dir_contents(dir: &Path) -> Result<()> {
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
            clean_dir_contents(&path)?;
            fs::remove_dir(&path).map_err(|source| AllieError::Io {
                context: format!("remove stale directory {}", path.display()),
                source,
            })?;
        } else {
            fs::remove_file(&path).map_err(|source| AllieError::Io {
                context: format!("remove stale file {}", path.display()),
                source,
            })?;
        }
    }
    Ok(())
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
        } else if path != manifest_path {
            out.push(crate::path_relative_to(root, &path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

//! Host-neutral public-publication projection.
//!
//! A verify directory is canonical local evidence and is sensitive by
//! default. This module never copies caller-selected files from that tree.
//! It emits a deliberately small summary whose schema excludes routes,
//! filesystem paths, artifact links, model payloads, and captured content.
//! Any attempt to include canonical evidence is a retryable policy refusal;
//! the source tree is read-only throughout.

use crate::model::PublicationClass;
use crate::{AllieError, ExitClass, Result};
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

const PUBLICATION_SCHEMA: &str = "allie.publication.v0";
const PUBLIC_SUMMARY_JSON: &str = "allie-public-summary.json";
const PUBLIC_SUMMARY_MARKDOWN: &str = "allie-public-summary.md";
const RECEIPT_JSON: &str = "publication-receipt.json";
const REFUSAL_REASON: &str = "public publishers accept only public_summary artifacts";

#[derive(Debug)]
pub(crate) struct PublicationOptions {
    pub(crate) verify_root: PathBuf,
    pub(crate) out_dir: PathBuf,
    pub(crate) requested_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PublicationStatus {
    Ready,
    Refused,
}

impl PublicationStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Refused => "refused",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicationArtifact {
    path: String,
    publication_class: PublicationClass,
    sha256: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RefusedArtifact {
    publication_class: PublicationClass,
    reason: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicationReceipt {
    schema: String,
    publication_class: PublicationClass,
    pub(crate) status: PublicationStatus,
    pub(crate) retryable: bool,
    pub(crate) published: Vec<PublicationArtifact>,
    pub(crate) refused: Vec<RefusedArtifact>,
    #[serde(skip)]
    pub(crate) exit_class: ExitClass,
    #[serde(skip)]
    pub(crate) receipt_path: PathBuf,
}

pub(crate) fn parse_publication_options(
    args: &[String],
) -> std::result::Result<PublicationOptions, String> {
    let mut verify_root = None;
    let mut out_dir = None;
    let mut requested_paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--verify-root" => {
                index += 1;
                verify_root =
                    Some(PathBuf::from(args.get(index).ok_or_else(|| {
                        "--verify-root requires a path".to_string()
                    })?));
            }
            "--out" => {
                index += 1;
                out_dir = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--out requires a path".to_string())?,
                ));
            }
            "--include" => {
                index += 1;
                requested_paths.push(
                    args.get(index)
                        .ok_or_else(|| {
                            "--include requires a verify-root-relative path".to_string()
                        })?
                        .to_string(),
                );
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }
    Ok(PublicationOptions {
        verify_root: verify_root.ok_or_else(|| "--verify-root is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
        requested_paths,
    })
}

pub(crate) fn run_publication(options: PublicationOptions) -> Result<PublicationReceipt> {
    ensure_separate_roots(&options.verify_root, &options.out_dir)?;
    crate::out_dir::prepare_out_dir(&options.out_dir, "publication")?;

    let refused = options
        .requested_paths
        .iter()
        .map(|_| RefusedArtifact {
            publication_class: PublicationClass::SensitiveLocal,
            reason: REFUSAL_REASON.to_string(),
        })
        .collect::<Vec<_>>();
    if !refused.is_empty() {
        let receipt = PublicationReceipt {
            schema: PUBLICATION_SCHEMA.to_string(),
            publication_class: PublicationClass::PublicSummary,
            status: PublicationStatus::Refused,
            retryable: true,
            published: Vec::new(),
            refused,
            exit_class: ExitClass::InfrastructureFailure,
            receipt_path: options.out_dir.join(RECEIPT_JSON),
        };
        persist_receipt(&options.out_dir, &receipt)?;
        return Ok(receipt);
    }

    let verify_summary_path = options.verify_root.join("reporters/allie-report.json");
    let summary = summary::read_verify_summary(&verify_summary_path)?;
    let summary_json_path = options.out_dir.join(PUBLIC_SUMMARY_JSON);
    let summary_markdown_path = options.out_dir.join(PUBLIC_SUMMARY_MARKDOWN);
    crate::write_json_pretty(&summary_json_path, &summary)?;
    crate::write_string(&summary_markdown_path, &summary::markdown(&summary))?;

    let receipt = PublicationReceipt {
        schema: PUBLICATION_SCHEMA.to_string(),
        publication_class: PublicationClass::PublicSummary,
        status: PublicationStatus::Ready,
        retryable: false,
        published: vec![
            publication_artifact(&summary_json_path, PUBLIC_SUMMARY_JSON)?,
            publication_artifact(&summary_markdown_path, PUBLIC_SUMMARY_MARKDOWN)?,
        ],
        refused: Vec::new(),
        exit_class: ExitClass::Success,
        receipt_path: options.out_dir.join(RECEIPT_JSON),
    };
    persist_receipt(&options.out_dir, &receipt)?;
    Ok(receipt)
}

fn ensure_separate_roots(verify_root: &Path, out_dir: &Path) -> Result<()> {
    reject_parent_components(verify_root)?;
    reject_parent_components(out_dir)?;
    let verify_root = canonical_location(verify_root)?;
    let out_dir = canonical_location(out_dir)?;
    if verify_root.starts_with(&out_dir) || out_dir.starts_with(&verify_root) {
        return Err(AllieError::Runtime(
            "publication verify root and output directory must not overlap".to_string(),
        ));
    }
    Ok(())
}

fn reject_parent_components(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(AllieError::Runtime(
            "publication paths must not contain parent-directory components".to_string(),
        ));
    }
    Ok(())
}

fn canonical_location(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).map_err(|source| AllieError::Io {
        context: format!("resolving publication path {}", path.display()),
        source,
    })?;
    let mut cursor = absolute.as_path();
    let mut missing = Vec::<OsString>::new();
    while !cursor.exists() {
        missing.push(
            cursor
                .file_name()
                .ok_or_else(|| {
                    AllieError::Runtime(format!(
                        "publication path {} has no existing ancestor",
                        path.display()
                    ))
                })?
                .to_os_string(),
        );
        cursor = cursor.parent().ok_or_else(|| {
            AllieError::Runtime(format!(
                "publication path {} has no existing ancestor",
                path.display()
            ))
        })?;
    }
    let mut canonical = fs::canonicalize(cursor).map_err(|source| AllieError::Io {
        context: format!("canonicalizing publication path {}", path.display()),
        source,
    })?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn publication_artifact(path: &Path, relative_path: &str) -> Result<PublicationArtifact> {
    Ok(PublicationArtifact {
        path: relative_path.to_string(),
        publication_class: PublicationClass::PublicSummary,
        sha256: format!("sha256:{}", crate::sha256_file(path)?),
    })
}

fn persist_receipt(out_dir: &Path, receipt: &PublicationReceipt) -> Result<()> {
    crate::write_json_pretty(&receipt.receipt_path, receipt)?;
    crate::out_dir::finalize_out_dir_manifest(out_dir, "publication")
}

mod summary;

#[cfg(test)]
#[path = "publication/tests.rs"]
mod tests;

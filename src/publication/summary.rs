use crate::model::PublicationClass;
use crate::{AllieError, Result};
use chrono::DateTime;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;

const PUBLIC_SUMMARY_SCHEMA: &str = "allie.public-summary.v0";
const MAX_VERIFY_SUMMARY_BYTES: u64 = 1_000_000;

#[derive(Debug, Deserialize)]
struct VerifySummary {
    #[serde(rename = "schema")]
    _schema: VerifySchema,
    status: VerifyStatus,
    exit_code: i32,
    generated_at: String,
    release_status: ReleaseStatus,
    run_status: RunStatus,
    why: VerifyWhy,
}

#[derive(Debug, Deserialize)]
enum VerifySchema {
    #[serde(rename = "allie.verify.v0")]
    V0,
}

#[derive(Debug, Deserialize)]
struct VerifyWhy {
    blocking: VerifyBlocking,
    compliance_summary: VerifyWcag,
}

#[derive(Debug, Deserialize)]
struct VerifyBlocking {
    deterministic_failures: u64,
    scripted_failures: u64,
    infrastructure_failures: u64,
    missing_required_evidence: Vec<IgnoredAny>,
}

#[derive(Debug, Deserialize, Serialize)]
struct VerifyWcag {
    pass: u64,
    fail: u64,
    needs_review: u64,
    not_tested: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum VerifyStatus {
    Approved,
    NeedsReview,
    Blocked,
    Failed,
}

impl VerifyStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::NeedsReview => "needs_review",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReleaseStatus {
    Approved,
    NeedsReview,
    Blocked,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RunStatus {
    Pass,
    Fail,
    Error,
}

#[derive(Debug, Serialize)]
pub(super) struct PublicSummary {
    schema: &'static str,
    status: VerifyStatus,
    exit_code: i32,
    generated_at: String,
    release_status: ReleaseStatus,
    run_status: RunStatus,
    blocking: PublicBlocking,
    wcag: VerifyWcag,
    publication_class: PublicationClass,
    legal_claim: &'static str,
}

#[derive(Debug, Serialize)]
struct PublicBlocking {
    deterministic_failures: u64,
    scripted_failures: u64,
    infrastructure_failures: u64,
    missing_required_evidence_count: usize,
}

pub(super) fn read_verify_summary(path: &Path) -> Result<PublicSummary> {
    let mut file = fs::File::open(path).map_err(|_| invalid_source())?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_VERIFY_SUMMARY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_source())?;
    if bytes.len() as u64 > MAX_VERIFY_SUMMARY_BYTES {
        return Err(AllieError::Runtime(
            "publication source report exceeds the safe size limit".to_string(),
        ));
    }
    let summary: VerifySummary = serde_json::from_slice(&bytes).map_err(|_| invalid_source())?;
    project(summary)
}

fn invalid_source() -> AllieError {
    AllieError::Runtime("publication source report is missing or invalid".to_string())
}

fn project(summary: VerifySummary) -> Result<PublicSummary> {
    if !matches!(summary.exit_code, 0..=2) {
        return Err(AllieError::Runtime(
            "publication source report is missing or invalid".to_string(),
        ));
    }
    let generated_at = DateTime::parse_from_rfc3339(&summary.generated_at)
        .map_err(|_| invalid_source())?
        .to_rfc3339();
    Ok(PublicSummary {
        schema: PUBLIC_SUMMARY_SCHEMA,
        status: summary.status,
        exit_code: summary.exit_code,
        generated_at,
        release_status: summary.release_status,
        run_status: summary.run_status,
        blocking: PublicBlocking {
            deterministic_failures: summary.why.blocking.deterministic_failures,
            scripted_failures: summary.why.blocking.scripted_failures,
            infrastructure_failures: summary.why.blocking.infrastructure_failures,
            missing_required_evidence_count: summary.why.blocking.missing_required_evidence.len(),
        },
        wcag: summary.why.compliance_summary,
        publication_class: PublicationClass::PublicSummary,
        legal_claim: "evidence visibility only; not a legal compliance guarantee",
    })
}

pub(super) fn markdown(summary: &PublicSummary) -> String {
    format!(
        "# Allie Public Verification Summary\n\nStatus: `{status}`\n\nBlocking evidence: deterministic failures {deterministic}, scripted failures {scripted}, infrastructure failures {infrastructure}, missing required evidence {missing}.\n\nWCAG matrix: pass {pass}, fail {fail}, needs review {review}, not tested {not_tested}.\n\nThis public summary intentionally excludes routes, paths, captured artifacts, prompts, URLs, and raw evidence. It is evidence visibility only, not a legal compliance guarantee.\n",
        status = summary.status.as_str(),
        deterministic = summary.blocking.deterministic_failures,
        scripted = summary.blocking.scripted_failures,
        infrastructure = summary.blocking.infrastructure_failures,
        missing = summary.blocking.missing_required_evidence_count,
        pass = summary.wcag.pass,
        fail = summary.wcag.fail,
        review = summary.wcag.needs_review,
        not_tested = summary.wcag.not_tested,
    )
}

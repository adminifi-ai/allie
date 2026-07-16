use serde::{Deserialize, Serialize};

use super::AgenticMediaRef;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ModelEgressEvent {
    pub(crate) schema: String,
    pub(crate) occurred_at: String,
    pub(crate) prompt_version: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) endpoint: String,
    pub(crate) status: String,
    pub(crate) calls: u64,
    pub(crate) prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) redaction_profile: crate::ModelRedactionMode,
    pub(crate) redaction_status: String,
    pub(crate) request_sha256: String,
    pub(crate) response_sha256: String,
}

/// One criterion's model assessment, with run-relative media paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AgenticAssessmentRecord {
    pub(crate) obligation: String,
    pub(crate) assessment: String,
    pub(crate) rationale: String,
    pub(crate) reviewer_guidance: String,
    pub(crate) confidence: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) media: Vec<AgenticMediaRef>,
}

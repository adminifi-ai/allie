use serde::{Deserialize, Serialize};

use super::AgenticMediaRef;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ModelEgressUsage {
    pub(crate) prompt_tokens: Option<u64>,
    pub(crate) completion_tokens: Option<u64>,
    pub(crate) total_tokens: Option<u64>,
    pub(crate) cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ModelEgressEvent {
    pub(crate) schema: String,
    pub(crate) attempt: u64,
    pub(crate) started_at: String,
    pub(crate) requested_provider: String,
    pub(crate) requested_model: String,
    pub(crate) prompt_version: String,
    pub(crate) prompt_sha256: String,
    pub(crate) media_sha256: Vec<String>,
    pub(crate) zdr_required: bool,
    pub(crate) allow_fallbacks: bool,
    pub(crate) outcome: String,
    pub(crate) http_status: Option<u16>,
    pub(crate) error_class: Option<String>,
    pub(crate) response_id: Option<String>,
    pub(crate) generation_id: Option<String>,
    pub(crate) routed_provider: Option<String>,
    pub(crate) routed_model: Option<String>,
    pub(crate) usage: Option<ModelEgressUsage>,
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

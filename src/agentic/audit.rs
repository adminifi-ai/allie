use crate::agentic::redaction::accepted_redaction_receipt;
use crate::model::{EvidencePacket, ModelEgressEvent};
use crate::{ModelRedactionMode, Result, sha256_file};
use std::path::Path;

pub(super) const PROMPT_VERSION: &str = "allie.agentic.wcag-review.v1";

pub(super) fn record_model_egress(
    packet: &mut EvidencePacket,
    request: &serde_json::Value,
    response: &serde_json::Value,
    request_path: &Path,
    response_path: &Path,
    expected_redaction: Option<ModelRedactionMode>,
    occurred_at: String,
) -> Result<(String, String)> {
    let redaction_receipt = accepted_redaction_receipt(response, expected_redaction)?;
    let calls = response["calls"].as_u64().unwrap_or_default();
    packet.policy.model_egress_redaction = Some(redaction_receipt.profile);
    packet.policy.budget.model_calls = u32::try_from(calls).unwrap_or(u32::MAX);
    let provider = response["provider"]
        .as_str()
        .unwrap_or("openrouter")
        .to_string();
    let model = response["model"].as_str().unwrap_or_default().to_string();
    packet.model_egress_events.push(ModelEgressEvent {
        schema: "allie.model-egress-event.v0".to_string(),
        occurred_at,
        prompt_version: PROMPT_VERSION.to_string(),
        provider: provider.clone(),
        model: model.clone(),
        endpoint: request["model"]["base_url"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        status: response["status"].as_str().unwrap_or("unknown").to_string(),
        calls,
        prompt_tokens: response["usage"]["prompt_tokens"]
            .as_u64()
            .unwrap_or_default(),
        completion_tokens: response["usage"]["completion_tokens"]
            .as_u64()
            .unwrap_or_default(),
        redaction_profile: redaction_receipt.profile,
        redaction_status: response["redaction_receipt"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        request_sha256: format!("sha256:{}", sha256_file(request_path)?),
        response_sha256: format!("sha256:{}", sha256_file(response_path)?),
    });
    Ok((provider, model))
}

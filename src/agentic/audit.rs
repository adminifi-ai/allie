use crate::agentic::redaction::accepted_redaction_receipt;
use crate::model::{EvidencePacket, ModelEgressEvent};
use crate::{AllieError, ModelRedactionMode, Result, sha256_file};
use std::path::Path;

pub(super) const PROMPT_VERSION: &str = "allie.agentic.wcag-review.v1";

fn required_string(value: &serde_json::Value, pointer: &str, field: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AllieError::Worker(format!("agentic worker response omitted {field}")))
}

fn required_u64(value: &serde_json::Value, pointer: &str, field: &str) -> Result<u64> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            AllieError::Worker(format!("agentic worker response omitted numeric {field}"))
        })
}

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
    let calls = required_u64(response, "/calls", "calls")?;
    let provider = required_string(response, "/provider", "provider")?;
    let model = required_string(response, "/model", "model")?;
    let endpoint = required_string(request, "/model/base_url", "request model base_url")?;
    let status = required_string(response, "/status", "status")?;
    let prompt_tokens = required_u64(response, "/usage/prompt_tokens", "usage.prompt_tokens")?;
    let completion_tokens = required_u64(
        response,
        "/usage/completion_tokens",
        "usage.completion_tokens",
    )?;
    packet.policy.model_egress_redaction = Some(redaction_receipt.profile);
    packet.policy.budget.model_calls = u32::try_from(calls).unwrap_or(u32::MAX);
    packet.model_egress_events.push(ModelEgressEvent {
        schema: "allie.model-egress-event.v0".to_string(),
        occurred_at,
        prompt_version: PROMPT_VERSION.to_string(),
        provider: provider.clone(),
        model: model.clone(),
        endpoint,
        status,
        calls,
        prompt_tokens,
        completion_tokens,
        redaction_profile: redaction_receipt.profile,
        redaction_status: redaction_receipt.status_str().to_string(),
        request_sha256: format!("sha256:{}", sha256_file(request_path)?),
        response_sha256: format!("sha256:{}", sha256_file(response_path)?),
    });
    Ok((provider, model))
}

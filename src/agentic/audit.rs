use crate::agentic::redaction::accepted_redaction_receipt;
use crate::model::{EvidencePacket, ModelEgressEvent, ModelEgressUsage};
use crate::{AllieError, ModelRedactionMode, Result};

pub(super) const PROMPT_VERSION: &str = "allie.agentic.wcag-review.v1";

pub(super) fn record_model_egress(
    packet: &mut EvidencePacket,
    request: &serde_json::Value,
    response: &serde_json::Value,
    expected_redaction: Option<ModelRedactionMode>,
) -> Result<(String, String)> {
    let redaction_receipt = accepted_redaction_receipt(response, expected_redaction)?;
    let calls = response
        .pointer("/calls")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            AllieError::Worker("agentic worker response omitted numeric calls".to_string())
        })?;
    let requested_provider = request_string(request, "/model/provider", "model.provider")?;
    let requested_model = request_string(request, "/model/model", "model.model")?;
    let zdr_required = request
        .pointer("/model/zdr_required")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            AllieError::Worker(
                "agentic worker request omitted boolean model.zdr_required".to_string(),
            )
        })?;
    let audit_value = response
        .get("model_call_audit")
        .ok_or_else(|| {
            AllieError::Worker("agentic worker response omitted model_call_audit".to_string())
        })?
        .clone();
    let events: Vec<ModelEgressEvent> = serde_json::from_value(audit_value).map_err(|source| {
        AllieError::Worker(format!(
            "agentic worker returned malformed model_call_audit: {source}"
        ))
    })?;

    validate_event_count(calls, &events)?;
    for (index, event) in events.iter().enumerate() {
        validate_event(
            event,
            index as u64 + 1,
            &requested_provider,
            &requested_model,
            zdr_required,
        )?;
    }

    let actual_route = events.iter().rev().find(|event| event.outcome == "success");
    let provider = actual_route
        .and_then(|event| event.routed_provider.clone())
        .unwrap_or_else(|| requested_provider.clone());
    let model = actual_route
        .and_then(|event| event.routed_model.clone())
        .unwrap_or_else(|| requested_model.clone());

    packet.policy.model_egress_redaction = Some(redaction_receipt.profile);
    packet.policy.budget.model_calls = u32::try_from(calls).unwrap_or(u32::MAX);
    packet.model_egress_events.extend(events);
    Ok((provider, model))
}

fn validate_event_count(calls: u64, events: &[ModelEgressEvent]) -> Result<()> {
    if calls == events.len() as u64 {
        return Ok(());
    }
    Err(AllieError::Worker(format!(
        "agentic worker calls {calls} contradict model_call_audit event count {}",
        events.len()
    )))
}

fn request_string(value: &serde_json::Value, pointer: &str, field: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AllieError::Worker(format!("agentic worker request omitted {field}")))
}

fn validate_event(
    event: &ModelEgressEvent,
    expected_attempt: u64,
    requested_provider: &str,
    requested_model: &str,
    zdr_required: bool,
) -> Result<()> {
    if event.schema != "allie.model-egress-event.v1" {
        return invalid_event(event, "schema is not allie.model-egress-event.v1");
    }
    if event.attempt != expected_attempt {
        return invalid_event(event, &format!("attempt must be {expected_attempt}"));
    }
    if chrono::DateTime::parse_from_rfc3339(&event.started_at).is_err() {
        return invalid_event(event, "started_at is not RFC 3339");
    }
    if event.requested_provider != requested_provider || event.requested_model != requested_model {
        return invalid_event(event, "requested route contradicts the Rust request");
    }
    if event.prompt_version != PROMPT_VERSION {
        return invalid_event(event, "prompt_version contradicts the Rust request");
    }
    if !valid_sha256(&event.prompt_sha256)
        || event.media_sha256.iter().any(|hash| !valid_sha256(hash))
    {
        return invalid_event(event, "prompt or media SHA-256 is malformed");
    }
    if event.zdr_required != zdr_required || event.allow_fallbacks == zdr_required {
        return invalid_event(event, "ZDR or fallback policy contradicts the Rust request");
    }
    if event.routed_provider.is_some() != event.routed_model.is_some() {
        return invalid_event(
            event,
            "actual route provider/model must both be present or null",
        );
    }

    match event.outcome.as_str() {
        "success" => {
            if !matches!(event.http_status, Some(200..=299))
                || event.error_class.is_some()
                || event.response_id.as_deref().is_none_or(str::is_empty)
                || event.generation_id.as_deref().is_none_or(str::is_empty)
                || event.routed_provider.as_deref().is_none_or(str::is_empty)
                || event.routed_model.as_deref().is_none_or(str::is_empty)
                || event.usage.is_none()
            {
                return invalid_event(event, "success metadata is contradictory or incomplete");
            }
            validate_usage(event, event.usage.as_ref().unwrap())?;
        }
        "http_error" => {
            if !matches!(event.http_status, Some(400..=599))
                || event.error_class.as_deref().is_none_or(str::is_empty)
                || event.response_id.is_some()
                || event.routed_provider.is_some()
                || event.usage.is_some()
            {
                return invalid_event(event, "HTTP error metadata is contradictory");
            }
        }
        "transport_error" => {
            if event.http_status.is_some()
                || event.error_class.as_deref().is_none_or(str::is_empty)
                || event.response_id.is_some()
                || event.generation_id.is_some()
                || event.routed_provider.is_some()
                || event.usage.is_some()
            {
                return invalid_event(event, "transport error metadata is contradictory");
            }
        }
        "response_error" => {
            if !matches!(event.http_status, Some(200..=299))
                || event.error_class.as_deref().is_none_or(str::is_empty)
                || event.response_id.is_some()
                || event.routed_provider.is_some()
                || event.usage.is_some()
            {
                return invalid_event(event, "response error metadata is contradictory");
            }
        }
        _ => return invalid_event(event, "outcome is not recognized"),
    }
    Ok(())
}

fn validate_usage(event: &ModelEgressEvent, usage: &ModelEgressUsage) -> Result<()> {
    if usage.prompt_tokens.is_none()
        || usage.completion_tokens.is_none()
        || usage.total_tokens.is_none()
    {
        return invalid_event(event, "success usage is incomplete");
    }
    if usage
        .cost
        .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return invalid_event(event, "usage cost is invalid");
    }
    if let (Some(prompt), Some(completion), Some(total)) = (
        usage.prompt_tokens,
        usage.completion_tokens,
        usage.total_tokens,
    ) && total < prompt.saturating_add(completion)
    {
        return invalid_event(event, "usage total_tokens is below prompt plus completion");
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_event<T>(event: &ModelEgressEvent, reason: &str) -> Result<T> {
    Err(AllieError::Worker(format!(
        "agentic model_call_audit event {} is invalid: {reason}",
        event.attempt
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> ModelEgressEvent {
        ModelEgressEvent {
            schema: "allie.model-egress-event.v1".to_string(),
            attempt: 1,
            started_at: "2026-07-21T12:00:00Z".to_string(),
            requested_provider: "openrouter".to_string(),
            requested_model: "fake-model".to_string(),
            prompt_version: PROMPT_VERSION.to_string(),
            prompt_sha256: "a".repeat(64),
            media_sha256: vec!["b".repeat(64)],
            zdr_required: true,
            allow_fallbacks: false,
            outcome: "success".to_string(),
            http_status: Some(200),
            error_class: None,
            response_id: Some("response-1".to_string()),
            generation_id: Some("generation-1".to_string()),
            routed_provider: Some("Fake Provider".to_string()),
            routed_model: Some("fake/routed-model".to_string()),
            usage: Some(ModelEgressUsage {
                prompt_tokens: Some(3),
                completion_tokens: Some(2),
                total_tokens: Some(5),
                cost: Some(0.001),
            }),
        }
    }

    fn assert_invalid(event: &ModelEgressEvent, expected: &str) {
        let error = validate_event(event, 1, "openrouter", "fake-model", true).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }

    #[test]
    fn rejects_calls_event_count_mismatch() {
        let error = validate_event_count(2, &[valid_event()]).unwrap_err();
        assert!(error.to_string().contains("event count 1"));
    }

    #[test]
    fn rejects_missing_required_event_field() {
        let mut event = serde_json::to_value(valid_event()).unwrap();
        event.as_object_mut().unwrap().remove("prompt_sha256");
        let error = serde_json::from_value::<ModelEgressEvent>(event).unwrap_err();
        assert!(error.to_string().contains("prompt_sha256"));
    }

    #[test]
    fn rejects_fabricated_failure_usage() {
        let mut event = valid_event();
        event.outcome = "http_error".to_string();
        event.http_status = Some(503);
        event.error_class = Some("http_503".to_string());
        assert_invalid(&event, "HTTP error metadata is contradictory");
    }

    #[test]
    fn rejects_incomplete_success_metadata_and_usage() {
        let mut event = valid_event();
        event.routed_provider = None;
        event.routed_model = None;
        assert_invalid(&event, "success metadata is contradictory or incomplete");

        let mut event = valid_event();
        event.usage.as_mut().unwrap().prompt_tokens = None;
        assert_invalid(&event, "success usage is incomplete");
    }

    #[test]
    fn accepts_direct_provider_receipt_without_unreported_cost() {
        let mut event = valid_event();
        event.requested_provider = "openai".to_string();
        event.routed_provider = Some("openai".to_string());
        event.zdr_required = false;
        event.allow_fallbacks = true;
        event.usage.as_mut().unwrap().cost = None;

        validate_event(&event, 1, "openai", "fake-model", false).unwrap();
    }

    #[test]
    fn rejects_malformed_timestamp_and_hashes() {
        let mut event = valid_event();
        event.started_at = "yesterday".to_string();
        assert_invalid(&event, "started_at is not RFC 3339");

        let mut event = valid_event();
        event.prompt_sha256 = "sha256:not-a-digest".to_string();
        assert_invalid(&event, "SHA-256 is malformed");
    }

    #[test]
    fn rejects_policy_contradictions() {
        let mut event = valid_event();
        event.allow_fallbacks = true;
        assert_invalid(&event, "fallback policy contradicts");

        let mut event = valid_event();
        event.requested_model = "copied-response-model".to_string();
        assert_invalid(&event, "requested route contradicts");
    }
}

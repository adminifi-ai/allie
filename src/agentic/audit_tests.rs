use super::allowlist_tests::minimal_agentic_packet;
use crate::{ModelRedactionMode, read_json_file, write_json_pretty};
use tempfile::tempdir;

#[test]
fn validated_model_egress_event_persists_in_evidence_packet() {
    let temp = tempdir().unwrap();
    let packet_path = temp.path().join("evidence.json");
    let mut packet =
        serde_json::from_value::<crate::model::EvidencePacket>(minimal_agentic_packet()).unwrap();
    let request = serde_json::json!({
        "model": {
            "provider": "openrouter",
            "model": "requested/model",
            "zdr_required": true
        }
    });
    let response = serde_json::json!({
        "calls": 1,
        "model_call_audit": [{
            "schema": "allie.model-egress-event.v1",
            "attempt": 1,
            "started_at": "2026-07-21T12:00:00Z",
            "requested_provider": "openrouter",
            "requested_model": "requested/model",
            "prompt_version": "allie.agentic.wcag-review.v1",
            "prompt_sha256": "a".repeat(64),
            "media_sha256": ["b".repeat(64)],
            "zdr_required": true,
            "allow_fallbacks": false,
            "outcome": "success",
            "http_status": 200,
            "error_class": null,
            "response_id": "response-1",
            "generation_id": "generation-1",
            "routed_provider": "Fake Provider",
            "routed_model": "actual/model",
            "usage": {
                "prompt_tokens": 3,
                "completion_tokens": 2,
                "total_tokens": 5,
                "cost": 0.001
            }
        }],
        "redaction_receipt": {
            "schema": "allie.model-redaction-receipt.v0",
            "profile": "none",
            "status": "not_applied"
        }
    });

    let route = super::audit::record_model_egress(
        &mut packet,
        &request,
        &response,
        Some(ModelRedactionMode::None),
    )
    .unwrap();
    assert_eq!(
        route,
        ("Fake Provider".to_string(), "actual/model".to_string())
    );
    write_json_pretty(&packet_path, &packet).unwrap();

    let persisted: serde_json::Value = read_json_file(&packet_path).unwrap();
    assert_eq!(persisted["policy"]["budget"]["model_calls"], 1);
    assert_eq!(
        persisted["model_egress_events"][0]["generation_id"],
        "generation-1"
    );
    assert_eq!(
        persisted["model_egress_events"][0]["prompt_sha256"],
        "a".repeat(64)
    );
}

#[test]
fn evidence_schema_requires_complete_model_egress_events() {
    let schema = std::fs::read_to_string("schemas/allie.evidence.v0.schema.json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
    assert!(
        parsed["properties"]["policy"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "model_egress_redaction")
    );
    let event = &parsed["properties"]["model_egress_events"]["items"];
    assert_eq!(
        event["properties"]["schema"]["const"],
        "allie.model-egress-event.v1"
    );
    let required = event["required"].as_array().unwrap();
    for field in [
        "attempt",
        "started_at",
        "requested_provider",
        "requested_model",
        "prompt_version",
        "prompt_sha256",
        "media_sha256",
        "zdr_required",
        "allow_fallbacks",
        "outcome",
        "http_status",
        "error_class",
        "response_id",
        "generation_id",
        "routed_provider",
        "routed_model",
        "usage",
    ] {
        assert!(
            required.contains(&serde_json::json!(field)),
            "model egress schema omitted required field {field}"
        );
    }
    assert_eq!(event["additionalProperties"], serde_json::json!(false));
}

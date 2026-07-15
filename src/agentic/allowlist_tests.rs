use super::{AGENTIC_WORKER_ENV_GUARD, run_agentic_review_with_timeout};
use crate::{FlowManifest, write_json_pretty};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::tempdir;

struct AgenticWorkerEnvGuard;

impl AgenticWorkerEnvGuard {
    fn set(worker_path: &Path, marker_path: &Path) -> Self {
        unsafe {
            std::env::set_var("ALLIE_AGENTIC_WORKER", worker_path);
            std::env::set_var("ALLIE_AGENTIC_MARKER", marker_path);
        }
        Self
    }
}

impl Drop for AgenticWorkerEnvGuard {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("ALLIE_AGENTIC_MARKER");
            std::env::remove_var("ALLIE_AGENTIC_WORKER");
        }
    }
}

fn write_marker_worker(directory: &Path, provider: &str) -> (PathBuf, PathBuf) {
    let worker_path = directory.join("marker-agentic-worker.cjs");
    let marker_path = directory.join("worker-spawned");
    fs::write(
        &worker_path,
        format!(
            r#"
const fs = require('node:fs');
fs.writeFileSync(process.env.ALLIE_AGENTIC_MARKER, 'spawned');
const responseIndex = process.argv.indexOf('--response') + 1;
fs.writeFileSync(process.argv[responseIndex], JSON.stringify({{
  schema: 'allie.agentic.response.v0',
  status: 'skipped',
  provider: '{provider}',
  model: 'test',
  calls: 0,
  redaction_receipt: {{
    schema: 'allie.model-redaction-receipt.v0',
    profile: 'none',
    status: 'not_sent'
  }},
  assessments: [],
  errors: []
}}, null, 2));
"#
        ),
    )
    .unwrap();
    (worker_path, marker_path)
}

#[test]
fn rejects_missing_redaction_before_request_write_or_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let (worker_path, marker_path) = write_marker_worker(temp.path(), "openrouter");
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.redaction = None;

    let _env = AgenticWorkerEnvGuard::set(&worker_path, &marker_path);
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();

    assert!(error.to_string().contains("model.redaction is missing"));
    assert!(!temp.path().join("agentic-request.json").exists());
    assert!(!marker_path.exists());
}

#[test]
fn rejects_off_allowlist_model_before_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let (worker_path, marker_path) = write_marker_worker(temp.path(), "openai");
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openai".to_string());
    manifest.model.base_url = Some("https://api.openai.com/v1".to_string());

    let _env = AgenticWorkerEnvGuard::set(&worker_path, &marker_path);
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();

    assert!(
        error.to_string().contains("provider openai"),
        "error should name the rejected provider: {error}"
    );
    assert!(
        !temp.path().join("agentic-request.json").exists(),
        "policy failure must happen before the worker request is written"
    );
    assert!(
        !marker_path.exists(),
        "policy failure must happen before the agentic worker is spawned"
    );
}

#[test]
fn rejects_hostile_endpoint_before_request_write_or_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let (worker_path, marker_path) = write_marker_worker(temp.path(), "openrouter");
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openrouter".to_string());
    manifest.model.base_url = Some("https://attacker.invalid/api/v1".to_string());

    let _env = AgenticWorkerEnvGuard::set(&worker_path, &marker_path);
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("resolved base_url https://attacker.invalid/api/v1 is not allowed"),
        "error should identify the rejected endpoint binding: {error}"
    );
    assert!(
        !temp.path().join("agentic-request.json").exists(),
        "endpoint policy failure must happen before the worker request is written"
    );
    assert!(
        !marker_path.exists(),
        "endpoint policy failure must happen before the agentic worker is spawned"
    );
}

#[test]
fn rejects_empty_allowlist_before_request_write_or_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let (worker_path, marker_path) = write_marker_worker(temp.path(), "openrouter");
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.enabled = true;
    manifest.model.provider_allowlist = Vec::new();
    manifest.model.provider = Some("openrouter".to_string());
    manifest.model.base_url = Some("https://attacker.invalid/api/v1".to_string());

    let _env = AgenticWorkerEnvGuard::set(&worker_path, &marker_path);
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();

    assert!(
        error.to_string().contains("provider_allowlist is empty"),
        "empty provider_allowlist must be reported as model-policy-incomplete: {error}"
    );
    assert!(
        !temp.path().join("agentic-request.json").exists(),
        "policy failure must happen before the worker request is written"
    );
    assert!(
        !marker_path.exists(),
        "policy failure must happen before the agentic worker is spawned"
    );
}

#[test]
fn evidence_packet_rejects_missing_required_model_egress_policy() {
    let mut packet = minimal_agentic_packet();
    packet["policy"]
        .as_object_mut()
        .unwrap()
        .remove("model_egress_redaction");

    assert!(serde_json::from_value::<crate::model::EvidencePacket>(packet).is_err());
}

fn minimal_agentic_packet() -> serde_json::Value {
    serde_json::json!({
        "schema": "allie.evidence.v0",
        "summary": {
            "status": "pass",
            "exit_code": 0,
            "deterministic_failures": 0,
            "scripted_failures": 0,
            "infrastructure_failures": 0,
            "states_captured": 1,
            "failure_class": null
        },
        "run": {
            "id": "agentic-allowlist-test",
            "started_at": "2026-06-30T00:00:00Z",
            "finished_at": "2026-06-30T00:00:01Z",
            "allie_version": env!("CARGO_PKG_VERSION"),
            "git_sha": "test",
            "git_branch": "test",
            "ci_provider": null,
            "actor": "test"
        },
        "target": {
            "base_url": "http://127.0.0.1:1",
            "environment": "test",
            "app_name": "Allie test",
            "auth_profile": "none",
            "credential_provider": {
                "provider": "none",
                "env": null,
                "required": false,
                "status": "not_required"
            },
            "flow_manifest": "examples/autonomous-workbench-agentic.yml"
        },
        "policy": {
            "profile": "wcag22-aa",
            "blocking_classes": ["deterministic"],
            "worker_timeout_ms": 30000,
            "model_provider_allowlist": ["openrouter"],
            "model_status": "enabled",
            "zdr_required": true,
            "model_egress_redaction": "none",
            "redaction_profile": "not_redacted_local_fixture",
            "budget": { "model_calls": 0, "max_states": 1 }
        },
        "coverage": {
            "routes_visited": ["/"],
            "surfaces_discovered": ["home"],
            "flows_exercised": ["allowlist-test"],
            "states_captured": ["home"],
            "state_metadata": [],
            "standards_obligations_evaluated": ["wcag22-aa:2.4.7-focus-visible"],
            "obligations_not_tested": [],
            "profile_human_review_scope": ["wcag22-aa:2.4.7-focus-visible"]
        },
        "artifacts": [],
        "findings": [],
        "verdicts": [{
            "obligation": "wcag22-aa:2.4.7-focus-visible",
            "status": "needs_review",
            "confidence": "needs_human",
            "evidence_class": "human",
            "source": "test",
            "affected_states": ["home"],
            "finding_refs": []
        }],
        "waivers": [],
        "agentic_assessments": [],
        "replay": {
            "command": "cargo run --locked -- run --manifest examples/autonomous-workbench-agentic.yml --out test",
            "manifest_path": "examples/autonomous-workbench-agentic.yml",
            "environment_requirements": [],
            "credential_profile": "none",
            "browser": {
                "viewport": { "width": 1280, "height": 900 },
                "color_scheme": "light",
                "reduced_motion": "reduce",
                "locale": "en-US",
                "zoom": 1.0
            },
            "seed_data": [],
            "known_nondeterminism": []
        }
    })
}

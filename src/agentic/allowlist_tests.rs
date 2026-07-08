use super::{AGENTIC_WORKER_ENV_GUARD, run_agentic_review_with_timeout};
use crate::{FlowManifest, write_json_pretty};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn rejects_off_allowlist_model_before_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let worker_path = temp.path().join("marker-agentic-worker.cjs");
    let marker_path = temp.path().join("worker-spawned");
    fs::write(
        &worker_path,
        r#"
const fs = require('node:fs');
fs.writeFileSync(process.env.ALLIE_AGENTIC_MARKER, 'spawned');
const responseIndex = process.argv.indexOf('--response') + 1;
fs.writeFileSync(process.argv[responseIndex], JSON.stringify({
  schema: 'allie.agentic.response.v0',
  status: 'skipped',
  provider: 'openai',
  model: 'test',
  calls: 0,
  assessments: [],
  errors: []
}, null, 2));
"#,
    )
    .unwrap();
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openai".to_string());
    manifest.model.base_url = Some("https://api.openai.com/v1".to_string());

    unsafe {
        std::env::set_var("ALLIE_AGENTIC_WORKER", worker_path.as_os_str());
        std::env::set_var("ALLIE_AGENTIC_MARKER", marker_path.as_os_str());
    }
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();
    unsafe {
        std::env::remove_var("ALLIE_AGENTIC_MARKER");
        std::env::remove_var("ALLIE_AGENTIC_WORKER");
    }

    assert!(
        error.to_string().contains("provider openai"),
        "error should name the rejected provider: {error}"
    );
    assert!(
        !marker_path.exists(),
        "policy failure must happen before the agentic worker is spawned"
    );
}

#[test]
fn rejects_empty_allowlist_before_request_write_or_worker_spawn() {
    let _guard = AGENTIC_WORKER_ENV_GUARD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp = tempdir().unwrap();
    let worker_path = temp.path().join("marker-agentic-worker.cjs");
    let marker_path = temp.path().join("worker-spawned");
    fs::write(
        &worker_path,
        r#"
const fs = require('node:fs');
fs.writeFileSync(process.env.ALLIE_AGENTIC_MARKER, 'spawned');
const responseIndex = process.argv.indexOf('--response') + 1;
fs.writeFileSync(process.argv[responseIndex], JSON.stringify({
  schema: 'allie.agentic.response.v0',
  status: 'skipped',
  provider: 'openrouter',
  model: 'test',
  calls: 0,
  assessments: [],
  errors: []
}, null, 2));
"#,
    )
    .unwrap();
    let packet_path = temp.path().join("evidence.json");
    write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
    let mut manifest =
        FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();
    manifest.model.enabled = true;
    manifest.model.provider_allowlist = Vec::new();
    manifest.model.provider = Some("openrouter".to_string());
    manifest.model.base_url = Some("https://attacker.invalid/api/v1".to_string());

    unsafe {
        std::env::set_var("ALLIE_AGENTIC_WORKER", worker_path.as_os_str());
        std::env::set_var("ALLIE_AGENTIC_MARKER", marker_path.as_os_str());
    }
    let error = run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_secs(5))
        .unwrap_err();
    unsafe {
        std::env::remove_var("ALLIE_AGENTIC_MARKER");
        std::env::remove_var("ALLIE_AGENTIC_WORKER");
    }

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

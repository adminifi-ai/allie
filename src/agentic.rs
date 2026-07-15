use crate::model::{AgenticAssessmentRecord, AgenticMediaRef, ArtifactPolicy, EvidencePacket};
use crate::standards::{wcag22_success_criteria, wcag22_success_criterion_ids};
use crate::worker_runtime;
use crate::{
    AllieError, FlowManifest, Result, artifact_for_path, now_utc, read_json_file, write_json_pretty,
};
use std::collections::BTreeSet;
use std::fmt::{self, Display};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;
use wait_timeout::ChildExt;

const AGENTIC_WORKER_TIMEOUT: Duration = Duration::from_secs(300);

mod redaction;
use redaction::accepted_redaction_receipt;

#[cfg(test)]
static AGENTIC_WORKER_ENV_GUARD: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub(crate) struct AgenticReviewSummary {
    pub(crate) criteria: usize,
    pub(crate) calls: u64,
    pub(crate) status: AgenticReviewOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgenticReviewOutcome {
    Skipped,
    Completed,
    Degraded,
}

impl Display for AgenticReviewOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Skipped => "skipped",
            Self::Completed => "ok",
            Self::Degraded => "degraded",
        })
    }
}

#[derive(serde::Serialize)]
struct AgenticReviewSurface {
    id: String,
    route: String,
    url: String,
    title: String,
}

fn agentic_review_surfaces(packet: &EvidencePacket) -> Vec<AgenticReviewSurface> {
    packet
        .coverage
        .state_metadata
        .iter()
        .map(|state| AgenticReviewSurface {
            id: state.id.clone(),
            route: state.route.clone(),
            url: state.url.clone(),
            title: state.title.clone(),
        })
        .collect()
}

/// Run model review over needs-review criteria and attach captured evidence.
/// Unavailable or inconclusive results stay neutral; protocol failure errors.
pub(crate) fn run_agentic_review(
    manifest: &FlowManifest,
    packet_path: &Path,
) -> Result<AgenticReviewSummary> {
    run_agentic_review_with_timeout(manifest, packet_path, AGENTIC_WORKER_TIMEOUT)
}

fn run_agentic_review_with_timeout(
    manifest: &FlowManifest,
    packet_path: &Path,
    timeout: Duration,
) -> Result<AgenticReviewSummary> {
    let mut packet: EvidencePacket = read_json_file(packet_path)?;

    let success_ids = wcag22_success_criterion_ids();
    let obligations = packet
        .verdicts
        .iter()
        .filter(|verdict| verdict.status == "needs_review")
        .map(|verdict| verdict.obligation.clone())
        .filter(|obligation| success_ids.contains(obligation))
        .collect::<BTreeSet<_>>();
    if obligations.is_empty() {
        return Ok(AgenticReviewSummary {
            criteria: 0,
            calls: 0,
            status: AgenticReviewOutcome::Skipped,
        });
    }
    manifest.enforce_model_provider_allowlist()?;

    let criteria = obligations
        .iter()
        .filter_map(|obligation| {
            let criterion = wcag22_success_criteria()
                .into_iter()
                .find(|criterion| criterion["obligation"].as_str() == Some(obligation))?;
            Some(serde_json::json!({
                "obligation": obligation,
                "num": criterion["num"],
                "handle": criterion["handle"],
                "level": criterion["level"],
                "principle": criterion["principle"],
            }))
        })
        .collect::<Vec<_>>();

    let run_dir = fs::canonicalize(packet_path.parent().unwrap_or_else(|| Path::new(".")))
        .unwrap_or_else(|_| {
            packet_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });
    let artifacts_dir = run_dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).map_err(|source| AllieError::Io {
        context: format!("create agentic artifacts dir {}", artifacts_dir.display()),
        source,
    })?;
    let target_base_url = if manifest.target.kind == "local_fixture" {
        manifest.target.base_url.clone()
    } else {
        packet
            .target
            .base_url
            .clone()
            .or_else(|| manifest.target.base_url.clone())
    };

    let model_route = manifest.model.resolved_route();
    let request = serde_json::json!({
        "schema": "allie.agentic.request.v0",
        "target": {
            "base_url": target_base_url,
            "fixture_dir": manifest.target.fixture_dir.as_ref().map(|dir| dir.to_string_lossy().to_string()),
        },
        "browser": {
            "viewport": { "width": manifest.browser.viewport.width, "height": manifest.browser.viewport.height },
            "color_scheme": manifest.browser.color_scheme,
            "reduced_motion": manifest.browser.reduced_motion,
            "locale": manifest.browser.locale,
        },
        "model": {
            "provider": model_route.provider,
            "model": model_route.model,
            "api_key_env": model_route.api_key_env,
            "base_url": model_route.base_url,
            "max_calls": manifest.model.max_model_calls.unwrap_or(4),
            "reasoning_effort": manifest.model.reasoning_effort.clone(),
            "redaction": manifest.model.redaction,
        },
        "artifacts_dir": artifacts_dir.to_string_lossy(),
        "surfaces": agentic_review_surfaces(&packet),
        "criteria": criteria,
    });

    let request_path = run_dir.join("agentic-request.json");
    let response_path = run_dir.join("agentic-response.json");
    write_json_pretty(&request_path, &request)?;

    let script = worker_runtime::agentic_worker_script().map_err(AllieError::Worker)?;
    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg("--request")
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    worker_runtime::apply_worker_environment(&mut command, &script);
    let mut child = command
        .spawn()
        .map_err(|source| AllieError::Worker(format!("spawn agentic worker: {source}")))?;
    let status = child
        .wait_timeout(timeout)
        .map_err(|source| AllieError::Worker(format!("wait for agentic worker: {source}")))?;
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AllieError::Worker(format!(
            "agentic worker timed out after {} ms",
            timeout.as_millis()
        )));
    };
    let output = child
        .wait_with_output()
        .map_err(|source| AllieError::Worker(format!("collect agentic worker output: {source}")))?;
    if !response_path.exists() {
        return Err(AllieError::Worker(format!(
            "agentic worker produced no response: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let response: serde_json::Value = read_json_file(&response_path)?;
    let redaction_receipt = accepted_redaction_receipt(&response, manifest.model.redaction)?;
    let outcome = agentic_response_outcome(&response, status.success())?;
    let timestamp = now_utc();
    let policy = ArtifactPolicy {
        redaction_status: "not_redacted_local".to_string(),
        retention_class: "local_review".to_string(),
    };
    let mut seen_artifacts = packet
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<BTreeSet<_>>();

    let provider = response["provider"]
        .as_str()
        .unwrap_or("openrouter")
        .to_string();
    let model = response["model"].as_str().unwrap_or_default().to_string();
    let fail_precision_passed = agentic_fail_precision_passed(&response);
    packet.policy.model_egress_redaction = Some(redaction_receipt.profile);
    let empty = Vec::new();
    for assessment in response["assessments"].as_array().unwrap_or(&empty) {
        let Some(obligation) = assessment["obligation"].as_str() else {
            continue;
        };
        let media = assessment["media"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let rel = item["path"].as_str()?;
                        Some(AgenticMediaRef {
                            kind: item["kind"].as_str().unwrap_or("screenshot").to_string(),
                            caption: item["caption"].as_str().unwrap_or_default().to_string(),
                            path: format!("artifacts/{rel}"),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for media_ref in &media {
            if seen_artifacts.insert(media_ref.path.clone()) {
                let absolute = run_dir.join(&media_ref.path);
                if let Ok(artifact) = artifact_for_path(
                    &agentic_artifact_id(&media_ref.path),
                    agentic_artifact_type(&media_ref.kind),
                    &run_dir,
                    &absolute,
                    None,
                    "allie-agentic-gateway",
                    &policy,
                    timestamp,
                ) {
                    packet.artifacts.push(artifact);
                }
            }
        }
        let verdict_str = assessment["verdict"]
            .as_str()
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "inconclusive".to_string());
        let confidence_str = assessment["confidence"]
            .as_str()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "not_observed".to_string());
        packet.agentic_assessments.push(AgenticAssessmentRecord {
            obligation: obligation.to_string(),
            assessment: verdict_str.clone(),
            rationale: assessment["rationale"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            reviewer_guidance: assessment["reviewer_guidance"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            confidence: confidence_str.clone(),
            provider: provider.clone(),
            model: model.clone(),
            media,
        });

        // Promote the agentic verdict to the criterion status, marked with an
        // "agentic" evidence class so the report renders it with an asterisk.
        // Never fabricate: only an explicit pass/fail promotes; "inconclusive"
        // or a missing verdict leaves the criterion at needs_review.
        if let Some(new_status) = agentic_promoted_status(&verdict_str, fail_precision_passed) {
            for verdict in packet.verdicts.iter_mut().filter(|verdict| {
                verdict.obligation == obligation && verdict.status == "needs_review"
            }) {
                verdict.status = new_status.to_string();
                verdict.evidence_class = "agentic".to_string();
                // Store the bare confidence token ("high"/"medium"/"low") so the
                // criterion footer and the AI verdict block render it identically;
                // evidence_class = "agentic" already carries the provenance.
                verdict.confidence = confidence_str.clone();
                verdict.source = format!("allie-agentic-review:{model}");
            }
        }
    }

    write_json_pretty(packet_path, &packet)?;
    Ok(AgenticReviewSummary {
        criteria: obligations.len(),
        calls: response["calls"].as_u64().unwrap_or_default(),
        status: outcome,
    })
}

fn agentic_response_outcome(
    response: &serde_json::Value,
    worker_success: bool,
) -> Result<AgenticReviewOutcome> {
    let status = response["status"].as_str().unwrap_or("unknown");
    match status {
        "ok" if worker_success => Ok(AgenticReviewOutcome::Completed),
        "degraded" if worker_success => Ok(AgenticReviewOutcome::Degraded),
        "skipped" if worker_success => Ok(AgenticReviewOutcome::Skipped),
        "error" => Err(AllieError::Worker(format!(
            "agentic worker returned error: {}",
            agentic_response_errors(response)
        ))),
        other if !worker_success => Err(AllieError::Worker(format!(
            "agentic worker exited unsuccessfully with response status {other}: {}",
            agentic_response_errors(response)
        ))),
        other => Err(AllieError::Worker(format!(
            "agentic worker returned unknown response status {other}"
        ))),
    }
}

fn agentic_response_errors(response: &serde_json::Value) -> String {
    response["errors"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| "no worker error details recorded".to_string())
}

fn agentic_fail_precision_passed(response: &serde_json::Value) -> bool {
    let gate = &response["precision_gate"];
    gate["status"].as_str() == Some("pass")
        && gate["labeled_cases"].as_u64().unwrap_or_default() > 0
        && gate["expected_pass_cases"].as_u64().unwrap_or_default() > 0
        && gate["fail_false_positives"].as_u64() == Some(0)
}

/// Map an agentic verdict string to the criterion status it may promote to.
/// PASS can promote because it does not create a false-failure release block.
/// FAIL promotes only after the worker's labeled precision gate proves zero
/// false-positive FAILs; otherwise it remains attached review context and the
/// criterion stays at needs_review. Anything else (notably "inconclusive" or an
/// empty/unknown value) returns None — the agentic reviewer never fabricates a
/// verdict.
fn agentic_promoted_status(verdict: &str, fail_precision_passed: bool) -> Option<&'static str> {
    match verdict.trim().to_lowercase().as_str() {
        "pass" => Some("pass"),
        "fail" if fail_precision_passed => Some("fail"),
        _ => None,
    }
}

fn agentic_artifact_id(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| path.to_string())
}

fn agentic_artifact_type(kind: &str) -> &'static str {
    match kind {
        "clip" | "video" | "video_clip" => "video_clip",
        _ => "screenshot",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn agentic_promoted_status_only_promotes_fail_with_precision_gate() {
        assert_eq!(agentic_promoted_status("pass", false), Some("pass"));
        assert_eq!(agentic_promoted_status("fail", false), None);
        assert_eq!(agentic_promoted_status("fail", true), Some("fail"));
        assert_eq!(agentic_promoted_status("FAIL", true), Some("fail"));
        // Inconclusive / empty / unknown never promote — no fabricated verdicts.
        assert_eq!(agentic_promoted_status("inconclusive", true), None);
        assert_eq!(agentic_promoted_status("", true), None);
        assert_eq!(agentic_promoted_status("needs_human", true), None);
    }

    #[test]
    fn agentic_fail_precision_gate_requires_zero_false_positives() {
        let passing_gate = serde_json::json!({
            "precision_gate": {
                "status": "pass",
                "labeled_cases": 2,
                "expected_pass_cases": 1,
                "fail_false_positives": 0
            }
        });
        assert!(agentic_fail_precision_passed(&passing_gate));

        let failing_gate = serde_json::json!({
            "precision_gate": {
                "status": "fail",
                "labeled_cases": 2,
                "expected_pass_cases": 1,
                "fail_false_positives": 1
            }
        });
        assert!(!agentic_fail_precision_passed(&failing_gate));

        let missing_labels = serde_json::json!({
            "precision_gate": {
                "status": "pass",
                "labeled_cases": 0,
                "expected_pass_cases": 0,
                "fail_false_positives": 0
            }
        });
        assert!(!agentic_fail_precision_passed(&missing_labels));

        let no_expected_pass_cases = serde_json::json!({
            "precision_gate": {
                "status": "pass",
                "labeled_cases": 1,
                "expected_pass_cases": 0,
                "fail_false_positives": 0
            }
        });
        assert!(!agentic_fail_precision_passed(&no_expected_pass_cases));

        let missing_gate = serde_json::json!({});
        assert!(!agentic_fail_precision_passed(&missing_gate));
    }

    #[test]
    fn agentic_response_outcome_rejects_worker_error_status() {
        let response = serde_json::json!({
            "status": "error",
            "errors": ["target could not be opened"]
        });

        let error = agentic_response_outcome(&response, false).unwrap_err();
        assert!(error.to_string().contains("target could not be opened"));
    }

    #[test]
    fn agentic_response_outcome_accepts_only_successful_terminal_statuses() {
        for status in ["ok", "degraded", "skipped"] {
            let response = serde_json::json!({ "status": status });

            assert!(
                agentic_response_outcome(&response, true).is_ok(),
                "{status} should be accepted when worker exit succeeded"
            );

            let error = agentic_response_outcome(&response, false).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("agentic worker exited unsuccessfully"),
                "{status} with a nonzero worker exit must fail closed"
            );
        }

        let unknown = serde_json::json!({ "status": "maybe" });
        let error = agentic_response_outcome(&unknown, true).unwrap_err();
        assert!(error.to_string().contains("unknown response status maybe"));
    }

    #[test]
    fn agentic_review_request_includes_packet_review_surfaces() {
        let mut packet = minimal_agentic_packet();
        packet["coverage"]["state_metadata"] = serde_json::json!([
            {
                "id": "home",
                "route": "/",
                "url": "http://127.0.0.1:1234/",
                "title": "Home",
                "http_status": 200,
                "keyboard_focus_order": ["Home", "Settings"],
                "console_errors": [],
                "network_errors": [],
                "state_errors": [],
                "features": null
            },
            {
                "id": "settings",
                "route": "/settings.html",
                "url": "http://127.0.0.1:1234/settings.html",
                "title": "Settings",
                "http_status": 200,
                "keyboard_focus_order": ["Email", "Save settings"],
                "console_errors": [],
                "network_errors": [],
                "state_errors": [],
                "features": null
            }
        ]);
        let packet: EvidencePacket = serde_json::from_value(packet).unwrap();

        let surfaces = agentic_review_surfaces(&packet);

        assert_eq!(surfaces.len(), 2);
        assert_eq!(surfaces[0].id, "home");
        assert_eq!(surfaces[0].route, "/");
        assert_eq!(surfaces[1].id, "settings");
        assert_eq!(surfaces[1].route, "/settings.html");
    }

    #[test]
    fn run_agentic_review_kills_worker_on_timeout_before_collecting_output() {
        let _guard = AGENTIC_WORKER_ENV_GUARD
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let temp = tempdir().unwrap();
        let worker_path = temp.path().join("sleeping-agentic-worker.cjs");
        fs::write(&worker_path, "setTimeout(() => process.exit(0), 250);\n").unwrap();
        let packet_path = temp.path().join("evidence.json");
        write_json_pretty(&packet_path, &minimal_agentic_packet()).unwrap();
        let manifest =
            FlowManifest::load(Path::new("examples/autonomous-workbench-agentic.yml")).unwrap();

        unsafe {
            std::env::set_var("ALLIE_AGENTIC_WORKER", worker_path.as_os_str());
        }
        let error =
            run_agentic_review_with_timeout(&manifest, &packet_path, Duration::from_millis(1))
                .unwrap_err();
        unsafe {
            std::env::remove_var("ALLIE_AGENTIC_WORKER");
        }

        assert!(error.to_string().contains("timed out after 1 ms"));
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
                "id": "agentic-timeout-test",
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
                "flows_exercised": ["timeout-test"],
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
}

#[cfg(test)]
mod allowlist_tests;

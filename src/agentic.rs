use crate::{
    AgenticAssessmentRecord, AgenticMediaRef, AllieError, ArtifactPolicy, EvidencePacket,
    FlowManifest, Result, artifact_for_path, now_utc, read_json_file, wcag22_success_criteria,
    wcag22_success_criterion_ids, write_json_pretty,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

#[derive(Debug)]
pub(crate) struct AgenticReviewSummary {
    pub(crate) criteria: usize,
    pub(crate) calls: u64,
    pub(crate) status: String,
}

fn agentic_worker_script() -> PathBuf {
    std::env::var_os("ALLIE_AGENTIC_WORKER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workers/agentic/review.mjs")
        })
}

fn agentic_model_setting(value: &Option<String>, fallback: &str) -> String {
    value
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

/// Run the agentic (vision-model) review over the criteria a run left as
/// needs_review, fold the model's assessments + captured media into the
/// evidence packet, and promote each committed verdict to the criterion's
/// pass/fail status — marked with the "agentic" evidence class so the report
/// renders it with an asterisk (a reviewer judgment, not a machine-proven
/// result). Best-effort: returns an error the caller can log, and never
/// fabricates a verdict — an "inconclusive" or unavailable result leaves the
/// criterion at needs_review.
pub(crate) fn run_agentic_review(
    manifest: &FlowManifest,
    packet_path: &Path,
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
            status: "skipped".to_string(),
        });
    }

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

    let request = serde_json::json!({
        "schema": "allie.agentic.request.v0",
        "target": {
            "base_url": packet.target.base_url.clone().or_else(|| manifest.target.base_url.clone()),
            "fixture_dir": manifest.target.fixture_dir.as_ref().map(|dir| dir.to_string_lossy().to_string()),
        },
        "browser": {
            "viewport": { "width": manifest.browser.viewport.width, "height": manifest.browser.viewport.height },
            "color_scheme": manifest.browser.color_scheme,
            "reduced_motion": manifest.browser.reduced_motion,
            "locale": manifest.browser.locale,
        },
        "model": {
            "provider": agentic_model_setting(&manifest.model.provider, "openrouter"),
            "model": agentic_model_setting(&manifest.model.model, "google/gemini-3.5-flash"),
            "api_key_env": agentic_model_setting(&manifest.model.api_key_env, "OPENROUTER_API_KEY"),
            "base_url": agentic_model_setting(&manifest.model.base_url, "https://openrouter.ai/api/v1"),
            "max_calls": manifest.model.max_model_calls.unwrap_or(4),
            "reasoning_effort": manifest.model.reasoning_effort.clone(),
        },
        "artifacts_dir": artifacts_dir.to_string_lossy(),
        "criteria": criteria,
    });

    let request_path = run_dir.join("agentic-request.json");
    let response_path = run_dir.join("agentic-response.json");
    write_json_pretty(&request_path, &request)?;

    let script = agentic_worker_script();
    if !script.exists() {
        return Err(AllieError::Worker(format!(
            "agentic worker script not found at {}",
            script.display()
        )));
    }
    let mut child = Command::new("node")
        .arg(&script)
        .arg("--request")
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| AllieError::Worker(format!("spawn agentic worker: {source}")))?;
    let status = child
        .wait_timeout(Duration::from_millis(300_000))
        .map_err(|source| AllieError::Worker(format!("wait for agentic worker: {source}")))?;
    let output = child
        .wait_with_output()
        .map_err(|source| AllieError::Worker(format!("collect agentic worker output: {source}")))?;
    if status.is_none() {
        return Err(AllieError::Worker(
            "agentic worker timed out after 300s".to_string(),
        ));
    }
    if !response_path.exists() {
        return Err(AllieError::Worker(format!(
            "agentic worker produced no response: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let response: serde_json::Value = read_json_file(&response_path)?;
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
        if let Some(new_status) = agentic_promoted_status(&verdict_str) {
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
        status: response["status"].as_str().unwrap_or("unknown").to_string(),
    })
}

/// Map an agentic verdict string to the criterion status it may promote to.
/// Only an explicit "pass"/"fail" promotes; anything else (notably
/// "inconclusive" or an empty/unknown value) returns None so the criterion
/// stays at needs_review — the agentic reviewer never fabricates a verdict.
pub(crate) fn agentic_promoted_status(verdict: &str) -> Option<&'static str> {
    match verdict.trim().to_lowercase().as_str() {
        "pass" => Some("pass"),
        "fail" => Some("fail"),
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

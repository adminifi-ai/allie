use crate::model::{
    EvidencePacket, GithubCheckOutput, GithubCheckPayload, ReleaseBlockingSummary,
    ReleaseDecisionPacket, ReleasePolicySummary, ReleaseReviewSummary,
};
use crate::{AllieError, EVIDENCE_SCHEMA, ExitClass, ReleaseOptions, Result, escape_html};
use chrono::{DateTime, Utc};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) struct ReleaseProjection {
    pub(crate) summary: ReleaseDecisionPacket,
    pub(crate) github_check: GithubCheckPayload,
    pub(crate) exit_class: ExitClass,
}

pub(crate) fn read_release_packet(packet_path: &Path) -> Result<EvidencePacket> {
    let packet_text = fs::read_to_string(packet_path).map_err(|source| AllieError::Io {
        context: format!("read evidence packet {}", packet_path.display()),
        source,
    })?;
    let packet = serde_json::from_str::<EvidencePacket>(&packet_text).map_err(|source| {
        AllieError::Json {
            context: format!("parse evidence packet {}", packet_path.display()),
            source,
        }
    })?;
    validate_release_packet(&packet)?;
    Ok(packet)
}

pub(crate) fn validate_release_packet(packet: &EvidencePacket) -> Result<()> {
    if packet.schema != EVIDENCE_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "invalid evidence packet schema {}; expected {EVIDENCE_SCHEMA}",
            packet.schema
        )));
    }

    if !matches!(packet.summary.status.as_str(), "pass" | "fail" | "error") {
        return Err(AllieError::InvalidManifest(format!(
            "invalid evidence packet status {}; expected pass, fail, or error",
            packet.summary.status
        )));
    }

    Ok(())
}

pub(crate) fn project_release_decision(
    packet: &EvidencePacket,
    options: &ReleaseOptions,
) -> ReleaseProjection {
    let review_needed = packet
        .verdicts
        .iter()
        .filter(|verdict| verdict.status == "needs_review")
        .map(|verdict| verdict.obligation.clone())
        .collect::<Vec<_>>();
    let not_tested = packet
        .verdicts
        .iter()
        .filter(|verdict| verdict.status == "not_tested")
        .map(|verdict| verdict.obligation.clone())
        .collect::<Vec<_>>();
    let model_findings_non_blocking = packet
        .findings
        .iter()
        .filter(|finding| finding.evidence_class == "agentic")
        .count();
    let evidence_artifacts = packet
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_type.clone())
        .collect::<Vec<_>>();

    let captured_states = packet
        .coverage
        .states_captured
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let discovered_surfaces = packet
        .coverage
        .surfaces_discovered
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let missing_required_evidence = options
        .changed_surfaces
        .iter()
        .filter(|surface| {
            !captured_states.contains(surface.as_str())
                && !discovered_surfaces.contains(surface.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();

    let stale_evidence = packet_is_stale(packet, options.stale_after_days);
    let expired_waivers = expired_touched_waivers(&packet.waivers, &options.changed_surfaces);
    let invalid_waivers = invalid_touched_waivers(&packet.waivers, &options.changed_surfaces);
    let has_blocker = packet.summary.status == "fail"
        || packet.summary.status == "error"
        || packet.summary.deterministic_failures > 0
        || packet.summary.scripted_failures > 0
        || packet.summary.infrastructure_failures > 0
        || !missing_required_evidence.is_empty()
        || !expired_waivers.is_empty()
        || !invalid_waivers.is_empty();
    let status = if has_blocker {
        "blocked"
    } else if stale_evidence
        || !review_needed.is_empty()
        || !not_tested.is_empty()
        || model_findings_non_blocking > 0
    {
        "needs_review"
    } else {
        "approved"
    };
    let conclusion = if has_blocker {
        "failure"
    } else if status == "needs_review" {
        "neutral"
    } else {
        "success"
    };
    let exit_class = if has_blocker {
        ExitClass::BlockingFinding
    } else {
        ExitClass::Success
    };

    let summary = ReleaseDecisionPacket {
        schema: "allie.release-decision.v0".to_string(),
        status: status.to_string(),
        packet_path: options.packet_path.to_string_lossy().to_string(),
        packet_run_id: packet.run.id.clone(),
        changed_surfaces: options.changed_surfaces.clone(),
        blocking: ReleaseBlockingSummary {
            deterministic_failures: packet.summary.deterministic_failures,
            scripted_failures: packet.summary.scripted_failures,
            infrastructure_failures: packet.summary.infrastructure_failures,
            missing_required_evidence,
            expired_waivers,
            invalid_waivers,
        },
        review: ReleaseReviewSummary { stale_evidence },
        review_needed_obligations: review_needed,
        not_tested_obligations: not_tested,
        model_findings_non_blocking,
        evidence_artifacts,
        policy: ReleasePolicySummary {
            model_status: packet.policy.model_status.clone(),
            model_provider_allowlist: packet.policy.model_provider_allowlist.clone(),
            zdr_required: packet.policy.zdr_required,
        },
    };
    let summary_text = release_summary_text(&summary);
    let github_check = GithubCheckPayload {
        name: "Allie accessibility evidence".to_string(),
        conclusion: conclusion.to_string(),
        output: GithubCheckOutput {
            title: format!("Allie release decision: {status}"),
            summary: summary_text.clone(),
            text: summary_text,
        },
    };

    ReleaseProjection {
        summary,
        github_check,
        exit_class,
    }
}

fn packet_is_stale(packet: &EvidencePacket, stale_after_days: i64) -> bool {
    let Ok(finished_at) = DateTime::parse_from_rfc3339(&packet.run.finished_at) else {
        return true;
    };
    let age = Utc::now().signed_duration_since(finished_at.with_timezone(&Utc));
    age.num_days() > stale_after_days
}

fn expired_touched_waivers(
    waivers: &[serde_json::Value],
    changed_surfaces: &[String],
) -> Vec<serde_json::Value> {
    let changed = changed_surfaces.iter().cloned().collect::<BTreeSet<_>>();
    waivers
        .iter()
        .filter(|waiver| waiver_is_expired_for_changed_surface(waiver, &changed))
        .cloned()
        .collect()
}

fn waiver_is_expired_for_changed_surface(
    waiver: &serde_json::Value,
    changed_surfaces: &BTreeSet<String>,
) -> bool {
    if !waiver_touches_changed_surface(waiver, changed_surfaces) {
        return false;
    }
    let Some(expires_at) = waiver["expires_at"].as_str() else {
        return false;
    };
    let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
        return true;
    };
    expires_at.with_timezone(&Utc) < Utc::now()
}

fn invalid_touched_waivers(
    waivers: &[serde_json::Value],
    changed_surfaces: &[String],
) -> Vec<serde_json::Value> {
    let changed = changed_surfaces.iter().cloned().collect::<BTreeSet<_>>();
    waivers
        .iter()
        .filter(|waiver| {
            waiver_touches_changed_surface(waiver, &changed)
                && !waiver_has_required_release_metadata(waiver)
        })
        .cloned()
        .collect()
}

fn waiver_touches_changed_surface(
    waiver: &serde_json::Value,
    changed_surfaces: &BTreeSet<String>,
) -> bool {
    if changed_surfaces.is_empty() {
        return true;
    }
    let Some(surface) = waiver["surface"].as_str() else {
        return true;
    };
    surface.trim().is_empty() || changed_surfaces.contains(surface)
}

fn waiver_has_required_release_metadata(waiver: &serde_json::Value) -> bool {
    let Some(surface) = waiver["surface"].as_str() else {
        return false;
    };
    if surface.trim().is_empty() {
        return false;
    }
    let Some(status) = waiver["status"].as_str() else {
        return false;
    };
    if !matches!(status, "waived" | "risk_accepted") {
        return false;
    }
    let Some(expires_at) = waiver["expires_at"].as_str() else {
        return false;
    };
    if DateTime::parse_from_rfc3339(expires_at).is_err() {
        return false;
    }
    let provenance_ok = waiver["provenance"]
        .as_str()
        .map(|value| !value.trim().is_empty())
        .or_else(|| {
            waiver["provenance"]
                .as_object()
                .map(|value| !value.is_empty())
        })
        .unwrap_or(false);
    let packet_ref_ok = waiver["packet_ref"]
        .as_str()
        .map(|value| !value.trim().is_empty())
        .or_else(|| {
            waiver["packet_refs"].as_array().map(|values| {
                values
                    .iter()
                    .any(|value| value.as_str().is_some_and(|item| !item.trim().is_empty()))
            })
        })
        .unwrap_or(false);

    provenance_ok && packet_ref_ok
}

pub(crate) fn release_summary_text(summary: &ReleaseDecisionPacket) -> String {
    format!(
        "status={} deterministic_failures={} scripted_failures={} infrastructure_failures={} review_needed={} not_tested={}",
        summary.status,
        summary.blocking.deterministic_failures,
        summary.blocking.scripted_failures,
        summary.blocking.infrastructure_failures,
        summary.review_needed_obligations.len(),
        summary.not_tested_obligations.len()
    )
}

pub(crate) fn render_release_report(summary: &ReleaseDecisionPacket) -> String {
    let text = escape_html(&release_summary_text(summary));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Allie Release Decision</title>
  <style>
    body {{ margin: 0; font: 16px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; color: #151719; background: #f5f7fa; }}
    main {{ width: min(100% - 40px, 900px); margin: 0 auto; padding: 40px 0; }}
    section {{ background: #fff; border: 1px solid #d7dde5; padding: 20px; margin-top: 18px; }}
    h1 {{ margin: 0; font-size: 42px; line-height: 1.05; letter-spacing: 0; }}
    h2 {{ margin: 0 0 10px; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: #58616c; }}
  </style>
</head>
<body>
  <main>
    <h1>Allie release decision: {status}</h1>
    <section>
      <h2>Evidence Projection</h2>
      <p>{text}</p>
      <p>This is a projection of evidence packets, not a legal compliance guarantee and not a global score.</p>
    </section>
  </main>
</body>
</html>
"#,
        status = escape_html(&summary.status),
        text = text
    )
}

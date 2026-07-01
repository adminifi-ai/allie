use crate::model::{
    AgenticAssessment, ArtifactMetadata, ComplianceObligation, ComplianceProfileView,
    ComplianceReportPacket, ComplianceSummary, ComplianceSupportingCheck, ComplianceSurfaceReport,
    CriterionCoverageCell, EvidenceMedia, EvidencePacket, Finding, ProductMapPacket,
    ProductSurface, StateEvidence, Verdict,
};
use crate::report;
use crate::standards::{
    applicability_reason, criterion_level, criterion_principle, criterion_source_url,
    criterion_title, deterministic_pass_obligation, profile_obligation_list, residual_review_need,
    supporting_check_related_criteria, wcag21_aa_profile_view, wcag22_success_criteria,
    wcag22_success_criterion_ids,
};
use crate::{COMPLIANCE_REPORT_SCHEMA, now_utc, unique_strings};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) fn build_compliance_report(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    map_path: &Path,
    packet_path: &Path,
) -> ComplianceReportPacket {
    let base_criteria = compliance_criterion_order(&map.policy_profile, packet)
        .into_iter()
        .map(|obligation| compliance_obligation(map, packet, &obligation))
        .collect::<Vec<_>>();
    let supporting_checks = supporting_check_order(&map.policy_profile, packet)
        .into_iter()
        .map(|obligation| compliance_supporting_check(map, packet, &obligation))
        .collect::<Vec<_>>();
    let criterion_coverage = criterion_coverage_matrix(map, packet);
    let criteria = aggregate_criteria_from_cells(base_criteria, &criterion_coverage);
    let criteria = attach_agentic_reviews(criteria, packet, packet_path);
    let summary = compliance_summary(packet, &criteria, supporting_checks.len());
    let profile_views = compliance_profile_views(&map.policy_profile, &criteria);
    let surfaces = map
        .surfaces
        .iter()
        .map(|surface| compliance_surface_report(surface, packet, &criteria, &criterion_coverage))
        .collect();
    ComplianceReportPacket {
        schema: COMPLIANCE_REPORT_SCHEMA.to_string(),
        generated_at: now_utc().to_rfc3339(),
        source_map: map_path.to_string_lossy().to_string(),
        source_packet: packet_path.to_string_lossy().to_string(),
        app_name: map.app_name.clone(),
        summary,
        criteria: criteria.clone(),
        criterion_coverage,
        supporting_checks,
        obligations: criteria,
        surfaces,
        profile_views,
        state_evidence: build_state_evidence(packet, packet_path),
    }
}

/// Resolve each captured state's full-page screenshot (and focus order) once,
/// inlining the image as a data URI so the report is self-contained.
fn build_state_evidence(packet: &EvidencePacket, packet_path: &Path) -> Vec<StateEvidence> {
    let run_dir = packet_path.parent().unwrap_or_else(|| Path::new("."));
    packet
        .coverage
        .state_metadata
        .iter()
        .map(|state| {
            let media = packet
                .artifacts
                .iter()
                .filter(|artifact| {
                    is_screenshot_artifact(artifact)
                        && artifact.related_flow_state.as_deref() == Some(state.id.as_str())
                        && is_safe_run_relative(&artifact.path)
                })
                .filter_map(|artifact| {
                    artifact_data_uri(&run_dir.join(&artifact.path)).map(|uri| {
                        let caption = if artifact.id.starts_with("mobile-screenshot-") {
                            format!(
                                "{} - mobile viewport full page as Allie captured it",
                                state.id
                            )
                        } else {
                            format!("{} - full page as Allie captured it", state.id)
                        };
                        EvidenceMedia {
                            kind: "screenshot".to_string(),
                            caption,
                            data_uri: Some(uri),
                            artifact_ref: Some(artifact.id.clone()),
                        }
                    })
                })
                .collect();
            StateEvidence {
                id: state.id.clone(),
                route: state.route.clone(),
                url: state.url.clone(),
                title: state.title.clone(),
                http_status: state.http_status,
                keyboard_focus_order: state.keyboard_focus_order.clone(),
                media,
            }
        })
        .collect()
}

/// Attach each criterion's agentic (vision-model) assessment and its captured
/// media (screenshots, focus montage, focus/motion clips) to the report,
/// inlining the media as data URIs so the report is self-contained.
fn attach_agentic_reviews(
    mut criteria: Vec<ComplianceObligation>,
    packet: &EvidencePacket,
    packet_path: &Path,
) -> Vec<ComplianceObligation> {
    if packet.agentic_assessments.is_empty() {
        return criteria;
    }
    let run_dir = packet_path.parent().unwrap_or_else(|| Path::new("."));
    let by_obligation = packet
        .agentic_assessments
        .iter()
        .map(|record| (record.obligation.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    for criterion in &mut criteria {
        let Some(record) = by_obligation.get(criterion.id.as_str()) else {
            continue;
        };
        let media = record
            .media
            .iter()
            .filter(|media_ref| is_safe_run_relative(&media_ref.path))
            .filter_map(|media_ref| {
                artifact_data_uri(&run_dir.join(&media_ref.path)).map(|uri| EvidenceMedia {
                    kind: media_ref.kind.clone(),
                    caption: media_ref.caption.clone(),
                    data_uri: Some(uri),
                    artifact_ref: None,
                })
            })
            .collect::<Vec<_>>();
        criterion.agentic_review = Some(AgenticAssessment {
            assessment: record.assessment.clone(),
            rationale: record.rationale.clone(),
            reviewer_guidance: record.reviewer_guidance.clone(),
            confidence: record.confidence.clone(),
            provider: record.provider.clone(),
            model: record.model.clone(),
            media,
        });
    }
    criteria
}

fn is_screenshot_artifact(artifact: &ArtifactMetadata) -> bool {
    artifact.artifact_type == "screenshot"
        || artifact.path.ends_with(".png")
        || artifact.path.ends_with(".jpg")
        || artifact.path.ends_with(".jpeg")
        || artifact.path.ends_with(".webp")
}

/// Reject run-relative paths that escape the run directory (absolute, or with a
/// `..` component), so a hand-edited evidence packet cannot make the report
/// inline an arbitrary file from disk.
fn is_safe_run_relative(rel: &str) -> bool {
    !rel.is_empty()
        && !Path::new(rel).is_absolute()
        && !Path::new(rel)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}

/// Read a binary artifact and encode it as a `data:` URI for inline embedding.
/// Returns None if the file is missing or too large to inline sensibly.
fn artifact_data_uri(abs_path: &Path) -> Option<String> {
    const MAX_INLINE_BYTES: u64 = 6 * 1024 * 1024;
    let metadata = fs::metadata(abs_path).ok()?;
    if metadata.len() > MAX_INLINE_BYTES {
        return None;
    }
    let bytes = fs::read(abs_path).ok()?;
    let mime = match abs_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webm") => "video/webm",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    };
    Some(format!("data:{};base64,{}", mime, base64_encode(&bytes)))
}

/// Minimal standard base64 encoder. Kept in-tree to inline evidence images
/// without adding a dependency to a deliberately small crate graph.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn compliance_criterion_order(policy_profile: &str, packet: &EvidencePacket) -> Vec<String> {
    let mut obligations = Vec::new();
    let mut seen = BTreeSet::new();
    if policy_profile == "wcag22-aa" {
        for criterion in wcag22_success_criteria() {
            if let Some(obligation) = criterion["obligation"].as_str()
                && seen.insert(obligation.to_string())
            {
                obligations.push(obligation.to_string());
            }
        }
        return obligations;
    }

    for verdict in &packet.verdicts {
        if seen.insert(verdict.obligation.clone()) {
            obligations.push(verdict.obligation.clone());
        }
    }
    for finding in &packet.findings {
        if seen.insert(finding.standard_obligation.clone()) {
            obligations.push(finding.standard_obligation.clone());
        }
    }
    obligations
}

fn compliance_profile_views(
    policy_profile: &str,
    criteria: &[ComplianceObligation],
) -> Vec<ComplianceProfileView> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    let by_id = criteria
        .iter()
        .map(|criterion| (criterion.id.as_str(), criterion))
        .collect::<BTreeMap<_, _>>();
    let mut view = wcag21_aa_profile_view();
    for criterion_id in &view.included_criteria {
        match by_id
            .get(criterion_id.as_str())
            .map(|criterion| criterion.status.as_str())
        {
            Some("pass") => view.pass += 1,
            Some("fail") => view.fail += 1,
            Some("needs_review") => view.needs_review += 1,
            Some("not_applicable") => view.not_applicable += 1,
            Some("waived") => view.waived += 1,
            Some("risk_accepted") => view.risk_accepted += 1,
            Some("not_tested") | None => view.not_tested += 1,
            Some(_) => view.needs_review += 1,
        }
    }
    view.not_tested += view.missing_legacy_criteria.len();
    vec![view]
}

fn supporting_check_order(policy_profile: &str, packet: &EvidencePacket) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    let criteria = wcag22_success_criterion_ids();
    let mut support = Vec::new();
    let mut seen = BTreeSet::new();
    for obligation in std::iter::once(deterministic_pass_obligation(policy_profile))
        .chain(profile_obligation_list(
            policy_profile,
            "scripted_obligations",
        ))
        .chain(profile_obligation_list(
            policy_profile,
            "human_review_obligations",
        ))
        .chain(
            packet
                .verdicts
                .iter()
                .map(|verdict| verdict.obligation.clone()),
        )
        .chain(
            packet
                .findings
                .iter()
                .map(|finding| finding.standard_obligation.clone()),
        )
    {
        if !criteria.contains(&obligation) && seen.insert(obligation.clone()) {
            support.push(obligation);
        }
    }
    support
}

fn compliance_obligation(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    obligation: &str,
) -> ComplianceObligation {
    let verdicts = packet
        .verdicts
        .iter()
        .filter(|verdict| verdict.obligation == obligation)
        .collect::<Vec<_>>();
    let findings = related_findings(packet, obligation, &verdicts);
    let status = compliance_status(&verdicts, &findings);
    let surfaces = obligation_surface_ids(map, &verdicts, &findings);
    let tests = unique_strings(
        verdicts
            .iter()
            .map(|verdict| verdict.source.clone())
            .chain(findings.iter().map(|finding| finding.source.clone())),
    );
    let artifact_refs = obligation_artifact_refs(packet, &verdicts, &findings);
    let agentic_context = obligation_agentic_context(map, packet, &findings);
    let confidence = verdicts
        .iter()
        .map(|verdict| verdict.confidence.clone())
        .chain(findings.iter().map(|finding| finding.confidence.clone()))
        .next()
        .unwrap_or_else(|| "not_observed".to_string());
    let evidence_class = verdicts
        .iter()
        .map(|verdict| verdict.evidence_class.clone())
        .chain(
            findings
                .iter()
                .map(|finding| finding.evidence_class.clone()),
        )
        .next()
        .unwrap_or_else(|| "none".to_string());
    ComplianceObligation {
        id: obligation.to_string(),
        title: criterion_title(obligation),
        status: status.clone(),
        why: obligation_why(obligation, &status, &verdicts, &findings),
        surfaces,
        tests,
        artifact_refs,
        agentic_context,
        human_review: human_review_status(&status, &evidence_class),
        confidence,
        evidence_class,
        source_url: criterion_source_url(obligation),
        finding_refs: findings.iter().map(|finding| finding.id.clone()).collect(),
        principle: criterion_principle(obligation),
        level: criterion_level(obligation),
        media: Vec::new(),
        agentic_review: None,
    }
}

fn compliance_supporting_check(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    obligation: &str,
) -> ComplianceSupportingCheck {
    let row = compliance_obligation(map, packet, obligation);
    ComplianceSupportingCheck {
        id: row.id.clone(),
        title: row.title.clone(),
        status: row.status.clone(),
        why: row.why.clone(),
        related_criteria: supporting_check_related_criteria(obligation),
        surfaces: row.surfaces,
        tests: row.tests,
        artifact_refs: row.artifact_refs,
        agentic_context: row.agentic_context,
        human_review: row.human_review,
        confidence: row.confidence,
        evidence_class: row.evidence_class,
        finding_refs: row.finding_refs,
    }
}

fn related_findings<'a>(
    packet: &'a EvidencePacket,
    obligation: &str,
    verdicts: &[&Verdict],
) -> Vec<&'a Finding> {
    let finding_refs = verdicts
        .iter()
        .flat_map(|verdict| verdict.finding_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    packet
        .findings
        .iter()
        .filter(|finding| {
            finding.standard_obligation == obligation || finding_refs.contains(&finding.id)
        })
        .collect()
}

fn compliance_status(verdicts: &[&Verdict], findings: &[&Finding]) -> String {
    if findings.iter().any(|finding| finding.status == "fail")
        || verdicts.iter().any(|verdict| verdict.status == "fail")
    {
        "fail".to_string()
    } else if findings
        .iter()
        .any(|finding| finding.status == "needs_review")
        || verdicts
            .iter()
            .any(|verdict| verdict.status == "needs_review")
    {
        "needs_review".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "not_applicable")
    {
        "not_applicable".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "waived") {
        "waived".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "risk_accepted")
    {
        "risk_accepted".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "pass") {
        "pass".to_string()
    } else {
        "needs_review".to_string()
    }
}

fn obligation_surface_ids(
    map: &ProductMapPacket,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> Vec<String> {
    let state_ids = verdicts
        .iter()
        .flat_map(|verdict| verdict.affected_states.iter().cloned())
        .chain(
            findings
                .iter()
                .map(|finding| finding.affected_state.clone()),
        )
        .collect::<BTreeSet<_>>();
    let routes = findings
        .iter()
        .map(|finding| finding.affected_route.clone())
        .collect::<BTreeSet<_>>();
    let mut surface_ids = map
        .surfaces
        .iter()
        .filter(|surface| {
            surface
                .evidence_refs
                .iter()
                .any(|state| state_ids.contains(state))
                || surface.id.as_str() != "run" && state_ids.contains(&surface.id)
                || surface.routes.iter().any(|route| routes.contains(route))
        })
        .map(|surface| surface.id.clone())
        .collect::<Vec<_>>();
    if surface_ids.is_empty() && !state_ids.is_empty() {
        surface_ids = state_ids.into_iter().collect();
    }
    if surface_ids.is_empty() && !map.surfaces.is_empty() {
        surface_ids = map
            .surfaces
            .iter()
            .map(|surface| surface.id.clone())
            .collect();
    }
    surface_ids.sort();
    surface_ids.dedup();
    surface_ids
}

fn obligation_artifact_refs(
    packet: &EvidencePacket,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> Vec<String> {
    let state_ids = verdicts
        .iter()
        .flat_map(|verdict| verdict.affected_states.iter().cloned())
        .chain(
            findings
                .iter()
                .map(|finding| finding.affected_state.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut refs = findings
        .iter()
        .flat_map(|finding| finding.artifact_refs.iter().cloned())
        .collect::<Vec<_>>();
    refs.extend(
        packet
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact
                    .related_flow_state
                    .as_ref()
                    .is_some_and(|state| state_ids.contains(state))
            })
            .map(|artifact| artifact.id.clone()),
    );
    unique_strings(refs)
}

fn obligation_agentic_context(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    findings: &[&Finding],
) -> Vec<String> {
    let mut context = findings
        .iter()
        .filter(|finding| finding.evidence_class == "agentic")
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    context.extend(packet.review.iter().map(|review| review.id.clone()));
    if let Some(transcript) = &map.agent.transcript_path {
        context.push(format!("map-agent-transcript:{transcript}"));
    }
    unique_strings(context)
}

fn obligation_why(
    obligation: &str,
    status: &str,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> String {
    if let Some(finding) = findings.first() {
        return format!("{}: {}", finding.title, finding.description);
    }
    match status {
        "pass" => format!(
            "{} is marked pass by {}.",
            criterion_title(obligation),
            verdicts
                .first()
                .map(|verdict| verdict.source.as_str())
                .unwrap_or("the evidence packet")
        ),
        "needs_review" => format!(
            "{} requires visual, assistive-technology, workflow, or content judgment beyond deterministic automation.",
            criterion_title(obligation)
        ),
        "not_tested" => format!(
            "{} has no deterministic, scripted, agentic, or human-attested evidence in this packet.",
            criterion_title(obligation)
        ),
        "not_applicable" => applicability_reason(obligation),
        _ => format!("{} has status {}.", criterion_title(obligation), status),
    }
}

fn human_review_status(status: &str, evidence_class: &str) -> String {
    match (status, evidence_class) {
        ("pass", "deterministic") | ("pass", "scripted") => {
            "not_required_for_machine_evidence".to_string()
        }
        ("pass", _) => "attested_or_promoted".to_string(),
        ("fail", _) => "required_for_release_signoff".to_string(),
        ("needs_review", _) => "required".to_string(),
        ("not_tested", _) => "required_before_claim".to_string(),
        ("not_applicable", _) => "not_required".to_string(),
        _ => "review_status_unknown".to_string(),
    }
}

fn criterion_coverage_matrix(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
) -> Vec<CriterionCoverageCell> {
    if map.policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    let criteria = wcag22_success_criteria();
    let mut cells = Vec::new();
    for surface in &map.surfaces {
        for state_id in surface_state_ids(surface, packet) {
            for criterion in &criteria {
                if let Some(cell) =
                    criterion_coverage_cell(map, packet, surface, &state_id, criterion)
                {
                    cells.push(cell);
                }
            }
        }
    }
    cells
}

fn aggregate_criteria_from_cells(
    criteria: Vec<ComplianceObligation>,
    cells: &[CriterionCoverageCell],
) -> Vec<ComplianceObligation> {
    criteria
        .into_iter()
        .map(|mut criterion| {
            let criterion_cells = cells
                .iter()
                .filter(|cell| cell.criterion_id == criterion.id)
                .collect::<Vec<_>>();
            if criterion_cells.is_empty() {
                return criterion;
            }
            criterion.status = aggregate_cell_status(&criterion_cells);
            criterion.surfaces = unique_strings(
                criterion_cells
                    .iter()
                    .map(|cell| cell.surface_id.clone())
                    .collect::<Vec<_>>(),
            );
            criterion.tests = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.test_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.artifact_refs = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.artifact_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.agentic_context = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.agentic_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.finding_refs = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.finding_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.confidence = aggregate_cell_confidence(
                &criterion.status,
                &criterion_cells,
                &criterion.confidence,
            );
            criterion.why = criterion_matrix_why(&criterion, &criterion_cells);
            criterion
        })
        .collect()
}

fn aggregate_cell_confidence(
    status: &str,
    cells: &[&CriterionCoverageCell],
    fallback: &str,
) -> String {
    cells
        .iter()
        .copied()
        .find(|cell| cell.status == status)
        .or_else(|| cells.first().copied())
        .map(|cell| cell.confidence.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn aggregate_cell_status(cells: &[&CriterionCoverageCell]) -> String {
    if cells.iter().any(|cell| cell.status == "fail") {
        "fail".to_string()
    } else if cells.iter().any(|cell| cell.status == "needs_review") {
        "needs_review".to_string()
    } else if cells.iter().any(|cell| cell.status == "not_tested") {
        "not_tested".to_string()
    } else if cells.iter().any(|cell| cell.status == "risk_accepted") {
        "risk_accepted".to_string()
    } else if cells.iter().any(|cell| cell.status == "waived") {
        "waived".to_string()
    } else if cells.iter().all(|cell| cell.status == "not_applicable") {
        "not_applicable".to_string()
    } else {
        "pass".to_string()
    }
}

fn criterion_matrix_why(
    criterion: &ComplianceObligation,
    cells: &[&CriterionCoverageCell],
) -> String {
    let status = criterion.status.as_str();
    let total = cells.len();
    let with_evidence = cells
        .iter()
        .filter(|cell| {
            !cell.evidence_refs.is_empty()
                || !cell.agentic_refs.is_empty()
                || !cell.waiver_refs.is_empty()
        })
        .count();
    match status {
        "fail" => format!(
            "{} has failing evidence in at least one surface/state cell.",
            criterion.title
        ),
        "needs_review" => format!(
            "{} has {} of {} cell(s) requiring human or agentic review.",
            criterion.title,
            cells
                .iter()
                .filter(|cell| cell.status == "needs_review")
                .count(),
            total
        ),
        "not_tested" => format!(
            "{} has no completed evidence in {} of {} cell(s).",
            criterion.title,
            cells
                .iter()
                .filter(|cell| cell.status == "not_tested")
                .count(),
            total
        ),
        "pass" => format!(
            "{} has supporting evidence in {} of {} surface/state cell(s).",
            criterion.title, with_evidence, total
        ),
        "not_applicable" => applicability_reason(&criterion.id),
        _ => format!(
            "{} has aggregate status {} across {} surface/state cell(s).",
            criterion.title, status, total
        ),
    }
}

fn criterion_coverage_cell(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    surface: &ProductSurface,
    state_id: &str,
    criterion: &serde_json::Value,
) -> Option<CriterionCoverageCell> {
    let criterion_id = criterion["obligation"].as_str()?;
    let method = criterion["method"].as_str().unwrap_or("human_review");
    let verdicts = packet
        .verdicts
        .iter()
        .filter(|verdict| {
            verdict.obligation == criterion_id && verdict_applies_to_state(verdict, state_id)
        })
        .collect::<Vec<_>>();
    let findings = packet
        .findings
        .iter()
        .filter(|finding| {
            finding.standard_obligation == criterion_id && finding.affected_state == state_id
        })
        .collect::<Vec<_>>();
    let deterministic_support = if method == "axe" {
        let deterministic_support_id = deterministic_pass_obligation(&map.policy_profile);
        packet.verdicts.iter().find(|verdict| {
            verdict.obligation == deterministic_support_id
                && verdict.status == "pass"
                && verdict_applies_to_state(verdict, state_id)
        })
    } else {
        None
    };
    let waiver_refs = waiver_refs_for_cell(packet, &surface.id, criterion_id);
    let waiver_status = waiver_status_for_cell(packet, &surface.id, criterion_id);
    let status = criterion_cell_status(
        method,
        &verdicts,
        &findings,
        deterministic_support,
        waiver_status.as_deref(),
    );
    let finding_refs = findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    let mut artifact_refs = obligation_artifact_refs(packet, &verdicts, &findings);
    if artifact_refs.is_empty() && deterministic_support.is_some() {
        artifact_refs = state_artifact_refs(packet, state_id);
    }
    let test_refs = unique_strings(
        verdicts
            .iter()
            .map(|verdict| verdict.source.clone())
            .chain(findings.iter().map(|finding| finding.source.clone()))
            .chain(deterministic_support.map(|verdict| verdict.source.clone())),
    );
    let evidence_refs = unique_strings(
        finding_refs
            .iter()
            .cloned()
            .chain(artifact_refs.iter().cloned())
            .chain(test_refs.iter().cloned()),
    );
    let agentic_refs = obligation_agentic_context(map, packet, &findings);
    let confidence = if waiver_status.is_some() {
        "human_attested".to_string()
    } else {
        verdicts
            .iter()
            .map(|verdict| verdict.confidence.clone())
            .chain(findings.iter().map(|finding| finding.confidence.clone()))
            .chain(deterministic_support.map(|verdict| verdict.confidence.clone()))
            .next()
            .unwrap_or_else(|| default_criterion_confidence(method).to_string())
    };

    Some(CriterionCoverageCell {
        id: format!(
            "{}|{}|{}|{}",
            criterion_id, surface.id, state_id, map.policy_profile
        ),
        criterion_id: criterion_id.to_string(),
        surface_id: surface.id.clone(),
        state_id: state_id.to_string(),
        policy_profile: map.policy_profile.clone(),
        status: status.clone(),
        applicability: if status == "not_applicable" {
            "not_applicable".to_string()
        } else {
            "applicable".to_string()
        },
        method: method.to_string(),
        confidence,
        evidence_refs,
        agentic_refs,
        waiver_refs,
        finding_refs,
        artifact_refs,
        test_refs,
        replay_command: Some(packet.replay.command.clone()),
        residual_review_need: residual_review_need(method, &status),
    })
}

fn criterion_cell_status(
    method: &str,
    verdicts: &[&Verdict],
    findings: &[&Finding],
    deterministic_support: Option<&Verdict>,
    waiver_status: Option<&str>,
) -> String {
    if waiver_status == Some("risk_accepted") {
        "risk_accepted".to_string()
    } else if waiver_status == Some("waived") {
        "waived".to_string()
    } else if findings.iter().any(|finding| finding.status == "fail")
        || verdicts.iter().any(|verdict| verdict.status == "fail")
    {
        "fail".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "not_applicable")
    {
        "not_applicable".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "needs_review")
        || findings
            .iter()
            .any(|finding| finding.status == "needs_review")
    {
        "needs_review".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "pass")
        || method == "axe" && deterministic_support.is_some()
    {
        "pass".to_string()
    } else if method == "human_review" {
        "needs_review".to_string()
    } else {
        "not_tested".to_string()
    }
}

fn default_criterion_confidence(method: &str) -> &'static str {
    match method {
        "axe" => "not_observed",
        "scripted" => "script_observed",
        _ => "requires_human_or_agent_review",
    }
}

fn verdict_applies_to_state(verdict: &Verdict, state_id: &str) -> bool {
    verdict.affected_states.is_empty()
        || verdict
            .affected_states
            .iter()
            .any(|affected| affected == state_id)
}

fn surface_state_ids(surface: &ProductSurface, packet: &EvidencePacket) -> Vec<String> {
    let mut states = surface
        .evidence_refs
        .iter()
        .filter(|state| !state.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    for state in &packet.coverage.state_metadata {
        if surface.routes.contains(&state.route) || surface.id == state.id {
            states.insert(state.id.clone());
        }
    }
    if states.is_empty() && !surface.id.trim().is_empty() {
        states.insert(surface.id.clone());
    }
    if states.is_empty() {
        states.extend(packet.coverage.states_captured.iter().cloned());
    }
    states.into_iter().collect()
}

fn state_artifact_refs(packet: &EvidencePacket, state_id: &str) -> Vec<String> {
    packet
        .artifacts
        .iter()
        .filter(|artifact| artifact.related_flow_state.as_deref() == Some(state_id))
        .map(|artifact| artifact.id.clone())
        .collect()
}

fn waiver_refs_for_cell(
    packet: &EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Vec<String> {
    matching_cell_waivers(packet, surface_id, criterion_id)
        .into_iter()
        .filter_map(|waiver| waiver["id"].as_str().map(ToString::to_string))
        .collect()
}

fn waiver_status_for_cell(
    packet: &EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Option<String> {
    let statuses = matching_cell_waivers(packet, surface_id, criterion_id)
        .into_iter()
        .filter_map(|waiver| waiver["status"].as_str())
        .collect::<Vec<_>>();
    if statuses.contains(&"risk_accepted") {
        Some("risk_accepted".to_string())
    } else if statuses.contains(&"waived") {
        Some("waived".to_string())
    } else {
        None
    }
}

fn matching_cell_waivers<'a>(
    packet: &'a EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Vec<&'a serde_json::Value> {
    packet
        .waivers
        .iter()
        .filter(|waiver| waiver["surface"].as_str() == Some(surface_id))
        .filter(|waiver| {
            let standard = waiver["standard_obligation"]
                .as_str()
                .or_else(|| waiver["obligation"].as_str())
                .or_else(|| waiver["criterion_id"].as_str());
            standard.is_none() || standard == Some(criterion_id)
        })
        .collect()
}

pub(crate) fn validate_criterion_coverage_cells(
    report: &serde_json::Value,
) -> std::result::Result<(), String> {
    let Some(cells) = report["criterion_coverage"].as_array() else {
        return Err("criterion_coverage must be an array".to_string());
    };
    let mut cell_keys = BTreeSet::new();
    for cell in cells {
        for field in [
            "criterion_id",
            "surface_id",
            "state_id",
            "policy_profile",
            "status",
            "applicability",
            "method",
            "confidence",
            "residual_review_need",
        ] {
            if cell[field]
                .as_str()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!("criterion coverage cell missing {field}"));
            }
        }
        for field in [
            "evidence_refs",
            "agentic_refs",
            "waiver_refs",
            "finding_refs",
            "artifact_refs",
            "test_refs",
        ] {
            if !cell[field].is_array() {
                return Err(format!("criterion coverage cell missing {field}"));
            }
        }
        let status = cell["status"].as_str().unwrap_or_default();
        if matches!(status, "pass" | "fail" | "waived" | "risk_accepted")
            && !cell_has_provenance(cell)
        {
            return Err(format!(
                "terminal criterion coverage cell lacks provenance: {}",
                cell["criterion_id"].as_str().unwrap_or("unknown")
            ));
        }
        let key = coverage_cell_key(cell)?;
        if !cell_keys.insert(key.clone()) {
            return Err(format!("duplicate criterion coverage cell: {key}"));
        }
    }
    validate_criterion_coverage_completeness(report, &cell_keys)?;
    Ok(())
}

fn coverage_cell_key(cell: &serde_json::Value) -> std::result::Result<String, String> {
    Ok(format!(
        "{}|{}|{}|{}",
        cell["criterion_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing criterion_id".to_string())?,
        cell["surface_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing surface_id".to_string())?,
        cell["state_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing state_id".to_string())?,
        cell["policy_profile"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing policy_profile".to_string())?
    ))
}

fn validate_criterion_coverage_completeness(
    report: &serde_json::Value,
    cell_keys: &BTreeSet<String>,
) -> std::result::Result<(), String> {
    let Some(criteria) = report["criteria"].as_array() else {
        return Ok(());
    };
    let Some(surfaces) = report["surfaces"].as_array() else {
        return Ok(());
    };
    let criterion_ids = criteria
        .iter()
        .filter_map(|criterion| criterion["id"].as_str())
        .collect::<Vec<_>>();
    let policy_profile = report["criterion_coverage"]
        .as_array()
        .and_then(|cells| cells.first())
        .and_then(|cell| cell["policy_profile"].as_str())
        .unwrap_or("wcag22-aa");
    for surface in surfaces {
        let Some(surface_id) = surface["surface_id"].as_str() else {
            continue;
        };
        let states = surface["states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|state| state.as_str())
            .collect::<Vec<_>>();
        for criterion_id in &criterion_ids {
            for state_id in &states {
                let key = format!("{criterion_id}|{surface_id}|{state_id}|{policy_profile}");
                if !cell_keys.contains(&key) {
                    return Err(format!("missing criterion coverage cell: {key}"));
                }
            }
        }
    }
    Ok(())
}

fn cell_has_provenance(cell: &serde_json::Value) -> bool {
    [
        "evidence_refs",
        "agentic_refs",
        "waiver_refs",
        "finding_refs",
        "artifact_refs",
        "test_refs",
    ]
    .iter()
    .any(|field| {
        cell[*field]
            .as_array()
            .is_some_and(|values| !values.is_empty())
    })
}

pub(crate) fn compliance_summary(
    packet: &EvidencePacket,
    criteria: &[ComplianceObligation],
    supporting_check_count: usize,
) -> ComplianceSummary {
    let pass = criteria
        .iter()
        .filter(|obligation| obligation.status == "pass")
        .count();
    let fail = criteria
        .iter()
        .filter(|obligation| obligation.status == "fail")
        .count();
    let needs_review = criteria
        .iter()
        .filter(|obligation| obligation.status == "needs_review")
        .count();
    let not_tested = criteria
        .iter()
        .filter(|obligation| obligation.status == "not_tested")
        .count();
    let not_applicable = criteria
        .iter()
        .filter(|obligation| obligation.status == "not_applicable")
        .count();
    let waived = criteria
        .iter()
        .filter(|obligation| obligation.status == "waived")
        .count();
    let risk_accepted = criteria
        .iter()
        .filter(|obligation| obligation.status == "risk_accepted")
        .count();
    let ai_pass = criteria
        .iter()
        .filter(|obligation| obligation.status == "pass" && report::is_agentic_verdict(obligation))
        .count();
    let ai_fail = criteria
        .iter()
        .filter(|obligation| obligation.status == "fail" && report::is_agentic_verdict(obligation))
        .count();
    let status = if packet.summary.status == "error" {
        "error"
    } else if fail > 0 {
        "fail"
    } else if needs_review > 0 || not_tested > 0 || waived > 0 || risk_accepted > 0 {
        "needs_review"
    } else {
        "pass"
    };
    ComplianceSummary {
        status: status.to_string(),
        total_obligations: criteria.len(),
        pass,
        fail,
        needs_review,
        not_tested,
        not_applicable,
        waived,
        risk_accepted,
        ai_pass,
        ai_fail,
        total_success_criteria: criteria.len(),
        total_supporting_checks: supporting_check_count,
        evidence_packet_status: packet.summary.status.clone(),
    }
}

pub(crate) fn compliance_surface_report(
    surface: &ProductSurface,
    packet: &EvidencePacket,
    criteria: &[ComplianceObligation],
    cells: &[CriterionCoverageCell],
) -> ComplianceSurfaceReport {
    let surface_cells = cells
        .iter()
        .filter(|cell| cell.surface_id == surface.id)
        .collect::<Vec<_>>();
    let surface_criteria = criteria
        .iter()
        .filter(|obligation| obligation.surfaces.contains(&surface.id))
        .collect::<Vec<_>>();
    let criterion_ids = criteria
        .iter()
        .filter(|obligation| obligation.surfaces.contains(&surface.id))
        .map(|obligation| obligation.id.clone())
        .collect::<Vec<_>>();
    let cell_ids = surface_cells
        .iter()
        .map(|cell| cell.id.clone())
        .collect::<Vec<_>>();
    let state_ids = unique_strings(surface_cells.iter().map(|cell| cell.state_id.clone()));
    let finding_refs = packet
        .findings
        .iter()
        .filter(|finding| {
            surface.evidence_refs.contains(&finding.affected_state)
                || surface.routes.contains(&finding.affected_route)
        })
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    let status = if surface_cells.is_empty() {
        surface_status_from_criteria(&surface_criteria)
    } else {
        surface_status_from_cells(&surface_cells)
    };
    ComplianceSurfaceReport {
        surface_id: surface.id.clone(),
        title: surface.title.clone(),
        routes: surface.routes.clone(),
        states: state_ids,
        status: status.to_string(),
        criteria: criterion_ids,
        cells: cell_ids,
        finding_refs,
    }
}

fn surface_status_from_cells(cells: &[&CriterionCoverageCell]) -> &'static str {
    if cells.iter().any(|cell| cell.status == "fail") {
        "fail"
    } else if cells.iter().any(|cell| {
        matches!(
            cell.status.as_str(),
            "needs_review" | "not_tested" | "waived" | "risk_accepted"
        )
    }) {
        "needs_review"
    } else {
        "pass"
    }
}

fn surface_status_from_criteria(criteria: &[&ComplianceObligation]) -> &'static str {
    if criteria.iter().any(|criterion| criterion.status == "fail") {
        "fail"
    } else if criteria.iter().any(|criterion| {
        matches!(
            criterion.status.as_str(),
            "needs_review" | "not_tested" | "waived" | "risk_accepted"
        )
    }) {
        "needs_review"
    } else {
        "pass"
    }
}

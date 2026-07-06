//! One reconciled definition of what "review" means in Allie.
//!
//! A 2026-07-01 dogfood run (`docs/dogfood/025-vanity-vs-olympus-cross-target.md`,
//! folded into `backlog.d/027-raise-wcag-report-efficacy-and-quality.md` finding
//! R3) surfaced three independently-computed "review" counts for the same run —
//! `needs_review: 2` in the compliance summary, `review_needed_obligations: 7`
//! in the release decision, and `obligations_requiring_human_review: 46` in the
//! evidence packet's coverage — with no shared definition tying them together.
//! They are genuinely different grains, not duplicates of one number:
//!
//! - [`ReviewSummary::criteria_needs_review`] is criterion-grain and run-outcome:
//!   how many WCAG success criteria landed on `needs_review` in this run, after
//!   aggregating every surface/state cell for that criterion.
//! - [`ReviewSummary::verdict_review_needed_obligations`] is verdict-grain and
//!   run-outcome: the raw obligation ids behind individual `needs_review`
//!   verdicts, before per-criterion aggregation, so one criterion can appear
//!   here zero, one, or many times.
//! - [`ReviewSummary::profile_human_review_scope`] is profile-grain and static:
//!   the fixed set of obligations the policy profile itself designates as
//!   requiring human judgment. It does not vary with what a run found.
//!
//! This module is the one place that computes them. Callers read a field off
//! [`ReviewSummary`]; they never re-filter verdicts, criteria, or profile
//! obligations themselves, so the three counts can't silently re-diverge.

use crate::model::{ComplianceObligation, EvidencePacket};

/// The three "review" grains one evidence run produces, computed together so
/// they read from a single definition. Deliberately not collapsed into one
/// number — see the module doc for why each is distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewSummary {
    /// Criterion-grain, run-outcome. `None` when the caller has no built
    /// `ComplianceObligation`s to aggregate over (the release decision path
    /// never builds them — it projects directly off the evidence packet) —
    /// left unset rather than reporting a misleading zero.
    pub(crate) criteria_needs_review: Option<usize>,
    /// Verdict-grain, run-outcome: obligation ids from `packet.verdicts` whose
    /// status is `needs_review`, in verdict order (duplicates possible when a
    /// criterion has more than one `needs_review` verdict).
    pub(crate) verdict_review_needed_obligations: Vec<String>,
    /// Profile-grain, static: the policy profile's fixed human-review scope,
    /// carried through unchanged from `packet.coverage.profile_human_review_scope`.
    pub(crate) profile_human_review_scope: Vec<String>,
}

/// Compute all three review grains for one evidence packet.
///
/// `criteria` is `Some` only on paths that have already built the
/// per-criterion compliance view (`compliance::compliance_summary`); pass
/// `None` on paths that operate on the packet alone (the release decision
/// projection), which leaves `criteria_needs_review` unset instead of
/// silently reporting zero criteria in review.
pub(crate) fn review_summary(
    packet: &EvidencePacket,
    criteria: Option<&[ComplianceObligation]>,
) -> ReviewSummary {
    ReviewSummary {
        criteria_needs_review: criteria.map(|rows| {
            rows.iter()
                .filter(|row| row.status == "needs_review")
                .count()
        }),
        verdict_review_needed_obligations: packet
            .verdicts
            .iter()
            .filter(|verdict| verdict.status == "needs_review")
            .map(|verdict| verdict.obligation.clone())
            .collect(),
        profile_human_review_scope: packet.coverage.profile_human_review_scope.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        BrowserSettings, Coverage, CredentialProviderMetadata, PacketSummary, PolicyBudget,
        PolicyMetadata, Replay, RunMetadata, TargetMetadata, Verdict, Viewport,
    };
    use crate::standards::{criterion_level, criterion_principle, criterion_source_url};

    fn sample_criterion(id: &str, status: &str) -> ComplianceObligation {
        ComplianceObligation {
            id: id.to_string(),
            title: id.to_string(),
            status: status.to_string(),
            why: "test row".to_string(),
            surfaces: Vec::new(),
            tests: Vec::new(),
            artifact_refs: Vec::new(),
            agentic_context: Vec::new(),
            human_review: "required".to_string(),
            confidence: "human_attested".to_string(),
            evidence_class: "human".to_string(),
            source_url: criterion_source_url(id),
            finding_refs: Vec::new(),
            principle: criterion_principle(id),
            level: criterion_level(id),
            media: Vec::new(),
            agentic_review: None,
        }
    }

    fn sample_verdict(obligation: &str, status: &str) -> Verdict {
        Verdict {
            obligation: obligation.to_string(),
            status: status.to_string(),
            confidence: "not_observed".to_string(),
            evidence_class: "deterministic".to_string(),
            source: "axe".to_string(),
            affected_states: Vec::new(),
            finding_refs: Vec::new(),
        }
    }

    /// Minimal but structurally complete `EvidencePacket`, with `verdicts` and
    /// `profile_human_review_scope` supplied by the caller so each test can
    /// pin its own shape.
    fn packet_with(
        verdicts: Vec<Verdict>,
        profile_human_review_scope: Vec<String>,
    ) -> EvidencePacket {
        EvidencePacket {
            schema: "allie.evidence.v0".to_string(),
            summary: PacketSummary {
                status: "pass".to_string(),
                exit_code: 0,
                deterministic_failures: 0,
                scripted_failures: 0,
                infrastructure_failures: 0,
                states_captured: 1,
                failure_class: None,
            },
            run: RunMetadata {
                id: "run-review-summary-test".to_string(),
                started_at: "2026-07-01T00:00:00Z".to_string(),
                finished_at: "2026-07-01T00:00:01Z".to_string(),
                allie_version: "0.1.0".to_string(),
                git_sha: "test".to_string(),
                git_branch: "test".to_string(),
                ci_provider: None,
                actor: "test".to_string(),
            },
            target: TargetMetadata {
                base_url: Some("http://127.0.0.1:1".to_string()),
                environment: "test".to_string(),
                app_name: "Allie test".to_string(),
                auth_profile: "none".to_string(),
                credential_provider: CredentialProviderMetadata {
                    provider: "none".to_string(),
                    env: None,
                    required: false,
                    status: "not_required".to_string(),
                },
                flow_manifest: "examples/login-flow.yml".to_string(),
            },
            policy: PolicyMetadata {
                profile: "wcag22-aa".to_string(),
                blocking_classes: vec!["deterministic".to_string()],
                worker_timeout_ms: 30000,
                model_provider_allowlist: Vec::new(),
                model_status: "disabled".to_string(),
                zdr_required: true,
                redaction_profile: "not_redacted_local_fixture".to_string(),
                budget: PolicyBudget {
                    model_calls: 0,
                    max_states: 1,
                },
            },
            coverage: Coverage {
                routes_visited: vec!["/".to_string()],
                surfaces_discovered: vec!["home".to_string()],
                flows_exercised: vec!["home-flow".to_string()],
                states_captured: vec!["home".to_string()],
                state_metadata: Vec::new(),
                standards_obligations_evaluated: Vec::new(),
                obligations_not_tested: Vec::new(),
                profile_human_review_scope,
            },
            artifacts: Vec::new(),
            findings: Vec::new(),
            verdicts,
            waivers: Vec::new(),
            review: Vec::new(),
            agentic_assessments: Vec::new(),
            replay: Replay {
                command: "cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest".to_string(),
                manifest_path: "examples/login-flow.yml".to_string(),
                environment_requirements: Vec::new(),
                credential_profile: "none".to_string(),
                browser: BrowserSettings {
                    viewport: Viewport {
                        width: 1280,
                        height: 720,
                    },
                    color_scheme: "light".to_string(),
                    reduced_motion: "reduce".to_string(),
                    locale: "en-US".to_string(),
                    zoom: 1.0,
                },
                seed_data: Vec::new(),
                known_nondeterminism: Vec::new(),
            },
        }
    }

    /// Pins the exact drift the 2026-07-01 dogfood run flagged (R3): three
    /// "review" counts for one run, deliberately unequal, all sourced from the
    /// one `review_summary` call.
    #[test]
    fn review_summary_reconciles_three_distinct_review_grains() {
        // Verdict-grain: 7 needs_review verdicts out of 10.
        let mut verdicts = Vec::new();
        for i in 0..7 {
            verdicts.push(sample_verdict(
                &format!("wcag22-aa:verdict-review-{i}"),
                "needs_review",
            ));
        }
        for i in 0..3 {
            verdicts.push(sample_verdict(
                &format!("wcag22-aa:verdict-pass-{i}"),
                "pass",
            ));
        }
        // Profile-grain: 46 obligations in the profile's static human-review scope.
        let profile_scope = (0..46)
            .map(|i| format!("wcag22-aa:profile-scope-{i}"))
            .collect::<Vec<_>>();
        let packet = packet_with(verdicts, profile_scope);

        // Criterion-grain: 2 aggregated criteria at needs_review out of 5.
        let mut criteria = Vec::new();
        criteria.push(sample_criterion("wcag22-aa:crit-a", "needs_review"));
        criteria.push(sample_criterion("wcag22-aa:crit-b", "needs_review"));
        criteria.push(sample_criterion("wcag22-aa:crit-c", "pass"));
        criteria.push(sample_criterion("wcag22-aa:crit-d", "pass"));
        criteria.push(sample_criterion("wcag22-aa:crit-e", "fail"));

        let summary = review_summary(&packet, Some(&criteria));

        assert_eq!(summary.criteria_needs_review, Some(2));
        assert_eq!(summary.verdict_review_needed_obligations.len(), 7);
        assert_eq!(summary.profile_human_review_scope.len(), 46);

        // The point of the ticket: these are not the same number, and that is
        // correct — each answers a different question about the same run.
        assert_ne!(
            summary.criteria_needs_review.unwrap(),
            summary.verdict_review_needed_obligations.len()
        );
        assert_ne!(
            summary.verdict_review_needed_obligations.len(),
            summary.profile_human_review_scope.len()
        );
    }

    #[test]
    fn review_summary_on_empty_packet_is_all_zero() {
        let packet = packet_with(Vec::new(), Vec::new());
        let summary = review_summary(&packet, Some(&[]));

        assert_eq!(summary.criteria_needs_review, Some(0));
        assert!(summary.verdict_review_needed_obligations.is_empty());
        assert!(summary.profile_human_review_scope.is_empty());
    }

    #[test]
    fn review_summary_on_all_pass_packet_has_zero_review_counts() {
        let verdicts = vec![
            sample_verdict("wcag22-aa:crit-a", "pass"),
            sample_verdict("wcag22-aa:crit-b", "pass"),
        ];
        let packet = packet_with(
            verdicts,
            vec!["wcag22-aa:human-content-meaning".to_string()],
        );
        let criteria = vec![
            sample_criterion("wcag22-aa:crit-a", "pass"),
            sample_criterion("wcag22-aa:crit-b", "pass"),
        ];

        let summary = review_summary(&packet, Some(&criteria));

        assert_eq!(summary.criteria_needs_review, Some(0));
        assert!(summary.verdict_review_needed_obligations.is_empty());
        // Profile scope is static and unaffected by run outcome.
        assert_eq!(summary.profile_human_review_scope.len(), 1);
    }

    #[test]
    fn review_summary_with_zero_profile_human_review_obligations() {
        // A profile (or a run against a non-wcag22-aa profile) that designates
        // no obligations as requiring human review by method.
        let packet = packet_with(
            vec![sample_verdict("custom:check-1", "needs_review")],
            Vec::new(),
        );

        let summary = review_summary(&packet, None);

        assert!(summary.profile_human_review_scope.is_empty());
        assert_eq!(summary.verdict_review_needed_obligations.len(), 1);
        // No criteria supplied on this call path (mirrors the release decision
        // path, which never builds ComplianceObligations).
        assert_eq!(summary.criteria_needs_review, None);
    }
}

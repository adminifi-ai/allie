use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod egress;
pub(crate) use egress::{AgenticAssessmentRecord, ModelEgressEvent, ModelEgressUsage};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PublicationClass {
    #[default]
    SensitiveLocal,
    RedactedShareable,
    PublicSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ManifestTarget {
    pub(crate) kind: String,
    pub(crate) fixture_dir: Option<PathBuf>,
    pub(crate) base_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ArtifactPolicy {
    pub(crate) redaction_status: String,
    pub(crate) retention_class: String,
}

impl Default for ArtifactPolicy {
    fn default() -> Self {
        Self {
            redaction_status: "not_redacted_local_fixture".to_string(),
            retention_class: "local_ephemeral".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BrowserSettings {
    pub(crate) viewport: Viewport,
    pub(crate) color_scheme: String,
    pub(crate) reduced_motion: String,
    pub(crate) locale: String,
    pub(crate) zoom: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Viewport {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProductMapPacket {
    pub(crate) schema: String,
    pub(crate) generated_at: String,
    pub(crate) source_manifest: String,
    pub(crate) project_root: String,
    pub(crate) app_name: String,
    pub(crate) environment: String,
    pub(crate) policy_profile: String,
    pub(crate) target: ManifestTarget,
    pub(crate) agent: AgentRunnerReceiptPacket,
    pub(crate) standards: StandardsProfileSummary,
    pub(crate) surfaces: Vec<ProductSurface>,
    pub(crate) workflows: Vec<ProductWorkflow>,
    #[serde(default)]
    pub(crate) discovery_diagnostics: Vec<DiscoveryDiagnostic>,
    pub(crate) open_questions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DiscoveryDiagnostic {
    pub(crate) source: String,
    pub(crate) severity: String,
    pub(crate) route: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AgentRunnerReceiptPacket {
    pub(crate) schema: String,
    pub(crate) runner: String,
    pub(crate) mode: String,
    pub(crate) status: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) command: Vec<String>,
    pub(crate) prompt_path: Option<String>,
    pub(crate) transcript_path: Option<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) sources: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StandardsProfileSummary {
    pub(crate) id: String,
    pub(crate) source_urls: Vec<String>,
    pub(crate) total_obligations: usize,
    pub(crate) methods: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProductSurface {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) routes: Vec<String>,
    pub(crate) files: Vec<String>,
    pub(crate) services: Vec<String>,
    pub(crate) user_stories: Vec<String>,
    pub(crate) workflow_refs: Vec<String>,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) confidence: String,
    pub(crate) review_status: String,
    pub(crate) provenance: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProductWorkflow {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) surface_refs: Vec<String>,
    pub(crate) user_story: String,
    pub(crate) generated_flow_manifest: String,
    pub(crate) states: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceReportPacket {
    pub(crate) schema: String,
    pub(crate) generated_at: String,
    pub(crate) source_map: String,
    pub(crate) source_packet: String,
    pub(crate) app_name: String,
    pub(crate) summary: ComplianceSummary,
    pub(crate) criteria: Vec<ComplianceObligation>,
    pub(crate) criterion_coverage: Vec<CriterionCoverageCell>,
    pub(crate) supporting_checks: Vec<ComplianceSupportingCheck>,
    pub(crate) obligations: Vec<ComplianceObligation>,
    pub(crate) surfaces: Vec<ComplianceSurfaceReport>,
    #[serde(default)]
    pub(crate) profile_views: Vec<ComplianceProfileView>,
    #[serde(default)]
    pub(crate) state_evidence: Vec<StateEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) model_egress_events: Vec<ModelEgressEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceProfileView {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) basis: String,
    pub(crate) source_urls: Vec<String>,
    pub(crate) total_success_criteria: usize,
    pub(crate) included_criteria: Vec<String>,
    pub(crate) excluded_criteria: Vec<String>,
    pub(crate) missing_legacy_criteria: Vec<String>,
    #[serde(default)]
    pub(crate) pass: usize,
    #[serde(default)]
    pub(crate) fail: usize,
    #[serde(default)]
    pub(crate) needs_review: usize,
    #[serde(default)]
    pub(crate) not_tested: usize,
    #[serde(default)]
    pub(crate) not_applicable: usize,
    #[serde(default)]
    pub(crate) waived: usize,
    #[serde(default)]
    pub(crate) risk_accepted: usize,
    pub(crate) notes: Vec<String>,
}

/// Per-state evidence surfaced once at the top of the report (the captured
/// screenshot and observed focus order), so criteria can reference it without
/// re-inlining the same image dozens of times.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StateEvidence {
    pub(crate) id: String,
    pub(crate) route: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) http_status: Option<u16>,
    #[serde(default)]
    pub(crate) keyboard_focus_order: Vec<String>,
    #[serde(default)]
    pub(crate) media: Vec<EvidenceMedia>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceSummary {
    pub(crate) status: String,
    pub(crate) total_obligations: usize,
    pub(crate) pass: usize,
    pub(crate) fail: usize,
    pub(crate) needs_review: usize,
    pub(crate) not_tested: usize,
    pub(crate) not_applicable: usize,
    pub(crate) waived: usize,
    pub(crate) risk_accepted: usize,
    /// Of `pass`/`fail` above, how many are agentic (asterisked) verdicts rather
    /// than machine-proven — surfaced so the headline distinguishes the two.
    #[serde(default)]
    pub(crate) ai_pass: usize,
    #[serde(default)]
    pub(crate) ai_fail: usize,
    pub(crate) total_success_criteria: usize,
    pub(crate) total_supporting_checks: usize,
    pub(crate) evidence_packet_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceObligation {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) why: String,
    pub(crate) surfaces: Vec<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) artifact_refs: Vec<String>,
    pub(crate) agentic_context: Vec<String>,
    pub(crate) human_review: String,
    pub(crate) confidence: String,
    pub(crate) evidence_class: String,
    pub(crate) source_url: Option<String>,
    pub(crate) finding_refs: Vec<String>,
    #[serde(default)]
    pub(crate) principle: String,
    #[serde(default)]
    pub(crate) level: String,
    #[serde(default)]
    pub(crate) media: Vec<EvidenceMedia>,
    #[serde(default)]
    pub(crate) agentic_review: Option<AgenticAssessment>,
}

/// A piece of visual evidence (screenshot, element crop, or motion GIF) inlined
/// into the report as a self-contained data URI so the report travels intact in
/// a PR diff, a CI artifact, or a committed snapshot without external files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EvidenceMedia {
    pub(crate) kind: String,
    pub(crate) caption: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifact_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AgenticAssessment {
    pub(crate) assessment: String,
    pub(crate) rationale: String,
    pub(crate) reviewer_guidance: String,
    pub(crate) confidence: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) media: Vec<EvidenceMedia>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CriterionCoverageCell {
    pub(crate) id: String,
    pub(crate) criterion_id: String,
    pub(crate) surface_id: String,
    pub(crate) state_id: String,
    pub(crate) policy_profile: String,
    pub(crate) status: String,
    pub(crate) applicability: String,
    pub(crate) method: String,
    pub(crate) confidence: String,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) agentic_refs: Vec<String>,
    pub(crate) waiver_refs: Vec<String>,
    pub(crate) finding_refs: Vec<String>,
    pub(crate) artifact_refs: Vec<String>,
    pub(crate) test_refs: Vec<String>,
    pub(crate) replay_command: Option<String>,
    pub(crate) residual_review_need: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceSupportingCheck {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) why: String,
    pub(crate) related_criteria: Vec<String>,
    pub(crate) surfaces: Vec<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) artifact_refs: Vec<String>,
    pub(crate) agentic_context: Vec<String>,
    pub(crate) human_review: String,
    pub(crate) confidence: String,
    pub(crate) evidence_class: String,
    pub(crate) finding_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComplianceSurfaceReport {
    pub(crate) surface_id: String,
    pub(crate) title: String,
    pub(crate) routes: Vec<String>,
    pub(crate) states: Vec<String>,
    pub(crate) status: String,
    pub(crate) criteria: Vec<String>,
    pub(crate) cells: Vec<String>,
    pub(crate) finding_refs: Vec<String>,
}

/// Lightweight inventory of page content + scripted signals the worker reports,
/// used by Allie's applicability oracle to decide automatically which criteria
/// do not apply and to run a couple of deterministic/scripted checks.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct PageFeatures {
    #[serde(default)]
    pub(crate) audio: u32,
    #[serde(default)]
    pub(crate) video: u32,
    #[serde(default)]
    pub(crate) forms: u32,
    #[serde(default)]
    pub(crate) inputs: u32,
    #[serde(default)]
    pub(crate) draggable: u32,
    #[serde(default)]
    pub(crate) iframes: u32,
    #[serde(default)]
    pub(crate) images: u32,
    #[serde(default)]
    pub(crate) links: u32,
    #[serde(default)]
    pub(crate) headings: u32,
    #[serde(default)]
    pub(crate) lang: bool,
    #[serde(default)]
    pub(crate) lang_value: String,
    #[serde(default)]
    pub(crate) reflow_overflow: bool,
    #[serde(default)]
    pub(crate) reflow_checked: bool,
    #[serde(default)]
    pub(crate) mobile_viewport_checked: bool,
    #[serde(default)]
    pub(crate) mobile_viewport_width: u32,
    #[serde(default)]
    pub(crate) mobile_viewport_height: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvidencePacket {
    pub(crate) schema: String,
    pub(crate) summary: PacketSummary,
    pub(crate) run: RunMetadata,
    pub(crate) target: TargetMetadata,
    pub(crate) policy: PolicyMetadata,
    pub(crate) coverage: Coverage,
    pub(crate) artifacts: Vec<ArtifactMetadata>,
    pub(crate) findings: Vec<Finding>,
    pub(crate) verdicts: Vec<Verdict>,
    pub(crate) waivers: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) agentic_assessments: Vec<AgenticAssessmentRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) model_egress_events: Vec<ModelEgressEvent>,
    pub(crate) replay: Replay,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AgenticMediaRef {
    pub(crate) kind: String,
    pub(crate) caption: String,
    pub(crate) path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PacketSummary {
    pub(crate) status: String,
    pub(crate) exit_code: i32,
    pub(crate) deterministic_failures: usize,
    pub(crate) scripted_failures: usize,
    pub(crate) infrastructure_failures: usize,
    pub(crate) states_captured: usize,
    pub(crate) failure_class: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RunMetadata {
    pub(crate) id: String,
    pub(crate) started_at: String,
    pub(crate) finished_at: String,
    pub(crate) allie_version: String,
    pub(crate) git_sha: String,
    pub(crate) git_branch: String,
    pub(crate) ci_provider: Option<String>,
    pub(crate) actor: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetMetadata {
    pub(crate) base_url: Option<String>,
    pub(crate) environment: String,
    pub(crate) app_name: String,
    pub(crate) auth_profile: String,
    pub(crate) credential_provider: CredentialProviderMetadata,
    pub(crate) flow_manifest: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CredentialProviderMetadata {
    pub(crate) provider: String,
    pub(crate) env: Option<String>,
    pub(crate) required: bool,
    pub(crate) status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PolicyMetadata {
    pub(crate) profile: String,
    pub(crate) blocking_classes: Vec<String>,
    pub(crate) worker_timeout_ms: u64,
    pub(crate) model_provider_allowlist: Vec<String>,
    pub(crate) model_status: String,
    pub(crate) zdr_required: bool,
    #[serde(deserialize_with = "deserialize_model_egress_redaction")]
    pub(crate) model_egress_redaction: Option<crate::ModelRedactionMode>,
    pub(crate) redaction_profile: String,
    pub(crate) budget: PolicyBudget,
}

fn deserialize_model_egress_redaction<'de, D>(
    deserializer: D,
) -> Result<Option<crate::ModelRedactionMode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::deserialize(deserializer)
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PolicyBudget {
    pub(crate) model_calls: u32,
    pub(crate) max_states: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Coverage {
    pub(crate) routes_visited: Vec<String>,
    pub(crate) surfaces_discovered: Vec<String>,
    pub(crate) flows_exercised: Vec<String>,
    pub(crate) states_captured: Vec<String>,
    pub(crate) state_metadata: Vec<StateMetadata>,
    pub(crate) standards_obligations_evaluated: Vec<String>,
    pub(crate) obligations_not_tested: Vec<String>,
    /// The fixed set of obligations this policy profile defines as requiring
    /// human judgment by method (WCAG's `human_review` success criteria plus explicit profile obligations). This is scope,
    /// not a run outcome: it is the same list for every run of a given
    /// profile regardless of what the run found. See `crate::review` for its
    /// relation to verdict and criterion-level `needs_review` outcomes.
    pub(crate) profile_human_review_scope: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StateMetadata {
    pub(crate) id: String,
    pub(crate) route: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) http_status: Option<u16>,
    pub(crate) keyboard_focus_order: Vec<String>,
    pub(crate) console_errors: Vec<String>,
    pub(crate) network_errors: Vec<String>,
    pub(crate) state_errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) features: Option<PageFeatures>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactMetadata {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) artifact_type: String,
    pub(crate) path: String,
    pub(crate) hash: String,
    pub(crate) redaction_status: String,
    pub(crate) retention_class: String,
    #[serde(default)]
    pub(crate) publication_class: PublicationClass,
    pub(crate) unavailable_reason: Option<String>,
    pub(crate) related_flow_state: Option<String>,
    pub(crate) creation_tool: String,
    pub(crate) timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Finding {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) evidence_class: String,
    pub(crate) standard_obligation: String,
    pub(crate) severity: String,
    pub(crate) status: String,
    pub(crate) confidence: String,
    pub(crate) source: String,
    pub(crate) affected_route: String,
    pub(crate) affected_state: String,
    pub(crate) artifact_refs: Vec<String>,
    pub(crate) replay_command: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Verdict {
    pub(crate) obligation: String,
    pub(crate) status: String,
    pub(crate) confidence: String,
    pub(crate) evidence_class: String,
    pub(crate) source: String,
    pub(crate) affected_states: Vec<String>,
    pub(crate) finding_refs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Replay {
    pub(crate) command: String,
    pub(crate) manifest_path: String,
    pub(crate) environment_requirements: Vec<String>,
    pub(crate) credential_profile: String,
    pub(crate) browser: BrowserSettings,
    pub(crate) seed_data: Vec<String>,
    pub(crate) known_nondeterminism: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleaseDecisionPacket {
    pub(crate) schema: String,
    pub(crate) status: String,
    pub(crate) packet_path: String,
    pub(crate) packet_run_id: String,
    pub(crate) changed_surfaces: Vec<String>,
    pub(crate) blocking: ReleaseBlockingSummary,
    pub(crate) review: ReleaseReviewSummary,
    pub(crate) review_needed_obligations: Vec<String>,
    pub(crate) not_tested_obligations: Vec<String>,
    pub(crate) model_findings_non_blocking: usize,
    pub(crate) evidence_artifacts: Vec<String>,
    pub(crate) policy: ReleasePolicySummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleaseBlockingSummary {
    pub(crate) deterministic_failures: usize,
    pub(crate) scripted_failures: usize,
    pub(crate) infrastructure_failures: usize,
    pub(crate) missing_required_evidence: Vec<String>,
    pub(crate) expired_waivers: Vec<serde_json::Value>,
    pub(crate) invalid_waivers: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleaseReviewSummary {
    pub(crate) stale_evidence: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReleasePolicySummary {
    pub(crate) model_status: String,
    pub(crate) model_provider_allowlist: Vec<String>,
    pub(crate) zdr_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GithubCheckPayload {
    pub(crate) name: String,
    pub(crate) conclusion: String,
    pub(crate) output: GithubCheckOutput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GithubCheckOutput {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) text: String,
}

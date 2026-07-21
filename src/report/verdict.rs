use crate::model::ComplianceObligation;

/// True when a criterion's pass/fail verdict came from the agentic reviewer
/// rather than a deterministic check. Keying the marker off the same
/// `agentic_review` value that renders the evidence block keeps them aligned.
pub(crate) fn is_agentic_verdict(obligation: &ComplianceObligation) -> bool {
    matches!(
        obligation
            .agentic_review
            .as_ref()
            .map(|review| review.assessment.as_str()),
        Some("pass" | "fail")
    ) && (obligation.status == "pass" || obligation.status == "fail")
}

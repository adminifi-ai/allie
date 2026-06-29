use crate::model::{PageFeatures, StandardsProfileSummary};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub(crate) const WCAG22_AA_PROFILE_JSON: &str = include_str!("../profiles/wcag22-aa.json");

pub(crate) fn standards_profile_summary(policy_profile: &str) -> StandardsProfileSummary {
    if policy_profile != "wcag22-aa" {
        return StandardsProfileSummary {
            id: policy_profile.to_string(),
            source_urls: Vec::new(),
            total_obligations: 0,
            methods: BTreeMap::new(),
        };
    }
    let mut methods = BTreeMap::new();
    for criterion in wcag22_success_criteria() {
        let method = criterion["method"].as_str().unwrap_or("unknown");
        *methods.entry(method.to_string()).or_insert(0) += 1;
    }
    StandardsProfileSummary {
        id: "wcag22-aa".to_string(),
        source_urls: vec![
            wcag22_profile()["source_url"]
                .as_str()
                .unwrap_or("https://www.w3.org/WAI/WCAG22/wcag.json")
                .to_string(),
            "https://www.w3.org/TR/WCAG22/".to_string(),
            "https://www.w3.org/WAI/test-evaluate/conformance/wcag-em/".to_string(),
        ],
        total_obligations: wcag22_success_criteria().len(),
        methods,
    }
}

pub(crate) fn obligation_from_tags(policy_profile: &str, tags: &[String]) -> String {
    if policy_profile != "wcag22-aa" {
        return tags
            .iter()
            .find(|tag| tag.starts_with("wcag"))
            .cloned()
            .unwrap_or_else(|| format!("{policy_profile}:unmapped-axe-rule"));
    }

    let profile = wcag22_profile();
    let Some(map) = profile
        .get("axe_tag_map")
        .and_then(|value| value.as_object())
    else {
        return "wcag22-aa:unmapped-axe-rule".to_string();
    };

    let mut candidates = tags.iter().collect::<Vec<_>>();
    candidates.sort_by_key(|tag| std::cmp::Reverse(tag.len()));
    for tag in candidates {
        if let Some(obligation) = map
            .get(tag)
            .and_then(|value| value.get("obligation"))
            .and_then(|value| value.as_str())
        {
            return obligation.to_string();
        }
    }

    "wcag22-aa:unmapped-axe-rule".to_string()
}

pub(crate) fn deterministic_pass_obligation(policy_profile: &str) -> String {
    if policy_profile != "wcag22-aa" {
        return format!("{policy_profile}:deterministic-machine-checks");
    }

    wcag22_profile()
        .get("deterministic_pass_obligation")
        .and_then(|value| value.get("obligation"))
        .and_then(|value| value.as_str())
        .unwrap_or("wcag22-aa:deterministic-axe-rules")
        .to_string()
}

pub(crate) fn scripted_profile_obligations(policy_profile: &str) -> Vec<String> {
    let mut obligations = profile_obligation_list(policy_profile, "scripted_obligations");
    obligations.extend(criteria_with_method(policy_profile, "scripted"));
    obligations.sort();
    obligations.dedup();
    obligations
}

pub(crate) fn human_review_profile_obligations(policy_profile: &str) -> Vec<String> {
    let mut obligations = profile_obligation_list(policy_profile, "human_review_obligations");
    obligations.extend(criteria_with_method(policy_profile, "human_review"));
    obligations.sort();
    obligations.dedup();
    obligations
}

pub(crate) fn profile_obligation_list(policy_profile: &str, key: &str) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    wcag22_profile()
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("obligation").and_then(|value| value.as_str()))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn wcag22_profile() -> &'static serde_json::Value {
    static PROFILE: OnceLock<serde_json::Value> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(WCAG22_AA_PROFILE_JSON)
            .expect("embedded wcag22-aa profile is valid JSON")
    })
}

fn criteria_with_method(policy_profile: &str, method: &str) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }
    wcag22_success_criteria()
        .into_iter()
        .filter(|criterion| criterion["method"].as_str() == Some(method))
        .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
        .collect()
}

pub(crate) fn wcag22_success_criteria() -> Vec<serde_json::Value> {
    wcag22_profile()
        .get("success_criteria")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn wcag22_success_criterion_ids() -> BTreeSet<String> {
    wcag22_success_criteria()
        .into_iter()
        .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
        .collect()
}

pub(crate) fn criterion_title(obligation: &str) -> String {
    wcag22_success_criteria()
        .into_iter()
        .find(|criterion| criterion["obligation"].as_str() == Some(obligation))
        .and_then(|criterion| {
            let num = criterion["num"].as_str()?;
            let handle = criterion["handle"].as_str()?;
            Some(format!("{num} {handle}"))
        })
        .or_else(|| profile_obligation_title(obligation))
        .unwrap_or_else(|| obligation.to_string())
}

pub(crate) fn criterion_principle(obligation: &str) -> String {
    criterion_field(obligation, "principle").unwrap_or_else(|| "Supporting Checks".to_string())
}

pub(crate) fn criterion_level(obligation: &str) -> String {
    criterion_field(obligation, "level").unwrap_or_default()
}

pub(crate) fn criterion_field(obligation: &str, field: &str) -> Option<String> {
    wcag22_success_criteria()
        .into_iter()
        .find(|criterion| criterion["obligation"].as_str() == Some(obligation))
        .and_then(|criterion| criterion[field].as_str().map(ToString::to_string))
}

pub(crate) fn criterion_source_url(obligation: &str) -> Option<String> {
    wcag22_success_criteria()
        .into_iter()
        .find(|criterion| criterion["obligation"].as_str() == Some(obligation))
        .and_then(|criterion| criterion["source_url"].as_str().map(ToString::to_string))
}

fn profile_obligation_title(obligation: &str) -> Option<String> {
    let profile = wcag22_profile();
    if profile["deterministic_pass_obligation"]["obligation"].as_str() == Some(obligation) {
        return profile["deterministic_pass_obligation"]["title"]
            .as_str()
            .map(ToString::to_string);
    }
    ["scripted_obligations", "human_review_obligations"]
        .into_iter()
        .filter_map(|key| profile.get(key).and_then(|value| value.as_array()))
        .flat_map(|items| items.iter())
        .find(|item| item["obligation"].as_str() == Some(obligation))
        .and_then(|item| item["title"].as_str().map(ToString::to_string))
}

fn feature_not_applicable(obligation: &str, features: &PageFeatures) -> bool {
    let media_absent = features.audio == 0 && features.video == 0;
    let inputs_absent = features.forms == 0 && features.inputs == 0;
    matches!(
        obligation,
        "wcag22-aa:1.2.1-audio-only-and-video-only-prerecorded"
            | "wcag22-aa:1.2.2-captions-prerecorded"
            | "wcag22-aa:1.2.3-audio-description-or-media-alternative-prerecorded"
            | "wcag22-aa:1.2.4-captions-live"
            | "wcag22-aa:1.2.5-audio-description-prerecorded"
        if media_absent
    ) || (obligation == "wcag22-aa:1.4.2-audio-control" && features.audio == 0)
        || (matches!(
            obligation,
            "wcag22-aa:1.3.5-identify-input-purpose"
                | "wcag22-aa:3.3.1-error-identification"
                | "wcag22-aa:3.3.3-error-suggestion"
                | "wcag22-aa:3.3.4-error-prevention-legal-financial-data"
                | "wcag22-aa:3.3.7-redundant-entry"
                | "wcag22-aa:3.3.8-accessible-authentication-minimum"
        ) && inputs_absent)
        || (obligation == "wcag22-aa:2.5.7-dragging-movements" && features.draggable == 0)
}

pub(crate) fn applicability_reason(obligation: &str) -> String {
    match obligation {
        o if o.starts_with("wcag22-aa:1.2.") => {
            "No <audio> or <video> elements were detected on the inspected states, so this time-based media criterion does not apply.".to_string()
        }
        "wcag22-aa:1.4.2-audio-control" => {
            "No <audio> elements were detected, so audio control does not apply.".to_string()
        }
        "wcag22-aa:1.3.5-identify-input-purpose"
        | "wcag22-aa:3.3.1-error-identification"
        | "wcag22-aa:3.3.3-error-suggestion"
        | "wcag22-aa:3.3.4-error-prevention-legal-financial-data"
        | "wcag22-aa:3.3.7-redundant-entry"
        | "wcag22-aa:3.3.8-accessible-authentication-minimum" => {
            "No forms or input fields were detected on the inspected states, so this input-assistance criterion does not apply.".to_string()
        }
        "wcag22-aa:2.5.7-dragging-movements" => {
            "No draggable elements were detected, so dragging-movement alternatives do not apply.".to_string()
        }
        other => format!(
            "{} was determined not applicable to the inspected content.",
            criterion_title(other)
        ),
    }
}

pub(crate) fn criterion_feature_verdict(
    obligation: &str,
    method: &str,
    features: &PageFeatures,
    keyboard_observed: bool,
) -> (&'static str, &'static str, &'static str, &'static str) {
    if feature_not_applicable(obligation, features) {
        return (
            "not_applicable",
            "machine_proven",
            "applicability",
            "allie-applicability-oracle",
        );
    }
    match obligation {
        "wcag22-aa:3.1.1-language-of-page" => {
            return if features.lang {
                (
                    "pass",
                    "machine_proven",
                    "deterministic",
                    "allie-lang-attribute-check",
                )
            } else {
                (
                    "fail",
                    "machine_proven",
                    "deterministic",
                    "allie-lang-attribute-check",
                )
            };
        }
        "wcag22-aa:1.4.10-reflow" if features.reflow_checked => {
            return if features.reflow_overflow {
                (
                    "fail",
                    "script_observed",
                    "scripted",
                    "allie-reflow-320px-check",
                )
            } else {
                (
                    "pass",
                    "script_observed",
                    "scripted",
                    "allie-reflow-320px-check",
                )
            };
        }
        _ => {}
    }
    match method {
        "axe" => (
            "pass",
            "machine_proven",
            "deterministic",
            "axe-core-success-criterion-tags",
        ),
        "scripted" if keyboard_observed && obligation.contains("keyboard") => (
            "pass",
            "script_observed",
            "scripted",
            "playwright-keyboard-traversal",
        ),
        _ => (
            "needs_review",
            "requires_human_or_agent_review",
            "agentic",
            "allie-agentic-review-queue",
        ),
    }
}

pub(crate) fn supporting_check_related_criteria(obligation: &str) -> Vec<String> {
    match obligation {
        "wcag22-aa:deterministic-axe-rules" => wcag22_success_criteria()
            .into_iter()
            .filter(|criterion| criterion["method"].as_str() == Some("axe"))
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        "wcag22-aa:2.1.1-keyboard-traversal" => vec![
            "wcag22-aa:2.1.1-keyboard".to_string(),
            "wcag22-aa:2.1.2-no-keyboard-trap".to_string(),
            "wcag22-aa:2.4.3-focus-order".to_string(),
            "wcag22-aa:2.4.7-focus-visible".to_string(),
            "wcag22-aa:2.4.11-focus-not-obscured-minimum".to_string(),
        ],
        "wcag22-aa:1.4.10-zoom-reflow" => vec![
            "wcag22-aa:1.4.4-resize-text".to_string(),
            "wcag22-aa:1.4.10-reflow".to_string(),
            "wcag22-aa:1.4.12-text-spacing".to_string(),
        ],
        "wcag22-aa:2.2.2-reduced-motion" => vec![
            "wcag22-aa:2.2.2-pause-stop-hide".to_string(),
            "wcag22-aa:2.3.1-three-flashes-or-below-threshold".to_string(),
            "wcag22-aa:2.5.4-motion-actuation".to_string(),
        ],
        "wcag22-aa:human-content-meaning" => wcag22_success_criteria()
            .into_iter()
            .filter(|criterion| criterion["method"].as_str() == Some("human_review"))
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        "wcag22-aa:human-assistive-technology-review" => wcag22_success_criteria()
            .into_iter()
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn residual_review_need(method: &str, status: &str) -> String {
    match status {
        "pass" if method == "axe" => {
            "Deterministic evidence is present; sample with human review if policy requires."
                .to_string()
        }
        "pass" => "Evidence is present; retain replay proof for review.".to_string(),
        "fail" => "Fix outside Allie, rerun, and sign off with updated evidence.".to_string(),
        "waived" | "risk_accepted" => {
            "Review waiver provenance and expiry before release reliance.".to_string()
        }
        "not_applicable" => "Confirm applicability rationale with the product owner.".to_string(),
        "needs_review" => {
            "Human or agentic review required before making a compliance claim.".to_string()
        }
        _ => "No evidence in this packet for this criterion, surface, and state.".to_string(),
    }
}

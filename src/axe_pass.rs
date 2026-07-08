use crate::FlowManifest;
use crate::model::Verdict;
use crate::standards::WCAG22_AA_PROFILE_JSON;
use crate::worker::WorkerResponse;
use std::sync::OnceLock;

pub(crate) fn verdict(
    manifest: &FlowManifest,
    response: &WorkerResponse,
    obligation: &str,
) -> Option<Verdict> {
    let rule_id = pass_trustworthy_rule(&manifest.policy.profile, obligation)?;
    let affected_states = response
        .states
        .iter()
        .filter(|state| {
            state
                .axe_passes
                .iter()
                .any(|pass| pass.id == rule_id && pass.nodes > 0)
        })
        .map(|state| state.id.clone())
        .collect::<Vec<_>>();
    if affected_states.is_empty() {
        return None;
    }
    Some(Verdict {
        obligation: obligation.to_string(),
        status: "pass".to_string(),
        confidence: "machine_proven".to_string(),
        evidence_class: "deterministic".to_string(),
        source: format!("axe-core:{rule_id}"),
        affected_states,
        finding_refs: Vec::new(),
    })
}

fn pass_trustworthy_rule(policy_profile: &str, obligation: &str) -> Option<String> {
    if policy_profile != "wcag22-aa" {
        return None;
    }
    profile()
        .get("axe_tag_map")
        .and_then(|value| value.as_object())
        .and_then(|map| {
            map.values().find_map(|entry| {
                let maps_obligation =
                    entry.get("obligation").and_then(|value| value.as_str()) == Some(obligation);
                let pass_trustworthy = entry
                    .get("pass_trustworthy")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if maps_obligation && pass_trustworthy {
                    entry
                        .get("axe_rule")
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
        })
}

fn profile() -> &'static serde_json::Value {
    static PROFILE: OnceLock<serde_json::Value> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(WCAG22_AA_PROFILE_JSON)
            .expect("embedded wcag22-aa profile is valid JSON")
    })
}

use crate::FlowManifest;
use crate::model::Verdict;
use crate::standards::{is_mobile_web_criterion, wcag22_profile};
use crate::worker::{AxeViewport, WorkerResponse};

pub(crate) fn verdict(
    manifest: &FlowManifest,
    response: &WorkerResponse,
    obligation: &str,
) -> Option<Verdict> {
    let rule = pass_trustworthy_rule(&manifest.policy.profile, obligation)?;
    let affected_states = response
        .states
        .iter()
        .filter(|state| {
            let requires_mobile_pass = is_mobile_web_criterion(obligation);
            if requires_mobile_pass
                && !state
                    .features
                    .as_ref()
                    .is_some_and(|features| features.mobile_viewport_checked)
            {
                return false;
            }
            state.axe_passes.iter().any(|pass| {
                pass.id == rule.axe_rule
                    && pass.nodes > 0
                    && pass.tags.iter().any(|tag| tag == &rule.axe_tag)
                    && (!requires_mobile_pass || pass.viewport == AxeViewport::Mobile)
            })
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
        source: format!("axe-core:{}", rule.axe_rule),
        affected_states,
        finding_refs: Vec::new(),
    })
}

#[derive(Debug)]
struct PassTrustworthyRule {
    axe_rule: String,
    axe_tag: String,
}

impl std::fmt::Display for PassTrustworthyRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.axe_rule.fmt(f)
    }
}

fn pass_trustworthy_rule(policy_profile: &str, obligation: &str) -> Option<PassTrustworthyRule> {
    if policy_profile != "wcag22-aa" {
        return None;
    }
    wcag22_profile()
        .get("axe_tag_map")
        .and_then(|value| value.as_object())
        .and_then(|map| {
            map.iter().find_map(|(tag, entry)| {
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
                        .map(|rule| PassTrustworthyRule {
                            axe_rule: rule.to_string(),
                            axe_tag: tag.to_string(),
                        })
                } else {
                    None
                }
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PageFeatures;
    use crate::worker::{AxeEvaluation, WorkerRunStatus, WorkerStateResult};
    use std::path::Path;

    #[test]
    fn non_wcag22_aa_profile_returns_none() {
        let mut manifest = manifest();
        manifest.policy.profile = "wcag21-aa".to_string();
        let response = response_with_states(vec![state(
            "desktop-pass",
            true,
            vec![pass("target-size", "mobile", 1)],
        )]);

        assert!(verdict(&manifest, &response, "wcag22-aa:2.5.8-target-size-minimum").is_none());
    }

    #[test]
    fn zero_nodes_returns_none() {
        let response = response_with_states(vec![state(
            "empty",
            true,
            vec![pass("target-size", "mobile", 0)],
        )]);

        assert!(
            verdict(
                &manifest(),
                &response,
                "wcag22-aa:2.5.8-target-size-minimum"
            )
            .is_none()
        );
    }

    #[test]
    fn desktop_only_pass_on_mobile_web_criterion_returns_none() {
        let response = response_with_states(vec![state(
            "desktop-only",
            true,
            vec![pass("target-size", "desktop", 1)],
        )]);

        assert!(
            verdict(
                &manifest(),
                &response,
                "wcag22-aa:2.5.8-target-size-minimum"
            )
            .is_none()
        );
    }

    #[test]
    fn mobile_pass_with_mobile_viewport_checked_scopes_affected_states() {
        let response = response_with_states(vec![
            state(
                "desktop-only",
                true,
                vec![pass("target-size", "desktop", 1)],
            ),
            state("mobile-pass", true, vec![pass("target-size", "mobile", 1)]),
            state(
                "mobile-not-checked",
                false,
                vec![pass("target-size", "mobile", 1)],
            ),
        ]);

        let verdict = verdict(
            &manifest(),
            &response,
            "wcag22-aa:2.5.8-target-size-minimum",
        )
        .expect("mobile pass evidence should produce a verdict");

        assert_eq!(verdict.status, "pass");
        assert_eq!(verdict.confidence, "machine_proven");
        assert_eq!(verdict.affected_states, vec!["mobile-pass"]);
    }

    fn manifest() -> FlowManifest {
        FlowManifest::load(Path::new("examples/axe-target-size-pass-flow.yml")).unwrap()
    }

    fn response_with_states(states: Vec<WorkerStateResult>) -> WorkerResponse {
        WorkerResponse {
            schema: crate::worker::response_schema().to_string(),
            status: WorkerRunStatus::Passed,
            actual_base_url: Some("http://127.0.0.1:49152".to_string()),
            states,
            errors: Vec::new(),
            nondeterminism: Vec::new(),
        }
    }

    fn state(
        id: &str,
        mobile_viewport_checked: bool,
        axe_passes: Vec<AxeEvaluation>,
    ) -> WorkerStateResult {
        WorkerStateResult {
            id: id.to_string(),
            route: "/".to_string(),
            url: "http://127.0.0.1:49152/".to_string(),
            title: "Target Size Fixture".to_string(),
            http_status: Some(200),
            screenshot_path: None,
            axe_json_path: None,
            mobile_screenshot_path: None,
            mobile_axe_json_path: None,
            dom_snapshot_path: None,
            accessibility_tree_path: None,
            video_path: None,
            trace_path: None,
            keyboard_focus_order: Vec::new(),
            axe_violations: Vec::new(),
            axe_passes,
            console_errors: Vec::new(),
            network_errors: Vec::new(),
            state_errors: Vec::new(),
            features: Some(PageFeatures {
                mobile_viewport_checked,
                mobile_viewport_width: 390,
                mobile_viewport_height: 844,
                ..Default::default()
            }),
        }
    }

    fn pass(id: &str, viewport: &str, nodes: usize) -> AxeEvaluation {
        AxeEvaluation {
            id: id.to_string(),
            tags: vec!["wcag258".to_string()],
            nodes,
            viewport: match viewport {
                "desktop" => AxeViewport::Desktop,
                "mobile" => AxeViewport::Mobile,
                unexpected => panic!("unexpected viewport {unexpected}"),
            },
        }
    }
}

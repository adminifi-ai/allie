//! Deterministic state-step inference for generated local-fixture flows.
//!
//! This module intentionally stays conservative: it reuses operator-declared
//! state steps first, and otherwise emits only non-secret fixture probes for
//! simple controls Allie can verify through the existing browser worker.

use super::DiscoveredSurface;
use crate::{FlowManifest, StateClick, StateFill, StateStep, StateType, StateWaitFor};
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_EMAIL_VALUE: &str = "qa@example.test";
const GENERATED_EMAIL_SUFFIX: &str = ".typed";

pub(super) fn generated_steps_for_surface(
    manifest: &FlowManifest,
    manifest_path: &Path,
    surface: &DiscoveredSurface,
) -> Vec<StateStep> {
    let manifest_steps = manifest_steps_for_route(manifest, &surface.route);
    if !manifest_steps.is_empty() {
        return manifest_steps;
    }

    fixture_steps_for_route(manifest, manifest_path, &surface.route)
}

pub(super) fn manifest_steps_for_route(manifest: &FlowManifest, route: &str) -> Vec<StateStep> {
    manifest
        .flow
        .states
        .iter()
        .find(|state| state.path == route && !state.steps.is_empty())
        .map(|state| state.steps.clone())
        .unwrap_or_default()
}

fn fixture_steps_for_route(
    manifest: &FlowManifest,
    manifest_path: &Path,
    route: &str,
) -> Vec<StateStep> {
    if manifest.target.kind != "local_fixture" {
        return Vec::new();
    }
    let Some(fixture_dir) = manifest.target.fixture_dir.as_ref() else {
        return Vec::new();
    };
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let fixture_root = if fixture_dir.is_absolute() {
        fixture_dir.clone()
    } else {
        manifest_dir.join(fixture_dir)
    };
    let Some(html_path) = fixture_html_path(&fixture_root, route) else {
        return Vec::new();
    };
    let Ok(html) = fs::read_to_string(html_path) else {
        return Vec::new();
    };

    let mut steps = Vec::new();
    if let Some(mut menu_steps) = controlled_region_steps(&html) {
        steps.append(&mut menu_steps);
    }
    if let Some(mut email_steps) = email_field_steps(&html) {
        steps.append(&mut email_steps);
    }
    steps
}

fn fixture_html_path(root: &Path, route: &str) -> Option<PathBuf> {
    let route = route.trim_start_matches('/');
    let path = if route.is_empty() {
        root.join("index.html")
    } else if route.ends_with('/') {
        root.join(route).join("index.html")
    } else {
        root.join(route)
    };
    path.is_file().then_some(path)
}

fn controlled_region_steps(html: &str) -> Option<Vec<StateStep>> {
    let controls_needle = "aria-controls=\"";
    let controls_index = html.find(controls_needle)?;
    let controlled_id = attr_value_from(&html[controls_index..], "aria-controls")?;
    let trigger_tag = enclosing_tag_at(html, controls_index)?;
    let trigger_id = attr_value_from(trigger_tag, "id")?;
    let trigger_selector = css_id_selector(&trigger_id)?;
    let controlled_selector = css_id_selector(&controlled_id)?;

    Some(vec![
        StateStep::Click {
            click: StateClick {
                selector: trigger_selector,
            },
        },
        StateStep::WaitFor {
            wait_for: StateWaitFor {
                selector: Some(format!("{controlled_selector}:not([hidden])")),
                url_contains: None,
            },
        },
    ])
}

fn email_field_steps(html: &str) -> Option<Vec<StateStep>> {
    let input_tag = first_input_tag_with_email_signal(html)?;
    let input_selector = css_id_selector(&attr_value_from(&input_tag, "id")?)?;
    let wait_selector = live_region_ready_selector(html).unwrap_or_else(|| input_selector.clone());

    Some(vec![
        StateStep::Fill {
            fill: StateFill {
                selector: input_selector.clone(),
                value: GENERATED_EMAIL_VALUE.to_string(),
            },
        },
        StateStep::Type {
            r#type: StateType {
                selector: input_selector,
                text: GENERATED_EMAIL_SUFFIX.to_string(),
            },
        },
        StateStep::WaitFor {
            wait_for: StateWaitFor {
                selector: Some(wait_selector),
                url_contains: None,
            },
        },
    ])
}

fn first_input_tag_with_email_signal(html: &str) -> Option<String> {
    for segment in html.split("<input").skip(1) {
        let tag_body = segment.split('>').next()?;
        let tag = format!("<input{tag_body}>");
        if tag.contains("type=\"email\"")
            || tag.contains("autocomplete=\"email\"")
            || tag.contains("name=\"email\"")
        {
            return Some(tag);
        }
    }
    None
}

fn live_region_ready_selector(html: &str) -> Option<String> {
    if !html.contains("data-ready") {
        return None;
    }
    for segment in html.split('<').skip(1) {
        let tag = segment.split('>').next()?;
        if tag.contains("aria-live") {
            let id = attr_value_from(tag, "id")?;
            return css_id_selector(&id).map(|selector| format!("{selector}[data-ready]"));
        }
    }
    None
}

fn enclosing_tag_at(text: &str, index: usize) -> Option<&str> {
    let start = text[..index].rfind('<')?;
    let end = text[index..].find('>')? + index + 1;
    Some(&text[start..end])
}

fn attr_value_from(text: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

fn css_id_selector(id: &str) -> Option<String> {
    let is_simple_id = !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    is_simple_id.then(|| format!("#{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_region_steps_use_the_aria_controls_trigger_tag() {
        let html = r#"
          <section id="wrong"></section>
          <button id="open-menu" aria-expanded="false" aria-controls="menu">Actions</button>
          <div id="menu" hidden></div>
        "#;

        let steps = controlled_region_steps(html).unwrap();

        match &steps[0] {
            StateStep::Click { click } => assert_eq!(click.selector, "#open-menu"),
            _ => panic!("expected click step"),
        }
        match &steps[1] {
            StateStep::WaitFor { wait_for } => {
                assert_eq!(wait_for.selector.as_deref(), Some("#menu:not([hidden])"));
            }
            _ => panic!("expected wait_for step"),
        }
    }

    #[test]
    fn email_field_steps_wait_for_ready_live_region_when_available() {
        let html = r#"
          <input id="email" name="email" autocomplete="email">
          <p id="email-preview" aria-live="polite"></p>
          <script>preview.toggleAttribute('data-ready', email.value.includes('.typed'));</script>
        "#;

        let steps = email_field_steps(html).unwrap();

        match &steps[0] {
            StateStep::Fill { fill } => {
                assert_eq!(fill.selector, "#email");
                assert_eq!(fill.value, GENERATED_EMAIL_VALUE);
            }
            _ => panic!("expected fill step"),
        }
        match &steps[1] {
            StateStep::Type { r#type } => assert_eq!(r#type.text, GENERATED_EMAIL_SUFFIX),
            _ => panic!("expected type step"),
        }
        match &steps[2] {
            StateStep::WaitFor { wait_for } => {
                assert_eq!(
                    wait_for.selector.as_deref(),
                    Some("#email-preview[data-ready]")
                );
            }
            _ => panic!("expected wait_for step"),
        }
    }
}

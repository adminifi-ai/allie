use crate::{FlowManifest, ModelRedactionMode};
use std::path::Path;

fn enable_model(manifest: &mut FlowManifest) {
    manifest.model.enabled = true;
    manifest.model.redaction = Some(ModelRedactionMode::None);
}

#[test]
fn enforce_rejects_empty_effective_allowlist() {
    for provider_allowlist in [Vec::new(), vec![" ".to_string(), "\t\n".to_string()]] {
        let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        enable_model(&mut manifest);
        manifest.model.provider_allowlist = provider_allowlist;

        let error = manifest.enforce_model_provider_allowlist().unwrap_err();

        assert!(
            error
                .to_string()
                .contains("model calls are enabled but provider_allowlist is empty"),
            "enabled model with no effective provider_allowlist must fail closed: {error}"
        );
    }
}

#[test]
fn rejects_provider_not_in_allowlist() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    enable_model(&mut manifest);
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openai".to_string());
    manifest.model.base_url = Some("https://api.openai.com/v1".to_string());

    let failures = manifest.preflight_failures();

    assert!(
        failures.iter().any(|failure| {
            failure.kind == "model-policy-not-allowed"
                && failure.message.contains("provider openai")
                && failure.message.contains("provider_allowlist")
        }),
        "off-allowlist provider must fail closed before model review: {failures:?}"
    );
}

#[test]
fn rejects_allowlisted_provider_with_hostile_base_url() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    enable_model(&mut manifest);
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openrouter".to_string());
    manifest.model.base_url = Some("https://attacker.invalid/api/v1".to_string());

    let failures = manifest.preflight_failures();

    assert!(
        failures.iter().any(|failure| {
            failure.kind == "model-policy-not-allowed"
                && failure.message.contains("provider openrouter")
                && failure.message.contains("https://openrouter.ai/api/v1")
        }),
        "allowlisted provider with a non-preset endpoint must fail closed: {failures:?}"
    );
}

#[test]
fn accepts_allowlisted_provider_with_preset_base_url() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    enable_model(&mut manifest);
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];
    manifest.model.provider = Some("openrouter".to_string());
    manifest.model.base_url = Some("https://openrouter.ai/api/v1".to_string());

    let failures = manifest.preflight_failures();

    assert!(
        failures
            .iter()
            .all(|failure| failure.kind != "model-policy-not-allowed"),
        "on-allowlist provider and preset endpoint should proceed: {failures:?}"
    );
}

#[test]
fn resolves_the_entire_route_from_the_selected_provider_preset() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    enable_model(&mut manifest);
    manifest.model.provider_allowlist = vec!["openai".to_string()];
    manifest.model.provider = Some("openai".to_string());
    manifest.model.zdr_required = false;
    manifest.model.model = None;
    manifest.model.api_key_env = None;
    manifest.model.base_url = None;

    manifest.enforce_model_provider_allowlist().unwrap();
    let route = manifest.model.resolved_route();

    assert_eq!(route.provider, "openai");
    assert_eq!(route.model, "gpt-4o-mini");
    assert_eq!(route.api_key_env, "OPENAI_API_KEY");
    assert_eq!(route.base_url, "https://api.openai.com/v1");
}

#[test]
fn enabled_model_requires_explicit_supported_redaction_mode() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    manifest.model.enabled = true;
    manifest.model.provider_allowlist = vec!["openrouter".to_string()];

    let error = manifest.validate().unwrap_err();
    assert!(error.to_string().contains("model.redaction is missing"));

    let yaml = std::fs::read_to_string("examples/autonomous-workbench-agentic.yml").unwrap();
    let unsupported = yaml.replace("redaction: none", "redaction: blur-v1");
    let error = serde_yaml::from_str::<FlowManifest>(&unsupported).unwrap_err();
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn disabled_model_never_publishes_a_stale_egress_mode() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    manifest.model.enabled = false;
    manifest.model.redaction = Some(ModelRedactionMode::None);

    assert_eq!(manifest.model.accepted_redaction_mode(), None);

    manifest.model.enabled = true;
    assert_eq!(
        manifest.model.accepted_redaction_mode(),
        Some(ModelRedactionMode::None)
    );
}

use crate::FlowManifest;
use std::path::Path;

#[test]
fn rejects_provider_not_in_allowlist() {
    let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
    manifest.model.enabled = true;
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
    manifest.model.enabled = true;
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
    manifest.model.enabled = true;
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

use super::*;
use crate::test_support::MODEL_ENV_GUARD;
use tempfile::tempdir;

fn clear_model_credential_env() {
    unsafe {
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }
}

fn init_options(manifest_path: PathBuf) -> InitOptions {
    InitOptions {
        manifest_path,
        app_name: "Doctor Fixture App".to_string(),
        base_url: "http://127.0.0.1:3000".to_string(),
        fixture_dir: None,
        force: false,
    }
}

#[test]
fn init_enables_model_review_when_an_api_key_resolves() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    let receipt = run_init(init_options(manifest_path)).unwrap();
    let manifest = FlowManifest::load(&receipt.manifest_path).unwrap();

    clear_model_credential_env();

    assert!(manifest.model.enabled);
    assert_eq!(
        manifest.model.provider_allowlist,
        vec!["openrouter".to_string()]
    );
    assert_eq!(manifest.model.provider.as_deref(), Some("openrouter"));
    assert_eq!(
        manifest.model.api_key_env.as_deref(),
        Some("OPENROUTER_API_KEY")
    );
    assert!(
        manifest
            .preflight_failures()
            .iter()
            .all(|failure| failure.kind != "model-policy-incomplete")
    );
}

#[test]
fn init_prefers_openrouter_over_openai_when_both_keys_resolve() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    let receipt = run_init(init_options(manifest_path)).unwrap();
    let manifest = FlowManifest::load(&receipt.manifest_path).unwrap();

    clear_model_credential_env();

    assert_eq!(manifest.model.provider.as_deref(), Some("openrouter"));
}

#[test]
fn init_falls_back_to_openai_when_only_that_key_resolves() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "sk-openai-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    let receipt = run_init(init_options(manifest_path)).unwrap();
    let manifest = FlowManifest::load(&receipt.manifest_path).unwrap();

    clear_model_credential_env();

    assert!(manifest.model.enabled);
    assert_eq!(manifest.model.provider.as_deref(), Some("openai"));
    assert_eq!(
        manifest.model.api_key_env.as_deref(),
        Some("OPENAI_API_KEY")
    );
}

#[test]
fn init_leaves_model_review_off_when_no_api_key_resolves() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    let receipt = run_init(init_options(manifest_path)).unwrap();
    let manifest = FlowManifest::load(&receipt.manifest_path).unwrap();

    assert!(!manifest.model.enabled);
    assert!(manifest.model.provider_allowlist.is_empty());
}

fn forced_init_options(manifest_path: PathBuf) -> InitOptions {
    InitOptions {
        force: true,
        ..init_options(manifest_path)
    }
}

#[test]
fn force_reinit_on_a_fresh_path_still_auto_enables_with_key() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    // `--force` on a path with nothing on disk yet is just a fresh init.
    let receipt = run_init(forced_init_options(manifest_path)).unwrap();
    let manifest = FlowManifest::load(&receipt.manifest_path).unwrap();

    clear_model_credential_env();

    assert!(manifest.model.enabled);
    assert_eq!(manifest.model.provider.as_deref(), Some("openrouter"));
    assert!(receipt.model_note.is_none());
}

#[test]
fn force_reinit_preserves_a_deliberately_disabled_model_even_with_a_key_present() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");

    // First init with no key: model.enabled stays false. A human could
    // equally have hand-written this file with model.enabled: false.
    run_init(init_options(manifest_path.clone())).unwrap();
    let before = FlowManifest::load(&manifest_path).unwrap();
    assert!(!before.model.enabled);

    // Re-init with --force while a real key is now present: the existing
    // (deliberately off) model policy must survive, not silently flip on.
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let receipt = run_init(forced_init_options(manifest_path.clone())).unwrap();
    let after = FlowManifest::load(&manifest_path).unwrap();

    clear_model_credential_env();

    assert!(!after.model.enabled);
    assert!(after.model.provider_allowlist.is_empty());
    let note = receipt.model_note.expect("expected a preserved-model note");
    assert!(note.contains("Preserved"));
    assert!(note.contains(&manifest_path.display().to_string()));
}

#[test]
fn force_reinit_scaffolds_fresh_when_the_manifest_has_no_model_section() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");
    // Hand-write a manifest with no `model:` key at all (as if a human
    // trimmed it out, or an older Allie version wrote it).
    fs::write(
        &manifest_path,
        r#"id: no-model-section
name: No model section fixture
app_name: No Model Section
environment: local
target:
  kind: local_fixture
  fixture_dir: .
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
browser:
  viewport:
    width: 1280
    height: 900
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: no-model-section-path
  description: fixture
  states:
    - id: home
      path: /
      description: home
      required: true
      axe: true
      screenshot: true
      dom_snapshot: true
      accessibility_tree: true
      keyboard: true
      video: false
      trace: true
"#,
    )
    .unwrap();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }

    let receipt = run_init(forced_init_options(manifest_path.clone())).unwrap();
    let after = FlowManifest::load(&manifest_path).unwrap();

    clear_model_credential_env();

    assert!(after.model.enabled);
    assert_eq!(after.model.provider.as_deref(), Some("openrouter"));
    assert!(receipt.model_note.is_none());
}

#[test]
fn force_reinit_fails_loud_on_a_malformed_model_section_without_overwriting() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_credential_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("manifest.yml");
    // A `model:` key that exists but is broken (wrong types for enabled,
    // provider_allowlist, zdr_required) — deliberate config, just malformed.
    let original = r#"id: malformed-model-section
name: Malformed model section fixture
app_name: Malformed Model Section
environment: local
target:
  kind: local_fixture
  fixture_dir: .
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
model:
  enabled: "yes please"
  provider_allowlist: openrouter
  zdr_required: 12345
browser:
  viewport:
    width: 1280
    height: 900
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: malformed-model-section-path
  description: fixture
  states:
    - id: home
      path: /
      description: home
      required: true
      axe: true
      screenshot: true
      dom_snapshot: true
      accessibility_tree: true
      keyboard: true
      video: false
      trace: true
"#;
    fs::write(&manifest_path, original).unwrap();

    let error = run_init(forced_init_options(manifest_path.clone())).unwrap_err();
    let after = fs::read_to_string(&manifest_path).unwrap();

    clear_model_credential_env();

    let message = error.to_string();
    assert!(message.contains("model:"));
    assert!(message.contains("Nothing was overwritten"));
    assert_eq!(
        after, original,
        "a malformed model: section must not be silently overwritten"
    );
}

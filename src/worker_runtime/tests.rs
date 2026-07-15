use super::*;
use crate::test_support::MODEL_ENV_GUARD;
use tempfile::tempdir;

#[test]
fn explicit_missing_manifest_is_a_failed_target_check() {
    let temp = tempdir().unwrap();
    let check = check_target(Some(&temp.path().join("missing.yml")), true);

    assert_eq!(check.name, "target");
    assert_eq!(check.status, DoctorCheckStatus::Fail);
    assert!(check.detail.contains("does not exist"));
}

fn write_model_fixture_manifest(dir: &Path, model_yaml: &str) -> PathBuf {
    let manifest_path = dir.join("manifest.yml");
    std::fs::write(
        &manifest_path,
        format!(
            r#"id: doctor-model-fixture
name: Doctor model fixture
app_name: Doctor Fixture App
environment: local
target:
  kind: local_fixture
  fixture_dir: .
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
{model_yaml}
browser:
  viewport:
    width: 1280
    height: 900
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: doctor-model-fixture-path
  description: Doctor model fixture
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
"#
        ),
    )
    .unwrap();
    manifest_path
}

fn clear_model_env() {
    unsafe {
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }
}

#[test]
fn missing_manifest_path_warns_model_check_instead_of_failing() {
    let check = check_model(None);

    assert_eq!(check.name, "model");
    assert_eq!(check.status, DoctorCheckStatus::Warn);
    assert!(check.detail.contains("no manifest supplied"));
}

#[test]
fn disabled_model_with_no_resolvable_key_names_the_checked_env_vars() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_env();
    let temp = tempdir().unwrap();
    let manifest_path = write_model_fixture_manifest(temp.path(), "");

    let check = check_model(Some(&manifest_path));

    assert_eq!(check.status, DoctorCheckStatus::Warn);
    assert!(check.detail.contains("OPENROUTER_API_KEY"));
    assert!(check.detail.contains("OPENAI_API_KEY"));
    assert!(check.fix.unwrap().contains("allie init --force"));
}

#[test]
fn disabled_model_with_a_resolvable_key_says_so() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = write_model_fixture_manifest(temp.path(), "");

    let check = check_model(Some(&manifest_path));

    clear_model_env();

    assert_eq!(check.status, DoctorCheckStatus::Warn);
    assert!(check.detail.contains("is off in the manifest"));
    assert!(check.detail.contains("OPENROUTER_API_KEY"));
}

#[test]
fn enabled_model_with_its_key_set_is_ok() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_env();
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
    }
    let temp = tempdir().unwrap();
    let manifest_path = write_model_fixture_manifest(
        temp.path(),
        "model:\n  enabled: true\n  redaction: none\n  provider_allowlist:\n    - openrouter\n  zdr_required: false\n  provider: openrouter\n  api_key_env: OPENROUTER_API_KEY\n",
    );

    let check = check_model(Some(&manifest_path));

    clear_model_env();

    assert_eq!(check.status, DoctorCheckStatus::Ok);
    assert!(check.detail.contains("openrouter"));
    assert!(check.detail.contains("OPENROUTER_API_KEY"));
    assert!(check.fix.is_none());
}

#[test]
fn enabled_model_with_its_key_missing_names_it_explicitly() {
    let _guard = MODEL_ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_model_env();
    let temp = tempdir().unwrap();
    let manifest_path = write_model_fixture_manifest(
        temp.path(),
        "model:\n  enabled: true\n  redaction: none\n  provider_allowlist:\n    - openrouter\n  zdr_required: false\n  provider: openrouter\n  api_key_env: SOME_OTHER_ENV\n",
    );

    let check = check_model(Some(&manifest_path));

    assert_eq!(check.status, DoctorCheckStatus::Warn);
    assert!(check.detail.contains("SOME_OTHER_ENV"));
    assert!(check.detail.contains("is not set"));
    assert!(check.fix.unwrap().contains("SOME_OTHER_ENV"));
}

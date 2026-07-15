use super::*;
use crate::test_support::MODEL_ENV_GUARD;
use tempfile::tempdir;

#[test]
fn worker_script_resolves_from_installed_lib_asset_dir_without_env() {
    let temp = tempdir().unwrap();
    let exe_path = temp.path().join("bin/allie");
    let worker_path = temp.path().join("lib/allie/workers/browser/run.mjs");
    std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
    std::fs::write(&worker_path, "console.log('worker');\n").unwrap();

    let resolution = resolve_worker_script_from(
        None,
        &exe_path,
        &temp.path().join("missing-source-checkout"),
    )
    .unwrap();

    assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
    assert_eq!(resolution.source, "installed lib directory");
}

#[test]
fn worker_script_resolves_from_bundled_distribution_root() {
    let temp = tempdir().unwrap();
    let exe_path = temp.path().join("allie/bin/allie");
    let worker_path = temp.path().join("allie/workers/browser/run.mjs");
    std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
    std::fs::write(&worker_path, "console.log('worker');\n").unwrap();

    let resolution = resolve_worker_script_from(
        None,
        &exe_path,
        &temp.path().join("missing-source-checkout"),
    )
    .unwrap();

    assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
    assert_eq!(resolution.source, "bundled distribution root");
}

#[test]
fn worker_script_env_override_is_authoritative() {
    let temp = tempdir().unwrap();
    let env_path = temp.path().join("custom-worker.mjs");
    let packaged_path = temp.path().join("lib/allie/workers/browser/run.mjs");
    std::fs::create_dir_all(packaged_path.parent().unwrap()).unwrap();
    std::fs::write(&packaged_path, "console.log('packaged');\n").unwrap();
    std::fs::write(&env_path, "console.log('env');\n").unwrap();

    let resolution = resolve_worker_script_from(
        Some(env_path.clone()),
        &temp.path().join("bin/allie"),
        temp.path(),
    )
    .unwrap();

    assert_eq!(resolution.path, std::fs::canonicalize(env_path).unwrap());
    assert_eq!(resolution.source, BROWSER_WORKER_ENV);
}

#[test]
fn missing_env_override_does_not_silently_fallback() {
    let temp = tempdir().unwrap();
    let missing_env_path = temp.path().join("missing-worker.mjs");
    let packaged_path = temp.path().join("lib/allie/workers/browser/run.mjs");
    std::fs::create_dir_all(packaged_path.parent().unwrap()).unwrap();
    std::fs::write(&packaged_path, "console.log('packaged');\n").unwrap();

    let search = resolve_worker_script_from(
        Some(missing_env_path.clone()),
        &temp.path().join("bin/allie"),
        temp.path(),
    )
    .unwrap_err();

    assert!(search.message.contains(BROWSER_WORKER_ENV));
    assert_eq!(search.searched_paths, vec![missing_env_path]);
}

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

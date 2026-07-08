use super::*;
use crate::FlowManifest;
use std::path::Path;
use std::sync::Mutex;
use tempfile::tempdir;

static AUTH_ENV_GUARD: Mutex<()> = Mutex::new(());

#[test]
fn worker_request_carries_auth_env_names_not_secret_values() {
    let _guard = AUTH_ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    let sentinel = "sentinel-secret-do-not-serialize-7f3c";
    unsafe {
        std::env::set_var("ALLIE_AUTH_FIXTURE_PASSWORD", sentinel);
        std::env::set_var("ALLIE_AUTH_FIXTURE_USER", "qa@example.test");
    }

    let manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();
    assert!(
        manifest.preflight_failures().is_empty(),
        "preflight should pass with both auth env vars set"
    );

    let temp = tempdir().unwrap();
    let request = WorkerRequest::from_manifest(
        "run-auth-secret",
        &manifest,
        Path::new("examples/auth-fixture-flow.yml"),
        &temp.path().join("artifacts"),
        None,
    )
    .unwrap();
    let json = serde_json::to_string_pretty(&request).unwrap();

    unsafe {
        std::env::remove_var("ALLIE_AUTH_FIXTURE_PASSWORD");
        std::env::remove_var("ALLIE_AUTH_FIXTURE_USER");
    }

    assert!(
        json.contains("ALLIE_AUTH_FIXTURE_PASSWORD"),
        "request must carry the env NAME so the worker can read it"
    );
    assert!(
        !json.contains(sentinel),
        "request must NOT carry the secret VALUE (secrets stay off disk)"
    );
}

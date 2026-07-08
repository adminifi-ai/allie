use super::*;
use crate::{ExitClass, ReleaseOptions};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn prepare_out_dir_creates_a_missing_directory() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().join("fresh");
    assert!(!out_dir.exists());

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(out_dir.is_dir());
}

#[test]
fn prepare_out_dir_accepts_an_already_empty_directory() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(out_dir.is_dir());
}

#[test]
fn prepare_out_dir_refuses_a_non_empty_directory_with_no_manifest() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    fs::write(out_dir.join("mystery.txt"), "not written by allie").unwrap();

    let error = prepare_out_dir(&out_dir, "run").unwrap_err();

    let message = error.to_string();
    assert!(message.contains("run"));
    assert!(message.contains("choose a new --out directory"));
    // Refusing must not touch the unrecognized content.
    assert!(out_dir.join("mystery.txt").exists());
}

#[test]
fn finalize_then_prepare_cleans_up_exactly_the_manifested_files() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    fs::create_dir_all(out_dir.join("nested")).unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    fs::write(out_dir.join("nested/report.html"), "<html></html>").unwrap();

    finalize_out_dir_manifest(&out_dir, "run").unwrap();
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    assert!(manifest_path.exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let files: Vec<String> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    assert!(files.contains(&"evidence.json".to_string()));
    assert!(
        files
            .iter()
            .any(|file| file.replace('\\', "/") == "nested/report.html")
    );

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(!out_dir.join("evidence.json").exists());
    assert!(!out_dir.join("nested").exists());
    assert!(out_dir.is_dir());
}

/// Regression test for dogfood R5: a stale artifact from an obsolete stage
/// (tracked by an old manifest, exactly like a file an earlier allie version
/// wrote and recorded before the stage was retired) must not survive a
/// rerun into the same --out directory.
#[test]
fn prepare_out_dir_deletes_a_stale_sentinel_recorded_in_an_old_manifest() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    fs::create_dir_all(out_dir.join("remediation")).unwrap();
    fs::write(
        out_dir.join("remediation/legacy-finding.json"),
        "stale artifact from a retired stage",
    )
    .unwrap();

    // An old allie version's manifest recorded both files it wrote,
    // including the since-retired remediation stage's output.
    let old_manifest = json!({
        "schema": "allie.run-manifest.v0",
        "command": "run",
        "written_at": "2026-01-01T00:00:00Z",
        "files": ["evidence.json", "remediation/legacy-finding.json"],
    });
    fs::write(
        out_dir.join(MANIFEST_FILE_NAME),
        serde_json::to_string_pretty(&old_manifest).unwrap(),
    )
    .unwrap();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(
        !out_dir.join("remediation/legacy-finding.json").exists(),
        "stale sentinel from the retired remediation stage must be gone after a rerun"
    );
    assert!(
        !out_dir.join("remediation").exists(),
        "the now-empty remediation directory must be pruned"
    );
    assert!(!out_dir.join("evidence.json").exists());
}

fn release_packet_json() -> serde_json::Value {
    json!({
        "schema": "allie.evidence.v0",
        "summary": {
            "status": "pass",
            "exit_code": 0,
            "deterministic_failures": 0,
            "scripted_failures": 0,
            "infrastructure_failures": 0,
            "states_captured": 1,
            "failure_class": null,
        },
        "run": {
            "id": "run-1",
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "2026-01-01T00:00:01Z",
            "allie_version": "0.0.0",
            "git_sha": "test-sha",
            "git_branch": "test-branch",
            "ci_provider": null,
            "actor": "test",
        },
        "target": {
            "base_url": "http://127.0.0.1",
            "environment": "local",
            "app_name": "Out Dir Fixture",
            "auth_profile": "none",
            "credential_provider": {
                "provider": "none",
                "env": null,
                "required": false,
                "status": "not_required",
            },
            "flow_manifest": "flow.yml",
        },
        "policy": {
            "profile": "wcag22-aa",
            "blocking_classes": ["deterministic"],
            "worker_timeout_ms": 30000,
            "model_provider_allowlist": [],
            "model_status": "disabled",
            "zdr_required": false,
            "redaction_profile": "not_redacted_local",
            "budget": {"model_calls": 0, "max_states": 1},
        },
        "coverage": {
            "routes_visited": ["/"],
            "surfaces_discovered": ["Out Dir Fixture"],
            "flows_exercised": ["fixture-flow"],
            "states_captured": ["home"],
            "state_metadata": [],
            "standards_obligations_evaluated": [],
            "obligations_not_tested": [],
            "profile_human_review_scope": [],
        },
        "artifacts": [],
        "findings": [],
        "verdicts": [],
        "waivers": [],
        "agentic_assessments": [],
        "replay": {
            "command": "cargo run -- run",
            "manifest_path": "flow.yml",
            "environment_requirements": [],
            "credential_profile": "none",
            "browser": {
                "viewport": {"width": 1280, "height": 900},
                "color_scheme": "light",
                "reduced_motion": "reduce",
                "locale": "en-US",
                "zoom": 1.0,
            },
            "seed_data": [],
            "known_nondeterminism": [],
        },
    })
}

/// End-to-end proof that a real command function (`run_release`, one of the
/// four out-dirs named by AL-117) is actually wired to this hygiene module,
/// not just the standalone helpers above.
#[test]
fn run_release_cleans_a_stale_file_from_its_own_prior_manifested_run() {
    let temp = tempdir().unwrap();
    let packet_path = temp.path().join("packet.json");
    fs::write(
        &packet_path,
        serde_json::to_string_pretty(&release_packet_json()).unwrap(),
    )
    .unwrap();
    let out_dir = temp.path().join("release-out");

    let options = || ReleaseOptions {
        packet_path: packet_path.clone(),
        out_dir: out_dir.clone(),
        changed_surfaces: Vec::new(),
        stale_after_days: 30,
    };

    let first = crate::run_release(options()).unwrap();
    assert_eq!(first.exit_class, ExitClass::Success);

    // Simulate a stale artifact from an obsolete stage that a prior allie
    // version wrote and recorded in its manifest.
    fs::write(out_dir.join("legacy-remediation.json"), "obsolete").unwrap();
    let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .push(json!("legacy-remediation.json"));
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(out_dir.join("legacy-remediation.json").exists());

    let second = crate::run_release(options()).unwrap();
    assert_eq!(second.exit_class, ExitClass::Success);

    assert!(
        !out_dir.join("legacy-remediation.json").exists(),
        "rerunning `allie release` into the same --out must not leave stale artifacts behind"
    );
    assert!(second.summary_path.exists());
    assert!(second.check_path.exists());
    assert!(second.report_path.exists());
}

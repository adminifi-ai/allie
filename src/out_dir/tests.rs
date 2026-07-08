use super::*;
use crate::{ExitClass, ReleaseOptions, ReportOptions};
use serde_json::json;
use tempfile::tempdir;

fn read_manifest_json(out_dir: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(out_dir.join(MANIFEST_FILE_NAME)).unwrap()).unwrap()
}

#[test]
fn prepare_out_dir_creates_a_missing_directory_and_claims_it_in_progress() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().join("fresh");
    assert!(!out_dir.exists());

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(out_dir.is_dir());
    let manifest = read_manifest_json(&out_dir);
    assert_eq!(manifest["schema"], "allie.run-manifest.v0");
    assert_eq!(manifest["command"], "run");
    assert_eq!(manifest["phase"], "in_progress");
    assert_eq!(manifest["files"], json!([]));
}

#[test]
fn prepare_out_dir_accepts_an_already_empty_directory() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(out_dir.is_dir());
    assert!(out_dir.join(MANIFEST_FILE_NAME).exists());
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
    // Refusing must not touch the unrecognized content or claim the dir.
    assert!(out_dir.join("mystery.txt").exists());
    assert!(!out_dir.join(MANIFEST_FILE_NAME).exists());
}

#[test]
fn finalize_then_prepare_cleans_the_entire_managed_dir() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    prepare_out_dir(&out_dir, "run").unwrap();
    fs::create_dir_all(out_dir.join("nested")).unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    fs::write(out_dir.join("nested/report.html"), "<html></html>").unwrap();

    finalize_out_dir_manifest(&out_dir, "run").unwrap();
    let manifest = read_manifest_json(&out_dir);
    assert_eq!(manifest["phase"], "complete");
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

    // A file dropped into the managed dir after finalize (not in the
    // manifest) must also be cleaned: manifest presence marks the whole
    // directory as allie-owned, so the next run starts from empty.
    fs::write(out_dir.join("dropped-later.txt"), "straggler").unwrap();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(!out_dir.join("evidence.json").exists());
    assert!(!out_dir.join("nested").exists());
    assert!(!out_dir.join("dropped-later.txt").exists());
    let manifest = read_manifest_json(&out_dir);
    assert_eq!(manifest["phase"], "in_progress");
    assert_eq!(manifest["files"], json!([]));
}

/// Regression test for dogfood R5: a stale artifact from an obsolete stage,
/// sitting in a directory an earlier allie run manifested (the old manifest
/// here has no `phase` field, exactly like one written before the field
/// existed), must not survive a rerun into the same --out directory.
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
    // including the since-retired remediation stage's output. No `phase`
    // field: written before it existed, defaults to complete.
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

/// BLOCKING 1 regression: a manifest whose file list tries to escape the
/// out-dir (absolute path, or any `..` component) must be rejected as a
/// whole — hard error, nothing deleted anywhere. Deletion never uses the
/// manifest entries as paths, but a tampered manifest is a red flag allie
/// refuses to act on at all.
#[test]
fn prepare_out_dir_rejects_manifest_entries_that_escape_the_out_dir() {
    let temp = tempdir().unwrap();
    let victim = temp.path().join("victim.txt");
    fs::write(&victim, "outside the out dir").unwrap();
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();

    for escaping_entry in ["../victim.txt", "/etc/hosts", "a/../../victim.txt"] {
        let manifest = json!({
            "schema": "allie.run-manifest.v0",
            "command": "run",
            "phase": "complete",
            "written_at": "2026-01-01T00:00:00Z",
            "files": ["evidence.json", escaping_entry],
        });
        fs::write(
            out_dir.join(MANIFEST_FILE_NAME),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let error = prepare_out_dir(&out_dir, "run").unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("escape"),
            "expected an escape refusal for {escaping_entry}, got: {message}"
        );
        assert!(
            victim.exists(),
            "file outside the out dir must be untouched"
        );
        assert!(
            out_dir.join("evidence.json").exists(),
            "a rejected manifest must delete nothing, not even in-dir files"
        );
    }
}

/// BLOCKING 2 regression: a symlink inside a managed out-dir must be treated
/// as a leaf — remove the link itself, never recurse through it or delete
/// anything on the other side.
#[cfg(unix)]
#[test]
fn prepare_out_dir_never_recurses_through_symlinks() {
    let temp = tempdir().unwrap();
    let external = temp.path().join("external");
    fs::create_dir_all(external.join("empty-sub")).unwrap();
    fs::write(external.join("keep.txt"), "precious external file").unwrap();

    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    std::os::unix::fs::symlink(&external, out_dir.join("linked")).unwrap();
    let manifest = json!({
        "schema": "allie.run-manifest.v0",
        "command": "run",
        "phase": "complete",
        "written_at": "2026-01-01T00:00:00Z",
        "files": ["evidence.json"],
    });
    fs::write(
        out_dir.join(MANIFEST_FILE_NAME),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(
        external.join("empty-sub").is_dir(),
        "an empty directory behind a symlink must never be pruned"
    );
    assert!(
        external.join("keep.txt").exists(),
        "files behind a symlink must never be deleted"
    );
    assert!(
        !out_dir.join("linked").exists(),
        "the symlink itself is out-dir content and is removed as a leaf"
    );
    assert!(!out_dir.join("evidence.json").exists());
}

/// BLOCKING 3 regression: a run that crashes between prepare and finalize
/// must not strand the directory. prepare claims the dir with an
/// in_progress manifest immediately, so the next prepare knows everything
/// present was written by the crashed run and recovers instead of refusing.
#[test]
fn prepare_recovers_a_dir_stranded_by_a_crash_between_prepare_and_finalize() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();

    prepare_out_dir(&out_dir, "run").unwrap();
    assert_eq!(read_manifest_json(&out_dir)["phase"], "in_progress");

    // Simulate the crashed run's partial output; no finalize ever happens.
    fs::create_dir_all(out_dir.join("artifacts")).unwrap();
    fs::write(out_dir.join("artifacts/partial.png"), "half-written").unwrap();
    fs::write(out_dir.join("worker-request.json"), "{}").unwrap();

    prepare_out_dir(&out_dir, "run").unwrap();

    assert!(!out_dir.join("artifacts").exists());
    assert!(!out_dir.join("worker-request.json").exists());
    let manifest = read_manifest_json(&out_dir);
    assert_eq!(manifest["phase"], "in_progress");
    assert_eq!(manifest["files"], json!([]));
}

/// IMPORTANT 4 regression: a manifest written by one command must not be
/// silently consumed (and its directory cleaned) by another.
#[test]
fn prepare_out_dir_refuses_a_manifest_written_by_a_different_command() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    prepare_out_dir(&out_dir, "run").unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    finalize_out_dir_manifest(&out_dir, "run").unwrap();

    let error = prepare_out_dir(&out_dir, "report").unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("allie run") && message.contains("report"),
        "refusal must name both commands, got: {message}"
    );
    assert!(
        out_dir.join("evidence.json").exists(),
        "a command mismatch must delete nothing"
    );
}

/// Same refusal proven through a real command entrypoint: `allie report`
/// pointed at a directory a run manifested fails before reading its inputs.
#[test]
fn run_compliance_report_refuses_an_out_dir_manifested_by_run() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().join("out");
    prepare_out_dir(&out_dir, "run").unwrap();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    finalize_out_dir_manifest(&out_dir, "run").unwrap();

    let error = crate::run_compliance_report(ReportOptions {
        map_path: temp.path().join("never-read-map.json"),
        packet_path: temp.path().join("never-read-packet.json"),
        out_dir: out_dir.clone(),
    })
    .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("allie run") && message.contains("report"),
        "refusal must name both commands, got: {message}"
    );
    assert!(out_dir.join("evidence.json").exists());
}

#[test]
fn prepare_out_dir_refuses_an_unknown_manifest_schema() {
    let temp = tempdir().unwrap();
    let out_dir = temp.path().to_path_buf();
    fs::write(out_dir.join("evidence.json"), "{}").unwrap();
    let manifest = json!({
        "schema": "allie.run-manifest.v999",
        "command": "run",
        "phase": "complete",
        "written_at": "2026-01-01T00:00:00Z",
        "files": ["evidence.json"],
    });
    fs::write(
        out_dir.join(MANIFEST_FILE_NAME),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let error = prepare_out_dir(&out_dir, "run").unwrap_err();

    assert!(error.to_string().contains("allie.run-manifest.v999"));
    assert!(out_dir.join("evidence.json").exists());
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
/// not just the standalone helpers above. The run path gets the same
/// end-to-end proof through the real CLI + browser worker in
/// scripts/evidence-smoke.sh, which plants a stale sentinel between two
/// frozen reruns into the same --out.
#[test]
fn run_release_cleans_a_stale_file_from_its_own_prior_run() {
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
    assert_eq!(read_manifest_json(&out_dir)["phase"], "complete");

    // Simulate a stale artifact from an obsolete stage that an older run
    // left behind in the managed directory.
    fs::create_dir_all(out_dir.join("remediation")).unwrap();
    fs::write(out_dir.join("remediation/legacy.json"), "obsolete").unwrap();

    let second = crate::run_release(options()).unwrap();
    assert_eq!(second.exit_class, ExitClass::Success);

    assert!(
        !out_dir.join("remediation").exists(),
        "rerunning `allie release` into the same --out must not leave stale artifacts behind"
    );
    assert!(second.summary_path.exists());
    assert!(second.check_path.exists());
    assert!(second.report_path.exists());
}

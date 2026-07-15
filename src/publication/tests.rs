use super::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use tempfile::tempdir;

fn verify_fixture(root: &Path) -> (Vec<String>, Vec<String>) {
    let raw = [
        ("run/evidence.json", "evidence-secret-sk-live-1234567890"),
        (
            "run/artifacts/dom-account.html",
            "dom-private-person@example.invalid",
        ),
        (
            "run/artifacts/account.png",
            "screenshot-exif-private-customer-1234",
        ),
        (
            "run/artifacts/console-account.json",
            "console-private-ssn-123-45-6789",
        ),
        (
            "run/artifacts/network-account.json",
            "https://example.invalid/?token=network-private-token",
        ),
        (
            "run/artifacts/axe-account.html",
            "axe-private-account-number-000011112222",
        ),
    ];
    for (path, content) in raw {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }
    let reporters = root.join("reporters");
    fs::create_dir_all(&reporters).unwrap();
    fs::write(
        reporters.join("allie-report.json"),
        serde_json::to_string_pretty(&json!({
            "schema": "allie.verify.v0",
            "status": "blocked",
            "exit_code": 1,
            "generated_at": "2026-07-15T00:00:00Z",
            "release_status": "blocked",
            "run_status": "fail",
            "why": {
                "summary": "private route failed",
                "blocking": {
                    "deterministic_failures": 2,
                    "scripted_failures": 1,
                    "infrastructure_failures": 0,
                    "missing_required_evidence": []
                },
                "compliance_summary": {
                    "pass": 10,
                    "fail": 2,
                    "needs_review": 40,
                    "not_tested": 3
                }
            },
            "project_root": "/private/checkout",
            "artifacts": {"evidence_json": "run/evidence.json"}
        }))
        .unwrap(),
    )
    .unwrap();
    (
        raw.into_iter().map(|(path, _)| path.to_string()).collect(),
        raw.into_iter()
            .map(|(_, content)| content.to_string())
            .collect(),
    )
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, snapshot);
            } else {
                snapshot.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    collect(root, root, &mut snapshot);
    snapshot
}

fn output_files(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.path().is_file());
            entry.file_name().into_string().unwrap()
        })
        .collect()
}

#[test]
fn public_projection_contains_only_policy_approved_summary() {
    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    let (raw, forbidden_content) = verify_fixture(&verify_root);
    let source_before = snapshot_tree(&verify_root);
    let out = temp.path().join("public");

    let receipt = run_publication(PublicationOptions {
        verify_root: verify_root.clone(),
        out_dir: out.clone(),
        requested_paths: Vec::new(),
    })
    .unwrap();

    assert_eq!(receipt.status, PublicationStatus::Ready);
    assert!(!receipt.retryable);
    assert_eq!(
        output_files(&out),
        BTreeSet::from([
            "allie-public-summary.json".to_string(),
            "allie-public-summary.md".to_string(),
            "publication-receipt.json".to_string(),
            "allie-run-manifest.json".to_string(),
        ])
    );
    let public_outputs = [
        fs::read_to_string(out.join("allie-public-summary.json")).unwrap(),
        fs::read_to_string(out.join("allie-public-summary.md")).unwrap(),
        fs::read_to_string(out.join("publication-receipt.json")).unwrap(),
    ]
    .join("\n");
    assert!(!public_outputs.contains("/private/checkout"));
    assert!(!public_outputs.contains("run/evidence.json"));
    assert!(!public_outputs.contains("private route failed"));
    for private_value in forbidden_content {
        assert!(!public_outputs.contains(&private_value));
    }
    for path in raw {
        assert!(!out.join(path).exists());
    }
    assert_eq!(snapshot_tree(&verify_root), source_before);
}

#[test]
fn malformed_source_contract_is_sanitized_at_the_shipped_cli_boundary() {
    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    verify_fixture(&verify_root);
    let report = verify_root.join("reporters/allie-report.json");
    let original: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    let mut hostile_status = original.clone();
    hostile_status["status"] = json!("status-private-secret");
    let mut wrong_schema = original.clone();
    wrong_schema["schema"] = json!("schema-private-secret");
    let mut missing_schema = original.clone();
    missing_schema.as_object_mut().unwrap().remove("schema");
    let mut oversized = original;
    oversized["ignored"] = json!("x".repeat(1_000_000));

    for (name, value, private_marker, expected) in [
        (
            "status",
            hostile_status,
            "status-private-secret",
            "missing or invalid",
        ),
        (
            "wrong-schema",
            wrong_schema,
            "schema-private-secret",
            "missing or invalid",
        ),
        (
            "missing-schema",
            missing_schema,
            "allie.verify.v0",
            "missing or invalid",
        ),
        (
            "oversized",
            oversized,
            "xxxxxxxxxxxxxxxx",
            "safe size limit",
        ),
    ] {
        fs::write(&report, serde_json::to_vec(&value).unwrap()).unwrap();
        let out = temp.path().join(format!("public-{name}"));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = crate::cli::run_cli_with_io(
            vec![
                "publication".to_string(),
                "--verify-root".to_string(),
                verify_root.to_string_lossy().to_string(),
                "--out".to_string(),
                out.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        let stderr = String::from_utf8(stderr).unwrap();
        assert_eq!(code, 2);
        assert!(stderr.contains(expected));
        assert!(!stderr.contains(private_marker));
        assert!(!out.join("allie-public-summary.json").exists());
    }
}

#[test]
fn overlapping_output_is_rejected_before_managed_cleanup_can_delete_evidence() {
    let temp = tempdir().unwrap();
    let first_verify = temp.path().join("first-verify");
    verify_fixture(&first_verify);
    let out = temp.path().join("public");
    run_publication(PublicationOptions {
        verify_root: first_verify,
        out_dir: out.clone(),
        requested_paths: Vec::new(),
    })
    .unwrap();
    let nested_verify = out.join("verify");
    verify_fixture(&nested_verify);
    let source_before = snapshot_tree(&nested_verify);

    let error = run_publication(PublicationOptions {
        verify_root: nested_verify.clone(),
        out_dir: out,
        requested_paths: Vec::new(),
    })
    .unwrap_err();

    assert!(error.to_string().contains("must not overlap"));
    assert_eq!(snapshot_tree(&nested_verify), source_before);
}

#[test]
fn dotdot_with_a_missing_prefix_cannot_bypass_overlap_rejection() {
    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    verify_fixture(&verify_root);
    let report = verify_root.join("reporters/allie-report.json");
    let out = temp
        .path()
        .join("missing-prefix")
        .join("..")
        .join("verify/public");

    let error = run_publication(PublicationOptions {
        verify_root,
        out_dir: out,
        requested_paths: Vec::new(),
    })
    .unwrap_err();

    assert!(error.to_string().contains("parent-directory"));
    assert!(report.exists());
    assert!(!temp.path().join("missing-prefix").exists());
}

#[cfg(unix)]
#[test]
fn symlink_plus_dotdot_is_rejected_before_path_resolution_can_enter_source() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    verify_fixture(&verify_root);
    let deep = verify_root.join("deep/deeper");
    fs::create_dir_all(&deep).unwrap();
    let source_before = snapshot_tree(&verify_root);
    let jump = temp.path().join("jump");
    symlink(&deep, &jump).unwrap();

    let error = run_publication(PublicationOptions {
        verify_root: verify_root.clone(),
        out_dir: jump.join("../public"),
        requested_paths: Vec::new(),
    })
    .unwrap_err();

    assert!(error.to_string().contains("parent-directory"));
    assert_eq!(snapshot_tree(&verify_root), source_before);
    assert!(!verify_root.join("deep/public").exists());
}

#[test]
fn raw_publication_request_is_retryably_refused_without_touching_source() {
    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    let (raw, _) = verify_fixture(&verify_root);
    let source_before = snapshot_tree(&verify_root);
    let out = temp.path().join("public");

    let receipt = run_publication(PublicationOptions {
        verify_root: verify_root.clone(),
        out_dir: out.clone(),
        requested_paths: raw.clone(),
    })
    .unwrap();

    assert_eq!(receipt.status, PublicationStatus::Refused);
    assert!(receipt.retryable);
    assert_eq!(receipt.refused.len(), raw.len());
    assert_eq!(
        output_files(&out),
        BTreeSet::from([
            "publication-receipt.json".to_string(),
            "allie-run-manifest.json".to_string(),
        ])
    );
    for path in raw {
        assert!(!out.join(path).exists());
    }
    assert_eq!(snapshot_tree(&verify_root), source_before);
    let persisted: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out.join("publication-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(persisted["status"], "refused");
    assert_eq!(persisted["retryable"], true);
    assert_eq!(persisted["publication_class"], "public_summary");
    assert!(persisted["refused"].as_array().unwrap().iter().all(|item| {
        item["publication_class"] == "sensitive_local"
            && item["reason"] == "public publishers accept only public_summary artifacts"
    }));
}

#[test]
fn hostile_include_paths_are_refused_without_becoming_public_receipt_content() {
    let temp = tempdir().unwrap();
    let verify_root = temp.path().join("verify");
    verify_fixture(&verify_root);
    let out = temp.path().join("public");
    let hostile = "/private/customer-name/../../secret-token";

    let receipt = run_publication(PublicationOptions {
        verify_root,
        out_dir: out.clone(),
        requested_paths: vec![hostile.to_string()],
    })
    .unwrap();

    assert_eq!(receipt.status, PublicationStatus::Refused);
    assert!(receipt.retryable);
    let persisted = fs::read_to_string(out.join("publication-receipt.json")).unwrap();
    assert!(!persisted.contains(hostile));
    assert!(!persisted.contains("customer-name"));
}

#[test]
fn publication_classes_have_stable_wire_names() {
    assert_eq!(
        serde_json::to_string(&PublicationClass::SensitiveLocal).unwrap(),
        "\"sensitive_local\""
    );
    assert_eq!(
        serde_json::to_string(&PublicationClass::RedactedShareable).unwrap(),
        "\"redacted_shareable\""
    );
    assert_eq!(
        serde_json::to_string(&PublicationClass::PublicSummary).unwrap(),
        "\"public_summary\""
    );
}

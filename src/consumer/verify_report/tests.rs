use super::*;
use std::path::Path;

/// AL-123: synthetic verify summary carrying all three review grains with
/// distinct counts so a reader (and this test) can tell them apart.
fn review_grain_summary() -> serde_json::Value {
    serde_json::json!({
        "schema": "allie.verify.v0",
        "status": "needs_review",
        "policy_source": "fixture/manifest.yml",
        "project_root": "fixture",
        "why": {
            "summary": "review-needed obligations remain",
            "blocking": {
                "deterministic_failures": 0,
                "scripted_failures": 0,
                "infrastructure_failures": 0,
                "missing_required_evidence": []
            },
            "review_needed_obligations": 7,
            "not_tested_obligations": 4,
            "compliance_summary": {
                "pass": 40,
                "fail": 1,
                "needs_review": 2,
                "not_tested": 3
            },
            "review": {
                "verdict_review_needed_obligations": {
                    "count": 7,
                    "grain": "verdict",
                    "label": VERDICT_REVIEW_GRAIN_LABEL
                },
                "criteria_needs_review": {
                    "count": 2,
                    "grain": "criterion",
                    "label": CRITERION_REVIEW_GRAIN_LABEL
                },
                "profile_human_review_scope": {
                    "count": 46,
                    "grain": "profile",
                    "label": PROFILE_REVIEW_GRAIN_LABEL
                }
            }
        },
        "reporters": {
            "json": "reporters/allie-report.json",
            "wcag_json": "reporters/allie-compliance-report.json",
            "html": "reporters/allie-report.html",
            "markdown": "reporters/allie-report.md",
            "junit": "reporters/junit.xml",
            "sarif": "reporters/allie.sarif"
        },
        "artifacts": {
            "evidence_json": "run/evidence.json",
            "product_map_json": "map/product-map.json",
            "surface_map_html": "map/surface-map.html",
            "compliance_html": "report/compliance-report.html",
            "release_summary_json": "release/release-summary.json",
            "release_html": "release/release-report.html"
        }
    })
}

#[test]
fn review_grains_keep_markdown_and_html_on_one_legacy_fallback_path() {
    let mut summary = review_grain_summary();
    summary["why"]["review"] = serde_json::Value::Null;

    let grains = ReviewGrains::from_summary(&summary);
    let markdown = render_verify_markdown(&summary);
    let html = render_verify_html(&summary, Path::new("/tmp/allie-out"));

    assert_eq!(grains.verdict_count, 7);
    assert_eq!(grains.criteria_count, 2);
    assert_eq!(grains.profile_count, 0);
    assert!(markdown.contains(&format!("{VERDICT_REVIEW_GRAIN_LABEL}: 7")));
    assert!(markdown.contains(&format!("{CRITERION_REVIEW_GRAIN_LABEL}: 2")));
    assert!(html.contains("Verdict-grain — 7"));
    assert!(html.contains("Criterion-grain — 2"));
}

#[test]
fn verify_markdown_labels_each_review_grain_and_reconciles_all_three() {
    let markdown = render_verify_markdown(&review_grain_summary());

    assert!(
        markdown.contains("## Review scope — what still needs review, and why"),
        "markdown must carry a single reconciled review-scope block:\n{markdown}"
    );
    assert!(markdown.contains(&format!("{VERDICT_REVIEW_GRAIN_LABEL}: 7")));
    assert!(markdown.contains(&format!("{CRITERION_REVIEW_GRAIN_LABEL}: 2")));
    assert!(markdown.contains(&format!("{PROFILE_REVIEW_GRAIN_LABEL}: 46")));
    assert!(markdown.contains("needs review 2 (criterion-grain, see Review scope below)"));
    assert!(
        !markdown
            .to_lowercase()
            .contains("is a legal compliance guarantee")
    );
}

#[test]
fn verify_html_labels_each_review_grain_and_reconciles_all_three() {
    let html = render_verify_html(&review_grain_summary(), Path::new("/tmp/allie-out"));

    assert!(html.contains("Review scope — what still needs review, and why"));
    assert!(html.contains("Review needed (verdict-grain)"));
    assert!(html.contains("Needs review (criterion-grain)"));
    assert!(html.contains(VERDICT_REVIEW_GRAIN_LABEL));
    assert!(html.contains(CRITERION_REVIEW_GRAIN_LABEL));
    assert!(html.contains(PROFILE_REVIEW_GRAIN_LABEL));
    assert!(html.contains("Verdict-grain — 7"));
    assert!(html.contains("Criterion-grain — 2"));
    assert!(html.contains("Profile-scope — 46"));
    assert!(html.contains(".statgrid, .statgrid.statgrid-3"));
    assert!(
        !html
            .to_lowercase()
            .contains("is a legal compliance guarantee")
    );
}

//! Verify reporter presentation: Markdown/HTML allie-report surfaces and the
//! labeled review-grain block (AL-123). Count computation lives in
//! `crate::review`; this module only labels and renders.

use crate::model::EvidencePacket;
use crate::review::ReviewSummary;
use std::path::Path;

// AL-123: the verify report prints three genuinely different "review" counts
// (see `crate::review` for why they're not duplicates). Each one gets a
// stable one-line grain label at the point it's printed, in both Markdown and
// HTML, plus a single reconciled block naming all three. These constants are
// the one place that wording lives so the Markdown and HTML renderers can't
// drift from each other.
pub(super) const VERDICT_REVIEW_GRAIN_LABEL: &str =
    "Verdict-grain: raw needs_review verdicts from this run, before per-criterion aggregation";
pub(super) const CRITERION_REVIEW_GRAIN_LABEL: &str =
    "Criterion-grain: WCAG success criteria aggregated to needs_review across every surface/state";
pub(super) const PROFILE_REVIEW_GRAIN_LABEL: &str = "Profile-scope: obligations this policy profile always designates for human judgment, independent of this run outcome";

/// Attach the three labeled review grains onto a verify summary's `why.review`
/// object so JSON, Markdown, and HTML all read one shared source.
pub(super) fn attach_review_grains(
    summary: &mut serde_json::Value,
    review: &ReviewSummary,
    criteria_needs_review: u64,
) {
    summary["why"]["review"] = serde_json::json!({
        "verdict_review_needed_obligations": {
            "count": review.verdict_review_needed_obligations.len(),
            "grain": "verdict",
            "label": VERDICT_REVIEW_GRAIN_LABEL
        },
        "criteria_needs_review": {
            "count": criteria_needs_review,
            "grain": "criterion",
            "label": CRITERION_REVIEW_GRAIN_LABEL
        },
        "profile_human_review_scope": {
            "count": review.profile_human_review_scope.len(),
            "grain": "profile",
            "label": PROFILE_REVIEW_GRAIN_LABEL
        }
    });
}

/// Load the evidence packet and attach labeled review grains to the summary.
pub(super) fn attach_review_grains_from_packet(
    summary: &mut serde_json::Value,
    packet: &EvidencePacket,
    criteria_needs_review: u64,
) {
    let review = crate::review::review_summary(packet, None);
    attach_review_grains(summary, &review, criteria_needs_review);
}

pub(super) fn render_verify_markdown(summary: &serde_json::Value) -> String {
    let verdict_count = summary["why"]["review"]["verdict_review_needed_obligations"]["count"]
        .as_u64()
        .unwrap_or_default();
    let verdict_label = summary["why"]["review"]["verdict_review_needed_obligations"]["label"]
        .as_str()
        .unwrap_or(VERDICT_REVIEW_GRAIN_LABEL);
    let criteria_count = summary["why"]["review"]["criteria_needs_review"]["count"]
        .as_u64()
        .unwrap_or_default();
    let criteria_label = summary["why"]["review"]["criteria_needs_review"]["label"]
        .as_str()
        .unwrap_or(CRITERION_REVIEW_GRAIN_LABEL);
    let profile_count = summary["why"]["review"]["profile_human_review_scope"]["count"]
        .as_u64()
        .unwrap_or_default();
    let profile_label = summary["why"]["review"]["profile_human_review_scope"]["label"]
        .as_str()
        .unwrap_or(PROFILE_REVIEW_GRAIN_LABEL);
    format!(
        "# Allie Verification Summary\n\n\
        Status: `{status}`\n\n\
        Why: {why}\n\n\
        Manifest: `{manifest}`\n\
        Project root: `{project_root}`\n\n\
        Blocking evidence: deterministic failures {deterministic_failures}, scripted failures {scripted_failures}, infrastructure failures {infrastructure_failures}, missing required evidence {missing_required}.\n\
        WCAG matrix: pass {wcag_pass}, fail {wcag_fail}, needs review {wcag_review} (criterion-grain, see Review scope below), not tested {wcag_not_tested}.\n\n\
        ## Review scope — what still needs review, and why\n\n\
        Three counts answer different questions about the same run and will not match, by design:\n\n\
        - {verdict_label}: {verdict_count}\n\
        - {criteria_label}: {criteria_count}\n\
        - {profile_label}: {profile_count}\n\n\
        Not-tested obligations (no verdict recorded this run): {not_tested}.\n\n\
        Reporters:\n\
        - JSON summary: `{reporters_json}`\n\
        - WCAG JSON: `{reporters_wcag_json}`\n\
        - HTML: `{reporters_html}`\n\
        - Markdown: `{reporters_markdown}`\n\
        - JUnit: `{reporters_junit}`\n\
        - SARIF: `{reporters_sarif}`\n\n\
        Evidence packet: `{evidence_json}`\n\
        Product map: `{product_map_json}`\n\
        WCAG HTML: `{compliance_html}`\n\
        Release summary: `{release_summary_json}`\n\n\
        This is evidence visibility for accessibility engineering review, not a legal compliance guarantee.\n",
        status = summary["status"].as_str().unwrap_or("unknown"),
        why = summary["why"]["summary"].as_str().unwrap_or("unknown"),
        manifest = summary["policy_source"].as_str().unwrap_or("unknown"),
        project_root = summary["project_root"].as_str().unwrap_or("unknown"),
        deterministic_failures = summary["why"]["blocking"]["deterministic_failures"]
            .as_u64()
            .unwrap_or_default(),
        scripted_failures = summary["why"]["blocking"]["scripted_failures"]
            .as_u64()
            .unwrap_or_default(),
        infrastructure_failures = summary["why"]["blocking"]["infrastructure_failures"]
            .as_u64()
            .unwrap_or_default(),
        missing_required = summary["why"]["blocking"]["missing_required_evidence"]
            .as_array()
            .map(|values| values.len())
            .unwrap_or_default(),
        wcag_pass = summary["why"]["compliance_summary"]["pass"]
            .as_u64()
            .unwrap_or_default(),
        wcag_fail = summary["why"]["compliance_summary"]["fail"]
            .as_u64()
            .unwrap_or_default(),
        wcag_review = criteria_count,
        wcag_not_tested = summary["why"]["compliance_summary"]["not_tested"]
            .as_u64()
            .unwrap_or_default(),
        verdict_label = verdict_label,
        verdict_count = verdict_count,
        criteria_label = criteria_label,
        criteria_count = criteria_count,
        profile_label = profile_label,
        profile_count = profile_count,
        not_tested = summary["why"]["not_tested_obligations"]
            .as_u64()
            .unwrap_or_default(),
        reporters_json = summary["reporters"]["json"].as_str().unwrap_or(""),
        reporters_wcag_json = summary["reporters"]["wcag_json"].as_str().unwrap_or(""),
        reporters_html = summary["reporters"]["html"].as_str().unwrap_or(""),
        reporters_markdown = summary["reporters"]["markdown"].as_str().unwrap_or(""),
        reporters_junit = summary["reporters"]["junit"].as_str().unwrap_or(""),
        reporters_sarif = summary["reporters"]["sarif"].as_str().unwrap_or(""),
        evidence_json = summary["artifacts"]["evidence_json"].as_str().unwrap_or(""),
        product_map_json = summary["artifacts"]["product_map_json"]
            .as_str()
            .unwrap_or(""),
        compliance_html = summary["artifacts"]["compliance_html"]
            .as_str()
            .unwrap_or(""),
        release_summary_json = summary["artifacts"]["release_summary_json"]
            .as_str()
            .unwrap_or("")
    )
}

pub(super) fn render_verify_html(summary: &serde_json::Value, out_dir: &Path) -> String {
    let status = summary["status"].as_str().unwrap_or("unknown");
    let links = [
        (
            "JSON summary",
            summary["reporters"]["json"].as_str().unwrap_or(""),
        ),
        (
            "WCAG JSON",
            summary["reporters"]["wcag_json"].as_str().unwrap_or(""),
        ),
        (
            "Markdown",
            summary["reporters"]["markdown"].as_str().unwrap_or(""),
        ),
        (
            "JUnit",
            summary["reporters"]["junit"].as_str().unwrap_or(""),
        ),
        (
            "SARIF",
            summary["reporters"]["sarif"].as_str().unwrap_or(""),
        ),
        (
            "WCAG report",
            summary["artifacts"]["compliance_html"]
                .as_str()
                .unwrap_or(""),
        ),
        (
            "Product map",
            summary["artifacts"]["surface_map_html"]
                .as_str()
                .unwrap_or(""),
        ),
        (
            "Release projection",
            summary["artifacts"]["release_html"].as_str().unwrap_or(""),
        ),
    ]
    .into_iter()
    .map(|(label, path)| {
        let href = if path.starts_with("reporters/") {
            path.trim_start_matches("reporters/").to_string()
        } else {
            format!("../{path}")
        };
        format!(
            "<li><a href=\"{}\">{}<code>{}</code></a></li>",
            escape_html(&href),
            escape_html(label),
            escape_html(path)
        )
    })
    .collect::<Vec<_>>()
    .join("");
    let why = summary["why"]["summary"].as_str().unwrap_or("unknown");
    let deterministic_failures = summary["why"]["blocking"]["deterministic_failures"]
        .as_u64()
        .unwrap_or_default();
    let scripted_failures = summary["why"]["blocking"]["scripted_failures"]
        .as_u64()
        .unwrap_or_default();
    let infrastructure_failures = summary["why"]["blocking"]["infrastructure_failures"]
        .as_u64()
        .unwrap_or_default();
    let missing_required = summary["why"]["blocking"]["missing_required_evidence"]
        .as_array()
        .map(|values| values.len())
        .unwrap_or_default();
    let not_tested = summary["why"]["not_tested_obligations"]
        .as_u64()
        .unwrap_or_default();
    let wcag_pass = summary["why"]["compliance_summary"]["pass"]
        .as_u64()
        .unwrap_or_default();
    let wcag_fail = summary["why"]["compliance_summary"]["fail"]
        .as_u64()
        .unwrap_or_default();
    let wcag_not_tested = summary["why"]["compliance_summary"]["not_tested"]
        .as_u64()
        .unwrap_or_default();
    // AL-123: the three review grains, reconciled once here so the stat
    // tiles above and the "Review scope" section below can't disagree.
    // Prefer the labeled why.review counts over the bare legacy fields so
    // every printed number shares one source.
    let verdict_review_count =
        summary["why"]["review"]["verdict_review_needed_obligations"]["count"]
            .as_u64()
            .unwrap_or_else(|| {
                summary["why"]["review_needed_obligations"]
                    .as_u64()
                    .unwrap_or_default()
            });
    let verdict_review_label =
        summary["why"]["review"]["verdict_review_needed_obligations"]["label"]
            .as_str()
            .unwrap_or(VERDICT_REVIEW_GRAIN_LABEL);
    let criteria_review_count = summary["why"]["review"]["criteria_needs_review"]["count"]
        .as_u64()
        .unwrap_or_else(|| {
            summary["why"]["compliance_summary"]["needs_review"]
                .as_u64()
                .unwrap_or_default()
        });
    let criteria_review_label = summary["why"]["review"]["criteria_needs_review"]["label"]
        .as_str()
        .unwrap_or(CRITERION_REVIEW_GRAIN_LABEL);
    let profile_review_count = summary["why"]["review"]["profile_human_review_scope"]["count"]
        .as_u64()
        .unwrap_or_default();
    let profile_review_label = summary["why"]["review"]["profile_human_review_scope"]["label"]
        .as_str()
        .unwrap_or(PROFILE_REVIEW_GRAIN_LABEL);
    let (bcls, dot) = match status {
        "blocked" | "failed" => ("b-fail", "#d23b30"),
        "approved" | "pass" => ("b-pass", "#1a9457"),
        _ => ("b-review", "#d8a32f"),
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Allie verification — {status}</title>
  <style>{css}
    .statgrid {{ display: grid; grid-template-columns: repeat(5, 1fr); gap: 10px; margin: 18px 0 6px; }}
    .statgrid.statgrid-3 {{ grid-template-columns: repeat(3, 1fr); }}
    .stat {{ background: #fff; border: 1px solid #e4e8ef; border-radius: 13px; padding: 13px 15px; }}
    .stat .n {{ font-size: 23px; font-weight: 700; font-variant-numeric: tabular-nums; }}
    .stat .k {{ font-size: 10.5px; letter-spacing: .06em; text-transform: uppercase; color: #5a6473; margin-top: 3px; }}
    ul.links {{ list-style: none; padding: 0; margin: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px; }}
    ul.links a {{ display: block; background: #fff; border: 1px solid #e4e8ef; border-radius: 11px; padding: 12px 14px; font-weight: 600; }}
    ul.links code {{ display: block; color: #5a6473; font-weight: 400; margin-top: 4px; font-size: 11px; background: none; }}
    dl.review-grains {{ margin: 6px 0 0; }}
    dl.review-grains dt {{ font-weight: 700; font-variant-numeric: tabular-nums; margin-top: 10px; }}
    dl.review-grains dd {{ margin: 2px 0 0; color: #5a6473; }}
    @media (max-width: 720px) {{ .statgrid {{ grid-template-columns: repeat(2, 1fr); }} }}
  </style>
</head>
<body>
  <main>
    <p class="eyebrow">Allie · host-agnostic verification</p>
    <h1>Verification</h1>
    <p class="sub">Manifest <code>{manifest}</code> · output <code>{out}</code> · evidence visibility, not a legal compliance guarantee</p>
    <div class="banner {bcls}"><span class="dot" style="background:{dot}"></span><div><h2>Status: {status_label}</h2><p>{why}</p></div></div>
    <section>
      <h2 class="sh">Blocking evidence</h2>
      <div class="statgrid">
        <div class="stat"><div class="n">{deterministic_failures}</div><div class="k">Deterministic fails</div></div>
        <div class="stat"><div class="n">{scripted_failures}</div><div class="k">Scripted fails</div></div>
        <div class="stat"><div class="n">{infrastructure_failures}</div><div class="k">Infra fails</div></div>
        <div class="stat"><div class="n">{missing_required}</div><div class="k">Missing evidence</div></div>
        <div class="stat"><div class="n">{verdict_review_count}</div><div class="k">Review needed (verdict-grain)</div></div>
      </div>
      <h2 class="sh">WCAG 2.2 matrix</h2>
      <div class="statgrid">
        <div class="stat"><div class="n">{wcag_pass}</div><div class="k">Pass</div></div>
        <div class="stat"><div class="n">{wcag_fail}</div><div class="k">Fail</div></div>
        <div class="stat"><div class="n">{criteria_review_count}</div><div class="k">Needs review (criterion-grain)</div></div>
        <div class="stat"><div class="n">{not_tested}</div><div class="k">Not-tested obligations</div></div>
        <div class="stat"><div class="n">{wcag_not_tested}</div><div class="k">WCAG not tested</div></div>
      </div>
    </section>
    <section>
      <h2 class="sh">Review scope — what still needs review, and why</h2>
      <p class="sub">Three counts answer different questions about the same run and will not match, by design.</p>
      <div class="statgrid statgrid-3">
        <div class="stat"><div class="n">{verdict_review_count}</div><div class="k">Verdict-grain</div></div>
        <div class="stat"><div class="n">{criteria_review_count}</div><div class="k">Criterion-grain</div></div>
        <div class="stat"><div class="n">{profile_review_count}</div><div class="k">Profile-scope</div></div>
      </div>
      <dl class="review-grains">
        <dt>Verdict-grain — {verdict_review_count}</dt>
        <dd>{verdict_review_label}</dd>
        <dt>Criterion-grain — {criteria_review_count}</dt>
        <dd>{criteria_review_label}</dd>
        <dt>Profile-scope — {profile_review_count}</dt>
        <dd>{profile_review_label}</dd>
      </dl>
    </section>
    <section>
      <h2 class="sh">Reporter artifacts</h2>
      <ul class="links">{links}</ul>
    </section>
  </main>
</body>
</html>
"#,
        css = crate::report::REPORT_CSS,
        status = escape_html(status),
        status_label = escape_html(&crate::report::cr_status_label(status)),
        bcls = bcls,
        dot = dot,
        why = escape_html(why),
        manifest = escape_html(summary["policy_source"].as_str().unwrap_or("unknown")),
        out = escape_html(&out_dir.to_string_lossy()),
        deterministic_failures = deterministic_failures,
        scripted_failures = scripted_failures,
        infrastructure_failures = infrastructure_failures,
        missing_required = missing_required,
        not_tested = not_tested,
        wcag_pass = wcag_pass,
        wcag_fail = wcag_fail,
        wcag_not_tested = wcag_not_tested,
        verdict_review_count = verdict_review_count,
        verdict_review_label = escape_html(verdict_review_label),
        criteria_review_count = criteria_review_count,
        criteria_review_label = escape_html(criteria_review_label),
        profile_review_count = profile_review_count,
        profile_review_label = escape_html(profile_review_label),
        links = links
    )
}

fn escape_html(value: &str) -> String {
    crate::escape_html(value)
}

#[cfg(test)]
mod tests {
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
    fn verify_markdown_labels_each_review_grain_and_reconciles_all_three() {
        let markdown = render_verify_markdown(&review_grain_summary());

        assert!(
            markdown.contains("## Review scope — what still needs review, and why"),
            "markdown must carry a single reconciled review-scope block:\n{markdown}"
        );
        assert!(
            markdown.contains(&format!("{VERDICT_REVIEW_GRAIN_LABEL}: 7")),
            "verdict-grain count must print with its label:\n{markdown}"
        );
        assert!(
            markdown.contains(&format!("{CRITERION_REVIEW_GRAIN_LABEL}: 2")),
            "criterion-grain count must print with its label:\n{markdown}"
        );
        assert!(
            markdown.contains(&format!("{PROFILE_REVIEW_GRAIN_LABEL}: 46")),
            "profile-scope count must print with its label:\n{markdown}"
        );
        assert!(
            markdown.contains("needs review 2 (criterion-grain, see Review scope below)"),
            "WCAG matrix needs-review must name the criterion grain:\n{markdown}"
        );
        assert!(
            !markdown
                .to_lowercase()
                .contains("is a legal compliance guarantee"),
            "must not claim legal compliance:\n{markdown}"
        );
    }

    #[test]
    fn verify_html_labels_each_review_grain_and_reconciles_all_three() {
        let html = render_verify_html(&review_grain_summary(), Path::new("/tmp/allie-out"));

        assert!(
            html.contains("Review scope — what still needs review, and why"),
            "html must carry a single reconciled review-scope block:\n{html}"
        );
        assert!(
            html.contains("Review needed (verdict-grain)"),
            "blocking tile must name verdict-grain:\n{html}"
        );
        assert!(
            html.contains("Needs review (criterion-grain)"),
            "WCAG tile must name criterion-grain:\n{html}"
        );
        assert!(
            html.contains(VERDICT_REVIEW_GRAIN_LABEL),
            "html must print the verdict-grain label:\n{html}"
        );
        assert!(
            html.contains(CRITERION_REVIEW_GRAIN_LABEL),
            "html must print the criterion-grain label:\n{html}"
        );
        assert!(
            html.contains(PROFILE_REVIEW_GRAIN_LABEL),
            "html must print the profile-scope label:\n{html}"
        );
        assert!(
            html.contains("Verdict-grain — 7"),
            "reconciled block must show verdict count 7:\n{html}"
        );
        assert!(
            html.contains("Criterion-grain — 2"),
            "reconciled block must show criterion count 2:\n{html}"
        );
        assert!(
            html.contains("Profile-scope — 46"),
            "reconciled block must show profile-scope count 46:\n{html}"
        );
        assert!(
            !html
                .to_lowercase()
                .contains("is a legal compliance guarantee"),
            "must not claim legal compliance:\n{html}"
        );
    }
}

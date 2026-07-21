use crate::escape_html;
use crate::model::{
    ComplianceObligation, ComplianceProfileView, ComplianceReportPacket, ComplianceSupportingCheck,
    CriterionCoverageCell, EvidenceMedia, StateEvidence,
};
mod model_egress;
mod verdict;
pub(crate) use verdict::is_agentic_verdict;

pub(crate) const REPORT_CSS: &str = r#"
/* Allie clean-atomic comic-ops report skin.
   Source decision: Misty Step aesthetic v2.5.1 was reachable, while the npm
   package and exact 9bbe0f9 CSS URL were not. Reports embed this local subset
   so evidence artifacts remain offline and self-contained. */
:root{color-scheme:light;--allie-aesthetic-source:"@misty-step/aesthetic@v2.5.1";--ink:#111111;--paper:#f6efe1;--panel:#fff9ea;--blue:#0a5ea8;--accent:#ffd22e;--pass:#167a3a;--fail:#c21d1d;--review:#ffd22e;--muted:#4c4a42;--line:#111111;--white:#ffffff;--black:#000000;--body:#25231f;--faint:#d8cfbd;--fail-wash:#fff1e8;--review-wash:#fff6c9;--pass-wash:#edfff3;--evidence-wash:#fbf4df;--neutral-wash:#eee4ce;--review-light:#fffbe2}
*{box-sizing:border-box}
body{margin:0;color:var(--ink);background:var(--paper);font:15px/1.55 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;-webkit-font-smoothing:antialiased}
main{width:min(100% - 32px,1160px);margin:0 auto;padding:34px 0 90px}
a{color:var(--blue);text-decoration:none;font-weight:800}
a:hover{text-decoration:underline}
:focus-visible{outline:3px solid var(--ink);outline-offset:3px;box-shadow:0 0 0 6px var(--accent);border-radius:0}
code,.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.92em}
.eyebrow{display:inline-block;background:var(--blue);color:var(--white);border:2px solid var(--ink);padding:4px 9px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px;letter-spacing:.14em;text-transform:uppercase;font-weight:800;margin:0 0 12px}
h1{margin:0;font-size:38px;line-height:1.02;letter-spacing:0;text-transform:uppercase;font-weight:900}
.sub{color:var(--muted);margin:8px 0 0;font-size:13px}
.hard-panel{background:var(--panel);border:2px solid var(--ink);border-radius:0}
.proof-strip{border:2px solid var(--ink);background:var(--ink)}
.banner{margin:24px 0 6px;padding:16px 18px;display:flex;gap:16px;align-items:center}
.banner .dot{width:30px;height:30px;border-radius:0;flex:none;border:2px solid var(--ink)}
.banner h2{margin:0;font-size:18px;letter-spacing:0;text-transform:uppercase}
.banner p{margin:3px 0 0;font-size:13.5px}
.b-fail{background:var(--fail-wash)}
.b-review{background:var(--review-wash)}
.b-pass{background:var(--pass-wash)}
.scorecard{display:grid;grid-template-columns:repeat(6,1fr);gap:0;margin:20px 0 10px}
.score{background:var(--panel);border:0;border-right:2px solid var(--ink);padding:13px 14px}
.score:last-child{border-right:0}
.score .n{font-size:27px;font-weight:900;letter-spacing:0;font-variant-numeric:tabular-nums}
.score .k{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:var(--muted);margin-top:3px}
.score.zero .n{color:var(--pass)}
.bar{height:14px;border-radius:0;overflow:hidden;display:flex;border:2px solid var(--ink);margin:8px 0 6px;background:var(--panel)}
.bar i{display:block;height:100%}
.legend{display:flex;gap:12px;flex-wrap:wrap;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px;color:var(--muted);margin:0 0 6px}
.legend span{display:inline-flex;align-items:center;gap:6px}
.legend b{width:12px;height:12px;border-radius:0;border:1px solid var(--ink);display:inline-block}
section{margin:38px 0}
.sh{display:inline-block;background:var(--blue);color:var(--white);border:2px solid var(--ink);padding:4px 9px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12.5px;letter-spacing:.12em;text-transform:uppercase;margin:0 0 16px;font-weight:900}
.gallery{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:16px}
figure{margin:0;overflow:hidden}
figure img{display:block;width:100%;height:auto;border-bottom:2px solid var(--ink);background:var(--evidence-wash)}
figure video{display:block;width:100%;height:auto;border-bottom:2px solid var(--ink);background:var(--black)}
figcaption{padding:11px 13px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px;color:var(--muted)}
.principle{margin:24px 0}
.principle h3{font-size:19px;margin:0 0 2px;letter-spacing:0;text-transform:uppercase}
.principle .pmeta{color:var(--muted);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12.5px;margin:0 0 14px}
.crit{padding:15px 17px;margin:10px 0;position:relative}
.crit.s-pass{box-shadow:inset 8px 0 0 var(--pass)}
.crit.s-fail{box-shadow:inset 8px 0 0 var(--fail)}
.crit.s-review{box-shadow:inset 8px 0 0 var(--review)}
.crit.s-na{box-shadow:inset 8px 0 0 var(--faint)}
.crit.s-nottested{box-shadow:inset 8px 0 0 var(--blue)}
.crit-head{display:flex;justify-content:space-between;gap:14px;align-items:flex-start;flex-wrap:wrap}
.crit-id{font-weight:900;font-size:15.5px;letter-spacing:0}
.crit-id .num{display:inline-block;background:var(--blue);color:var(--white);border:2px solid var(--ink);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-variant-numeric:tabular-nums;margin-right:9px;padding:0 5px}
.chips{display:flex;gap:6px;flex-wrap:wrap;align-items:center}
.chip{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px;font-weight:900;padding:3px 8px;border-radius:0;border:2px solid var(--ink);white-space:nowrap;letter-spacing:.04em;text-transform:uppercase;color:var(--ink);background:var(--panel)}
.chip-pass{background:var(--pass);color:var(--white)}
.chip-fail{background:var(--fail);color:var(--white)}
.chip-review{background:var(--review);color:var(--ink)}
.chip-na{background:var(--neutral-wash);color:var(--ink)}
.chip-nottested{background:var(--blue);color:var(--white)}
.chip-method{background:var(--white);color:var(--ink)}
.chip-level{background:var(--white);color:var(--ink)}
.chip-ai{outline:2px solid var(--accent);outline-offset:2px}
.chip sup{font-size:.72em;font-weight:800;margin-left:1px}
.why{margin:11px 0 0;color:var(--body);font-size:14px}
.crit-foot{margin-top:12px;display:flex;gap:18px;flex-wrap:wrap;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px;color:var(--muted);align-items:center}
.ai{margin-top:13px;background:var(--review-wash);padding:12px 14px}
.ai-h{display:flex;gap:8px;align-items:center;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11.5px;font-weight:900;letter-spacing:.06em;text-transform:uppercase;color:var(--ink)}
.ai-h .v{margin-left:auto;font-weight:700}
.ai p{margin:8px 0 0;font-size:13.5px;color:var(--body)}
.ai .guide{margin-top:9px;padding:10px 12px;font-size:13px}
.ai .guide b{display:block;font-size:11px;text-transform:uppercase;letter-spacing:.05em;color:var(--muted);margin-bottom:3px}
.ai-media{display:flex;gap:10px;flex-wrap:wrap;margin-top:11px}
.ai-media figure{max-width:260px}
.pending{margin-top:13px;border:2px dashed var(--ink);background:var(--review-light);padding:11px 14px;color:var(--muted);font-size:13px}
.matrix-wrap>summary{cursor:pointer;font-weight:900;color:var(--blue);padding:10px 0;font-size:13.5px;text-transform:uppercase}
table.matrix{width:100%;border-collapse:collapse;font-size:12.5px;margin-top:8px}
table.ledger{border:2px solid var(--ink);background:var(--panel)}
table.ledger th,table.ledger td{border:1px solid var(--ink);padding:8px 10px;text-align:left;vertical-align:top}
table.ledger th{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:10.5px;text-transform:uppercase;letter-spacing:.06em;color:var(--white);position:sticky;top:0;background:var(--blue)}
.foot{margin-top:46px;color:var(--muted);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px;border-top:2px solid var(--ink);padding-top:16px}
@media(max-width:840px){.scorecard{grid-template-columns:repeat(3,1fr)}}
@media(max-width:560px){.scorecard{grid-template-columns:repeat(2,1fr)}h1{font-size:27px}}
"#;

fn cr_status_suffix(status: &str) -> &'static str {
    match status {
        "pass" => "pass",
        "fail" => "fail",
        "needs_review" => "review",
        "not_applicable" | "waived" | "risk_accepted" => "na",
        "not_tested" => "nottested",
        _ => "na",
    }
}

pub(crate) fn cr_status_label(status: &str) -> String {
    match status {
        "pass" => "Pass".to_string(),
        "fail" => "Fail".to_string(),
        "needs_review" => "Needs review".to_string(),
        "not_applicable" => "Not applicable".to_string(),
        "not_tested" => "Not tested".to_string(),
        "waived" => "Waived".to_string(),
        "risk_accepted" => "Risk accepted".to_string(),
        other => pretty_token(other),
    }
}

fn pretty_token(value: &str) -> String {
    let spaced = value.replace(['_', '-'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

fn cr_status_chip(status: &str) -> String {
    format!(
        "<span class=\"chip chip-{}\">{}</span>",
        cr_status_suffix(status),
        escape_html(&cr_status_label(status))
    )
}

/// Status chip that adds an asterisk for agentic pass/fail verdicts.
fn cr_status_chip_marked(obligation: &ComplianceObligation) -> String {
    if is_agentic_verdict(obligation) {
        format!(
            "<span class=\"chip chip-{} chip-ai\" title=\"AI reviewer's determination from the visual evidence shown below; not a machine-proven check or a human sign-off\">{}<sup>*</sup></span>",
            cr_status_suffix(&obligation.status),
            escape_html(&cr_status_label(&obligation.status)),
        )
    } else {
        cr_status_chip(&obligation.status)
    }
}

fn cr_method_label(evidence_class: &str) -> Option<&'static str> {
    match evidence_class {
        "deterministic" => Some("Automated · axe"),
        "scripted" => Some("Scripted"),
        "agentic" => Some("AI review"),
        "human" => Some("Needs human"),
        "applicability" => Some("Applicability"),
        _ => None,
    }
}

fn cr_method_chip(evidence_class: &str) -> String {
    match cr_method_label(evidence_class) {
        Some(label) => format!(
            "<span class=\"chip chip-method\">{}</span>",
            escape_html(label)
        ),
        None => String::new(),
    }
}

fn cr_media(media: &[EvidenceMedia]) -> String {
    if media.is_empty() {
        return String::new();
    }
    let figures = media
        .iter()
        .filter_map(|item| {
            let uri = item.data_uri.as_ref()?;
            let inner = if matches!(item.kind.as_str(), "clip" | "video" | "video_clip") {
                format!(
                    "<video src=\"{}\" autoplay loop muted playsinline controls></video>",
                    escape_html(uri)
                )
            } else {
                format!(
                    "<img loading=\"lazy\" src=\"{}\" alt=\"{}\">",
                    escape_html(uri),
                    escape_html(&item.caption)
                )
            };
            Some(format!(
                "<figure class=\"hard-panel\">{}<figcaption>{}</figcaption></figure>",
                inner,
                escape_html(&item.caption)
            ))
        })
        .collect::<Vec<_>>()
        .join("");
    if figures.is_empty() {
        String::new()
    } else {
        format!("<div class=\"ai-media\">{figures}</div>")
    }
}

fn cr_agentic_block(obligation: &ComplianceObligation) -> String {
    if let Some(review) = &obligation.agentic_review {
        let guide = if review.reviewer_guidance.trim().is_empty() {
            String::new()
        } else {
            format!(
                "<div class=\"guide hard-panel\"><b>For the human reviewer</b>{}</div>",
                escape_html(&review.reviewer_guidance)
            )
        };
        format!(
            "<div class=\"ai hard-panel\"><div class=\"ai-h\">AI reviewer verdict <span class=\"v\">{verdict} · {confidence} confidence</span></div><p>{rationale}</p>{guide}{media}<p class=\"sub\" style=\"margin-top:9px\">{provider} · {model}</p></div>",
            verdict = escape_html(&pretty_token(&review.assessment)),
            confidence = escape_html(&pretty_token(&review.confidence)),
            rationale = escape_html(&review.rationale),
            guide = guide,
            media = cr_media(&review.media),
            provider = escape_html(&review.provider),
            model = escape_html(&review.model),
        )
    } else if matches!(obligation.evidence_class.as_str(), "human" | "agentic")
        || obligation.status == "needs_review"
    {
        "<div class=\"pending hard-panel\">Agentic review pending; this criterion needs visual or contextual judgment. The AI reviewer will attach a screenshot, assessment, and reviewer guidance here.</div>".to_string()
    } else {
        String::new()
    }
}

fn cr_criterion_card(obligation: &ComplianceObligation) -> String {
    let level = if obligation.level.is_empty() {
        String::new()
    } else {
        format!(
            "<span class=\"chip chip-level\">Level {}</span>",
            escape_html(&obligation.level)
        )
    };
    let source = obligation
        .source_url
        .as_ref()
        .map(|url| {
            format!(
                " · <a href=\"{}\" rel=\"noreferrer\">WCAG reference ↗</a>",
                escape_html(url)
            )
        })
        .unwrap_or_default();
    let (num, title) = split_criterion_title(&obligation.title);
    format!(
        "<article class=\"crit hard-panel s-{suffix}\"><div class=\"crit-head\"><div class=\"crit-id\"><span class=\"num\">{num}</span>{title}</div><div class=\"chips\">{level}{method}{status}</div></div><p class=\"why\">{why}</p>{ai}<div class=\"crit-foot\"><span>Confidence: {confidence}</span><span>{review}{source}</span></div></article>",
        suffix = cr_status_suffix(&obligation.status),
        num = escape_html(&num),
        title = escape_html(&title),
        level = level,
        method = cr_method_chip(&obligation.evidence_class),
        status = cr_status_chip_marked(obligation),
        why = escape_html(&obligation.why),
        ai = cr_agentic_block(obligation),
        confidence = escape_html(&pretty_token(&obligation.confidence)),
        review = escape_html(&pretty_token(&obligation.human_review)),
        source = source,
    )
}

fn split_criterion_title(title: &str) -> (String, String) {
    match title.split_once(' ') {
        Some((num, rest)) if num.chars().all(|c| c.is_ascii_digit() || c == '.') => {
            (num.to_string(), rest.to_string())
        }
        _ => (String::new(), title.to_string()),
    }
}

fn cr_principle_sections(criteria: &[ComplianceObligation]) -> String {
    const ORDER: [&str; 4] = ["Perceivable", "Operable", "Understandable", "Robust"];
    let mut sections = String::new();
    for principle in ORDER {
        let rows = criteria
            .iter()
            .filter(|criterion| criterion.principle == principle)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            continue;
        }
        let pass = rows.iter().filter(|row| row.status == "pass").count();
        let cards = rows
            .iter()
            .map(|row| cr_criterion_card(row))
            .collect::<Vec<_>>()
            .join("");
        sections.push_str(&format!(
            "<div class=\"principle\"><h3>{principle}</h3><p class=\"pmeta\">{total} criteria · {pass} passing</p>{cards}</div>",
            principle = escape_html(principle),
            total = rows.len(),
            pass = pass,
            cards = cards,
        ));
    }
    sections
}

fn cr_state_gallery(states: &[StateEvidence]) -> String {
    let mut figures = Vec::new();
    for state in states {
        let mut rendered_media = false;
        for item in &state.media {
            let Some(uri) = item.data_uri.as_ref() else {
                continue;
            };
            rendered_media = true;
            figures.push(format!(
                "<figure class=\"hard-panel\"><img loading=\"lazy\" src=\"{uri}\" alt=\"{alt}\">{caption}</figure>",
                uri = escape_html(uri),
                alt = escape_html(&item.caption),
                caption = cr_state_caption(state, Some(&item.caption)),
            ));
        }
        if !rendered_media {
            figures.push(format!(
                "<figure class=\"hard-panel\">{caption}</figure>",
                caption = cr_state_caption(state, None),
            ));
        }
    }
    let figures = figures.join("");
    if figures.is_empty() {
        String::new()
    } else {
        format!(
            "<section><h2 class=\"sh\">What Allie inspected</h2><div class=\"gallery\">{figures}</div></section>"
        )
    }
}

fn cr_state_caption(state: &StateEvidence, media_caption: Option<&str>) -> String {
    let focus = if state.keyboard_focus_order.is_empty() {
        String::new()
    } else {
        format!(
            "<br>Tab order: {}",
            escape_html(&state.keyboard_focus_order.join(" -> "))
        )
    };
    let media_caption = media_caption
        .map(|caption| format!("<br>{}", escape_html(caption)))
        .unwrap_or_default();
    format!(
        "<figcaption><strong>{id}</strong> · <code>{route}</code><br>{title}{media_caption}{focus}</figcaption>",
        id = escape_html(&state.id),
        route = escape_html(&state.route),
        title = escape_html(&state.title),
        media_caption = media_caption,
        focus = focus,
    )
}

fn cr_matrix_rows(cells: &[CriterionCoverageCell]) -> String {
    cells
        .iter()
        .map(|cell| {
            format!(
                "<tr><td><code>{criterion}</code><br>{surface} · {state}</td><td>{status}</td><td>{applicability}</td><td>{method}</td><td>{confidence}</td><td>{residual}</td></tr>",
                criterion = escape_html(&cell.criterion_id),
                surface = escape_html(&cell.surface_id),
                state = escape_html(&cell.state_id),
                status = cr_status_chip(&cell.status),
                applicability = escape_html(&pretty_token(&cell.applicability)),
                method = escape_html(&pretty_token(&cell.method)),
                confidence = escape_html(&pretty_token(&cell.confidence)),
                residual = escape_html(&cell.residual_review_need),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn cr_supporting_checks(checks: &[ComplianceSupportingCheck]) -> String {
    if checks.is_empty() {
        return String::new();
    }
    let cards = checks
        .iter()
        .map(|check| {
            let related = if check.related_criteria.is_empty() {
                String::new()
            } else {
                format!(
                    "<div class=\"crit-foot\"><span>Covers {} criteria: <code>{}</code></span></div>",
                    check.related_criteria.len(),
                    escape_html(&check.related_criteria.join(", "))
                )
            };
            format!(
                "<article class=\"crit hard-panel s-{suffix}\"><div class=\"crit-head\"><div class=\"crit-id\">{title}<br><code style=\"font-size:11px;color:var(--muted)\">{id}</code></div><div class=\"chips\">{method}{status}</div></div><p class=\"why\">{why}</p>{related}</article>",
                suffix = cr_status_suffix(&check.status),
                title = escape_html(&check.title),
                id = escape_html(&check.id),
                method = cr_method_chip(&check.evidence_class),
                status = cr_status_chip(&check.status),
                why = escape_html(&check.why),
                related = related,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section><h2 class=\"sh\">Supporting checks</h2><p class=\"sub\" style=\"margin:0 0 14px\">Cross-cutting evidence passes that back the individual criteria above.</p>{cards}</section>"
    )
}

fn cr_profile_views(views: &[ComplianceProfileView]) -> String {
    if views.is_empty() {
        return String::new();
    }
    let cards = views
        .iter()
        .map(|view| {
            let missing = if view.missing_legacy_criteria.is_empty() {
                String::new()
            } else {
                format!(
                    "<p class=\"sub\">Explicit legacy gap: <code>{}</code></p>",
                    escape_html(&view.missing_legacy_criteria.join(", "))
                )
            };
            let excluded = if view.excluded_criteria.is_empty() {
                String::new()
            } else {
                format!(
                    "<p class=\"sub\">Excluded from this view: <code>{}</code></p>",
                    escape_html(&view.excluded_criteria.join(", "))
                )
            };
            let notes = if view.notes.is_empty() {
                String::new()
            } else {
                let items = view
                    .notes
                    .iter()
                    .map(|note| format!("<li>{}</li>", escape_html(note)))
                    .collect::<Vec<_>>()
                    .join("");
                format!("<ul class=\"sub\">{items}</ul>")
            };
            format!(
                "<article class=\"crit hard-panel s-review\"><div class=\"crit-head\"><div class=\"crit-id\">{label}<br><code style=\"font-size:11px;color:var(--muted)\">{id}</code></div><div class=\"chips\"><span class=\"chip chip-method\">Profile view</span></div></div><p class=\"why\">{basis}</p><div class=\"crit-foot\"><span>{included} covered ledger criteria</span><span>{total} total WCAG 2.1 A/AA criteria</span><span>{pass} pass</span><span>{fail} fail</span><span>{review} need review</span><span>{nt} not tested</span></div>{missing}{excluded}{notes}</article>",
                label = escape_html(&view.label),
                id = escape_html(&view.id),
                basis = escape_html(&view.basis),
                included = view.included_criteria.len(),
                total = view.total_success_criteria,
                pass = view.pass,
                fail = view.fail,
                review = view.needs_review,
                nt = view.not_tested,
                missing = missing,
                excluded = excluded,
                notes = notes,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section><h2 class=\"sh\">WCAG profile views</h2><p class=\"sub\" style=\"margin:0 0 14px\">Derived views over the same evidence ledger for standards consumers.</p>{cards}</section>"
    )
}

pub(crate) fn render_compliance_report(report: &ComplianceReportPacket) -> String {
    let s = &report.summary;
    let banner_class = match s.status.as_str() {
        "fail" | "blocked" => "b-fail",
        "pass" | "approved" => "b-pass",
        _ => "b-review",
    };
    let total = s.total_obligations.max(1);
    // How many pass/fail verdicts came from the AI reviewer (asterisked); derived
    // by compliance_summary alongside the other counts, not re-filtered here.
    let (ai_pass, ai_fail) = (s.ai_pass, s.ai_fail);
    let ai_total = ai_pass + ai_fail;
    let seg = |count: usize, color: &str| -> String {
        if count == 0 {
            String::new()
        } else {
            format!(
                "<i style=\"width:{:.3}%;background:{}\"></i>",
                count as f64 / total as f64 * 100.0,
                color
            )
        }
    };
    let bar = format!(
        "{}{}{}{}{}",
        seg(s.pass, "var(--pass)"),
        seg(s.fail, "var(--fail)"),
        seg(s.needs_review, "var(--review)"),
        seg(s.not_applicable, "var(--faint)"),
        seg(s.not_tested, "var(--blue)"),
    );
    let score = |n: usize, k: &str, zero_good: bool| -> String {
        let cls = if zero_good && n == 0 {
            "score zero"
        } else {
            "score"
        };
        format!(
            "<div class=\"{cls}\"><div class=\"n\">{n}</div><div class=\"k\">{k}</div></div>",
            cls = cls,
            n = n,
            k = k
        )
    };

    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\" data-allie-design=\"clean-atomic\" data-allie-aesthetic=\"misty-step-v2.5.1-local-subset\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Allie · ");
    html.push_str(&escape_html(&report.app_name));
    html.push_str(" accessibility evidence</title>\n<style>");
    html.push_str(REPORT_CSS);
    html.push_str("</style>\n</head>\n<body>\n<main>\n");
    html.push_str(&format!(
        "<p class=\"eyebrow\">Allie · accessibility release evidence</p><h1>{app}</h1><p class=\"sub\">WCAG 2.2 A/AA · generated {generated} · evidence visibility, not a legal compliance guarantee</p>",
        app = escape_html(&report.app_name),
        generated = escape_html(&report.generated_at),
    ));
    html.push_str(&format!(
        "<div class=\"banner hard-panel {bcls}\"><span class=\"dot\" style=\"background:{dot}\"></span><div><h2>Status: {status}</h2><p>{pass} passing · {fail} failing · {review} need review · {na} not applicable · {nt} not tested across {total} success criteria.</p>{aip}</div></div>",
        bcls = banner_class,
        dot = match s.status.as_str() { "fail" | "blocked" => "var(--fail)", "pass" | "approved" => "var(--pass)", _ => "var(--review)" },
        status = escape_html(&cr_status_label(&s.status)),
        pass = s.pass, fail = s.fail, review = s.needs_review, na = s.not_applicable, nt = s.not_tested,
        total = s.total_obligations,
        aip = if ai_total > 0 {
            format!("<p class=\"sub\" style=\"margin:6px 0 0\">{ai_total} of these are <b>AI-reviewed verdicts</b> (marked <sup>*</sup>): the agentic reviewer judged the page from screenshots and clips the way a human reviewer would, with the evidence attached under each criterion. {ai_pass} pass<sup>*</sup>, {ai_fail} fail<sup>*</sup>.</p>")
        } else {
            String::new()
        },
    ));
    html.push_str("<div class=\"scorecard proof-strip\">");
    html.push_str(&score(s.pass, "Pass", false));
    html.push_str(&score(s.fail, "Fail", false));
    html.push_str(&score(s.needs_review, "Needs review", false));
    html.push_str(&score(s.not_applicable, "Not applicable", false));
    html.push_str(&score(s.not_tested, "Not tested", true));
    if ai_total > 0 {
        html.push_str(&score(ai_total, "AI-reviewed *", false));
    }
    html.push_str(&score(s.total_obligations, "Criteria", false));
    html.push_str("</div>");
    html.push_str(&format!("<div class=\"bar\">{bar}</div>"));
    html.push_str("<div class=\"legend\"><span><b style=\"background:var(--pass)\"></b>Pass</span><span><b style=\"background:var(--fail)\"></b>Fail</span><span><b style=\"background:var(--review)\"></b>Needs review</span><span><b style=\"background:var(--faint)\"></b>Not applicable</span><span><b style=\"background:var(--blue)\"></b>Not tested</span></div>");
    if ai_total > 0 {
        html.push_str("<p class=\"sub\" style=\"margin:10px 0 0\"><sup>*</sup> <b>AI-reviewed verdict</b>: the agentic vision reviewer's pass/fail call from the attached visual evidence, shown with its confidence; a judgment call, not a machine-proven check or a human sign-off. Screenshots and clips are inlined under each criterion so a human can confirm or override.</p>");
    }

    html.push_str(&cr_state_gallery(&report.state_evidence));
    html.push_str(&model_egress::render(&report.model_egress_events));

    html.push_str("<section><h2 class=\"sh\">WCAG 2.2 success criteria</h2>");
    html.push_str(&cr_principle_sections(&report.criteria));
    html.push_str("</section>");

    html.push_str(&cr_profile_views(&report.profile_views));

    html.push_str(&cr_supporting_checks(&report.supporting_checks));

    if !report.criterion_coverage.is_empty() {
        html.push_str(&format!(
            "<section><details class=\"matrix-wrap\"><summary>Criterion coverage matrix: {} cells (criterion · surface · state drilldown)</summary><table class=\"matrix ledger\"><thead><tr><th>Criterion · surface · state</th><th>Status</th><th>Applicability</th><th>Method</th><th>Confidence</th><th>Residual review</th></tr></thead><tbody>{}</tbody></table></details></section>",
            report.criterion_coverage.len(),
            cr_matrix_rows(&report.criterion_coverage),
        ));
    }

    let replay = report
        .criterion_coverage
        .iter()
        .find_map(|cell| cell.replay_command.clone())
        .filter(|command| !command.is_empty());
    let reproduce = replay
        .map(|command| {
            format!(
                "Reproduce this run: <code>{}</code>. ",
                escape_html(&command)
            )
        })
        .unwrap_or_default();
    html.push_str(&format!(
        "<p class=\"foot\">{reproduce}Source packet <code>{packet}</code> · source map <code>{map}</code>. Allie reports evidence, status, confidence, and residual review needs. It does not claim legal compliance and is not a replacement for expert or lived accessibility review; evidence visibility, not a legal compliance guarantee.</p>",
        reproduce = reproduce,
        packet = escape_html(&report.source_packet),
        map = escape_html(&report.source_map),
    ));
    html.push_str("</main>\n</body>\n</html>\n");
    html
}

pub(crate) fn render_compliance_summary(report: &ComplianceReportPacket) -> String {
    let failing = report
        .criteria
        .iter()
        .filter(|obligation| obligation.status == "fail")
        .map(|obligation| format!("- {}: {}", obligation.id, obligation.why))
        .collect::<Vec<_>>()
        .join("\n");
    let egress_summary = model_egress::summary(&report.model_egress_events);
    format!(
        "# Allie WCAG Evidence Summary\n\nStatus: `{}`\n\nPass: {}. Fail: {}. Needs review: {}. Not tested: {}. Waived: {}. Risk accepted: {}. Total WCAG success criteria: {}. Supporting checks: {}.\n\n{}\n\nSource map: `{}`\nSource packet: `{}`\n\nThis report is evidence visibility for accessibility engineering review, not a legal compliance guarantee.\n\n## Failing Criteria\n\n{}\n",
        report.summary.status,
        report.summary.pass,
        report.summary.fail,
        report.summary.needs_review,
        report.summary.not_tested,
        report.summary.waived,
        report.summary.risk_accepted,
        report.summary.total_success_criteria,
        report.summary.total_supporting_checks,
        egress_summary,
        report.source_map,
        report.source_packet,
        if failing.is_empty() {
            "None.".to_string()
        } else {
            failing
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgenticAssessment, ComplianceObligation};
    use crate::standards::{
        criterion_level, criterion_principle, criterion_source_url, criterion_title,
    };

    fn sample_obligation(id: &str, status: &str) -> ComplianceObligation {
        ComplianceObligation {
            id: id.to_string(),
            title: criterion_title(id),
            status: status.to_string(),
            why: "test row".to_string(),
            surfaces: vec!["home".to_string()],
            tests: Vec::new(),
            artifact_refs: Vec::new(),
            agentic_context: Vec::new(),
            human_review: "required".to_string(),
            confidence: "human_attested".to_string(),
            evidence_class: "human".to_string(),
            source_url: criterion_source_url(id),
            finding_refs: Vec::new(),
            principle: criterion_principle(id),
            level: criterion_level(id),
            media: Vec::new(),
            agentic_review: None,
        }
    }

    #[test]
    fn agentic_pass_fail_render_with_asterisk_but_machine_results_do_not() {
        // Machine-proven pass: plain chip, no asterisk, no AI block.
        let mut machine = sample_obligation("wcag22-aa:1.4.3-contrast-minimum", "pass");
        machine.evidence_class = "deterministic".to_string();
        let machine_html = cr_criterion_card(&machine);
        assert!(
            !machine_html.contains("chip-ai") && !machine_html.contains("<sup>*</sup>"),
            "a machine-proven pass must never be asterisked"
        );

        // Agentic fail verdict: asterisked chip + the AI verdict block + evidence.
        let mut agentic = sample_obligation("wcag22-aa:1.4.11-non-text-contrast", "fail");
        agentic.evidence_class = "agentic".to_string();
        agentic.confidence = "medium".to_string();
        agentic.agentic_review = Some(AgenticAssessment {
            assessment: "fail".to_string(),
            rationale: "Icons are light gray on a white background.".to_string(),
            reviewer_guidance: "Measure icon contrast with a tool.".to_string(),
            confidence: "medium".to_string(),
            provider: "openrouter".to_string(),
            model: "google/gemini-3.5-flash".to_string(),
            media: Vec::new(),
        });
        let html = cr_criterion_card(&agentic);
        assert!(
            html.contains("chip-ai"),
            "agentic fail must carry the marker"
        );
        assert!(
            html.contains("<sup>*</sup>"),
            "agentic fail must be asterisked"
        );
        assert!(html.contains("AI reviewer verdict"));
        assert!(html.contains("Icons are light gray on a white background."));

        // The marker keys off the agentic_review evidence, so the asterisk and the
        // AI block can never diverge.
        assert!(is_agentic_verdict(&agentic));
        assert!(
            !is_agentic_verdict(&machine),
            "machine pass is never asterisked"
        );

        // Inconclusive stays unmarked (its AI block renders, but it is not a verdict).
        let mut inconclusive =
            sample_obligation("wcag22-aa:1.3.2-meaningful-sequence", "needs_review");
        inconclusive.evidence_class = "agentic".to_string();
        inconclusive.agentic_review = Some(AgenticAssessment {
            assessment: "inconclusive".to_string(),
            rationale: "Cannot settle from the captured evidence.".to_string(),
            reviewer_guidance: "Review manually.".to_string(),
            confidence: "low".to_string(),
            provider: "openrouter".to_string(),
            model: "google/gemini-3.5-flash".to_string(),
            media: Vec::new(),
        });
        assert!(!is_agentic_verdict(&inconclusive));

        // Regression guard for the split-write trap: an "agentic" evidence_class
        // with NO AI verdict attached must not be asterisked — no evidence, no mark.
        let mut agentic_class_no_evidence =
            sample_obligation("wcag22-aa:2.4.6-headings-and-labels", "pass");
        agentic_class_no_evidence.evidence_class = "agentic".to_string();
        agentic_class_no_evidence.agentic_review = None;
        assert!(!is_agentic_verdict(&agentic_class_no_evidence));
    }
}

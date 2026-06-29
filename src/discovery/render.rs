use super::{DiscoveryPacket, FlowPlanPacket};
use crate::escape_html;
use crate::model::ProductMapPacket;

pub(super) fn render_product_surface_map(map: &ProductMapPacket) -> String {
    let surfaces = map
        .surfaces
        .iter()
        .map(|surface| {
            format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&surface.id),
                escape_html(&surface.title),
                escape_html(&surface.routes.join(", ")),
                escape_html(&surface.files.join(", ")),
                escape_html(&surface.confidence),
                escape_html(&surface.user_stories.join(" "))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let workflows = map
        .workflows
        .iter()
        .map(|workflow| {
            format!(
                "<li><strong>{}</strong><br>{}<br><code>{}</code></li>",
                escape_html(&workflow.title),
                escape_html(&workflow.user_story),
                escape_html(&workflow.generated_flow_manifest)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let questions = map
        .open_questions
        .iter()
        .map(|question| format!("<li>{}</li>", escape_html(question)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Allie Product Map</title>
  <style>
    body {{ margin: 0; color: #151719; background: #f5f7fa; font: 16px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
    main {{ width: min(100% - 40px, 1120px); margin: 0 auto; padding: 40px 0; }}
    h1 {{ margin: 0 0 8px; font-size: 42px; line-height: 1.05; letter-spacing: 0; }}
    h2 {{ margin: 0 0 12px; color: #58616c; font-size: 13px; letter-spacing: 0.08em; text-transform: uppercase; }}
    section {{ background: #fff; border: 1px solid #d7dde5; margin-top: 18px; padding: 20px; }}
    table {{ width: 100%; border-collapse: collapse; }}
    th, td {{ border-bottom: 1px solid #d7dde5; padding: 10px; text-align: left; vertical-align: top; }}
    th {{ color: #58616c; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; }}
    code {{ background: #edf1f6; padding: 0.08em 0.28em; border-radius: 4px; }}
    @media (max-width: 760px) {{ main {{ width: min(100% - 24px, 1120px); }} table {{ display: block; overflow-x: auto; }} }}
  </style>
</head>
<body>
  <main>
    <p>Allie generated product map, not a legal compliance guarantee</p>
    <h1>{app_name}</h1>
    <p>Source manifest <code>{manifest}</code>. Agent runner <code>{runner}</code> status <code>{runner_status}</code>.</p>
    <section>
      <h2>Surfaces</h2>
      <table>
        <thead><tr><th>ID</th><th>Title</th><th>Routes</th><th>Files</th><th>Confidence</th><th>User Stories</th></tr></thead>
        <tbody>{surfaces}</tbody>
      </table>
    </section>
    <section>
      <h2>Workflows</h2>
      <ul>{workflows}</ul>
    </section>
    <section>
      <h2>Standards Profile</h2>
      <p><code>{profile}</code> contains {total} WCAG A/AA success criteria obligations for this report.</p>
    </section>
    <section>
      <h2>Open Review Questions</h2>
      <ul>{questions}</ul>
    </section>
  </main>
</body>
</html>
"#,
        app_name = escape_html(&map.app_name),
        manifest = escape_html(&map.source_manifest),
        runner = escape_html(&map.agent.runner),
        runner_status = escape_html(&map.agent.status),
        surfaces = surfaces,
        workflows = workflows,
        profile = escape_html(&map.standards.id),
        total = map.standards.total_obligations,
        questions = questions
    )
}

pub(super) fn render_discovery_report(
    discovery: &DiscoveryPacket,
    flow_plan: &FlowPlanPacket,
) -> String {
    let surfaces = discovery
        .surfaces
        .iter()
        .map(|surface| {
            format!(
                "<li><strong>{}</strong> <code>{}</code><br>{}</li>",
                escape_html(&surface.title),
                escape_html(&surface.route),
                escape_html(&surface.confidence)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Allie Discovery</title></head><body><main><h1>Allie discovery</h1><p>Source manifest: <code>{}</code></p><p>Generated candidates: {}</p><ul>{}</ul><p>Generated flows must replay before enforcement.</p></main></body></html>"#,
        escape_html(&discovery.run.source_manifest),
        flow_plan.candidates.len(),
        surfaces
    )
}

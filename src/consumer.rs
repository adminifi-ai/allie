use super::*;
use std::fs;
use std::path::{Path, PathBuf};

const VERIFY_SCHEMA: &str = "allie.verify.v0";

#[derive(Debug)]
pub(super) struct InitReceipt {
    pub(super) manifest_path: PathBuf,
    pub(super) next_command: String,
}

#[derive(Debug)]
pub(super) struct VerifyReceipt {
    pub(super) status: String,
    pub(super) exit_class: ExitClass,
    pub(super) summary_json_path: PathBuf,
    pub(super) summary_markdown_path: PathBuf,
    pub(super) report_json_path: PathBuf,
    pub(super) report_html_path: PathBuf,
    pub(super) junit_path: PathBuf,
    pub(super) sarif_path: PathBuf,
    pub(super) release_summary_path: PathBuf,
    pub(super) product_map_path: PathBuf,
    pub(super) evidence_path: PathBuf,
}

#[derive(Debug)]
pub(super) struct InitOptions {
    manifest_path: PathBuf,
    app_name: String,
    base_url: String,
    fixture_dir: Option<PathBuf>,
    force: bool,
}

#[derive(Debug)]
pub(super) struct VerifyOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
    project_root: Option<PathBuf>,
    agent_runner: AgentRunnerKind,
    changed_surfaces: Vec<String>,
    stale_after_days: i64,
}

#[derive(Debug)]
struct VerifyPipelineReceipts {
    discovery: DiscoveryReceipt,
    promoted: PromoteFlowReceipt,
    map: MapReceipt,
    run: RunReceipt,
    report: ComplianceReportReceipt,
    release: ReleaseReceipt,
    project_root: PathBuf,
    changed_surfaces: Vec<String>,
}

#[derive(Debug)]
struct VerifyReporterReceipt {
    summary_json_path: PathBuf,
    summary_markdown_path: PathBuf,
    report_json_path: PathBuf,
    report_html_path: PathBuf,
    junit_path: PathBuf,
    sarif_path: PathBuf,
}

pub(super) fn parse_init_options(args: &[String]) -> std::result::Result<InitOptions, String> {
    let mut manifest_path = PathBuf::from(".allie/manifest.yml");
    let mut app_name = default_app_name();
    let mut base_url = "http://127.0.0.1:3000".to_string();
    let mut fixture_dir = None;
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = PathBuf::from(value);
            }
            "--app-name" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--app-name requires a value".to_string())?;
                app_name = value.to_string();
            }
            "--base-url" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--base-url requires a URL".to_string())?;
                base_url = value.to_string();
            }
            "--fixture-dir" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--fixture-dir requires a directory".to_string())?;
                fixture_dir = Some(PathBuf::from(value));
            }
            "--force" => {
                force = true;
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    if app_name.trim().is_empty() {
        return Err("--app-name cannot be empty".to_string());
    }
    if fixture_dir.is_none() && base_url.trim().is_empty() {
        return Err("--base-url cannot be empty without --fixture-dir".to_string());
    }

    Ok(InitOptions {
        manifest_path,
        app_name,
        base_url,
        fixture_dir,
        force,
    })
}

pub(super) fn parse_verify_options(args: &[String]) -> std::result::Result<VerifyOptions, String> {
    let mut manifest_path = PathBuf::from(".allie/manifest.yml");
    let mut out_dir = PathBuf::from(".allie/verify/latest");
    let mut project_root = None;
    let mut agent_runner = AgentRunnerKind::Local;
    let mut changed_surfaces = Vec::new();
    let mut stale_after_days = 7;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = PathBuf::from(value);
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = PathBuf::from(value);
            }
            "--project-root" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--project-root requires a directory".to_string())?;
                project_root = Some(PathBuf::from(value));
            }
            "--agent" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--agent requires local, opencode, or omp".to_string())?;
                agent_runner = AgentRunnerKind::parse(value)?;
            }
            "--changed-surface" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--changed-surface requires an id".to_string())?;
                changed_surfaces.push(value.to_string());
            }
            "--stale-after-days" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--stale-after-days requires a number".to_string())?;
                stale_after_days = value
                    .parse::<i64>()
                    .map_err(|_| "--stale-after-days must be an integer".to_string())?;
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(VerifyOptions {
        manifest_path,
        out_dir,
        project_root,
        agent_runner,
        changed_surfaces,
        stale_after_days,
    })
}

pub(super) fn run_init(options: InitOptions) -> Result<InitReceipt> {
    if options.manifest_path.exists() && !options.force {
        return Err(AllieError::InvalidManifest(format!(
            "manifest {} already exists; pass --force to replace it",
            options.manifest_path.display()
        )));
    }

    let manifest = scaffold_manifest(&options);
    manifest.validate()?;
    let yaml = serde_yaml::to_string(&manifest).map_err(|source| AllieError::Yaml {
        context: format!("serialize manifest {}", options.manifest_path.display()),
        source,
    })?;
    write_string(&options.manifest_path, &yaml)?;

    Ok(InitReceipt {
        next_command: next_verify_command(&options.manifest_path),
        manifest_path: options.manifest_path,
    })
}

pub(super) fn run_verify(options: VerifyOptions) -> Result<VerifyReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create verify output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;

    let pipeline = run_verify_pipeline(&options)?;
    let reporters = write_verify_reporters(&options, &pipeline)?;
    let exit_class = verify_exit_class(pipeline.run.exit_class, pipeline.release.exit_class);
    let status = verify_status(&pipeline.release.status, exit_class);

    Ok(VerifyReceipt {
        status,
        exit_class,
        summary_json_path: reporters.summary_json_path,
        summary_markdown_path: reporters.summary_markdown_path,
        report_json_path: reporters.report_json_path,
        report_html_path: reporters.report_html_path,
        junit_path: reporters.junit_path,
        sarif_path: reporters.sarif_path,
        release_summary_path: pipeline.release.summary_path,
        product_map_path: pipeline.map.map_path,
        evidence_path: pipeline.run.evidence_path,
    })
}

fn scaffold_manifest(options: &InitOptions) -> FlowManifest {
    let slug = slug_id(&options.app_name);
    let target = if let Some(fixture_dir) = &options.fixture_dir {
        ManifestTarget {
            kind: "local_fixture".to_string(),
            fixture_dir: Some(fixture_dir.clone()),
            base_url: None,
        }
    } else {
        ManifestTarget {
            kind: "web".to_string(),
            fixture_dir: None,
            base_url: Some(options.base_url.clone()),
        }
    };

    FlowManifest {
        id: format!("{slug}-allie-flow"),
        name: format!("{} Allie verification flow", options.app_name),
        app_name: options.app_name.clone(),
        environment: "local".to_string(),
        auth_profile: Some("none".to_string()),
        credentials: CredentialConfig {
            profile: Some("none".to_string()),
            provider: "none".to_string(),
            env: None,
            required: false,
        },
        target,
        policy: ManifestPolicy {
            profile: "wcag22-aa".to_string(),
            blocking_classes: vec!["deterministic".to_string(), "scripted_required".to_string()],
            worker_timeout_ms: DEFAULT_WORKER_TIMEOUT_MS,
        },
        artifacts: ArtifactPolicy {
            redaction_status: "not_redacted_local".to_string(),
            retention_class: "local_ephemeral".to_string(),
        },
        model: ModelPolicy::default(),
        known_nondeterminism: Vec::new(),
        browser: BrowserSettings {
            viewport: Viewport {
                width: 1280,
                height: 900,
            },
            color_scheme: "light".to_string(),
            reduced_motion: "reduce".to_string(),
            locale: "en-US".to_string(),
            zoom: 1.0,
        },
        flow: ManifestFlow {
            id: format!("{slug}-critical-path"),
            description: "Allie generated first-smoke verification flow.".to_string(),
            states: vec![ManifestState {
                id: "home".to_string(),
                path: "/".to_string(),
                description: "Home route first-smoke state.".to_string(),
                required: true,
                axe: true,
                screenshot: true,
                dom_snapshot: true,
                accessibility_tree: true,
                keyboard: true,
                video: false,
                trace: true,
                promotion_state: Some("operator_seed".to_string()),
            }],
        },
    }
}

fn run_verify_pipeline(options: &VerifyOptions) -> Result<VerifyPipelineReceipts> {
    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let project_root = options
        .project_root
        .clone()
        .unwrap_or_else(|| default_project_root_for_manifest(&options.manifest_path, &manifest));
    let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);

    let discovery = run_discovery(DiscoveryOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: options.out_dir.join("discovery"),
    })?;
    let promoted = run_promote_flow(PromoteFlowOptions {
        discovery_path: discovery.discovery_path.clone(),
        flow_plan_path: discovery.flow_plan_path.clone(),
        out_path: options.out_dir.join("flow/generated-flow.yml"),
    })?;
    let map = run_map(MapOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: options.out_dir.join("map"),
        project_root: project_root.clone(),
        agent_runner: options.agent_runner,
    })?;
    let run = run_v0(RunOptions {
        manifest_path: promoted.manifest_path.clone(),
        out_dir: options.out_dir.join("run"),
    })?;
    if manifest.model.enabled {
        match crate::agentic::run_agentic_review(&manifest, &run.evidence_path) {
            Ok(summary) => eprintln!(
                "Agentic review: {} criteria, {} model call(s), status {}",
                summary.criteria, summary.calls, summary.status
            ),
            Err(error) => eprintln!("Agentic review skipped (criteria stay needs_review): {error}"),
        }
    }
    let report = run_compliance_report(ReportOptions {
        map_path: map.map_path.clone(),
        packet_path: run.evidence_path.clone(),
        out_dir: options.out_dir.join("report"),
    })?;
    let changed_surfaces = verify_changed_surfaces(&discovery, &options.changed_surfaces)?;
    let release = run_release(ReleaseOptions {
        packet_path: run.evidence_path.clone(),
        out_dir: options.out_dir.join("release"),
        changed_surfaces: changed_surfaces.clone(),
        stale_after_days: options.stale_after_days,
    })?;

    Ok(VerifyPipelineReceipts {
        discovery,
        promoted,
        map,
        run,
        report,
        release,
        project_root,
        changed_surfaces,
    })
}

fn write_verify_reporters(
    options: &VerifyOptions,
    pipeline: &VerifyPipelineReceipts,
) -> Result<VerifyReporterReceipt> {
    let reporter_dir = options.out_dir.join("reporters");
    fs::create_dir_all(&reporter_dir).map_err(|source| AllieError::Io {
        context: format!("create reporter directory {}", reporter_dir.display()),
        source,
    })?;

    let reporters = VerifyReporterReceipt {
        summary_json_path: reporter_dir.join("allie-report.json"),
        summary_markdown_path: reporter_dir.join("allie-report.md"),
        report_json_path: reporter_dir.join("allie-compliance-report.json"),
        report_html_path: reporter_dir.join("allie-report.html"),
        junit_path: reporter_dir.join("junit.xml"),
        sarif_path: reporter_dir.join("allie.sarif"),
    };
    let exit_class = verify_exit_class(pipeline.run.exit_class, pipeline.release.exit_class);
    let status = verify_status(&pipeline.release.status, exit_class);
    let release_summary: serde_json::Value = read_json_file(&pipeline.release.summary_path)?;
    let compliance_report: serde_json::Value = read_json_file(&pipeline.report.report_json_path)?;
    let summary = verify_summary_value(
        options,
        pipeline,
        &reporters,
        &release_summary,
        &compliance_report,
        &status,
        exit_class,
    );

    write_json_pretty(&reporters.summary_json_path, &summary)?;
    write_json_pretty(&reporters.report_json_path, &compliance_report)?;
    write_string(
        &reporters.summary_markdown_path,
        &render_verify_markdown(&summary),
    )?;
    write_string(
        &reporters.report_html_path,
        &render_verify_html(&summary, &options.out_dir),
    )?;
    write_string(
        &reporters.junit_path,
        &render_verify_junit(&summary, exit_class),
    )?;
    write_json_pretty(&reporters.sarif_path, &verify_sarif(&summary, exit_class))?;

    Ok(reporters)
}

fn verify_changed_surfaces(
    discovery: &DiscoveryReceipt,
    explicit: &[String],
) -> Result<Vec<String>> {
    if !explicit.is_empty() {
        return Ok(unique_strings(explicit.to_vec()));
    }
    let flow_plan: FlowPlanPacket = read_json_file(&discovery.flow_plan_path)?;
    let surfaces = unique_strings(
        flow_plan
            .candidates
            .into_iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
    );
    if surfaces.is_empty() {
        Ok(vec!["generated-flow".to_string()])
    } else {
        Ok(surfaces)
    }
}

fn verify_summary_value(
    options: &VerifyOptions,
    pipeline: &VerifyPipelineReceipts,
    reporters: &VerifyReporterReceipt,
    release_summary: &serde_json::Value,
    compliance_report: &serde_json::Value,
    status: &str,
    exit_class: ExitClass,
) -> serde_json::Value {
    serde_json::json!({
        "schema": VERIFY_SCHEMA,
        "status": status,
        "exit_code": exit_class.code(),
        "generated_at": now_utc().to_rfc3339(),
        "host_agnostic": true,
        "policy_source": options.manifest_path.to_string_lossy(),
        "project_root": pipeline.project_root.to_string_lossy(),
        "agent_runner": options.agent_runner.as_str(),
        "changed_surfaces": pipeline.changed_surfaces.clone(),
        "release_status": pipeline.release.status.clone(),
        "run_status": pipeline.run.exit_class.packet_status(),
        "why": {
            "summary": verify_reason(status, release_summary, compliance_report),
            "blocking": release_summary["blocking"].clone(),
            "review_needed_obligations": json_array_len(&release_summary["review_needed_obligations"]),
            "not_tested_obligations": json_array_len(&release_summary["not_tested_obligations"]),
            "compliance_summary": compliance_report["summary"].clone()
        },
        "reporters": {
            "json": path_relative_to(&options.out_dir, &reporters.summary_json_path),
            "html": path_relative_to(&options.out_dir, &reporters.report_html_path),
            "markdown": path_relative_to(&options.out_dir, &reporters.summary_markdown_path),
            "junit": path_relative_to(&options.out_dir, &reporters.junit_path),
            "sarif": path_relative_to(&options.out_dir, &reporters.sarif_path),
            "wcag_json": path_relative_to(&options.out_dir, &reporters.report_json_path)
        },
        "artifacts": {
            "summary_json": path_relative_to(&options.out_dir, &reporters.summary_json_path),
            "discovery_json": path_relative_to(&options.out_dir, &pipeline.discovery.discovery_path),
            "flow_plan_json": path_relative_to(&options.out_dir, &pipeline.discovery.flow_plan_path),
            "generated_flow": path_relative_to(&options.out_dir, &pipeline.promoted.manifest_path),
            "product_map_json": path_relative_to(&options.out_dir, &pipeline.map.map_path),
            "surface_map_html": path_relative_to(&options.out_dir, &pipeline.map.report_path),
            "agent_runner_receipt_json": path_relative_to(&options.out_dir, &pipeline.map.runner_receipt_path),
            "evidence_json": path_relative_to(&options.out_dir, &pipeline.run.evidence_path),
            "evidence_html": path_relative_to(&options.out_dir, &pipeline.run.report_path),
            "compliance_json": path_relative_to(&options.out_dir, &pipeline.report.report_json_path),
            "compliance_html": path_relative_to(&options.out_dir, &pipeline.report.report_html_path),
            "compliance_markdown": path_relative_to(&options.out_dir, &pipeline.report.summary_path),
            "release_summary_json": path_relative_to(&options.out_dir, &pipeline.release.summary_path),
            "release_check_json": path_relative_to(&options.out_dir, &pipeline.release.check_path),
            "release_html": path_relative_to(&options.out_dir, &pipeline.release.report_path)
        },
        "contract": {
            "local_command": format!("allie verify --manifest {} --out {}", options.manifest_path.display(), options.out_dir.display()),
            "ci_contract": "CI hosts call the same allie verify command and archive the full verify artifact root.",
            "legal_claim": "evidence visibility only; not a legal compliance guarantee"
        }
    })
}

fn render_verify_markdown(summary: &serde_json::Value) -> String {
    format!(
        "# Allie Verification Summary\n\nStatus: `{}`\n\nWhy: {}\n\nManifest: `{}`\nProject root: `{}`\n\nBlocking evidence: deterministic failures {}, scripted failures {}, infrastructure failures {}, missing required evidence {}.\nReview evidence: review-needed obligations {}, not-tested obligations {}.\nWCAG matrix: pass {}, fail {}, needs review {}, not tested {}.\n\nReporters:\n- JSON summary: `{}`\n- WCAG JSON: `{}`\n- HTML: `{}`\n- Markdown: `{}`\n- JUnit: `{}`\n- SARIF: `{}`\n\nEvidence packet: `{}`\nProduct map: `{}`\nWCAG HTML: `{}`\nRelease summary: `{}`\n\nThis is evidence visibility for accessibility engineering review, not a legal compliance guarantee.\n",
        summary["status"].as_str().unwrap_or("unknown"),
        summary["why"]["summary"].as_str().unwrap_or("unknown"),
        summary["policy_source"].as_str().unwrap_or("unknown"),
        summary["project_root"].as_str().unwrap_or("unknown"),
        summary["why"]["blocking"]["deterministic_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["blocking"]["scripted_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["blocking"]["infrastructure_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["blocking"]["missing_required_evidence"]
            .as_array()
            .map(|values| values.len())
            .unwrap_or_default(),
        summary["why"]["review_needed_obligations"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["not_tested_obligations"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["compliance_summary"]["pass"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["compliance_summary"]["fail"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["compliance_summary"]["needs_review"]
            .as_u64()
            .unwrap_or_default(),
        summary["why"]["compliance_summary"]["not_tested"]
            .as_u64()
            .unwrap_or_default(),
        summary["reporters"]["json"].as_str().unwrap_or(""),
        summary["reporters"]["wcag_json"].as_str().unwrap_or(""),
        summary["reporters"]["html"].as_str().unwrap_or(""),
        summary["reporters"]["markdown"].as_str().unwrap_or(""),
        summary["reporters"]["junit"].as_str().unwrap_or(""),
        summary["reporters"]["sarif"].as_str().unwrap_or(""),
        summary["artifacts"]["evidence_json"].as_str().unwrap_or(""),
        summary["artifacts"]["product_map_json"]
            .as_str()
            .unwrap_or(""),
        summary["artifacts"]["compliance_html"]
            .as_str()
            .unwrap_or(""),
        summary["artifacts"]["release_summary_json"]
            .as_str()
            .unwrap_or("")
    )
}

fn render_verify_html(summary: &serde_json::Value, out_dir: &Path) -> String {
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
    let review_needed = summary["why"]["review_needed_obligations"]
        .as_u64()
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
    let wcag_review = summary["why"]["compliance_summary"]["needs_review"]
        .as_u64()
        .unwrap_or_default();
    let wcag_not_tested = summary["why"]["compliance_summary"]["not_tested"]
        .as_u64()
        .unwrap_or_default();
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
    .stat {{ background: #fff; border: 1px solid #e4e8ef; border-radius: 13px; padding: 13px 15px; }}
    .stat .n {{ font-size: 23px; font-weight: 700; font-variant-numeric: tabular-nums; }}
    .stat .k {{ font-size: 10.5px; letter-spacing: .06em; text-transform: uppercase; color: #5a6473; margin-top: 3px; }}
    ul.links {{ list-style: none; padding: 0; margin: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px; }}
    ul.links a {{ display: block; background: #fff; border: 1px solid #e4e8ef; border-radius: 11px; padding: 12px 14px; font-weight: 600; }}
    ul.links code {{ display: block; color: #5a6473; font-weight: 400; margin-top: 4px; font-size: 11px; background: none; }}
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
        <div class="stat"><div class="n">{review_needed}</div><div class="k">Review needed</div></div>
      </div>
      <h2 class="sh">WCAG 2.2 matrix</h2>
      <div class="statgrid">
        <div class="stat"><div class="n">{wcag_pass}</div><div class="k">Pass</div></div>
        <div class="stat"><div class="n">{wcag_fail}</div><div class="k">Fail</div></div>
        <div class="stat"><div class="n">{wcag_review}</div><div class="k">Needs review</div></div>
        <div class="stat"><div class="n">{not_tested}</div><div class="k">Not-tested obligations</div></div>
        <div class="stat"><div class="n">{wcag_not_tested}</div><div class="k">WCAG not tested</div></div>
      </div>
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
        review_needed = review_needed,
        not_tested = not_tested,
        wcag_pass = wcag_pass,
        wcag_fail = wcag_fail,
        wcag_review = wcag_review,
        wcag_not_tested = wcag_not_tested,
        links = links
    )
}

fn verify_reason(
    status: &str,
    release_summary: &serde_json::Value,
    compliance_report: &serde_json::Value,
) -> String {
    let deterministic_failures = json_u64(&release_summary["blocking"]["deterministic_failures"]);
    let scripted_failures = json_u64(&release_summary["blocking"]["scripted_failures"]);
    let infrastructure_failures = json_u64(&release_summary["blocking"]["infrastructure_failures"]);
    let missing_required =
        json_array_len(&release_summary["blocking"]["missing_required_evidence"]);
    let review_needed = json_array_len(&release_summary["review_needed_obligations"]);
    let not_tested = json_array_len(&release_summary["not_tested_obligations"]);
    let wcag_fail = json_u64(&compliance_report["summary"]["fail"]);
    let wcag_review = json_u64(&compliance_report["summary"]["needs_review"]);
    match status {
        "blocked" => format!(
            "Release projection blocked on evidence: deterministic failures {deterministic_failures}, scripted failures {scripted_failures}, infrastructure failures {infrastructure_failures}, missing required evidence {missing_required}, WCAG failing criteria {wcag_fail}."
        ),
        "needs_review" => format!(
            "Evidence ran successfully, but review remains: review-needed obligations {review_needed}, not-tested obligations {not_tested}, WCAG criteria needing review {wcag_review}."
        ),
        "approved" => {
            "Evidence projection found no blocking or review-required signals in this packet."
                .to_string()
        }
        "failed" => format!(
            "Verification failed before a release decision could be trusted: infrastructure failures {infrastructure_failures}."
        ),
        other => format!("Verification produced status {other}; inspect release and WCAG reports."),
    }
}

fn json_array_len(value: &serde_json::Value) -> usize {
    value
        .as_array()
        .map(|values| values.len())
        .unwrap_or_default()
}

fn json_u64(value: &serde_json::Value) -> u64 {
    value.as_u64().unwrap_or_default()
}

fn render_verify_junit(summary: &serde_json::Value, exit_class: ExitClass) -> String {
    let status = summary["status"].as_str().unwrap_or("unknown");
    let message = format!("Allie verification status: {status}");
    let (failures, errors, body) = match exit_class {
        ExitClass::Success => (0, 0, String::new()),
        ExitClass::BlockingFinding => (
            1,
            0,
            format!(
                "<failure message=\"{}\">{}</failure>",
                escape_html(&message),
                escape_html(&message)
            ),
        ),
        ExitClass::InfrastructureFailure | ExitClass::Usage => (
            0,
            1,
            format!(
                "<error message=\"{}\">{}</error>",
                escape_html(&message),
                escape_html(&message)
            ),
        ),
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"allie.verify\" tests=\"1\" failures=\"{failures}\" errors=\"{errors}\">\n  <testcase classname=\"allie\" name=\"verify consumer contract\">{body}</testcase>\n</testsuite>\n"
    )
}

fn verify_sarif(summary: &serde_json::Value, exit_class: ExitClass) -> serde_json::Value {
    let status = summary["status"].as_str().unwrap_or("unknown");
    let level = match exit_class {
        ExitClass::Success if status == "needs_review" => "warning",
        ExitClass::Success => "note",
        ExitClass::BlockingFinding | ExitClass::InfrastructureFailure | ExitClass::Usage => "error",
    };
    let results = if status == "approved" {
        Vec::new()
    } else {
        vec![serde_json::json!({
            "ruleId": "allie.verify.status",
            "level": level,
            "message": {
                "text": format!(
                    "Allie verification completed with status {status}; inspect the evidence packet, product map, WCAG report, and release projection."
                )
            },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": {
                        "uri": summary["artifacts"]["release_summary_json"].as_str().unwrap_or("")
                    }
                }
            }]
        })]
    };
    serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "Allie",
                    "semanticVersion": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/adminifi-ai/allie",
                    "rules": [{
                        "id": "allie.verify.status",
                        "shortDescription": {
                            "text": "Allie verification status"
                        },
                        "help": {
                            "text": "Allie reports evidence, status, confidence, and residual review needs. It does not claim legal compliance."
                        }
                    }]
                }
            },
            "results": results
        }]
    })
}

fn verify_exit_class(run_exit: ExitClass, release_exit: ExitClass) -> ExitClass {
    if run_exit == ExitClass::InfrastructureFailure {
        ExitClass::InfrastructureFailure
    } else {
        release_exit
    }
}

fn verify_status(release_status: &str, exit_class: ExitClass) -> String {
    if exit_class == ExitClass::InfrastructureFailure {
        "failed".to_string()
    } else {
        release_status.to_string()
    }
}

fn next_verify_command(manifest_path: &Path) -> String {
    format!(
        "allie verify --manifest {} --out .allie/verify/latest",
        manifest_path.display()
    )
}

fn default_app_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Allie app".to_string())
}

fn slug_id(value: &str) -> String {
    let mut slug = value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    if slug.is_empty() {
        "app".to_string()
    } else {
        slug
    }
}

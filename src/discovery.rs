use crate::model::*;
use crate::standards::standards_profile_summary;
use crate::{
    AgentRunnerKind, AllieError, FlowManifest, ManifestState, PRODUCT_MAP_SCHEMA, Result,
    new_run_id, now_utc, path_relative_to, read_json_file, write_json_pretty, write_string,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

mod live;
mod render;

use live::discover_live_base_url_surfaces;
use render::{render_discovery_report, render_product_surface_map};

const DEFAULT_AGENT_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug)]
pub(crate) struct DiscoveryOptions {
    pub(crate) manifest_path: PathBuf,
    pub(crate) out_dir: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PromoteFlowOptions {
    pub(crate) discovery_path: PathBuf,
    pub(crate) flow_plan_path: PathBuf,
    pub(crate) out_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct MapOptions {
    pub(crate) manifest_path: PathBuf,
    pub(crate) out_dir: PathBuf,
    pub(crate) project_root: PathBuf,
    pub(crate) agent_runner: AgentRunnerKind,
}

#[derive(Debug)]
pub(crate) struct DiscoveryReceipt {
    pub(crate) discovery_path: PathBuf,
    pub(crate) flow_plan_path: PathBuf,
    pub(crate) report_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PromoteFlowReceipt {
    pub(crate) manifest_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct MapReceipt {
    pub(crate) map_path: PathBuf,
    pub(crate) report_path: PathBuf,
    pub(crate) runner_receipt_path: PathBuf,
    pub(crate) flow_manifest_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FlowPlanPacket {
    pub(crate) schema: String,
    pub(crate) source_discovery: String,
    pub(crate) flow_id: String,
    pub(crate) candidates: Vec<FlowCandidate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FlowCandidate {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) description: String,
    pub(crate) promotion_state: String,
    pub(crate) required: bool,
    pub(crate) axe: bool,
    pub(crate) screenshot: bool,
    pub(crate) dom_snapshot: bool,
    pub(crate) accessibility_tree: bool,
    pub(crate) keyboard: bool,
    pub(crate) video: bool,
    pub(crate) trace: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryPacket {
    schema: String,
    run: DiscoveryRun,
    target: ManifestTarget,
    browser: BrowserSettings,
    promotion: DiscoveryPromotion,
    surfaces: Vec<DiscoveredSurface>,
    #[serde(default)]
    diagnostics: Vec<DiscoveryDiagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryRun {
    id: String,
    started_at: String,
    finished_at: String,
    source_manifest: String,
    app_name: String,
    policy_profile: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryPromotion {
    default_state: String,
    enforcement_rule: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveredSurface {
    id: String,
    route: String,
    title: String,
    source: String,
    confidence: String,
    user_stories: Vec<String>,
    provenance: Vec<String>,
}

#[derive(Clone, Debug)]
struct SurfaceDiscovery {
    surfaces: Vec<DiscoveredSurface>,
    diagnostics: Vec<DiscoveryDiagnostic>,
}

struct ProductSurfaceDiscovery {
    surfaces: Vec<ProductSurface>,
    diagnostics: Vec<DiscoveryDiagnostic>,
}

pub(crate) fn run_discovery(options: DiscoveryOptions) -> Result<DiscoveryReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create discovery output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;

    let started_at = now_utc();
    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let discovery_result = discover_surfaces(&manifest, &options.manifest_path)?;
    let surfaces = discovery_result.surfaces;
    let discovery = DiscoveryPacket {
        schema: "allie.discovery.v0".to_string(),
        run: DiscoveryRun {
            id: new_run_id(),
            started_at: started_at.to_rfc3339(),
            finished_at: now_utc().to_rfc3339(),
            source_manifest: options.manifest_path.to_string_lossy().to_string(),
            app_name: manifest.app_name.clone(),
            policy_profile: manifest.policy.profile.clone(),
        },
        target: manifest.target.clone(),
        browser: manifest.browser.clone(),
        promotion: DiscoveryPromotion {
            default_state: "generated_candidate".to_string(),
            enforcement_rule:
                "generated flows must replay through allie run before release enforcement"
                    .to_string(),
        },
        surfaces: surfaces.clone(),
        diagnostics: discovery_result.diagnostics,
    };
    let flow_plan = FlowPlanPacket {
        schema: "allie.flow-plan.v0".to_string(),
        source_discovery: "discovery.json".to_string(),
        flow_id: "autonomous-discovered-flow".to_string(),
        candidates: surfaces
            .iter()
            .map(|surface| FlowCandidate {
                id: surface.id.clone(),
                path: surface.route.clone(),
                description: format!("Generated coverage candidate for {}", surface.title),
                promotion_state: "generated_candidate".to_string(),
                required: true,
                axe: true,
                screenshot: true,
                dom_snapshot: true,
                accessibility_tree: true,
                keyboard: true,
                video: true,
                trace: true,
            })
            .collect(),
    };

    let discovery_path = options.out_dir.join("discovery.json");
    let flow_plan_path = options.out_dir.join("flow-plan.json");
    let report_path = options.out_dir.join("discovery-report.html");
    write_json_pretty(&discovery_path, &discovery)?;
    write_json_pretty(&flow_plan_path, &flow_plan)?;
    write_string(
        &report_path,
        &render_discovery_report(&discovery, &flow_plan),
    )?;

    Ok(DiscoveryReceipt {
        discovery_path,
        flow_plan_path,
        report_path,
    })
}

pub(crate) fn run_promote_flow(options: PromoteFlowOptions) -> Result<PromoteFlowReceipt> {
    let discovery: DiscoveryPacket = read_json_file(&options.discovery_path)?;
    let flow_plan: FlowPlanPacket = read_json_file(&options.flow_plan_path)?;
    if discovery.schema != "allie.discovery.v0" || flow_plan.schema != "allie.flow-plan.v0" {
        return Err(AllieError::InvalidManifest(
            "discovery and flow-plan schemas must be v0".to_string(),
        ));
    }

    let source_manifest_path = PathBuf::from(&discovery.run.source_manifest);
    let mut manifest = FlowManifest::load(&source_manifest_path)?;
    if let Some(fixture_dir) = manifest.target.fixture_dir.clone()
        && !fixture_dir.is_absolute()
    {
        let source_dir = source_manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let normalized = source_dir.join(fixture_dir);
        manifest.target.fixture_dir = Some(fs::canonicalize(&normalized).unwrap_or(normalized));
    }
    manifest.id = format!("{}-generated", manifest.id);
    manifest.name = format!("{} generated accessibility flow", manifest.name);
    manifest.flow.id = flow_plan.flow_id.clone();
    manifest.flow.description =
        "Generated from an Allie discovery packet and promoted after operator review.".to_string();
    manifest.flow.states = flow_plan
        .candidates
        .iter()
        .map(|candidate| ManifestState {
            id: candidate.id.clone(),
            path: candidate.path.clone(),
            description: candidate.description.clone(),
            required: candidate.required,
            steps: Vec::new(),
            axe: candidate.axe,
            screenshot: candidate.screenshot,
            dom_snapshot: candidate.dom_snapshot,
            accessibility_tree: candidate.accessibility_tree,
            keyboard: candidate.keyboard,
            video: candidate.video,
            trace: candidate.trace,
            promotion_state: Some("verified_flow".to_string()),
        })
        .collect();

    let yaml = serde_yaml::to_string(&manifest).map_err(|source| AllieError::Yaml {
        context: format!(
            "serialize generated manifest {}",
            options.out_path.display()
        ),
        source,
    })?;
    write_string(&options.out_path, &yaml)?;

    Ok(PromoteFlowReceipt {
        manifest_path: options.out_path,
    })
}

pub(crate) fn run_map(options: MapOptions) -> Result<MapReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!("create map output directory {}", options.out_dir.display()),
        source,
    })?;

    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let project_root =
        fs::canonicalize(&options.project_root).unwrap_or_else(|_| options.project_root.clone());
    let product_discovery = product_surfaces(&manifest, &options.manifest_path, &project_root)?;
    let surfaces = product_discovery.surfaces;
    let workflows = vec![ProductWorkflow {
        id: manifest.flow.id.clone(),
        title: manifest.flow.description.clone(),
        surface_refs: surfaces.iter().map(|surface| surface.id.clone()).collect(),
        user_story: format!(
            "As an accessibility compliance engineer, I can assess {} across the discovered product surface.",
            manifest.app_name
        ),
        generated_flow_manifest: "generated-flow.yml".to_string(),
        states: manifest
            .flow
            .states
            .iter()
            .map(|state| state.id.clone())
            .collect(),
    }];
    let agent = run_agent_mapper(
        options.agent_runner,
        &options.out_dir,
        &project_root,
        &manifest,
        &options.manifest_path,
        &surfaces,
    )?;
    let map = ProductMapPacket {
        schema: PRODUCT_MAP_SCHEMA.to_string(),
        generated_at: now_utc().to_rfc3339(),
        source_manifest: options.manifest_path.to_string_lossy().to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        app_name: manifest.app_name.clone(),
        environment: manifest.environment.clone(),
        policy_profile: manifest.policy.profile.clone(),
        target: manifest.target.clone(),
        agent,
        standards: standards_profile_summary(&manifest.policy.profile),
        surfaces,
        workflows,
        discovery_diagnostics: product_discovery.diagnostics.clone(),
        open_questions: product_map_open_questions(&manifest, &product_discovery.diagnostics),
    };
    let generated_manifest = generated_flow_manifest(&manifest, &map.surfaces);

    let map_path = options.out_dir.join("product-map.json");
    let report_path = options.out_dir.join("surface-map.html");
    let runner_receipt_path = options.out_dir.join("agent-runner-receipt.json");
    let flow_manifest_path = options.out_dir.join("generated-flow.yml");
    write_json_pretty(&map_path, &map)?;
    write_string(&report_path, &render_product_surface_map(&map))?;
    write_json_pretty(&runner_receipt_path, &map.agent)?;
    let flow_yaml =
        serde_yaml::to_string(&generated_manifest).map_err(|source| AllieError::Yaml {
            context: format!(
                "serialize generated flow manifest {}",
                flow_manifest_path.display()
            ),
            source,
        })?;
    write_string(&flow_manifest_path, &flow_yaml)?;

    Ok(MapReceipt {
        map_path,
        report_path,
        runner_receipt_path,
        flow_manifest_path,
    })
}

pub(crate) fn default_project_root_for_manifest(
    manifest_path: &Path,
    manifest: &FlowManifest,
) -> PathBuf {
    if manifest.target.kind == "local_fixture"
        && let Some(fixture_dir) = &manifest.target.fixture_dir
    {
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        return if fixture_dir.is_absolute() {
            fixture_dir.clone()
        } else {
            manifest_dir.join(fixture_dir)
        };
    }
    PathBuf::from(".")
}

fn run_agent_mapper(
    runner: AgentRunnerKind,
    out_dir: &Path,
    project_root: &Path,
    manifest: &FlowManifest,
    manifest_path: &Path,
    surfaces: &[ProductSurface],
) -> Result<AgentRunnerReceiptPacket> {
    let base_receipt = AgentRunnerReceiptPacket {
        schema: "allie.agent-runner-receipt.v0".to_string(),
        runner: runner.as_str().to_string(),
        mode: "deterministic-local-map".to_string(),
        status: "local_scan_completed".to_string(),
        capabilities: agent_runner_capabilities(runner),
        command: Vec::new(),
        prompt_path: None,
        transcript_path: None,
        warnings: vec![
            "Core map generation is deterministic; agent findings are advisory until promoted by evidence.".to_string(),
        ],
        sources: agent_runner_sources(runner),
    };
    if runner == AgentRunnerKind::Local {
        return Ok(base_receipt);
    }

    let context_dir = out_dir.join("agent-context");
    fs::create_dir_all(&context_dir).map_err(|source| AllieError::Io {
        context: format!("create agent context directory {}", context_dir.display()),
        source,
    })?;
    let seed_path = context_dir.join("map-seed.json");
    let prompt_path = context_dir.join("agent-map-prompt.md");
    let transcript_path = out_dir.join(format!("{}-map-transcript.txt", runner.as_str()));
    let seed = serde_json::json!({
        "schema": "allie.agent-map-seed.v0",
        "app_name": manifest.app_name.clone(),
        "environment": manifest.environment.clone(),
        "policy_profile": manifest.policy.profile.clone(),
        "source_manifest": manifest_path.to_string_lossy(),
        "project_root": project_root.to_string_lossy(),
        "surfaces": surfaces,
        "states": manifest.flow.states.clone(),
        "standards": standards_profile_summary(&manifest.policy.profile)
    });
    write_json_pretty(&seed_path, &seed)?;
    write_string(
        &prompt_path,
        &agent_map_prompt(manifest, &seed_path, surfaces.len()),
    )?;

    let (program, args) = agent_command(runner, &context_dir, &prompt_path, &seed_path);
    let mut command = Command::new(&program);
    command
        .args(&args)
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let command_line = std::iter::once(program.clone())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();

    let mut receipt = base_receipt;
    receipt.mode = "isolated-agent-advisory-pass".to_string();
    receipt.command = command_line;
    receipt.prompt_path = Some(path_relative_to(out_dir, &prompt_path));
    receipt.transcript_path = Some(path_relative_to(out_dir, &transcript_path));

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            receipt.status = "agent_unavailable_local_fallback".to_string();
            receipt
                .warnings
                .push(format!("spawn {} failed: {source}", runner.as_str()));
            write_string(
                &transcript_path,
                &format!("agent spawn failed for {}: {source}\n", runner.as_str()),
            )?;
            return Ok(receipt);
        }
    };

    let status = match child
        .wait_timeout(Duration::from_millis(DEFAULT_AGENT_TIMEOUT_MS))
        .map_err(|source| AllieError::Io {
            context: format!("wait for {} map agent", runner.as_str()),
            source,
        })? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|source| AllieError::Io {
                context: format!("collect timed out {} map agent output", runner.as_str()),
                source,
            })?;
            receipt.status = "agent_timeout_local_fallback".to_string();
            receipt.warnings.push(format!(
                "{} exceeded {} ms; deterministic local map was kept",
                runner.as_str(),
                DEFAULT_AGENT_TIMEOUT_MS
            ));
            write_agent_transcript(&transcript_path, &receipt.command, None, &output)?;
            return Ok(receipt);
        }
    };
    let output = child.wait_with_output().map_err(|source| AllieError::Io {
        context: format!("collect {} map agent output", runner.as_str()),
        source,
    })?;
    if status.success() {
        receipt.status = "agent_advisory_completed".to_string();
    } else {
        receipt.status = "agent_failed_local_fallback".to_string();
        receipt.warnings.push(format!(
            "{} exited with {}; deterministic local map was kept",
            runner.as_str(),
            status
        ));
    }
    write_agent_transcript(&transcript_path, &receipt.command, Some(status), &output)?;
    Ok(receipt)
}

fn agent_command(
    runner: AgentRunnerKind,
    context_dir: &Path,
    prompt_path: &Path,
    seed_path: &Path,
) -> (String, Vec<String>) {
    match runner {
        AgentRunnerKind::Local => ("true".to_string(), Vec::new()),
        AgentRunnerKind::OpenCode => (
            "opencode".to_string(),
            vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--dir".to_string(),
                context_dir.to_string_lossy().to_string(),
                format!(
                    "Review `{}` and `{}` in this directory. Return concise JSON with missing surfaces, workflows, and WCAG review hypotheses. Do not edit files.",
                    prompt_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("agent-map-prompt.md"),
                    seed_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("map-seed.json")
                ),
            ],
        ),
        AgentRunnerKind::Omp => (
            "omp".to_string(),
            vec![
                "-p".to_string(),
                "--mode".to_string(),
                "json".to_string(),
                "--max-time".to_string(),
                (DEFAULT_AGENT_TIMEOUT_MS / 1000).to_string(),
                "--no-session".to_string(),
                "--cwd".to_string(),
                context_dir.to_string_lossy().to_string(),
                format!("@{}", prompt_path.to_string_lossy()),
                format!("@{}", seed_path.to_string_lossy()),
            ],
        ),
    }
}

fn write_agent_transcript(
    path: &Path,
    command: &[String],
    status: Option<std::process::ExitStatus>,
    output: &std::process::Output,
) -> Result<()> {
    let contents = format!(
        "command: {}\nstatus: {}\n\nstdout:\n{}\n\nstderr:\n{}\n",
        command.join(" "),
        status
            .map(|value| value.to_string())
            .unwrap_or_else(|| "timeout".to_string()),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    write_string(path, &contents)
}

fn agent_map_prompt(manifest: &FlowManifest, seed_path: &Path, surface_count: usize) -> String {
    format!(
        "# Allie Agent Map Review\n\nYou are Allie, an accessibility evidence agent. Inspect the attached map seed at `{}` for `{}`. Return JSON only with keys `missing_surfaces`, `missing_workflows`, `wcag_review_hypotheses`, and `reporting_notes`.\n\nConstraints:\n- Do not edit files.\n- Do not claim legal compliance.\n- Treat deterministic axe/playwright evidence as stronger than model-only judgment.\n- Recommend agentic or human review where the evidence requires visual, assistive-technology, content, or workflow judgment.\n\nSeed surface count: {}.\n",
        seed_path.display(),
        manifest.app_name,
        surface_count
    )
}

fn agent_runner_capabilities(runner: AgentRunnerKind) -> Vec<String> {
    match runner {
        AgentRunnerKind::Local => vec![
            "manifest-state-normalization".to_string(),
            "static-html-surface-discovery".to_string(),
            "wcag-profile-linkage".to_string(),
        ],
        AgentRunnerKind::OpenCode => vec![
            "headless-opencode-run".to_string(),
            "custom-agent-compatible".to_string(),
            "session-transcript-capture".to_string(),
            "isolated-advisory-context".to_string(),
        ],
        AgentRunnerKind::Omp => vec![
            "interactive-or-print-agent".to_string(),
            "vision-capable-model-routing".to_string(),
            "json-output-mode".to_string(),
            "isolated-advisory-context".to_string(),
        ],
    }
}

fn agent_runner_sources(runner: AgentRunnerKind) -> Vec<String> {
    let mut sources = vec![
        "https://www.w3.org/WAI/WCAG22/wcag.json".to_string(),
        "https://www.w3.org/WAI/test-evaluate/".to_string(),
    ];
    match runner {
        AgentRunnerKind::OpenCode => {
            sources.push("https://opencode.ai/docs/cli/".to_string());
            sources.push("https://opencode.ai/docs/server/".to_string());
        }
        AgentRunnerKind::Omp => {
            sources.push("local:omp --help".to_string());
        }
        AgentRunnerKind::Local => {}
    }
    sources
}

fn product_surfaces(
    manifest: &FlowManifest,
    manifest_path: &Path,
    project_root: &Path,
) -> Result<ProductSurfaceDiscovery> {
    let mut surfaces: BTreeMap<String, ProductSurface> = BTreeMap::new();
    let discovery_result = discover_surfaces(manifest, manifest_path)?;
    for discovered in discovery_result.surfaces {
        let route = discovered.route.clone();
        merge_product_surface(
            &mut surfaces,
            route.clone(),
            ProductSurface {
                id: discovered.id,
                title: discovered.title,
                routes: vec![route.clone()],
                files: Vec::new(),
                services: vec![service_label_for_target(&manifest.target)],
                user_stories: discovered.user_stories,
                workflow_refs: vec![manifest.flow.id.clone()],
                evidence_refs: manifest
                    .flow
                    .states
                    .iter()
                    .filter(|state| state.path == route)
                    .map(|state| state.id.clone())
                    .collect(),
                confidence: discovered.confidence,
                review_status: "generated_needs_operator_review".to_string(),
                provenance: discovered.provenance,
            },
        );
    }

    for html_path in project_html_files(project_root)? {
        let route = route_for_project_file(project_root, &html_path);
        let title = html_title(&html_path).unwrap_or_else(|| route_to_id(&route));
        let relative = path_relative_to(project_root, &html_path);
        merge_product_surface(
            &mut surfaces,
            route.clone(),
            ProductSurface {
                id: route_to_id(&route),
                title,
                routes: vec![route.clone()],
                files: vec![relative.clone()],
                services: vec!["static-html".to_string()],
                user_stories: vec![format!("As an application user, I can reach {}", relative)],
                workflow_refs: vec![manifest.flow.id.clone()],
                evidence_refs: manifest
                    .flow
                    .states
                    .iter()
                    .filter(|state| state.path == route)
                    .map(|state| state.id.clone())
                    .collect(),
                confidence: "repo_static_scan".to_string(),
                review_status: "generated_needs_operator_review".to_string(),
                provenance: vec![html_path.to_string_lossy().to_string()],
            },
        );
    }

    Ok(ProductSurfaceDiscovery {
        surfaces: surfaces.into_values().collect(),
        diagnostics: discovery_result.diagnostics,
    })
}

fn merge_product_surface(
    surfaces: &mut BTreeMap<String, ProductSurface>,
    route: String,
    incoming: ProductSurface,
) {
    if let Some(existing) = surfaces.get_mut(&route) {
        if existing.title == existing.id && incoming.title != incoming.id {
            existing.title = incoming.title;
        }
        extend_unique(&mut existing.routes, incoming.routes);
        extend_unique(&mut existing.files, incoming.files);
        extend_unique(&mut existing.services, incoming.services);
        extend_unique(&mut existing.user_stories, incoming.user_stories);
        extend_unique(&mut existing.workflow_refs, incoming.workflow_refs);
        extend_unique(&mut existing.evidence_refs, incoming.evidence_refs);
        extend_unique(&mut existing.provenance, incoming.provenance);
        if existing.confidence != "operator_supplied" {
            existing.confidence = incoming.confidence;
        }
    } else {
        surfaces.insert(route, incoming);
    }
}

fn extend_unique(target: &mut Vec<String>, incoming: Vec<String>) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for item in incoming {
        if seen.insert(item.clone()) {
            target.push(item);
        }
    }
}

fn service_label_for_target(target: &ManifestTarget) -> String {
    match target.kind.as_str() {
        "local_fixture" => "local-fixture".to_string(),
        "static" => "static-site".to_string(),
        "web" => "web-app".to_string(),
        other => other.to_string(),
    }
}

fn project_html_files(root: &Path) -> Result<Vec<PathBuf>> {
    html_files_with_filter(root, true)
}

fn html_files_with_filter(root: &Path, skip_generated: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|source| AllieError::Io {
            context: format!("read html directory {}", dir.display()),
            source,
        })? {
            let entry = entry.map_err(|source| AllieError::Io {
                context: format!("read html entry {}", dir.display()),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                if skip_generated && should_skip_project_dir(&path) {
                    continue;
                }
                stack.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("html") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn should_skip_project_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git"
            | ".allie"
            | ".next"
            | "build"
            | "coverage"
            | "dist"
            | "docs"
            | "explore"
            | "node_modules"
            | "target"
    )
}

fn route_for_project_file(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative == Path::new("index.html") {
        return "/".to_string();
    }
    if relative.file_name() == Some(std::ffi::OsStr::new("index.html")) {
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let parent = parent.to_string_lossy().replace('\\', "/");
        return if parent.is_empty() {
            "/".to_string()
        } else {
            format!("/{parent}/")
        };
    }
    format!("/{}", relative.to_string_lossy().replace('\\', "/"))
}

fn product_map_open_questions(
    manifest: &FlowManifest,
    diagnostics: &[DiscoveryDiagnostic],
) -> Vec<String> {
    let mut questions = Vec::new();
    if manifest.model.enabled {
        questions.push(
            "Confirm provider, ZDR, and artifact redaction policy before sending screenshots or DOM captures to model review."
                .to_string(),
        );
    } else {
        questions.push(
            "Model review is disabled in the manifest; human or agentic review findings remain uncollected until review runs."
                .to_string(),
        );
    }
    questions.push(
        "Verify generated user stories and workflow names with the application owner before using them as release blockers."
            .to_string(),
    );
    for diagnostic in diagnostics {
        questions.push(format!(
            "Discovery diagnostic from {}: {}",
            diagnostic.source, diagnostic.message
        ));
    }
    questions
}

fn generated_flow_manifest(manifest: &FlowManifest, surfaces: &[ProductSurface]) -> FlowManifest {
    let mut generated = manifest.clone();
    generated.id = format!("{}-allie-generated", manifest.id);
    generated.name = format!("{} Allie generated product-surface flow", manifest.app_name);
    generated.flow.id = "allie-generated-product-surface-flow".to_string();
    generated.flow.description =
        "Generated from the Allie product map. Replay before enforcement.".to_string();
    generated.flow.states = surfaces
        .iter()
        .flat_map(|surface| {
            surface.routes.iter().map(move |route| ManifestState {
                id: surface.id.clone(),
                path: route.clone(),
                description: surface.title.clone(),
                required: true,
                steps: Vec::new(),
                axe: true,
                screenshot: true,
                dom_snapshot: true,
                accessibility_tree: true,
                keyboard: true,
                video: false,
                trace: true,
                promotion_state: Some("generated_candidate".to_string()),
            })
        })
        .collect();
    generated
}

fn discover_surfaces(manifest: &FlowManifest, manifest_path: &Path) -> Result<SurfaceDiscovery> {
    let mut surfaces = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for state in &manifest.flow.states {
        surfaces.insert(
            state.path.clone(),
            DiscoveredSurface {
                id: state.id.clone(),
                route: state.path.clone(),
                title: state.description.clone(),
                source: "manifest".to_string(),
                confidence: "operator_supplied".to_string(),
                user_stories: vec![format!("As a user, I can complete {}", state.description)],
                provenance: vec![manifest_path.to_string_lossy().to_string()],
            },
        );
    }

    if manifest.target.kind == "local_fixture"
        && let Some(fixture_dir) = &manifest.target.fixture_dir
    {
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let fixture_root = if fixture_dir.is_absolute() {
            fixture_dir.clone()
        } else {
            manifest_dir.join(fixture_dir)
        };
        for html_path in html_files(&fixture_root)? {
            let route = route_for_fixture_file(&fixture_root, &html_path);
            surfaces
                .entry(route.clone())
                .or_insert_with(|| DiscoveredSurface {
                    id: route_to_id(&route),
                    title: html_title(&html_path).unwrap_or_else(|| route_to_id(&route)),
                    route: route.clone(),
                    source: "fixture-crawl".to_string(),
                    confidence: "browser_discovered".to_string(),
                    user_stories: vec![format!("As an application user, I can reach {}", route)],
                    provenance: vec![html_path.to_string_lossy().to_string()],
                });
        }
    } else if let Some(base_url) = &manifest.target.base_url {
        let live = discover_live_base_url_surfaces(base_url)?;
        diagnostics.extend(live.diagnostics);
        for discovered in live.surfaces {
            surfaces
                .entry(discovered.route.clone())
                .or_insert(discovered);
        }
    }

    Ok(SurfaceDiscovery {
        surfaces: surfaces.into_values().collect(),
        diagnostics,
    })
}

fn html_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|source| AllieError::Io {
            context: format!("read fixture directory {}", dir.display()),
            source,
        })? {
            let entry = entry.map_err(|source| AllieError::Io {
                context: format!("read fixture entry {}", dir.display()),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("html") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn route_for_fixture_file(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative == Path::new("index.html") {
        "/".to_string()
    } else {
        format!("/{}", relative.to_string_lossy().replace('\\', "/"))
    }
}

fn route_to_id(route: &str) -> String {
    let mut id = route
        .trim_matches('/')
        .trim_end_matches(".html")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if id.is_empty() {
        id = "home".to_string();
    }
    id
}

fn html_title(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    html_title_from_text(&text)
}

fn html_title_from_text(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(text[start..end].trim().to_string())
}

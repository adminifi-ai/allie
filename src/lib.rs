use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display};
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

mod model;

mod agentic;
mod auth;
mod axe_pass;
mod cli;
mod compliance;
mod consumer;
mod discovery;
mod model_credentials;
mod model_policy;
pub(crate) use model_policy::{ModelPolicy, ModelRedactionMode};
mod out_dir;
mod pipeline;
mod publication;
mod release;
mod report;
mod review;
mod runtime;
mod standards;
#[cfg(test)]
mod test_support;
mod workbench;
mod worker;
mod worker_runtime;

use crate::auth::AuthFlow;
pub(crate) use crate::discovery::{
    DiscoveryOptions, DiscoveryReceipt, FlowPlanPacket, MapOptions, MapReceipt, PromoteFlowOptions,
    PromoteFlowReceipt, default_project_root_for_manifest, run_discovery, run_map,
    run_promote_flow,
};
use crate::model::*;
use crate::runtime::{GitProvenance, RunContext, current_time_millis};
use crate::standards::{
    criterion_feature_verdict, criterion_title, deterministic_pass_obligation,
    human_review_profile_obligations, obligation_from_tags, profile_obligation_list,
    scripted_profile_obligations, wcag22_success_criteria,
};
#[cfg(test)]
use crate::worker::{AxeViolation, WorkerStateResult};
use crate::worker::{RunFailure, WorkerResponse, WorkerRunStatus, aggregate_features};

const PRODUCT_LINE: &str = "Allie: accessibility evidence for every release.";
const NEXT_STEP: &str = "Next implementation target: allie run --manifest <flow.yml>";
const EVIDENCE_SCHEMA: &str = "allie.evidence.v0";
pub(crate) const PRODUCT_MAP_SCHEMA: &str = "allie.product-map.v0";
const COMPLIANCE_REPORT_SCHEMA: &str = "allie.compliance-report.v0";
const JOB_SCHEMA: &str = "allie.job.v0";
const DEFAULT_WORKER_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_WORKBENCH_MAX_RUNTIME_MS: u64 = 24 * 60 * 60 * 1000;
const DEFAULT_WORKBENCH_IDLE_TIMEOUT_MS: u64 = 10 * 60 * 1000;

#[derive(Debug)]
pub enum AllieError {
    Io {
        context: String,
        source: io::Error,
    },
    Json {
        context: String,
        source: serde_json::Error,
    },
    Yaml {
        context: String,
        source: serde_yaml::Error,
    },
    InvalidManifest(String),
    Provenance(String),
    Runtime(String),
    Worker(String),
}

impl Display for AllieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Json { context, source } => write!(f, "{context}: {source}"),
            Self::Yaml { context, source } => write!(f, "{context}: {source}"),
            Self::InvalidManifest(message) => write!(f, "invalid manifest: {message}"),
            Self::Provenance(message) => write!(f, "provenance error: {message}"),
            Self::Runtime(message) => write!(f, "runtime error: {message}"),
            Self::Worker(message) => write!(f, "worker failed: {message}"),
        }
    }
}

impl Error for AllieError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Yaml { source, .. } => Some(source),
            Self::InvalidManifest(_) | Self::Provenance(_) | Self::Runtime(_) | Self::Worker(_) => {
                None
            }
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, AllieError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitClass {
    Success,
    BlockingFinding,
    InfrastructureFailure,
    Usage,
}

impl ExitClass {
    pub fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::BlockingFinding => 1,
            Self::InfrastructureFailure => 2,
            Self::Usage => 64,
        }
    }

    fn packet_status(self) -> &'static str {
        match self {
            Self::Success => "pass",
            Self::BlockingFinding => "fail",
            Self::InfrastructureFailure | Self::Usage => "error",
        }
    }
}

#[derive(Debug)]
pub struct RunReceipt {
    pub run_id: String,
    pub exit_class: ExitClass,
    pub evidence_path: PathBuf,
    pub report_path: PathBuf,
}

#[derive(Debug)]
struct RunOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
    project_root: Option<PathBuf>,
}

#[derive(Debug)]
struct DoctorOptions {
    manifest_path: Option<PathBuf>,
    out_dir: PathBuf,
}

#[derive(Debug)]
struct ReleaseOptions {
    packet_path: PathBuf,
    out_dir: PathBuf,
    changed_surfaces: Vec<String>,
    stale_after_days: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentRunnerKind {
    Local,
    OpenCode,
    Omp,
}

impl AgentRunnerKind {
    pub(crate) fn parse(value: &str) -> std::result::Result<Self, String> {
        match value {
            "local" => Ok(Self::Local),
            "opencode" => Ok(Self::OpenCode),
            "omp" => Ok(Self::Omp),
            unexpected => Err(format!(
                "unsupported agent runner {unexpected}; expected local, opencode, or omp"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::OpenCode => "opencode",
            Self::Omp => "omp",
        }
    }
}

#[derive(Debug)]
struct ReportOptions {
    map_path: PathBuf,
    packet_path: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug)]
struct ReleaseReceipt {
    status: String,
    exit_class: ExitClass,
    summary_path: PathBuf,
    check_path: PathBuf,
    report_path: PathBuf,
}

#[derive(Debug)]
struct ComplianceReportReceipt {
    report_json_path: PathBuf,
    report_html_path: PathBuf,
    summary_path: PathBuf,
}

pub fn run_cli(args: impl IntoIterator<Item = String>) -> i32 {
    cli::run_cli(args)
}

pub fn run_cli_with_io(
    args: impl IntoIterator<Item = String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    cli::run_cli_with_io(args, stdout, stderr)
}

fn parse_run_options(args: &[String]) -> std::result::Result<RunOptions, String> {
    let mut manifest_path = None;
    let mut out_dir = None;
    let mut project_root = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            "--project-root" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--project-root requires a directory".to_string())?;
                project_root = Some(PathBuf::from(value));
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(RunOptions {
        manifest_path: manifest_path.ok_or_else(|| "--manifest is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
        project_root,
    })
}

fn parse_doctor_options(args: &[String]) -> std::result::Result<DoctorOptions, String> {
    let mut manifest_path = Some(PathBuf::from(".allie/manifest.yml"));
    let mut out_dir = PathBuf::from(".allie/doctor");
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--no-manifest" => {
                manifest_path = None;
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = PathBuf::from(value);
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(DoctorOptions {
        manifest_path,
        out_dir,
    })
}

fn parse_release_options(args: &[String]) -> std::result::Result<ReleaseOptions, String> {
    let mut packet_path = None;
    let mut out_dir = None;
    let mut changed_surfaces = Vec::new();
    let mut stale_after_days = 7;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--packet" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--packet requires a path".to_string())?;
                packet_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
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

    Ok(ReleaseOptions {
        packet_path: packet_path.ok_or_else(|| "--packet is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
        changed_surfaces,
        stale_after_days,
    })
}

fn parse_discovery_options(args: &[String]) -> std::result::Result<DiscoveryOptions, String> {
    let mut manifest_path = None;
    let mut out_dir = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(DiscoveryOptions {
        manifest_path: manifest_path.ok_or_else(|| "--manifest is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
    })
}

fn parse_map_options(args: &[String]) -> std::result::Result<MapOptions, String> {
    let mut manifest_path = None;
    let mut out_dir = None;
    let mut project_root = None;
    let mut agent_runner = AgentRunnerKind::Local;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--manifest requires a path".to_string())?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
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
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(MapOptions {
        manifest_path: manifest_path.ok_or_else(|| "--manifest is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        agent_runner,
    })
}

fn parse_report_options(args: &[String]) -> std::result::Result<ReportOptions, String> {
    let mut map_path = None;
    let mut packet_path = None;
    let mut out_dir = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--map" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--map requires a path".to_string())?;
                map_path = Some(PathBuf::from(value));
            }
            "--packet" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--packet requires a path".to_string())?;
                packet_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(ReportOptions {
        map_path: map_path.ok_or_else(|| "--map is required".to_string())?,
        packet_path: packet_path.ok_or_else(|| "--packet is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
    })
}

fn parse_promote_flow_options(args: &[String]) -> std::result::Result<PromoteFlowOptions, String> {
    let mut discovery_path = None;
    let mut flow_plan_path = None;
    let mut out_path = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--discovery" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--discovery requires a path".to_string())?;
                discovery_path = Some(PathBuf::from(value));
            }
            "--flow-plan" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--flow-plan requires a path".to_string())?;
                flow_plan_path = Some(PathBuf::from(value));
            }
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--out requires a path".to_string())?;
                out_path = Some(PathBuf::from(value));
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(PromoteFlowOptions {
        discovery_path: discovery_path.ok_or_else(|| "--discovery is required".to_string())?,
        flow_plan_path: flow_plan_path.ok_or_else(|| "--flow-plan is required".to_string())?,
        out_path: out_path.ok_or_else(|| "--out is required".to_string())?,
    })
}

fn run_v0(options: RunOptions) -> Result<RunReceipt> {
    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let context =
        RunContext::from_manifest_path(&options.manifest_path, options.project_root.as_deref())?;
    let started_at = context.now();
    out_dir::prepare_out_dir(&options.out_dir, "run")?;

    let run_id = context.new_run_id();
    // Absolutize the worker handshake paths. The bundled Node worker resolves
    // request/response/artifacts paths against its own repoRoot (the Allie
    // checkout), so relative paths only line up when Allie runs from its own
    // repo. Run from a consumer repo, relative paths resolve under the Allie
    // tree and the worker crashes on a missing request. Absolute paths make the
    // handshake independent of the worker's CWD assumptions.
    let out_dir_abs =
        fs::canonicalize(&options.out_dir).unwrap_or_else(|_| options.out_dir.clone());
    let worker_execution = worker::execute(
        &run_id,
        &manifest,
        &options.manifest_path,
        &out_dir_abs,
        context.worker_determinism(),
        manifest.preflight_failures(),
    )?;

    write_packet_and_report(
        &manifest,
        &options.manifest_path,
        &options.out_dir,
        worker_execution.response,
        worker_execution.run_failures,
        started_at,
        context.now(),
        run_id,
        &context.provenance,
    )
}

fn run_release(options: ReleaseOptions) -> Result<ReleaseReceipt> {
    out_dir::prepare_out_dir(&options.out_dir, "release")?;

    let packet = release::read_release_packet(&options.packet_path)?;

    let projection = release::project_release_decision(&packet, &options);
    let summary_path = options.out_dir.join("release-summary.json");
    let check_path = options.out_dir.join("github-check.json");
    let report_path = options.out_dir.join("release-report.html");
    write_json_pretty(&summary_path, &projection.summary)?;
    write_json_pretty(&check_path, &projection.github_check)?;
    write_string(
        &report_path,
        &release::render_release_report(&projection.summary),
    )?;

    out_dir::finalize_out_dir_manifest(&options.out_dir, "release")?;
    Ok(ReleaseReceipt {
        status: projection.summary.status.clone(),
        exit_class: projection.exit_class,
        summary_path,
        check_path,
        report_path,
    })
}

fn status_for_exit_class(exit_class: ExitClass) -> &'static str {
    match exit_class {
        ExitClass::Success => "completed",
        ExitClass::BlockingFinding => "blocked",
        ExitClass::InfrastructureFailure | ExitClass::Usage => "failed",
    }
}

fn run_compliance_report(options: ReportOptions) -> Result<ComplianceReportReceipt> {
    out_dir::prepare_out_dir(&options.out_dir, "report")?;
    let map: ProductMapPacket = read_json_file(&options.map_path)?;
    if map.schema != PRODUCT_MAP_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "invalid product map schema {}; expected {PRODUCT_MAP_SCHEMA}",
            map.schema
        )));
    }
    let packet: EvidencePacket = read_json_file(&options.packet_path)?;
    release::validate_release_packet(&packet)?;
    let report =
        compliance::build_compliance_report(&map, &packet, &options.map_path, &options.packet_path);
    let report_value = serde_json::to_value(&report).map_err(|source| AllieError::Json {
        context: "serialize compliance report for validation".to_string(),
        source,
    })?;
    compliance::validate_criterion_coverage_cells(&report_value)
        .map_err(AllieError::InvalidManifest)?;

    let report_json_path = options.out_dir.join("compliance-report.json");
    let report_html_path = options.out_dir.join("compliance-report.html");
    let summary_path = options.out_dir.join("summary.md");
    write_json_pretty(&report_json_path, &report)?;
    write_string(
        &report_html_path,
        &report::render_compliance_report(&report),
    )?;
    write_string(&summary_path, &report::render_compliance_summary(&report))?;

    out_dir::finalize_out_dir_manifest(&options.out_dir, "report")?;
    Ok(ComplianceReportReceipt {
        report_json_path,
        report_html_path,
        summary_path,
    })
}

pub(crate) fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        if !value.trim().is_empty() && seen.insert(value.clone()) {
            output.push(value);
        }
    }
    output
}

pub(crate) fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|source| AllieError::Io {
        context: format!("read json {}", path.display()),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| AllieError::Json {
        context: format!("parse json {}", path.display()),
        source,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FlowManifest {
    id: String,
    name: String,
    app_name: String,
    environment: String,
    auth_profile: Option<String>,
    #[serde(default)]
    credentials: CredentialConfig,
    /// Optional authenticated-audit recipe. When present, the worker establishes
    /// a session (form-login steps or a storageState file) before auditing gated
    /// states, and asserts the `authenticated_marker` on each one.
    #[serde(default)]
    auth: Option<AuthFlow>,
    target: ManifestTarget,
    policy: ManifestPolicy,
    #[serde(default)]
    artifacts: ArtifactPolicy,
    #[serde(default)]
    model: ModelPolicy,
    #[serde(default)]
    known_nondeterminism: Vec<String>,
    browser: BrowserSettings,
    flow: ManifestFlow,
}

impl FlowManifest {
    fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| AllieError::Io {
            context: format!("read manifest {}", path.display()),
            source,
        })?;
        serde_yaml::from_str(&text).map_err(|source| AllieError::Yaml {
            context: format!("parse manifest {}", path.display()),
            source,
        })
    }

    fn validate(&self) -> Result<()> {
        require_name("manifest id", &self.id)?;
        require_name("flow id", &self.flow.id)?;
        require_name("policy profile", &self.policy.profile)?;
        require_name("credential profile", &self.auth_profile_name())?;
        self.credentials.validate()?;
        self.model.validate()?;
        if self.flow.states.is_empty() {
            return Err(AllieError::InvalidManifest(
                "flow.states must contain at least one state".to_string(),
            ));
        }

        if self.target.kind == "local_fixture" {
            if self.target.fixture_dir.is_none() {
                return Err(AllieError::InvalidManifest(
                    "local_fixture target requires fixture_dir".to_string(),
                ));
            }
        } else if self.target.base_url.is_none() {
            return Err(AllieError::InvalidManifest(
                "non-fixture target requires base_url".to_string(),
            ));
        }

        for state in &self.flow.states {
            require_name("state id", &state.id)?;
            if !state.path.starts_with('/') {
                return Err(AllieError::InvalidManifest(format!(
                    "state {} path must start with /",
                    state.id
                )));
            }
            for (index, step) in state.steps.iter().enumerate() {
                step.validate(&state.id, index)?;
            }
        }

        if let Some(auth) = &self.auth {
            for assert in auth.assertions() {
                if assert.is_empty() {
                    return Err(AllieError::InvalidManifest(
                        "auth assertion requires a selector or url_contains".to_string(),
                    ));
                }
            }
            if auth.storage_state_env.is_none() && auth.steps.is_empty() {
                return Err(AllieError::InvalidManifest(
                    "auth without storage_state_env must declare at least one step".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn auth_profile_name(&self) -> String {
        self.credentials
            .profile
            .clone()
            .or_else(|| self.auth_profile.clone())
            .unwrap_or_else(|| "none".to_string())
    }

    fn credential_metadata(&self) -> CredentialProviderMetadata {
        let status = if self.credentials.provider == "env" {
            match self.credentials.env.as_deref() {
                Some(env_name) if std::env::var_os(env_name).is_some() => "available",
                Some(_) if self.credentials.required => "missing",
                Some(_) => "not_required",
                None => "misconfigured",
            }
        } else {
            "not_required"
        };

        CredentialProviderMetadata {
            provider: self.credentials.provider.clone(),
            env: self.credentials.env.clone(),
            required: self.credentials.required,
            status: status.to_string(),
        }
    }

    fn preflight_failures(&self) -> Vec<RunFailure> {
        let mut failures = Vec::new();

        if self.credentials.provider == "env" {
            match self.credentials.env.as_deref() {
                Some(env_name)
                    if self.credentials.required && std::env::var_os(env_name).is_none() =>
                {
                    failures.push(RunFailure::new(
                        "missing-credential",
                        "credential-provider",
                        format!(
                            "credential profile {} requires env {} but it is not set",
                            self.auth_profile_name(),
                            env_name
                        ),
                    ));
                }
                Some(_) | None => {}
            }
        }

        if let Some(auth) = &self.auth {
            // When the storageState hatch is used, require the named env var set
            // AND its target path readable; otherwise require every referenced
            // login `value_env`. Failures name only the env var, never a value.
            if let Some(storage_env) = auth.storage_state_env.as_deref() {
                match std::env::var_os(storage_env) {
                    None => failures.push(RunFailure::new(
                        "missing-credential",
                        "auth-storage-state",
                        format!(
                            "auth storage_state_env {storage_env} is required but it is not set"
                        ),
                    )),
                    Some(path) if !Path::new(&path).is_file() => failures.push(RunFailure::new(
                        "missing-credential",
                        "auth-storage-state",
                        format!(
                            "auth storage_state_env {storage_env} is set but its path is not a readable file"
                        ),
                    )),
                    Some(_) => {}
                }
            } else {
                for value_env in auth.referenced_value_envs() {
                    if std::env::var_os(value_env).is_none() {
                        failures.push(RunFailure::new(
                            "missing-credential",
                            "auth-credential",
                            format!("auth requires env {value_env} but it is not set"),
                        ));
                    }
                }
            }
        }

        if let Some(failure) = self.model.provider_allowlist_incomplete_failure() {
            failures.push(failure);
        } else if let Some(failure) = self.model.redaction_mode_failure() {
            failures.push(failure);
        } else if let Some(failure) = self.model.provider_allowlist_failure() {
            failures.push(failure);
        }

        failures
    }
}

fn require_name(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(AllieError::InvalidManifest(format!("{label} is required")));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CredentialConfig {
    profile: Option<String>,
    provider: String,
    env: Option<String>,
    required: bool,
}

impl Default for CredentialConfig {
    fn default() -> Self {
        Self {
            profile: None,
            provider: "none".to_string(),
            env: None,
            required: false,
        }
    }
}

impl CredentialConfig {
    fn validate(&self) -> Result<()> {
        match self.provider.as_str() {
            "none" => Ok(()),
            "env" => {
                if self.env.as_deref().unwrap_or_default().trim().is_empty() {
                    return Err(AllieError::InvalidManifest(
                        "env credential provider requires credentials.env".to_string(),
                    ));
                }
                Ok(())
            }
            provider => Err(AllieError::InvalidManifest(format!(
                "unsupported credential provider {provider}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestPolicy {
    profile: String,
    blocking_classes: Vec<String>,
    #[serde(default = "default_worker_timeout_ms")]
    worker_timeout_ms: u64,
}

fn default_worker_timeout_ms() -> u64 {
    DEFAULT_WORKER_TIMEOUT_MS
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestFlow {
    id: String,
    description: String,
    states: Vec<ManifestState>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestState {
    id: String,
    path: String,
    description: String,
    required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    steps: Vec<StateStep>,
    axe: bool,
    screenshot: bool,
    #[serde(default)]
    dom_snapshot: bool,
    #[serde(default)]
    accessibility_tree: bool,
    #[serde(default)]
    keyboard: bool,
    #[serde(default)]
    video: bool,
    #[serde(default)]
    trace: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    promotion_state: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum StateStep {
    Fill { fill: StateFill },
    Type { r#type: StateType },
    Click { click: StateClick },
    WaitFor { wait_for: StateWaitFor },
}

impl StateStep {
    fn validate(&self, state_id: &str, index: usize) -> Result<()> {
        let label = format!("state {state_id} step {index}");
        match self {
            StateStep::Fill { fill } => {
                require_name(&format!("{label} fill selector"), &fill.selector)?;
            }
            StateStep::Type { r#type } => {
                require_name(&format!("{label} type selector"), &r#type.selector)?;
                require_name(&format!("{label} type text"), &r#type.text)?;
            }
            StateStep::Click { click } => {
                require_name(&format!("{label} click selector"), &click.selector)?;
            }
            StateStep::WaitFor { wait_for } => {
                if wait_for.is_empty() {
                    return Err(AllieError::InvalidManifest(format!(
                        "{label} wait_for requires a selector or url_contains"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StateFill {
    pub(crate) selector: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StateType {
    pub(crate) selector: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StateClick {
    pub(crate) selector: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct StateWaitFor {
    #[serde(default)]
    pub(crate) selector: Option<String>,
    #[serde(default)]
    pub(crate) url_contains: Option<String>,
}

impl StateWaitFor {
    fn is_empty(&self) -> bool {
        self.selector.is_none() && self.url_contains.is_none()
    }
}

pub(crate) fn normalize_relative(base: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        base.join(path).to_string_lossy().to_string()
    }
}

#[derive(Debug)]
struct ContractFailure {
    state_id: String,
    route: String,
    message: String,
}

#[expect(
    clippy::too_many_arguments,
    reason = "packet writer is the narrow boundary where each receipt component stays explicit"
)]
fn write_packet_and_report(
    manifest: &FlowManifest,
    manifest_path: &Path,
    out_dir: &Path,
    response: WorkerResponse,
    run_failures: Vec<RunFailure>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    run_id: String,
    provenance: &GitProvenance,
) -> Result<RunReceipt> {
    fs::create_dir_all(out_dir).map_err(|source| AllieError::Io {
        context: format!("create output directory {}", out_dir.display()),
        source,
    })?;

    let replay_command = format!(
        "cargo run --locked -- run --manifest {} --out {}",
        manifest_path.display(),
        out_dir.display()
    );
    let contract_failures = if matches!(response.status, WorkerRunStatus::Error) {
        Vec::new()
    } else {
        response_contract_failures(manifest, &response)
    };
    let exit_class = exit_class_for_response(&response, &contract_failures, &run_failures);
    let deterministic_failures = response
        .states
        .iter()
        .map(|state| state.axe_violations.len())
        .sum::<usize>();
    let scripted_failures = response
        .states
        .iter()
        .map(|state| state.state_errors.len())
        .sum::<usize>()
        + contract_failures.len();
    let response_error_count = if run_failures.is_empty() {
        response.errors.len()
    } else {
        0
    };
    let infrastructure_failures =
        run_failures.len() + response_error_count + response.nondeterminism.len();

    let mut artifacts = worker::artifacts(out_dir, &response, &manifest.artifacts, finished_at)?;
    let findings = findings_from_response(
        &response,
        &artifacts,
        &contract_failures,
        &run_failures,
        &manifest.policy.profile,
        &replay_command,
    );
    let verdicts = verdicts_from_findings(manifest, &response, &findings);
    let failure_class = failure_class_for(exit_class, &response, &contract_failures, &run_failures);
    let mut packet = EvidencePacket {
        schema: EVIDENCE_SCHEMA.to_string(),
        summary: PacketSummary {
            status: exit_class.packet_status().to_string(),
            exit_code: exit_class.code(),
            deterministic_failures,
            scripted_failures,
            infrastructure_failures,
            states_captured: response.states.len(),
            failure_class,
        },
        run: RunMetadata {
            id: run_id.clone(),
            started_at: started_at.to_rfc3339(),
            finished_at: finished_at.to_rfc3339(),
            allie_version: env!("CARGO_PKG_VERSION").to_string(),
            git_sha: provenance.sha.clone(),
            git_branch: provenance.branch.clone(),
            ci_provider: std::env::var("CI").ok().map(|_| "generic-ci".to_string()),
            actor: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
        },
        target: TargetMetadata {
            base_url: response
                .actual_base_url
                .clone()
                .or_else(|| manifest.target.base_url.clone()),
            environment: manifest.environment.clone(),
            app_name: manifest.app_name.clone(),
            auth_profile: manifest.auth_profile_name(),
            credential_provider: manifest.credential_metadata(),
            flow_manifest: manifest_path.to_string_lossy().to_string(),
        },
        policy: PolicyMetadata {
            profile: manifest.policy.profile.clone(),
            blocking_classes: manifest.policy.blocking_classes.clone(),
            worker_timeout_ms: manifest.policy.worker_timeout_ms,
            model_provider_allowlist: manifest.model.provider_allowlist.clone(),
            model_status: if manifest.model.enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            zdr_required: manifest.model.zdr_required,
            model_egress_redaction: manifest.model.accepted_redaction_mode(),
            redaction_profile: manifest.artifacts.redaction_status.clone(),
            budget: PolicyBudget {
                model_calls: 0,
                max_states: manifest.flow.states.len(),
            },
        },
        coverage: coverage_from_response(manifest, &response, &findings),
        artifacts: Vec::new(),
        findings,
        verdicts,
        waivers: Vec::new(),
        agentic_assessments: Vec::new(),
        replay: Replay {
            command: replay_command,
            manifest_path: manifest_path.to_string_lossy().to_string(),
            environment_requirements: worker::environment_requirements(),
            credential_profile: manifest.auth_profile_name(),
            browser: manifest.browser.clone(),
            seed_data: vec!["checked-in fixture fixtures/login".to_string()],
            known_nondeterminism: manifest.known_nondeterminism.clone(),
        },
    };

    packet.artifacts = artifacts.clone();
    let report_path = out_dir.join("report.html");
    write_string(&report_path, &render_report(&packet))?;

    artifacts.push(artifact_for_path(
        "report-html",
        "html_report",
        out_dir,
        &report_path,
        None,
        "allie-report-writer",
        &manifest.artifacts,
        finished_at,
    )?);
    packet.artifacts = artifacts;
    let evidence_path = out_dir.join("evidence.json");
    write_json_pretty(&evidence_path, &packet)?;

    out_dir::finalize_out_dir_manifest(out_dir, "run")?;
    Ok(RunReceipt {
        run_id,
        exit_class,
        evidence_path,
        report_path,
    })
}

fn exit_class_for_response(
    response: &WorkerResponse,
    contract_failures: &[ContractFailure],
    run_failures: &[RunFailure],
) -> ExitClass {
    if matches!(response.status, WorkerRunStatus::Error)
        || !run_failures.is_empty()
        || !response.errors.is_empty()
        || !response.nondeterminism.is_empty()
    {
        ExitClass::InfrastructureFailure
    } else if response
        .states
        .iter()
        .any(|state| !state.axe_violations.is_empty() || !state.state_errors.is_empty())
        || !contract_failures.is_empty()
        || matches!(response.status, WorkerRunStatus::Failed)
    {
        ExitClass::BlockingFinding
    } else {
        ExitClass::Success
    }
}

fn failure_class_for(
    exit_class: ExitClass,
    response: &WorkerResponse,
    contract_failures: &[ContractFailure],
    run_failures: &[RunFailure],
) -> Option<String> {
    if let Some(failure) = run_failures.first() {
        return Some(failure.kind.clone());
    }
    if !response.nondeterminism.is_empty() {
        return Some("nondeterminism".to_string());
    }
    if matches!(response.status, WorkerRunStatus::Error) || !response.errors.is_empty() {
        return Some("worker-error".to_string());
    }
    if !contract_failures.is_empty() {
        return Some("required-evidence-missing".to_string());
    }
    match exit_class {
        ExitClass::BlockingFinding => Some("blocking-finding".to_string()),
        ExitClass::InfrastructureFailure => Some("infrastructure-failure".to_string()),
        ExitClass::Success | ExitClass::Usage => None,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "artifact metadata hashing keeps path, policy, and provenance explicit at call sites"
)]
pub(crate) fn artifact_for_path(
    id: &str,
    artifact_type: &str,
    out_dir: &Path,
    path: &Path,
    related_flow_state: Option<String>,
    creation_tool: &str,
    artifact_policy: &ArtifactPolicy,
    timestamp: DateTime<Utc>,
) -> Result<ArtifactMetadata> {
    Ok(ArtifactMetadata {
        id: id.to_string(),
        artifact_type: artifact_type.to_string(),
        path: path_relative_to(out_dir, path),
        hash: format!("sha256:{}", sha256_file(path)?),
        redaction_status: artifact_policy.redaction_status.clone(),
        retention_class: artifact_policy.retention_class.clone(),
        publication_class: PublicationClass::SensitiveLocal,
        unavailable_reason: None,
        related_flow_state,
        creation_tool: creation_tool.to_string(),
        timestamp: timestamp.to_rfc3339(),
    })
}

fn findings_from_response(
    response: &WorkerResponse,
    artifacts: &[ArtifactMetadata],
    contract_failures: &[ContractFailure],
    run_failures: &[RunFailure],
    policy_profile: &str,
    replay_command: &str,
) -> Vec<Finding> {
    let mut findings = response
        .states
        .iter()
        .flat_map(|state| {
            state
                .axe_violations
                .iter()
                .enumerate()
                .map(move |(index, violation)| {
                    let refs = artifacts
                        .iter()
                        .filter(|artifact| {
                            artifact.related_flow_state.as_deref() == Some(&state.id)
                        })
                        .map(|artifact| artifact.id.clone())
                        .collect::<Vec<_>>();
                    Finding {
                        id: format!("{}-axe-{}-{}", state.id, violation.id, index + 1),
                        title: violation
                            .help
                            .clone()
                            .unwrap_or_else(|| violation.id.clone()),
                        description: violation.description.clone().unwrap_or_else(|| {
                            format!("axe-core reported {} affected node(s)", violation.nodes)
                        }),
                        evidence_class: "deterministic".to_string(),
                        standard_obligation: obligation_from_tags(policy_profile, &violation.tags),
                        severity: violation
                            .impact
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        status: "fail".to_string(),
                        confidence: "machine_proven".to_string(),
                        source: "axe-core".to_string(),
                        affected_route: state.route.clone(),
                        affected_state: state.id.clone(),
                        artifact_refs: refs,
                        replay_command: replay_command.to_string(),
                    }
                })
        })
        .collect::<Vec<_>>();

    for state in &response.states {
        for (index, message) in state.state_errors.iter().enumerate() {
            findings.push(Finding {
                id: format!("{}-state-error-{}", state.id, index + 1),
                title: "Required route state failed".to_string(),
                description: message.clone(),
                evidence_class: "scripted".to_string(),
                standard_obligation: "required-route-state".to_string(),
                severity: "blocking".to_string(),
                status: "fail".to_string(),
                confidence: "script_observed".to_string(),
                source: "playwright-worker".to_string(),
                affected_route: state.route.clone(),
                affected_state: state.id.clone(),
                artifact_refs: Vec::new(),
                replay_command: replay_command.to_string(),
            });
        }
    }

    for (index, failure) in contract_failures.iter().enumerate() {
        findings.push(Finding {
            id: format!("{}-contract-failure-{}", failure.state_id, index + 1),
            title: "Required evidence artifact missing".to_string(),
            description: failure.message.clone(),
            evidence_class: "scripted".to_string(),
            standard_obligation: "required-evidence-artifact".to_string(),
            severity: "blocking".to_string(),
            status: "fail".to_string(),
            confidence: "script_observed".to_string(),
            source: "allie-evidence-contract".to_string(),
            affected_route: failure.route.clone(),
            affected_state: failure.state_id.clone(),
            artifact_refs: Vec::new(),
            replay_command: replay_command.to_string(),
        });
    }

    for (index, failure) in run_failures.iter().enumerate() {
        findings.push(Finding {
            id: format!("{}-{}", failure.kind, index + 1),
            title: "Run preflight failed".to_string(),
            description: failure.message.clone(),
            evidence_class: "infrastructure".to_string(),
            standard_obligation: failure.kind.clone(),
            severity: "blocking".to_string(),
            status: "fail".to_string(),
            confidence: "script_observed".to_string(),
            source: failure.source.clone(),
            affected_route: "run".to_string(),
            affected_state: "run".to_string(),
            artifact_refs: Vec::new(),
            replay_command: replay_command.to_string(),
        });
    }

    if run_failures.is_empty() {
        for (index, message) in response.errors.iter().enumerate() {
            findings.push(Finding {
                id: format!("worker-error-{}", index + 1),
                title: "Worker failed before producing complete evidence".to_string(),
                description: message.clone(),
                evidence_class: "infrastructure".to_string(),
                standard_obligation: "worker-error".to_string(),
                severity: "blocking".to_string(),
                status: "fail".to_string(),
                confidence: "script_observed".to_string(),
                source: "browser-worker".to_string(),
                affected_route: "run".to_string(),
                affected_state: "run".to_string(),
                artifact_refs: Vec::new(),
                replay_command: replay_command.to_string(),
            });
        }
    }

    for (index, message) in response.nondeterminism.iter().enumerate() {
        findings.push(Finding {
            id: format!("nondeterminism-{}", index + 1),
            title: "Run was marked nondeterministic".to_string(),
            description: message.clone(),
            evidence_class: "infrastructure".to_string(),
            standard_obligation: "nondeterminism".to_string(),
            severity: "blocking".to_string(),
            status: "fail".to_string(),
            confidence: "script_observed".to_string(),
            source: "browser-worker".to_string(),
            affected_route: "run".to_string(),
            affected_state: "run".to_string(),
            artifact_refs: Vec::new(),
            replay_command: replay_command.to_string(),
        });
    }

    findings
}

fn verdicts_from_findings(
    manifest: &FlowManifest,
    response: &WorkerResponse,
    findings: &[Finding],
) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    let finding_by_obligation = findings
        .iter()
        .map(|finding| (finding.standard_obligation.clone(), finding))
        .collect::<BTreeMap<_, _>>();

    if findings.is_empty() {
        verdicts.push(Verdict {
            obligation: deterministic_pass_obligation(&manifest.policy.profile),
            status: "pass".to_string(),
            confidence: "machine_proven".to_string(),
            evidence_class: "deterministic".to_string(),
            source: "axe-core".to_string(),
            affected_states: response
                .states
                .iter()
                .map(|state| state.id.clone())
                .collect(),
            finding_refs: Vec::new(),
        });
    }

    verdicts.extend(
        findings
            .iter()
            .filter(|finding| !finding.standard_obligation.starts_with("wcag22-aa:"))
            .map(|finding| Verdict {
                obligation: finding.standard_obligation.clone(),
                status: "fail".to_string(),
                confidence: finding.confidence.clone(),
                evidence_class: finding.evidence_class.clone(),
                source: finding.source.clone(),
                affected_states: vec![finding.affected_state.clone()],
                finding_refs: vec![finding.id.clone()],
            }),
    );

    let captured_states = response
        .states
        .iter()
        .map(|state| state.id.clone())
        .collect::<Vec<_>>();

    if manifest.policy.profile == "wcag22-aa" {
        let features =
            aggregate_features(response.states.iter().map(|state| state.features.as_ref()));
        let keyboard_observed = response
            .states
            .iter()
            .any(|state| !state.keyboard_focus_order.is_empty());
        for criterion in wcag22_success_criteria() {
            let Some(obligation) = criterion["obligation"].as_str() else {
                continue;
            };
            if let Some(finding) = finding_by_obligation.get(obligation) {
                verdicts.push(Verdict {
                    obligation: obligation.to_string(),
                    status: "fail".to_string(),
                    confidence: finding.confidence.clone(),
                    evidence_class: finding.evidence_class.clone(),
                    source: finding.source.clone(),
                    affected_states: vec![finding.affected_state.clone()],
                    finding_refs: vec![finding.id.clone()],
                });
                continue;
            }
            if let Some(verdict) = axe_pass::verdict(manifest, response, obligation) {
                verdicts.push(verdict);
                continue;
            }
            let method = criterion["method"].as_str().unwrap_or("human_review");
            let (status, confidence, evidence_class, source) =
                criterion_feature_verdict(obligation, method, &features, keyboard_observed);
            verdicts.push(Verdict {
                obligation: obligation.to_string(),
                status: status.to_string(),
                confidence: confidence.to_string(),
                evidence_class: evidence_class.to_string(),
                source: source.to_string(),
                affected_states: captured_states.clone(),
                finding_refs: Vec::new(),
            });
        }
        let mut seen = verdicts
            .iter()
            .map(|verdict| verdict.obligation.clone())
            .collect::<BTreeSet<_>>();
        for obligation in profile_obligation_list(&manifest.policy.profile, "scripted_obligations")
        {
            if seen.insert(obligation.clone()) {
                verdicts.push(Verdict {
                    obligation,
                    status: "needs_review".to_string(),
                    confidence: "requires_human_or_agent_review".to_string(),
                    evidence_class: "scripted".to_string(),
                    source: "allie-agentic-review-queue".to_string(),
                    affected_states: captured_states.clone(),
                    finding_refs: Vec::new(),
                });
            }
        }
        for obligation in
            profile_obligation_list(&manifest.policy.profile, "human_review_obligations")
        {
            if seen.insert(obligation.clone()) {
                verdicts.push(Verdict {
                    obligation,
                    status: "needs_review".to_string(),
                    confidence: "requires_human_or_agent_review".to_string(),
                    evidence_class: "human".to_string(),
                    source: "allie-obligation-profile".to_string(),
                    affected_states: captured_states.clone(),
                    finding_refs: Vec::new(),
                });
            }
        }
    } else {
        verdicts.extend(
            scripted_profile_obligations(&manifest.policy.profile)
                .into_iter()
                .map(|obligation| Verdict {
                    obligation,
                    status: "needs_review".to_string(),
                    confidence: "requires_human_or_agent_review".to_string(),
                    evidence_class: "scripted".to_string(),
                    source: "allie-agentic-review-queue".to_string(),
                    affected_states: captured_states.clone(),
                    finding_refs: Vec::new(),
                }),
        );

        verdicts.extend(
            human_review_profile_obligations(&manifest.policy.profile)
                .into_iter()
                .map(|obligation| Verdict {
                    obligation,
                    status: "needs_review".to_string(),
                    confidence: "script_observed".to_string(),
                    evidence_class: "human".to_string(),
                    source: "allie-obligation-profile".to_string(),
                    affected_states: captured_states.clone(),
                    finding_refs: Vec::new(),
                }),
        );
    }

    verdicts
}

fn coverage_from_response(
    manifest: &FlowManifest,
    response: &WorkerResponse,
    findings: &[Finding],
) -> Coverage {
    let mut routes = BTreeSet::new();
    let mut states = BTreeSet::new();
    let mut obligations = BTreeSet::new();

    for state in &response.states {
        routes.insert(state.route.clone());
        states.insert(state.id.clone());
    }

    if findings.is_empty() {
        obligations.insert(deterministic_pass_obligation(&manifest.policy.profile));
    } else {
        for finding in findings {
            obligations.insert(finding.standard_obligation.clone());
        }
    }
    if manifest.policy.profile == "wcag22-aa" {
        for criterion in wcag22_success_criteria() {
            if let Some(obligation) = criterion["obligation"].as_str() {
                obligations.insert(obligation.to_string());
            }
        }
    }

    let not_tested = scripted_profile_obligations(&manifest.policy.profile);
    let profile_human_review_scope = human_review_profile_obligations(&manifest.policy.profile);
    for obligation in not_tested.iter().chain(profile_human_review_scope.iter()) {
        obligations.insert(obligation.clone());
    }

    Coverage {
        routes_visited: routes.into_iter().collect(),
        surfaces_discovered: vec![manifest.app_name.clone()],
        flows_exercised: vec![manifest.flow.id.clone()],
        states_captured: states.into_iter().collect(),
        state_metadata: response
            .states
            .iter()
            .map(|state| StateMetadata {
                id: state.id.clone(),
                route: state.route.clone(),
                url: state.url.clone(),
                title: state.title.clone(),
                http_status: state.http_status,
                keyboard_focus_order: state.keyboard_focus_order.clone(),
                console_errors: state.console_errors.clone(),
                network_errors: state.network_errors.clone(),
                state_errors: state.state_errors.clone(),
                features: state.features.clone(),
            })
            .collect(),
        standards_obligations_evaluated: obligations.into_iter().collect(),
        obligations_not_tested: not_tested,
        profile_human_review_scope,
    }
}

fn render_report(packet: &EvidencePacket) -> String {
    let findings = if packet.findings.is_empty() {
        "<p>No deterministic axe failures were reported for the captured states.</p>".to_string()
    } else {
        let items = packet
            .findings
            .iter()
            .map(|finding| {
                format!(
                    "<li><strong>{}</strong><br><span>{}</span><br><code>{}</code></li>",
                    escape_html(&finding.title),
                    escape_html(&finding.description),
                    escape_html(&finding.affected_state)
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!("<ul>{items}</ul>")
    };

    let artifacts = packet
        .artifacts
        .iter()
        .map(|artifact| {
            format!(
                "<li><a href=\"{}\">{}</a> <span>{}</span><br><span>redaction: {}; retention: {}; unavailable: {}</span></li>",
                escape_html(&artifact.path),
                escape_html(&artifact.id),
                escape_html(&artifact.hash),
                escape_html(&artifact.redaction_status),
                escape_html(&artifact.retention_class),
                escape_html(artifact.unavailable_reason.as_deref().unwrap_or("none"))
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let state_metadata = packet
        .coverage
        .state_metadata
        .iter()
        .map(|state| {
            let mobile = state.features.as_ref().map_or_else(
                || "mobile viewport: not recorded".to_string(),
                |features| {
                    if features.mobile_viewport_checked {
                        format!(
                            "mobile viewport: {}x{} checked",
                            features.mobile_viewport_width, features.mobile_viewport_height
                        )
                    } else {
                        "mobile viewport: not checked".to_string()
                    }
                },
            );
            format!(
                "<li><strong>{}</strong> <span>{}</span><br><code>{}</code><br><span>HTTP status: {}; console errors: {}; network errors: {}; state errors: {}; keyboard stops: {}; {}</span></li>",
                escape_html(&state.id),
                escape_html(&state.title),
                escape_html(&state.url),
                state.http_status
                    .map(|status| status.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                state.console_errors.len(),
                state.network_errors.len(),
                state.state_errors.len(),
                state.keyboard_focus_order.len(),
                escape_html(&mobile)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let verdicts = packet
        .verdicts
        .iter()
        .map(|verdict| {
            format!(
                "<li><strong>{}</strong> <span>{}</span><br><code>{}</code><br><span>confidence: {}; evidence: {}; source: {}</span></li>",
                escape_html(&criterion_title(&verdict.obligation)),
                escape_html(&verdict.status),
                escape_html(&verdict.obligation),
                escape_html(&verdict.confidence),
                escape_html(&verdict.evidence_class),
                escape_html(&verdict.source)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Allie Evidence Report {run_id}</title>
  <style>
    :root {{ color-scheme: light; --ink: #151719; --muted: #58616c; --line: #d7dde5; --wash: #f5f7fa; --panel: #ffffff; --accent: #1f5eff; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; color: var(--ink); background: var(--wash); font: 16px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
    main {{ width: min(100% - 40px, 980px); margin: 0 auto; padding: 40px 0; }}
    h1 {{ font-size: 44px; line-height: 1.05; margin: 0 0 10px; letter-spacing: 0; }}
    h2 {{ font-size: 13px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); margin: 0 0 12px; }}
    p {{ margin: 0; }}
    p + p {{ margin-top: 8px; }}
    code {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: #edf1f6; padding: 0.08em 0.28em; border-radius: 4px; }}
    section {{ background: var(--panel); border: 1px solid var(--line); padding: 20px; margin-top: 18px; }}
    .summary {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 1px; background: var(--line); border: 1px solid var(--line); margin-top: 22px; }}
    .summary div {{ background: var(--panel); padding: 16px; }}
    .label {{ color: var(--muted); font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; }}
    .value {{ font-size: 24px; font-weight: 700; margin-top: 4px; }}
    a {{ color: var(--accent); }}
    li + li {{ margin-top: 8px; }}
    @media (max-width: 760px) {{ main {{ width: min(100% - 24px, 980px); }} .summary {{ grid-template-columns: 1fr; }} }}
  </style>
</head>
<body>
  <main>
    <p class="label">Allie evidence status, not a legal compliance guarantee</p>
    <h1>{app_name}</h1>
    <p>Run <code>{run_id}</code> exercised <code>{flow_manifest}</code> with policy profile <code>{policy}</code>.</p>
    <div class="summary" aria-label="Run summary">
      <div><p class="label">Status</p><p class="value">{status}</p></div>
      <div><p class="label">Exit</p><p class="value">{exit_code}</p></div>
      <div><p class="label">States</p><p class="value">{states}</p></div>
      <div><p class="label">Deterministic Failures</p><p class="value">{failures}</p></div>
    </div>
    <section>
      <h2>Replay</h2>
      <p><code>{replay}</code></p>
    </section>
    <section>
      <h2>Captured States</h2>
      <ul>{state_metadata}</ul>
    </section>
    <section>
      <h2>Findings</h2>
      {findings}
    </section>
    <section>
      <h2>Verdicts</h2>
      <ul>{verdicts}</ul>
    </section>
    <section>
      <h2>Artifacts</h2>
      <ul>{artifacts}</ul>
    </section>
    <section>
      <h2>Residual Review Needs</h2>
      <p>{review_needs}</p>
    </section>
  </main>
</body>
</html>
"#,
        run_id = escape_html(&packet.run.id),
        app_name = escape_html(&packet.target.app_name),
        flow_manifest = escape_html(&packet.target.flow_manifest),
        policy = escape_html(&packet.policy.profile),
        status = escape_html(&packet.summary.status),
        exit_code = packet.summary.exit_code,
        states = packet.summary.states_captured,
        failures = packet.summary.deterministic_failures,
        replay = escape_html(&packet.replay.command),
        state_metadata = state_metadata,
        findings = findings,
        verdicts = verdicts,
        artifacts = artifacts,
        review_needs = escape_html(&packet.coverage.profile_human_review_scope.join(", ")),
    )
}

fn response_contract_failures(
    manifest: &FlowManifest,
    response: &WorkerResponse,
) -> Vec<ContractFailure> {
    let mut failures = Vec::new();

    for expected in &manifest.flow.states {
        let Some(actual) = response.states.iter().find(|state| state.id == expected.id) else {
            failures.push(ContractFailure {
                state_id: expected.id.clone(),
                route: expected.path.clone(),
                message: format!("required state {} was not captured", expected.id),
            });
            continue;
        };

        if expected.required && expected.axe && actual.axe_json_path.is_none() {
            failures.push(ContractFailure {
                state_id: expected.id.clone(),
                route: expected.path.clone(),
                message: format!(
                    "required state {} did not include raw axe JSON",
                    expected.id
                ),
            });
        }

        if expected.required && expected.screenshot && actual.screenshot_path.is_none() {
            failures.push(ContractFailure {
                state_id: expected.id.clone(),
                route: expected.path.clone(),
                message: format!(
                    "required state {} did not include a screenshot",
                    expected.id
                ),
            });
        }
    }

    failures
}

pub(crate) fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(|source| AllieError::Json {
        context: format!("serialize json {}", path.display()),
        source,
    })?;
    write_string(path, &(json + "\n"))
}

pub(crate) fn write_string_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AllieError::Io {
            context: format!("create directory {}", parent.display()),
            source,
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("atomic-write");
    for attempt in 0..16 {
        let temp_path = path.with_file_name(format!(
            ".{file_name}.tmp-{}-{}-{attempt}",
            std::process::id(),
            current_time_millis()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(AllieError::Io {
                    context: format!("open temp file {}", temp_path.display()),
                    source,
                });
            }
        };
        if let Err(source) = file.write_all(contents.as_bytes()) {
            let _ = fs::remove_file(&temp_path);
            return Err(AllieError::Io {
                context: format!("write temp file {}", temp_path.display()),
                source,
            });
        }
        if let Err(source) = file.sync_all() {
            let _ = fs::remove_file(&temp_path);
            return Err(AllieError::Io {
                context: format!("sync temp file {}", temp_path.display()),
                source,
            });
        }
        drop(file);
        return fs::rename(&temp_path, path).map_err(|source| AllieError::Io {
            context: format!("replace {}", path.display()),
            source,
        });
    }
    Err(AllieError::Io {
        context: format!("create temp file for {}", path.display()),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "exhausted atomic write temp file attempts",
        ),
    })
}

fn write_string(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AllieError::Io {
            context: format!("create directory {}", parent.display()),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| AllieError::Io {
        context: format!("write {}", path.display()),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| AllieError::Io {
        context: format!("read artifact {}", path.display()),
        source,
    })?;
    let digest = Sha256::digest(&bytes);
    Ok(hex_lower(&digest))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn path_relative_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

fn new_run_id() -> String {
    let millis = current_time_millis();
    format!("run-{millis}")
}

fn new_job_id() -> String {
    let millis = current_time_millis();
    format!("job-{millis}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;
    use crate::standards::{criterion_level, criterion_principle, criterion_source_url};
    use crate::test_support::{
        start_live_discovery_site, unused_local_base_url, write_live_discovery_manifest,
    };
    use std::process::Command;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Serializes tests that mutate process-wide auth env vars so parallel test
    // threads cannot observe each other's set/remove. Poisoning is irrelevant
    // (we only need exclusion), so an outer panic must not block other tests.
    static AUTH_ENV_GUARD: Mutex<()> = Mutex::new(());
    static SOURCE_DATE_EPOCH_GUARD: Mutex<()> = Mutex::new(());
    // Serializes Playwright-backed workbench CLI tests because the browser
    // worker process and artifact paths are not isolated enough for parallel
    // launches to be a meaningful unit test signal.
    static WORKBENCH_CLI_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn placeholder_cli_points_to_v0_command() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(Vec::<String>::new(), &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        let output = String::from_utf8(stdout).unwrap();
        assert!(output.contains("accessibility evidence"));
        assert!(output.contains("allie run --manifest"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn example_manifest_validates_the_checked_in_fixture() {
        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();

        manifest.validate().unwrap();

        assert_eq!(manifest.id, "login-flow");
        assert_eq!(manifest.policy.profile, "wcag22-aa");
        assert_eq!(manifest.flow.states[0].id, "login-form");
    }

    #[test]
    fn run_context_freezes_clock_and_worker_determinism_explicitly() {
        let _guard = SOURCE_DATE_EPOCH_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
            std::env::set_var("ALLIE_FIXTURE_PORT", "51423");
        }

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let context =
            RunContext::from_manifest_path(Path::new("examples/login-flow.yml"), None).unwrap();
        let determinism = context.worker_determinism().unwrap();

        assert_eq!(manifest.id, "login-flow");
        assert_eq!(context.now().to_rfc3339(), "2023-11-14T22:13:20+00:00");
        assert_eq!(context.new_run_id(), "run-1700000000000");
        assert_eq!(
            determinism.timestamp.as_deref(),
            Some("2023-11-14T22:13:20.000Z")
        );
        assert_eq!(determinism.fixture_port, Some(51423));

        unsafe {
            std::env::remove_var("SOURCE_DATE_EPOCH");
            std::env::remove_var("ALLIE_FIXTURE_PORT");
        }
    }

    #[test]
    fn run_context_reads_git_provenance_from_manifest_repo_not_process_cwd() {
        let temp = tempdir().unwrap();
        let manifest_path = write_static_manifest(temp.path(), "home");
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "allie-test@example.invalid"])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Allie Test"])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["add", "flow.yml"])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "fixture flow"])
            .current_dir(temp.path())
            .status()
            .unwrap();

        let context = RunContext::from_manifest_path(&manifest_path, None).unwrap();

        assert_eq!(
            context.provenance.sha,
            runtime::git_metadata_at(temp.path(), &["rev-parse", "--short", "HEAD"]).unwrap()
        );
        assert_eq!(
            context.provenance.branch,
            runtime::git_metadata_at(temp.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap()
        );
    }

    #[test]
    fn state_action_steps_validate_and_serialize_to_state_contract() {
        let manifest = FlowManifest::load(Path::new("examples/action-steps-flow.yml")).unwrap();

        manifest.validate().unwrap();

        assert_eq!(manifest.flow.states[0].id, "open-menu");
        assert_eq!(manifest.flow.states[0].steps.len(), 2);
        let state_json = serde_json::to_string(&manifest.flow.states[0]).unwrap();
        assert!(state_json.contains("\"steps\""));
        assert!(state_json.contains("\"click\""));
        assert!(state_json.contains("\"wait_for\""));
        let typed_state_json = serde_json::to_string(&manifest.flow.states[1]).unwrap();
        assert!(typed_state_json.contains("\"fill\""));
        assert!(typed_state_json.contains("\"type\""));
        assert!(!state_json.contains("value_env"));
    }

    #[test]
    fn state_action_wait_for_requires_a_real_assertion() {
        let mut manifest = FlowManifest::load(Path::new("examples/action-steps-flow.yml")).unwrap();
        manifest.flow.states[0].steps = vec![StateStep::WaitFor {
            wait_for: StateWaitFor::default(),
        }];

        let error = manifest.validate().unwrap_err().to_string();

        assert!(error.contains("state open-menu step 0 wait_for requires"));
    }

    #[test]
    fn init_cli_scaffolds_manifest_and_next_verify_command() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join(".allie/manifest.yml");
        let fixture_dir = fs::canonicalize("fixtures/login").unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "init".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--app-name".to_string(),
                "Allie Consumer Fixture".to_string(),
                "--fixture-dir".to_string(),
                fixture_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("Setup checklist:"));
        assert!(stdout.contains("allie doctor --manifest"));
        assert!(stdout.contains("npx playwright install chromium"));
        assert!(stdout.contains("allie verify --manifest"));
        assert!(!stdout.to_lowercase().contains("github"));
        let manifest = FlowManifest::load(&manifest_path).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.app_name, "Allie Consumer Fixture");
        assert_eq!(manifest.target.kind, "local_fixture");
        assert_eq!(manifest.policy.profile, "wcag22-aa");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_cli_with_io(
            vec![
                "init".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--app-name".to_string(),
                "Allie Consumer Fixture".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, ExitClass::InfrastructureFailure.code());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("already exists")
        );
    }

    #[test]
    fn packet_and_report_capture_worker_artifacts_and_replay() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let artifacts_dir = out_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        fs::write(
            artifacts_dir.join("axe-login-form.json"),
            br#"{"violations":[]}"#,
        )
        .unwrap();
        fs::write(artifacts_dir.join("login-form.png"), b"fake-png").unwrap();

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let response = passing_worker_response();
        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-test".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::Success);
        assert!(receipt.evidence_path.exists());
        assert!(receipt.report_path.exists());

        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"schema\": \"allie.evidence.v0\""));
        assert!(packet.contains("sha256:"));
        assert!(packet.contains("\"retention_class\": \"local_ephemeral\""));
        assert!(packet.contains("\"publication_class\": \"sensitive_local\""));
        assert!(packet.contains("\"infrastructure_failures\": 0"));
        assert!(packet.contains("\"title\": \"Allie Fixture Login\""));
        assert!(packet.contains("wcag22-aa:deterministic-axe-rules"));
        // Every criterion is attempted: nothing is left "not tested".
        assert!(!packet.contains("\"status\": \"not_tested\""));
        assert!(packet.contains("\"status\": \"needs_review\""));
        assert!(packet.contains("cargo run --locked -- run --manifest examples/login-flow.yml"));
        let packet_json: EvidencePacket = serde_json::from_str(&packet).unwrap();
        assert_eq!(
            packet_json.run.git_sha,
            runtime::git_metadata(&["rev-parse", "--short", "HEAD"]).unwrap()
        );
        assert_eq!(
            packet_json.run.git_branch,
            runtime::git_metadata(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap()
        );

        let report = fs::read_to_string(receipt.report_path).unwrap();
        assert!(report.contains("Allie evidence status"));
        assert!(report.contains("No deterministic axe failures"));
        assert!(report.contains("wcag22-aa:2.1.1-keyboard-traversal"));
        assert!(!report.to_lowercase().contains("compliance score"));
    }

    #[test]
    fn deterministic_axe_violations_return_blocking_exit_class() {
        let mut response = passing_worker_response();
        response.status = WorkerRunStatus::Failed;
        response.states[0].axe_violations.push(AxeViolation {
            id: "color-contrast".to_string(),
            impact: Some("serious".to_string()),
            help: Some("Elements must meet minimum color contrast ratio thresholds".to_string()),
            description: Some("axe reported contrast failure".to_string()),
            tags: vec!["wcag143".to_string()],
            nodes: 1,
        });

        assert_eq!(
            exit_class_for_response(&response, &[], &[]),
            ExitClass::BlockingFinding
        );
    }

    #[test]
    fn missing_required_worker_artifacts_block_success_packets() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let artifacts_dir = out_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        fs::write(
            artifacts_dir.join("axe-login-form.json"),
            br#"{"violations":[]}"#,
        )
        .unwrap();

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let mut response = passing_worker_response();
        response.states[0].screenshot_path = None;
        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-missing-artifact".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::BlockingFinding);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"status\": \"fail\""));
        assert!(packet.contains("did not include a screenshot"));
    }

    #[test]
    fn missing_required_credentials_write_error_packet_without_secret_values() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        manifest.credentials = CredentialConfig {
            profile: Some("staging-secret-profile".to_string()),
            provider: "env".to_string(),
            env: Some("ALLIE_TEST_DO_NOT_SET_SECRET".to_string()),
            required: true,
        };
        let failures = manifest.preflight_failures();
        let response = WorkerResponse::error(
            failures
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        );

        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            failures,
            Utc::now(),
            Utc::now(),
            "run-missing-credential".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::InfrastructureFailure);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"failure_class\": \"missing-credential\""));
        assert!(packet.contains("\"auth_profile\": \"staging-secret-profile\""));
        assert!(packet.contains("\"env\": \"ALLIE_TEST_DO_NOT_SET_SECRET\""));
        assert!(!packet.contains("\"standard_obligation\": \"worker-error\""));
        assert!(!packet.contains("super-secret-value"));

        let report = fs::read_to_string(receipt.report_path).unwrap();
        assert!(report.contains("Run preflight failed"));
        assert!(!report.contains("super-secret-value"));
    }

    #[test]
    fn auth_fixture_manifest_validates_and_carries_auth_block() {
        let manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.policy.worker_timeout_ms, 90_000);

        let auth = manifest.auth.as_ref().expect("auth block present");
        assert_eq!(auth.start_path.as_deref(), Some("/login.html"));
        assert_eq!(auth.steps.len(), 4);
        let marker = auth.authenticated_marker.as_ref().expect("marker present");
        assert_eq!(marker.selector.as_deref(), Some("#dashboard"));
        assert_eq!(
            auth.referenced_value_envs(),
            vec!["ALLIE_AUTH_FIXTURE_USER", "ALLIE_AUTH_FIXTURE_PASSWORD"]
        );

        let negative =
            FlowManifest::load(Path::new("examples/auth-fixture-flow-negative.yml")).unwrap();
        negative.validate().unwrap();
        assert_eq!(negative.policy.worker_timeout_ms, 90_000);

        let storage =
            FlowManifest::load(Path::new("examples/auth-fixture-storage-state-flow.yml")).unwrap();
        storage.validate().unwrap();
        assert_eq!(storage.policy.worker_timeout_ms, 90_000);
    }

    #[test]
    fn auth_preflight_fails_with_missing_credential_when_value_env_unset() {
        let _guard = AUTH_ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure the auth env vars are unset for this assertion.
        // SAFETY: single-threaded test.
        unsafe {
            std::env::remove_var("ALLIE_AUTH_FIXTURE_PASSWORD");
            std::env::remove_var("ALLIE_AUTH_FIXTURE_USER");
        }

        let manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();
        let failures = manifest.preflight_failures();

        assert!(
            failures
                .iter()
                .any(|failure| failure.kind == "missing-credential"
                    && failure.message.contains("ALLIE_AUTH_FIXTURE_PASSWORD")),
            "expected a missing-credential failure naming the unset auth env var"
        );
    }

    #[test]
    fn auth_storage_state_preflight_requires_a_readable_file() {
        let _guard = AUTH_ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempdir().unwrap();
        let state_file = temp.path().join("storage-state.json");
        fs::write(&state_file, "{}").unwrap();

        let mut manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();
        // Switch the auth flow to the storageState hatch; clear the credential
        // block so only the hatch drives preflight.
        manifest.credentials = CredentialConfig::default();
        let auth = manifest.auth.as_mut().unwrap();
        auth.storage_state_env = Some("ALLIE_AUTH_FIXTURE_STORAGE_STATE".to_string());

        // SAFETY: single-threaded under AUTH_ENV_GUARD.
        unsafe {
            std::env::remove_var("ALLIE_AUTH_FIXTURE_STORAGE_STATE");
        }
        assert!(
            manifest
                .preflight_failures()
                .iter()
                .any(|f| f.kind == "missing-credential"
                    && f.message.contains("ALLIE_AUTH_FIXTURE_STORAGE_STATE")),
            "unset storage_state_env must fail preflight"
        );

        unsafe {
            std::env::set_var("ALLIE_AUTH_FIXTURE_STORAGE_STATE", state_file.as_os_str());
        }
        assert!(
            manifest.preflight_failures().is_empty(),
            "storage_state_env pointing at a readable file must pass preflight"
        );

        unsafe {
            std::env::remove_var("ALLIE_AUTH_FIXTURE_STORAGE_STATE");
        }
    }

    #[test]
    fn auth_validation_rejects_empty_assertions_and_stepless_flows() {
        let manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();

        // An assertion with neither selector nor url_contains is meaningless.
        let mut empty_marker = manifest.clone();
        empty_marker.auth.as_mut().unwrap().authenticated_marker = Some(crate::auth::AuthAssert {
            selector: None,
            url_contains: None,
        });
        assert!(empty_marker.validate().is_err());

        // No steps and no storageState hatch leaves nothing to establish a session.
        let mut no_steps = manifest.clone();
        {
            let auth = no_steps.auth.as_mut().unwrap();
            auth.steps.clear();
            auth.storage_state_env = None;
        }
        assert!(no_steps.validate().is_err());

        // The shipped example is well-formed.
        manifest.validate().unwrap();
    }

    #[test]
    fn model_policy_enabled_without_allowlist_fails_closed() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        manifest.model.enabled = true;
        manifest.model.redaction = Some(ModelRedactionMode::None);
        manifest.model.provider_allowlist = Vec::new();
        let failures = manifest.preflight_failures();
        let response = WorkerResponse::error(
            failures
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        );

        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            failures,
            Utc::now(),
            Utc::now(),
            "run-model-policy".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::InfrastructureFailure);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"failure_class\": \"model-policy-incomplete\""));
        assert!(packet.contains("\"model_status\": \"enabled\""));
        assert!(!packet.contains("\"standard_obligation\": \"worker-error\""));
    }

    #[test]
    fn worker_error_and_partial_write_responses_map_to_infrastructure_failure() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let response = WorkerResponse::error(
            "worker partial-write: parse response .allie/run/worker-response.json".to_string(),
        );

        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-partial-write".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::InfrastructureFailure);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"status\": \"error\""));
        assert!(packet.contains("\"failure_class\": \"worker-error\""));
        assert!(packet.contains("worker partial-write"));
    }

    #[test]
    fn worker_timeout_and_crash_kinds_are_stable_packet_failure_classes() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let failure = RunFailure::new(
            "worker-timeout",
            "worker-adapter",
            "worker timed out after 1 ms".to_string(),
        );
        let response = WorkerResponse::error(failure.message.clone());

        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            vec![failure],
            Utc::now(),
            Utc::now(),
            "run-timeout".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::InfrastructureFailure);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"failure_class\": \"worker-timeout\""));
        assert!(packet.contains("\"infrastructure_failures\": 1"));
    }

    #[test]
    fn nondeterminism_marks_packet_error_instead_of_release_pass() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let artifacts_dir = out_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        fs::write(
            artifacts_dir.join("axe-login-form.json"),
            br#"{"violations":[]}"#,
        )
        .unwrap();
        fs::write(artifacts_dir.join("login-form.png"), b"fake-png").unwrap();

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let mut response = passing_worker_response();
        response
            .nondeterminism
            .push("route state changed between capture attempts".to_string());

        let receipt = write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            &out_dir,
            response,
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-nondeterminism".to_string(),
            &test_provenance(),
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::InfrastructureFailure);
        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"failure_class\": \"nondeterminism\""));
        assert!(packet.contains("route state changed between capture attempts"));
    }

    #[test]
    fn evidence_schema_is_formal_v0_schema() {
        let schema = fs::read_to_string("schemas/allie.evidence.v0.schema.json").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();

        assert_eq!(parsed["properties"]["schema"]["const"], "allie.evidence.v0");
        assert!(
            parsed["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "replay")
        );
        assert!(
            parsed["properties"]["waivers"]["items"]["anyOf"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["required"][0] == "packet_ref")
        );
        assert!(
            parsed["properties"]["policy"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "model_egress_redaction")
        );
        let artifact = &parsed["properties"]["artifacts"]["items"];
        let required = artifact["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("publication_class")));
        assert_eq!(
            artifact["properties"]["publication_class"]["enum"],
            serde_json::json!(["sensitive_local", "redacted_shareable", "public_summary"])
        );
    }

    #[test]
    fn wcag22_profile_maps_axe_tags_to_versioned_obligations() {
        let profile: serde_json::Value =
            serde_json::from_str(standards::WCAG22_AA_PROFILE_JSON).unwrap();

        assert_eq!(profile["id"], "wcag22-aa");
        assert_eq!(
            obligation_from_tags("wcag22-aa", &["wcag2aa".to_string(), "wcag143".to_string()]),
            "wcag22-aa:1.4.3-contrast-minimum"
        );
    }

    #[test]
    fn standards_verdicts_preserve_residual_review_without_legal_claims() {
        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let response = passing_worker_response();
        let verdicts = verdicts_from_findings(&manifest, &response, &[]);

        assert!(verdicts.iter().any(|verdict| verdict.status == "pass"
            && verdict.obligation == "wcag22-aa:deterministic-axe-rules"));
        assert!(
            verdicts
                .iter()
                .any(|verdict| verdict.status == "needs_review"
                    && verdict.obligation == "wcag22-aa:2.1.1-keyboard-traversal")
        );
        assert!(
            !verdicts
                .iter()
                .any(|verdict| verdict.status == "not_tested"),
            "no criterion may be left not_tested"
        );
        assert!(
            verdicts
                .iter()
                .any(|verdict| verdict.status == "needs_review"
                    && verdict.obligation == "wcag22-aa:human-assistive-technology-review")
        );
        assert!(
            !verdicts
                .iter()
                .any(|verdict| verdict.obligation == "legal-compliance")
        );
    }

    #[test]
    fn wcag22_profile_contains_complete_aa_obligation_ledger() {
        let profile: serde_json::Value =
            serde_json::from_str(standards::WCAG22_AA_PROFILE_JSON).unwrap();
        let criteria = profile["success_criteria"].as_array().unwrap();

        assert_eq!(criteria.len(), 55);
        assert!(
            criteria
                .iter()
                .any(|criterion| criterion["num"] == "2.4.11")
        );
        assert!(criteria.iter().any(|criterion| criterion["num"] == "3.3.8"));
        assert!(!criteria.iter().any(|criterion| criterion["num"] == "4.1.1"));
        assert!(
            criteria
                .iter()
                .all(|criterion| criterion["level"] == "A" || criterion["level"] == "AA")
        );
        assert!(criteria.iter().all(|criterion| {
            criterion["obligation"]
                .as_str()
                .is_some_and(|value| value.starts_with("wcag22-aa:"))
        }));
    }

    #[test]
    fn wcag22_ledger_projects_to_eaa_wcag21_aa_view_without_overclaiming() {
        let projection = standards::wcag21_aa_profile_view();

        assert_eq!(projection.id, "wcag21-aa");
        assert_eq!(projection.total_success_criteria, 50);
        assert_eq!(projection.included_criteria.len(), 49);
        assert!(
            projection
                .included_criteria
                .contains(&"wcag22-aa:1.4.10-reflow".to_string())
        );
        assert!(
            projection
                .included_criteria
                .contains(&"wcag22-aa:2.5.1-pointer-gestures".to_string())
        );
        assert!(
            projection
                .included_criteria
                .contains(&"wcag22-aa:2.5.4-motion-actuation".to_string())
        );
        assert!(
            !projection
                .included_criteria
                .contains(&"wcag22-aa:2.5.8-target-size-minimum".to_string()),
            "WCAG 2.2-only target-size-minimum must not inflate the EAA/WCAG 2.1 view"
        );
        assert_eq!(
            projection.missing_legacy_criteria,
            vec!["wcag21-aa:4.1.1-parsing".to_string()],
            "the WCAG 2.1-only Parsing criterion must be explicit, not silently dropped"
        );
    }

    #[test]
    fn discovery_cli_emits_packet_and_flow_plan_then_promotes_manifest() {
        let temp = tempdir().unwrap();
        let discovery_dir = temp.path().join("discovery");
        let generated_manifest = temp.path().join("generated.yml");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "discover".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                discovery_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let packet_path = discovery_dir.join("discovery.json");
        let flow_plan_path = discovery_dir.join("flow-plan.json");
        assert!(packet_path.exists());
        assert!(flow_plan_path.exists());
        let packet: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&packet_path).unwrap()).unwrap();
        assert_eq!(packet["schema"], "allie.discovery.v0");
        assert!(
            packet["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|surface| surface["id"] == "settings")
        );
        assert_eq!(packet["promotion"]["default_state"], "generated_candidate");
        let flow_plan: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&flow_plan_path).unwrap()).unwrap();
        let candidates = flow_plan["candidates"].as_array().unwrap();
        let home = candidates
            .iter()
            .find(|candidate| candidate["id"] == "home")
            .expect("home candidate present");
        assert!(
            home["steps"]
                .as_array()
                .is_some_and(|steps| steps.len() >= 2),
            "generated home candidate should include click/wait steps for the menu"
        );
        let settings = candidates
            .iter()
            .find(|candidate| candidate["id"] == "settings")
            .expect("settings candidate present");
        assert!(
            settings["steps"]
                .as_array()
                .is_some_and(|steps| steps.len() >= 3),
            "generated settings candidate should include fill/type/wait steps for the email form"
        );

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "promote-flow".to_string(),
                "--discovery".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--flow-plan".to_string(),
                flow_plan_path.to_string_lossy().to_string(),
                "--out".to_string(),
                generated_manifest.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let promoted = FlowManifest::load(&generated_manifest).unwrap();
        assert_eq!(promoted.policy.worker_timeout_ms, 90_000);
        let generated = fs::read_to_string(generated_manifest).unwrap();
        assert!(generated.contains("promotion_state: verified_flow"));
        assert!(generated.contains("accessibility_tree: true"));
        assert!(generated.contains("keyboard: true"));
        assert!(generated.contains("steps:"));
        assert!(generated.contains("click:"));
        assert!(generated.contains("wait_for:"));
        assert!(generated.contains("qa@example.test"));
    }

    #[test]
    fn promote_flow_rejects_invalid_candidate_steps() {
        let temp = tempdir().unwrap();
        let discovery_dir = temp.path().join("discovery");
        let generated_manifest = temp.path().join("generated.yml");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "discover".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                discovery_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let packet_path = discovery_dir.join("discovery.json");
        let flow_plan_path = discovery_dir.join("flow-plan.json");
        let mut flow_plan: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&flow_plan_path).unwrap()).unwrap();
        flow_plan["candidates"][0]["steps"] = serde_json::json!([{ "wait_for": {} }]);
        fs::write(
            &flow_plan_path,
            serde_json::to_string_pretty(&flow_plan).unwrap(),
        )
        .unwrap();

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "promote-flow".to_string(),
                "--discovery".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--flow-plan".to_string(),
                flow_plan_path.to_string_lossy().to_string(),
                "--out".to_string(),
                generated_manifest.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_ne!(code, 0, "promote-flow must reject invalid candidate steps");
        assert!(
            String::from_utf8_lossy(&stderr)
                .contains("wait_for requires a selector or url_contains"),
            "stderr={}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(!generated_manifest.exists());
    }

    #[test]
    fn auth_discovery_does_not_promote_bootstrap_route_as_a_gated_state() {
        let temp = tempdir().unwrap();
        let discovery_dir = temp.path().join("discovery");
        let generated_manifest = temp.path().join("generated.yml");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "discover".to_string(),
                "--manifest".to_string(),
                "examples/auth-fixture-flow.yml".to_string(),
                "--out".to_string(),
                discovery_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let flow_plan_path = discovery_dir.join("flow-plan.json");
        let flow_plan: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&flow_plan_path).unwrap()).unwrap();
        let candidates = flow_plan["candidates"].as_array().unwrap();
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate["id"] == "dashboard"),
            "discovery must keep the operator-declared gated state"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate["path"] != "/login.html"),
            "auth bootstrap route must not be promoted as gated coverage"
        );

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "promote-flow".to_string(),
                "--discovery".to_string(),
                discovery_dir
                    .join("discovery.json")
                    .to_string_lossy()
                    .to_string(),
                "--flow-plan".to_string(),
                flow_plan_path.to_string_lossy().to_string(),
                "--out".to_string(),
                generated_manifest.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let generated = FlowManifest::load(&generated_manifest).unwrap();
        assert!(
            generated
                .flow
                .states
                .iter()
                .any(|state| state.path == "/dashboard.html")
        );
        assert!(
            generated
                .flow
                .states
                .iter()
                .all(|state| state.path != "/login.html")
        );
    }

    #[test]
    fn discovery_and_map_cli_crawl_live_base_url_same_origin_routes() {
        let site = start_live_discovery_site();
        let temp = tempdir().unwrap();
        let manifest_path = write_live_discovery_manifest(temp.path(), &site.base_url);
        let discovery_dir = temp.path().join("discovery");
        let map_dir = temp.path().join("map");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "discover".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--out".to_string(),
                discovery_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let discovery: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(discovery_dir.join("discovery.json")).unwrap(),
        )
        .unwrap();
        let discovered_routes = discovery["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|surface| surface["route"].as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            discovered_routes,
            BTreeSet::from([
                "/".to_string(),
                "/account".to_string(),
                "/help".to_string(),
                "/settings".to_string()
            ])
        );
        assert!(
            discovery["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|surface| surface["source"] == "base-url-crawl"
                    && surface["confidence"] == "live_http_discovered")
        );
        assert!(discovery["diagnostics"].as_array().unwrap().is_empty());

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "map".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--project-root".to_string(),
                project_root.to_string_lossy().to_string(),
                "--out".to_string(),
                map_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let map: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(map_dir.join("product-map.json")).unwrap())
                .unwrap();
        let map_routes = map["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|surface| surface["routes"].as_array().unwrap())
            .map(|route| route.as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            map_routes,
            BTreeSet::from([
                "/".to_string(),
                "/account".to_string(),
                "/help".to_string(),
                "/settings".to_string()
            ])
        );
        assert!(map["discovery_diagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn discovery_and_map_record_live_base_url_diagnostics_for_unreachable_targets() {
        let temp = tempdir().unwrap();
        let base_url = unused_local_base_url();
        let manifest_path = write_live_discovery_manifest(temp.path(), &base_url);
        let discovery_dir = temp.path().join("discovery");
        let map_dir = temp.path().join("map");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "discover".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--out".to_string(),
                discovery_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let discovery: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(discovery_dir.join("discovery.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(discovery["surfaces"][0]["route"], "/");
        let diagnostics = discovery["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty());
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["source"]
                .as_str()
                .unwrap()
                .starts_with("base-url-crawl:")
                && diagnostic["severity"] == "warning"
        }));

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "map".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--project-root".to_string(),
                project_root.to_string_lossy().to_string(),
                "--out".to_string(),
                map_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let map: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(map_dir.join("product-map.json")).unwrap())
                .unwrap();
        assert!(!map["discovery_diagnostics"].as_array().unwrap().is_empty());
        assert!(
            map["open_questions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|question| question.as_str().unwrap().contains("Discovery diagnostic"))
        );
    }

    #[test]
    fn map_cli_writes_product_map_agent_receipt_and_generated_flow() {
        let temp = tempdir().unwrap();
        let site_dir = temp.path().join("site");
        fs::create_dir_all(&site_dir).unwrap();
        fs::write(
            site_dir.join("index.html"),
            "<!doctype html><html><head><title>Vanity Test</title></head><body><main>hello</main></body></html>",
        )
        .unwrap();
        let manifest_path = write_static_manifest(temp.path(), "login-form");
        let out_dir = temp.path().join("map");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "map".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--project-root".to_string(),
                site_dir.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        assert!(out_dir.join("product-map.json").exists());
        assert!(out_dir.join("surface-map.html").exists());
        assert!(out_dir.join("agent-runner-receipt.json").exists());
        assert!(out_dir.join("generated-flow.yml").exists());
        let map: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(out_dir.join("product-map.json")).unwrap())
                .unwrap();
        assert_eq!(map["schema"], PRODUCT_MAP_SCHEMA);
        assert_eq!(map["agent"]["runner"], "local");
        assert_eq!(map["surfaces"][0]["files"][0], "index.html");
        assert_eq!(map["standards"]["total_obligations"], 55);
        let generated = fs::read_to_string(out_dir.join("generated-flow.yml")).unwrap();
        assert!(generated.contains("allie-generated-product-surface-flow"));
        assert!(generated.contains("promotion_state: generated_candidate"));
    }

    #[test]
    fn report_cli_writes_wcag_drilldown_from_product_map_and_packet() {
        let temp = tempdir().unwrap();
        let site_dir = temp.path().join("site");
        fs::create_dir_all(&site_dir).unwrap();
        fs::write(
            site_dir.join("index.html"),
            "<!doctype html><html><head><title>Vanity Test</title></head><body><main>hello</main></body></html>",
        )
        .unwrap();
        let manifest_path = write_static_manifest(temp.path(), "login-form");
        let map_dir = temp.path().join("map");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_cli_with_io(
            vec![
                "map".to_string(),
                "--manifest".to_string(),
                manifest_path.to_string_lossy().to_string(),
                "--project-root".to_string(),
                site_dir.to_string_lossy().to_string(),
                "--out".to_string(),
                map_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));

        let packet_path = write_failing_evidence_packet(&temp.path().join("run"));
        let report_dir = temp.path().join("report");
        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "report".to_string(),
                "--map".to_string(),
                map_dir
                    .join("product-map.json")
                    .to_string_lossy()
                    .to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                report_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        assert!(report_dir.join("compliance-report.json").exists());
        assert!(report_dir.join("compliance-report.html").exists());
        assert!(report_dir.join("summary.md").exists());
        let report: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(report_dir.join("compliance-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["schema"], COMPLIANCE_REPORT_SCHEMA);
        assert_eq!(report["summary"]["status"], "fail");
        assert_eq!(report["summary"]["total_obligations"], 55);
        assert_eq!(report["summary"]["total_success_criteria"], 55);
        assert_eq!(
            report["criteria"].as_array().unwrap().len(),
            55,
            "wcag22-aa report denominator must be the 55 WCAG success criteria"
        );
        let contrast = report["criteria"]
            .as_array()
            .unwrap()
            .iter()
            .find(|obligation| obligation["id"] == "wcag22-aa:1.4.3-contrast-minimum")
            .unwrap();
        assert_eq!(contrast["status"], "fail");
        assert!(
            contrast["artifact_refs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|artifact| artifact == "screenshot-login-form")
        );
        assert!(
            report["criterion_coverage"].as_array().unwrap().iter().any(
                |cell| cell["criterion_id"] == "wcag22-aa:1.4.3-contrast-minimum"
                    && cell["surface_id"] == "login-form"
                    && cell["state_id"] == "login-form"
                    && cell["status"] == "fail"
                    && cell["method"] == "axe"
                    && cell["evidence_refs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|reference| reference == "login-form-axe-color-contrast-1")
                    && !cell["residual_review_need"].as_str().unwrap().is_empty()
            )
        );
        let html = fs::read_to_string(report_dir.join("compliance-report.html")).unwrap();
        assert!(html.contains("WCAG 2.2 success criteria"));
        assert!(html.contains("Criterion coverage matrix"));
        assert!(html.contains("Reproduce this run"));
        assert!(html.contains("not a legal compliance guarantee"));
    }

    #[test]
    fn vanity_fixture_report_keeps_support_checks_out_of_wcag_denominator() {
        let temp = tempdir().unwrap();
        let report_dir = temp.path().join("report");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "report".to_string(),
                "--map".to_string(),
                "fixtures/vanity-dogfood-legacy-61/product-map.json".to_string(),
                "--packet".to_string(),
                "fixtures/vanity-dogfood-legacy-61/evidence.json".to_string(),
                "--out".to_string(),
                report_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let report: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(report_dir.join("compliance-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["summary"]["total_obligations"], 55);
        assert_eq!(report["summary"]["total_success_criteria"], 55);
        assert_eq!(report["summary"]["total_supporting_checks"], 6);
        assert_eq!(report["criteria"].as_array().unwrap().len(), 55);
        assert_eq!(report["criterion_coverage"].as_array().unwrap().len(), 55);
        let criterion_ids = report["criteria"]
            .as_array()
            .unwrap()
            .iter()
            .map(|criterion| criterion["id"].as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(criterion_ids.len(), 55);
        for support in [
            "wcag22-aa:deterministic-axe-rules",
            "wcag22-aa:2.1.1-keyboard-traversal",
            "wcag22-aa:1.4.10-zoom-reflow",
            "wcag22-aa:2.2.2-reduced-motion",
            "wcag22-aa:human-content-meaning",
            "wcag22-aa:human-assistive-technology-review",
        ] {
            assert!(
                !criterion_ids.contains(support),
                "{support} must not be counted as a WCAG success criterion"
            );
            assert!(
                report["supporting_checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|check| check["id"] == support)
            );
        }
        compliance::validate_criterion_coverage_cells(&report).unwrap();
        let html = fs::read_to_string(report_dir.join("compliance-report.html")).unwrap();
        assert!(html.contains("Supporting checks"));
        assert!(html.contains("wcag22-aa:deterministic-axe-rules"));
        assert!(html.contains("Reproduce this run"));
        assert!(html.contains("not a legal compliance guarantee"));
    }

    #[test]
    fn criterion_coverage_validation_rejects_terminal_status_without_provenance() {
        let mut report = serde_json::json!({
            "criterion_coverage": [
                {
                    "criterion_id": "wcag22-aa:1.1.1-non-text-content",
                    "surface_id": "home",
                    "state_id": "home",
                    "policy_profile": "wcag22-aa",
                    "status": "pass",
                    "applicability": "applicable",
                    "method": "axe",
                    "confidence": "machine_proven",
                    "evidence_refs": [],
                    "agentic_refs": [],
                    "waiver_refs": [],
                    "finding_refs": [],
                    "artifact_refs": [],
                    "test_refs": [],
                    "replay_command": null,
                    "residual_review_need": "No linked evidence was provided."
                }
            ]
        });

        let error = compliance::validate_criterion_coverage_cells(&report).unwrap_err();
        assert!(error.contains("terminal criterion coverage cell lacks provenance"));

        report["criterion_coverage"][0]["replay_command"] =
            serde_json::json!("cargo run --locked -- run --manifest fixture.yml");
        let error = compliance::validate_criterion_coverage_cells(&report).unwrap_err();
        assert!(error.contains("terminal criterion coverage cell lacks provenance"));

        report["criterion_coverage"][0]["evidence_refs"] = serde_json::json!(["axe-home"]);
        compliance::validate_criterion_coverage_cells(&report).unwrap();
    }

    #[test]
    fn coverage_cells_do_not_overstate_unrelated_deterministic_support() {
        let report = build_vanity_fixture_report();
        let human_cell = report
            .criterion_coverage
            .iter()
            .find(|cell| {
                cell.criterion_id == "wcag22-aa:1.2.1-audio-only-and-video-only-prerecorded"
            })
            .unwrap();
        let scripted_cell = report
            .criterion_coverage
            .iter()
            .find(|cell| cell.criterion_id == "wcag22-aa:1.4.10-reflow")
            .unwrap();

        assert_eq!(human_cell.status, "needs_review");
        assert_eq!(human_cell.confidence, "requires_human_or_agent_review");
        assert!(human_cell.artifact_refs.is_empty());
        assert!(human_cell.test_refs.is_empty());
        assert!(
            !human_cell
                .evidence_refs
                .iter()
                .any(|value| value == "axe-core")
        );

        assert_eq!(scripted_cell.status, "not_tested");
        assert_eq!(scripted_cell.confidence, "script_observed");
        assert!(scripted_cell.artifact_refs.is_empty());
        assert!(scripted_cell.test_refs.is_empty());
        assert!(
            !scripted_cell
                .evidence_refs
                .iter()
                .any(|value| value == "axe-home")
        );
    }

    #[test]
    fn uncovered_criterion_cell_renders_not_tested_instead_of_review() {
        let map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let mut packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        packet
            .verdicts
            .retain(|verdict| verdict.obligation != "wcag22-aa:1.4.10-reflow");
        packet
            .findings
            .retain(|finding| finding.standard_obligation != "wcag22-aa:1.4.10-reflow");

        let report = compliance::build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );

        let cell = report
            .criterion_coverage
            .iter()
            .find(|cell| cell.criterion_id == "wcag22-aa:1.4.10-reflow")
            .unwrap();
        let criterion = report
            .criteria
            .iter()
            .find(|criterion| criterion.id == "wcag22-aa:1.4.10-reflow")
            .unwrap();

        assert_eq!(cell.status, "not_tested");
        assert_eq!(criterion.status, "not_tested");
        assert!(report.summary.not_tested > 0);
    }

    #[test]
    fn axe_mapped_review_finding_beats_deterministic_aggregate_pass() {
        let map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let mut packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        packet.findings.push(Finding {
            id: "agentic-non-text-review".to_string(),
            title: "Image alt text needs judgment".to_string(),
            description: "Agent could not confirm non-text alternative usefulness.".to_string(),
            evidence_class: "agentic".to_string(),
            standard_obligation: "wcag22-aa:1.1.1-non-text-content".to_string(),
            severity: "review".to_string(),
            status: "needs_review".to_string(),
            confidence: "agent_inferred".to_string(),
            source: "offline-agentic-review".to_string(),
            affected_route: "/".to_string(),
            affected_state: "home".to_string(),
            artifact_refs: vec!["screenshot-home".to_string()],
            replay_command: packet.replay.command.clone(),
        });

        let report = compliance::build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );

        let cell = report
            .criterion_coverage
            .iter()
            .find(|cell| cell.criterion_id == "wcag22-aa:1.1.1-non-text-content")
            .unwrap();
        let criterion = report
            .criteria
            .iter()
            .find(|criterion| criterion.id == "wcag22-aa:1.1.1-non-text-content")
            .unwrap();
        assert_eq!(cell.status, "needs_review");
        assert_eq!(cell.confidence, "agent_inferred");
        assert_eq!(criterion.status, "needs_review");
        assert_eq!(criterion.confidence, "agent_inferred");
    }

    #[test]
    fn waiver_inputs_reach_coverage_cells_and_summary() {
        let map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let mut packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        packet.waivers = vec![
            serde_json::json!({
                "id": "waiver-non-text",
                "surface": "home",
                "criterion_id": "wcag22-aa:1.1.1-non-text-content",
                "status": "waived",
                "provenance": {"actor": "accessibility-lead", "reason": "fixture"},
                "expires_at": "2026-07-20T00:00:00Z",
                "packet_ref": "vanity-legacy-61"
            }),
            serde_json::json!({
                "id": "risk-contrast",
                "surface": "home",
                "criterion_id": "wcag22-aa:1.4.3-contrast-minimum",
                "status": "risk_accepted",
                "provenance": {"actor": "accessibility-lead", "reason": "fixture"},
                "expires_at": "2026-07-20T00:00:00Z",
                "packet_ref": "vanity-legacy-61"
            }),
        ];

        let report = compliance::build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );
        let report_value = serde_json::to_value(&report).unwrap();

        compliance::validate_criterion_coverage_cells(&report_value).unwrap();
        assert_eq!(report.summary.status, "needs_review");
        assert_eq!(report.summary.waived, 1);
        assert_eq!(report.summary.risk_accepted, 1);
        assert_eq!(report.surfaces[0].status, "needs_review");
        assert!(report.criterion_coverage.iter().any(|cell| {
            cell.criterion_id == "wcag22-aa:1.1.1-non-text-content"
                && cell.status == "waived"
                && cell.waiver_refs == ["waiver-non-text"]
        }));
        assert!(report.criterion_coverage.iter().any(|cell| {
            cell.criterion_id == "wcag22-aa:1.4.3-contrast-minimum"
                && cell.status == "risk_accepted"
                && cell.waiver_refs == ["risk-contrast"]
        }));
    }

    #[test]
    fn coverage_matrix_validates_all_surface_state_pairs_and_unique_keys() {
        let mut map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let mut packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        map.surfaces.push(ProductSurface {
            id: "settings".to_string(),
            title: "Vanity settings".to_string(),
            routes: vec!["/settings".to_string()],
            files: vec!["app/settings/page.tsx".to_string()],
            services: vec!["web".to_string()],
            user_stories: vec!["As a reader, I can open Vanity settings.".to_string()],
            workflow_refs: vec!["vanity-settings-flow".to_string()],
            evidence_refs: vec!["settings".to_string()],
            confidence: "operator_supplied".to_string(),
            review_status: "required".to_string(),
            provenance: vec!["test".to_string()],
        });
        packet.coverage.states_captured.push("settings".to_string());
        packet.coverage.state_metadata.push(StateMetadata {
            id: "settings".to_string(),
            route: "/settings".to_string(),
            url: "http://127.0.0.1:4174/settings".to_string(),
            title: "Vanity Settings".to_string(),
            http_status: Some(200),
            keyboard_focus_order: Vec::new(),
            console_errors: Vec::new(),
            network_errors: Vec::new(),
            state_errors: Vec::new(),
            features: None,
        });

        let report = compliance::build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );
        let report_value = serde_json::to_value(&report).unwrap();

        assert_eq!(report.criteria.len(), 55);
        assert_eq!(report.criterion_coverage.len(), 110);
        compliance::validate_criterion_coverage_cells(&report_value).unwrap();

        let mut duplicate = report_value.clone();
        let first_cell = duplicate["criterion_coverage"][0].clone();
        duplicate["criterion_coverage"]
            .as_array_mut()
            .unwrap()
            .push(first_cell);
        let error = compliance::validate_criterion_coverage_cells(&duplicate).unwrap_err();
        assert!(error.contains("duplicate criterion coverage cell"));

        let mut missing = report_value;
        missing["criterion_coverage"].as_array_mut().unwrap().pop();
        let error = compliance::validate_criterion_coverage_cells(&missing).unwrap_err();
        assert!(error.contains("missing criterion coverage cell"));
    }

    #[test]
    fn compliance_summary_surfaces_waived_and_risk_accepted_counts() {
        let packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        let criteria = vec![
            sample_compliance_obligation("wcag22-aa:1.1.1-non-text-content", "waived"),
            sample_compliance_obligation("wcag22-aa:1.4.3-contrast-minimum", "risk_accepted"),
        ];

        let summary = compliance::compliance_summary(&packet, &criteria, 0);

        assert_eq!(summary.status, "needs_review");
        assert_eq!(summary.total_obligations, 2);
        assert_eq!(summary.waived, 1);
        assert_eq!(summary.risk_accepted, 1);
        assert_eq!(summary.pass, 0);
    }

    #[test]
    fn surface_report_falls_back_to_criteria_when_matrix_is_absent() {
        let packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        let surface = ProductSurface {
            id: "home".to_string(),
            title: "Home".to_string(),
            routes: vec!["/".to_string()],
            files: Vec::new(),
            services: Vec::new(),
            user_stories: Vec::new(),
            workflow_refs: Vec::new(),
            evidence_refs: vec!["home".to_string()],
            confidence: "operator_supplied".to_string(),
            review_status: "required".to_string(),
            provenance: Vec::new(),
        };
        let criteria = vec![sample_compliance_obligation(
            "custom-policy:required-home-check",
            "fail",
        )];

        let report = compliance::compliance_surface_report(&surface, &packet, &criteria, &[]);

        assert_eq!(report.status, "fail");
        assert_eq!(
            report.criteria,
            vec!["custom-policy:required-home-check".to_string()]
        );
        assert!(report.cells.is_empty());
    }

    #[test]
    fn workbench_start_writes_durable_job_lifecycle() {
        let _guard = WORKBENCH_CLI_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "start".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 1, "stderr={}", String::from_utf8_lossy(&stderr));
        let job_path = job_dir.join("job.json");
        let events_path = job_dir.join("events.jsonl");
        assert!(job_path.exists());
        assert!(events_path.exists());
        assert!(job_dir.join("steps/discovery/discovery.json").exists());
        assert!(job_dir.join("steps/map/product-map.json").exists());
        assert!(job_dir.join("steps/run/evidence.json").exists());
        assert!(job_dir.join("steps/report/compliance-report.json").exists());
        assert!(
            !job_dir.join("steps/review").exists(),
            "model-off workbench review must not write an offline review artifact directory"
        );
        assert!(!job_dir.join("steps/remediation").exists());
        assert!(job_dir.join("steps/release/release-summary.json").exists());

        let job: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(job_path).unwrap()).unwrap();
        assert_eq!(job["schema"], "allie.job.v0");
        assert_eq!(job["status"], "blocked");
        assert_eq!(job["current_step"], "finished");
        assert_eq!(
            job["runtime_policy"]["agent_step_timeout_ms"],
            serde_json::Value::Null
        );
        assert_eq!(job["runner"]["kind"], "local");
        assert!(job["resumable"].as_bool().unwrap());
        assert_eq!(job["pointers"]["product_map"], "steps/map/product-map.json");
        assert_eq!(
            job["pointers"]["compliance_report"],
            "steps/report/compliance-report.json"
        );
        assert_eq!(
            job["pointers"]["reviewed_packet"],
            "steps/run/evidence.json"
        );
        assert_eq!(
            job["pointers"]["release_summary"],
            "steps/release/release-summary.json"
        );

        let evidence: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(job_dir.join("steps/run/evidence.json")).unwrap(),
        )
        .unwrap();
        assert!(
            evidence.get("review").is_none(),
            "model-off workbench review must not fabricate a review attempt"
        );
        assert!(
            !evidence["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["evidence_class"] == "agentic"),
            "model-off workbench review must not fabricate an agentic finding"
        );

        let events = fs::read_to_string(events_path).unwrap();
        assert!(events.contains("\"event\":\"job_started\""));
        assert!(events.contains("\"event\":\"step_completed\""));
        assert!(events.contains("\"step\":\"map\""));
        assert!(!events.contains("\"step\":\"remediation\""));
        assert!(events.contains("\"event\":\"job_finished\""));
    }

    #[test]
    fn workbench_status_cancel_and_resume_are_auditable() {
        let _guard = WORKBENCH_CLI_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "start".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 1, "stderr={}", String::from_utf8_lossy(&stderr));

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "status".to_string(),
                "--job".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let output = String::from_utf8(stdout.clone()).unwrap();
        assert!(output.contains("Status: blocked"));
        assert!(output.contains("Current step: finished"));
        assert!(output.contains("Resumable: true"));

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "cancel".to_string(),
                "--job".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let cancelled: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(job_dir.join("job.json")).unwrap()).unwrap();
        assert_eq!(cancelled["status"], "cancelled");
        assert_eq!(cancelled["cancel_requested"], true);

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "resume".to_string(),
                "--job".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 1, "stderr={}", String::from_utf8_lossy(&stderr));
        let resumed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(job_dir.join("job.json")).unwrap()).unwrap();
        assert_eq!(resumed["status"], "blocked");
        assert_eq!(resumed["cancel_requested"], false);
        assert_eq!(resumed["resume_count"], 1);

        let events = fs::read_to_string(job_dir.join("events.jsonl")).unwrap();
        assert!(events.contains("\"event\":\"job_cancel_requested\""));
        assert!(events.contains("\"event\":\"job_resumed\""));
        assert!(events.matches("\"event\":\"step_started\"").count() >= 12);
    }

    #[test]
    fn workbench_start_rejects_existing_durable_job_directory() {
        let _guard = WORKBENCH_CLI_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "start".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 1, "stderr={}", String::from_utf8_lossy(&stderr));

        stdout.clear();
        stderr.clear();
        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "start".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                job_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(String::from_utf8_lossy(&stderr).contains("already contains durable state"));
    }

    #[test]
    fn workbench_start_rejects_non_local_agent_mode_until_durable_adapter_exists() {
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "workbench".to_string(),
                "start".to_string(),
                "--manifest".to_string(),
                "examples/autonomous-workbench.yml".to_string(),
                "--out".to_string(),
                job_dir.to_string_lossy().to_string(),
                "--agent".to_string(),
                "opencode".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 64);
        assert!(String::from_utf8_lossy(&stderr).contains("support only --agent local"));
        assert!(!job_dir.exists());
    }

    #[test]
    fn review_is_not_a_dispatchable_command() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(vec!["review".to_string()], &mut stdout, &mut stderr);

        assert_eq!(code, ExitClass::Usage.code());
        assert!(String::from_utf8_lossy(&stderr).contains("unknown command"));
    }

    #[test]
    fn remediation_cli_is_not_part_of_allie() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(vec!["remediate".to_string()], &mut stdout, &mut stderr);

        assert_eq!(code, ExitClass::Usage.code());
        assert!(String::from_utf8_lossy(&stderr).contains("unknown command"));
    }

    #[test]
    fn release_cli_writes_neutral_check_for_residual_review_packet() {
        let temp = tempdir().unwrap();
        let packet_path = write_passing_evidence_packet(&temp.path().join("run"));
        let out_dir = temp.path().join("release");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "release".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
                "--changed-surface".to_string(),
                "login-form".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("Status: needs_review"));
        assert!(out_dir.join("release-summary.json").exists());
        assert!(out_dir.join("github-check.json").exists());
        assert!(out_dir.join("release-report.html").exists());

        let check = fs::read_to_string(out_dir.join("github-check.json")).unwrap();
        let check: serde_json::Value = serde_json::from_str(&check).unwrap();
        assert_eq!(check["conclusion"], "neutral");
        assert!(
            check["output"]["summary"]
                .as_str()
                .unwrap()
                .contains("status=needs_review")
        );

        let report = fs::read_to_string(out_dir.join("release-report.html")).unwrap();
        assert!(report.contains("Allie release decision: needs_review"));
        assert!(report.contains("not a legal compliance guarantee"));
    }

    #[test]
    fn release_cli_rejects_invalid_packet_status_before_projection() {
        let temp = tempdir().unwrap();
        let source_packet_path = write_passing_evidence_packet(&temp.path().join("run"));
        let mut packet: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(source_packet_path).unwrap()).unwrap();
        packet["summary"]["status"] = serde_json::json!("approved");
        let packet_path = temp.path().join("invalid-evidence.json");
        let out_dir = temp.path().join("release");
        write_json_pretty(&packet_path, &packet).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "release".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
                "--changed-surface".to_string(),
                "login-form".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(String::from_utf8(stdout).unwrap().is_empty());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("invalid evidence packet status approved"));
        assert!(!out_dir.join("release-summary.json").exists());
        assert!(!out_dir.join("github-check.json").exists());
    }

    #[test]
    fn release_cli_rejects_invalid_packet_schema_before_projection() {
        let temp = tempdir().unwrap();
        let source_packet_path = write_passing_evidence_packet(&temp.path().join("run"));
        let mut packet: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(source_packet_path).unwrap()).unwrap();
        packet["schema"] = serde_json::json!("allie.evidence.future");
        let packet_path = temp.path().join("wrong-schema-evidence.json");
        let out_dir = temp.path().join("release");
        write_json_pretty(&packet_path, &packet).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "release".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(String::from_utf8(stdout).unwrap().is_empty());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("invalid evidence packet schema allie.evidence.future"));
        assert!(!out_dir.join("release-summary.json").exists());
    }

    #[test]
    fn release_cli_rejects_schema_unknown_fields_before_projection() {
        let temp = tempdir().unwrap();
        let source_packet_path = write_passing_evidence_packet(&temp.path().join("run"));
        let mut packet: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(source_packet_path).unwrap()).unwrap();
        packet["summary"]["unexpected"] = serde_json::json!(true);
        let packet_path = temp.path().join("unknown-field-evidence.json");
        let out_dir = temp.path().join("release");
        write_json_pretty(&packet_path, &packet).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "release".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(String::from_utf8(stdout).unwrap().is_empty());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("unknown field"));
        assert!(!out_dir.join("release-summary.json").exists());
    }

    #[test]
    fn release_projection_blocks_packet_failures() {
        let mut packet = minimal_release_packet();
        packet["summary"]["status"] = serde_json::json!("fail");
        packet["summary"]["deterministic_failures"] = serde_json::json!(1);

        let projection = project_release_value(&packet, &release_options(vec![]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary.status, "blocked");
        assert_eq!(projection.github_check.conclusion, "failure");
    }

    #[test]
    fn release_projection_does_not_block_model_only_findings() {
        let mut packet = minimal_release_packet();
        packet["findings"] = serde_json::json!([
            {
                "id": "agentic-1",
                "title": "Possible label ambiguity",
                "description": "Agentic review reported possible ambiguity.",
                "evidence_class": "agentic",
                "standard_obligation": "wcag22-aa:3.3.2-labels-or-instructions",
                "severity": "moderate",
                "status": "needs_review",
                "confidence": "agent_inferred"
                ,"source": "allie-agentic-review"
                ,"affected_route": "/"
                ,"affected_state": "login-form"
                ,"artifact_refs": []
                ,"replay_command": "cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest"
            }
        ]);

        let projection = project_release_value(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::Success);
        assert_eq!(projection.summary.status, "needs_review");
        assert_eq!(projection.github_check.conclusion, "neutral");
        assert_eq!(projection.summary.model_findings_non_blocking, 1);
    }

    #[test]
    fn release_projection_blocks_missing_changed_surface_evidence() {
        let packet = minimal_release_packet();

        let projection = project_release_value(&packet, &release_options(vec!["settings"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary.status, "blocked");
        assert_eq!(
            projection.summary.blocking.missing_required_evidence[0],
            "settings"
        );
    }

    #[test]
    fn release_projection_blocks_expired_touched_waivers() {
        let mut packet = minimal_release_packet();
        packet["waivers"] = serde_json::json!([
            {
                "id": "waiver-1",
                "surface": "login-form",
                "status": "risk_accepted",
                "provenance": {"actor": "accessibility-lead"},
                "expires_at": (Utc::now() - chrono::Duration::days(1)).to_rfc3339(),
                "packet_ref": "run-release"
            }
        ]);

        let projection = project_release_value(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary.status, "blocked");
        assert_eq!(
            projection.summary.blocking.expired_waivers[0]["id"],
            "waiver-1"
        );
    }

    #[test]
    fn release_projection_blocks_invalid_touched_waivers() {
        let mut packet = minimal_release_packet();
        packet["waivers"] = serde_json::json!([
            {
                "id": "waiver-2",
                "surface": "login-form",
                "status": "waived",
                "expires_at": (Utc::now() + chrono::Duration::days(3)).to_rfc3339()
            }
        ]);

        let projection = project_release_value(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary.status, "blocked");
        assert_eq!(
            projection.summary.blocking.invalid_waivers[0]["id"],
            "waiver-2"
        );
    }

    #[test]
    fn release_projection_routes_stale_evidence_to_review() {
        let mut packet = minimal_release_packet();
        packet["run"]["finished_at"] =
            serde_json::json!((Utc::now() - chrono::Duration::days(30)).to_rfc3339());

        let projection = project_release_value(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::Success);
        assert_eq!(projection.summary.status, "needs_review");
        assert_eq!(projection.github_check.conclusion, "neutral");
        assert!(projection.summary.review.stale_evidence);
    }

    fn release_options(changed_surfaces: Vec<&str>) -> ReleaseOptions {
        ReleaseOptions {
            packet_path: PathBuf::from("evidence.json"),
            out_dir: PathBuf::from("release"),
            changed_surfaces: changed_surfaces
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            stale_after_days: 7,
        }
    }

    fn project_release_value(
        packet: &serde_json::Value,
        options: &ReleaseOptions,
    ) -> release::ReleaseProjection {
        let packet: EvidencePacket = serde_json::from_value(packet.clone()).unwrap();
        release::project_release_decision(&packet, options)
    }

    fn minimal_release_packet() -> serde_json::Value {
        serde_json::json!({
            "schema": "allie.evidence.v0",
            "summary": {
                "status": "pass",
                "exit_code": 0,
                "deterministic_failures": 0,
                "scripted_failures": 0,
                "infrastructure_failures": 0,
                "states_captured": 1,
                "failure_class": null
            },
            "run": {
                "id": "run-release",
                "started_at": Utc::now().to_rfc3339(),
                "finished_at": Utc::now().to_rfc3339(),
                "allie_version": "0.1.0",
                "git_sha": "test-sha",
                "git_branch": "test-branch",
                "ci_provider": null,
                "actor": "test"
            },
            "target": {
                "base_url": "http://127.0.0.1:49152",
                "environment": "test",
                "app_name": "Allie Fixture",
                "auth_profile": "none",
                "credential_provider": {
                    "provider": "none",
                    "env": null,
                    "required": false,
                    "status": "not_required"
                },
                "flow_manifest": "examples/login-flow.yml"
            },
            "policy": {
                "profile": "wcag22-aa",
                "blocking_classes": ["deterministic"],
                "worker_timeout_ms": 30000,
                "model_provider_allowlist": [],
                "model_status": "disabled",
                "zdr_required": true,
                "model_egress_redaction": null,
                "redaction_profile": "not_redacted_local_fixture",
                "budget": {
                    "model_calls": 0,
                    "max_states": 1
                }
            },
            "coverage": {
                "routes_visited": ["/"],
                "states_captured": ["login-form"],
                "surfaces_discovered": ["Allie Fixture"],
                "flows_exercised": ["login-flow"],
                "state_metadata": [],
                "standards_obligations_evaluated": [],
                "obligations_not_tested": [],
                "profile_human_review_scope": []
            },
            "artifacts": [
                {"id":"axe-json-login-form","type":"axe_json","path":"artifacts/axe-login-form.json","hash":"sha256:test","redaction_status":"not_redacted_local_fixture","retention_class":"local_ephemeral","unavailable_reason":null,"related_flow_state":"login-form","creation_tool":"allie-release-test-fixture","timestamp":Utc::now().to_rfc3339()},
                {"id":"screenshot-login-form","type":"screenshot","path":"artifacts/login-form.png","hash":"sha256:test","redaction_status":"not_redacted_local_fixture","retention_class":"local_ephemeral","unavailable_reason":null,"related_flow_state":"login-form","creation_tool":"allie-release-test-fixture","timestamp":Utc::now().to_rfc3339()},
                {"id":"report-html","type":"html_report","path":"report.html","hash":"sha256:test","redaction_status":"not_redacted_local_fixture","retention_class":"local_ephemeral","unavailable_reason":null,"related_flow_state":null,"creation_tool":"allie-report-writer","timestamp":Utc::now().to_rfc3339()}
            ],
            "findings": [],
            "verdicts": [],
            "waivers": [],
            "agentic_assessments": [],
            "replay": {
                "command": "cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest",
                "manifest_path": "examples/login-flow.yml",
                "environment_requirements": [],
                "credential_profile": "none",
                "browser": {
                    "viewport": {"width": 1280, "height": 720},
                    "color_scheme": "light",
                    "reduced_motion": "reduce",
                    "locale": "en-US",
                    "zoom": 1.0
                },
                "seed_data": [],
                "known_nondeterminism": []
            }
        })
    }

    fn write_static_manifest(root: &Path, state_id: &str) -> PathBuf {
        let manifest_path = root.join("flow.yml");
        fs::write(
            &manifest_path,
            format!(
                r#"id: vanity-static-flow
name: Vanity static flow
app_name: Vanity
environment: local
target:
  kind: web
  base_url: http://127.0.0.1:4174
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
browser:
  viewport:
    width: 1280
    height: 720
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: vanity-home-flow
  description: Vanity homepage
  states:
    - id: {state_id}
      path: /
      description: Vanity homepage
      required: true
      axe: true
      screenshot: true
      dom_snapshot: true
      accessibility_tree: true
      keyboard: true
      video: false
      trace: true
"#
            ),
        )
        .unwrap();
        manifest_path
    }

    fn sample_compliance_obligation(id: &str, status: &str) -> ComplianceObligation {
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

    fn build_vanity_fixture_report() -> ComplianceReportPacket {
        let map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        compliance::build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        )
    }

    fn write_passing_evidence_packet(out_dir: &Path) -> PathBuf {
        let artifacts_dir = out_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        fs::write(
            artifacts_dir.join("axe-login-form.json"),
            br#"{"violations":[]}"#,
        )
        .unwrap();
        fs::write(artifacts_dir.join("login-form.png"), b"fake-png").unwrap();

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            out_dir,
            passing_worker_response(),
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-release-cli".to_string(),
            &test_provenance(),
        )
        .unwrap()
        .evidence_path
    }

    fn write_failing_evidence_packet(out_dir: &Path) -> PathBuf {
        let artifacts_dir = out_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        fs::write(
            artifacts_dir.join("axe-login-form.json"),
            br#"{"violations":[{"id":"color-contrast"}]}"#,
        )
        .unwrap();
        fs::write(artifacts_dir.join("login-form.png"), b"fake-png").unwrap();

        let manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        let mut response = passing_worker_response();
        response.status = WorkerRunStatus::Failed;
        response.states[0].axe_violations.push(AxeViolation {
            id: "color-contrast".to_string(),
            impact: Some("serious".to_string()),
            help: Some("Elements must meet minimum color contrast ratio thresholds".to_string()),
            description: Some("axe reported contrast failure".to_string()),
            tags: vec!["wcag143".to_string()],
            nodes: 1,
        });
        write_packet_and_report(
            &manifest,
            Path::new("examples/login-flow.yml"),
            out_dir,
            response,
            Vec::new(),
            Utc::now(),
            Utc::now(),
            "run-failing-evidence".to_string(),
            &test_provenance(),
        )
        .unwrap()
        .evidence_path
    }

    fn test_provenance() -> GitProvenance {
        GitProvenance::read(Path::new(".")).unwrap()
    }

    fn passing_worker_response() -> WorkerResponse {
        WorkerResponse {
            schema: worker::response_schema().to_string(),
            status: WorkerRunStatus::Passed,
            actual_base_url: Some("http://127.0.0.1:49152".to_string()),
            states: vec![WorkerStateResult {
                id: "login-form".to_string(),
                route: "/".to_string(),
                url: "http://127.0.0.1:49152/".to_string(),
                title: "Allie Fixture Login".to_string(),
                http_status: Some(200),
                screenshot_path: Some("artifacts/login-form.png".to_string()),
                axe_json_path: Some("artifacts/axe-login-form.json".to_string()),
                mobile_screenshot_path: None,
                mobile_axe_json_path: None,
                dom_snapshot_path: None,
                accessibility_tree_path: None,
                video_path: None,
                trace_path: None,
                keyboard_focus_order: Vec::new(),
                axe_violations: Vec::new(),
                axe_passes: Vec::new(),
                console_errors: Vec::new(),
                network_errors: Vec::new(),
                state_errors: Vec::new(),
                features: Some(PageFeatures {
                    forms: 1,
                    inputs: 2,
                    links: 1,
                    headings: 1,
                    lang: true,
                    lang_value: "en".to_string(),
                    reflow_checked: true,
                    reflow_overflow: false,
                    mobile_viewport_checked: true,
                    mobile_viewport_width: 390,
                    mobile_viewport_height: 844,
                    ..Default::default()
                }),
            }],
            errors: Vec::new(),
            nondeterminism: Vec::new(),
        }
    }
}

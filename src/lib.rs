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
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wait_timeout::ChildExt;

const PRODUCT_LINE: &str = "Allie: accessibility evidence for every release.";
const NEXT_STEP: &str = "Next implementation target: allie run --manifest <flow.yml>";
const EVIDENCE_SCHEMA: &str = "allie.evidence.v0";
const WORKER_REQUEST_SCHEMA: &str = "allie.worker.request.v0";
const WORKER_RESPONSE_SCHEMA: &str = "allie.worker.response.v0";
const PRODUCT_MAP_SCHEMA: &str = "allie.product-map.v0";
const COMPLIANCE_REPORT_SCHEMA: &str = "allie.compliance-report.v0";
const JOB_SCHEMA: &str = "allie.job.v0";
const DEFAULT_WORKER_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_AGENT_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_WORKBENCH_MAX_RUNTIME_MS: u64 = 24 * 60 * 60 * 1000;
const DEFAULT_WORKBENCH_IDLE_TIMEOUT_MS: u64 = 10 * 60 * 1000;
const WCAG22_AA_PROFILE_JSON: &str = include_str!("../profiles/wcag22-aa.json");

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
    Worker(String),
}

impl Display for AllieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Json { context, source } => write!(f, "{context}: {source}"),
            Self::Yaml { context, source } => write!(f, "{context}: {source}"),
            Self::InvalidManifest(message) => write!(f, "invalid manifest: {message}"),
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
            Self::InvalidManifest(_) | Self::Worker(_) => None,
        }
    }
}

type Result<T> = std::result::Result<T, AllieError>;

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
}

#[derive(Debug)]
struct ReleaseOptions {
    packet_path: PathBuf,
    out_dir: PathBuf,
    changed_surfaces: Vec<String>,
    stale_after_days: i64,
}

#[derive(Debug)]
struct DiscoveryOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug)]
struct PromoteFlowOptions {
    discovery_path: PathBuf,
    flow_plan_path: PathBuf,
    out_path: PathBuf,
}

#[derive(Debug)]
struct ReviewOptions {
    packet_path: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug)]
struct RemediateOptions {
    packet_path: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug)]
struct MapOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
    project_root: PathBuf,
    agent_runner: AgentRunnerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentRunnerKind {
    Local,
    OpenCode,
    Omp,
}

impl AgentRunnerKind {
    fn parse(value: &str) -> std::result::Result<Self, String> {
        match value {
            "local" => Ok(Self::Local),
            "opencode" => Ok(Self::OpenCode),
            "omp" => Ok(Self::Omp),
            unexpected => Err(format!(
                "unsupported agent runner {unexpected}; expected local, opencode, or omp"
            )),
        }
    }

    fn as_str(self) -> &'static str {
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
struct WorkbenchStartOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
    project_root: Option<PathBuf>,
    agent_runner: AgentRunnerKind,
}

#[derive(Debug)]
enum WorkbenchCommand {
    Start(WorkbenchStartOptions),
    Status { job_dir: PathBuf },
    Cancel { job_dir: PathBuf },
    Resume { job_dir: PathBuf },
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
struct DiscoveryReceipt {
    discovery_path: PathBuf,
    flow_plan_path: PathBuf,
    report_path: PathBuf,
}

#[derive(Debug)]
struct PromoteFlowReceipt {
    manifest_path: PathBuf,
}

#[derive(Debug)]
struct ReviewReceipt {
    packet_path: PathBuf,
    report_path: PathBuf,
}

#[derive(Debug)]
struct RemediationReceipt {
    queue_path: PathBuf,
    ledger_path: PathBuf,
    report_path: PathBuf,
    patch_plan_path: PathBuf,
}

#[derive(Debug)]
struct MapReceipt {
    map_path: PathBuf,
    report_path: PathBuf,
    runner_receipt_path: PathBuf,
    flow_manifest_path: PathBuf,
}

#[derive(Debug)]
struct ComplianceReportReceipt {
    report_json_path: PathBuf,
    report_html_path: PathBuf,
    summary_path: PathBuf,
}

#[derive(Debug)]
struct WorkbenchReceipt {
    job_path: PathBuf,
    events_path: PathBuf,
    status: String,
    current_step: String,
    resumable: bool,
    exit_class: ExitClass,
}

pub fn run_cli(args: impl IntoIterator<Item = String>) -> i32 {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    run_cli_with_io(args, &mut stdout, &mut stderr)
}

pub fn run_cli_with_io(
    args: impl IntoIterator<Item = String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let args = args.into_iter().collect::<Vec<_>>();

    if args.is_empty() {
        let _ = writeln!(stdout, "{PRODUCT_LINE}");
        let _ = writeln!(stdout, "{NEXT_STEP}");
        let _ = writeln!(
            stdout,
            "Run: allie run --manifest examples/login-flow.yml --out .allie/runs/latest"
        );
        return ExitClass::Success.code();
    }

    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        print_usage(stdout);
        return ExitClass::Success.code();
    }

    match args.first().map(String::as_str) {
        Some("run") => match parse_run_options(&args[1..]) {
            Ok(options) => match run_v0(options) {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "Allie evidence run: {}", receipt.run_id);
                    let _ = writeln!(stdout, "Evidence: {}", receipt.evidence_path.display());
                    let _ = writeln!(stdout, "Report: {}", receipt.report_path.display());
                    let _ = writeln!(stdout, "Status: {}", receipt.exit_class.packet_status());
                    receipt.exit_class.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("discover") => match parse_discovery_options(&args[1..]) {
            Ok(options) => match run_discovery(options) {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "Discovery: {}", receipt.discovery_path.display());
                    let _ = writeln!(stdout, "Flow plan: {}", receipt.flow_plan_path.display());
                    let _ = writeln!(stdout, "Report: {}", receipt.report_path.display());
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("promote-flow") => match parse_promote_flow_options(&args[1..]) {
            Ok(options) => match run_promote_flow(options) {
                Ok(receipt) => {
                    let _ = writeln!(
                        stdout,
                        "Generated manifest: {}",
                        receipt.manifest_path.display()
                    );
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("map") => match parse_map_options(&args[1..]) {
            Ok(options) => match run_map(options) {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "Product map: {}", receipt.map_path.display());
                    let _ = writeln!(stdout, "Surface map: {}", receipt.report_path.display());
                    let _ = writeln!(
                        stdout,
                        "Agent receipt: {}",
                        receipt.runner_receipt_path.display()
                    );
                    let _ = writeln!(
                        stdout,
                        "Generated flow: {}",
                        receipt.flow_manifest_path.display()
                    );
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("report") => match parse_report_options(&args[1..]) {
            Ok(options) => match run_compliance_report(options) {
                Ok(receipt) => {
                    let _ = writeln!(
                        stdout,
                        "Compliance JSON: {}",
                        receipt.report_json_path.display()
                    );
                    let _ = writeln!(
                        stdout,
                        "Compliance report: {}",
                        receipt.report_html_path.display()
                    );
                    let _ = writeln!(stdout, "Summary: {}", receipt.summary_path.display());
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("workbench") => match parse_workbench_command(&args[1..]) {
            Ok(command) => match run_workbench(command) {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "Workbench job: {}", receipt.job_path.display());
                    let _ = writeln!(stdout, "Events: {}", receipt.events_path.display());
                    let _ = writeln!(stdout, "Status: {}", receipt.status);
                    let _ = writeln!(stdout, "Current step: {}", receipt.current_step);
                    let _ = writeln!(stdout, "Resumable: {}", receipt.resumable);
                    receipt.exit_class.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("review") => match parse_review_options(&args[1..]) {
            Ok(options) => match run_review(options) {
                Ok(receipt) => {
                    let _ = writeln!(stdout, "Reviewed packet: {}", receipt.packet_path.display());
                    let _ = writeln!(stdout, "Review report: {}", receipt.report_path.display());
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("remediate") => match parse_remediate_options(&args[1..]) {
            Ok(options) => match run_remediate(options) {
                Ok(receipt) => {
                    let _ = writeln!(
                        stdout,
                        "Remediation queue: {}",
                        receipt.queue_path.display()
                    );
                    let _ = writeln!(stdout, "Action ledger: {}", receipt.ledger_path.display());
                    let _ = writeln!(stdout, "Report: {}", receipt.report_path.display());
                    let _ = writeln!(stdout, "Patch plan: {}", receipt.patch_plan_path.display());
                    ExitClass::Success.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        Some("release") => match parse_release_options(&args[1..]) {
            Ok(options) => match run_release(options) {
                Ok(receipt) => {
                    let _ = writeln!(
                        stdout,
                        "Release summary: {}",
                        receipt.summary_path.display()
                    );
                    let _ = writeln!(stdout, "GitHub check: {}", receipt.check_path.display());
                    let _ = writeln!(stdout, "Release report: {}", receipt.report_path.display());
                    let _ = writeln!(stdout, "Status: {}", receipt.status);
                    receipt.exit_class.code()
                }
                Err(error) => {
                    let _ = writeln!(stderr, "allie: {error}");
                    ExitClass::InfrastructureFailure.code()
                }
            },
            Err(error) => {
                let _ = writeln!(stderr, "allie: {error}");
                print_usage(stderr);
                ExitClass::Usage.code()
            }
        },
        _ => {
            let _ = writeln!(stderr, "allie: unknown command");
            print_usage(stderr);
            ExitClass::Usage.code()
        }
    }
}

fn print_usage(writer: &mut dyn Write) {
    let _ = writeln!(
        writer,
        "Usage:\n  allie run --manifest <flow.yml> --out <output-dir>\n  allie discover --manifest <flow.yml> --out <output-dir>\n  allie promote-flow --discovery <discovery.json> --flow-plan <flow-plan.json> --out <flow.yml>\n  allie map --manifest <flow.yml> --out <output-dir> [--project-root <dir>] [--agent local|opencode|omp]\n  allie report --map <product-map.json> --packet <evidence.json> --out <output-dir>\n  allie workbench start --manifest <flow.yml> --out <job-dir> [--project-root <dir>]\n  allie workbench status --job <job-dir>\n  allie workbench cancel --job <job-dir>\n  allie workbench resume --job <job-dir>\n  allie review --packet <evidence.json> --out <output-dir>\n  allie remediate --packet <evidence.json> --out <output-dir>\n  allie release --packet <evidence.json> --out <output-dir> [--changed-surface <id>] [--stale-after-days <days>]"
    );
}

fn parse_run_options(args: &[String]) -> std::result::Result<RunOptions, String> {
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

    Ok(RunOptions {
        manifest_path: manifest_path.ok_or_else(|| "--manifest is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
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

fn parse_workbench_command(args: &[String]) -> std::result::Result<WorkbenchCommand, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err("workbench requires start, status, cancel, or resume".to_string());
    };
    match command {
        "start" => parse_workbench_start_options(&args[1..]).map(WorkbenchCommand::Start),
        "status" => {
            parse_workbench_job_dir(&args[1..]).map(|job_dir| WorkbenchCommand::Status { job_dir })
        }
        "cancel" => {
            parse_workbench_job_dir(&args[1..]).map(|job_dir| WorkbenchCommand::Cancel { job_dir })
        }
        "resume" => {
            parse_workbench_job_dir(&args[1..]).map(|job_dir| WorkbenchCommand::Resume { job_dir })
        }
        unexpected => Err(format!(
            "unsupported workbench command {unexpected}; expected start, status, cancel, or resume"
        )),
    }
}

fn parse_workbench_start_options(
    args: &[String],
) -> std::result::Result<WorkbenchStartOptions, String> {
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
                if agent_runner != AgentRunnerKind::Local {
                    return Err(
                        "workbench jobs currently support only --agent local; use allie map --agent for one-shot advisory agent runners"
                            .to_string(),
                    );
                }
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(WorkbenchStartOptions {
        manifest_path: manifest_path.ok_or_else(|| "--manifest is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
        project_root,
        agent_runner,
    })
}

fn parse_workbench_job_dir(args: &[String]) -> std::result::Result<PathBuf, String> {
    let mut job_dir = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--job" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--job requires a directory".to_string())?;
                job_dir = Some(PathBuf::from(value));
            }
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }
    job_dir.ok_or_else(|| "--job is required".to_string())
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

fn parse_review_options(args: &[String]) -> std::result::Result<ReviewOptions, String> {
    let mut packet_path = None;
    let mut out_dir = None;
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
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(ReviewOptions {
        packet_path: packet_path.ok_or_else(|| "--packet is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
    })
}

fn parse_remediate_options(args: &[String]) -> std::result::Result<RemediateOptions, String> {
    let mut packet_path = None;
    let mut out_dir = None;
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
            unexpected => return Err(format!("unexpected argument: {unexpected}")),
        }
        index += 1;
    }

    Ok(RemediateOptions {
        packet_path: packet_path.ok_or_else(|| "--packet is required".to_string())?,
        out_dir: out_dir.ok_or_else(|| "--out is required".to_string())?,
    })
}

fn run_v0(options: RunOptions) -> Result<RunReceipt> {
    let started_at = now_utc();
    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!("create output directory {}", options.out_dir.display()),
        source,
    })?;

    let run_id = new_run_id();
    let request_path = options.out_dir.join("worker-request.json");
    let response_path = options.out_dir.join("worker-response.json");
    let mut run_failures = manifest.preflight_failures();
    let response = if run_failures.is_empty() {
        let request = WorkerRequest::from_manifest(
            &run_id,
            &manifest,
            &options.manifest_path,
            &options.out_dir.join("artifacts"),
        )?;
        write_json_pretty(&request_path, &request)?;

        match invoke_worker(
            &request_path,
            &response_path,
            manifest.policy.worker_timeout_ms,
        ) {
            Ok(()) => read_worker_response(&response_path),
            Err(failure) => {
                let message = failure.message.clone();
                run_failures.push(failure);
                Ok(WorkerResponse::error(message))
            }
        }?
    } else {
        WorkerResponse::error(
            run_failures
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    };
    response.validate()?;

    write_packet_and_report(
        &manifest,
        &options.manifest_path,
        &options.out_dir,
        response,
        run_failures,
        started_at,
        now_utc(),
        run_id,
    )
}

fn run_release(options: ReleaseOptions) -> Result<ReleaseReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create release output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;

    let packet = read_release_packet(&options.packet_path)?;

    let projection = project_release_decision(&packet, &options);
    let summary_path = options.out_dir.join("release-summary.json");
    let check_path = options.out_dir.join("github-check.json");
    let report_path = options.out_dir.join("release-report.html");
    write_json_pretty(&summary_path, &projection.summary)?;
    write_json_pretty(&check_path, &projection.github_check)?;
    write_string(&report_path, &render_release_report(&projection.summary))?;

    Ok(ReleaseReceipt {
        status: projection.summary["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        exit_class: projection.exit_class,
        summary_path,
        check_path,
        report_path,
    })
}

fn run_discovery(options: DiscoveryOptions) -> Result<DiscoveryReceipt> {
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
    let surfaces = discover_surfaces(&manifest, &options.manifest_path)?;
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

fn run_promote_flow(options: PromoteFlowOptions) -> Result<PromoteFlowReceipt> {
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

fn run_map(options: MapOptions) -> Result<MapReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!("create map output directory {}", options.out_dir.display()),
        source,
    })?;

    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let project_root =
        fs::canonicalize(&options.project_root).unwrap_or_else(|_| options.project_root.clone());
    let surfaces = product_surfaces(&manifest, &options.manifest_path, &project_root)?;
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
        open_questions: product_map_open_questions(&manifest),
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

fn run_workbench(command: WorkbenchCommand) -> Result<WorkbenchReceipt> {
    match command {
        WorkbenchCommand::Start(options) => run_workbench_start(options),
        WorkbenchCommand::Status { job_dir } => run_workbench_status(&job_dir),
        WorkbenchCommand::Cancel { job_dir } => run_workbench_cancel(&job_dir),
        WorkbenchCommand::Resume { job_dir } => run_workbench_resume(&job_dir),
    }
}

fn run_workbench_start(options: WorkbenchStartOptions) -> Result<WorkbenchReceipt> {
    ensure_new_workbench_dir(&options.out_dir)?;
    run_workbench_start_with_job(options, None)
}

fn run_workbench_start_with_job(
    options: WorkbenchStartOptions,
    resume_from: Option<WorkbenchJobPacket>,
) -> Result<WorkbenchReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create workbench job directory {}",
            options.out_dir.display()
        ),
        source,
    })?;
    let resuming = resume_from.is_some();
    if !resuming {
        write_string(&options.out_dir.join("events.jsonl"), "")?;
    }

    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let project_root = options
        .project_root
        .unwrap_or_else(|| default_project_root_for_manifest(&options.manifest_path, &manifest));
    let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);
    let mut job = match resume_from {
        Some(job) => reset_workbench_job_for_resume(
            job,
            &options.manifest_path,
            &project_root,
            options.agent_runner,
            manifest.policy.worker_timeout_ms,
        ),
        None => new_workbench_job(
            &options.manifest_path,
            &project_root,
            options.agent_runner,
            manifest.policy.worker_timeout_ms,
        ),
    };
    append_workbench_event(
        &options.out_dir,
        if resuming {
            "job_resumed"
        } else {
            "job_started"
        },
        None,
        Some(&job.status),
        if resuming {
            "operator resumed durable job execution"
        } else {
            "durable workbench job started"
        },
    )?;
    write_workbench_job(&options.out_dir, &job)?;

    let discovery_dir = options.out_dir.join("steps/discovery");
    let map_dir = options.out_dir.join("steps/map");
    let run_dir = options.out_dir.join("steps/run");
    let report_dir = options.out_dir.join("steps/report");
    let review_dir = options.out_dir.join("steps/review");
    let remediation_dir = options.out_dir.join("steps/remediation");
    let release_dir = options.out_dir.join("steps/release");

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "discover")? {
        return Ok(receipt);
    }
    let discovery = match run_discovery(DiscoveryOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: discovery_dir.clone(),
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "discover", error),
    };
    job.pointers.discovery = Some(path_relative_to(
        &options.out_dir,
        &discovery.discovery_path,
    ));
    job.pointers.flow_plan = Some(path_relative_to(
        &options.out_dir,
        &discovery.flow_plan_path,
    ));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "discovery".to_string(),
        path: path_relative_to(&options.out_dir, &discovery.discovery_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "discover",
        "completed",
        Some(&discovery.discovery_path),
        ExitClass::Success,
        "discovery packet written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    let generated_flow_path = discovery_dir.join("generated-flow.yml");
    if let Some(receipt) =
        workbench_start_step_or_cancel(&options.out_dir, &mut job, "promote-flow")?
    {
        return Ok(receipt);
    }
    let promoted = match run_promote_flow(PromoteFlowOptions {
        discovery_path: discovery.discovery_path.clone(),
        flow_plan_path: discovery.flow_plan_path.clone(),
        out_path: generated_flow_path,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "promote-flow", error),
    };
    job.pointers.generated_flow = Some(path_relative_to(&options.out_dir, &promoted.manifest_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "generated_flow".to_string(),
        path: path_relative_to(&options.out_dir, &promoted.manifest_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "promote-flow",
        "completed",
        Some(&promoted.manifest_path),
        ExitClass::Success,
        "generated flow manifest written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "map")? {
        return Ok(receipt);
    }
    let map = match run_map(MapOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: map_dir,
        project_root: project_root.clone(),
        agent_runner: options.agent_runner,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "map", error),
    };
    job.pointers.product_map = Some(path_relative_to(&options.out_dir, &map.map_path));
    job.pointers.surface_map = Some(path_relative_to(&options.out_dir, &map.report_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "product_map".to_string(),
        path: path_relative_to(&options.out_dir, &map.map_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "map",
        "completed",
        Some(&map.map_path),
        ExitClass::Success,
        "product map written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "run")? {
        return Ok(receipt);
    }
    let run = match run_v0(RunOptions {
        manifest_path: promoted.manifest_path.clone(),
        out_dir: run_dir,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "run", error),
    };
    job.pointers.evidence_packet = Some(path_relative_to(&options.out_dir, &run.evidence_path));
    job.pointers.evidence_report = Some(path_relative_to(&options.out_dir, &run.report_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "evidence_packet".to_string(),
        path: path_relative_to(&options.out_dir, &run.evidence_path),
    });
    let run_step_status = status_for_exit_class(run.exit_class);
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "run",
        run_step_status,
        Some(&run.evidence_path),
        run.exit_class,
        "evidence replay completed",
    )?;
    if run.exit_class == ExitClass::InfrastructureFailure {
        return workbench_finish(
            &options.out_dir,
            job,
            "failed",
            ExitClass::InfrastructureFailure,
            "run stopped on infrastructure failure",
        );
    }
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "report")? {
        return Ok(receipt);
    }
    let report = match run_compliance_report(ReportOptions {
        map_path: map.map_path.clone(),
        packet_path: run.evidence_path.clone(),
        out_dir: report_dir,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "report", error),
    };
    job.pointers.compliance_report =
        Some(path_relative_to(&options.out_dir, &report.report_json_path));
    job.pointers.compliance_html =
        Some(path_relative_to(&options.out_dir, &report.report_html_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "compliance_report".to_string(),
        path: path_relative_to(&options.out_dir, &report.report_json_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "report",
        "completed",
        Some(&report.report_json_path),
        ExitClass::Success,
        "compliance report written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "review")? {
        return Ok(receipt);
    }
    let review = match run_review(ReviewOptions {
        packet_path: run.evidence_path.clone(),
        out_dir: review_dir,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "review", error),
    };
    job.pointers.reviewed_packet = Some(path_relative_to(&options.out_dir, &review.packet_path));
    job.pointers.review_report = Some(path_relative_to(&options.out_dir, &review.report_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "reviewed_packet".to_string(),
        path: path_relative_to(&options.out_dir, &review.packet_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "review",
        "completed",
        Some(&review.packet_path),
        ExitClass::Success,
        "agentic review context written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) =
        workbench_start_step_or_cancel(&options.out_dir, &mut job, "remediation")?
    {
        return Ok(receipt);
    }
    let remediation = match run_remediate(RemediateOptions {
        packet_path: run.evidence_path.clone(),
        out_dir: remediation_dir,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "remediation", error),
    };
    job.pointers.remediation_queue =
        Some(path_relative_to(&options.out_dir, &remediation.queue_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "remediation_queue".to_string(),
        path: path_relative_to(&options.out_dir, &remediation.queue_path),
    });
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "remediation",
        "completed",
        Some(&remediation.queue_path),
        ExitClass::Success,
        "remediation queue written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "release")? {
        return Ok(receipt);
    }
    let changed_surface = workbench_changed_surface(&discovery.flow_plan_path)?;
    let release = match run_release(ReleaseOptions {
        packet_path: review.packet_path.clone(),
        out_dir: release_dir,
        changed_surfaces: vec![changed_surface],
        stale_after_days: 7,
    }) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "release", error),
    };
    job.pointers.release_summary = Some(path_relative_to(&options.out_dir, &release.summary_path));
    job.pointers.release_report = Some(path_relative_to(&options.out_dir, &release.report_path));
    job.artifacts.push(WorkbenchArtifactRef {
        kind: "release_summary".to_string(),
        path: path_relative_to(&options.out_dir, &release.summary_path),
    });
    let release_step_status = status_for_exit_class(release.exit_class);
    workbench_step_complete(
        &options.out_dir,
        &mut job,
        "release",
        release_step_status,
        Some(&release.summary_path),
        release.exit_class,
        "release projection written",
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }

    let final_status = match release.exit_class {
        ExitClass::Success => "completed",
        ExitClass::BlockingFinding => "blocked",
        ExitClass::InfrastructureFailure | ExitClass::Usage => "failed",
    };
    workbench_finish(
        &options.out_dir,
        job,
        final_status,
        release.exit_class,
        "workbench job finished",
    )
}

fn run_workbench_status(job_dir: &Path) -> Result<WorkbenchReceipt> {
    let job = read_workbench_job(job_dir)?;
    Ok(workbench_receipt(job_dir, &job, ExitClass::Success))
}

fn run_workbench_cancel(job_dir: &Path) -> Result<WorkbenchReceipt> {
    let mut job = read_workbench_job(job_dir)?;
    job.status = "cancelled".to_string();
    job.current_step = "cancelled".to_string();
    job.cancel_requested = true;
    job.resumable = true;
    job.updated_at = now_utc().to_rfc3339();
    append_workbench_event(
        job_dir,
        "job_cancel_requested",
        None,
        Some(&job.status),
        "operator requested cancellation",
    )?;
    write_workbench_job(job_dir, &job)?;
    Ok(workbench_receipt(job_dir, &job, ExitClass::Success))
}

fn run_workbench_resume(job_dir: &Path) -> Result<WorkbenchReceipt> {
    let job = read_workbench_job(job_dir)?;
    let runner = AgentRunnerKind::parse(&job.runner.kind).map_err(AllieError::InvalidManifest)?;
    if runner != AgentRunnerKind::Local {
        return Err(AllieError::InvalidManifest(format!(
            "workbench resume currently supports only local jobs; rerun allie map --agent {} for one-shot advisory agent mapping",
            runner.as_str()
        )));
    }
    run_workbench_start_with_job(
        WorkbenchStartOptions {
            manifest_path: PathBuf::from(&job.manifest_path),
            out_dir: job_dir.to_path_buf(),
            project_root: Some(PathBuf::from(&job.project_root)),
            agent_runner: runner,
        },
        Some(job),
    )
}

fn new_workbench_job(
    manifest_path: &Path,
    project_root: &Path,
    runner: AgentRunnerKind,
    worker_timeout_ms: u64,
) -> WorkbenchJobPacket {
    let now = now_utc().to_rfc3339();
    WorkbenchJobPacket {
        schema: JOB_SCHEMA.to_string(),
        id: new_job_id(),
        status: "running".to_string(),
        current_step: "queued".to_string(),
        created_at: now.clone(),
        updated_at: now,
        finished_at: None,
        manifest_path: manifest_path.to_string_lossy().to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        runtime_policy: WorkbenchRuntimePolicy {
            max_runtime_ms: DEFAULT_WORKBENCH_MAX_RUNTIME_MS,
            idle_timeout_ms: DEFAULT_WORKBENCH_IDLE_TIMEOUT_MS,
            agent_step_timeout_ms: None,
            worker_timeout_ms,
            ci_mode: false,
            enforcement_note:
                "agent job lifecycle is budgeted by job policy, not the 120s advisory runner guard"
                    .to_string(),
        },
        runner: WorkbenchRunnerState {
            kind: runner.as_str().to_string(),
            adapter_mode: "foreground-durable-local-job".to_string(),
            resume_contract:
                "job.json plus events.jsonl are sufficient to inspect, cancel, and resume"
                    .to_string(),
        },
        steps: Vec::new(),
        pointers: WorkbenchPointers::default(),
        artifacts: Vec::new(),
        resumable: true,
        cancel_requested: false,
        resume_count: 0,
        warnings: Vec::new(),
    }
}

fn ensure_new_workbench_dir(job_dir: &Path) -> Result<()> {
    for relative in ["job.json", "events.jsonl", "steps"] {
        let path = job_dir.join(relative);
        if path.exists() {
            return Err(AllieError::InvalidManifest(format!(
                "workbench job directory {} already contains durable state at {}; choose a new --out directory or use workbench resume",
                job_dir.display(),
                relative
            )));
        }
    }
    Ok(())
}

fn reset_workbench_job_for_resume(
    mut job: WorkbenchJobPacket,
    manifest_path: &Path,
    project_root: &Path,
    runner: AgentRunnerKind,
    worker_timeout_ms: u64,
) -> WorkbenchJobPacket {
    let now = now_utc().to_rfc3339();
    job.status = "running".to_string();
    job.current_step = "queued".to_string();
    job.updated_at = now;
    job.finished_at = None;
    job.manifest_path = manifest_path.to_string_lossy().to_string();
    job.project_root = project_root.to_string_lossy().to_string();
    job.runtime_policy.worker_timeout_ms = worker_timeout_ms;
    job.runner.kind = runner.as_str().to_string();
    job.resumable = true;
    job.cancel_requested = false;
    job.resume_count += 1;
    job
}

fn workbench_start_step_or_cancel(
    job_dir: &Path,
    job: &mut WorkbenchJobPacket,
    step: &str,
) -> Result<Option<WorkbenchReceipt>> {
    if let Some(receipt) = workbench_cancel_checkpoint(job_dir, job)? {
        return Ok(Some(receipt));
    }
    workbench_step_start(job_dir, job, step)?;
    workbench_cancel_checkpoint(job_dir, job)
}

fn workbench_step_start(job_dir: &Path, job: &mut WorkbenchJobPacket, step: &str) -> Result<()> {
    let now = now_utc().to_rfc3339();
    job.status = "running".to_string();
    job.current_step = step.to_string();
    job.updated_at = now.clone();
    job.steps.push(WorkbenchStepRecord {
        id: step.to_string(),
        status: "running".to_string(),
        started_at: Some(now),
        finished_at: None,
        receipt_path: None,
        exit_code: None,
        message: "step started".to_string(),
    });
    append_workbench_event(job_dir, "step_started", Some(step), Some("running"), "")?;
    write_workbench_job(job_dir, job)
}

fn workbench_step_complete(
    job_dir: &Path,
    job: &mut WorkbenchJobPacket,
    step: &str,
    status: &str,
    receipt_path: Option<&Path>,
    exit_class: ExitClass,
    message: &str,
) -> Result<()> {
    let now = now_utc().to_rfc3339();
    job.updated_at = now.clone();
    if let Some(record) = job.steps.iter_mut().rev().find(|record| record.id == step) {
        record.status = status.to_string();
        record.finished_at = Some(now);
        record.receipt_path = receipt_path.map(|path| path_relative_to(job_dir, path));
        record.exit_code = Some(exit_class.code());
        record.message = message.to_string();
    }
    append_workbench_event(job_dir, "step_completed", Some(step), Some(status), message)?;
    write_workbench_job(job_dir, job)
}

fn workbench_step_error(
    job_dir: &Path,
    mut job: WorkbenchJobPacket,
    step: &str,
    error: AllieError,
) -> Result<WorkbenchReceipt> {
    let message = error.to_string();
    job.warnings.push(message.clone());
    workbench_step_complete(
        job_dir,
        &mut job,
        step,
        "failed",
        None,
        ExitClass::InfrastructureFailure,
        &message,
    )?;
    workbench_finish(
        job_dir,
        job,
        "failed",
        ExitClass::InfrastructureFailure,
        "workbench job failed",
    )
}

fn workbench_finish(
    job_dir: &Path,
    mut job: WorkbenchJobPacket,
    status: &str,
    exit_class: ExitClass,
    message: &str,
) -> Result<WorkbenchReceipt> {
    let now = now_utc().to_rfc3339();
    let cancelled = status == "cancelled" || workbench_cancel_requested(job_dir)?;
    let durable_status = if cancelled { "cancelled" } else { status };
    job.status = durable_status.to_string();
    job.current_step = if cancelled {
        "cancelled".to_string()
    } else {
        "finished".to_string()
    };
    job.updated_at = now.clone();
    job.finished_at = Some(now);
    if cancelled {
        job.cancel_requested = true;
    }
    append_workbench_event(
        job_dir,
        if cancelled {
            "job_cancelled"
        } else {
            "job_finished"
        },
        None,
        Some(durable_status),
        if cancelled {
            "workbench job stopped after operator cancellation"
        } else {
            message
        },
    )?;
    write_workbench_job(job_dir, &job)?;
    Ok(workbench_receipt(
        job_dir,
        &job,
        if cancelled {
            ExitClass::Success
        } else {
            exit_class
        },
    ))
}

fn workbench_cancel_checkpoint(
    job_dir: &Path,
    job: &mut WorkbenchJobPacket,
) -> Result<Option<WorkbenchReceipt>> {
    if workbench_cancel_requested(job_dir)? {
        let cancelled_job = job.clone();
        return workbench_finish(
            job_dir,
            cancelled_job,
            "cancelled",
            ExitClass::Success,
            "workbench job cancelled",
        )
        .map(Some);
    }
    Ok(None)
}

fn status_for_exit_class(exit_class: ExitClass) -> &'static str {
    match exit_class {
        ExitClass::Success => "completed",
        ExitClass::BlockingFinding => "blocked",
        ExitClass::InfrastructureFailure | ExitClass::Usage => "failed",
    }
}

fn workbench_receipt(
    job_dir: &Path,
    job: &WorkbenchJobPacket,
    exit_class: ExitClass,
) -> WorkbenchReceipt {
    WorkbenchReceipt {
        job_path: job_dir.join("job.json"),
        events_path: job_dir.join("events.jsonl"),
        status: job.status.clone(),
        current_step: job.current_step.clone(),
        resumable: job.resumable,
        exit_class,
    }
}

fn write_workbench_job(job_dir: &Path, job: &WorkbenchJobPacket) -> Result<()> {
    let mut durable_job = job.clone();
    match read_workbench_job(job_dir) {
        Ok(existing) => {
            let same_resume_generation = existing.resume_count == durable_job.resume_count;
            if same_resume_generation && existing.cancel_requested {
                durable_job.cancel_requested = true;
            }
            if same_resume_generation
                && existing.status == "cancelled"
                && durable_job.status != "cancelled"
            {
                durable_job.status = existing.status;
                durable_job.current_step = existing.current_step;
                durable_job.finished_at = existing.finished_at;
            }
        }
        Err(AllieError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let job_path = job_dir.join("job.json");
    let json = serde_json::to_string_pretty(&durable_job).map_err(|source| AllieError::Json {
        context: format!("serialize json {}", job_path.display()),
        source,
    })?;
    write_string_atomic(&job_path, &(json + "\n"))
}

fn read_workbench_job(job_dir: &Path) -> Result<WorkbenchJobPacket> {
    let job: WorkbenchJobPacket = read_json_file(&job_dir.join("job.json"))?;
    if job.schema != JOB_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "invalid workbench job schema {}; expected {JOB_SCHEMA}",
            job.schema
        )));
    }
    Ok(job)
}

fn workbench_cancel_requested(job_dir: &Path) -> Result<bool> {
    match read_workbench_job(job_dir) {
        Ok(job) => Ok(job.cancel_requested || job.status == "cancelled"),
        Err(AllieError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn append_workbench_event(
    job_dir: &Path,
    event: &str,
    step: Option<&str>,
    status: Option<&str>,
    message: &str,
) -> Result<()> {
    let event_path = job_dir.join("events.jsonl");
    if let Some(parent) = event_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AllieError::Io {
            context: format!("create directory {}", parent.display()),
            source,
        })?;
    }
    let line = serde_json::json!({
        "at": now_utc().to_rfc3339(),
        "event": event,
        "step": step,
        "status": status,
        "message": message
    })
    .to_string();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&event_path)
        .map_err(|source| AllieError::Io {
            context: format!("open event log {}", event_path.display()),
            source,
        })?;
    writeln!(file, "{line}").map_err(|source| AllieError::Io {
        context: format!("append event log {}", event_path.display()),
        source,
    })
}

fn default_project_root_for_manifest(manifest_path: &Path, manifest: &FlowManifest) -> PathBuf {
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

fn workbench_changed_surface(flow_plan_path: &Path) -> Result<String> {
    let flow_plan: FlowPlanPacket = read_json_file(flow_plan_path)?;
    Ok(flow_plan
        .candidates
        .first()
        .map(|candidate| candidate.id.clone())
        .unwrap_or_else(|| "generated-flow".to_string()))
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
) -> Result<Vec<ProductSurface>> {
    let mut surfaces: BTreeMap<String, ProductSurface> = BTreeMap::new();
    for discovered in discover_surfaces(manifest, manifest_path)? {
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

    Ok(surfaces.into_values().collect())
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

fn standards_profile_summary(policy_profile: &str) -> StandardsProfileSummary {
    if policy_profile != "wcag22-aa" {
        return StandardsProfileSummary {
            id: policy_profile.to_string(),
            source_urls: Vec::new(),
            total_obligations: 0,
            methods: BTreeMap::new(),
        };
    }
    let mut methods = BTreeMap::new();
    for criterion in wcag22_success_criteria() {
        let method = criterion["method"].as_str().unwrap_or("unknown");
        *methods.entry(method.to_string()).or_insert(0) += 1;
    }
    StandardsProfileSummary {
        id: "wcag22-aa".to_string(),
        source_urls: vec![
            wcag22_profile()["source_url"]
                .as_str()
                .unwrap_or("https://www.w3.org/WAI/WCAG22/wcag.json")
                .to_string(),
            "https://www.w3.org/TR/WCAG22/".to_string(),
            "https://www.w3.org/WAI/test-evaluate/conformance/wcag-em/".to_string(),
        ],
        total_obligations: wcag22_success_criteria().len(),
        methods,
    }
}

fn product_map_open_questions(manifest: &FlowManifest) -> Vec<String> {
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

fn render_product_surface_map(map: &ProductMapPacket) -> String {
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

fn run_compliance_report(options: ReportOptions) -> Result<ComplianceReportReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create compliance report output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;
    let map: ProductMapPacket = read_json_file(&options.map_path)?;
    if map.schema != PRODUCT_MAP_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "invalid product map schema {}; expected {PRODUCT_MAP_SCHEMA}",
            map.schema
        )));
    }
    let packet: EvidencePacket = read_json_file(&options.packet_path)?;
    validate_release_packet(&packet)?;
    let report = build_compliance_report(&map, &packet, &options.map_path, &options.packet_path);
    let report_value = serde_json::to_value(&report).map_err(|source| AllieError::Json {
        context: "serialize compliance report for validation".to_string(),
        source,
    })?;
    validate_criterion_coverage_cells(&report_value).map_err(AllieError::InvalidManifest)?;

    let report_json_path = options.out_dir.join("compliance-report.json");
    let report_html_path = options.out_dir.join("compliance-report.html");
    let summary_path = options.out_dir.join("summary.md");
    write_json_pretty(&report_json_path, &report)?;
    write_string(&report_html_path, &render_compliance_report(&report))?;
    write_string(&summary_path, &render_compliance_summary(&report))?;

    Ok(ComplianceReportReceipt {
        report_json_path,
        report_html_path,
        summary_path,
    })
}

fn build_compliance_report(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    map_path: &Path,
    packet_path: &Path,
) -> ComplianceReportPacket {
    let base_criteria = compliance_criterion_order(&map.policy_profile, packet)
        .into_iter()
        .map(|obligation| compliance_obligation(map, packet, &obligation))
        .collect::<Vec<_>>();
    let supporting_checks = supporting_check_order(&map.policy_profile, packet)
        .into_iter()
        .map(|obligation| compliance_supporting_check(map, packet, &obligation))
        .collect::<Vec<_>>();
    let criterion_coverage = criterion_coverage_matrix(map, packet);
    let criteria = aggregate_criteria_from_cells(base_criteria, &criterion_coverage);
    let summary = compliance_summary(packet, &criteria, supporting_checks.len());
    let surfaces = map
        .surfaces
        .iter()
        .map(|surface| compliance_surface_report(surface, packet, &criteria, &criterion_coverage))
        .collect();
    ComplianceReportPacket {
        schema: COMPLIANCE_REPORT_SCHEMA.to_string(),
        generated_at: now_utc().to_rfc3339(),
        source_map: map_path.to_string_lossy().to_string(),
        source_packet: packet_path.to_string_lossy().to_string(),
        app_name: map.app_name.clone(),
        summary,
        criteria: criteria.clone(),
        criterion_coverage,
        supporting_checks,
        obligations: criteria,
        surfaces,
    }
}

fn compliance_criterion_order(policy_profile: &str, packet: &EvidencePacket) -> Vec<String> {
    let mut obligations = Vec::new();
    let mut seen = BTreeSet::new();
    if policy_profile == "wcag22-aa" {
        for criterion in wcag22_success_criteria() {
            if let Some(obligation) = criterion["obligation"].as_str()
                && seen.insert(obligation.to_string())
            {
                obligations.push(obligation.to_string());
            }
        }
        return obligations;
    }

    for verdict in &packet.verdicts {
        if seen.insert(verdict.obligation.clone()) {
            obligations.push(verdict.obligation.clone());
        }
    }
    for finding in &packet.findings {
        if seen.insert(finding.standard_obligation.clone()) {
            obligations.push(finding.standard_obligation.clone());
        }
    }
    obligations
}

fn supporting_check_order(policy_profile: &str, packet: &EvidencePacket) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    let criteria = wcag22_success_criterion_ids();
    let mut support = Vec::new();
    let mut seen = BTreeSet::new();
    for obligation in std::iter::once(deterministic_pass_obligation(policy_profile))
        .chain(profile_obligation_list(
            policy_profile,
            "scripted_obligations",
        ))
        .chain(profile_obligation_list(
            policy_profile,
            "human_review_obligations",
        ))
        .chain(
            packet
                .verdicts
                .iter()
                .map(|verdict| verdict.obligation.clone()),
        )
        .chain(
            packet
                .findings
                .iter()
                .map(|finding| finding.standard_obligation.clone()),
        )
    {
        if !criteria.contains(&obligation) && seen.insert(obligation.clone()) {
            support.push(obligation);
        }
    }
    support
}

fn wcag22_success_criterion_ids() -> BTreeSet<String> {
    wcag22_success_criteria()
        .into_iter()
        .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
        .collect()
}

fn compliance_obligation(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    obligation: &str,
) -> ComplianceObligation {
    let verdicts = packet
        .verdicts
        .iter()
        .filter(|verdict| verdict.obligation == obligation)
        .collect::<Vec<_>>();
    let findings = related_findings(packet, obligation, &verdicts);
    let status = compliance_status(&verdicts, &findings);
    let surfaces = obligation_surface_ids(map, &verdicts, &findings);
    let tests = unique_strings(
        verdicts
            .iter()
            .map(|verdict| verdict.source.clone())
            .chain(findings.iter().map(|finding| finding.source.clone())),
    );
    let artifact_refs = obligation_artifact_refs(packet, &verdicts, &findings);
    let agentic_context = obligation_agentic_context(map, packet, &findings);
    let confidence = verdicts
        .iter()
        .map(|verdict| verdict.confidence.clone())
        .chain(findings.iter().map(|finding| finding.confidence.clone()))
        .next()
        .unwrap_or_else(|| "not_observed".to_string());
    let evidence_class = verdicts
        .iter()
        .map(|verdict| verdict.evidence_class.clone())
        .chain(
            findings
                .iter()
                .map(|finding| finding.evidence_class.clone()),
        )
        .next()
        .unwrap_or_else(|| "none".to_string());
    ComplianceObligation {
        id: obligation.to_string(),
        title: criterion_title(obligation),
        status: status.clone(),
        why: obligation_why(obligation, &status, &verdicts, &findings),
        surfaces,
        tests,
        artifact_refs,
        agentic_context,
        human_review: human_review_status(&status, &evidence_class),
        confidence,
        evidence_class,
        source_url: criterion_source_url(obligation),
        finding_refs: findings.iter().map(|finding| finding.id.clone()).collect(),
    }
}

fn compliance_supporting_check(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    obligation: &str,
) -> ComplianceSupportingCheck {
    let row = compliance_obligation(map, packet, obligation);
    ComplianceSupportingCheck {
        id: row.id.clone(),
        title: row.title.clone(),
        status: row.status.clone(),
        why: row.why.clone(),
        related_criteria: supporting_check_related_criteria(obligation),
        surfaces: row.surfaces,
        tests: row.tests,
        artifact_refs: row.artifact_refs,
        agentic_context: row.agentic_context,
        human_review: row.human_review,
        confidence: row.confidence,
        evidence_class: row.evidence_class,
        finding_refs: row.finding_refs,
    }
}

fn supporting_check_related_criteria(obligation: &str) -> Vec<String> {
    match obligation {
        "wcag22-aa:deterministic-axe-rules" => wcag22_success_criteria()
            .into_iter()
            .filter(|criterion| criterion["method"].as_str() == Some("axe"))
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        "wcag22-aa:2.1.1-keyboard-traversal" => vec![
            "wcag22-aa:2.1.1-keyboard".to_string(),
            "wcag22-aa:2.1.2-no-keyboard-trap".to_string(),
            "wcag22-aa:2.4.3-focus-order".to_string(),
            "wcag22-aa:2.4.7-focus-visible".to_string(),
            "wcag22-aa:2.4.11-focus-not-obscured-minimum".to_string(),
        ],
        "wcag22-aa:1.4.10-zoom-reflow" => vec![
            "wcag22-aa:1.4.4-resize-text".to_string(),
            "wcag22-aa:1.4.10-reflow".to_string(),
            "wcag22-aa:1.4.12-text-spacing".to_string(),
        ],
        "wcag22-aa:2.2.2-reduced-motion" => vec![
            "wcag22-aa:2.2.2-pause-stop-hide".to_string(),
            "wcag22-aa:2.3.1-three-flashes-or-below-threshold".to_string(),
            "wcag22-aa:2.5.4-motion-actuation".to_string(),
        ],
        "wcag22-aa:human-content-meaning" => wcag22_success_criteria()
            .into_iter()
            .filter(|criterion| criterion["method"].as_str() == Some("human_review"))
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        "wcag22-aa:human-assistive-technology-review" => wcag22_success_criteria()
            .into_iter()
            .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn related_findings<'a>(
    packet: &'a EvidencePacket,
    obligation: &str,
    verdicts: &[&Verdict],
) -> Vec<&'a Finding> {
    let finding_refs = verdicts
        .iter()
        .flat_map(|verdict| verdict.finding_refs.iter().cloned())
        .collect::<BTreeSet<_>>();
    packet
        .findings
        .iter()
        .filter(|finding| {
            finding.standard_obligation == obligation || finding_refs.contains(&finding.id)
        })
        .collect()
}

fn compliance_status(verdicts: &[&Verdict], findings: &[&Finding]) -> String {
    if findings.iter().any(|finding| finding.status == "fail")
        || verdicts.iter().any(|verdict| verdict.status == "fail")
    {
        "fail".to_string()
    } else if findings
        .iter()
        .any(|finding| finding.status == "needs_review")
        || verdicts
            .iter()
            .any(|verdict| verdict.status == "needs_review")
    {
        "needs_review".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "not_applicable")
    {
        "not_applicable".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "waived") {
        "waived".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "risk_accepted")
    {
        "risk_accepted".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "pass") {
        "pass".to_string()
    } else {
        "not_tested".to_string()
    }
}

fn obligation_surface_ids(
    map: &ProductMapPacket,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> Vec<String> {
    let state_ids = verdicts
        .iter()
        .flat_map(|verdict| verdict.affected_states.iter().cloned())
        .chain(
            findings
                .iter()
                .map(|finding| finding.affected_state.clone()),
        )
        .collect::<BTreeSet<_>>();
    let routes = findings
        .iter()
        .map(|finding| finding.affected_route.clone())
        .collect::<BTreeSet<_>>();
    let mut surface_ids = map
        .surfaces
        .iter()
        .filter(|surface| {
            surface
                .evidence_refs
                .iter()
                .any(|state| state_ids.contains(state))
                || surface.id.as_str() != "run" && state_ids.contains(&surface.id)
                || surface.routes.iter().any(|route| routes.contains(route))
        })
        .map(|surface| surface.id.clone())
        .collect::<Vec<_>>();
    if surface_ids.is_empty() && !state_ids.is_empty() {
        surface_ids = state_ids.into_iter().collect();
    }
    if surface_ids.is_empty() && !map.surfaces.is_empty() {
        surface_ids = map
            .surfaces
            .iter()
            .map(|surface| surface.id.clone())
            .collect();
    }
    surface_ids.sort();
    surface_ids.dedup();
    surface_ids
}

fn obligation_artifact_refs(
    packet: &EvidencePacket,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> Vec<String> {
    let state_ids = verdicts
        .iter()
        .flat_map(|verdict| verdict.affected_states.iter().cloned())
        .chain(
            findings
                .iter()
                .map(|finding| finding.affected_state.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut refs = findings
        .iter()
        .flat_map(|finding| finding.artifact_refs.iter().cloned())
        .collect::<Vec<_>>();
    refs.extend(
        packet
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact
                    .related_flow_state
                    .as_ref()
                    .is_some_and(|state| state_ids.contains(state))
            })
            .map(|artifact| artifact.id.clone()),
    );
    unique_strings(refs)
}

fn obligation_agentic_context(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    findings: &[&Finding],
) -> Vec<String> {
    let mut context = findings
        .iter()
        .filter(|finding| finding.evidence_class == "agentic")
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    context.extend(packet.review.iter().map(|review| review.id.clone()));
    if let Some(transcript) = &map.agent.transcript_path {
        context.push(format!("map-agent-transcript:{transcript}"));
    }
    unique_strings(context)
}

fn obligation_why(
    obligation: &str,
    status: &str,
    verdicts: &[&Verdict],
    findings: &[&Finding],
) -> String {
    if let Some(finding) = findings.first() {
        return format!("{}: {}", finding.title, finding.description);
    }
    match status {
        "pass" => format!(
            "{} is marked pass by {}.",
            criterion_title(obligation),
            verdicts
                .first()
                .map(|verdict| verdict.source.as_str())
                .unwrap_or("the evidence packet")
        ),
        "needs_review" => format!(
            "{} requires visual, assistive-technology, workflow, or content judgment beyond deterministic automation.",
            criterion_title(obligation)
        ),
        "not_tested" => format!(
            "{} has no deterministic, scripted, agentic, or human-attested evidence in this packet.",
            criterion_title(obligation)
        ),
        "not_applicable" => format!("{} was marked not applicable.", criterion_title(obligation)),
        _ => format!("{} has status {}.", criterion_title(obligation), status),
    }
}

fn human_review_status(status: &str, evidence_class: &str) -> String {
    match (status, evidence_class) {
        ("pass", "deterministic") | ("pass", "scripted") => {
            "not_required_for_machine_evidence".to_string()
        }
        ("pass", _) => "attested_or_promoted".to_string(),
        ("fail", _) => "required_for_remediation_signoff".to_string(),
        ("needs_review", _) => "required".to_string(),
        ("not_tested", _) => "required_before_claim".to_string(),
        _ => "review_status_unknown".to_string(),
    }
}

fn criterion_source_url(obligation: &str) -> Option<String> {
    wcag22_success_criteria()
        .into_iter()
        .find(|criterion| criterion["obligation"].as_str() == Some(obligation))
        .and_then(|criterion| criterion["source_url"].as_str().map(ToString::to_string))
}

fn criterion_coverage_matrix(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
) -> Vec<CriterionCoverageCell> {
    if map.policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    let criteria = wcag22_success_criteria();
    let mut cells = Vec::new();
    for surface in &map.surfaces {
        for state_id in surface_state_ids(surface, packet) {
            for criterion in &criteria {
                if let Some(cell) =
                    criterion_coverage_cell(map, packet, surface, &state_id, criterion)
                {
                    cells.push(cell);
                }
            }
        }
    }
    cells
}

fn aggregate_criteria_from_cells(
    criteria: Vec<ComplianceObligation>,
    cells: &[CriterionCoverageCell],
) -> Vec<ComplianceObligation> {
    criteria
        .into_iter()
        .map(|mut criterion| {
            let criterion_cells = cells
                .iter()
                .filter(|cell| cell.criterion_id == criterion.id)
                .collect::<Vec<_>>();
            if criterion_cells.is_empty() {
                return criterion;
            }
            criterion.status = aggregate_cell_status(&criterion_cells);
            criterion.surfaces = unique_strings(
                criterion_cells
                    .iter()
                    .map(|cell| cell.surface_id.clone())
                    .collect::<Vec<_>>(),
            );
            criterion.tests = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.test_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.artifact_refs = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.artifact_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.agentic_context = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.agentic_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.finding_refs = unique_strings(
                criterion_cells
                    .iter()
                    .flat_map(|cell| cell.finding_refs.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            criterion.confidence = criterion_cells
                .iter()
                .map(|cell| cell.confidence.clone())
                .next()
                .unwrap_or_else(|| criterion.confidence.clone());
            criterion.why = criterion_matrix_why(&criterion, &criterion_cells);
            criterion
        })
        .collect()
}

fn aggregate_cell_status(cells: &[&CriterionCoverageCell]) -> String {
    if cells.iter().any(|cell| cell.status == "fail") {
        "fail".to_string()
    } else if cells.iter().any(|cell| cell.status == "needs_review") {
        "needs_review".to_string()
    } else if cells.iter().any(|cell| cell.status == "not_tested") {
        "not_tested".to_string()
    } else if cells.iter().any(|cell| cell.status == "risk_accepted") {
        "risk_accepted".to_string()
    } else if cells.iter().any(|cell| cell.status == "waived") {
        "waived".to_string()
    } else if cells.iter().all(|cell| cell.status == "not_applicable") {
        "not_applicable".to_string()
    } else {
        "pass".to_string()
    }
}

fn criterion_matrix_why(
    criterion: &ComplianceObligation,
    cells: &[&CriterionCoverageCell],
) -> String {
    let status = criterion.status.as_str();
    let total = cells.len();
    let with_evidence = cells
        .iter()
        .filter(|cell| {
            !cell.evidence_refs.is_empty()
                || !cell.agentic_refs.is_empty()
                || !cell.waiver_refs.is_empty()
        })
        .count();
    match status {
        "fail" => format!(
            "{} has failing evidence in at least one surface/state cell.",
            criterion.title
        ),
        "needs_review" => format!(
            "{} has {} of {} cell(s) requiring human or agentic review.",
            criterion.title,
            cells
                .iter()
                .filter(|cell| cell.status == "needs_review")
                .count(),
            total
        ),
        "not_tested" => format!(
            "{} has no completed evidence in {} of {} cell(s).",
            criterion.title,
            cells
                .iter()
                .filter(|cell| cell.status == "not_tested")
                .count(),
            total
        ),
        "pass" => format!(
            "{} has supporting evidence in {} of {} surface/state cell(s).",
            criterion.title, with_evidence, total
        ),
        _ => format!(
            "{} has aggregate status {} across {} surface/state cell(s).",
            criterion.title, status, total
        ),
    }
}

fn criterion_coverage_cell(
    map: &ProductMapPacket,
    packet: &EvidencePacket,
    surface: &ProductSurface,
    state_id: &str,
    criterion: &serde_json::Value,
) -> Option<CriterionCoverageCell> {
    let criterion_id = criterion["obligation"].as_str()?;
    let method = criterion["method"].as_str().unwrap_or("human_review");
    let verdicts = packet
        .verdicts
        .iter()
        .filter(|verdict| {
            verdict.obligation == criterion_id && verdict_applies_to_state(verdict, state_id)
        })
        .collect::<Vec<_>>();
    let findings = packet
        .findings
        .iter()
        .filter(|finding| {
            finding.standard_obligation == criterion_id && finding.affected_state == state_id
        })
        .collect::<Vec<_>>();
    let deterministic_support = if method == "axe" {
        let deterministic_support_id = deterministic_pass_obligation(&map.policy_profile);
        packet.verdicts.iter().find(|verdict| {
            verdict.obligation == deterministic_support_id
                && verdict.status == "pass"
                && verdict_applies_to_state(verdict, state_id)
        })
    } else {
        None
    };
    let waiver_refs = waiver_refs_for_cell(packet, &surface.id, criterion_id);
    let waiver_status = waiver_status_for_cell(packet, &surface.id, criterion_id);
    let status = criterion_cell_status(
        method,
        &verdicts,
        &findings,
        deterministic_support,
        waiver_status.as_deref(),
    );
    let finding_refs = findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    let mut artifact_refs = obligation_artifact_refs(packet, &verdicts, &findings);
    if artifact_refs.is_empty() && deterministic_support.is_some() {
        artifact_refs = state_artifact_refs(packet, state_id);
    }
    let test_refs = unique_strings(
        verdicts
            .iter()
            .map(|verdict| verdict.source.clone())
            .chain(findings.iter().map(|finding| finding.source.clone()))
            .chain(deterministic_support.map(|verdict| verdict.source.clone())),
    );
    let evidence_refs = unique_strings(
        finding_refs
            .iter()
            .cloned()
            .chain(artifact_refs.iter().cloned())
            .chain(test_refs.iter().cloned()),
    );
    let agentic_refs = obligation_agentic_context(map, packet, &findings);
    let confidence = if waiver_status.is_some() {
        "human_attested".to_string()
    } else {
        verdicts
            .iter()
            .map(|verdict| verdict.confidence.clone())
            .chain(findings.iter().map(|finding| finding.confidence.clone()))
            .chain(deterministic_support.map(|verdict| verdict.confidence.clone()))
            .next()
            .unwrap_or_else(|| default_criterion_confidence(method).to_string())
    };

    Some(CriterionCoverageCell {
        id: format!(
            "{}|{}|{}|{}",
            criterion_id, surface.id, state_id, map.policy_profile
        ),
        criterion_id: criterion_id.to_string(),
        surface_id: surface.id.clone(),
        state_id: state_id.to_string(),
        policy_profile: map.policy_profile.clone(),
        status: status.clone(),
        applicability: if status == "not_applicable" {
            "not_applicable".to_string()
        } else {
            "applicable".to_string()
        },
        method: method.to_string(),
        confidence,
        evidence_refs,
        agentic_refs,
        waiver_refs,
        finding_refs,
        artifact_refs,
        test_refs,
        replay_command: Some(packet.replay.command.clone()),
        residual_review_need: residual_review_need(method, &status),
    })
}

fn criterion_cell_status(
    method: &str,
    verdicts: &[&Verdict],
    findings: &[&Finding],
    deterministic_support: Option<&Verdict>,
    waiver_status: Option<&str>,
) -> String {
    if waiver_status == Some("risk_accepted") {
        "risk_accepted".to_string()
    } else if waiver_status == Some("waived") {
        "waived".to_string()
    } else if findings.iter().any(|finding| finding.status == "fail")
        || verdicts.iter().any(|verdict| verdict.status == "fail")
    {
        "fail".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "not_applicable")
    {
        "not_applicable".to_string()
    } else if verdicts.iter().any(|verdict| verdict.status == "pass")
        || method == "axe" && deterministic_support.is_some()
    {
        "pass".to_string()
    } else if verdicts
        .iter()
        .any(|verdict| verdict.status == "needs_review")
        || method == "human_review"
    {
        "needs_review".to_string()
    } else {
        "not_tested".to_string()
    }
}

fn default_criterion_confidence(method: &str) -> &'static str {
    match method {
        "axe" => "not_observed",
        "scripted" => "script_observed",
        _ => "requires_human_or_agent_review",
    }
}

fn residual_review_need(method: &str, status: &str) -> String {
    match status {
        "pass" if method == "axe" => {
            "Deterministic evidence is present; sample with human review if policy requires."
                .to_string()
        }
        "pass" => "Evidence is present; retain replay proof for review.".to_string(),
        "fail" => "Remediate, rerun, and sign off with updated evidence.".to_string(),
        "waived" | "risk_accepted" => {
            "Review waiver provenance and expiry before release reliance.".to_string()
        }
        "not_applicable" => "Confirm applicability rationale with the product owner.".to_string(),
        "needs_review" => {
            "Human or agentic review required before making a compliance claim.".to_string()
        }
        _ => "No evidence in this packet for this criterion, surface, and state.".to_string(),
    }
}

fn verdict_applies_to_state(verdict: &Verdict, state_id: &str) -> bool {
    verdict.affected_states.is_empty()
        || verdict
            .affected_states
            .iter()
            .any(|affected| affected == state_id)
}

fn surface_state_ids(surface: &ProductSurface, packet: &EvidencePacket) -> Vec<String> {
    let mut states = surface
        .evidence_refs
        .iter()
        .filter(|state| !state.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    for state in &packet.coverage.state_metadata {
        if surface.routes.contains(&state.route) || surface.id == state.id {
            states.insert(state.id.clone());
        }
    }
    if states.is_empty() && !surface.id.trim().is_empty() {
        states.insert(surface.id.clone());
    }
    if states.is_empty() {
        states.extend(packet.coverage.states_captured.iter().cloned());
    }
    states.into_iter().collect()
}

fn state_artifact_refs(packet: &EvidencePacket, state_id: &str) -> Vec<String> {
    packet
        .artifacts
        .iter()
        .filter(|artifact| artifact.related_flow_state.as_deref() == Some(state_id))
        .map(|artifact| artifact.id.clone())
        .collect()
}

fn waiver_refs_for_cell(
    packet: &EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Vec<String> {
    matching_cell_waivers(packet, surface_id, criterion_id)
        .into_iter()
        .filter_map(|waiver| waiver["id"].as_str().map(ToString::to_string))
        .collect()
}

fn waiver_status_for_cell(
    packet: &EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Option<String> {
    let statuses = matching_cell_waivers(packet, surface_id, criterion_id)
        .into_iter()
        .filter_map(|waiver| waiver["status"].as_str())
        .collect::<Vec<_>>();
    if statuses.contains(&"risk_accepted") {
        Some("risk_accepted".to_string())
    } else if statuses.contains(&"waived") {
        Some("waived".to_string())
    } else {
        None
    }
}

fn matching_cell_waivers<'a>(
    packet: &'a EvidencePacket,
    surface_id: &str,
    criterion_id: &str,
) -> Vec<&'a serde_json::Value> {
    packet
        .waivers
        .iter()
        .filter(|waiver| waiver["surface"].as_str() == Some(surface_id))
        .filter(|waiver| {
            let standard = waiver["standard_obligation"]
                .as_str()
                .or_else(|| waiver["obligation"].as_str())
                .or_else(|| waiver["criterion_id"].as_str());
            standard.is_none() || standard == Some(criterion_id)
        })
        .collect()
}

fn validate_criterion_coverage_cells(
    report: &serde_json::Value,
) -> std::result::Result<(), String> {
    let Some(cells) = report["criterion_coverage"].as_array() else {
        return Err("criterion_coverage must be an array".to_string());
    };
    let mut cell_keys = BTreeSet::new();
    for cell in cells {
        for field in [
            "criterion_id",
            "surface_id",
            "state_id",
            "policy_profile",
            "status",
            "applicability",
            "method",
            "confidence",
            "residual_review_need",
        ] {
            if cell[field]
                .as_str()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!("criterion coverage cell missing {field}"));
            }
        }
        for field in [
            "evidence_refs",
            "agentic_refs",
            "waiver_refs",
            "finding_refs",
            "artifact_refs",
            "test_refs",
        ] {
            if !cell[field].is_array() {
                return Err(format!("criterion coverage cell missing {field}"));
            }
        }
        let status = cell["status"].as_str().unwrap_or_default();
        if matches!(status, "pass" | "fail" | "waived" | "risk_accepted")
            && !cell_has_provenance(cell)
        {
            return Err(format!(
                "terminal criterion coverage cell lacks provenance: {}",
                cell["criterion_id"].as_str().unwrap_or("unknown")
            ));
        }
        let key = coverage_cell_key(cell)?;
        if !cell_keys.insert(key.clone()) {
            return Err(format!("duplicate criterion coverage cell: {key}"));
        }
    }
    validate_criterion_coverage_completeness(report, &cell_keys)?;
    Ok(())
}

fn coverage_cell_key(cell: &serde_json::Value) -> std::result::Result<String, String> {
    Ok(format!(
        "{}|{}|{}|{}",
        cell["criterion_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing criterion_id".to_string())?,
        cell["surface_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing surface_id".to_string())?,
        cell["state_id"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing state_id".to_string())?,
        cell["policy_profile"]
            .as_str()
            .ok_or_else(|| "criterion coverage cell missing policy_profile".to_string())?
    ))
}

fn validate_criterion_coverage_completeness(
    report: &serde_json::Value,
    cell_keys: &BTreeSet<String>,
) -> std::result::Result<(), String> {
    let Some(criteria) = report["criteria"].as_array() else {
        return Ok(());
    };
    let Some(surfaces) = report["surfaces"].as_array() else {
        return Ok(());
    };
    let criterion_ids = criteria
        .iter()
        .filter_map(|criterion| criterion["id"].as_str())
        .collect::<Vec<_>>();
    for surface in surfaces {
        let Some(surface_id) = surface["surface_id"].as_str() else {
            continue;
        };
        let states = surface["states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|state| state.as_str())
            .collect::<Vec<_>>();
        for criterion_id in &criterion_ids {
            for state_id in &states {
                let key = format!("{criterion_id}|{surface_id}|{state_id}|wcag22-aa");
                if !cell_keys.contains(&key) {
                    return Err(format!("missing criterion coverage cell: {key}"));
                }
            }
        }
    }
    Ok(())
}

fn cell_has_provenance(cell: &serde_json::Value) -> bool {
    [
        "evidence_refs",
        "agentic_refs",
        "waiver_refs",
        "finding_refs",
        "artifact_refs",
        "test_refs",
    ]
    .iter()
    .any(|field| {
        cell[*field]
            .as_array()
            .is_some_and(|values| !values.is_empty())
    })
}

fn compliance_summary(
    packet: &EvidencePacket,
    criteria: &[ComplianceObligation],
    supporting_check_count: usize,
) -> ComplianceSummary {
    let pass = criteria
        .iter()
        .filter(|obligation| obligation.status == "pass")
        .count();
    let fail = criteria
        .iter()
        .filter(|obligation| obligation.status == "fail")
        .count();
    let needs_review = criteria
        .iter()
        .filter(|obligation| obligation.status == "needs_review")
        .count();
    let not_tested = criteria
        .iter()
        .filter(|obligation| obligation.status == "not_tested")
        .count();
    let not_applicable = criteria
        .iter()
        .filter(|obligation| obligation.status == "not_applicable")
        .count();
    let waived = criteria
        .iter()
        .filter(|obligation| obligation.status == "waived")
        .count();
    let risk_accepted = criteria
        .iter()
        .filter(|obligation| obligation.status == "risk_accepted")
        .count();
    let status = if packet.summary.status == "error" {
        "error"
    } else if fail > 0 {
        "fail"
    } else if needs_review > 0 || not_tested > 0 || waived > 0 || risk_accepted > 0 {
        "needs_review"
    } else {
        "pass"
    };
    ComplianceSummary {
        status: status.to_string(),
        total_obligations: criteria.len(),
        pass,
        fail,
        needs_review,
        not_tested,
        not_applicable,
        waived,
        risk_accepted,
        total_success_criteria: criteria.len(),
        total_supporting_checks: supporting_check_count,
        evidence_packet_status: packet.summary.status.clone(),
    }
}

fn compliance_surface_report(
    surface: &ProductSurface,
    packet: &EvidencePacket,
    criteria: &[ComplianceObligation],
    cells: &[CriterionCoverageCell],
) -> ComplianceSurfaceReport {
    let criterion_ids = criteria
        .iter()
        .filter(|obligation| obligation.surfaces.contains(&surface.id))
        .map(|obligation| obligation.id.clone())
        .collect::<Vec<_>>();
    let cell_ids = cells
        .iter()
        .filter(|cell| cell.surface_id == surface.id)
        .map(|cell| cell.id.clone())
        .collect::<Vec<_>>();
    let state_ids = unique_strings(
        cells
            .iter()
            .filter(|cell| cell.surface_id == surface.id)
            .map(|cell| cell.state_id.clone()),
    );
    let finding_refs = packet
        .findings
        .iter()
        .filter(|finding| {
            surface.evidence_refs.contains(&finding.affected_state)
                || surface.routes.contains(&finding.affected_route)
        })
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();
    let status = if cells
        .iter()
        .filter(|cell| cell.surface_id == surface.id)
        .any(|cell| cell.status == "fail")
    {
        "fail"
    } else if cells
        .iter()
        .filter(|cell| cell.surface_id == surface.id)
        .any(|cell| {
            matches!(
                cell.status.as_str(),
                "needs_review" | "not_tested" | "waived" | "risk_accepted"
            )
        })
    {
        "needs_review"
    } else {
        "pass"
    };
    ComplianceSurfaceReport {
        surface_id: surface.id.clone(),
        title: surface.title.clone(),
        routes: surface.routes.clone(),
        states: state_ids,
        status: status.to_string(),
        criteria: criterion_ids,
        cells: cell_ids,
        finding_refs,
    }
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        if !value.trim().is_empty() && seen.insert(value.clone()) {
            output.push(value);
        }
    }
    output
}

fn render_compliance_report(report: &ComplianceReportPacket) -> String {
    let criteria = report
        .criteria
        .iter()
        .map(|obligation| {
            format!(
                "<tr class=\"status-{status}\"><td><code>{id}</code><br>{title}</td><td>{status}</td><td>{why}<details><summary>Evidence</summary><p>Surfaces: <code>{surfaces}</code></p><p>Tests: <code>{tests}</code></p><p>Artifacts: <code>{artifacts}</code></p><p>Agentic context: <code>{agentic}</code></p><p>Human review: {human}</p></details></td></tr>",
                status = escape_html(&obligation.status),
                id = escape_html(&obligation.id),
                title = escape_html(&obligation.title),
                why = escape_html(&obligation.why),
                surfaces = escape_html(&obligation.surfaces.join(", ")),
                tests = escape_html(&obligation.tests.join(", ")),
                artifacts = escape_html(&obligation.artifact_refs.join(", ")),
                agentic = escape_html(&obligation.agentic_context.join(", ")),
                human = escape_html(&obligation.human_review)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let cells = report
        .criterion_coverage
        .iter()
        .map(|cell| {
            format!(
                "<tr class=\"status-{status}\"><td><code>{criterion}</code><br>Surface <code>{surface}</code><br>State <code>{state}</code></td><td>{status}</td><td>{applicability}</td><td>{method}</td><td>{confidence}</td><td><details><summary>criterion -> surface -> state -> finding -> artifact -> replay command</summary><p>Evidence refs: <code>{evidence}</code></p><p>Findings: <code>{findings}</code></p><p>Artifacts: <code>{artifacts}</code></p><p>Tests: <code>{tests}</code></p><p>Agentic refs: <code>{agentic}</code></p><p>Waivers: <code>{waivers}</code></p><p>Replay command: <code>{replay}</code></p><p>Residual review: {residual}</p></details></td></tr>",
                status = escape_html(&cell.status),
                criterion = escape_html(&cell.criterion_id),
                surface = escape_html(&cell.surface_id),
                state = escape_html(&cell.state_id),
                applicability = escape_html(&cell.applicability),
                method = escape_html(&cell.method),
                confidence = escape_html(&cell.confidence),
                evidence = escape_html(&cell.evidence_refs.join(", ")),
                findings = escape_html(&cell.finding_refs.join(", ")),
                artifacts = escape_html(&cell.artifact_refs.join(", ")),
                tests = escape_html(&cell.test_refs.join(", ")),
                agentic = escape_html(&cell.agentic_refs.join(", ")),
                waivers = escape_html(&cell.waiver_refs.join(", ")),
                replay = escape_html(cell.replay_command.as_deref().unwrap_or("")),
                residual = escape_html(&cell.residual_review_need)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let supporting_checks = report
        .supporting_checks
        .iter()
        .map(|check| {
            format!(
                "<tr class=\"status-{status}\"><td><code>{id}</code><br>{title}</td><td>{status}</td><td>{why}<details><summary>Related Criteria And Evidence</summary><p>Related criteria: <code>{criteria}</code></p><p>Surfaces: <code>{surfaces}</code></p><p>Tests: <code>{tests}</code></p><p>Artifacts: <code>{artifacts}</code></p><p>Agentic context: <code>{agentic}</code></p><p>Human review: {human}</p></details></td></tr>",
                status = escape_html(&check.status),
                id = escape_html(&check.id),
                title = escape_html(&check.title),
                why = escape_html(&check.why),
                criteria = escape_html(&check.related_criteria.join(", ")),
                surfaces = escape_html(&check.surfaces.join(", ")),
                tests = escape_html(&check.tests.join(", ")),
                artifacts = escape_html(&check.artifact_refs.join(", ")),
                agentic = escape_html(&check.agentic_context.join(", ")),
                human = escape_html(&check.human_review)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let surfaces = report
        .surfaces
        .iter()
        .map(|surface| {
            format!(
                "<li><strong>{}</strong> <code>{}</code><br>Status: {}. Routes: <code>{}</code>. Findings: <code>{}</code>.</li>",
                escape_html(&surface.title),
                escape_html(&surface.surface_id),
                escape_html(&surface.status),
                escape_html(&surface.routes.join(", ")),
                escape_html(&surface.finding_refs.join(", "))
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
  <title>Allie WCAG Evidence Report</title>
  <style>
    body {{ margin: 0; color: #151719; background: #f5f7fa; font: 16px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
    main {{ width: min(100% - 40px, 1180px); margin: 0 auto; padding: 40px 0; }}
    h1 {{ margin: 0 0 8px; font-size: 42px; line-height: 1.05; letter-spacing: 0; }}
    h2 {{ margin: 0 0 12px; color: #58616c; font-size: 13px; letter-spacing: 0.08em; text-transform: uppercase; }}
    section {{ background: #fff; border: 1px solid #d7dde5; margin-top: 18px; padding: 20px; }}
    table {{ width: 100%; border-collapse: collapse; }}
    th, td {{ border-bottom: 1px solid #d7dde5; padding: 10px; text-align: left; vertical-align: top; }}
    th {{ color: #58616c; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; }}
    code {{ background: #edf1f6; padding: 0.08em 0.28em; border-radius: 4px; }}
    details {{ margin-top: 8px; }}
    .summary {{ display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); gap: 1px; background: #d7dde5; border: 1px solid #d7dde5; margin-top: 22px; }}
    .summary div {{ background: #fff; padding: 14px; }}
    .label {{ color: #58616c; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; }}
    .value {{ font-size: 24px; font-weight: 700; margin: 0; }}
    .status-fail td:first-child {{ border-left: 4px solid #b42318; }}
    .status-needs_review td:first-child, .status-not_tested td:first-child {{ border-left: 4px solid #b7791f; }}
    .status-pass td:first-child {{ border-left: 4px solid #2f855a; }}
    @media (max-width: 760px) {{ main {{ width: min(100% - 24px, 1180px); }} .summary {{ grid-template-columns: 1fr 1fr; }} table {{ display: block; overflow-x: auto; }} }}
  </style>
</head>
<body>
  <main>
    <p class="label">Allie WCAG evidence report, not a legal compliance guarantee</p>
    <h1>{app_name}</h1>
    <p>Source packet <code>{packet}</code>. Source map <code>{map}</code>.</p>
    <div class="summary" aria-label="Compliance evidence summary">
      <div><p class="label">Status</p><p class="value">{status}</p></div>
      <div><p class="label">Pass</p><p class="value">{pass}</p></div>
      <div><p class="label">Fail</p><p class="value">{fail}</p></div>
      <div><p class="label">Review</p><p class="value">{review}</p></div>
      <div><p class="label">Not Tested</p><p class="value">{not_tested}</p></div>
      <div><p class="label">Waived</p><p class="value">{waived}</p></div>
      <div><p class="label">Risk</p><p class="value">{risk_accepted}</p></div>
      <div><p class="label">Total</p><p class="value">{total}</p></div>
    </div>
    <section>
      <h2>Surfaces</h2>
      <ul>{surfaces}</ul>
    </section>
    <section>
      <h2>WCAG 2.2 A/AA Success Criteria</h2>
      <table>
        <thead><tr><th>Requirement</th><th>Status</th><th>Why And Evidence</th></tr></thead>
        <tbody>{criteria}</tbody>
      </table>
    </section>
    <section>
      <h2>Criterion Coverage Matrix</h2>
      <table>
        <thead><tr><th>Criterion / Surface / State</th><th>Status</th><th>Applicability</th><th>Method</th><th>Confidence</th><th>Drilldown</th></tr></thead>
        <tbody>{cells}</tbody>
      </table>
    </section>
    <section>
      <h2>Supporting Checks</h2>
      <table>
        <thead><tr><th>Check</th><th>Status</th><th>Why And Evidence</th></tr></thead>
        <tbody>{supporting_checks}</tbody>
      </table>
    </section>
  </main>
</body>
</html>
"#,
        app_name = escape_html(&report.app_name),
        packet = escape_html(&report.source_packet),
        map = escape_html(&report.source_map),
        status = escape_html(&report.summary.status),
        pass = report.summary.pass,
        fail = report.summary.fail,
        review = report.summary.needs_review,
        not_tested = report.summary.not_tested,
        waived = report.summary.waived,
        risk_accepted = report.summary.risk_accepted,
        total = report.summary.total_obligations,
        surfaces = surfaces,
        criteria = criteria,
        cells = cells,
        supporting_checks = supporting_checks
    )
}

fn render_compliance_summary(report: &ComplianceReportPacket) -> String {
    let failing = report
        .criteria
        .iter()
        .filter(|obligation| obligation.status == "fail")
        .map(|obligation| format!("- {}: {}", obligation.id, obligation.why))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# Allie WCAG Evidence Summary\n\nStatus: `{}`\n\nPass: {}. Fail: {}. Needs review: {}. Not tested: {}. Waived: {}. Risk accepted: {}. Total WCAG success criteria: {}. Supporting checks: {}.\n\nSource map: `{}`\nSource packet: `{}`\n\nThis report is evidence visibility for accessibility engineering review, not a legal compliance guarantee.\n\n## Failing Criteria\n\n{}\n",
        report.summary.status,
        report.summary.pass,
        report.summary.fail,
        report.summary.needs_review,
        report.summary.not_tested,
        report.summary.waived,
        report.summary.risk_accepted,
        report.summary.total_success_criteria,
        report.summary.total_supporting_checks,
        report.source_map,
        report.source_packet,
        if failing.is_empty() {
            "None.".to_string()
        } else {
            failing
        }
    )
}

fn discover_surfaces(
    manifest: &FlowManifest,
    manifest_path: &Path,
) -> Result<Vec<DiscoveredSurface>> {
    let mut surfaces = BTreeMap::new();
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
    }

    Ok(surfaces.into_values().collect())
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
    let lower = text.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(text[start..end].trim().to_string())
}

fn render_discovery_report(discovery: &DiscoveryPacket, flow_plan: &FlowPlanPacket) -> String {
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

fn run_review(options: ReviewOptions) -> Result<ReviewReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create review output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;
    let mut packet: EvidencePacket = read_json_file(&options.packet_path)?;
    validate_release_packet(&packet)?;

    let artifacts_dir = options.out_dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create review artifacts directory {}",
            artifacts_dir.display()
        ),
        source,
    })?;
    let prompt_path = artifacts_dir.join("model-prompt-review-1.txt");
    let response_path = artifacts_dir.join("model-response-review-1.json");
    let redaction_path = artifacts_dir.join("redaction-receipt-review-1.json");
    let prompt = format!(
        "Review Allie packet {} for WCAG criteria that need visual or contextual judgment. Return hypotheses only; do not claim legal compliance.",
        packet.run.id
    );
    write_string(&prompt_path, &(prompt.clone() + "\n"))?;
    let response = serde_json::json!({
        "schema": "allie.offline-model-response.v0",
        "provider": "offline-recorded",
        "model": "allie-vision-fixture",
        "finding": {
            "title": "Agentic visual review requested",
            "description": "Offline vision review recommends human confirmation for visual order, focus visibility, and label usefulness.",
            "standard_obligation": "wcag22-aa:2.4.7-focus-visible",
            "confidence": "agent_inferred"
        }
    });
    write_json_pretty(&response_path, &response)?;
    let redaction = serde_json::json!({
        "schema": "allie.redaction-receipt.v0",
        "status": "redacted",
        "source_packet": options.packet_path,
        "artifacts_reviewed": packet.artifacts.iter().map(|artifact| artifact.id.clone()).collect::<Vec<_>>(),
        "egress": "none-offline-recorded"
    });
    write_json_pretty(&redaction_path, &redaction)?;

    let artifact_policy = ArtifactPolicy {
        redaction_status: "redacted_by_receipt".to_string(),
        retention_class: "local_review".to_string(),
    };
    let timestamp = now_utc();
    let prompt_artifact = artifact_for_path(
        "model-prompt-review-1",
        "model_prompt",
        &options.out_dir,
        &prompt_path,
        None,
        "allie-model-gateway",
        &artifact_policy,
        timestamp,
    )?;
    let response_artifact = artifact_for_path(
        "model-response-review-1",
        "model_response",
        &options.out_dir,
        &response_path,
        None,
        "allie-model-gateway",
        &artifact_policy,
        timestamp,
    )?;
    let redaction_artifact = artifact_for_path(
        "redaction-receipt-review-1",
        "redaction_receipt",
        &options.out_dir,
        &redaction_path,
        None,
        "allie-model-gateway",
        &artifact_policy,
        timestamp,
    )?;
    packet.artifacts.extend([
        prompt_artifact.clone(),
        response_artifact.clone(),
        redaction_artifact.clone(),
    ]);
    packet.review.push(ReviewAttempt {
        id: "review-1".to_string(),
        provider: "offline-recorded".to_string(),
        model: "allie-vision-fixture".to_string(),
        prompt_artifact: prompt_artifact.id.clone(),
        response_artifact: response_artifact.id.clone(),
        redaction_receipt: redaction_artifact.id.clone(),
        status: "needs_review".to_string(),
        confidence: "agent_inferred".to_string(),
        promotion_state: "model_hypothesis".to_string(),
    });
    packet.findings.push(Finding {
        id: "agentic-review-1".to_string(),
        title: "Agentic visual review requested".to_string(),
        description: "Offline vision review recommends human confirmation for visual order, focus visibility, and label usefulness.".to_string(),
        evidence_class: "agentic".to_string(),
        standard_obligation: "wcag22-aa:2.4.7-focus-visible".to_string(),
        severity: "review".to_string(),
        status: "needs_review".to_string(),
        confidence: "agent_inferred".to_string(),
        source: "offline-recorded-vision-review".to_string(),
        affected_route: packet
            .coverage
            .routes_visited
            .first()
            .cloned()
            .unwrap_or_else(|| "run".to_string()),
        affected_state: packet
            .coverage
            .states_captured
            .first()
            .cloned()
            .unwrap_or_else(|| "run".to_string()),
        artifact_refs: vec![prompt_artifact.id, response_artifact.id, redaction_artifact.id],
        suggested_remediation: "Use the linked prompt/response as a review hypothesis; promote only after scripted reproduction or human attestation.".to_string(),
        replay_command: packet.replay.command.clone(),
    });

    let packet_path = options.out_dir.join("evidence-reviewed.json");
    let report_path = options.out_dir.join("review-report.html");
    write_json_pretty(&packet_path, &packet)?;
    write_string(&report_path, &render_review_report(&packet))?;
    Ok(ReviewReceipt {
        packet_path,
        report_path,
    })
}

fn run_remediate(options: RemediateOptions) -> Result<RemediationReceipt> {
    fs::create_dir_all(&options.out_dir).map_err(|source| AllieError::Io {
        context: format!(
            "create remediation output directory {}",
            options.out_dir.display()
        ),
        source,
    })?;
    let packet: EvidencePacket = read_json_file(&options.packet_path)?;
    validate_release_packet(&packet)?;
    let items = packet
        .findings
        .iter()
        .filter(|finding| finding.status == "fail" || finding.evidence_class == "agentic")
        .map(|finding| RemediationItem {
            id: format!("remediate-{}", finding.id),
            finding_refs: vec![finding.id.clone()],
            standard_obligation: finding.standard_obligation.clone(),
            affected_state: finding.affected_state.clone(),
            artifact_refs: finding.artifact_refs.clone(),
            source_hint: format!(
                "inspect route {} state {}",
                finding.affected_route, finding.affected_state
            ),
            suggested_fix: finding.suggested_remediation.clone(),
            confidence: finding.confidence.clone(),
            replay_command: finding.replay_command.clone(),
            policy_effect: if finding.evidence_class == "agentic" {
                "needs_review"
            } else {
                "blocks_release"
            }
            .to_string(),
        })
        .collect::<Vec<_>>();
    let queue = RemediationQueue {
        schema: "allie.remediation-queue.v0".to_string(),
        source_packet: options.packet_path.to_string_lossy().to_string(),
        items,
    };
    let ledger = serde_json::json!({
        "schema": "allie.action-ledger.v0",
        "source_packet": options.packet_path,
        "actions": [{
            "id": "remediation-queue-created",
            "kind": "queue",
            "status": "recorded",
            "requires_replay_before_close": true
        }]
    });
    let queue_path = options.out_dir.join("remediation-queue.json");
    let ledger_path = options.out_dir.join("action-ledger.json");
    let report_path = options.out_dir.join("remediation-report.html");
    let patch_plan_path = options.out_dir.join("patch-plan.md");
    write_json_pretty(&queue_path, &queue)?;
    write_json_pretty(&ledger_path, &ledger)?;
    write_string(&report_path, &render_remediation_report(&queue))?;
    write_string(&patch_plan_path, &render_patch_plan(&queue))?;
    Ok(RemediationReceipt {
        queue_path,
        ledger_path,
        report_path,
        patch_plan_path,
    })
}

fn render_review_report(packet: &EvidencePacket) -> String {
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Allie Agentic Review</title></head><body><main><h1>Agentic review</h1><p>Review attempts: {}</p><p>Model-only findings stay neutral until promoted by scripted proof or human attestation.</p></main></body></html>"#,
        packet.review.len()
    )
}

fn render_remediation_report(queue: &RemediationQueue) -> String {
    let items = queue
        .items
        .iter()
        .map(|item| {
            format!(
                "<li><strong>{}</strong><br>{}<br><code>{}</code></li>",
                escape_html(&item.standard_obligation),
                escape_html(&item.suggested_fix),
                escape_html(&item.replay_command)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Allie Remediation</title></head><body><main><h1>Remediation workbench</h1><ul>{}</ul><p>No patch should be applied without evidence refs and replay proof.</p></main></body></html>"#,
        items
    )
}

fn render_patch_plan(queue: &RemediationQueue) -> String {
    let mut output = String::from("# Allie Patch Plan\n\n");
    output.push_str("This is a reviewable remediation draft, not an applied patch. Apply changes only on a branch and rerun the replay command.\n\n");
    for item in &queue.items {
        output.push_str(&format!("## {}\n\n", item.id));
        output.push_str(&format!("- Findings: {}\n", item.finding_refs.join(", ")));
        output.push_str(&format!("- Obligation: {}\n", item.standard_obligation));
        output.push_str(&format!("- Source hint: {}\n", item.source_hint));
        output.push_str(&format!("- Suggested fix: {}\n", item.suggested_fix));
        output.push_str(&format!("- Replay: `{}`\n\n", item.replay_command));
    }
    output
}

struct ReleaseProjection {
    summary: serde_json::Value,
    github_check: serde_json::Value,
    exit_class: ExitClass,
}

fn project_release_decision(
    packet: &serde_json::Value,
    options: &ReleaseOptions,
) -> ReleaseProjection {
    let deterministic_failures = packet["summary"]["deterministic_failures"]
        .as_u64()
        .unwrap_or_default();
    let scripted_failures = packet["summary"]["scripted_failures"]
        .as_u64()
        .unwrap_or_default();
    let infrastructure_failures = packet["summary"]["infrastructure_failures"]
        .as_u64()
        .unwrap_or_default();
    let packet_status = packet["summary"]["status"].as_str().unwrap_or("error");
    let evidence_artifacts = packet["artifacts"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|artifact| artifact["type"].as_str())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let verdicts = packet["verdicts"].as_array().cloned().unwrap_or_default();
    let review_needed = verdicts
        .iter()
        .filter(|verdict| verdict["status"].as_str() == Some("needs_review"))
        .filter_map(|verdict| verdict["obligation"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let not_tested = verdicts
        .iter()
        .filter(|verdict| verdict["status"].as_str() == Some("not_tested"))
        .filter_map(|verdict| verdict["obligation"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let model_findings_non_blocking = packet["findings"]
        .as_array()
        .map(|findings| {
            findings
                .iter()
                .filter(|finding| finding["evidence_class"].as_str() == Some("agentic"))
                .count()
        })
        .unwrap_or_default();

    let captured_states = string_set_at(&packet["coverage"]["states_captured"]);
    let discovered_surfaces = string_set_at(&packet["coverage"]["surfaces_discovered"]);
    let missing_required_evidence = options
        .changed_surfaces
        .iter()
        .filter(|surface| {
            !captured_states.contains(surface.as_str())
                && !discovered_surfaces.contains(surface.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();

    let stale_evidence = packet_is_stale(packet, options.stale_after_days);
    let expired_waivers = expired_touched_waivers(packet, &options.changed_surfaces);
    let invalid_waivers = invalid_touched_waivers(packet, &options.changed_surfaces);
    let has_blocker = packet_status == "fail"
        || packet_status == "error"
        || deterministic_failures > 0
        || scripted_failures > 0
        || infrastructure_failures > 0
        || !missing_required_evidence.is_empty()
        || !expired_waivers.is_empty()
        || !invalid_waivers.is_empty();
    let status = if has_blocker {
        "blocked"
    } else if stale_evidence
        || !review_needed.is_empty()
        || !not_tested.is_empty()
        || model_findings_non_blocking > 0
    {
        "needs_review"
    } else {
        "approved"
    };
    let conclusion = if has_blocker {
        "failure"
    } else if status == "needs_review" {
        "neutral"
    } else {
        "success"
    };
    let exit_class = if has_blocker {
        ExitClass::BlockingFinding
    } else {
        ExitClass::Success
    };

    let summary = serde_json::json!({
        "schema": "allie.release-decision.v0",
        "status": status,
        "packet_path": options.packet_path.to_string_lossy(),
        "packet_run_id": packet["run"]["id"].as_str().unwrap_or("unknown"),
        "changed_surfaces": options.changed_surfaces,
        "blocking": {
            "deterministic_failures": deterministic_failures,
            "scripted_failures": scripted_failures,
            "infrastructure_failures": infrastructure_failures,
            "missing_required_evidence": missing_required_evidence,
            "expired_waivers": expired_waivers,
            "invalid_waivers": invalid_waivers
        },
        "review": {
            "stale_evidence": stale_evidence
        },
        "review_needed_obligations": review_needed,
        "not_tested_obligations": not_tested,
        "model_findings_non_blocking": model_findings_non_blocking,
        "evidence_artifacts": evidence_artifacts,
        "policy": {
            "model_status": packet["policy"]["model_status"].clone(),
            "model_provider_allowlist": packet["policy"]["model_provider_allowlist"].clone(),
            "zdr_required": packet["policy"]["zdr_required"].clone()
        }
    });
    let summary_text = release_summary_text(&summary);
    let github_check = serde_json::json!({
        "name": "Allie accessibility evidence",
        "conclusion": conclusion,
        "output": {
            "title": format!("Allie release decision: {status}"),
            "summary": summary_text,
            "text": summary_text
        }
    });

    ReleaseProjection {
        summary,
        github_check,
        exit_class,
    }
}

fn string_set_at(value: &serde_json::Value) -> BTreeSet<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn packet_is_stale(packet: &serde_json::Value, stale_after_days: i64) -> bool {
    let Some(finished_at) = packet["run"]["finished_at"].as_str() else {
        return true;
    };
    let Ok(finished_at) = DateTime::parse_from_rfc3339(finished_at) else {
        return true;
    };
    let age = Utc::now().signed_duration_since(finished_at.with_timezone(&Utc));
    age.num_days() > stale_after_days
}

fn expired_touched_waivers(
    packet: &serde_json::Value,
    changed_surfaces: &[String],
) -> Vec<serde_json::Value> {
    let changed = changed_surfaces.iter().cloned().collect::<BTreeSet<_>>();
    packet["waivers"]
        .as_array()
        .map(|waivers| {
            waivers
                .iter()
                .filter(|waiver| waiver_is_expired_for_changed_surface(waiver, &changed))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn waiver_is_expired_for_changed_surface(
    waiver: &serde_json::Value,
    changed_surfaces: &BTreeSet<String>,
) -> bool {
    if !waiver_touches_changed_surface(waiver, changed_surfaces) {
        return false;
    }
    let Some(expires_at) = waiver["expires_at"].as_str() else {
        return false;
    };
    let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
        return true;
    };
    if expires_at.with_timezone(&Utc) >= Utc::now() {
        return false;
    }
    true
}

fn invalid_touched_waivers(
    packet: &serde_json::Value,
    changed_surfaces: &[String],
) -> Vec<serde_json::Value> {
    let changed = changed_surfaces.iter().cloned().collect::<BTreeSet<_>>();
    packet["waivers"]
        .as_array()
        .map(|waivers| {
            waivers
                .iter()
                .filter(|waiver| {
                    waiver_touches_changed_surface(waiver, &changed)
                        && !waiver_has_required_release_metadata(waiver)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn waiver_touches_changed_surface(
    waiver: &serde_json::Value,
    changed_surfaces: &BTreeSet<String>,
) -> bool {
    if changed_surfaces.is_empty() {
        return true;
    }
    let Some(surface) = waiver["surface"].as_str() else {
        return true;
    };
    surface.trim().is_empty() || changed_surfaces.contains(surface)
}

fn waiver_has_required_release_metadata(waiver: &serde_json::Value) -> bool {
    let Some(surface) = waiver["surface"].as_str() else {
        return false;
    };
    if surface.trim().is_empty() {
        return false;
    }
    let Some(status) = waiver["status"].as_str() else {
        return false;
    };
    if !matches!(status, "waived" | "risk_accepted") {
        return false;
    }
    let Some(expires_at) = waiver["expires_at"].as_str() else {
        return false;
    };
    if DateTime::parse_from_rfc3339(expires_at).is_err() {
        return false;
    }
    let provenance_ok = waiver["provenance"]
        .as_str()
        .map(|value| !value.trim().is_empty())
        .or_else(|| {
            waiver["provenance"]
                .as_object()
                .map(|value| !value.is_empty())
        })
        .unwrap_or(false);
    let packet_ref_ok = waiver["packet_ref"]
        .as_str()
        .map(|value| !value.trim().is_empty())
        .or_else(|| {
            waiver["packet_refs"].as_array().map(|values| {
                values
                    .iter()
                    .any(|value| value.as_str().is_some_and(|item| !item.trim().is_empty()))
            })
        })
        .unwrap_or(false);

    provenance_ok && packet_ref_ok
}

fn release_summary_text(summary: &serde_json::Value) -> String {
    format!(
        "status={} deterministic_failures={} scripted_failures={} infrastructure_failures={} review_needed={} not_tested={}",
        summary["status"].as_str().unwrap_or("unknown"),
        summary["blocking"]["deterministic_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["blocking"]["scripted_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["blocking"]["infrastructure_failures"]
            .as_u64()
            .unwrap_or_default(),
        summary["review_needed_obligations"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default(),
        summary["not_tested_obligations"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default()
    )
}

fn render_release_report(summary: &serde_json::Value) -> String {
    let text = escape_html(&release_summary_text(summary));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Allie Release Decision</title>
  <style>
    body {{ margin: 0; font: 16px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; color: #151719; background: #f5f7fa; }}
    main {{ width: min(100% - 40px, 900px); margin: 0 auto; padding: 40px 0; }}
    section {{ background: #fff; border: 1px solid #d7dde5; padding: 20px; margin-top: 18px; }}
    h1 {{ margin: 0; font-size: 42px; line-height: 1.05; letter-spacing: 0; }}
    h2 {{ margin: 0 0 10px; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: #58616c; }}
  </style>
</head>
<body>
  <main>
    <h1>Allie release decision: {status}</h1>
    <section>
      <h2>Evidence Projection</h2>
      <p>{text}</p>
      <p>This is a projection of evidence packets, not a legal compliance guarantee and not a global score.</p>
    </section>
  </main>
</body>
</html>
"#,
        status = escape_html(summary["status"].as_str().unwrap_or("unknown")),
        text = text
    )
}

fn read_release_packet(packet_path: &Path) -> Result<serde_json::Value> {
    let packet_text = fs::read_to_string(packet_path).map_err(|source| AllieError::Io {
        context: format!("read evidence packet {}", packet_path.display()),
        source,
    })?;
    let packet = serde_json::from_str::<EvidencePacket>(&packet_text).map_err(|source| {
        AllieError::Json {
            context: format!("parse evidence packet {}", packet_path.display()),
            source,
        }
    })?;
    validate_release_packet(&packet)?;
    serde_json::to_value(packet).map_err(|source| AllieError::Json {
        context: format!("normalize evidence packet {}", packet_path.display()),
        source,
    })
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|source| AllieError::Io {
        context: format!("read json {}", path.display()),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| AllieError::Json {
        context: format!("parse json {}", path.display()),
        source,
    })
}

fn validate_release_packet(packet: &EvidencePacket) -> Result<()> {
    if packet.schema != EVIDENCE_SCHEMA {
        return Err(AllieError::InvalidManifest(format!(
            "invalid evidence packet schema {}; expected {EVIDENCE_SCHEMA}",
            packet.schema
        )));
    }

    if !matches!(packet.summary.status.as_str(), "pass" | "fail" | "error") {
        return Err(AllieError::InvalidManifest(format!(
            "invalid evidence packet status {}; expected pass, fail, or error",
            packet.summary.status
        )));
    }

    Ok(())
}

fn read_worker_response(response_path: &Path) -> Result<WorkerResponse> {
    let response_text = match fs::read_to_string(response_path) {
        Ok(text) => text,
        Err(source) => {
            return Ok(WorkerResponse::error(format!(
                "worker partial-write: read response {}: {source}",
                response_path.display()
            )));
        }
    };

    match serde_json::from_str::<WorkerResponse>(&response_text) {
        Ok(response) => Ok(response),
        Err(source) => Ok(WorkerResponse::error(format!(
            "worker partial-write: parse response {}: {source}",
            response_path.display()
        ))),
    }
}

fn invoke_worker(
    request_path: &Path,
    response_path: &Path,
    timeout_ms: u64,
) -> std::result::Result<(), RunFailure> {
    let worker_script = std::env::var_os("ALLIE_BROWSER_WORKER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workers/browser/run.mjs")
        });

    if !worker_script.exists() {
        return Err(RunFailure::new(
            "worker-missing",
            "worker-adapter",
            format!("worker script not found at {}", worker_script.display()),
        ));
    }

    let mut child = Command::new("node")
        .arg(&worker_script)
        .arg("--request")
        .arg(request_path)
        .arg("--response")
        .arg(response_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| {
            RunFailure::new(
                "worker-spawn-failed",
                "worker-adapter",
                format!("spawn worker {}: {source}", worker_script.display()),
            )
        })?;

    match child
        .wait_timeout(Duration::from_millis(timeout_ms))
        .map_err(|source| {
            RunFailure::new(
                "worker-wait-failed",
                "worker-adapter",
                format!("wait for worker {}: {source}", worker_script.display()),
            )
        })? {
        Some(status) => {
            let output = child.wait_with_output().map_err(|source| {
                RunFailure::new(
                    "worker-output-failed",
                    "worker-adapter",
                    format!(
                        "collect worker output {}: {source}",
                        worker_script.display()
                    ),
                )
            })?;
            if !status.success() {
                return Err(RunFailure::new(
                    "worker-crash",
                    "worker-adapter",
                    format!(
                        "{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                ));
            }
        }
        None => {
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|source| {
                RunFailure::new(
                    "worker-timeout",
                    "worker-adapter",
                    format!("worker timed out after {timeout_ms} ms and output collection failed: {source}"),
                )
            })?;
            return Err(RunFailure::new(
                "worker-timeout",
                "worker-adapter",
                format!(
                    "worker timed out after {timeout_ms} ms\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FlowManifest {
    id: String,
    name: String,
    app_name: String,
    environment: String,
    auth_profile: Option<String>,
    #[serde(default)]
    credentials: CredentialConfig,
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

        if self.model.enabled && self.model.provider_allowlist.is_empty() {
            failures.push(RunFailure::new(
                "model-policy-incomplete",
                "model-policy",
                "model calls are enabled but provider_allowlist is empty".to_string(),
            ));
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
struct ManifestTarget {
    kind: String,
    fixture_dir: Option<PathBuf>,
    base_url: Option<String>,
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
struct ArtifactPolicy {
    redaction_status: String,
    retention_class: String,
}

impl Default for ArtifactPolicy {
    fn default() -> Self {
        Self {
            redaction_status: "not_redacted_local_fixture".to_string(),
            retention_class: "local_ephemeral".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModelPolicy {
    enabled: bool,
    provider_allowlist: Vec<String>,
    zdr_required: bool,
}

impl Default for ModelPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_allowlist: Vec::new(),
            zdr_required: true,
        }
    }
}

impl ModelPolicy {
    fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BrowserSettings {
    viewport: Viewport,
    color_scheme: String,
    reduced_motion: String,
    locale: String,
    zoom: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Viewport {
    width: u32,
    height: u32,
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

#[derive(Debug, Serialize)]
struct WorkerRequest {
    schema: &'static str,
    run_id: String,
    manifest_id: String,
    target: WorkerTarget,
    browser: BrowserSettings,
    states: Vec<ManifestState>,
    artifacts_dir: String,
}

impl WorkerRequest {
    fn from_manifest(
        run_id: &str,
        manifest: &FlowManifest,
        manifest_path: &Path,
        artifacts_dir: &Path,
    ) -> Result<Self> {
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let target = WorkerTarget {
            kind: manifest.target.kind.clone(),
            fixture_dir: manifest
                .target
                .fixture_dir
                .as_ref()
                .map(|path| normalize_relative(manifest_dir, path)),
            base_url: manifest.target.base_url.clone(),
        };

        Ok(Self {
            schema: WORKER_REQUEST_SCHEMA,
            run_id: run_id.to_string(),
            manifest_id: manifest.id.clone(),
            target,
            browser: manifest.browser.clone(),
            states: manifest.flow.states.clone(),
            artifacts_dir: artifacts_dir.to_string_lossy().to_string(),
        })
    }
}

fn normalize_relative(base: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        base.join(path).to_string_lossy().to_string()
    }
}

#[derive(Debug, Serialize)]
struct WorkerTarget {
    kind: String,
    fixture_dir: Option<String>,
    base_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryPacket {
    schema: String,
    run: DiscoveryRun,
    target: ManifestTarget,
    browser: BrowserSettings,
    promotion: DiscoveryPromotion,
    surfaces: Vec<DiscoveredSurface>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FlowPlanPacket {
    schema: String,
    source_discovery: String,
    flow_id: String,
    candidates: Vec<FlowCandidate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FlowCandidate {
    id: String,
    path: String,
    description: String,
    promotion_state: String,
    required: bool,
    axe: bool,
    screenshot: bool,
    dom_snapshot: bool,
    accessibility_tree: bool,
    keyboard: bool,
    video: bool,
    trace: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductMapPacket {
    schema: String,
    generated_at: String,
    source_manifest: String,
    project_root: String,
    app_name: String,
    environment: String,
    policy_profile: String,
    target: ManifestTarget,
    agent: AgentRunnerReceiptPacket,
    standards: StandardsProfileSummary,
    surfaces: Vec<ProductSurface>,
    workflows: Vec<ProductWorkflow>,
    open_questions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentRunnerReceiptPacket {
    schema: String,
    runner: String,
    mode: String,
    status: String,
    capabilities: Vec<String>,
    command: Vec<String>,
    prompt_path: Option<String>,
    transcript_path: Option<String>,
    warnings: Vec<String>,
    sources: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchJobPacket {
    schema: String,
    id: String,
    status: String,
    current_step: String,
    created_at: String,
    updated_at: String,
    finished_at: Option<String>,
    manifest_path: String,
    project_root: String,
    runtime_policy: WorkbenchRuntimePolicy,
    runner: WorkbenchRunnerState,
    steps: Vec<WorkbenchStepRecord>,
    pointers: WorkbenchPointers,
    artifacts: Vec<WorkbenchArtifactRef>,
    resumable: bool,
    cancel_requested: bool,
    resume_count: u32,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchRuntimePolicy {
    max_runtime_ms: u64,
    idle_timeout_ms: u64,
    agent_step_timeout_ms: Option<u64>,
    worker_timeout_ms: u64,
    ci_mode: bool,
    enforcement_note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchRunnerState {
    kind: String,
    adapter_mode: String,
    resume_contract: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchStepRecord {
    id: String,
    status: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    receipt_path: Option<String>,
    exit_code: Option<i32>,
    message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct WorkbenchPointers {
    discovery: Option<String>,
    flow_plan: Option<String>,
    generated_flow: Option<String>,
    product_map: Option<String>,
    surface_map: Option<String>,
    evidence_packet: Option<String>,
    evidence_report: Option<String>,
    compliance_report: Option<String>,
    compliance_html: Option<String>,
    reviewed_packet: Option<String>,
    review_report: Option<String>,
    remediation_queue: Option<String>,
    release_summary: Option<String>,
    release_report: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchArtifactRef {
    kind: String,
    path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StandardsProfileSummary {
    id: String,
    source_urls: Vec<String>,
    total_obligations: usize,
    methods: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductSurface {
    id: String,
    title: String,
    routes: Vec<String>,
    files: Vec<String>,
    services: Vec<String>,
    user_stories: Vec<String>,
    workflow_refs: Vec<String>,
    evidence_refs: Vec<String>,
    confidence: String,
    review_status: String,
    provenance: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProductWorkflow {
    id: String,
    title: String,
    surface_refs: Vec<String>,
    user_story: String,
    generated_flow_manifest: String,
    states: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplianceReportPacket {
    schema: String,
    generated_at: String,
    source_map: String,
    source_packet: String,
    app_name: String,
    summary: ComplianceSummary,
    criteria: Vec<ComplianceObligation>,
    criterion_coverage: Vec<CriterionCoverageCell>,
    supporting_checks: Vec<ComplianceSupportingCheck>,
    obligations: Vec<ComplianceObligation>,
    surfaces: Vec<ComplianceSurfaceReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplianceSummary {
    status: String,
    total_obligations: usize,
    pass: usize,
    fail: usize,
    needs_review: usize,
    not_tested: usize,
    not_applicable: usize,
    waived: usize,
    risk_accepted: usize,
    total_success_criteria: usize,
    total_supporting_checks: usize,
    evidence_packet_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplianceObligation {
    id: String,
    title: String,
    status: String,
    why: String,
    surfaces: Vec<String>,
    tests: Vec<String>,
    artifact_refs: Vec<String>,
    agentic_context: Vec<String>,
    human_review: String,
    confidence: String,
    evidence_class: String,
    source_url: Option<String>,
    finding_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CriterionCoverageCell {
    id: String,
    criterion_id: String,
    surface_id: String,
    state_id: String,
    policy_profile: String,
    status: String,
    applicability: String,
    method: String,
    confidence: String,
    evidence_refs: Vec<String>,
    agentic_refs: Vec<String>,
    waiver_refs: Vec<String>,
    finding_refs: Vec<String>,
    artifact_refs: Vec<String>,
    test_refs: Vec<String>,
    replay_command: Option<String>,
    residual_review_need: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplianceSupportingCheck {
    id: String,
    title: String,
    status: String,
    why: String,
    related_criteria: Vec<String>,
    surfaces: Vec<String>,
    tests: Vec<String>,
    artifact_refs: Vec<String>,
    agentic_context: Vec<String>,
    human_review: String,
    confidence: String,
    evidence_class: String,
    finding_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplianceSurfaceReport {
    surface_id: String,
    title: String,
    routes: Vec<String>,
    states: Vec<String>,
    status: String,
    criteria: Vec<String>,
    cells: Vec<String>,
    finding_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    schema: String,
    status: WorkerRunStatus,
    actual_base_url: Option<String>,
    #[serde(default)]
    states: Vec<WorkerStateResult>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    nondeterminism: Vec<String>,
}

impl WorkerResponse {
    fn validate(&self) -> Result<()> {
        if self.schema != WORKER_RESPONSE_SCHEMA {
            return Err(AllieError::Worker(format!(
                "unexpected worker response schema {}",
                self.schema
            )));
        }
        Ok(())
    }
}

impl WorkerResponse {
    fn error(message: String) -> Self {
        Self {
            schema: WORKER_RESPONSE_SCHEMA.to_string(),
            status: WorkerRunStatus::Error,
            actual_base_url: None,
            states: Vec::new(),
            errors: vec![message],
            nondeterminism: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkerRunStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Deserialize)]
struct WorkerStateResult {
    id: String,
    route: String,
    url: String,
    title: String,
    http_status: Option<u16>,
    screenshot_path: Option<String>,
    axe_json_path: Option<String>,
    #[serde(default)]
    dom_snapshot_path: Option<String>,
    #[serde(default)]
    accessibility_tree_path: Option<String>,
    #[serde(default)]
    video_path: Option<String>,
    #[serde(default)]
    trace_path: Option<String>,
    #[serde(default)]
    keyboard_focus_order: Vec<String>,
    #[serde(default)]
    axe_violations: Vec<AxeViolation>,
    #[serde(default)]
    console_errors: Vec<String>,
    #[serde(default)]
    network_errors: Vec<String>,
    #[serde(default)]
    state_errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct AxeViolation {
    id: String,
    impact: Option<String>,
    help: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    nodes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidencePacket {
    schema: String,
    summary: PacketSummary,
    run: RunMetadata,
    target: TargetMetadata,
    policy: PolicyMetadata,
    coverage: Coverage,
    artifacts: Vec<ArtifactMetadata>,
    findings: Vec<Finding>,
    verdicts: Vec<Verdict>,
    waivers: Vec<serde_json::Value>,
    review: Vec<ReviewAttempt>,
    replay: Replay,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PacketSummary {
    status: String,
    exit_code: i32,
    deterministic_failures: usize,
    scripted_failures: usize,
    infrastructure_failures: usize,
    states_captured: usize,
    failure_class: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunMetadata {
    id: String,
    started_at: String,
    finished_at: String,
    allie_version: String,
    git_sha: String,
    git_branch: String,
    ci_provider: Option<String>,
    actor: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TargetMetadata {
    base_url: Option<String>,
    environment: String,
    app_name: String,
    auth_profile: String,
    credential_provider: CredentialProviderMetadata,
    flow_manifest: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredentialProviderMetadata {
    provider: String,
    env: Option<String>,
    required: bool,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyMetadata {
    profile: String,
    blocking_classes: Vec<String>,
    worker_timeout_ms: u64,
    model_provider_allowlist: Vec<String>,
    model_status: String,
    zdr_required: bool,
    redaction_profile: String,
    budget: PolicyBudget,
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyBudget {
    model_calls: u32,
    max_states: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Coverage {
    routes_visited: Vec<String>,
    surfaces_discovered: Vec<String>,
    flows_exercised: Vec<String>,
    states_captured: Vec<String>,
    state_metadata: Vec<StateMetadata>,
    standards_obligations_evaluated: Vec<String>,
    obligations_not_tested: Vec<String>,
    obligations_requiring_human_review: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateMetadata {
    id: String,
    route: String,
    url: String,
    title: String,
    http_status: Option<u16>,
    keyboard_focus_order: Vec<String>,
    console_errors: Vec<String>,
    network_errors: Vec<String>,
    state_errors: Vec<String>,
}

#[derive(Debug)]
struct ContractFailure {
    state_id: String,
    route: String,
    message: String,
}

#[derive(Clone, Debug)]
struct RunFailure {
    kind: String,
    source: String,
    message: String,
}

impl RunFailure {
    fn new(kind: &str, source: &str, message: String) -> Self {
        Self {
            kind: kind.to_string(),
            source: source.to_string(),
            message,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactMetadata {
    id: String,
    #[serde(rename = "type")]
    artifact_type: String,
    path: String,
    hash: String,
    redaction_status: String,
    retention_class: String,
    unavailable_reason: Option<String>,
    related_flow_state: Option<String>,
    creation_tool: String,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Finding {
    id: String,
    title: String,
    description: String,
    evidence_class: String,
    standard_obligation: String,
    severity: String,
    status: String,
    confidence: String,
    source: String,
    affected_route: String,
    affected_state: String,
    artifact_refs: Vec<String>,
    suggested_remediation: String,
    replay_command: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Verdict {
    obligation: String,
    status: String,
    confidence: String,
    evidence_class: String,
    source: String,
    affected_states: Vec<String>,
    finding_refs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReviewAttempt {
    id: String,
    provider: String,
    model: String,
    prompt_artifact: String,
    response_artifact: String,
    redaction_receipt: String,
    status: String,
    confidence: String,
    promotion_state: String,
}

#[derive(Debug, Serialize)]
struct RemediationQueue {
    schema: String,
    source_packet: String,
    items: Vec<RemediationItem>,
}

#[derive(Debug, Serialize)]
struct RemediationItem {
    id: String,
    finding_refs: Vec<String>,
    standard_obligation: String,
    affected_state: String,
    artifact_refs: Vec<String>,
    source_hint: String,
    suggested_fix: String,
    confidence: String,
    replay_command: String,
    policy_effect: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    command: String,
    manifest_path: String,
    environment_requirements: Vec<String>,
    credential_profile: String,
    browser: BrowserSettings,
    seed_data: Vec<String>,
    known_nondeterminism: Vec<String>,
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

    let mut artifacts = worker_artifacts(out_dir, &response, &manifest.artifacts, finished_at)?;
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
            git_sha: git_metadata(&["rev-parse", "--short", "HEAD"]).unwrap_or_default(),
            git_branch: git_metadata(&["branch", "--show-current"]).unwrap_or_default(),
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
        review: Vec::new(),
        replay: Replay {
            command: replay_command,
            manifest_path: manifest_path.to_string_lossy().to_string(),
            environment_requirements: vec![
                "npm install".to_string(),
                "npx playwright install chromium".to_string(),
            ],
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

fn worker_artifacts(
    out_dir: &Path,
    response: &WorkerResponse,
    artifact_policy: &ArtifactPolicy,
    timestamp: DateTime<Utc>,
) -> Result<Vec<ArtifactMetadata>> {
    let mut artifacts = Vec::new();
    for state in &response.states {
        if let Some(path) = &state.axe_json_path {
            artifacts.push(artifact_for_path(
                &format!("axe-json-{}", state.id),
                "axe_json",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
        if let Some(path) = &state.screenshot_path {
            artifacts.push(artifact_for_path(
                &format!("screenshot-{}", state.id),
                "screenshot",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
        if let Some(path) = &state.dom_snapshot_path {
            artifacts.push(artifact_for_path(
                &format!("dom-snapshot-{}", state.id),
                "dom_snapshot",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
        if let Some(path) = &state.accessibility_tree_path {
            artifacts.push(artifact_for_path(
                &format!("accessibility-tree-{}", state.id),
                "accessibility_tree",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
        if let Some(path) = &state.video_path {
            artifacts.push(artifact_for_path(
                &format!("video-{}", state.id),
                "video",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
        if let Some(path) = &state.trace_path {
            artifacts.push(artifact_for_path(
                &format!("trace-{}", state.id),
                "trace",
                out_dir,
                &out_dir.join(path),
                Some(state.id.clone()),
                "playwright-axe-worker",
                artifact_policy,
                timestamp,
            )?);
        }
    }
    Ok(artifacts)
}

#[expect(
    clippy::too_many_arguments,
    reason = "artifact metadata hashing keeps path, policy, and provenance explicit at call sites"
)]
fn artifact_for_path(
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
                        .filter(|artifact| artifact.related_flow_state.as_deref() == Some(&state.id))
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
                        suggested_remediation: format!(
                            "Review axe rule {} in the linked raw axe JSON and rerun the replay command.",
                            violation.id
                        ),
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
                suggested_remediation:
                    "Fix the route or manifest path, then rerun the replay command.".to_string(),
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
            suggested_remediation:
                "Fix the worker response or manifest requirements, then rerun the replay command."
                    .to_string(),
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
            suggested_remediation:
                "Fix the run configuration or environment, then rerun the replay command."
                    .to_string(),
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
                suggested_remediation:
                    "Inspect worker-request.json and worker stderr, then rerun the replay command."
                        .to_string(),
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
            suggested_remediation:
                "Stabilize the fixture or mark known nondeterminism in the manifest before release use."
                    .to_string(),
            replay_command: replay_command.to_string(),
        });
    }

    findings
}

fn obligation_from_tags(policy_profile: &str, tags: &[String]) -> String {
    if policy_profile != "wcag22-aa" {
        return tags
            .iter()
            .find(|tag| tag.starts_with("wcag"))
            .cloned()
            .unwrap_or_else(|| format!("{policy_profile}:unmapped-axe-rule"));
    }

    let profile = wcag22_profile();
    let Some(map) = profile
        .get("axe_tag_map")
        .and_then(|value| value.as_object())
    else {
        return "wcag22-aa:unmapped-axe-rule".to_string();
    };

    let mut candidates = tags.iter().collect::<Vec<_>>();
    candidates.sort_by_key(|tag| std::cmp::Reverse(tag.len()));
    for tag in candidates {
        if let Some(obligation) = map
            .get(tag)
            .and_then(|value| value.get("obligation"))
            .and_then(|value| value.as_str())
        {
            return obligation.to_string();
        }
    }

    "wcag22-aa:unmapped-axe-rule".to_string()
}

fn deterministic_pass_obligation(policy_profile: &str) -> String {
    if policy_profile != "wcag22-aa" {
        return format!("{policy_profile}:deterministic-machine-checks");
    }

    wcag22_profile()
        .get("deterministic_pass_obligation")
        .and_then(|value| value.get("obligation"))
        .and_then(|value| value.as_str())
        .unwrap_or("wcag22-aa:deterministic-axe-rules")
        .to_string()
}

fn scripted_profile_obligations(policy_profile: &str) -> Vec<String> {
    let mut obligations = profile_obligation_list(policy_profile, "scripted_obligations");
    obligations.extend(criteria_with_method(policy_profile, "scripted"));
    obligations.sort();
    obligations.dedup();
    obligations
}

fn human_review_profile_obligations(policy_profile: &str) -> Vec<String> {
    let mut obligations = profile_obligation_list(policy_profile, "human_review_obligations");
    obligations.extend(criteria_with_method(policy_profile, "human_review"));
    obligations.sort();
    obligations.dedup();
    obligations
}

fn profile_obligation_list(policy_profile: &str, key: &str) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }

    wcag22_profile()
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("obligation").and_then(|value| value.as_str()))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn wcag22_profile() -> serde_json::Value {
    serde_json::from_str(WCAG22_AA_PROFILE_JSON).expect("embedded wcag22-aa profile is valid JSON")
}

fn criteria_with_method(policy_profile: &str, method: &str) -> Vec<String> {
    if policy_profile != "wcag22-aa" {
        return Vec::new();
    }
    wcag22_success_criteria()
        .into_iter()
        .filter(|criterion| criterion["method"].as_str() == Some(method))
        .filter_map(|criterion| criterion["obligation"].as_str().map(ToString::to_string))
        .collect()
}

fn wcag22_success_criteria() -> Vec<serde_json::Value> {
    wcag22_profile()
        .get("success_criteria")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

fn criterion_title(obligation: &str) -> String {
    wcag22_success_criteria()
        .into_iter()
        .find(|criterion| criterion["obligation"].as_str() == Some(obligation))
        .and_then(|criterion| {
            let num = criterion["num"].as_str()?;
            let handle = criterion["handle"].as_str()?;
            Some(format!("{num} {handle}"))
        })
        .or_else(|| profile_obligation_title(obligation))
        .unwrap_or_else(|| obligation.to_string())
}

fn profile_obligation_title(obligation: &str) -> Option<String> {
    let profile = wcag22_profile();
    if profile["deterministic_pass_obligation"]["obligation"].as_str() == Some(obligation) {
        return profile["deterministic_pass_obligation"]["title"]
            .as_str()
            .map(ToString::to_string);
    }
    ["scripted_obligations", "human_review_obligations"]
        .into_iter()
        .filter_map(|key| profile.get(key).and_then(|value| value.as_array()))
        .flat_map(|items| items.iter())
        .find(|item| item["obligation"].as_str() == Some(obligation))
        .and_then(|item| item["title"].as_str().map(ToString::to_string))
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
            let method = criterion["method"].as_str().unwrap_or("human_review");
            let keyboard_observed = response
                .states
                .iter()
                .any(|state| !state.keyboard_focus_order.is_empty());
            let (status, confidence, evidence_class, source) = match method {
                "axe" => (
                    "pass",
                    "machine_proven",
                    "deterministic",
                    "axe-core-success-criterion-tags",
                ),
                "scripted" if keyboard_observed && obligation.contains("keyboard") => (
                    "pass",
                    "script_observed",
                    "scripted",
                    "playwright-keyboard-traversal",
                ),
                "scripted" => (
                    "not_tested",
                    "script_observed",
                    "scripted",
                    "allie-obligation-profile",
                ),
                _ => (
                    "needs_review",
                    "requires_human_or_agent_review",
                    "human",
                    "allie-obligation-profile",
                ),
            };
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
                    status: "not_tested".to_string(),
                    confidence: "script_observed".to_string(),
                    evidence_class: "scripted".to_string(),
                    source: "allie-obligation-profile".to_string(),
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
                    status: "not_tested".to_string(),
                    confidence: "script_observed".to_string(),
                    evidence_class: "scripted".to_string(),
                    source: "allie-obligation-profile".to_string(),
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
    let needs_review = human_review_profile_obligations(&manifest.policy.profile);
    for obligation in not_tested.iter().chain(needs_review.iter()) {
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
            })
            .collect(),
        standards_obligations_evaluated: obligations.into_iter().collect(),
        obligations_not_tested: not_tested,
        obligations_requiring_human_review: needs_review,
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
            format!(
                "<li><strong>{}</strong> <span>{}</span><br><code>{}</code><br><span>HTTP status: {}; console errors: {}; network errors: {}; state errors: {}; keyboard stops: {}</span></li>",
                escape_html(&state.id),
                escape_html(&state.title),
                escape_html(&state.url),
                state.http_status
                    .map(|status| status.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                state.console_errors.len(),
                state.network_errors.len(),
                state.state_errors.len(),
                state.keyboard_focus_order.len()
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
        review_needs = escape_html(
            &packet
                .coverage
                .obligations_requiring_human_review
                .join(", ")
        ),
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

fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(|source| AllieError::Json {
        context: format!("serialize json {}", path.display()),
        source,
    })?;
    write_string(path, &(json + "\n"))
}

fn write_string_atomic(path: &Path, contents: &str) -> Result<()> {
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

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn new_run_id() -> String {
    let millis = current_time_millis();
    format!("run-{millis}")
}

fn new_job_id() -> String {
    let millis = current_time_millis();
    format!("job-{millis}")
}

fn git_metadata(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
        )
        .unwrap();

        assert_eq!(receipt.exit_class, ExitClass::Success);
        assert!(receipt.evidence_path.exists());
        assert!(receipt.report_path.exists());

        let packet = fs::read_to_string(receipt.evidence_path).unwrap();
        assert!(packet.contains("\"schema\": \"allie.evidence.v0\""));
        assert!(packet.contains("sha256:"));
        assert!(packet.contains("\"retention_class\": \"local_ephemeral\""));
        assert!(packet.contains("\"infrastructure_failures\": 0"));
        assert!(packet.contains("\"title\": \"Allie Fixture Login\""));
        assert!(packet.contains("wcag22-aa:deterministic-axe-rules"));
        assert!(packet.contains("\"status\": \"not_tested\""));
        assert!(packet.contains("\"status\": \"needs_review\""));
        assert!(packet.contains("cargo run --locked -- run --manifest examples/login-flow.yml"));

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
    fn model_policy_enabled_without_allowlist_fails_closed() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("latest");
        let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        manifest.model.enabled = true;
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
    }

    #[test]
    fn wcag22_profile_maps_axe_tags_to_versioned_obligations() {
        let profile: serde_json::Value = serde_json::from_str(WCAG22_AA_PROFILE_JSON).unwrap();

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
        assert!(verdicts.iter().any(|verdict| verdict.status == "not_tested"
            && verdict.obligation == "wcag22-aa:2.1.1-keyboard-traversal"));
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
        let profile: serde_json::Value = serde_json::from_str(WCAG22_AA_PROFILE_JSON).unwrap();
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
        let generated = fs::read_to_string(generated_manifest).unwrap();
        assert!(generated.contains("promotion_state: verified_flow"));
        assert!(generated.contains("accessibility_tree: true"));
        assert!(generated.contains("keyboard: true"));
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
        assert!(html.contains("WCAG 2.2 A/AA Success Criteria"));
        assert!(html.contains("criterion -> surface -> state -> finding -> artifact -> replay"));
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
        validate_criterion_coverage_cells(&report).unwrap();
        let html = fs::read_to_string(report_dir.join("compliance-report.html")).unwrap();
        assert!(html.contains("Supporting Checks"));
        assert!(html.contains("wcag22-aa:deterministic-axe-rules"));
        assert!(html.contains("replay command"));
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

        let error = validate_criterion_coverage_cells(&report).unwrap_err();
        assert!(error.contains("terminal criterion coverage cell lacks provenance"));

        report["criterion_coverage"][0]["replay_command"] =
            serde_json::json!("cargo run --locked -- run --manifest fixture.yml");
        let error = validate_criterion_coverage_cells(&report).unwrap_err();
        assert!(error.contains("terminal criterion coverage cell lacks provenance"));

        report["criterion_coverage"][0]["evidence_refs"] = serde_json::json!(["axe-home"]);
        validate_criterion_coverage_cells(&report).unwrap();
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

        let report = build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );
        let report_value = serde_json::to_value(&report).unwrap();

        validate_criterion_coverage_cells(&report_value).unwrap();
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
        });

        let report = build_compliance_report(
            &map,
            &packet,
            Path::new("fixtures/vanity-dogfood-legacy-61/product-map.json"),
            Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json"),
        );
        let report_value = serde_json::to_value(&report).unwrap();

        assert_eq!(report.criteria.len(), 55);
        assert_eq!(report.criterion_coverage.len(), 110);
        validate_criterion_coverage_cells(&report_value).unwrap();

        let mut duplicate = report_value.clone();
        let first_cell = duplicate["criterion_coverage"][0].clone();
        duplicate["criterion_coverage"]
            .as_array_mut()
            .unwrap()
            .push(first_cell);
        let error = validate_criterion_coverage_cells(&duplicate).unwrap_err();
        assert!(error.contains("duplicate criterion coverage cell"));

        let mut missing = report_value;
        missing["criterion_coverage"].as_array_mut().unwrap().pop();
        let error = validate_criterion_coverage_cells(&missing).unwrap_err();
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

        let summary = compliance_summary(&packet, &criteria, 0);

        assert_eq!(summary.status, "needs_review");
        assert_eq!(summary.total_obligations, 2);
        assert_eq!(summary.waived, 1);
        assert_eq!(summary.risk_accepted, 1);
        assert_eq!(summary.pass, 0);
    }

    #[test]
    fn workbench_start_writes_durable_job_lifecycle() {
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
        assert!(job_dir.join("steps/review/evidence-reviewed.json").exists());
        assert!(
            job_dir
                .join("steps/remediation/remediation-queue.json")
                .exists()
        );
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
            job["pointers"]["release_summary"],
            "steps/release/release-summary.json"
        );

        let events = fs::read_to_string(events_path).unwrap();
        assert!(events.contains("\"event\":\"job_started\""));
        assert!(events.contains("\"event\":\"step_completed\""));
        assert!(events.contains("\"step\":\"map\""));
        assert!(events.contains("\"event\":\"job_finished\""));
    }

    #[test]
    fn workbench_status_cancel_and_resume_are_auditable() {
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
        assert!(events.matches("\"event\":\"step_started\"").count() >= 16);
    }

    #[test]
    fn workbench_start_rejects_existing_durable_job_directory() {
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
    fn workbench_resume_rejects_legacy_non_local_job_mode() {
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        fs::create_dir_all(&job_dir).unwrap();
        let mut job = new_workbench_job(
            Path::new("examples/autonomous-workbench.yml"),
            temp.path(),
            AgentRunnerKind::OpenCode,
            DEFAULT_WORKER_TIMEOUT_MS,
        );
        job.status = "cancelled".to_string();
        job.current_step = "cancelled".to_string();
        job.cancel_requested = true;
        write_workbench_job(&job_dir, &job).unwrap();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
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

        assert_eq!(code, 2);
        assert!(String::from_utf8_lossy(&stderr).contains("supports only local jobs"));
    }

    #[test]
    fn workbench_job_writes_preserve_same_generation_cancel_request() {
        let temp = tempdir().unwrap();
        let job_dir = temp.path().join("job");
        fs::create_dir_all(&job_dir).unwrap();
        let job = new_workbench_job(
            Path::new("examples/autonomous-workbench.yml"),
            temp.path(),
            AgentRunnerKind::Local,
            DEFAULT_WORKER_TIMEOUT_MS,
        );
        write_workbench_job(&job_dir, &job).unwrap();

        let mut stale_running_job = read_workbench_job(&job_dir).unwrap();
        let receipt = run_workbench_cancel(&job_dir).unwrap();
        assert_eq!(receipt.status, "cancelled");
        stale_running_job.status = "running".to_string();
        stale_running_job.current_step = "run".to_string();
        stale_running_job.cancel_requested = false;

        write_workbench_job(&job_dir, &stale_running_job).unwrap();

        let final_job = read_workbench_job(&job_dir).unwrap();
        assert_eq!(final_job.status, "cancelled");
        assert_eq!(final_job.current_step, "cancelled");
        assert!(final_job.cancel_requested);
    }

    #[test]
    fn review_cli_adds_agentic_context_without_blocking_release() {
        let temp = tempdir().unwrap();
        let packet_path = write_passing_evidence_packet(&temp.path().join("run"));
        let review_dir = temp.path().join("review");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "review".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                review_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let reviewed_packet_path = review_dir.join("evidence-reviewed.json");
        assert!(reviewed_packet_path.exists());
        assert!(
            review_dir
                .join("artifacts/model-prompt-review-1.txt")
                .exists()
        );
        assert!(
            review_dir
                .join("artifacts/model-response-review-1.json")
                .exists()
        );
        assert!(
            review_dir
                .join("artifacts/redaction-receipt-review-1.json")
                .exists()
        );
        let reviewed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(reviewed_packet_path).unwrap()).unwrap();
        assert_eq!(reviewed["review"][0]["provider"], "offline-recorded");
        assert_eq!(reviewed["findings"][0]["evidence_class"], "agentic");

        let projection = project_release_decision(&reviewed, &release_options(vec!["login-form"]));
        assert_eq!(projection.exit_class, ExitClass::Success);
        assert_eq!(projection.summary["status"], "needs_review");
    }

    #[test]
    fn remediation_cli_writes_evidence_linked_queue() {
        let temp = tempdir().unwrap();
        let packet_path = write_failing_evidence_packet(&temp.path().join("run"));
        let out_dir = temp.path().join("remediation");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_cli_with_io(
            vec![
                "remediate".to_string(),
                "--packet".to_string(),
                packet_path.to_string_lossy().to_string(),
                "--out".to_string(),
                out_dir.to_string_lossy().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0, "stderr={}", String::from_utf8_lossy(&stderr));
        let queue_path = out_dir.join("remediation-queue.json");
        assert!(queue_path.exists());
        assert!(out_dir.join("action-ledger.json").exists());
        assert!(out_dir.join("remediation-report.html").exists());
        assert!(out_dir.join("patch-plan.md").exists());
        let queue: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(queue_path).unwrap()).unwrap();
        assert_eq!(queue["schema"], "allie.remediation-queue.v0");
        assert_eq!(
            queue["items"][0]["finding_refs"][0],
            "login-form-axe-color-contrast-1"
        );
        assert!(
            queue["items"][0]["replay_command"]
                .as_str()
                .unwrap()
                .contains("run --manifest")
        );
        assert!(queue["items"][0]["artifact_refs"].as_array().unwrap().len() >= 1);
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

        let projection = project_release_decision(&packet, &release_options(vec![]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary["status"], "blocked");
        assert_eq!(projection.github_check["conclusion"], "failure");
    }

    #[test]
    fn release_projection_does_not_block_model_only_findings() {
        let mut packet = minimal_release_packet();
        packet["findings"] = serde_json::json!([
            {
                "id": "agentic-1",
                "title": "Possible label ambiguity",
                "evidence_class": "agentic",
                "confidence": "agent_inferred"
            }
        ]);

        let projection = project_release_decision(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::Success);
        assert_eq!(projection.summary["status"], "needs_review");
        assert_eq!(projection.github_check["conclusion"], "neutral");
        assert_eq!(projection.summary["model_findings_non_blocking"], 1);
    }

    #[test]
    fn release_projection_blocks_missing_changed_surface_evidence() {
        let packet = minimal_release_packet();

        let projection = project_release_decision(&packet, &release_options(vec!["settings"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary["status"], "blocked");
        assert_eq!(
            projection.summary["blocking"]["missing_required_evidence"][0],
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

        let projection = project_release_decision(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary["status"], "blocked");
        assert_eq!(
            projection.summary["blocking"]["expired_waivers"][0]["id"],
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

        let projection = project_release_decision(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::BlockingFinding);
        assert_eq!(projection.summary["status"], "blocked");
        assert_eq!(
            projection.summary["blocking"]["invalid_waivers"][0]["id"],
            "waiver-2"
        );
    }

    #[test]
    fn release_projection_routes_stale_evidence_to_review() {
        let mut packet = minimal_release_packet();
        packet["run"]["finished_at"] =
            serde_json::json!((Utc::now() - chrono::Duration::days(30)).to_rfc3339());

        let projection = project_release_decision(&packet, &release_options(vec!["login-form"]));

        assert_eq!(projection.exit_class, ExitClass::Success);
        assert_eq!(projection.summary["status"], "needs_review");
        assert_eq!(projection.github_check["conclusion"], "neutral");
        assert_eq!(projection.summary["review"]["stale_evidence"], true);
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
                "finished_at": Utc::now().to_rfc3339()
            },
            "coverage": {
                "states_captured": ["login-form"],
                "surfaces_discovered": ["Allie Fixture"]
            },
            "artifacts": [
                {"type": "axe_json"},
                {"type": "screenshot"},
                {"type": "html_report"}
            ],
            "findings": [],
            "verdicts": [],
            "waivers": [],
            "policy": {
                "model_status": "disabled",
                "model_provider_allowlist": [],
                "zdr_required": true
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
        }
    }

    fn build_vanity_fixture_report() -> ComplianceReportPacket {
        let map: ProductMapPacket = read_json_file(Path::new(
            "fixtures/vanity-dogfood-legacy-61/product-map.json",
        ))
        .unwrap();
        let packet: EvidencePacket =
            read_json_file(Path::new("fixtures/vanity-dogfood-legacy-61/evidence.json")).unwrap();
        build_compliance_report(
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
            "run-remediation-cli".to_string(),
        )
        .unwrap()
        .evidence_path
    }

    fn passing_worker_response() -> WorkerResponse {
        WorkerResponse {
            schema: WORKER_RESPONSE_SCHEMA.to_string(),
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
                dom_snapshot_path: None,
                accessibility_tree_path: None,
                video_path: None,
                trace_path: None,
                keyboard_focus_order: Vec::new(),
                axe_violations: Vec::new(),
                console_errors: Vec::new(),
                network_errors: Vec::new(),
                state_errors: Vec::new(),
            }],
            errors: Vec::new(),
            nondeterminism: Vec::new(),
        }
    }
}

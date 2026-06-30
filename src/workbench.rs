use crate::{
    AgentRunnerKind, AllieError, DiscoveryOptions, ExitClass, FlowManifest, FlowPlanPacket,
    MapOptions, PromoteFlowOptions, ReleaseOptions, ReportOptions, Result, ReviewOptions,
    RunOptions, default_project_root_for_manifest, new_job_id, now_utc, path_relative_to,
    read_json_file, run_compliance_report, run_discovery, run_map, run_promote_flow, run_release,
    run_review, run_v0, status_for_exit_class, write_string, write_string_atomic,
};
use crate::{DEFAULT_WORKBENCH_IDLE_TIMEOUT_MS, DEFAULT_WORKBENCH_MAX_RUNTIME_MS, JOB_SCHEMA};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct WorkbenchStartOptions {
    manifest_path: PathBuf,
    out_dir: PathBuf,
    project_root: Option<PathBuf>,
    agent_runner: AgentRunnerKind,
}

#[derive(Debug)]
pub(crate) enum WorkbenchCommand {
    Start(WorkbenchStartOptions),
    Status { job_dir: PathBuf },
    Cancel { job_dir: PathBuf },
    Resume { job_dir: PathBuf },
}

#[derive(Debug)]
pub(crate) struct WorkbenchReceipt {
    pub(crate) job_path: PathBuf,
    pub(crate) events_path: PathBuf,
    pub(crate) status: String,
    pub(crate) current_step: String,
    pub(crate) resumable: bool,
    pub(crate) exit_class: ExitClass,
}

pub(crate) fn parse_workbench_command(
    args: &[String],
) -> std::result::Result<WorkbenchCommand, String> {
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

pub(crate) fn run_workbench(command: WorkbenchCommand) -> Result<WorkbenchReceipt> {
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

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "review")? {
        return Ok(receipt);
    }
    let review_manifest = match FlowManifest::load(&promoted.manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => return workbench_step_error(&options.out_dir, job, "review", error),
    };
    let review = match run_workbench_review(&review_manifest, &run.evidence_path, &review_dir) {
        Ok(receipt) => receipt,
        Err(error) => return workbench_step_error(&options.out_dir, job, "review", error),
    };
    if let Some(warning) = &review.warning {
        job.warnings.push(warning.clone());
    }
    job.pointers.reviewed_packet = Some(path_relative_to(&options.out_dir, &review.packet_path));
    if let Some(report_path) = &review.report_path {
        job.pointers.review_report = Some(path_relative_to(&options.out_dir, report_path));
    }
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
        &review.message,
    )?;
    if let Some(receipt) = workbench_cancel_checkpoint(&options.out_dir, &mut job)? {
        return Ok(receipt);
    }
    let reviewed_packet_path = review.packet_path;

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "report")? {
        return Ok(receipt);
    }
    let report = match run_compliance_report(ReportOptions {
        map_path: map.map_path.clone(),
        packet_path: reviewed_packet_path.clone(),
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

    if let Some(receipt) = workbench_start_step_or_cancel(&options.out_dir, &mut job, "release")? {
        return Ok(receipt);
    }
    let changed_surface = workbench_changed_surface(&discovery.flow_plan_path)?;
    let release = match run_release(ReleaseOptions {
        packet_path: reviewed_packet_path,
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

struct WorkbenchReviewStepReceipt {
    packet_path: PathBuf,
    report_path: Option<PathBuf>,
    message: String,
    warning: Option<String>,
}

fn run_workbench_review(
    manifest: &FlowManifest,
    packet_path: &Path,
    review_dir: &Path,
) -> Result<WorkbenchReviewStepReceipt> {
    if manifest.model.enabled {
        return match crate::agentic::run_agentic_review(manifest, packet_path) {
            Ok(summary) => Ok(WorkbenchReviewStepReceipt {
                packet_path: packet_path.to_path_buf(),
                report_path: None,
                message: format!(
                    "live agentic review completed: {} criteria, {} model call(s), status {}",
                    summary.criteria, summary.calls, summary.status
                ),
                warning: None,
            }),
            Err(error) => {
                let warning =
                    format!("agentic review skipped (criteria stay needs_review): {error}");
                Ok(WorkbenchReviewStepReceipt {
                    packet_path: packet_path.to_path_buf(),
                    report_path: None,
                    message: warning.clone(),
                    warning: Some(warning),
                })
            }
        };
    }

    let review = run_review(ReviewOptions {
        packet_path: packet_path.to_path_buf(),
        out_dir: review_dir.to_path_buf(),
    })?;
    Ok(WorkbenchReviewStepReceipt {
        packet_path: review.packet_path,
        report_path: Some(review.report_path),
        message: "offline agentic review context written".to_string(),
        warning: None,
    })
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

fn workbench_changed_surface(flow_plan_path: &Path) -> Result<String> {
    let flow_plan: FlowPlanPacket = read_json_file(flow_plan_path)?;
    Ok(flow_plan
        .candidates
        .first()
        .map(|candidate| candidate.id.clone())
        .unwrap_or_else(|| "generated-flow".to_string()))
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
    release_summary: Option<String>,
    release_report: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkbenchArtifactRef {
    kind: String,
    path: String,
}

#[cfg(test)]
mod tests;

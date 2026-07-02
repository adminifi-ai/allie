use crate::pipeline::{
    DisabledModelReview, PipelineCheckpoint, PipelineOptions, PipelinePaths, PipelineRunResult,
    PipelineStepComplete, run_pipeline,
};
use crate::{
    AgentRunnerKind, AllieError, ExitClass, FlowManifest, FlowPlanPacket, Result,
    default_project_root_for_manifest, new_job_id, now_utc, path_relative_to, read_json_file,
    write_string, write_string_atomic,
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

    let pipeline = run_pipeline(
        PipelineOptions {
            manifest_path: options.manifest_path.clone(),
            project_root: Some(project_root),
            agent_runner: options.agent_runner,
            paths: PipelinePaths::workbench(&options.out_dir),
            disabled_model_review: DisabledModelReview::WriteOfflineReview,
            stale_after_days: 7,
        },
        |checkpoint| workbench_pipeline_checkpoint(&options.out_dir, &mut job, checkpoint),
        |discovery| workbench_changed_surfaces(&discovery.flow_plan_path),
    )?;
    let pipeline = match pipeline {
        PipelineRunResult::Completed(pipeline) => pipeline,
        PipelineRunResult::Stopped(receipt) => return Ok(receipt),
    };

    let final_status = match pipeline.release.exit_class {
        ExitClass::Success => "completed",
        ExitClass::BlockingFinding => "blocked",
        ExitClass::InfrastructureFailure | ExitClass::Usage => "failed",
    };
    workbench_finish(
        &options.out_dir,
        job,
        final_status,
        pipeline.release.exit_class,
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

fn workbench_pipeline_checkpoint(
    job_dir: &Path,
    job: &mut WorkbenchJobPacket,
    checkpoint: PipelineCheckpoint<'_>,
) -> Result<Option<WorkbenchReceipt>> {
    match checkpoint {
        PipelineCheckpoint::BeforeStep(step) => {
            workbench_start_step_or_cancel(job_dir, job, step.id())
        }
        PipelineCheckpoint::StepFailed { step, message } => {
            workbench_step_error(job_dir, job, step.id(), message.to_string()).map(Some)
        }
        PipelineCheckpoint::StepCompleted(complete) => {
            let run_stopped_on_infrastructure = matches!(
                &complete,
                PipelineStepComplete::Run(run)
                    if run.exit_class == ExitClass::InfrastructureFailure
            );
            workbench_record_pipeline_step(job_dir, job, &complete)?;
            if run_stopped_on_infrastructure {
                return workbench_finish(
                    job_dir,
                    job.clone(),
                    "failed",
                    ExitClass::InfrastructureFailure,
                    "run stopped on infrastructure failure",
                )
                .map(Some);
            }
            workbench_cancel_checkpoint(job_dir, job)
        }
    }
}

fn workbench_record_pipeline_step(
    job_dir: &Path,
    job: &mut WorkbenchJobPacket,
    complete: &PipelineStepComplete<'_>,
) -> Result<()> {
    match complete {
        PipelineStepComplete::Discovery(discovery) => {
            job.pointers.discovery = Some(path_relative_to(job_dir, &discovery.discovery_path));
            job.pointers.flow_plan = Some(path_relative_to(job_dir, &discovery.flow_plan_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "discovery".to_string(),
                path: path_relative_to(job_dir, &discovery.discovery_path),
            });
        }
        PipelineStepComplete::PromoteFlow(promoted) => {
            job.pointers.generated_flow = Some(path_relative_to(job_dir, &promoted.manifest_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "generated_flow".to_string(),
                path: path_relative_to(job_dir, &promoted.manifest_path),
            });
        }
        PipelineStepComplete::Map(map) => {
            job.pointers.product_map = Some(path_relative_to(job_dir, &map.map_path));
            job.pointers.surface_map = Some(path_relative_to(job_dir, &map.report_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "product_map".to_string(),
                path: path_relative_to(job_dir, &map.map_path),
            });
        }
        PipelineStepComplete::Run(run) => {
            job.pointers.evidence_packet = Some(path_relative_to(job_dir, &run.evidence_path));
            job.pointers.evidence_report = Some(path_relative_to(job_dir, &run.report_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "evidence_packet".to_string(),
                path: path_relative_to(job_dir, &run.evidence_path),
            });
        }
        PipelineStepComplete::Review(review) => {
            job.pointers.reviewed_packet = Some(path_relative_to(job_dir, &review.packet_path));
            if let Some(report_path) = &review.report_path {
                job.pointers.review_report = Some(path_relative_to(job_dir, report_path));
            }
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "reviewed_packet".to_string(),
                path: path_relative_to(job_dir, &review.packet_path),
            });
        }
        PipelineStepComplete::Report(report) => {
            job.pointers.compliance_report =
                Some(path_relative_to(job_dir, &report.report_json_path));
            job.pointers.compliance_html =
                Some(path_relative_to(job_dir, &report.report_html_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "compliance_report".to_string(),
                path: path_relative_to(job_dir, &report.report_json_path),
            });
        }
        PipelineStepComplete::Release(release) => {
            job.pointers.release_summary = Some(path_relative_to(job_dir, &release.summary_path));
            job.pointers.release_report = Some(path_relative_to(job_dir, &release.report_path));
            job.artifacts.push(WorkbenchArtifactRef {
                kind: "release_summary".to_string(),
                path: path_relative_to(job_dir, &release.summary_path),
            });
        }
    }

    workbench_step_complete(
        job_dir,
        job,
        complete.step().id(),
        complete.status(),
        complete.receipt_path(),
        complete.exit_class(),
        complete.message(),
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
    job: &mut WorkbenchJobPacket,
    step: &str,
    message: String,
) -> Result<WorkbenchReceipt> {
    job.warnings.push(message.clone());
    workbench_step_complete(
        job_dir,
        job,
        step,
        "failed",
        None,
        ExitClass::InfrastructureFailure,
        &message,
    )?;
    workbench_finish(
        job_dir,
        job.clone(),
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

fn workbench_changed_surfaces(flow_plan_path: &Path) -> Result<Vec<String>> {
    let flow_plan: FlowPlanPacket = read_json_file(flow_plan_path)?;
    Ok(vec![
        flow_plan
            .candidates
            .first()
            .map(|candidate| candidate.id.clone())
            .unwrap_or_else(|| "generated-flow".to_string()),
    ])
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

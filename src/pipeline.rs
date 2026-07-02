use crate::agentic::AgenticReviewSummary;
use crate::{
    AgentRunnerKind, AllieError, ComplianceReportReceipt, DiscoveryOptions, DiscoveryReceipt,
    ExitClass, FlowManifest, MapOptions, MapReceipt, PromoteFlowOptions, PromoteFlowReceipt,
    ReleaseOptions, ReleaseReceipt, ReportOptions, Result, ReviewOptions, RunOptions, RunReceipt,
    default_project_root_for_manifest, run_compliance_report, run_discovery, run_map,
    run_promote_flow, run_release, run_review, run_v0, status_for_exit_class,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct PipelineOptions {
    pub(crate) manifest_path: PathBuf,
    pub(crate) project_root: Option<PathBuf>,
    pub(crate) agent_runner: AgentRunnerKind,
    pub(crate) paths: PipelinePaths,
    pub(crate) disabled_model_review: DisabledModelReview,
    pub(crate) stale_after_days: i64,
}

#[derive(Debug)]
pub(crate) struct PipelinePaths {
    discovery_dir: PathBuf,
    generated_flow_path: PathBuf,
    map_dir: PathBuf,
    run_dir: PathBuf,
    review_dir: PathBuf,
    report_dir: PathBuf,
    release_dir: PathBuf,
}

impl PipelinePaths {
    pub(crate) fn verify(out_dir: &Path) -> Self {
        Self {
            discovery_dir: out_dir.join("discovery"),
            generated_flow_path: out_dir.join("flow/generated-flow.yml"),
            map_dir: out_dir.join("map"),
            run_dir: out_dir.join("run"),
            review_dir: out_dir.join("review"),
            report_dir: out_dir.join("report"),
            release_dir: out_dir.join("release"),
        }
    }

    pub(crate) fn workbench(job_dir: &Path) -> Self {
        let discovery_dir = job_dir.join("steps/discovery");
        Self {
            generated_flow_path: discovery_dir.join("generated-flow.yml"),
            discovery_dir,
            map_dir: job_dir.join("steps/map"),
            run_dir: job_dir.join("steps/run"),
            review_dir: job_dir.join("steps/review"),
            report_dir: job_dir.join("steps/report"),
            release_dir: job_dir.join("steps/release"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DisabledModelReview {
    KeepRunPacket,
    WriteOfflineReview,
}

#[derive(Debug)]
pub(crate) struct PipelineReceipts {
    pub(crate) discovery: DiscoveryReceipt,
    pub(crate) promoted: PromoteFlowReceipt,
    pub(crate) map: MapReceipt,
    pub(crate) run: RunReceipt,
    pub(crate) report: ComplianceReportReceipt,
    pub(crate) release: ReleaseReceipt,
    pub(crate) project_root: PathBuf,
    pub(crate) changed_surfaces: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct PipelineReviewReceipt {
    pub(crate) packet_path: PathBuf,
    pub(crate) report_path: Option<PathBuf>,
    pub(crate) message: String,
    pub(crate) agentic_summary: Option<AgenticReviewSummary>,
}

#[derive(Debug)]
pub(crate) enum PipelineRunResult<S> {
    Completed(Box<PipelineReceipts>),
    Stopped(S),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PipelineStep {
    Discover,
    PromoteFlow,
    Map,
    Run,
    Review,
    Report,
    Release,
}

impl PipelineStep {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::PromoteFlow => "promote-flow",
            Self::Map => "map",
            Self::Run => "run",
            Self::Review => "review",
            Self::Report => "report",
            Self::Release => "release",
        }
    }
}

pub(crate) enum PipelineCheckpoint<'a> {
    BeforeStep(PipelineStep),
    StepFailed {
        step: PipelineStep,
        message: &'a str,
    },
    StepCompleted(PipelineStepComplete<'a>),
}

pub(crate) enum PipelineStepComplete<'a> {
    Discovery(&'a DiscoveryReceipt),
    PromoteFlow(&'a PromoteFlowReceipt),
    Map(&'a MapReceipt),
    Run(&'a RunReceipt),
    Review(&'a PipelineReviewReceipt),
    Report(&'a ComplianceReportReceipt),
    Release(&'a ReleaseReceipt),
}

impl PipelineStepComplete<'_> {
    pub(crate) fn step(&self) -> PipelineStep {
        match self {
            Self::Discovery(_) => PipelineStep::Discover,
            Self::PromoteFlow(_) => PipelineStep::PromoteFlow,
            Self::Map(_) => PipelineStep::Map,
            Self::Run(_) => PipelineStep::Run,
            Self::Review(_) => PipelineStep::Review,
            Self::Report(_) => PipelineStep::Report,
            Self::Release(_) => PipelineStep::Release,
        }
    }

    pub(crate) fn status(&self) -> &'static str {
        match self {
            Self::Run(receipt) => status_for_exit_class(receipt.exit_class),
            Self::Release(receipt) => status_for_exit_class(receipt.exit_class),
            _ => "completed",
        }
    }

    pub(crate) fn receipt_path(&self) -> Option<&Path> {
        match self {
            Self::Discovery(receipt) => Some(&receipt.discovery_path),
            Self::PromoteFlow(receipt) => Some(&receipt.manifest_path),
            Self::Map(receipt) => Some(&receipt.map_path),
            Self::Run(receipt) => Some(&receipt.evidence_path),
            Self::Review(receipt) => Some(&receipt.packet_path),
            Self::Report(receipt) => Some(&receipt.report_json_path),
            Self::Release(receipt) => Some(&receipt.summary_path),
        }
    }

    pub(crate) fn exit_class(&self) -> ExitClass {
        match self {
            Self::Run(receipt) => receipt.exit_class,
            Self::Release(receipt) => receipt.exit_class,
            _ => ExitClass::Success,
        }
    }

    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Discovery(_) => "discovery packet written",
            Self::PromoteFlow(_) => "generated flow manifest written",
            Self::Map(_) => "product map written",
            Self::Run(_) => "evidence replay completed",
            Self::Review(receipt) => &receipt.message,
            Self::Report(_) => "compliance report written",
            Self::Release(_) => "release projection written",
        }
    }
}

enum StepErrorOutcome<S> {
    Stop(S),
    Propagate(AllieError),
}

pub(crate) fn run_pipeline<S>(
    options: PipelineOptions,
    mut checkpoint: impl FnMut(PipelineCheckpoint<'_>) -> Result<Option<S>>,
    release_changed_surfaces: impl FnOnce(&DiscoveryReceipt) -> Result<Vec<String>>,
) -> Result<PipelineRunResult<S>> {
    // Keep these blocks explicit: each step consumes different receipt inputs,
    // so the typed data flow is easier to audit here than behind a macro.
    let manifest = FlowManifest::load(&options.manifest_path)?;
    manifest.validate()?;
    let project_root = options
        .project_root
        .clone()
        .unwrap_or_else(|| default_project_root_for_manifest(&options.manifest_path, &manifest));
    let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Discover))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let discovery = match run_pipeline_discovery(&options) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Discover, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Discovery(&discovery),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::PromoteFlow))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let promoted = match run_pipeline_promote_flow(&options, &discovery) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::PromoteFlow, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::PromoteFlow(&promoted),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Map))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let map = match run_pipeline_map(&options, &project_root) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Map, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Map(&map),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Run))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let run = match run_pipeline_replay(&options, &promoted, &project_root) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Run, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Run(&run),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Review))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let review = match run_pipeline_review(&options, &promoted, &run) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Review, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Review(&review),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Report))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let report = match run_pipeline_report(&options, &map, &review) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Report, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Report(&report),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    if let Some(stop) = checkpoint(PipelineCheckpoint::BeforeStep(PipelineStep::Release))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }
    let changed_surfaces = match release_changed_surfaces(&discovery) {
        Ok(changed_surfaces) => changed_surfaces,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Release, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    let release = match run_pipeline_release(&options, &review, &changed_surfaces) {
        Ok(receipt) => receipt,
        Err(error) => match step_error(&mut checkpoint, PipelineStep::Release, error)? {
            StepErrorOutcome::Stop(stop) => return Ok(PipelineRunResult::Stopped(stop)),
            StepErrorOutcome::Propagate(error) => return Err(error),
        },
    };
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepCompleted(
        PipelineStepComplete::Release(&release),
    ))? {
        return Ok(PipelineRunResult::Stopped(stop));
    }

    Ok(PipelineRunResult::Completed(Box::new(PipelineReceipts {
        discovery,
        promoted,
        map,
        run,
        report,
        release,
        project_root,
        changed_surfaces,
    })))
}

fn step_error<S>(
    checkpoint: &mut impl FnMut(PipelineCheckpoint<'_>) -> Result<Option<S>>,
    step: PipelineStep,
    error: AllieError,
) -> Result<StepErrorOutcome<S>> {
    let message = error.to_string();
    if let Some(stop) = checkpoint(PipelineCheckpoint::StepFailed {
        step,
        message: &message,
    })? {
        Ok(StepErrorOutcome::Stop(stop))
    } else {
        Ok(StepErrorOutcome::Propagate(error))
    }
}

fn run_pipeline_discovery(options: &PipelineOptions) -> Result<DiscoveryReceipt> {
    run_discovery(DiscoveryOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: options.paths.discovery_dir.clone(),
    })
}

fn run_pipeline_promote_flow(
    options: &PipelineOptions,
    discovery: &DiscoveryReceipt,
) -> Result<PromoteFlowReceipt> {
    run_promote_flow(PromoteFlowOptions {
        discovery_path: discovery.discovery_path.clone(),
        flow_plan_path: discovery.flow_plan_path.clone(),
        out_path: options.paths.generated_flow_path.clone(),
    })
}

fn run_pipeline_map(options: &PipelineOptions, project_root: &Path) -> Result<MapReceipt> {
    run_map(MapOptions {
        manifest_path: options.manifest_path.clone(),
        out_dir: options.paths.map_dir.clone(),
        project_root: project_root.to_path_buf(),
        agent_runner: options.agent_runner,
    })
}

fn run_pipeline_replay(
    options: &PipelineOptions,
    promoted: &PromoteFlowReceipt,
    project_root: &Path,
) -> Result<RunReceipt> {
    run_v0(RunOptions {
        manifest_path: promoted.manifest_path.clone(),
        out_dir: options.paths.run_dir.clone(),
        project_root: Some(project_root.to_path_buf()),
    })
}

fn run_pipeline_review(
    options: &PipelineOptions,
    promoted: &PromoteFlowReceipt,
    run: &RunReceipt,
) -> Result<PipelineReviewReceipt> {
    let manifest = FlowManifest::load(&promoted.manifest_path)?;
    if manifest.model.enabled {
        let summary = crate::agentic::run_agentic_review(&manifest, &run.evidence_path)?;
        return Ok(PipelineReviewReceipt {
            packet_path: run.evidence_path.clone(),
            report_path: None,
            message: format!(
                "live agentic review completed: {} criteria, {} model call(s), status {}",
                summary.criteria, summary.calls, summary.status
            ),
            agentic_summary: Some(summary),
        });
    }

    match options.disabled_model_review {
        DisabledModelReview::KeepRunPacket => Ok(PipelineReviewReceipt {
            packet_path: run.evidence_path.clone(),
            report_path: None,
            message: "agentic review disabled; using replay evidence packet".to_string(),
            agentic_summary: None,
        }),
        DisabledModelReview::WriteOfflineReview => {
            let review = run_review(ReviewOptions {
                packet_path: run.evidence_path.clone(),
                out_dir: options.paths.review_dir.clone(),
            })?;
            Ok(PipelineReviewReceipt {
                packet_path: review.packet_path,
                report_path: Some(review.report_path),
                message: "offline agentic review context written".to_string(),
                agentic_summary: None,
            })
        }
    }
}

fn run_pipeline_report(
    options: &PipelineOptions,
    map: &MapReceipt,
    review: &PipelineReviewReceipt,
) -> Result<ComplianceReportReceipt> {
    run_compliance_report(ReportOptions {
        map_path: map.map_path.clone(),
        packet_path: review.packet_path.clone(),
        out_dir: options.paths.report_dir.clone(),
    })
}

fn run_pipeline_release(
    options: &PipelineOptions,
    review: &PipelineReviewReceipt,
    changed_surfaces: &[String],
) -> Result<ReleaseReceipt> {
    run_release(ReleaseOptions {
        packet_path: review.packet_path.clone(),
        out_dir: options.paths.release_dir.clone(),
        changed_surfaces: changed_surfaces.to_vec(),
        stale_after_days: options.stale_after_days,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn release_surface_resolution_errors_are_attributed_to_release_step() {
        let temp = tempdir().unwrap();
        let out_dir = temp.path().join("verify");
        let mut failed_steps = Vec::new();
        let mut before_release = false;
        let mut release_completed = false;

        let result = run_pipeline(
            PipelineOptions {
                manifest_path: PathBuf::from("examples/autonomous-workbench.yml"),
                project_root: None,
                agent_runner: AgentRunnerKind::Local,
                paths: PipelinePaths::verify(&out_dir),
                disabled_model_review: DisabledModelReview::KeepRunPacket,
                stale_after_days: 7,
            },
            |checkpoint| {
                match checkpoint {
                    PipelineCheckpoint::BeforeStep(PipelineStep::Release) => {
                        before_release = true;
                    }
                    PipelineCheckpoint::StepFailed { step, message } => {
                        failed_steps.push((step, message.to_string()));
                        return Ok(Some("release failure recorded"));
                    }
                    PipelineCheckpoint::StepCompleted(complete)
                        if complete.step() == PipelineStep::Release =>
                    {
                        release_completed = true;
                    }
                    _ => {}
                }
                Ok(None)
            },
            |_| {
                Err(AllieError::InvalidManifest(
                    "flow-plan unavailable at release".to_string(),
                ))
            },
        )
        .unwrap();

        assert!(matches!(
            result,
            PipelineRunResult::Stopped("release failure recorded")
        ));
        assert!(
            before_release,
            "release surface resolution should fail after the release checkpoint starts"
        );
        assert_eq!(
            failed_steps,
            vec![(
                PipelineStep::Release,
                "invalid manifest: flow-plan unavailable at release".to_string()
            )]
        );
        assert!(
            !release_completed,
            "release must not complete after changed-surface resolution fails"
        );
    }
}

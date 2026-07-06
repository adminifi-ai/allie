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

#[test]
fn workbench_review_step_keeps_run_packet_unchanged_when_model_disabled() {
    let temp = tempdir().unwrap();
    let job_dir = temp.path().join("job");
    let evidence_path = job_dir.join("steps/run/evidence.json");
    fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
    fs::write(&evidence_path, r#"{"findings":[]}"#).unwrap();

    let options = PipelineOptions {
        manifest_path: PathBuf::from("examples/autonomous-workbench.yml"),
        project_root: None,
        agent_runner: AgentRunnerKind::Local,
        paths: PipelinePaths::workbench(&job_dir),
        stale_after_days: 7,
    };
    let promoted = PromoteFlowReceipt {
        manifest_path: PathBuf::from("examples/autonomous-workbench.yml"),
    };
    let run = RunReceipt {
        run_id: "test-run".to_string(),
        exit_class: ExitClass::Success,
        evidence_path: evidence_path.clone(),
        report_path: job_dir.join("steps/run/report.html"),
    };

    let review = run_pipeline_review(&options, &promoted, &run).unwrap();

    assert_eq!(review.packet_path, evidence_path);
    assert!(review.report_path.is_none());
    assert!(review.agentic_summary.is_none());

    let packet: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).unwrap()).unwrap();
    assert!(
        packet["findings"].as_array().unwrap().is_empty(),
        "model-off workbench review must not fabricate findings"
    );
    assert!(
        !job_dir.join("steps/review").exists(),
        "model-off workbench review must not write an offline review artifact directory"
    );
}

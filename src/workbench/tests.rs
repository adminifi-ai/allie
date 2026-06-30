use super::*;
use crate::{DEFAULT_WORKER_TIMEOUT_MS, run_cli_with_io};
use tempfile::tempdir;

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

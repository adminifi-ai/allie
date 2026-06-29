use crate::auth::AuthFlow;
use crate::model::{ArtifactMetadata, ArtifactPolicy, BrowserSettings, PageFeatures};
use crate::{
    AllieError, FlowManifest, ManifestState, Result, artifact_for_path, normalize_relative,
    write_json_pretty,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub(crate) const WORKER_RESPONSE_SCHEMA: &str = "allie.worker.response.v0";
const WORKER_REQUEST_SCHEMA: &str = "allie.worker.request.v0";
const WORKER_CREATION_TOOL: &str = "playwright-axe-worker";

pub(crate) struct WorkerExecution {
    pub(crate) response: WorkerResponse,
    pub(crate) run_failures: Vec<RunFailure>,
}

#[cfg(test)]
pub(crate) fn response_schema() -> &'static str {
    WORKER_RESPONSE_SCHEMA
}

pub(crate) fn execute(
    run_id: &str,
    manifest: &FlowManifest,
    manifest_path: &Path,
    out_dir_abs: &Path,
    mut run_failures: Vec<RunFailure>,
) -> Result<WorkerExecution> {
    let request_path = out_dir_abs.join("worker-request.json");
    let response_path = out_dir_abs.join("worker-response.json");
    let response = if run_failures.is_empty() {
        let request = WorkerRequest::from_manifest(
            run_id,
            manifest,
            manifest_path,
            &out_dir_abs.join("artifacts"),
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

    Ok(WorkerExecution {
        response,
        run_failures,
    })
}

pub(crate) fn environment_requirements() -> Vec<String> {
    vec![
        "npm install".to_string(),
        "npx playwright install chromium".to_string(),
    ]
}

pub(crate) fn artifacts(
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
                WORKER_CREATION_TOOL,
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
                WORKER_CREATION_TOOL,
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
                WORKER_CREATION_TOOL,
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
                WORKER_CREATION_TOOL,
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
                WORKER_CREATION_TOOL,
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
                WORKER_CREATION_TOOL,
                artifact_policy,
                timestamp,
            )?);
        }
    }
    Ok(artifacts)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<AuthFlow>,
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
            auth: manifest.auth.clone(),
        })
    }
}

#[derive(Debug, Serialize)]
struct WorkerTarget {
    kind: String,
    fixture_dir: Option<String>,
    base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkerResponse {
    pub(crate) schema: String,
    pub(crate) status: WorkerRunStatus,
    pub(crate) actual_base_url: Option<String>,
    #[serde(default)]
    pub(crate) states: Vec<WorkerStateResult>,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
    #[serde(default)]
    pub(crate) nondeterminism: Vec<String>,
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

    pub(crate) fn error(message: String) -> Self {
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
pub(crate) enum WorkerRunStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkerStateResult {
    pub(crate) id: String,
    pub(crate) route: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) http_status: Option<u16>,
    pub(crate) screenshot_path: Option<String>,
    pub(crate) axe_json_path: Option<String>,
    #[serde(default)]
    pub(crate) dom_snapshot_path: Option<String>,
    #[serde(default)]
    pub(crate) accessibility_tree_path: Option<String>,
    #[serde(default)]
    pub(crate) video_path: Option<String>,
    #[serde(default)]
    pub(crate) trace_path: Option<String>,
    #[serde(default)]
    pub(crate) keyboard_focus_order: Vec<String>,
    #[serde(default)]
    pub(crate) axe_violations: Vec<AxeViolation>,
    #[serde(default)]
    pub(crate) console_errors: Vec<String>,
    #[serde(default)]
    pub(crate) network_errors: Vec<String>,
    #[serde(default)]
    pub(crate) state_errors: Vec<String>,
    #[serde(default)]
    pub(crate) features: Option<PageFeatures>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct AxeViolation {
    pub(crate) id: String,
    pub(crate) impact: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) nodes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RunFailure {
    pub(crate) kind: String,
    pub(crate) source: String,
    pub(crate) message: String,
}

impl RunFailure {
    pub(crate) fn new(kind: &str, source: &str, message: String) -> Self {
        Self {
            kind: kind.to_string(),
            source: source.to_string(),
            message,
        }
    }
}

fn read_worker_response(response_path: &Path) -> Result<WorkerResponse> {
    let response_text = match std::fs::read_to_string(response_path) {
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

    let status = child
        .wait_timeout(Duration::from_millis(timeout_ms))
        .map_err(|source| {
            RunFailure::new(
                "worker-wait-failed",
                "worker-adapter",
                format!("wait for worker {}: {source}", worker_script.display()),
            )
        })?;

    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(RunFailure::new(
            "worker-timeout",
            "worker-adapter",
            format!("worker timed out after {timeout_ms} ms"),
        ));
    };

    if status.success() {
        return Ok(());
    }

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
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(RunFailure::new(
        "worker-crash",
        "worker-adapter",
        format!("worker exited with {}; stderr: {}", status, stderr.trim()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlowManifest;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static AUTH_ENV_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn worker_request_carries_auth_env_names_not_secret_values() {
        let _guard = AUTH_ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let sentinel = "sentinel-secret-do-not-serialize-7f3c";
        unsafe {
            std::env::set_var("ALLIE_AUTH_FIXTURE_PASSWORD", sentinel);
            std::env::set_var("ALLIE_AUTH_FIXTURE_USER", "qa@example.test");
        }

        let manifest = FlowManifest::load(Path::new("examples/auth-fixture-flow.yml")).unwrap();
        assert!(
            manifest.preflight_failures().is_empty(),
            "preflight should pass with both auth env vars set"
        );

        let temp = tempdir().unwrap();
        let request = WorkerRequest::from_manifest(
            "run-auth-secret",
            &manifest,
            Path::new("examples/auth-fixture-flow.yml"),
            &temp.path().join("artifacts"),
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&request).unwrap();

        unsafe {
            std::env::remove_var("ALLIE_AUTH_FIXTURE_PASSWORD");
            std::env::remove_var("ALLIE_AUTH_FIXTURE_USER");
        }

        assert!(
            json.contains("ALLIE_AUTH_FIXTURE_PASSWORD"),
            "request must carry the env NAME so the worker can read it"
        );
        assert!(
            !json.contains(sentinel),
            "request must NOT carry the secret VALUE (secrets stay off disk)"
        );
    }
}

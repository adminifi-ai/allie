use crate::model_credentials::{
    MODEL_PROVIDER_PRESETS, env_var_non_empty, model_provider_preset_env_names,
    resolve_model_credentials,
};
use crate::{ExitClass, FlowManifest, normalize_relative, write_json_pretty};
use serde::Serialize;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

mod assets;
use assets::{
    AGENTIC_WORKER, BROWSER_WORKER, WorkerAsset, WorkerScriptResolution, WorkerScriptSearch,
    resolve_worker_script,
};
pub(crate) use assets::{agentic_worker_script, apply_worker_environment, browser_worker_script};

const DOCTOR_SCHEMA: &str = "allie.doctor.v0";
const DOCTOR_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug)]
pub(crate) struct DoctorOptions {
    pub(crate) manifest_path: Option<PathBuf>,
    pub(crate) out_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorCheckStatus {
    Ok,
    Warn,
    Fail,
}

impl Display for DoctorCheckStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorStatus {
    Pass,
    Warn,
    Fail,
}

impl Display for DoctorStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DoctorCheck {
    pub(crate) name: String,
    pub(crate) status: DoctorCheckStatus,
    pub(crate) detail: String,
    pub(crate) fix: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DoctorReceipt {
    pub(crate) schema: &'static str,
    pub(crate) status: DoctorStatus,
    #[serde(skip_serializing)]
    pub(crate) exit_class: ExitClass,
    pub(crate) checks: Vec<DoctorCheck>,
}

pub(crate) fn run_doctor(options: DoctorOptions) -> DoctorReceipt {
    let out_dir = absolute_out_dir(&options.out_dir);
    let browser_resolution = resolve_worker_script(BROWSER_WORKER);
    let agentic_resolution = resolve_worker_script(AGENTIC_WORKER);
    let node_check = check_node();
    let node_ok = node_check.status == DoctorCheckStatus::Ok;
    let mut checks = vec![
        check_worker_script(BROWSER_WORKER, &browser_resolution),
        check_worker_script(AGENTIC_WORKER, &agentic_resolution),
        node_check,
        check_playwright(browser_resolution.as_ref().ok(), &out_dir),
        check_target(options.manifest_path.as_deref(), node_ok),
        check_model(options.manifest_path.as_deref()),
    ];

    checks.sort_by(|left, right| left.name.cmp(&right.name));
    let mut receipt = DoctorReceipt {
        schema: DOCTOR_SCHEMA,
        status: doctor_status(&checks),
        exit_class: doctor_exit_class(&checks),
        checks,
    };

    if let Err(error) = write_json_pretty(&out_dir.join("doctor.json"), &receipt) {
        receipt.checks.push(DoctorCheck {
            name: "doctor receipt".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: error.to_string(),
            fix: Some(format!("Ensure {} is writable.", out_dir.display())),
        });
        receipt
            .checks
            .sort_by(|left, right| left.name.cmp(&right.name));
        receipt.status = doctor_status(&receipt.checks);
        receipt.exit_class = doctor_exit_class(&receipt.checks);
    }

    receipt
}

fn absolute_out_dir(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn check_worker_script(
    worker: WorkerAsset,
    resolution: &std::result::Result<WorkerScriptResolution, WorkerScriptSearch>,
) -> DoctorCheck {
    match resolution {
        Ok(resolution) => DoctorCheck {
            name: worker.label.to_string(),
            status: DoctorCheckStatus::Ok,
            detail: format!("{} ({})", resolution.path.display(), resolution.source),
            fix: None,
        },
        Err(search) => DoctorCheck {
            name: worker.label.to_string(),
            status: DoctorCheckStatus::Fail,
            detail: search.message.clone(),
            fix: Some(format!(
                "Run from an Allie checkout, install a package that includes {}, or set {}. Searched: {}",
                worker.relative_path,
                worker.env_var,
                search
                    .searched_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
    }
}

fn check_node() -> DoctorCheck {
    let mut command = Command::new("node");
    command.arg("--version");
    match run_command(&mut command, Duration::from_millis(5000)) {
        Ok(output) if output.status.success() => DoctorCheck {
            name: "node".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            fix: None,
        },
        Ok(output) => DoctorCheck {
            name: "node".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            fix: Some("Install Node.js and ensure `node` is on PATH.".to_string()),
        },
        Err(message) => DoctorCheck {
            name: "node".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: message,
            fix: Some("Install Node.js and ensure `node` is on PATH.".to_string()),
        },
    }
}

fn check_playwright(resolution: Option<&WorkerScriptResolution>, out_dir: &Path) -> DoctorCheck {
    let Some(resolution) = resolution else {
        return DoctorCheck {
            name: "playwright".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: "browser worker was not found, so Playwright could not be checked".to_string(),
            fix: Some("Fix the browser worker path first, then rerun `allie doctor`.".to_string()),
        };
    };

    let smoke_dir = out_dir.join("browser-worker-smoke");
    let _ = std::fs::create_dir_all(out_dir);
    let mut command = Command::new("node");
    command
        .arg(&resolution.path)
        .arg("--smoke")
        .arg(&smoke_dir)
        .current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    apply_worker_environment(&mut command, &resolution.path);

    match run_command(&mut command, Duration::from_millis(DOCTOR_TIMEOUT_MS)) {
        Ok(output) if output.status.success() => DoctorCheck {
            name: "playwright".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: format!("worker smoke passed at {}", smoke_dir.display()),
            fix: None,
        },
        Ok(output) => DoctorCheck {
            name: "playwright".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: stderr_or_stdout(&output),
            fix: Some("Run `npm ci` and `npx playwright install chromium` in the Allie worker asset directory.".to_string()),
        },
        Err(message) => DoctorCheck {
            name: "playwright".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: message,
            fix: Some("Run `npm ci` and `npx playwright install chromium` in the Allie worker asset directory.".to_string()),
        },
    }
}

fn check_target(manifest_path: Option<&Path>, node_ok: bool) -> DoctorCheck {
    let Some(manifest_path) = manifest_path else {
        return DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Warn,
            detail: "no manifest supplied; target reachability was not checked".to_string(),
            fix: Some(
                "Run `allie doctor --manifest .allie/manifest.yml` after `allie init`.".to_string(),
            ),
        };
    };
    if !manifest_path.exists() {
        return DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: format!("manifest {} does not exist", manifest_path.display()),
            fix: Some(
                "Run `allie init` or pass --manifest to an existing flow manifest.".to_string(),
            ),
        };
    }

    let manifest = match FlowManifest::load(manifest_path).and_then(|manifest| {
        manifest.validate()?;
        Ok(manifest)
    }) {
        Ok(manifest) => manifest,
        Err(error) => {
            return DoctorCheck {
                name: "target".to_string(),
                status: DoctorCheckStatus::Fail,
                detail: error.to_string(),
                fix: Some("Fix the manifest, then rerun `allie doctor`.".to_string()),
            };
        }
    };

    match manifest.target.kind.as_str() {
        "local_fixture" => check_fixture_target(manifest_path, &manifest),
        "web" => check_web_target(&manifest, node_ok),
        other => DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: format!("unsupported target kind {other}"),
            fix: Some("Use target.kind: web or local_fixture.".to_string()),
        },
    }
}

fn check_fixture_target(manifest_path: &Path, manifest: &FlowManifest) -> DoctorCheck {
    let Some(fixture_dir) = manifest.target.fixture_dir.as_ref() else {
        return DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: "local_fixture target is missing fixture_dir".to_string(),
            fix: Some("Add target.fixture_dir to the manifest.".to_string()),
        };
    };
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let fixture_path = PathBuf::from(normalize_relative(manifest_dir, fixture_dir));
    if fixture_path.is_dir() {
        DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: format!("local fixture found at {}", fixture_path.display()),
            fix: None,
        }
    } else {
        DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: format!("fixture_dir {} is not a directory", fixture_path.display()),
            fix: Some(
                "Point target.fixture_dir at an existing static fixture directory.".to_string(),
            ),
        }
    }
}

fn check_web_target(manifest: &FlowManifest, node_ok: bool) -> DoctorCheck {
    let Some(base_url) = manifest.target.base_url.as_ref() else {
        return DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: "web target is missing base_url".to_string(),
            fix: Some("Add target.base_url or use --fixture-dir during init.".to_string()),
        };
    };
    if !node_ok {
        return DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Warn,
            detail: "node is unavailable, so web target reachability was not checked".to_string(),
            fix: Some("Install Node.js, then rerun `allie doctor`.".to_string()),
        };
    }

    let script = r#"
const url = process.argv[1];
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 5000);
fetch(url, { signal: controller.signal })
  .then((response) => {
    clearTimeout(timer);
    if (response.status >= 200 && response.status < 500) {
      console.log(`${response.status} ${url}`);
      process.exit(0);
    }
    console.error(`HTTP ${response.status} ${url}`);
    process.exit(1);
  })
  .catch((error) => {
    clearTimeout(timer);
    console.error(error.message);
    process.exit(1);
  });
"#;
    let mut command = Command::new("node");
    command.arg("-e").arg(script).arg(base_url);
    match run_command(&mut command, Duration::from_millis(7000)) {
        Ok(output) if output.status.success() => DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            fix: None,
        },
        Ok(output) => DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: stderr_or_stdout(&output),
            fix: Some(format!(
                "Start the app at {base_url}, or update target.base_url."
            )),
        },
        Err(message) => DoctorCheck {
            name: "target".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: message,
            fix: Some(format!(
                "Start the app at {base_url}, or update target.base_url."
            )),
        },
    }
}

/// Reports whether agentic (vision-model) review will actually run, and if
/// not, why — model-only findings never block a release, but a silent
/// `model.enabled: false` should never be a mystery.
fn check_model(manifest_path: Option<&Path>) -> DoctorCheck {
    let Some(manifest_path) = manifest_path else {
        return DoctorCheck {
            name: "model".to_string(),
            status: DoctorCheckStatus::Warn,
            detail: "no manifest supplied; model policy was not checked".to_string(),
            fix: Some(
                "Run `allie doctor --manifest .allie/manifest.yml` after `allie init`.".to_string(),
            ),
        };
    };
    if !manifest_path.exists() {
        return DoctorCheck {
            name: "model".to_string(),
            status: DoctorCheckStatus::Warn,
            detail: format!("manifest {} does not exist", manifest_path.display()),
            fix: Some(
                "Run `allie init` or pass --manifest to an existing flow manifest.".to_string(),
            ),
        };
    }

    let manifest = match FlowManifest::load(manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return DoctorCheck {
                name: "model".to_string(),
                status: DoctorCheckStatus::Fail,
                detail: error.to_string(),
                fix: Some("Fix the manifest, then rerun `allie doctor`.".to_string()),
            };
        }
    };

    if !manifest.model.enabled {
        return match resolve_model_credentials() {
            Some(preset) => DoctorCheck {
                name: "model".to_string(),
                status: DoctorCheckStatus::Warn,
                detail: format!(
                    "model review is off in the manifest, but {} resolves in this environment",
                    preset.api_key_env
                ),
                fix: Some(
                    "Rerun `allie init --force` to pick it up automatically, or set model.enabled: true and model.provider_allowlist in the manifest.".to_string(),
                ),
            },
            None => DoctorCheck {
                name: "model".to_string(),
                status: DoctorCheckStatus::Warn,
                detail: format!(
                    "model review is off: no resolvable API key found (checked {})",
                    model_provider_preset_env_names()
                ),
                fix: Some(format!(
                    "Set one of {} in the environment, then rerun `allie init --force` to enable agentic review.",
                    model_provider_preset_env_names()
                )),
            },
        };
    }

    let api_key_env = manifest
        .model
        .api_key_env
        .as_deref()
        .unwrap_or(MODEL_PROVIDER_PRESETS[0].api_key_env);
    if env_var_non_empty(api_key_env) {
        DoctorCheck {
            name: "model".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: format!(
                "model review enabled via {} ({api_key_env})",
                manifest
                    .model
                    .provider
                    .as_deref()
                    .unwrap_or(MODEL_PROVIDER_PRESETS[0].provider)
            ),
            fix: None,
        }
    } else {
        DoctorCheck {
            name: "model".to_string(),
            status: DoctorCheckStatus::Warn,
            detail: format!(
                "model.enabled is true but {api_key_env} is not set in this environment"
            ),
            fix: Some(format!("Export {api_key_env}, then rerun `allie doctor`.")),
        }
    }
}

fn doctor_status(checks: &[DoctorCheck]) -> DoctorStatus {
    if checks
        .iter()
        .any(|check| check.status == DoctorCheckStatus::Fail)
    {
        DoctorStatus::Fail
    } else if checks
        .iter()
        .any(|check| check.status == DoctorCheckStatus::Warn)
    {
        DoctorStatus::Warn
    } else {
        DoctorStatus::Pass
    }
}

fn doctor_exit_class(checks: &[DoctorCheck]) -> ExitClass {
    if checks
        .iter()
        .any(|check| check.status == DoctorCheckStatus::Fail)
    {
        ExitClass::InfrastructureFailure
    } else {
        ExitClass::Success
    }
}

fn run_command(
    command: &mut Command,
    timeout: Duration,
) -> std::result::Result<std::process::Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| source.to_string())?;
    let status = child
        .wait_timeout(timeout)
        .map_err(|source| source.to_string())?;
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "command timed out after {} ms",
            timeout.as_millis()
        ));
    }
    child
        .wait_with_output()
        .map_err(|source| source.to_string())
}

fn stderr_or_stdout(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(test)]
mod tests;

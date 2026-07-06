use crate::{
    ExitClass, FlowManifest, MODEL_PROVIDER_PRESETS, env_var_non_empty,
    model_provider_preset_env_names, normalize_relative, resolve_model_credentials,
    write_json_pretty,
};
use serde::Serialize;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

const DOCTOR_SCHEMA: &str = "allie.doctor.v0";
const BROWSER_WORKER_ENV: &str = "ALLIE_BROWSER_WORKER";
const BROWSER_WORKER_RELATIVE: &str = "workers/browser/run.mjs";
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
    let worker_resolution = resolve_worker_script();
    let node_check = check_node();
    let node_ok = node_check.status == DoctorCheckStatus::Ok;
    let mut checks = vec![
        check_worker_script(&worker_resolution),
        node_check,
        check_playwright(worker_resolution.as_ref().ok(), &out_dir),
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

pub(crate) fn browser_worker_script() -> std::result::Result<PathBuf, String> {
    resolve_worker_script()
        .map(|resolution| resolution.path)
        .map_err(|search| search.message)
}

pub(crate) fn apply_worker_environment(command: &mut Command, worker_script: &Path) {
    let Some(root) = worker_asset_root(worker_script) else {
        return;
    };
    let browsers = root.join("ms-playwright");
    if browsers.is_dir() {
        command.env("PLAYWRIGHT_BROWSERS_PATH", browsers);
    }
}

fn worker_asset_root(worker_script: &Path) -> Option<PathBuf> {
    worker_script
        .parent()?
        .parent()?
        .parent()
        .map(Path::to_path_buf)
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

#[derive(Debug)]
struct WorkerScriptResolution {
    path: PathBuf,
    source: String,
}

#[derive(Debug)]
struct WorkerScriptSearch {
    message: String,
    searched_paths: Vec<PathBuf>,
}

fn resolve_worker_script() -> std::result::Result<WorkerScriptResolution, WorkerScriptSearch> {
    let env_override = std::env::var_os(BROWSER_WORKER_ENV).map(PathBuf::from);
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("allie"));
    resolve_worker_script_from(
        env_override,
        &exe_path,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}

fn resolve_worker_script_from(
    env_override: Option<PathBuf>,
    exe_path: &Path,
    manifest_dir: &Path,
) -> std::result::Result<WorkerScriptResolution, WorkerScriptSearch> {
    if let Some(path) = env_override {
        if path.exists() {
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            return Ok(WorkerScriptResolution {
                path,
                source: BROWSER_WORKER_ENV.to_string(),
            });
        }
        return Err(WorkerScriptSearch {
            message: format!(
                "{BROWSER_WORKER_ENV} points to missing browser worker at {}; unset it or point it at {}",
                path.display(),
                BROWSER_WORKER_RELATIVE
            ),
            searched_paths: vec![path],
        });
    }

    let candidates = worker_script_candidates(exe_path, manifest_dir);
    for (path, source) in &candidates {
        if path.exists() {
            let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            return Ok(WorkerScriptResolution {
                path,
                source: (*source).to_string(),
            });
        }
    }

    Err(WorkerScriptSearch {
        message: format!(
            "browser worker script not found; searched {}; install Allie with bundled worker assets, run from a checkout, or set {BROWSER_WORKER_ENV}",
            candidates
                .iter()
                .map(|(path, _)| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        searched_paths: candidates.into_iter().map(|(path, _)| path).collect(),
    })
}

fn worker_script_candidates(exe_path: &Path, manifest_dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
    let mut candidates = Vec::new();
    push_worker_candidate(
        &mut candidates,
        exe_dir.join(BROWSER_WORKER_RELATIVE),
        "executable directory",
    );
    push_worker_candidate(
        &mut candidates,
        exe_dir.join("../").join(BROWSER_WORKER_RELATIVE),
        "bundled distribution root",
    );
    push_worker_candidate(
        &mut candidates,
        exe_dir.join("../lib/allie").join(BROWSER_WORKER_RELATIVE),
        "installed lib directory",
    );
    push_worker_candidate(
        &mut candidates,
        exe_dir.join("../share/allie").join(BROWSER_WORKER_RELATIVE),
        "installed share directory",
    );
    push_worker_candidate(
        &mut candidates,
        exe_dir.join("../../").join(BROWSER_WORKER_RELATIVE),
        "cargo target directory",
    );
    push_worker_candidate(
        &mut candidates,
        manifest_dir.join(BROWSER_WORKER_RELATIVE),
        "source checkout",
    );
    candidates
}

fn push_worker_candidate(
    candidates: &mut Vec<(PathBuf, &'static str)>,
    path: PathBuf,
    source: &'static str,
) {
    if !candidates.iter().any(|(existing, _)| existing == &path) {
        candidates.push((path, source));
    }
}

fn check_worker_script(
    resolution: &std::result::Result<WorkerScriptResolution, WorkerScriptSearch>,
) -> DoctorCheck {
    match resolution {
        Ok(resolution) => DoctorCheck {
            name: "browser worker".to_string(),
            status: DoctorCheckStatus::Ok,
            detail: format!("{} ({})", resolution.path.display(), resolution.source),
            fix: None,
        },
        Err(search) => DoctorCheck {
            name: "browser worker".to_string(),
            status: DoctorCheckStatus::Fail,
            detail: search.message.clone(),
            fix: Some(format!(
                "Run from an Allie checkout, install a package that includes {}, or set {BROWSER_WORKER_ENV}. Searched: {}",
                BROWSER_WORKER_RELATIVE,
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
mod tests {
    use super::*;
    use crate::test_support::MODEL_ENV_GUARD;
    use tempfile::tempdir;

    #[test]
    fn worker_script_resolves_from_installed_lib_asset_dir_without_env() {
        let temp = tempdir().unwrap();
        let exe_path = temp.path().join("bin/allie");
        let worker_path = temp.path().join("lib/allie/workers/browser/run.mjs");
        std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
        std::fs::write(&worker_path, "console.log('worker');\n").unwrap();

        let resolution = resolve_worker_script_from(
            None,
            &exe_path,
            &temp.path().join("missing-source-checkout"),
        )
        .unwrap();

        assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
        assert_eq!(resolution.source, "installed lib directory");
    }

    #[test]
    fn worker_script_resolves_from_bundled_distribution_root() {
        let temp = tempdir().unwrap();
        let exe_path = temp.path().join("allie/bin/allie");
        let worker_path = temp.path().join("allie/workers/browser/run.mjs");
        std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
        std::fs::write(&worker_path, "console.log('worker');\n").unwrap();

        let resolution = resolve_worker_script_from(
            None,
            &exe_path,
            &temp.path().join("missing-source-checkout"),
        )
        .unwrap();

        assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
        assert_eq!(resolution.source, "bundled distribution root");
    }

    #[test]
    fn worker_script_env_override_is_authoritative() {
        let temp = tempdir().unwrap();
        let env_path = temp.path().join("custom-worker.mjs");
        let packaged_path = temp.path().join("lib/allie/workers/browser/run.mjs");
        std::fs::create_dir_all(packaged_path.parent().unwrap()).unwrap();
        std::fs::write(&packaged_path, "console.log('packaged');\n").unwrap();
        std::fs::write(&env_path, "console.log('env');\n").unwrap();

        let resolution = resolve_worker_script_from(
            Some(env_path.clone()),
            &temp.path().join("bin/allie"),
            temp.path(),
        )
        .unwrap();

        assert_eq!(resolution.path, std::fs::canonicalize(env_path).unwrap());
        assert_eq!(resolution.source, BROWSER_WORKER_ENV);
    }

    #[test]
    fn missing_env_override_does_not_silently_fallback() {
        let temp = tempdir().unwrap();
        let missing_env_path = temp.path().join("missing-worker.mjs");
        let packaged_path = temp.path().join("lib/allie/workers/browser/run.mjs");
        std::fs::create_dir_all(packaged_path.parent().unwrap()).unwrap();
        std::fs::write(&packaged_path, "console.log('packaged');\n").unwrap();

        let search = resolve_worker_script_from(
            Some(missing_env_path.clone()),
            &temp.path().join("bin/allie"),
            temp.path(),
        )
        .unwrap_err();

        assert!(search.message.contains(BROWSER_WORKER_ENV));
        assert_eq!(search.searched_paths, vec![missing_env_path]);
    }

    #[test]
    fn explicit_missing_manifest_is_a_failed_target_check() {
        let temp = tempdir().unwrap();
        let check = check_target(Some(&temp.path().join("missing.yml")), true);

        assert_eq!(check.name, "target");
        assert_eq!(check.status, DoctorCheckStatus::Fail);
        assert!(check.detail.contains("does not exist"));
    }

    fn write_model_fixture_manifest(dir: &Path, model_yaml: &str) -> PathBuf {
        let manifest_path = dir.join("manifest.yml");
        std::fs::write(
            &manifest_path,
            format!(
                r#"id: doctor-model-fixture
name: Doctor model fixture
app_name: Doctor Fixture App
environment: local
target:
  kind: local_fixture
  fixture_dir: .
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
{model_yaml}
browser:
  viewport:
    width: 1280
    height: 900
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: doctor-model-fixture-path
  description: Doctor model fixture
  states:
    - id: home
      path: /
      description: home
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

    fn clear_model_env() {
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn missing_manifest_path_warns_model_check_instead_of_failing() {
        let check = check_model(None);

        assert_eq!(check.name, "model");
        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.detail.contains("no manifest supplied"));
    }

    #[test]
    fn disabled_model_with_no_resolvable_key_names_the_checked_env_vars() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_env();
        let temp = tempdir().unwrap();
        let manifest_path = write_model_fixture_manifest(temp.path(), "");

        let check = check_model(Some(&manifest_path));

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.detail.contains("OPENROUTER_API_KEY"));
        assert!(check.detail.contains("OPENAI_API_KEY"));
        assert!(check.fix.unwrap().contains("allie init --force"));
    }

    #[test]
    fn disabled_model_with_a_resolvable_key_says_so() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
        }
        let temp = tempdir().unwrap();
        let manifest_path = write_model_fixture_manifest(temp.path(), "");

        let check = check_model(Some(&manifest_path));

        clear_model_env();

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.detail.contains("is off in the manifest"));
        assert!(check.detail.contains("OPENROUTER_API_KEY"));
    }

    #[test]
    fn enabled_model_with_its_key_set_is_ok() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
        }
        let temp = tempdir().unwrap();
        let manifest_path = write_model_fixture_manifest(
            temp.path(),
            "model:\n  enabled: true\n  provider_allowlist:\n    - openrouter\n  zdr_required: false\n  provider: openrouter\n  api_key_env: OPENROUTER_API_KEY\n",
        );

        let check = check_model(Some(&manifest_path));

        clear_model_env();

        assert_eq!(check.status, DoctorCheckStatus::Ok);
        assert!(check.detail.contains("openrouter"));
        assert!(check.detail.contains("OPENROUTER_API_KEY"));
        assert!(check.fix.is_none());
    }

    #[test]
    fn enabled_model_with_its_key_missing_names_it_explicitly() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_env();
        let temp = tempdir().unwrap();
        let manifest_path = write_model_fixture_manifest(
            temp.path(),
            "model:\n  enabled: true\n  provider_allowlist:\n    - openrouter\n  zdr_required: false\n  provider: openrouter\n  api_key_env: SOME_OTHER_ENV\n",
        );

        let check = check_model(Some(&manifest_path));

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.detail.contains("SOME_OTHER_ENV"));
        assert!(check.detail.contains("is not set"));
        assert!(check.fix.unwrap().contains("SOME_OTHER_ENV"));
    }
}

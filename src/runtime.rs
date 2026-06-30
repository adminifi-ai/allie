use crate::worker;
use crate::{AllieError, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct RunContext {
    clock: RunClock,
    pub(crate) provenance: GitProvenance,
    fixture_port: Option<u16>,
}

impl RunContext {
    pub(crate) fn from_manifest_path(
        manifest_path: &Path,
        project_root: Option<&Path>,
    ) -> Result<Self> {
        let project_root = project_git_root(manifest_path, project_root)?;
        Ok(Self {
            clock: RunClock::from_env()?,
            provenance: GitProvenance::read(&project_root)?,
            fixture_port: fixture_port_from_env()?,
        })
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    pub(crate) fn new_run_id(&self) -> String {
        format!("run-{}", self.clock.current_millis())
    }

    pub(crate) fn worker_determinism(&self) -> Option<worker::WorkerDeterminism> {
        if self.clock.frozen_at.is_none() && self.fixture_port.is_none() {
            return None;
        }
        Some(worker::WorkerDeterminism {
            timestamp: self
                .clock
                .frozen_at
                .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)),
            fixture_port: self.fixture_port,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct RunClock {
    frozen_at: Option<DateTime<Utc>>,
}

impl RunClock {
    fn from_env() -> Result<Self> {
        let frozen_at = match std::env::var("SOURCE_DATE_EPOCH") {
            Ok(raw) => Some(parse_source_date_epoch(&raw)?),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(AllieError::Runtime(
                    "SOURCE_DATE_EPOCH must be valid unicode integer seconds".to_string(),
                ));
            }
        };
        Ok(Self { frozen_at })
    }

    fn now(&self) -> DateTime<Utc> {
        self.frozen_at.unwrap_or_else(Utc::now)
    }

    fn current_millis(&self) -> u128 {
        self.frozen_at
            .map(|timestamp| {
                timestamp.timestamp() as u128 * 1000
                    + u128::from(timestamp.timestamp_subsec_millis())
            })
            .unwrap_or_else(current_time_millis)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GitProvenance {
    pub(crate) sha: String,
    pub(crate) branch: String,
}

impl GitProvenance {
    pub(crate) fn read(project_root: &Path) -> Result<Self> {
        Ok(Self {
            sha: required_git_metadata_at(
                project_root,
                &["rev-parse", "--short", "HEAD"],
                "git_sha",
            )?,
            branch: required_git_metadata_at(
                project_root,
                &["rev-parse", "--abbrev-ref", "HEAD"],
                "git_branch",
            )?,
        })
    }
}

pub(crate) fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn project_git_root(manifest_path: &Path, project_root: Option<&Path>) -> Result<PathBuf> {
    if let Some(project_root) = project_root {
        return required_git_root(project_root);
    }
    let manifest_dir = manifest_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    required_git_root(manifest_dir)
}

fn required_git_root(start: &Path) -> Result<PathBuf> {
    required_git_metadata_at(start, &["rev-parse", "--show-toplevel"], "project_root")
        .map(PathBuf::from)
}

fn required_git_metadata_at(cwd: &Path, args: &[&str], field: &str) -> Result<String> {
    git_metadata_at(cwd, args)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AllieError::Provenance(format!(
                "{field} is required; run Allie against a git checkout with at least one commit"
            ))
        })
}

fn fixture_port_from_env() -> Result<Option<u16>> {
    let raw = match std::env::var("ALLIE_FIXTURE_PORT") {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(AllieError::Runtime(
                "ALLIE_FIXTURE_PORT must be valid unicode".to_string(),
            ));
        }
    };
    let port = raw.parse::<u16>().map_err(|_| {
        AllieError::Runtime(format!(
            "ALLIE_FIXTURE_PORT must be an integer between 1 and 65535, got {raw:?}"
        ))
    })?;
    if port == 0 {
        return Err(AllieError::Runtime(
            "ALLIE_FIXTURE_PORT must be between 1 and 65535".to_string(),
        ));
    }
    Ok(Some(port))
}

fn parse_source_date_epoch(raw: &str) -> Result<DateTime<Utc>> {
    let seconds = raw.parse::<i64>().map_err(|_| {
        AllieError::Runtime(format!(
            "SOURCE_DATE_EPOCH must be integer seconds, got {raw:?}"
        ))
    })?;
    if seconds < 0 {
        return Err(AllieError::Runtime(
            "SOURCE_DATE_EPOCH must be non-negative integer seconds".to_string(),
        ));
    }
    DateTime::<Utc>::from_timestamp(seconds, 0).ok_or_else(|| {
        AllieError::Runtime("SOURCE_DATE_EPOCH is outside chrono's supported range".to_string())
    })
}

#[cfg(test)]
pub(crate) fn git_metadata(args: &[&str]) -> Option<String> {
    git_metadata_at(Path::new("."), args)
}

pub(crate) fn git_metadata_at(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

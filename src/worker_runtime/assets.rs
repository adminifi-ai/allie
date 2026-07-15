use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkerAsset {
    pub(super) label: &'static str,
    pub(super) env_var: &'static str,
    pub(super) relative_path: &'static str,
}

pub(super) const BROWSER_WORKER: WorkerAsset = WorkerAsset {
    label: "browser worker",
    env_var: "ALLIE_BROWSER_WORKER",
    relative_path: "workers/browser/run.mjs",
};
pub(super) const AGENTIC_WORKER: WorkerAsset = WorkerAsset {
    label: "agentic worker",
    env_var: "ALLIE_AGENTIC_WORKER",
    relative_path: "workers/agentic/review.mjs",
};

#[derive(Debug)]
pub(super) struct WorkerScriptResolution {
    pub(super) path: PathBuf,
    pub(super) source: String,
}

#[derive(Debug)]
pub(super) struct WorkerScriptSearch {
    pub(super) message: String,
    pub(super) searched_paths: Vec<PathBuf>,
}

pub(crate) fn browser_worker_script() -> std::result::Result<PathBuf, String> {
    worker_script(BROWSER_WORKER)
}

pub(crate) fn agentic_worker_script() -> std::result::Result<PathBuf, String> {
    worker_script(AGENTIC_WORKER)
}

fn worker_script(worker: WorkerAsset) -> std::result::Result<PathBuf, String> {
    resolve_worker_script(worker)
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

pub(super) fn resolve_worker_script(
    worker: WorkerAsset,
) -> std::result::Result<WorkerScriptResolution, WorkerScriptSearch> {
    let env_override = std::env::var_os(worker.env_var).map(PathBuf::from);
    resolve_worker_script_with_executable(worker, env_override, std::env::current_exe())
}

#[cfg(test)]
fn resolve_worker_script_from(
    worker: WorkerAsset,
    env_override: Option<PathBuf>,
    exe_path: &Path,
) -> std::result::Result<WorkerScriptResolution, WorkerScriptSearch> {
    resolve_worker_script_with_executable(worker, env_override, Ok(exe_path.to_path_buf()))
}

fn resolve_worker_script_with_executable(
    worker: WorkerAsset,
    env_override: Option<PathBuf>,
    exe_path: std::io::Result<PathBuf>,
) -> std::result::Result<WorkerScriptResolution, WorkerScriptSearch> {
    if let Some(path) = env_override {
        if path.exists() {
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            return Ok(WorkerScriptResolution {
                path,
                source: worker.env_var.to_string(),
            });
        }
        return Err(WorkerScriptSearch {
            message: format!(
                "{} points to missing {} at {}; unset it or point it at {}",
                worker.env_var,
                worker.label,
                path.display(),
                worker.relative_path
            ),
            searched_paths: vec![path],
        });
    }

    let exe_path = exe_path.map_err(|error| WorkerScriptSearch {
        message: format!(
            "cannot resolve current executable for {}: {error}; set {} explicitly",
            worker.label, worker.env_var
        ),
        searched_paths: Vec::new(),
    })?;

    let candidates = worker_script_candidates(worker, &exe_path);
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
            "{} script not found; searched {}; install Allie with bundled worker assets, run from a checkout, or set {}",
            worker.label,
            candidates
                .iter()
                .map(|(path, _)| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            worker.env_var
        ),
        searched_paths: candidates.into_iter().map(|(path, _)| path).collect(),
    })
}

fn worker_script_candidates(worker: WorkerAsset, exe_path: &Path) -> Vec<(PathBuf, &'static str)> {
    let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
    let layouts = [
        ("", "executable directory"),
        ("../", "bundled distribution root"),
        ("../lib/allie", "installed lib directory"),
        ("../share/allie", "installed share directory"),
        ("../../", "cargo target directory"),
        ("../../../", "cargo test target directory"),
    ];
    let mut candidates = Vec::new();
    for (root, source) in layouts {
        let path = exe_dir.join(root).join(worker.relative_path);
        if !candidates.iter().any(|(existing, _)| existing == &path) {
            candidates.push((path, source));
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn both_workers_resolve_from_installed_and_bundled_layouts() {
        for (exe_relative, root_relative, source) in [
            ("bin/allie", "lib/allie", "installed lib directory"),
            ("bin/allie", "share/allie", "installed share directory"),
            ("allie/bin/allie", "allie", "bundled distribution root"),
        ] {
            let temp = tempdir().unwrap();
            let exe_path = temp.path().join(exe_relative);
            std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
            for worker in [BROWSER_WORKER, AGENTIC_WORKER] {
                let worker_path = temp.path().join(root_relative).join(worker.relative_path);
                std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
                std::fs::write(&worker_path, "console.log('worker');\n").unwrap();
                let resolution = resolve_worker_script_from(worker, None, &exe_path).unwrap();
                assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
                assert_eq!(resolution.source, source);
            }
        }
    }

    #[test]
    fn candidate_paths_are_deduplicated_for_each_worker() {
        let exe_path = Path::new("target/debug/allie");
        for worker in [BROWSER_WORKER, AGENTIC_WORKER] {
            let candidates = worker_script_candidates(worker, exe_path);
            let unique = candidates
                .iter()
                .map(|(path, _)| path)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(candidates.len(), unique.len());
        }
    }

    #[test]
    fn both_workers_resolve_from_cargo_binary_and_test_layouts() {
        for (exe_relative, source) in [
            ("target/debug/allie", "cargo target directory"),
            (
                "target/debug/deps/allie-test-harness",
                "cargo test target directory",
            ),
        ] {
            let temp = tempdir().unwrap();
            let exe_path = temp.path().join(exe_relative);
            std::fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
            for worker in [BROWSER_WORKER, AGENTIC_WORKER] {
                let worker_path = temp.path().join(worker.relative_path);
                std::fs::create_dir_all(worker_path.parent().unwrap()).unwrap();
                std::fs::write(&worker_path, "console.log('worker');\n").unwrap();
                let resolution = resolve_worker_script_from(worker, None, &exe_path).unwrap();
                assert_eq!(resolution.path, std::fs::canonicalize(worker_path).unwrap());
                assert_eq!(resolution.source, source);
            }
        }
    }

    #[test]
    fn each_worker_env_override_is_authoritative_even_when_missing() {
        let temp = tempdir().unwrap();
        for worker in [BROWSER_WORKER, AGENTIC_WORKER] {
            let env_path = temp.path().join(format!("custom-{}.mjs", worker.label));
            let packaged_path = temp.path().join("lib/allie").join(worker.relative_path);
            std::fs::create_dir_all(packaged_path.parent().unwrap()).unwrap();
            std::fs::write(&packaged_path, "console.log('packaged');\n").unwrap();
            std::fs::write(&env_path, "console.log('env');\n").unwrap();
            let resolution = resolve_worker_script_from(
                worker,
                Some(env_path.clone()),
                &temp.path().join("bin/allie"),
            )
            .unwrap();
            assert_eq!(resolution.path, std::fs::canonicalize(&env_path).unwrap());
            assert_eq!(resolution.source, worker.env_var);

            std::fs::remove_file(&env_path).unwrap();
            let search = resolve_worker_script_from(
                worker,
                Some(env_path.clone()),
                &temp.path().join("bin/allie"),
            )
            .unwrap_err();
            assert!(search.message.contains(worker.env_var));
            assert_eq!(search.searched_paths, vec![env_path]);
        }
    }

    #[test]
    fn executable_lookup_failure_is_fail_closed_but_does_not_block_an_override() {
        let temp = tempdir().unwrap();
        let override_path = temp.path().join("worker.mjs");
        std::fs::write(&override_path, "console.log('worker');\n").unwrap();

        let resolution = resolve_worker_script_with_executable(
            BROWSER_WORKER,
            Some(override_path.clone()),
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "denied",
            )),
        )
        .unwrap();
        assert_eq!(
            resolution.path,
            std::fs::canonicalize(override_path).unwrap()
        );

        let search = resolve_worker_script_with_executable(
            BROWSER_WORKER,
            None,
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "denied",
            )),
        )
        .unwrap_err();
        assert!(search.message.contains("cannot resolve current executable"));
        assert!(search.message.contains(BROWSER_WORKER.env_var));
        assert!(search.searched_paths.is_empty());
    }

    #[test]
    fn agentic_worker_gets_bundled_playwright_environment() {
        let temp = tempdir().unwrap();
        let script = temp.path().join("allie/workers/agentic/review.mjs");
        let browsers = temp.path().join("allie/ms-playwright");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&browsers).unwrap();
        let mut command = Command::new("node");
        apply_worker_environment(&mut command, &script);
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == "PLAYWRIGHT_BROWSERS_PATH")
                .and_then(|(_, value)| value),
            Some(browsers.as_os_str())
        );
    }
}

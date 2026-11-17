//! The optional faithful renderer: the official `helm template`, confined.
//!
//! Confinement: the environment is cleared and Helm's config, cache and data
//! homes point at a throwaway directory, so no repository, plugin or
//! credential of the host is read. The command is `helm template` only: no
//! `--dependency-update`, no `--repo`, no `--validate`, so it does not reach
//! the network or a cluster. The run is bounded in time and output.
//!
//! A binary counts as available only if it answers `helm version` inside that
//! same confinement. A launcher shim that resolves the real binary through
//! the caller's `PATH` or `HOME` (the shims of a version manager) is found on
//! `PATH` but cannot run here, and is reported as unavailable with the reason.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::iac::helm::{ChartRenderer, RenderRequest, RenderResponse};

use super::rendered_stream::parse_rendered_stream;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;

/// A renderer backed by a `helm` binary, if one can run.
#[derive(Debug, Clone)]
pub struct HelmCli {
    /// The binary that answered the probe, or why there is none.
    engine: Result<PathBuf, String>,
    timeout: Duration,
}

impl HelmCli {
    /// Look for `helm` on `PATH` and probe the first one found. When it is
    /// absent, or cannot run in the confined environment, the renderer still
    /// exists and answers [`RenderResponse::Unavailable`] with the reason.
    pub fn from_path() -> Self {
        Self::from_search_path(std::env::var_os("PATH"))
    }

    fn from_search_path(search_path: Option<OsString>) -> Self {
        match find_on_path("helm", search_path) {
            Some(binary) => Self::with_binary(binary),
            None => Self::unavailable("`helm` binary not found on PATH".to_string()),
        }
    }

    /// Use a specific binary, which is probed like one found on `PATH`.
    pub fn with_binary(binary: PathBuf) -> Self {
        match probe(&binary) {
            Ok(()) => Self {
                engine: Ok(binary),
                timeout: DEFAULT_TIMEOUT,
            },
            Err(message) => Self::unavailable(format!(
                "`{}` cannot run in the confined environment ({message}); \
                 it must be a real binary, not a launcher that needs the caller's PATH or HOME",
                binary.display()
            )),
        }
    }

    fn unavailable(reason: String) -> Self {
        Self {
            engine: Err(reason),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Bound the run time of one render.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Whether a binary was found and answered the probe in the confined
    /// environment.
    pub fn is_available(&self) -> bool {
        self.engine.is_ok()
    }

    /// Why no render can run, or `None` when the engine is available.
    pub fn unavailable_reason(&self) -> Option<&str> {
        self.engine.as_ref().err().map(String::as_str)
    }
}

fn find_on_path(name: &str, search_path: Option<OsString>) -> Option<PathBuf> {
    std::env::split_paths(&search_path?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

impl ChartRenderer for HelmCli {
    fn render(&self, request: &RenderRequest<'_>) -> RenderResponse {
        let binary = match &self.engine {
            Ok(binary) => binary,
            Err(reason) => {
                return RenderResponse::Unavailable {
                    reason: reason.clone(),
                };
            }
        };
        match run_template(binary, request, self.timeout) {
            Ok(stdout) => match parse_rendered_stream(&stdout) {
                Ok(documents) => RenderResponse::Rendered(documents),
                Err(message) => RenderResponse::Failed {
                    message: format!("helm output was not a YAML stream: {message}"),
                },
            },
            Err(message) => RenderResponse::Failed { message },
        }
    }
}

/// The one confinement every helm run shares: cleared environment, Helm homes
/// in the scratch directory, no stdin.
fn confined_command(binary: &Path, scratch: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .env_clear()
        .env("HOME", scratch)
        .env("HELM_CACHE_HOME", scratch.join("cache"))
        .env("HELM_CONFIG_HOME", scratch.join("config"))
        .env("HELM_DATA_HOME", scratch.join("data"))
        .current_dir(scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Ask the binary for its version inside the confinement a render runs in.
fn probe(binary: &Path) -> Result<(), String> {
    let scratch = tempfile::tempdir().map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let mut command = confined_command(binary, scratch.path());
    command.arg("version");
    run_bounded(command, PROBE_TIMEOUT).map(drop)
}

fn run_template(
    binary: &Path,
    request: &RenderRequest<'_>,
    timeout: Duration,
) -> Result<String, String> {
    let scratch = tempfile::tempdir().map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let mut command = confined_command(binary, scratch.path());
    command
        .arg("template")
        .arg(request.release_name)
        .arg(&request.chart.location);
    for (index, layer) in request.overrides.iter().enumerate() {
        let file = scratch.path().join(format!("values-{index}.yaml"));
        let text = serde_norway::to_string(layer)
            .map_err(|e| format!("cannot serialise values layer {index}: {e}"))?;
        std::fs::write(&file, text).map_err(|e| format!("cannot write values layer: {e}"))?;
        command.arg("--values").arg(file);
    }
    run_bounded(command, timeout)
}

fn run_bounded(mut command: Command, timeout: Duration) -> Result<String, String> {
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start helm: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "helm exceeded the {}s time limit",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(format!("cannot wait for helm: {error}")),
        }
    };
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if status.success() {
        Ok(out)
    } else {
        Err(format!("helm exited with {status}: {}", err.trim()))
    }
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stream) = stream {
            let _ = stream.take(MAX_OUTPUT_BYTES).read_to_string(&mut text);
        }
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::iac::helm::ChartModel;
    use serde_json::Value;

    fn chart() -> ChartModel {
        ChartModel {
            name: "c".into(),
            version: "1".into(),
            location: PathBuf::from("."),
            content_digest: String::new(),
            default_values: Value::Null,
            schema: None,
            dependencies: vec![],
            templates: vec![],
            unread_template_dirs: vec![],
        }
    }

    #[test]
    fn a_missing_binary_degrades_to_unavailable_without_running_anything() {
        let cli = HelmCli::from_search_path(None);
        let chart = chart();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        assert!(
            matches!(response, RenderResponse::Unavailable { reason } if reason.contains("helm"))
        );
        assert!(!cli.is_available());
        assert!(cli.unavailable_reason().unwrap().contains("not found"));
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_engine_is_reported_with_its_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let cli = HelmCli::with_binary(executable(
            dir.path(),
            "failing-helm",
            "#!/bin/sh\n[ \"$1\" = version ] && exit 0\necho 'chart is broken' >&2\nexit 1\n",
        ));
        let chart = chart();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        assert!(
            matches!(response, RenderResponse::Failed { message } if message.contains("exited") && message.contains("chart is broken"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_engine_runs_with_a_cleared_environment_and_values_layers_as_files() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-helm");
        // Prints the argument list and the environment as a Source-tagged document.
        std::fs::write(
            &script,
            "#!/bin/sh\necho '---'\necho '# Source: c/templates/cm.yaml'\necho 'kind: ConfigMap'\necho 'metadata:'\necho \"  name: args-$#\"\necho \"data:\"\necho \"  secret: ${SECRET_TOKEN:-absent}\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY of the test: the variable is only read by the child, which must not see it.
        let cli = HelmCli::with_binary(script);
        let chart = chart();
        let layers = [serde_json::json!({"a": 1}), serde_json::json!({"b": 2})];
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &layers,
            release_name: "r",
        });
        let RenderResponse::Rendered(docs) = response else {
            panic!("expected a render, got {response:?}")
        };
        // template + release + chart + two `--values file` pairs = 7 arguments.
        assert_eq!(docs[0].value["metadata"]["name"], "args-7");
        assert_eq!(docs[0].value["data"]["secret"], "absent");
    }

    #[cfg(unix)]
    #[test]
    fn a_slow_engine_is_killed_at_the_time_limit() {
        let dir = tempfile::tempdir().unwrap();
        let script = executable(
            dir.path(),
            "slow-helm",
            "#!/bin/sh\n[ \"$1\" = version ] && exit 0\nsleep 5\n",
        );
        let cli = HelmCli::with_binary(script).with_timeout(Duration::from_millis(200));
        let chart = chart();
        let started = Instant::now();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        assert!(
            matches!(response, RenderResponse::Failed { message } if message.contains("time limit"))
        );
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[cfg(unix)]
    fn executable(dir: &Path, name: &str, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A launcher shim like the one a version manager puts on `PATH`: it
    /// resolves the real binary through `PATH`, which the confined
    /// environment does not carry.
    #[cfg(unix)]
    fn shim_that_needs_the_caller_environment(dir: &Path) -> PathBuf {
        executable(dir, "launcher", "#!/bin/sh\necho 'v3.0.0'\n");
        executable(dir, "helm", "#!/bin/sh\nexec launcher \"$@\"\n")
    }

    #[cfg(unix)]
    #[test]
    fn the_shim_only_fails_because_the_confined_environment_lacks_path() {
        let dir = tempfile::tempdir().unwrap();
        let shim = shim_that_needs_the_caller_environment(dir.path());
        let with_path = Command::new(&shim)
            .env_clear()
            .env("PATH", dir.path())
            .output()
            .unwrap();
        assert!(with_path.status.success());
        let confined = Command::new(&shim).env_clear().output().unwrap();
        assert!(!confined.status.success());
    }

    #[cfg(unix)]
    #[test]
    fn a_binary_that_cannot_run_in_the_confined_environment_is_unavailable_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let shim = shim_that_needs_the_caller_environment(dir.path());
        let cli = HelmCli::with_binary(shim.clone());
        assert!(!cli.is_available());
        let chart = chart();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        let RenderResponse::Unavailable { reason } = response else {
            panic!("expected Unavailable, got {response:?}")
        };
        assert_eq!(cli.unavailable_reason(), Some(reason.as_str()));
        assert!(reason.contains(&shim.display().to_string()), "{reason}");
        assert!(reason.contains("confined environment"), "{reason}");
        assert!(reason.contains("not found"), "{reason}");
    }

    #[cfg(unix)]
    #[test]
    fn a_shim_found_on_the_search_path_is_unavailable_not_a_failing_engine() {
        let dir = tempfile::tempdir().unwrap();
        shim_that_needs_the_caller_environment(dir.path());
        let cli = HelmCli::from_search_path(Some(dir.path().as_os_str().to_owned()));
        assert!(!cli.is_available());
        let chart = chart();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        assert!(
            matches!(response, RenderResponse::Unavailable { .. }),
            "{response:?}"
        );
    }
}

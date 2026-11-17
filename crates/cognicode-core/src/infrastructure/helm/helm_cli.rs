//! The optional faithful renderer: the official `helm template`, confined.
//!
//! Confinement: the environment is cleared and Helm's config, cache and data
//! homes point at a throwaway directory, so no repository, plugin or
//! credential of the host is read. The command is `helm template` only: no
//! `--dependency-update`, no `--repo`, no `--validate`, so it does not reach
//! the network or a cluster. The run is bounded in time and output.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::iac::helm::{ChartRenderer, RenderRequest, RenderResponse};

use super::rendered_stream::parse_rendered_stream;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;

/// A renderer backed by a `helm` binary, if one exists.
#[derive(Debug, Clone)]
pub struct HelmCli {
    binary: Option<PathBuf>,
    timeout: Duration,
}

impl HelmCli {
    /// Look for `helm` on `PATH`. When it is absent the renderer still exists
    /// and answers [`RenderResponse::Unavailable`].
    pub fn from_path() -> Self {
        Self {
            binary: find_on_path("helm"),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Use a specific binary.
    pub fn with_binary(binary: PathBuf) -> Self {
        Self {
            binary: Some(binary),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Bound the run time of one render.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Whether a binary was found.
    pub fn is_available(&self) -> bool {
        self.binary.is_some()
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

impl ChartRenderer for HelmCli {
    fn render(&self, request: &RenderRequest<'_>) -> RenderResponse {
        let Some(binary) = &self.binary else {
            return RenderResponse::Unavailable {
                reason: "`helm` binary not found on PATH".to_string(),
            };
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

fn run_template(
    binary: &Path,
    request: &RenderRequest<'_>,
    timeout: Duration,
) -> Result<String, String> {
    let scratch = tempfile::tempdir().map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let mut command = Command::new(binary);
    command
        .arg("template")
        .arg(request.release_name)
        .arg(&request.chart.location)
        .env_clear()
        .env("HOME", scratch.path())
        .env("HELM_CACHE_HOME", scratch.path().join("cache"))
        .env("HELM_CONFIG_HOME", scratch.path().join("config"))
        .env("HELM_DATA_HOME", scratch.path().join("data"))
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (index, layer) in request.overrides.iter().enumerate() {
        let file = scratch.path().join(format!("values-{index}.yaml"));
        let text = serde_norway::to_string(layer)
            .map_err(|e| format!("cannot serialise values layer {index}: {e}"))?;
        std::fs::write(&file, text).map_err(|e| format!("cannot write values layer: {e}"))?;
        command.arg("--values").arg(file);
    }
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
        }
    }

    #[test]
    fn a_missing_binary_degrades_to_unavailable_without_running_anything() {
        let cli = HelmCli {
            binary: None,
            timeout: DEFAULT_TIMEOUT,
        };
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
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_engine_is_reported_with_its_diagnostic() {
        let cli = HelmCli::with_binary(PathBuf::from("/bin/false"));
        let chart = chart();
        let response = cli.render(&RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        });
        assert!(
            matches!(response, RenderResponse::Failed { message } if message.contains("exited"))
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
        let script = dir.path().join("slow-helm");
        std::fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
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
}

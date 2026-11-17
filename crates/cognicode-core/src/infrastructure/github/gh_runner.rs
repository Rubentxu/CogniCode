//! The effect boundary of the GitHub review adapter: the only place a
//! process is spawned. The adapter above it builds argv; this runs it.
//!
//! The program is `gh`, fixed at construction and never taken from a
//! request. It is run without a shell, with a cleared environment that
//! carries only what `gh` needs to find its configuration and credential
//! (an allowlist, so nothing else of the analysis process leaks into it),
//! a bounded capture of its output, and a deadline.

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::forge::ForgeError;
use crate::infrastructure::git::scm_adapter::read_bounded;

/// Environment variables `gh` is allowed to see.
const ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_HOST",
    "GH_CONFIG_DIR",
    "XDG_CONFIG_HOME",
];

const STDOUT_CAP: u64 = 8 << 20;
const STDERR_CAP: u64 = 64 << 10;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// One invocation: the arguments after the program, and the bytes to
/// feed on standard input, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    pub args: Vec<String>,
    pub stdin: Option<Vec<u8>>,
}

/// What the program answered. A non-zero `code` is data for the caller
/// to classify, not an error of the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs one call of the forge's command line tool. Implementations fail
/// with [`ForgeError::ProviderFailure`] when the program cannot be run,
/// its output exceeds the cap or it does not finish in time.
pub trait CommandRunner: Send + Sync {
    fn run(&self, call: &CommandCall) -> Result<CommandOutput, ForgeError>;
}

/// Runs the system `gh`.
///
/// The credential comes from the environment `gh` is given
/// (`GH_TOKEN`/`GITHUB_TOKEN`) or its own configuration. It exists only
/// in this process boundary: the evaluator of a pull request never
/// receives it. Installations that expose `gh` through a version-manager
/// shim need `with_program` pointing at the real binary, since the
/// shim's own variables are not passed through.
#[derive(Debug, Clone)]
pub struct GhCliRunner {
    program: OsString,
    timeout: Duration,
}

impl Default for GhCliRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl GhCliRunner {
    pub fn new() -> Self {
        Self {
            program: OsString::from("gh"),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Binds the runner to an explicit `gh` binary (a pinned path, or a
    /// stand-in for the "program absent" failure mode).
    pub fn with_program(program: &str) -> Self {
        Self {
            program: OsString::from(program),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Replaces the deadline of every call.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

fn failure(detail: impl Into<String>) -> ForgeError {
    ForgeError::ProviderFailure {
        detail: detail.into(),
    }
}

impl CommandRunner for GhCliRunner {
    fn run(&self, call: &CommandCall) -> Result<CommandOutput, ForgeError> {
        let mut command = Command::new(&self.program);
        command
            .args(&call.args)
            .env_clear()
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(if call.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in ENV_ALLOWLIST {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command
            .spawn()
            .map_err(|error| failure(format!("cannot spawn gh: {error}")))?;

        if let (Some(bytes), Some(mut stdin)) = (call.stdin.as_deref(), child.stdin.take()) {
            // A program that exits early closes the pipe; its exit
            // status, read below, is the answer then.
            let _ = stdin.write_all(bytes);
        }

        let stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| failure("gh stdout pipe missing"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| failure("gh stderr pipe missing"))?;
        let stdout_join = std::thread::spawn(move || read_bounded(stdout_pipe, STDOUT_CAP));
        let stderr_join = std::thread::spawn(move || read_bounded(stderr_pipe, STDERR_CAP));

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(failure(format!(
                        "gh did not finish within {} seconds",
                        self.timeout.as_secs()
                    )));
                }
                Ok(None) => std::thread::sleep(POLL_INTERVAL),
                Err(error) => return Err(failure(format!("cannot wait for gh: {error}"))),
            }
        };

        let stdout = stdout_join
            .join()
            .map_err(|_| failure("gh stdout reader panicked"))?
            .map_err(|error| failure(format!("cannot read gh stdout: {error}")))?
            .ok_or_else(|| {
                failure(format!(
                    "gh output exceeded the {STDOUT_CAP} byte cap; refusing a partial read"
                ))
            })?;
        // Diagnostics are best effort: a stderr over its cap is dropped.
        let stderr = stderr_join.join().ok().and_then(Result::ok).flatten();
        Ok(CommandOutput {
            code: status.code().unwrap_or(-1),
            stdout,
            stderr: stderr.unwrap_or_default(),
        })
    }
}

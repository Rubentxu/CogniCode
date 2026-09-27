//! Harness compartido para UATs de persistencia (binario real).
//! Duplicación eliminada tras §70-§73: un único spawn + call_tool.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// Resolve the absolute path to the `cognicode-mcp` binary under test.
///
/// Convenience wrapper for `binary_path_for("cognicode-mcp")`. Existing
/// callers that don't care about the binary name (the mcp crate only
/// has one binary, `cognicode-mcp`) can keep using this form unchanged.
pub fn binary_path() -> PathBuf {
    binary_path_for("cognicode-mcp")
}

/// Resolve the absolute path to the named binary under test.
///
/// Resolution order (first match wins):
///
/// 1. `CARGO_BIN_EXE_<name>` — set by Cargo when an integration test in
///    the same crate as the binary is run; always correct, no filesystem
///    traversal required. Falls back gracefully when unset.
/// 2. `CARGO_BIN_EXE_<name>` at **runtime** — set by `cargo-nextest` and
///    by wrappers that invoke cargo manually. This branch is the
///    difference between "tests run under cargo-nextest" and "tests
///    fail with confusing build error under cargo-nextest" (§125.V7).
/// 3. `CARGO_TARGET_DIR/release/<name>` — honors the user's cargo
///    target-dir override (e.g. `~/.cargo/config.toml` redirecting
///    `target-dir` away from the workspace `target/`).
/// 4. `<workspace_root>/target/release/<name>` — historical fallback
///    (works when cargo and the workspace agree on `target/`).
///
/// Resolution is delegated to `resolve_binary_path_for` so the
/// precedence list is the single source of truth across every test
/// that needs the binary.
pub fn binary_path_for(name: &str) -> PathBuf {
    resolve_binary_path_for(name)
}

fn resolve_binary_path_for(name: &str) -> PathBuf {
    // 1. Compile-time env var.
    if let Some(p) = compile_time_bin_exe(name) {
        return PathBuf::from(p);
    }

    // 2. Runtime env var (cargo-nextest).
    if let Some(p) = runtime_bin_exe(name) {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }

    // 3. CARGO_TARGET_DIR override (release profile).
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let release = PathBuf::from(&target).join("release").join(name);
        if release.exists() {
            return release;
        }
        let debug = PathBuf::from(&target).join("debug").join(name);
        if debug.exists() {
            return debug;
        }
    }

    // 4. Workspace-relative fallback.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let release = workspace_root.join("target").join("release").join(name);
    if release.exists() {
        return release;
    }
    let debug = workspace_root.join("target").join("debug").join(name);
    if debug.exists() {
        // A stale binary here is worse than no binary at all: a UAT would run
        // against whatever was built hours ago and report a green result that
        // says nothing about the code under review. When Cargo is actually
        // writing somewhere else (a shared `CARGO_TARGET_DIR`, or the
        // `.cargo/config.toml` this workspace sets), prefer that build even
        // though `CARGO_TARGET_DIR` is unset in the test's environment.
        if let Some(fresh) = cargo_configured_target_dir(workspace_root.join("target")) {
            return fresh.join("debug").join(name);
        }
        return debug;
    }
    debug
}

/// The target directory Cargo is configured to write to, if the workspace
/// declares one in `.cargo/config.toml`.
///
/// Returns `None` when the build lands in the default location, in which case
/// the caller already has the right path and this is all noise.
fn cargo_configured_target_dir(default: PathBuf) -> Option<PathBuf> {
    let workspace_root = default.parent()?;
    let config = workspace_root.join(".cargo").join("config.toml");
    let text = std::fs::read_to_string(config).ok()?;
    for line in text.lines() {
        let line = line.trim();
        // Skip commented-out or section-header lines; only a real
        // `target-dir = "..."` under `[build]` redirects the output.
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some(value) = line.strip_prefix("target-dir") else {
            continue;
        };
        let value = value
            .trim_start()
            .strip_prefix('=')?
            .trim()
            .trim_matches('"');
        if value.is_empty() {
            continue;
        }
        let resolved = PathBuf::from(value);
        return Some(if resolved.is_absolute() {
            resolved
        } else {
            workspace_root.join(resolved)
        });
    }
    None
}

fn compile_time_bin_exe(name: &str) -> Option<&'static str> {
    // env! requires a literal string. Match on the known names.
    match name {
        "cognicode-mcp" => option_env!("CARGO_BIN_EXE_cognicode-mcp"),
        _ => None,
    }
}

fn runtime_bin_exe(name: &str) -> Option<std::ffi::OsString> {
    let var = format!("CARGO_BIN_EXE_{name}");
    std::env::var_os(var)
}

pub struct McpSession {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpSession {
    /// Spawn + initialize. `kill_on_drop` como red de seguridad.
    pub async fn spawn(ws: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(ws);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut s = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        s.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat","version":"0"}}})).await?;
        let mut line = String::new();
        s.stdout
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if line.is_empty() {
            return Err("sin respuesta a initialize".into());
        }
        s.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        s.next_id = 2;
        Ok(s)
    }

    async fn send(&mut self, v: &Value) -> Result<(), String> {
        self.stdin
            .write_all(format!("{}\n", v).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())
    }

    /// tools/call + lectura de la respuesta con el id esperado.
    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await?;
        let mut resp = String::new();
        loop {
            resp.clear();
            let n = self
                .stdout
                .read_line(&mut resp)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("server cerró stdout".into());
            }
            let m: Value = serde_json::from_str(resp.trim()).map_err(|e| e.to_string())?;
            if m.get("id").and_then(|v| v.as_u64()) == Some(id) {
                let text = m["result"]["content"][0]["text"]
                    .as_str()
                    .ok_or("no content")?;
                return serde_json::from_str(text).map_err(|e| e.to_string());
            }
        }
    }

    /// Cierre completo: stdin shutdown + kill + wait (no huérfanos).
    pub async fn shutdown(mut self) {
        let _ = self.stdin.shutdown().await;
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

/// Absolute path to the `release/` directory that contains the
/// release-profile binaries (cogh, cognicode, cognicode-mcp, ...).
///
/// Resolution order (first match wins):
///
/// 1. `<probe parent>/release/<self_name>` — when the resolved
///    binary happens to live in the release profile (e.g.
///    `cargo test --release -p cognicode-mcp`).
/// 2. `<probe parent>/release/` — sibling sibling directory
///    (when `cargo test -p cognicode-mcp` was run without `--release`,
///    the resolved debug binary points at `debug/cognicode-mcp`,
///    but a sibling `release/` dir is expected to exist if the user
///    built with `cargo build --release --bin cognicode-mcp`).
/// 3. The literal `<repo_root>/target/release` — workspace default.
///
/// Lesson 84 (M0.13): never hard-code `<repo_root>/target/release`
/// because it breaks under a global `~/.cargo/config.toml`
/// target-dir override. We honour the same location Cargo chose for
/// the binary resolved by `binary_path`, falling back to the
/// repo-root default only when that fails.
pub fn release_dir() -> PathBuf {
    let probe = binary_path();
    // case 1+2: probe parent already is the release dir.
    let parent = probe
        .parent()
        .map(PathBuf::from)
        .expect("release_dir: probe has no parent");
    if parent.ends_with("release") {
        return parent;
    }
    if parent.ends_with("debug") {
        // Sibling release dir at the same level.
        let sibling = parent.parent().map(|p| p.join("release"));
        if let Some(s) = sibling
            && s.exists()
        {
            return s;
        }
    }
    // 3. Workspace-relative fallback.
    repo_root().join("target").join("release")
}

/// Compute the repo root the same way as in binary_path resolution.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `binary_path` must return an absolute, well-formed path that ends with
    /// the binary's filename. The actual existence check is environment
    /// dependent (CARGO_BIN_EXE_* may or may not be set, the workspace target
    /// may or may not be built); this test only pins the path shape so a
    /// refactor that, say, dropped the file name is caught immediately.
    #[test]
    fn binary_path_resolves_to_cognicode_mcp_filename() {
        let path = binary_path();
        assert!(
            path.is_absolute() || path.starts_with("/"),
            "binary path must be absolute, got: {}",
            path.display()
        );
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("cognicode-mcp"),
            "binary path must end with the binary name, got: {}",
            path.display()
        );
    }

    /// When `CARGO_BIN_EXE_cognicode-mcp` is set at compile time, the path
    /// returned must point at that env var. The env var is set by Cargo for
    /// integration tests in the same crate as the binary; without it the
    /// resolution falls through to the other branches.
    #[test]
    fn binary_path_prefers_cargo_bin_exe_env_var_when_set() {
        if let Some(expected) = option_env!("CARGO_BIN_EXE_cognicode-mcp") {
            let path = binary_path();
            assert_eq!(
                path.to_str(),
                Some(expected),
                "binary_path must return CARGO_BIN_EXE_cognicode-mcp when set at compile time"
            );
        }
        // If the env var is unset at compile time, the test is a no-op:
        // `binary_path` falls back to the other resolution branches, which
        // are environment dependent and not worth pinning here.
    }

    /// The binary the UATs will actually run must not predate the code.
    ///
    /// The failure this catches is silent and expensive: a workspace that
    /// builds into a shared `CARGO_TARGET_DIR` keeps an older
    /// `target/debug/<binary>` around, every black-box UAT runs against that
    /// stale build, the suite stays green, and the results describe code that
    /// no longer exists.
    ///
    /// The oracle is the newest library source under `crates/`, not the test
    /// binary. Comparing against the test binary looks tempting and is
    /// wrong: Cargo links the two in the same build and their mtimes differ
    /// by however long the link step took, so a correct setup fails by
    /// milliseconds. Source mtime is the thing that decides whether the
    /// binary reflects the tree.
    #[test]
    fn the_binary_under_test_is_not_older_than_the_sources_it_was_built_from() {
        let path = binary_path();
        assert!(path.exists(), "resolved binary does not exist: {path:?}");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
        let binary_time =
            modified(&path).unwrap_or_else(|| panic!("cannot stat the resolved binary: {path:?}"));
        let Some((newest, newest_time)) = newest_source_under(&workspace_root.join("crates"))
        else {
            // No sources to compare against: nothing to assert.
            return;
        };
        assert!(
            binary_time >= newest_time,
            "the binary the UATs will run is stale: {path:?} (mtime {binary_time:?}) predates \
             {newest:?} ({newest_time:?}); every black-box UAT would run against a build that \
             does not contain the current code"
        );
    }

    /// The most recently modified compilable source under `root`, with mtime.
    ///
    /// `tests/` is excluded on purpose: an integration test never links into
    /// the server binary, so a test edited seconds ago must not make a
    /// perfectly current binary look stale. Only files the binary is actually
    /// built from are a valid oracle.
    fn newest_source_under(root: &Path) -> Option<(PathBuf, std::time::SystemTime)> {
        let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
        // `target` holds build output, not sources; skipping it keeps the walk
        // bounded and stops a stale artifact from counting as input.
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name();
                    if name.is_some_and(|n| n == "target" || n == "tests" || n == "benches") {
                        continue;
                    }
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && let Some(time) = modified(&path)
                    && newest.as_ref().is_none_or(|(_, best)| time > *best)
                {
                    newest = Some((path, time));
                }
            }
        }
        newest
    }

    fn modified(path: &Path) -> Option<std::time::SystemTime> {
        std::fs::metadata(path).ok()?.modified().ok()
    }
}

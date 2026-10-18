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

    // 4. The directory cargo itself reports. Covers a `target-dir` set in
    //    ~/.cargo/config.toml, which is how this machine coexists with four
    //    other OpenCode instances and is invisible as an environment variable.
    if let Some(target) = cargo_target_dir() {
        let release = target.join("release").join(name);
        if release.exists() {
            return release;
        }
        let debug = target.join("debug").join(name);
        if debug.exists() {
            return debug;
        }
    }

    // 5. Workspace-relative fallback (the default layout).
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
        return debug;
    }
    debug
}

/// The target directory Cargo is configured to write to, if the workspace
/// declares one in `.cargo/config.toml`.
///
/// Returns `None` when the build lands in the default location, in which case
/// the caller already has the right path and this is all noise.
fn cargo_target_dir() -> Option<PathBuf> {
    // The only authority that answers "where is Cargo actually writing".
    // `CARGO_TARGET_DIR` is unset here, and a `target-dir` set in
    // ~/.cargo/config.toml is invisible to both the environment and any
    // workspace-local config file, so heuristics that read those two
    // sources report a directory that does not exist. Asking cargo is one
    // subprocess and is right in every configuration.
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let out = std::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let v: Value = serde_json::from_slice(&out.stdout).ok()?;
        Some(PathBuf::from(v.get("target_directory")?.as_str()?))
    })
    .clone()
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

    /// Send any JSON-RPC request and return the raw response value.
    ///
    /// Generic request helper added for A-013 (Black-box lifecycle
    /// UAT). Most callers should keep using `call_tool` (which
    /// understands the `result.content[0].text` envelope) and
    /// `shutdown` (which guarantees the child is reaped). This
    /// method is for lifecycle UAT steps that exercise JSON-RPC
    /// methods outside the tool-call envelope (`tools/list`,
    /// `ping`, future ones) without paying for a second harness.
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
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
                return Ok(m);
            }
        }
    }

    /// Fetch every advertised tool name, following `next_cursor` to the
    /// last page.
    ///
    /// The catalogue is paginated at 20 tools per page, so a single
    /// `tools/list` call does not return the whole surface. Tests that
    /// assert on the advertised set must walk the pages; hand-rolling that
    /// walk in each test is how a suite ends up with three slightly
    /// different paginations, one of which only reads page 1 and calls the
    /// result "the tool list". One implementation, used everywhere.
    pub async fn list_tools(&mut self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut cursor: Option<String> = None;
        // Bounded so a server that returns a self-referential cursor
        // fails the test instead of hanging it.
        for _ in 0..64 {
            let params = match &cursor {
                Some(c) => json!({"cursor": c}),
                None => json!({}),
            };
            let resp = self.request("tools/list", params).await?;
            let result = resp
                .get("result")
                .ok_or_else(|| format!("tools/list sin result: {resp}"))?;
            if let Some(arr) = result.get("tools").and_then(|v| v.as_array()) {
                for t in arr {
                    if let Some(n) = t.get("name").and_then(|v| v.as_str()) {
                        names.push(n.to_string());
                    }
                }
            }
            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(c) => cursor = Some(c.to_string()),
                None => break,
            }
        }
        Ok(names)
    }

    /// Spawn an extra `cognicode-mcp` child process with the given
    /// extra CLI flags, initialize it, and return a `McpSession`
    /// over it. Used by A-013 to test `--read-only` posture in
    /// isolation without coupling to A-009's hand-rolled Session.
    pub async fn spawn_with_flags(ws: &Path, extra_args: &[&str]) -> Result<Self, String> {
        Self::spawn_raw(ws, extra_args, None).await
    }

    /// Spawn the server with an explicit **process** working directory,
    /// independent of the `--cwd` value passed to the child.
    ///
    /// Why this exists: `spawn` and `spawn_with_flags` both inherit the
    /// test process's CWD, which is the crate root — always inside the
    /// repository. A real MCP client (Claude Code, Cursor, an agent
    /// runtime) spawns the server with CWD set to *the user's project*,
    /// which is outside the repository. That difference is invisible to
    /// every other test in this suite and it hid a real regression: the
    /// tool catalogue is resolved by walking up from the **process** CWD,
    /// so a server launched from outside the repository resolved an empty
    /// catalogue and answered `tools/list` with `[]`.
    ///
    /// A test that cannot reproduce the launch context of its consumers
    /// is a test that cannot catch their failures. Pinning the launch
    /// context is what makes this suite honest about the deployment it
    /// actually has.
    pub async fn spawn_from(ws: &Path, process_cwd: &Path) -> Result<Self, String> {
        Self::spawn_raw(ws, &[], Some(process_cwd)).await
    }

    /// [`Self::spawn_from`] with extra CLI flags, so a test can reproduce the
    /// launch context of a real client *and* the posture the client asked for.
    /// Without this, "spawned outside the repo in read-only mode" is a
    /// combination no test can express, which is precisely the combination
    /// that regressed to zero advertised tools.
    pub async fn spawn_from_with_flags(
        ws: &Path,
        process_cwd: &Path,
        extra_args: &[&str],
    ) -> Result<Self, String> {
        Self::spawn_raw(ws, extra_args, Some(process_cwd)).await
    }

    /// Shared spawn body for [`Self::spawn_with_flags`] and
    /// [`Self::spawn_from`]. `process_cwd` of `None` means "inherit ours",
    /// which is what the pre-existing harness did.
    async fn spawn_raw(
        ws: &Path,
        extra_args: &[&str],
        process_cwd: Option<&Path>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(ws);
        for a in extra_args {
            cmd.arg(a);
        }
        // The harness must not lend the child any build-time context it
        // would not have in production. `cargo` sets `CARGO_MANIFEST_DIR`
        // for the test process and every child inherits it, which is how a
        // suite can stay green while the deployed server — which never has
        // that variable — finds no catalogue at all. Removing it here makes
        // the child indistinguishable from a published binary.
        cmd.env_remove("CARGO_MANIFEST_DIR");
        if let Some(dir) = process_cwd {
            cmd.current_dir(dir);
        }
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
        s.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"a013","version":"0"}}})).await?;
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

    /// PID of the spawned child, or 0 if the OS has not assigned one
    /// yet (rare; tokio assigns a pid synchronously in `spawn`).
    pub fn pid(&self) -> u32 {
        self.child.id().unwrap_or(0)
    }

    /// Send a Unix signal to the child by name and wait for it to exit.
    ///
    /// Used by A-013 to assert the binario shuts down cleanly
    /// under SIGTERM (signal cancel) without paying for a separate
    /// `nix` dependency. We shell out to `/bin/kill` (POSIX) so the
    /// test runs on any Linux/macOS with no extra crate.
    /// Unix-only.
    #[cfg(unix)]
    pub async fn signal_and_wait(
        &mut self,
        signal_name: &str,
    ) -> Result<std::process::ExitStatus, String> {
        use std::process::Command;
        let pid = self.child.id().ok_or("child sin pid")?;
        let status = Command::new("kill")
            .arg("-s")
            .arg(signal_name)
            .arg("--")
            .arg(format!("{}", pid))
            .status()
            .map_err(|e| format!("kill no se pudo ejecutar: {e}"))?;
        if !status.success() {
            return Err(format!("kill falló con {status}"));
        }
        self.child.wait().await.map_err(|e| e.to_string())
    }
}

/// Absolute path to the `release/` directory that contains the
/// release-profile binaries (cogh, cognicode, cognicode-mcp, ...).
///
/// Resolution order (first match wins):
///
/// 1. `COGNICODE_RELEASE_BIN_DIR` — absolute path to the release
///    directory. Asserted to be an existing directory and returned
///    verbatim.
/// 2. `COGNICODE_RELEASE_BIN` — absolute path to a release-profile
///    binary. Asserted to exist (and to be absolute, since its parent
///    directory is what gets returned), then the parent directory is the
///    release dir. Same knob pair as `cognicode-cli/tests/common`: one
///    variable names the artifact, the other names the directory.
/// 3. `<probe parent>/release/` — when the resolved binary happens to
///    live in the release profile (e.g.
///    `cargo test --release -p cognicode-mcp`).
/// 4. `<probe parent>/release/` — sibling directory (when
///    `cargo test -p cognicode-mcp` ran without `--release`, the resolved
///    debug binary points at `debug/cognicode-mcp`, but a sibling
///    `release/` dir is expected to exist if the user built with
///    `cargo build --release --bin cognicode-mcp`).
/// 5. The literal `<repo_root>/target/release` — workspace default.
///
/// Without an override the resolved path must end with a `release`
/// component or this function fails. Reason: Cargo exports
/// `CARGO_BIN_EXE_<name>` to integration tests and under a plain
/// `cargo test` that variable is the **debug** build, so a release-flow
/// suite could splice release names onto a debug directory and stay green
/// while reading artifacts that were never built for release. A suite
/// certifying the release candidate must test release-profile bytes.
///
/// Lesson 84 (M0.13): never hard-code `<repo_root>/target/release`
/// because it breaks under a global `~/.cargo/config.toml`
/// target-dir override. We honour the same location Cargo chose for
/// the binary resolved by `binary_path`, falling back to the
/// repo-root default only when that fails.
pub fn release_dir() -> PathBuf {
    if let Some(raw) = std::env::var_os("COGNICODE_RELEASE_BIN_DIR") {
        let dir = PathBuf::from(raw);
        assert!(
            dir.is_dir(),
            "COGNICODE_RELEASE_BIN_DIR={} is not an existing directory; point it \
             at the directory holding the release-profile binaries (cogh, \
             cognicode, cognicode-mcp), e.g. `$CARGO_TARGET_DIR/release`",
            dir.display()
        );
        return dir;
    }
    if let Some(raw) = std::env::var_os("COGNICODE_RELEASE_BIN") {
        let bin = PathBuf::from(raw);
        assert!(
            bin.exists(),
            "COGNICODE_RELEASE_BIN={} does not exist; point it at the absolute \
             path of a release-profile binary (built with `cargo build --release \
             --bin cognicode-mcp`) or unset it to use the probe-based resolution",
            bin.display()
        );
        assert!(
            bin.is_absolute(),
            "COGNICODE_RELEASE_BIN={} must be an absolute path: this override \
             contributes its parent directory as the release dir, and a relative \
             path would resolve against the test's working directory",
            bin.display()
        );
        return bin
            .parent()
            .map(PathBuf::from)
            .expect("COGNICODE_RELEASE_BIN has no parent directory");
    }
    let resolved = resolve_release_dir();
    assert!(
        resolved.ends_with("release"),
        "release_dir: resolved {} does not end with a `release` component; the \
         release-flow suites must read release-profile artifacts, not the debug \
         build Cargo produced for this test. Build with `cargo build --release` \
         or set COGNICODE_RELEASE_BIN_DIR=/abs/path/to/release and re-run.",
        resolved.display()
    );
    resolved
}

/// `release_dir` without the overrides, so the profile assertion above
/// covers every branch it can return (today they all end in `release` by
/// construction; the assertion is what keeps that true).
fn resolve_release_dir() -> PathBuf {
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
    /// The oracle is the newest source under the binary's own crate
    /// (`crates/<this-crate>/`), not the workspace as a whole. The
    /// workspace-wide scan was wrong: cargo's per-target incremental build
    /// watches the bin's own crate plus the fingerprints of crates it
    /// depends on, so source edits in *unrelated* crates are not a reason
    /// to rebuild this bin. Walking the whole tree made the guard a
    /// duplicate (and coarser) freshness check that fired on edits the
    /// build system would not, and made every test stage that ran in an
    /// isolated target dir panic when an unrelated file (e.g. a CHANGELOG
    /// edit or a sibling crate's test) was touched after the build.
    ///
    /// Cargo handles cross-crate staleness itself through fingerprint
    /// propagation, so this test only needs to mirror its per-crate view.
    /// Comparing against the test binary is also wrong: cargo links the
    /// two in the same build and their mtimes differ by however long the
    /// link step took, so a correct setup fails by milliseconds.
    #[test]
    fn the_binary_under_test_is_not_older_than_the_sources_it_was_built_from() {
        let path = binary_path();
        assert!(path.exists(), "resolved binary does not exist: {path:?}");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let binary_time =
            modified(&path).unwrap_or_else(|| panic!("cannot stat the resolved binary: {path:?}"));
        let Some((newest, newest_time)) = newest_source_under(&manifest_dir) else {
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

    /// Pin the scope of `newest_source_under` (and the caller
    /// `the_binary_under_test_is_not_older_than_the_sources_it_was_built_from`):
    /// the walk must NOT cross into sibling crates' `src/` trees. An edit
    /// to a file outside this crate is not a reason to rebuild this bin
    /// (cargo's per-crate incremental build agrees), and the guard
    /// pretending it is makes the merge-gate red for unrelated changes.
    ///
    /// The temporary file lives only for the duration of the test; it is
    /// placed inside a subdirectory of the current crate so the walk
    /// actually visits it. The other file is placed OUTSIDE the crate
    /// (sibling directory) and the walk must skip it.
    #[test]
    fn newest_source_under_does_not_see_sibling_crate_sources() {
        let tmp = std::env::temp_dir().join(format!("newest-source-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Fake crate layout: tmp/crate_a/src/ and tmp/crate_b/src/. The
        // walk is rooted at tmp/crate_a and must only see crate_a/src/.
        let in_scope = tmp.join("crate_a").join("src");
        let out_of_scope = tmp.join("crate_b").join("src");
        std::fs::create_dir_all(&in_scope).unwrap();
        std::fs::create_dir_all(&out_of_scope).unwrap();

        // Both files exist before we muck with mtimes. The in-scope
        // file is older than the out-of-scope one, so a buggy walk
        // would report the out-of-scope file as the "newest".
        let in_path = in_scope.join("main.rs");
        let out_path = out_of_scope.join("main.rs");
        std::fs::write(&in_path, b"// inside\n").unwrap();
        std::fs::write(&out_path, b"// outside, newer\n").unwrap();

        let t_in = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        let t_out = std::time::SystemTime::now();
        filetime_set(&in_path, t_in);
        filetime_set(&out_path, t_out);

        fn filetime_set(p: &Path, t: std::time::SystemTime) {
            let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
            f.set_modified(t).unwrap();
        }

        let root = tmp.join("crate_a");
        let (found_path, found_time) =
            newest_source_under(&root).expect("walk must find at least one source");

        assert!(
            found_path.starts_with(&in_scope),
            "walk crossed into a sibling crate: {found_path:?}"
        );
        assert_eq!(
            found_time, t_in,
            "the newest source under the crate must come from inside it"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

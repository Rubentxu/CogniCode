//! Harness compartido para UATs de persistencia (binario real).
//! Duplicación eliminada tras §70-§73: un único spawn + call_tool.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
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
        let (_id, resp) = self.request_with_id(method, params).await?;
        Ok(resp)
    }

    /// Send a JSON-RPC request and return its id together with the raw
    /// response.
    ///
    /// [`Self::request`] hides the id it used, and a client that wants to
    /// cancel a request has to name it: `notifications/cancelled` carries
    /// the id of the request being cancelled, so a test that cannot see
    /// the id cannot send a cancellation that refers to anything.
    pub async fn request_with_id(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(u64, Value), String> {
        let id = self.send_request(method, params).await?;
        let response = self.await_response(id).await?;
        Ok((id, response))
    }

    /// Send a JSON-RPC request without waiting for its answer, and return
    /// the id it went out with.
    ///
    /// A cancellation that names a request has to be sent while that
    /// request is still running, and that is only possible if the answer
    /// is not being waited for. The answer is collected afterwards with
    /// [`Self::await_response`].
    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }

    /// Wait for the answer to the request with the given id.
    ///
    /// Answers to other requests are read and discarded: the caller asked
    /// for one id and gets that one.
    pub async fn await_response(&mut self, id: u64) -> Result<Value, String> {
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

    /// Send `notifications/cancelled` for the given request id.
    ///
    /// The protocol allows this notification to arrive after the request
    /// has already finished, so a caller can legitimately cancel a request
    /// whose answer is already on its way.
    pub async fn notify_cancelled(&mut self, request_id: u64) -> Result<(), String> {
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {"requestId": request_id, "reason": "cancelled by the test"}
        }))
        .await
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

/// A response read off the wire.
pub struct HttpResponse {
    /// The status line, as the server wrote it.
    pub status_line: String,
    /// The headers, with their names in lowercase.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: String,
}

impl HttpResponse {
    /// The value of a header, looked up by its lowercase name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// One GET against a local server, with the credential it asks for.
///
/// The crate has no HTTP client in `[dev-dependencies]`, and a one-shot GET
/// against a local socket is not worth one. Reading the status line and the
/// headers off the socket is what lets a check assert what the server
/// actually said, instead of what the source says it says.
///
/// One attempt per connection on purpose: the probe for readiness is the
/// request itself. A connection opened to find out whether the server is
/// there and then dropped crosses with the real request, and the read of
/// the status line fails with a reset that has nothing to do with the
/// server.
pub fn http_get(
    address: std::net::SocketAddr,
    path: &str,
    token: Option<&str>,
) -> Result<HttpResponse, String> {
    let mut stream = std::net::TcpStream::connect(address)
        .map_err(|error| format!("connect to {path}: {error}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .map_err(|error| format!("read timeout: {error}"))?;
    let credential = match token {
        Some(token) => format!("Authorization: Bearer {token}\r\n"),
        None => String::new(),
    };
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{credential}Connection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("send GET: {error}"))?;

    let mut reader = std::io::BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|error| format!("read status line: {error}"))?;
    if status_line.trim().is_empty() {
        return Err(
            "the server accepted the connection and closed it without a status line".into(),
        );
    }

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }

    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .map_err(|error| format!("read body: {error}"))?;

    Ok(HttpResponse {
        status_line,
        headers,
        body,
    })
}

/// A port the operating system says is free.
///
/// Binding it and releasing it before the server starts is not atomic, but
/// the server is started right after and this is the only way to keep a
/// check parallel-safe without pinning a port a concurrent runner would
/// collide with.
///
/// The window between releasing it and the server binding it is NOT
/// theoretical. Measured, with 16 processes asking for ports at the same time:
///
///     puertos emitidos        : 6400
///     puertos distintos      : 5141
///     puertos emitidos >1 vez: 1074
///     emisiones repetidas     : 1259
///
/// Uno de cada cinco emisiones devuelve un puerto que otro proceso acaba de
/// soltar. Por eso el servidor se arranca con `start_server_until`, que
/// reintenta con otro puerto cuando el servidor no pudo abrir el suyo.
///
/// Notese que el servidor puede perder la carrera en dos sentidos: que otro
/// proceso tome el puerto, o que lo tome y su servidor siga vivo. En el
/// segundo caso el test que se scrapea no es el suyo. Eso no se puede
/// distinguir desde fuera —los dos sirven el mismo binario— y por eso la
/// garantia que se busca aqui es que cada test tenga un servidor propio, no
/// que el scrape sea el suyo.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Un servidor arrancado para un test, y todo lo que es suyo.
///
/// El `Child` se guarda aqui porque su `Drop` no mata: suelta el
/// handle y deja el proceso corriendo. Un test que entra en panico despues
/// de arrancar el servidor se llevaba por delante un proceso de la maquina,
/// con su puerto cogido, hasta que alguien se daba cuenta.
///
/// Measured, with the account closed: four orphaned MCP servers, and the age
/// of each one matches a concrete failed run.
/// Uno por cada ejecucion que fallo, ni una mas.
pub struct TestServer {
    /// The address the server was told to listen on.
    pub address: std::net::SocketAddr,
    child: std::process::Child,
}

impl TestServer {
    /// The process id of the server, so a test can check that it is gone.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Kill the server and reap it. Idempotent: `Drop` calls it too.
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// How many ports to try before giving up.
const PORT_ATTEMPTS: usize = 5;

/// Starts the server on a free port and waits until `probe` says it answered.
///
/// `spawn` receives the address to listen on and returns the child.
/// `probe` receives the same address and returns `Some` once the server has
/// answered as the check needs.
///
/// If the server dies before answering, another port is tried: the usual
/// cause is that the port was taken between choosing it and the server
/// binding it (see `free_port`). If the server stays alive but does not
/// answer, that is a real failure and it is reported at once, with the
/// server's stderr, rather than after five timeouts.
pub fn start_server_until<S, P>(mut spawn: S, mut probe: P) -> TestServer
where
    S: FnMut(std::net::SocketAddr) -> std::process::Child,
    P: FnMut(std::net::SocketAddr) -> Option<HttpResponse>,
{
    let mut last_error = String::new();
    for attempt in 1..=PORT_ATTEMPTS {
        let port = free_port();
        let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().expect("address");
        let mut child = spawn(address);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut answered = None;
        let mut died = false;
        while std::time::Instant::now() < deadline {
            if let Ok(Some(status)) = child.try_wait() {
                last_error = format!("the server ended with {status} before answering");
                died = true;
                break;
            }
            if let Some(response) = probe(address) {
                answered = Some(response);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if let Some(_response) = answered {
            return TestServer { address, child };
        }

        let mut stderr = String::new();
        if let Some(stream) = child.stderr.as_mut() {
            let _ = stream.read_to_string(&mut stderr);
        }
        let _ = child.kill();
        let _ = child.wait();

        if !died {
            // The server was alive and simply did not answer. That is not a
            // port problem, and retrying it five times would turn a clear
            // failure into a five-minute one.
            panic!("the server never answered on {address}:\n{stderr}");
        }
        if attempt < PORT_ATTEMPTS {
            eprintln!(
                "the server could not take port {port}; trying another one ({attempt}/{PORT_ATTEMPTS})\n{stderr}"
            );
        }
    }
    panic!("could not start the server on any of {PORT_ATTEMPTS} ports:\n{last_error}");
}

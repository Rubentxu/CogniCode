//! PRF-SEC-03 UAT: telemetry is opt-in.
//!
//! Real-binary: by default the server must NOT attempt any OTLP export
//! (no MeterProvider built, "telemetry disabled" notice on stderr).
//! With COGNICODE_TELEMETRY=1 the provider is built and the stderr
//! notice says so. No source code is ever transmitted (metrics only).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/release/cognicode-mcp")
}

fn fixture_ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

fn run_and_capture(env_optin: bool) -> String {
    use std::io::Write;
    let mut cmd = Command::new(binary_path());
    cmd.arg("--cwd").arg(fixture_ws());
    if env_optin {
        cmd.env("COGNICODE_TELEMETRY", "1");
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn");
    {
        let stdin = child.stdin.as_mut().unwrap();
        let _ = writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05","capabilities":{{}},"clientInfo":{{"name":"t","version":"0"}}}}}}"#
        );
        let _ = writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        );
    }
    // Give it a moment, then close stdin for clean shutdown.
    std::thread::sleep(Duration::from_millis(300));
    drop(child.stdin.take());
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("server did not exit after stdin EOF");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn telemetry_is_opt_in_by_default() {
    let stderr = run_and_capture(false);
    assert!(
        stderr.contains("telemetry disabled"),
        "default run must not enable telemetry, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("MeterProvider.Built"),
        "default run must not build an OTLP meter provider, stderr: {stderr}"
    );
}

#[test]
fn telemetry_can_be_enabled_explicitly() {
    let stderr = run_and_capture(true);
    assert!(
        stderr.contains("telemetry enabled"),
        "opt-in run must enable telemetry, stderr: {stderr}"
    );
}

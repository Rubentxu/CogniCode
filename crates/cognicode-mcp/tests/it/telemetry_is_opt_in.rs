//! The telemetry exporter is opt-in, and the opt-in has to be observable.
//!
//! `main` decides whether to build the OTLP exporter from the
//! `COGNICODE_TELEMETRY` environment variable. The mutation gate found
//! that the decision was not asserted anywhere: flipping the comparison
//! from `==` to `!=` turned telemetry ON for every server that did not
//! ask for it, and no test in the repository noticed.
//!
//! The decision is announced on stderr — "telemetry disabled" — and that
//! announcement is what this test reads. Nothing here talks to the
//! network: with the variable unset, the server never builds an
//! exporter, so there is nothing to connect to.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// The server binary under test.
fn server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp"))
}

/// A workspace the server can be pointed at.
fn fixture_ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Read the server's stderr until the telemetry announcement shows up.
///
/// The read happens on its own thread because `read` blocks: the deadline
/// has to be enforced by the caller, not by the reader. A test that waits
/// forever for a line that never arrives is a test that never finishes.
///
/// The stop condition is the announcement itself — "telemetry disabled" or
/// "telemetry enabled" — and not the bare word "telemetry". Stopping at the
/// word would return whatever happened to be on stderr at that moment, and
/// the assertion would then be testing the log's wording instead of the
/// server's decision.
fn read_until_telemetry_announcement(stderr: impl Read + Send + 'static) -> String {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = stderr;
        let mut all = String::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            all.push_str(&String::from_utf8_lossy(&buf[..n]));
            if all.contains("telemetry disabled") || all.contains("telemetry enabled") {
                break;
            }
        }
        let _ = tx.send(all);
    });
    rx.recv_timeout(Duration::from_secs(30)).unwrap_or_default()
}

#[test]
fn telemetry_is_off_when_the_environment_does_not_ask_for_it() {
    let mut child = Command::new(server_binary())
        .arg("--cwd")
        .arg(fixture_ws())
        .env_remove("COGNICODE_TELEMETRY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cognicode-mcp");

    let stderr = child.stderr.take().expect("stderr is piped");
    let announced = read_until_telemetry_announcement(stderr);
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        announced.contains("telemetry disabled"),
        "the server did not report telemetry as disabled, so the opt-in is \
         not what the code does. stderr:\n{announced}"
    );
}

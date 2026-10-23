//! The endpoint that announces the watcher names what the watcher watches.
//!
//! The endpoint is how someone asks the server what it notices. An answer
//! that names a different set of files than the server watches is worse
//! than no answer at all: it is an answer that gets believed.
//!
//! The check runs the real binary, over real HTTP, with the credential the
//! server demands, and reads the list off the wire. What the source says
//! the endpoint publishes is not what matters here; what the server says
//! over the socket is.

mod common;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cognicode_core::application::ingest::{noise_directories, observed_extensions};
use serde_json::Value;

use common::{free_port, http_get};

/// The credential the server is started with.
const TOKEN: &str = "a-credential-for-the-watch-endpoint";

/// How long the server is given to start answering.
const STARTUP_WINDOW: Duration = Duration::from_secs(60);

/// The server binary under test.
fn server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp-server"))
}

/// The workspace the server is pointed at.
fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Starts the server and waits until it answers on `/watch`.
fn start_server() -> (SocketAddr, Child) {
    let port = free_port();
    let address: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .expect("the address is valid");

    let mut child = Command::new(server_binary())
        .arg("--cwd")
        .arg(workspace())
        .arg("--listen")
        .arg(address.to_string())
        .env("COGNICODE_MCP_AUTH_TOKEN", TOKEN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");

    let deadline = Instant::now() + STARTUP_WINDOW;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            let _ = child.wait();
            let mut stderr = String::new();
            if let Some(stream) = child.stderr.as_mut() {
                use std::io::Read;
                let _ = stream.read_to_string(&mut stderr);
            }
            panic!("the server ended with {status} before answering:\n{stderr}");
        }
        if let Ok(response) = http_get(address, "/watch", Some(TOKEN))
            && response.status_line.contains("200")
        {
            return (address, child);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("the server never answered on `/watch`");
}

/// The announced body, read as JSON.
fn announced(address: SocketAddr) -> Value {
    let response = http_get(address, "/watch", Some(TOKEN)).expect("the endpoint answers");
    assert!(
        response.status_line.contains("200"),
        "the endpoint must answer 200, got `{}`",
        response.status_line.trim()
    );
    serde_json::from_str(&response.body).expect("the body is JSON")
}

/// The endpoint names the files the product observes.
#[test]
fn the_endpoint_names_the_files_the_product_observes() {
    let (address, mut child) = start_server();
    let body = announced(address);
    let _ = child.kill();
    let _ = child.wait();

    let announced: Vec<String> = body["watched_extensions"]
        .as_array()
        .expect("the endpoint publishes a list of extensions")
        .iter()
        .map(|extension| extension.as_str().unwrap_or_default().to_string())
        .collect();
    let observed: Vec<String> = observed_extensions()
        .iter()
        .map(|extension| extension.to_string())
        .collect();

    assert_eq!(
        announced, observed,
        "the endpoint names a different set of extensions than the product observes"
    );
}

/// The endpoint names the extensions of the languages the registry reads.
#[test]
fn the_endpoint_names_every_language_the_registry_reads() {
    let (address, mut child) = start_server();
    let body = announced(address);
    let _ = child.kill();
    let _ = child.wait();

    let announced: Vec<String> = body["watched_extensions"]
        .as_array()
        .expect("the endpoint publishes a list of extensions")
        .iter()
        .map(|extension| extension.as_str().unwrap_or_default().to_string())
        .collect();

    for extension in [
        "rs", "py", "ts", "tsx", "js", "go", "java", "c", "h", "cpp", "cs", "tf", "yml", "yaml",
        "rb", "php", "swift", "hs", "scala", "sh", "jl", "erl", "zig", "dart", "ex", "ps1", "md",
        "json", "toml",
    ] {
        assert!(
            announced.iter().any(|candidate| candidate == extension),
            "the product reads `{extension}` and the endpoint does not name it"
        );
    }

    assert!(
        !announced.iter().any(|candidate| candidate == "png"),
        "the product does not read images, and the endpoint names them"
    );
}

/// The endpoint names the places the watcher does not look.
#[test]
fn the_endpoint_names_the_places_the_watcher_does_not_look() {
    let (address, mut child) = start_server();
    let body = announced(address);
    let _ = child.kill();
    let _ = child.wait();

    let announced: Vec<String> = body["ignored_directories"]
        .as_array()
        .expect("the endpoint publishes a list of directories")
        .iter()
        .map(|directory| directory.as_str().unwrap_or_default().to_string())
        .collect();
    let ignored: Vec<String> = noise_directories()
        .iter()
        .map(|directory| directory.to_string())
        .collect();

    assert_eq!(
        announced, ignored,
        "the endpoint names a different set of directories than the ones the watcher skips"
    );
}

//! Que arrancar un servidor para un test sobreviva a un puerto ocupado, y que
//! no deje el servidor vivo cuando el test se acaba.
//!
//! Measured. El servidor se arranca en un puerto que el sistema
//! operativo dice libre, pero el sistema lo suelta antes de que el servidor
//! lo abra. En esa ventana otro proceso puede tomarlo. La frecuencia no es
//! teoria:
//!
//!     puertos emitidos        : 6400
//!     puertos distintos      : 5141
//!     puertos emitidos >1 vez: 1074
//!     emisiones repetidas     : 1259
//!
//! Uno de cada cinco emisiones devuelve un puerto que otro proceso acaba de
//! soltar. Y con dos binarios de test corriendo en paralelo —el de `/metrics`
//! y el de `/watch`— laINES probable que uno pille el puerto del otro.
//!
//! Measured against the code that is in `main` today, with the two binaries
//! corriendo a la vez y seis instancias de cada uno:
//!
//!     antes : 1 fallo de 72
//!         thread 'the_endpoint_names_the_files_the_product_observes' panicked at
//!         the_watch_endpoint_names_what_is_watched.rs:68:13:
//!         the server ended with exit status: 1 before answering:
//!         Error: Address already in use (os error 98)
//!
//! Ese "Address already in use" es del servidor de verdad, no una simulacion:
//! el puerto se lo quedo el otro binario de test.
//!
//! Y the second half of the same defect: `Child` does not kill on drop. A
//! test that panicked after starting the server took a process of the machine
//! with it, its port still taken. Measured, with the account closed: four
//! orphaned MCP servers, and the age of each one matches a concrete failed
//! run. One per failed run, not one more.

#![cfg(unix)]

mod common;

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use common::{http_get, start_server_until};

/// How many ports the helper has tried.
static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

/// The ports the helper has tried, in order.
static ATTEMPTED: Mutex<Vec<u16>> = Mutex::new(Vec::new());

/// The listeners that keep the refused ports taken.
static HELD: Mutex<Vec<TcpListener>> = Mutex::new(Vec::new());

/// The server binary under test.
fn server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp-server"))
}

/// The workspace the server is pointed at.
fn fixture_ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Whether a process is still alive.
fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn a_port_that_cannot_be_bound_does_not_fail_the_test() {
    ATTEMPTS.store(0, Ordering::SeqCst);
    ATTEMPTED.lock().expect("lock").clear();

    let server = start_server_until(
        |address| {
            ATTEMPTED.lock().expect("lock").push(address.port());
            if ATTEMPTS.fetch_add(1, Ordering::SeqCst) == 0 {
                // The first port is taken by this test, so the server cannot
                // have it. The listener stays bound: dropping it here would
                // free the port before the server ever tried.
                let listener = TcpListener::bind(address).expect("occupy the port");
                HELD.lock().expect("lock").push(listener);
            }
            Command::new(server_binary())
                .arg("--cwd")
                .arg(fixture_ws())
                .arg("--listen")
                .arg(address.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn cognicode-mcp-server")
        },
        |address| http_get(address, "/metrics", None).ok(),
    );

    let attempted = ATTEMPTED.lock().expect("lock").clone();
    assert!(
        attempted.len() >= 2,
        "the helper had to try more than one port; it tried {attempted:?}"
    );
    assert_ne!(
        server.address.port(),
        attempted[0],
        "the server has to end up on a port it could actually bind"
    );
    let response = http_get(server.address, "/metrics", None).expect("/metrics answers");
    assert!(
        response.status_line.contains("200"),
        "/metrics must answer 200 on the port that was free, got `{}`",
        response.status_line.trim()
    );
}

#[test]
fn the_server_is_gone_when_the_guard_is_dropped() {
    let server = start_server_until(
        |address| {
            Command::new(server_binary())
                .arg("--cwd")
                .arg(fixture_ws())
                .arg("--listen")
                .arg(address.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn cognicode-mcp-server")
        },
        |address| http_get(address, "/metrics", None).ok(),
    );

    let pid = server.pid();
    assert!(
        process_alive(pid),
        "the server has to be running while the guard is alive"
    );

    drop(server);

    // The guard kills and reaps. Give the signal a moment to land.
    let mut alive = true;
    for _ in 0..50 {
        if !process_alive(pid) {
            alive = false;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !alive,
        "the server survived the guard: pid {pid} is still running with its port taken"
    );
}

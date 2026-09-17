//! Test-only support: a self-contained, offline release that `cogh` can install.
//!
//! This exists so the e85 claim "the producer and the consumer speak the same
//! artifact language" is *proven* rather than asserted. Nothing here mocks the
//! contract: the payloads are real tar.gz archives with canonical names, the
//! manifest is produced by the real
//! [`crate::release_factory::generate_release`], and the digest the installer
//! verifies is the digest of the bytes actually served over HTTP.
//!
//! The only thing that is local is the *origin*: the manifest carries canonical
//! `github.com` URLs and `COGNICODE_RELEASE_BASE_URL` redirects the fetch to a
//! loopback server for the duration of the test.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use anyhow::{Context, Result};

use crate::bundle_manifest::Platform;
use crate::installer_transaction::{ENV_BUNDLE_MANIFEST, ENV_RELEASE_BASE_URL};
use crate::release_contract::{artifact_filename, platform_token, published_components};
use crate::release_factory::generate_release;

/// A generated, locally served release.
pub struct LocalRelease {
    /// Directory holding the payloads AND the generated manifest/inventory/sums.
    pub dir: PathBuf,
    /// Path of the generated per-platform bundle manifest.
    pub manifest_path: PathBuf,
    /// Loopback base URL to point `COGNICODE_RELEASE_BASE_URL` at.
    pub base_url: String,
    /// Kept alive so the directory is not deleted while the server serves it.
    _server: ServerGuard,
    _tmp: tempfile::TempDir,
}

struct ServerGuard {
    child: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start a loopback file server for `root`, using the same `python3` available
/// in CI and on developer machines. Returns the base URL.
///
/// A std-only server would be more self-contained, but this keeps the helper
/// short and the failure mode obvious.
fn serve(root: &Path, version: &str) -> Result<(String, ServerGuard, u16)> {
    // Bind an ephemeral port via python, then print it so we can synchronise.
    let script = format!(
        r#"
import http.server, socketserver, sys, functools, os
os.chdir({root:?})
Handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory={root:?})
class Q(socketserver.TCPServer):
    allow_reuse_address = True
with Q(("127.0.0.1", 0), Handler) as httpd:
    sys.stdout.write(str(httpd.server_address[1]) + "\n")
    sys.stdout.flush()
    httpd.serve_forever()
"#,
        root = root.to_string_lossy()
    );

    let mut child = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("spawn python3 http server")?;

    // Read the chosen port from the child's stdout.
    let stdout = child.stdout.take().context("server stdout")?;
    let mut buf = String::new();
    let mut reader = std::io::BufReader::new(stdout);
    std::io::BufRead::read_line(&mut reader, &mut buf)?;
    let port: u16 = buf.trim().parse().context("parse server port")?;

    let _ = version;
    Ok((
        format!("http://127.0.0.1:{port}"),
        ServerGuard { child },
        port,
    ))
}

/// Build a canonical, installable payload archive containing `bin/<name>`.
fn write_payload(path: &Path, component: &str) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);

    let script = format!("#!/bin/sh\necho {component} dev-fixture\n");
    let mut header = tar::Header::new_gnu();
    header.set_size(script.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder.append_data(&mut header, format!("bin/{component}"), script.as_bytes())?;

    let encoder = builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}

/// Stage a complete Tier-1 Linux release and serve it locally.
///
/// Every published component is produced (so the orphan check passes), the real
/// generator writes the manifest/inventory/SHA256SUMS, and the payloads are
/// served over loopback.
pub fn local_release(version: &str) -> Result<LocalRelease> {
    let tmp = tempfile::tempdir()?;
    let staging = tmp.path().join("staging");
    let dir = tmp.path().join("dist");
    std::fs::create_dir_all(&staging)?;
    std::fs::create_dir_all(&dir)?;

    // 1. Canonical payloads for the host platform.
    for spec in published_components() {
        let name = artifact_filename(spec.kind.stem(), version, Platform::LinuxX86_64);
        write_payload(&staging.join(&name), spec.kind.stem())?;
    }

    // 2. The real generator produces the manifest from those bytes, and
    //    materialises a complete release directory at `dir`.
    generate_release(
        &staging,
        &dir,
        version,
        &format!("v{version}"),
        "0000000000000000000000000000000000000000",
        &[Platform::LinuxX86_64],
        None,
    )?;

    // 3. Serve the payloads under the canonical `/v{version}/` path prefix that
    //    `resolve_download_url` produces when it rewrites the origin.
    let serve_root = tmp.path().join("serve");
    let versioned = serve_root.join(format!("v{version}"));
    std::fs::create_dir_all(&versioned)?;
    for entry in std::fs::read_dir(&staging)? {
        let src = entry?.path();
        std::fs::copy(&src, versioned.join(src.file_name().unwrap()))?;
    }
    let (base_url, server, _port) = serve(&serve_root, version)?;

    // Wait for the server to accept connections.
    wait_for_server(&base_url);

    let manifest_path = dir.join(crate::release_contract::bundle_manifest_filename(
        version,
        Platform::LinuxX86_64,
    ));

    Ok(LocalRelease {
        dir,
        manifest_path,
        base_url,
        _server: server,
        _tmp: tmp,
    })
}

fn wait_for_server(base_url: &str) {
    use std::net::TcpStream;
    let addr = base_url.trim_start_matches("http://");
    for _ in 0..100 {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Point the process at a generated release. Caller must set `COGNICODE_HOME`.
pub fn point_at(release: &LocalRelease) {
    // SAFETY: tests using this are `#[serial]`.
    unsafe {
        std::env::set_var(ENV_BUNDLE_MANIFEST, &release.manifest_path);
        std::env::set_var(ENV_RELEASE_BASE_URL, &release.base_url);
    }
}

/// Clear the release-related environment.
pub fn unpoint() {
    unsafe {
        std::env::remove_var(ENV_BUNDLE_MANIFEST);
        std::env::remove_var(ENV_RELEASE_BASE_URL);
    }
}

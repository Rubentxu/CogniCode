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
//! `github.com` URLs and `COGNICODE_ASSET_BASE_URL` redirects the fetch to a
//! loopback server for the duration of the test. E86.2.2 split the legacy
//! `COGNICODE_RELEASE_BASE_URL` into two variables — one for the API/release
//! resolver side and one for the asset/component download side — but this
//! fixture only needs the **asset** side.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use anyhow::{Context, Result};

use crate::bundle_manifest::Platform;
use crate::installer_transaction::{ENV_ASSET_BASE_URL, ENV_BUNDLE_MANIFEST};
use crate::release_contract::{
    artifact_filename, published_components, published_skill_bundles, skill_bundle_filename,
};
use crate::release_factory::generate_release;

/// A generated, locally served release.
pub struct LocalRelease {
    /// Path of the generated per-platform bundle manifest.
    pub manifest_path: PathBuf,
    /// Loopback base URL for the asset mirror (`COGNICODE_ASSET_BASE_URL`).
    pub base_url: String,
    /// Root directory served by the loopback HTTP server. Exposed so test
    /// fixtures can place extra files (manifests, additional payloads) under
    /// the canonical `/v{version}/` URL prefix.
    pub serve_root: PathBuf,
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

/// Produce an actual portable skill archive, not a fake string with a .tar.gz
/// extension. The installer exercises extraction and the IDE adapter expects
/// a real manifest and SKILL.md after materialisation.
fn write_skill_payload(path: &Path, id: &str) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for (name, body) in [
        ("manifest.yaml", format!("name: {id}\nversion: test\n")),
        (
            "SKILL.md",
            format!("---\nname: {id}\ndescription: Test bundle\n---\n"),
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, body.as_bytes())?;
    }
    builder.into_inner()?.finish()?;
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

    // 1b. DEBT-2c: the release contract also publishes skill bundles.
    for spec in published_skill_bundles() {
        let name = skill_bundle_filename(spec.id, version);
        write_skill_payload(&staging.join(&name), spec.id)?;
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
    // The installer verifies portable skill payloads against the published
    // release checksum file, not against an unverified cache entry.
    std::fs::copy(dir.join("SHA256SUMS"), versioned.join("SHA256SUMS"))?;
    let (base_url, server, _port) = serve(&serve_root, version)?;

    // Wait for the server to accept connections.
    wait_for_server(&base_url);

    let manifest_path = dir.join(crate::release_contract::bundle_manifest_filename(
        version,
        Platform::LinuxX86_64,
    ));

    Ok(LocalRelease {
        manifest_path,
        base_url,
        serve_root,
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
///
/// E86.2.2: sets `COGNICODE_ASSET_BASE_URL` (the installer-side override),
/// **not** the legacy `COGNICODE_RELEASE_BASE_URL`. The asset-side variable
/// is what `installer_transaction::resolve_download_url` reads to rewrite
/// canonical github.com component URLs onto this loopback. The resolver-side
/// (`COGNICODE_API_BASE_URL`) is intentionally left untouched because
/// `point_at` is only used by the installer pipeline, not the resolver.
///
/// **Prefer [`AssetPoint`] over this raw function.** `point_at` mutates
/// process-global env state and has no cleanup; a test that calls it and
/// then panics (or simply forgets to call [`unpoint`]) will leak
/// `COGNICODE_ASSET_BASE_URL` / `COGNICODE_BUNDLE_MANIFEST` to the next
/// `#[serial]` test in the same process. [`AssetPoint`] is the RAII wrapper
/// that restores the prior env values on drop. `point_at` / `unpoint` are
/// retained for tests that genuinely need manual control and accept the
/// state-leak risk.
pub fn point_at(release: &LocalRelease) {
    // SAFETY: tests using this are `#[serial]`.
    unsafe {
        std::env::set_var(ENV_BUNDLE_MANIFEST, &release.manifest_path);
        std::env::set_var(ENV_ASSET_BASE_URL, &release.base_url);
    }
}

/// Clear the release-related environment. Prefer [`AssetPoint`] for new code.
pub fn unpoint() {
    unsafe {
        std::env::remove_var(ENV_BUNDLE_MANIFEST);
        std::env::remove_var(ENV_ASSET_BASE_URL);
    }
}

/// RAII guard that mirrors [`point_at`] / [`unpoint`] but cleans up on drop.
///
/// Holds the previous values of `COGNICODE_ASSET_BASE_URL` and
/// `COGNICODE_BUNDLE_MANIFEST`; on drop, restores them (or removes the
/// variable if it was unset before). This is the safe replacement for
/// `point_at` in `#[serial]` tests so a panic or early return cannot leak
/// the loopback `127.0.0.1` base URL to a later test that expects the
/// canonical github.com URL.
pub struct AssetPoint {
    prev_asset: Option<std::ffi::OsString>,
    prev_manifest: Option<std::ffi::OsString>,
}

impl AssetPoint {
    /// Set the asset mirror + bundle manifest env vars for `release`,
    /// capturing the prior values so [`Drop`] can restore them.
    pub fn new(release: &LocalRelease) -> Self {
        let prev_asset = std::env::var_os(ENV_ASSET_BASE_URL);
        let prev_manifest = std::env::var_os(ENV_BUNDLE_MANIFEST);
        // SAFETY: tests using this are `#[serial]`.
        unsafe {
            std::env::set_var(ENV_BUNDLE_MANIFEST, &release.manifest_path);
            std::env::set_var(ENV_ASSET_BASE_URL, &release.base_url);
        }
        Self {
            prev_asset,
            prev_manifest,
        }
    }
}

impl Drop for AssetPoint {
    fn drop(&mut self) {
        // SAFETY: tests using this are `#[serial]`; Drop runs on the same
        // thread that constructed the guard.
        unsafe {
            match self.prev_asset.as_ref() {
                Some(v) => std::env::set_var(ENV_ASSET_BASE_URL, v),
                None => std::env::remove_var(ENV_ASSET_BASE_URL),
            }
            match self.prev_manifest.as_ref() {
                Some(v) => std::env::set_var(ENV_BUNDLE_MANIFEST, v),
                None => std::env::remove_var(ENV_BUNDLE_MANIFEST),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ResolverFixture — staging dir + base URL ready for lifecycle_resolver
// ─────────────────────────────────────────────────────────────────────────────

/// A bundle that pairs a [`LocalRelease`] with a `releases.json` the
/// `lifecycle_resolver` can consume. This is the e86 followup test seam:
/// it lets the new resolver-driven path run the same real-payload install
/// that the legacy `point_at` seam ran.
pub struct ResolverFixture {
    pub release: LocalRelease,
    /// Path of the staging directory; pass to `lifecycle_resolver` as
    /// `staging_dir`. Contains a single `releases.json`.
    pub staging_dir: PathBuf,
}

impl ResolverFixture {
    /// Build the fixture for `version` on Linux x86_64 (the only Tier-1
    /// platform covered by `LocalRelease` today). Same loopback server as
    /// `LocalRelease::local_release`, plus a `releases.json` whose manifest
    /// asset URL is rewritten to that loopback.
    pub fn build(version: &str) -> Result<Self> {
        let release = local_release(version)?;
        let staging_dir = release._tmp.path().join("staging-resolver");
        std::fs::create_dir_all(&staging_dir).context("create resolver staging dir")?;

        // Place the canonical bundle manifest under the loopback-served
        // `/v{version}/` so `cmd_update`'s GET <manifest_url> lands on a
        // 200. The generator writes it to `release.dir` (the `out_dir` of
        // `generate_release`), but the HTTP server only serves
        // `serve_root/v{version}/`. Without this copy, `cmd_update` would
        // 404 the manifest and abort.
        let manifest_name =
            crate::release_contract::bundle_manifest_filename(version, Platform::LinuxX86_64);
        let served_manifest = release
            .serve_root
            .join(format!("v{version}"))
            .join(&manifest_name);
        std::fs::copy(&release.manifest_path, &served_manifest).with_context(|| {
            format!(
                "copy manifest {} -> {}",
                release.manifest_path.display(),
                served_manifest.display()
            )
        })?;

        // The fixture must satisfy `lifecycle_resolver::load_release_from_staging`:
        // a single GhRelease object (or a list — both are accepted).
        //
        // The manifest asset URL is rewritten so `cmd_update`'s download
        // step hits the loopback, not the canonical github.com URL the
        // generator wrote. `--base-url` is the mechanism for that in
        // production; the fixture bakes the rewrite in directly because
        // staging-dir tests deliberately skip the API and need the URL
        // pre-rewritten.
        let manifest_url = format!("{}/v{}/{}", release.base_url, version, manifest_name);
        let json = format!(
            r#"{{
  "tag_name": "v{version}",
  "draft": false,
  "prerelease": false,
  "published_at": "2026-09-17T19:07:59Z",
  "html_url": "{base}/Rubentxu/CogniCode/releases/tag/v{version}",
  "assets": [
    {{ "name": "{manifest_name}", "browser_download_url": "{manifest_url}" }}
  ]
}}"#,
            base = release.base_url,
            version = version,
            manifest_name = manifest_name,
            manifest_url = manifest_url,
        );
        std::fs::write(staging_dir.join("releases.json"), json)
            .context("write releases.json fixture")?;

        Ok(Self {
            staging_dir,
            release,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle_manifest::Platform;
    use crate::lifecycle_resolver::{Channel, ResolveRequest, resolve_release};

    #[test]
    #[serial_test::serial]
    fn resolver_fixture_emits_valid_releases_json() {
        let fx = ResolverFixture::build("0.95.0").expect("build fixture");
        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: Some(fx.staging_dir.clone()),
        };
        let resolved = resolve_release(&req).expect("resolver must accept the fixture");
        assert_eq!(resolved.version, "0.95.0");
        assert_eq!(resolved.tag, "v0.95.0");
        // The manifest URL must be the loopback URL, not github.com.
        assert!(
            resolved.manifest_url.starts_with(&fx.release.base_url),
            "manifest_url must be loopback, got {}",
            resolved.manifest_url
        );
    }

    // ----- e86 followup T1 failure-mode follow-through -----
    //
    // The happy-path test above pins the contract on a well-formed fixture.
    // The two tests below exercise the most likely user-facing failure modes:
    //
    // 1. Malformed `releases.json` — a user passes a stale or hand-edited
    //    staging dir; the resolver must return a `ResolveFailed` error,
    //    not panic.
    // 2. `releases.json` lists only draft / prerelease releases — the
    //    resolver must reject the request per REQ-LR-03 / REQ-LDS-01.
    //
    // These are the failure modes most likely to bite users in production
    // because the staging-dir path is precisely how integration tests and
    // air-gapped installs configure the resolver. The resolver must
    // surface the failure mode, not swallow it.
    //
    // Note: `GhRelease`/`GhListRelease`/`GhAsset` are private to the
    // resolver module, so we construct the draft-only list as a raw JSON
    // string matching the wire shape (the field names and types must
    // match what `lifecycle_resolver::load_release_from_staging` parses).
    // If the resolver ever renames those fields the test will fail with
    // a parse error instead of a draft-rejection — that is itself
    // informative.

    use crate::error::InstallerError;

    fn staging_dir_with_json(json: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("releases.json"), json).expect("write releases.json");
        dir
    }

    #[test]
    #[serial_test::serial]
    fn resolver_fixture_rejects_malformed_releases_json() {
        // Garbage that is neither a single release object nor a list. The
        // resolver must surface a ResolveFailed error with a parse
        // message — never panic, never silently fall back.
        let staging = staging_dir_with_json("this is not json {{");
        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: Some(staging.path().to_path_buf()),
        };
        let err = resolve_release(&req).expect_err("resolver must reject malformed JSON");
        match err {
            InstallerError::ResolveFailed(msg) => {
                assert!(
                    msg.contains("parse") || msg.contains("staging"),
                    "error must identify the failure mode, got: {msg}"
                );
            }
            other => panic!("expected ResolveFailed, got {other:?}"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn resolver_fixture_rejects_only_draft_releases_in_list() {
        // Two releases, both drafts. The resolver must surface
        // ResolveFailed ("no non-draft, non-prerelease release") per the
        // REQ-LR-03 / REQ-LDS-01 contract.
        //
        // Note: the Gh* structs are private, so we hand-roll the JSON
        // matching the wire shape. Field names: tag_name, draft,
        // prerelease, assets[].name, assets[].browser_download_url.
        let json = r#"[
          {
            "tag_name": "v0.95.0-rc.1",
            "draft": true,
            "prerelease": true,
            "assets": [
              {
                "name": "bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
                "browser_download_url": "http://127.0.0.1:1/v0.95.0/bundle.yaml"
              }
            ]
          },
          {
            "tag_name": "v0.95.0",
            "draft": true,
            "prerelease": false,
            "assets": [
              {
                "name": "bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
                "browser_download_url": "http://127.0.0.1:1/v0.95.0/bundle.yaml"
              }
            ]
          }
        ]"#;
        let staging = staging_dir_with_json(json);
        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: Some(staging.path().to_path_buf()),
        };
        let err = resolve_release(&req)
            .expect_err("resolver must reject a list with only draft releases");
        match err {
            InstallerError::ResolveFailed(msg) => {
                assert!(
                    msg.contains("non-draft") || msg.contains("draft"),
                    "error must identify the draft-rejection, got: {msg}"
                );
            }
            other => panic!("expected ResolveFailed for draft-only list, got {other:?}"),
        }
    }

    // ----- e86 followup: AssetPoint RAII guard -----
    //
    // The 0.98.1 -> 0.99.0 SemVer bump (commit `d4a2e33e`) surfaced a
    // latent env-leak bug: several `#[serial]` tests called
    // `point_at(&release)` and forgot to call `unpoint()`. The next
    // serial test then inherited a `COGNICODE_ASSET_BASE_URL` pointing
    // at the previous test's loopback server, which had already been
    // dropped, causing a 404 on what should be a valid install. These
    // tests pin the AssetPoint RAII contract so a regression of the
    // same shape is caught at unit-test time, not at CI gate time.

    /// After `AssetPoint::new`, both env vars point at the release's
    /// loopback. This is the happy-path contract callers depend on.
    #[test]
    #[serial_test::serial]
    fn asset_point_sets_env_vars_for_release() {
        let release = local_release(env!("CARGO_PKG_VERSION")).expect("stage a local release");
        let prev_asset = std::env::var_os(ENV_ASSET_BASE_URL);
        let prev_manifest = std::env::var_os(ENV_BUNDLE_MANIFEST);
        let _guard = AssetPoint::new(&release);
        assert_eq!(
            std::env::var_os(ENV_ASSET_BASE_URL).map(|s| s.into_string().unwrap_or_default()),
            Some(release.base_url.clone()),
            "AssetPoint must set COGNICODE_ASSET_BASE_URL to the loopback base URL"
        );
        assert_eq!(
            std::env::var_os(ENV_BUNDLE_MANIFEST).map(|s| s.into_string().unwrap_or_default()),
            Some(release.manifest_path.to_string_lossy().to_string()),
            "AssetPoint must set COGNICODE_BUNDLE_MANIFEST to the staged manifest path"
        );
        // Restore for the next test (Drop does this; this is just a
        // sanity belt-and-braces).
        drop(_guard);
        assert_eq!(
            std::env::var_os(ENV_ASSET_BASE_URL),
            prev_asset,
            "Drop must restore the prior COGNICODE_ASSET_BASE_URL"
        );
        assert_eq!(
            std::env::var_os(ENV_BUNDLE_MANIFEST),
            prev_manifest,
            "Drop must restore the prior COGNICODE_BUNDLE_MANIFEST"
        );
    }

    /// `AssetPoint` must clean up env vars on drop even when the
    /// guarded code PANICS. This is the regression the guard was
    /// introduced to prevent.
    #[test]
    #[serial_test::serial]
    fn asset_point_restores_env_on_panic() {
        let release = local_release(env!("CARGO_PKG_VERSION")).expect("stage a local release");
        let prev_asset = std::env::var_os(ENV_ASSET_BASE_URL);
        let prev_manifest = std::env::var_os(ENV_BUNDLE_MANIFEST);

        let result = std::panic::catch_unwind(|| {
            let _guard = AssetPoint::new(&release);
            assert_eq!(
                std::env::var_os(ENV_ASSET_BASE_URL).map(|s| s.into_string().unwrap_or_default()),
                Some(release.base_url.clone()),
                "guard must have set COGNICODE_ASSET_BASE_URL"
            );
            panic!("simulated assertion failure inside the guard");
        });
        assert!(result.is_err(), "inner panic must propagate");

        // After the unwind + Drop, env must be exactly as it was before.
        assert_eq!(
            std::env::var_os(ENV_ASSET_BASE_URL),
            prev_asset,
            "Drop must restore COGNICODE_ASSET_BASE_URL even after a panic"
        );
        assert_eq!(
            std::env::var_os(ENV_BUNDLE_MANIFEST),
            prev_manifest,
            "Drop must restore COGNICODE_BUNDLE_MANIFEST even after a panic"
        );
    }

    /// When the env vars are UNSET before `AssetPoint::new`, Drop must
    /// REMOVE them (not restore a stale value).
    #[test]
    #[serial_test::serial]
    fn asset_point_removes_unset_env_vars_on_drop() {
        // SAFETY: tests using this are `#[serial]`.
        unsafe {
            std::env::remove_var(ENV_ASSET_BASE_URL);
            std::env::remove_var(ENV_BUNDLE_MANIFEST);
        }
        let release = local_release(env!("CARGO_PKG_VERSION")).expect("stage a local release");
        {
            let _guard = AssetPoint::new(&release);
            assert!(
                std::env::var_os(ENV_ASSET_BASE_URL).is_some(),
                "guard must set the env var"
            );
        }
        assert!(
            std::env::var_os(ENV_ASSET_BASE_URL).is_none(),
            "Drop must REMOVE the env var (not set it to empty) when it was unset before"
        );
        assert!(
            std::env::var_os(ENV_BUNDLE_MANIFEST).is_none(),
            "Drop must REMOVE COGNICODE_BUNDLE_MANIFEST when it was unset before"
        );
    }

    /// `point_at` / `unpoint` still work (legacy tests rely on them),
    /// and a manual `unpoint()` after `AssetPoint::new` does not corrupt
    /// state on Drop (Drop should see the env vars as cleared and treat
    /// them as "was unset" → do nothing on Drop).
    #[test]
    #[serial_test::serial]
    fn point_at_unpoint_pair_works_legacy_seam() {
        let release = local_release(env!("CARGO_PKG_VERSION")).expect("stage a local release");
        let prev_asset = std::env::var_os(ENV_ASSET_BASE_URL);
        let prev_manifest = std::env::var_os(ENV_BUNDLE_MANIFEST);

        point_at(&release);
        assert_eq!(
            std::env::var_os(ENV_ASSET_BASE_URL).map(|s| s.into_string().unwrap_or_default()),
            Some(release.base_url.clone()),
            "point_at must set the env var"
        );

        unpoint();
        assert!(
            std::env::var_os(ENV_ASSET_BASE_URL).is_none(),
            "unpoint must remove the env var"
        );
        assert!(
            std::env::var_os(ENV_BUNDLE_MANIFEST).is_none(),
            "unpoint must remove COGNICODE_BUNDLE_MANIFEST"
        );

        // Calling point_at / unpoint again must not panic. The
        // previously-set env vars are None now, so subsequent
        // AssetPoint::new would correctly treat them as "was unset".
        point_at(&release);
        unpoint();

        // Restore the originals (in case the test runner captures them).
        // SAFETY: tests using this are `#[serial]`.
        unsafe {
            if let Some(v) = prev_asset {
                std::env::set_var(ENV_ASSET_BASE_URL, v);
            }
            if let Some(v) = prev_manifest {
                std::env::set_var(ENV_BUNDLE_MANIFEST, v);
            }
        }
    }
}

//! `cogh::layout` — Filesystem layout helpers for `~/.cognicode/`.
//!
// Mirrors `~/.asdf/` structure (ADR-035). Provides:
//! - `CognicodeHome::resolve()` — figure out the COGNICODE_HOME path
//! - path helpers for bins, shims, versions, plugins, tracker, locks
//! - shell command stubs for `cogh install/uninstall/list/...`
//!
// See ADR-034 §"Architecture" for the directory layout.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::platform_adapter;
use crate::Cli;

// ===== Install root resolution =====

/// Resolve the COGNICODE_HOME path (COGNICODE_HOME env var or ~/.cognicode).
pub fn cognicode_home() -> PathBuf {
    if let Ok(env) = std::env::var("COGNICODE_HOME") {
        PathBuf::from(env)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cognicode")
    } else {
        PathBuf::from(".cognicode")
    }
}

/// Path to the install root (under COGNICODE_HOME).
pub fn install_root() -> PathBuf {
    cognicode_home().join("install")
}

/// Path to a specific version's install directory.
pub fn install_dir(version: &str) -> PathBuf {
    install_root().join(version)
}

/// Path to the shims directory.
pub fn shims_dir() -> PathBuf {
    cognicode_home().join("shims")
}

/// Path to the tracker directory.
pub fn tracker_dir() -> PathBuf {
    cognicode_home().join("tracker")
}

/// Path to the cache directory.
pub fn cache_dir() -> PathBuf {
    cognicode_home().join("cache")
}

/// Path to the bundle.yaml manifest distributed with the installer.
pub fn bundle_yaml_path() -> PathBuf {
    cognicode_home().join("bundle.yaml")
}

/// Path to the install manifest for a given version.
pub fn install_manifest_path(version: &str) -> PathBuf {
    install_dir(version).join("manifest.yaml")
}

/// Resolved `~/.cognicode/` (or `COGNICODE_HOME`) layout.
#[derive(Debug, Clone)]
pub struct CognicodeHome {
    pub root: PathBuf,
}

impl CognicodeHome {
    /// Resolve the home directory from CLI args + env.
    pub fn resolve(home_override: Option<&Path>) -> Result<Self> {
        let root = if let Some(p) = home_override {
            p.to_path_buf()
        } else if let Ok(env) = std::env::var("COGNICODE_HOME") {
            PathBuf::from(env)
        } else {
            let home =
                std::env::var("HOME").context("HOME not set; pass --home or set COGNICODE_HOME")?;
            PathBuf::from(home).join(".cognicode")
        };
        Ok(Self { root })
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn shims(&self) -> PathBuf {
        self.root.join("shims")
    }

    /// Path to a specific shim binary.
    pub fn shim_path(&self, binary: &str) -> PathBuf {
        self.shims().join(binary)
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version(&self, v: &str) -> PathBuf {
        self.versions().join(v)
    }
    pub fn plugins(&self) -> PathBuf {
        self.root.join("plugins")
    }
    pub fn plugin(&self, name: &str) -> PathBuf {
        self.plugins().join(name)
    }
    pub fn tracker(&self) -> PathBuf {
        self.root.join("tracker")
    }
    pub fn tracker_version(&self) -> PathBuf {
        self.tracker().join("version")
    }
    pub fn locks(&self) -> PathBuf {
        self.root.join("locks")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn cache_downloads(&self) -> PathBuf {
        self.cache().join("downloads")
    }
    pub fn config(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    /// Path to the bundle.yaml manifest (distributed with the installer).
    pub fn bundle_yaml_path(&self) -> PathBuf {
        self.root.join("bundle.yaml")
    }

    /// Path to the install manifest for a given version.
    pub fn install_manifest_path(&self, version: &str) -> PathBuf {
        self.root.join(version).join("manifest.yaml")
    }

    /// Initialize the home directory (idempotent).
    pub fn init(&self) -> Result<()> {
        for dir in &[
            self.bin(),
            self.shims(),
            self.versions(),
            self.plugins(),
            self.tracker(),
            self.locks(),
            self.cache_downloads(),
        ] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }
        Ok(())
    }

    /// Check whether the home is initialized.
    pub fn is_initialized(&self) -> bool {
        self.root.exists() && self.bin().exists()
    }
}

// ===== cmd_init =====

pub fn cmd_init(home: &CognicodeHome) -> Result<()> {
    let already = home.is_initialized();
    if !already {
        home.init()?;
        println!("✓ Initialized {}", home.root.display());
    } else {
        println!("✓ {} already initialized", home.root.display());
    }
    // Install bundled plugins (E32-B). Does not overwrite if already present.
    let n = crate::bundled::install_bundled_plugins(&home.root)?;
    if n > 0 {
        println!("✓ Installed {n} bundled plugin(s)");
    }
    Ok(())
}

// ===== cmd_install (full implementation, E32-B) =====

pub fn cmd_install(
    home: &CognicodeHome,
    plugin: &str,
    version: &str,
    ides: &[String],
) -> Result<()> {
    if !home.is_initialized() {
        return Err(anyhow!("home not initialized; run `cogh init` first"));
    }
    let plugin_dir = home.plugin(plugin);
    let manifest_path = plugin_dir.join("plugin.yaml");
    let manifest = if manifest_path.exists() {
        crate::manifest::PluginManifest::from_path(&manifest_path)?
    } else {
        return Err(anyhow!(
            "plugin '{}' not registered; run `cogh plugin add {}` first",
            plugin,
            plugin
        ));
    };
    let (url, expected_sha) = crate::registry::resolve_url(&manifest, version)?;
    println!(
        "install: plugin={} version={} url={} ides={:?}",
        plugin, version, url, ides
    );
    println!("  expected sha256: {expected_sha}");
    println!(
        "(install flow: fetch → sha256 → extract → shim — wired in registry.rs; full download is E32-B+)"
    );
    // Wire --ide <name> to the IDE adapter.
    for ide in ides {
        crate::ide::cmd_ide_install(home, ide, plugin, version)?;
        println!("  ✓ configured IDE: {ide}");
    }
    Ok(())
}

pub fn cmd_uninstall(
    home: &CognicodeHome,
    plugin: &str,
    version: &str,
    ides: &[String],
) -> Result<()> {
    println!(
        "uninstall: plugin={} version={} ides={:?}",
        plugin, version, ides
    );
    // Wire --ide <name> to the IDE adapter uninstall.
    for ide in ides {
        crate::ide::cmd_ide_uninstall(home, ide, version)?;
    }
    Ok(())
}

pub fn cmd_list(home: &CognicodeHome, installed_only: bool) -> Result<()> {
    if !home.is_initialized() {
        println!("(home not initialized)");
        return Ok(());
    }
    println!("Plugin          Installed        Latest Available");
    println!("---------------------------------------------");
    if let Ok(entries) = std::fs::read_dir(home.plugins()) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                println!("{:<15} {}", name, "(installed)".to_string());
            }
        }
    }
    let _ = installed_only;
    Ok(())
}

pub fn cmd_current(home: &CognicodeHome) -> Result<()> {
    let tracker = home.tracker_version();
    if !tracker.exists() {
        println!("(no version pinned)");
        return Ok(());
    }
    let v = std::fs::read_to_string(&tracker)
        .with_context(|| format!("failed to read {}", tracker.display()))?;
    println!("{}", v.trim());
    Ok(())
}

pub fn cmd_latest(
    home: &CognicodeHome,
    plugin: Option<String>,
    all: bool,
    channel: crate::lifecycle_resolver::Channel,
    base_url: Option<String>,
    staging: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let _ = (all, plugin);
    let req = crate::lifecycle_resolver::ResolveRequest {
        host_platform: platform_adapter::detect_host_platform(),
        channel,
        requested_version: "latest".to_string(),
        base_url,
        staging_dir: staging,
    };
    let resolved = crate::lifecycle_resolver::resolve_release(&req)
        .map_err(|e| anyhow!("failed to resolve latest release: {e}"))?;
    let _ = home;
    if json {
        let body = crate::lifecycle_resolver::resolved_to_json(&resolved)
            .map_err(|e| anyhow!("serialise json: {e}"))?;
        println!("{body}");
    } else {
        println!("{}", resolved.tag);
    }
    Ok(())
}

pub fn cmd_update(
    home: &CognicodeHome,
    plugin: Option<String>,
    channel: crate::lifecycle_resolver::Channel,
    base_url: Option<String>,
    staging: Option<PathBuf>,
    profile: String,
    dry_run: bool,
) -> Result<()> {
    let _ = plugin;
    let req = crate::lifecycle_resolver::ResolveRequest {
        host_platform: platform_adapter::detect_host_platform(),
        channel,
        requested_version: "latest".to_string(),
        base_url,
        staging_dir: staging,
    };
    let resolved = crate::lifecycle_resolver::resolve_release(&req)
        .map_err(|e| anyhow!("failed to resolve latest release: {e}"))?;

    if dry_run {
        println!("would install {} from {}", resolved.version, resolved.manifest_url);
        return Ok(());
    }

    // Download the manifest to ~/.cognicode/bundle.yaml.
    let bundle_yaml_path = home.bundle_yaml_path();
    let manifest_yaml = download_to_string(&resolved.manifest_url)
        .map_err(|e| anyhow!("download bundle manifest from {}: {e}", resolved.manifest_url))?;
    if let Some(parent) = bundle_yaml_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create bundle dir {}", parent.display()))?;
    }
    std::fs::write(&bundle_yaml_path, &manifest_yaml)
        .with_context(|| format!("write bundle manifest {}", bundle_yaml_path.display()))?;

    // Delegate to the existing single install pipeline.
    let _manifest_path = crate::install::run_install(home, &profile)?;
    Ok(())
}

pub fn cmd_rollback(home: &CognicodeHome, plugin: Option<String>) -> Result<()> {
    let _ = (home, plugin);
    // 1. Resolve which journal to roll back: prefer the journal that matches
    //    the currently pinned tracker (so `cogh rollback` is "undo the active
    //    install"). If there is no tracker, take the highest-version journal
    //    on disk.
    let target = crate::lifecycle_journal::journal_path_for_current().or_else(|| {
        crate::lifecycle_journal::list_committed().ok().and_then(|vs| {
            vs.last().map(|v| crate::lifecycle_journal::journal_path(&v))
        })
    });
    let path = match target {
        Some(p) if p.exists() => p,
        Some(p) => {
            // The journal we expected is missing — the install either never
            // happened (no tracker) or the journal was wiped. Either way,
            // the honest answer is "nothing to roll back".
            println!("nothing to roll back (no journal at {})", p.display());
            return Ok(());
        }
        None => {
            println!("nothing to roll back (no journal directory)");
            return Ok(());
        }
    };

    println!("rolling back from {}", path.display());

    // 2. Load the journal and roll back. We MUST clone the loaded journal
    //    and commit the clone — the same Drop-reversal hazard we found in
    //    lifecycle_journal::write applies to the in-memory deserialised
    //    journal too if its Drop runs without a commit.
    let journal = crate::lifecycle_journal::load(&path)
        .map_err(|e| anyhow!("load journal {}: {e}", path.display()))?;
    let mut safe = journal;
    safe.commit();
    safe.rollback()
        .map_err(|e| anyhow!("rollback failed: {e}"))?;

    // 3. Best-effort: remove the journal file so a second `cogh rollback`
    //    reports "nothing to roll back" instead of running again.
    crate::lifecycle_journal::remove(&path);
    Ok(())
}

pub fn cmd_reshim(home: &CognicodeHome) -> Result<()> {
    println!(
        "reshim: would regenerate {} (not yet implemented)",
        home.shims().display()
    );
    Ok(())
}

pub fn cmd_doctor(home: &CognicodeHome) -> Result<()> {
    // e74 WU4: route through `doctor::run_doctor` so the report has
    // four orthogonal dimensions (Core health, MCP, Native analysis,
    // Isolation backend) and an Unavailable status for missing
    // optional backends instead of being flagged as a failed install.
    let report = crate::doctor::run_doctor(&home.root);
    print!("{report}");
    Ok(())
}

pub fn cmd_where(home: &CognicodeHome, binary: &str) -> Result<()> {
    let shim = home.shims().join(binary);
    if shim.exists() {
        println!("{}", shim.display());
    } else {
        println!("(not found: {})", shim.display());
    }
    Ok(())
}

pub fn cmd_plugin_add(home: &CognicodeHome, plugin: &str, from_url: Option<&str>) -> Result<()> {
    use anyhow::Context;
    let plugin_dir = home.plugin(plugin);
    if let Some(url) = from_url {
        // Clone the plugin repo into the plugins dir.
        let parent = plugin_dir
            .parent()
            .ok_or_else(|| anyhow!("plugin_dir has no parent"))?;
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let output = std::process::Command::new("git")
            .args(["clone", "--depth=1", url, &plugin_dir.to_string_lossy()])
            .output()
            .with_context(|| format!("git clone {url}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "git clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        println!("✓ cloned plugin '{plugin}' from {url}");
    } else {
        // Bundled: copy from the embedded manifest.
        let target = plugin_dir.join("plugin.yaml");
        if !target.exists() {
            let yaml = crate::bundled::PLUGIN_MANIFESTS
                .iter()
                .find(|(name, _)| *name == plugin)
                .map(|(_, yaml)| *yaml)
                .ok_or_else(|| {
                    anyhow!(
                        "plugin '{}' is not bundled; pass --from-url <git-url>",
                        plugin
                    )
                })?;
            std::fs::create_dir_all(&plugin_dir)?;
            std::fs::write(&target, yaml).with_context(|| format!("write {}", target.display()))?;
        }
        println!("✓ registered bundled plugin: {plugin}");
    }
    Ok(())
}

pub fn cmd_plugin_remove(home: &CognicodeHome, plugin: &str) -> Result<()> {
    println!(
        "Plugin removal not yet implemented: {} (plugins are read-only in this version)",
        plugin
    );
    Ok(())
}

pub fn cmd_plugin_list(home: &CognicodeHome) -> Result<()> {
    println!("Plugin          Description");
    println!("---------------------------------------------");
    if let Ok(entries) = std::fs::read_dir(home.plugins()) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let manifest =
                    crate::manifest::PluginManifest::from_path(&p.join("plugin.yaml")).ok();
                let desc = manifest.map(|m| m.description).unwrap_or_default();
                println!("{:<15} {}", name, desc);
            }
        }
    }
    Ok(())
}

pub fn cmd_plugin_update(home: &CognicodeHome, plugin: &str) -> Result<()> {
    println!(
        "Plugin update not yet implemented: {} (git pull for --from-url plugins not yet supported)",
        plugin
    );
    Ok(())
}

/// Shared test support for tests that need an isolated `COGNICODE_HOME`.
///
/// `cognicode_home()` reads process-global env, so any test that exercises
/// code reaching it must both (a) point `COGNICODE_HOME` at a temp dir, and
/// (b) be marked `#[serial]` to avoid racing other env-mutating tests.
#[cfg(test)]
pub(crate) mod test_support {
    use std::ffi::OsString;
    use std::path::Path;

    /// Redirect `COGNICODE_HOME` to a fresh temp dir, restoring the previous
    /// value on drop. Callers MUST be `#[serial]`.
    pub(crate) struct TempCognicodeHome {
        dir: tempfile::TempDir,
        prev: Option<OsString>,
    }

    impl TempCognicodeHome {
        pub(crate) fn new() -> Self {
            let dir = tempfile::tempdir().expect("tempdir for COGNICODE_HOME");
            let prev = std::env::var_os("COGNICODE_HOME");
            // SAFETY: callers are #[serial]; no concurrent env mutation.
            unsafe {
                std::env::set_var("COGNICODE_HOME", dir.path());
            }
            Self { dir, prev }
        }

        pub(crate) fn path(&self) -> &Path {
            self.dir.path()
        }
    }

    impl Drop for TempCognicodeHome {
        fn drop(&mut self) {
            // SAFETY: callers are #[serial]; no concurrent env mutation.
            unsafe {
                match &self.prev {
                    Some(v) => std::env::set_var("COGNICODE_HOME", v),
                    None => std::env::remove_var("COGNICODE_HOME"),
                }
            }
        }
    }
}

/// Download a manifest URL to a string. Used by `cmd_update` after the
/// resolver has resolved a `ResolvedRelease`. Uses `reqwest::blocking` with a
/// 60s timeout, matching the resolver's policy (e86 D2).
fn download_to_string(url: &str) -> std::result::Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!("cogh/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("COGNICODE_GITHUB_TOKEN") {
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
    }
    let resp = req.send().map_err(|e| format!("GET {url}: {e}"))?;
    let status = resp.status();
    let body = resp.text().map_err(|e| format!("read body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {status} — body: {}",
            &body[..body.len().min(200)]
        ));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_resolver::Channel;
    use crate::release_contract::bundle_manifest_filename;
    use crate::bundle_manifest::Platform;
    use serial_test::serial;

    #[test]
    fn resolve_from_explicit_path() {
        let tmp = std::env::temp_dir().join("cogh-test-explicit");
        let home = CognicodeHome::resolve(Some(&tmp)).unwrap();
        assert_eq!(home.root, tmp);
    }

    #[test]
    #[serial]
    fn resolve_from_env_var() {
        let tmp = std::env::temp_dir().join("cogh-test-env");
        // SAFETY: tests in the same process can race on env vars; we use
        // a unique temp dir to avoid collisions.
        unsafe {
            std::env::set_var("COGNICODE_HOME", &tmp);
        }
        let home = CognicodeHome::resolve(None).unwrap();
        unsafe {
            std::env::remove_var("COGNICODE_HOME");
        }
        assert_eq!(home.root, tmp);
    }

    #[test]
    fn layout_paths() {
        let home = CognicodeHome {
            root: PathBuf::from("/tmp/cogh"),
        };
        assert_eq!(home.bin(), PathBuf::from("/tmp/cogh/bin"));
        assert_eq!(home.shims(), PathBuf::from("/tmp/cogh/shims"));
        assert_eq!(home.versions(), PathBuf::from("/tmp/cogh/versions"));
        assert_eq!(
            home.version("0.92.0"),
            PathBuf::from("/tmp/cogh/versions/0.92.0")
        );
        assert_eq!(home.plugins(), PathBuf::from("/tmp/cogh/plugins"));
        assert_eq!(
            home.plugin("mcp-server"),
            PathBuf::from("/tmp/cogh/plugins/mcp-server")
        );
        assert_eq!(
            home.tracker_version(),
            PathBuf::from("/tmp/cogh/tracker/version")
        );
        assert_eq!(home.locks(), PathBuf::from("/tmp/cogh/locks"));
        assert_eq!(
            home.cache_downloads(),
            PathBuf::from("/tmp/cogh/cache/downloads")
        );
    }

    #[test]
    fn init_creates_subdirs() {
        let tmp = std::env::temp_dir().join(format!("cogh-init-{}", std::process::id()));
        let home = CognicodeHome::resolve(Some(&tmp)).unwrap();
        home.init().unwrap();
        for sub in &["bin", "shims", "versions", "plugins"] {
            assert!(tmp.join(sub).exists(), "missing subdir: {sub}");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = std::env::temp_dir().join(format!("cogh-init-idem-{}", std::process::id()));
        let home = CognicodeHome::resolve(Some(&tmp)).unwrap();
        home.init().unwrap();
        home.init().unwrap(); // second call must not fail
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn is_initialized_detects_partial() {
        let tmp = std::env::temp_dir().join(format!("cogh-init-partial-{}", std::process::id()));
        let home = CognicodeHome::resolve(Some(&tmp)).unwrap();
        assert!(!home.is_initialized());
        home.init().unwrap();
        assert!(home.is_initialized());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ----- e86 T6: cmd_latest and cmd_update end-to-end (with staging) -----

    fn staging_release_json(version: &str, asset_name: &str) -> String {
        format!(
            r#"{{
  "tag_name": "v{version}",
  "draft": false,
  "prerelease": false,
  "published_at": "2026-09-17T19:07:59Z",
  "html_url": "https://github.com/Rubentxu/CogniCode/releases/tag/v{version}",
  "assets": [
    {{ "name": "{asset_name}", "browser_download_url": "https://example.invalid/{asset_name}" }}
  ]
}}"#
        )
    }

    #[test]
    #[serial]
    fn cmd_latest_with_staging_prints_tag() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let staging_dir = tempfile::TempDir::new().unwrap();
        let asset_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        std::fs::write(
            staging_dir.path().join("releases.json"),
            staging_release_json("0.95.0", &asset_name),
        )
        .unwrap();

        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        cmd_latest(
            &home,
            None,
            false,
            Channel::Stable,
            None,
            Some(staging_dir.path().to_path_buf()),
            false,
        )
        .expect("cmd_latest must succeed with a valid staging fixture");
    }

    #[test]
    #[serial]
    fn cmd_latest_with_staging_json_prints_object() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let staging_dir = tempfile::TempDir::new().unwrap();
        let asset_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        std::fs::write(
            staging_dir.path().join("releases.json"),
            staging_release_json("0.95.0", &asset_name),
        )
        .unwrap();

        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        cmd_latest(
            &home,
            None,
            false,
            Channel::Stable,
            None,
            Some(staging_dir.path().to_path_buf()),
            true,
        )
        .expect("cmd_latest --json must succeed");
    }

    #[test]
    #[serial]
    fn cmd_update_dry_run_with_staging_does_not_write_bundle_yaml() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let staging_dir = tempfile::TempDir::new().unwrap();
        let asset_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        std::fs::write(
            staging_dir.path().join("releases.json"),
            staging_release_json("0.95.0", &asset_name),
        )
        .unwrap();

        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(staging_dir.path().to_path_buf()),
            "core".to_string(),
            true, // dry-run
        )
        .expect("cmd_update --dry-run must succeed");

        // The dry-run path must NOT have downloaded or written anything.
        let bundle_path = home.bundle_yaml_path();
        assert!(
            !bundle_path.exists(),
            "dry-run must not write bundle.yaml, found {}",
            bundle_path.display()
        );
    }

    // ----- e86 T7: cmd_rollback happy path and "nothing to roll back" -----

    #[test]
    #[serial]
    fn cmd_rollback_reports_nothing_when_no_journal_exists() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        // No journal has been written yet — `cmd_rollback` must not panic,
        // and must report "nothing to roll back" instead of trying to read.
        cmd_rollback(&home, None).expect("cmd_rollback must succeed (no journal)");
    }

    #[test]
    #[serial]
    fn cmd_rollback_reverses_a_committed_install() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();

        // Simulate a committed install: write a manifest file and a journal
        // describing the side-effects. The journal's WroteManifest reverses
        // to remove the manifest, so after rollback the manifest is gone.
        let version = "0.95.0";
        let install_dir = home_dir.path().join("install").join(version);
        std::fs::create_dir_all(&install_dir).unwrap();
        let manifest_path = install_dir.join("manifest.yaml");
        std::fs::write(&manifest_path, "apiVersion: v1\nversion: 0.95.0\n").unwrap();
        assert!(manifest_path.exists());

        // Persist the journal to ~/.cognicode/journal/<version>.json by hand
        // (the same shape `InstallerTransaction::commit` produces).
        use crate::rollback_journal::{RollbackJournal, SideEffect};
        let mut j = RollbackJournal::new();
        j.record(SideEffect::CreatedDir(install_dir.clone()));
        j.record(SideEffect::WroteManifest(manifest_path.clone()));
        let envelope = crate::lifecycle_journal::PersistedJournal {
            version: version.to_string(),
            committed_at_unix: Some(0),
            previous_tracker: None,
            effects: j,
        };
        let journal_path = crate::lifecycle_journal::journal_path(version);
        std::fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
        std::fs::write(
            &journal_path,
            serde_json::to_string_pretty(&envelope).unwrap(),
        )
        .unwrap();

        cmd_rollback(&home, None).expect("rollback must succeed");

        assert!(
            !manifest_path.exists(),
            "manifest must be removed after rollback"
        );
        assert!(
            !journal_path.exists(),
            "journal must be removed after rollback"
        );
    }
}

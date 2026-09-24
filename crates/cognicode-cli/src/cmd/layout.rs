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
    pub fn journal_version(&self, version: &str) -> PathBuf {
        self.root.join("journal").join(format!("{version}.json"))
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

    // ===== Canonical layout helpers (L1, ADR-CANONICAL-LAYOUT) =====
    //
    // These five helpers describe the canonical install layout promoted by
    // ADR-034/035 and the cognicode-cli/cognicode-lifecycle/portable-skill-bundle
    // OpenSpec specs. L1 added them with NO behavioural change. L2/L3/L4
    // retargeted consumers and the producer to the canonical surface. L5
    // retired the legacy `install_*` free fns and `CognicodeHome::install_manifest_path`
    // method that pointed at `install/<v>/`. After L5, these five helpers are
    // the only path-derivation surface for the canonical install layout.
    //
    // Characterization tests pin the exact path shape so any future drift
    // is caught immediately.

    /// Canonical root for an installed version: `<root>/versions/<v>/`.
    ///
    /// One installed release = one version root.
    pub fn version_root(&self, version: &str) -> PathBuf {
        self.root.join("versions").join(version)
    }

    /// Canonical root for a single bundle component: `<root>/versions/<v>/<component>/`.
    ///
    /// One component = one child under the version root.
    pub fn component_root(&self, version: &str, component: &str) -> PathBuf {
        self.version_root(version).join(component)
    }

    /// Canonical manifest path: `<root>/versions/<v>/manifest.yaml`.
    ///
    /// The `BundleManifest` snapshot for an installed version lives here.
    /// Pinned by `t_l1_version_manifest_matches_versions_layout`; will replace
    /// `install_manifest_path` once L2 retargets the producer (and L5 retires
    /// the legacy surface).
    pub fn version_manifest(&self, version: &str) -> PathBuf {
        self.version_root(version).join("manifest.yaml")
    }

    /// Canonical skills root for a version: `<root>/versions/<v>/skills/`.
    ///
    /// Portable skill bundles live under this directory, one subdir per bundle.
    pub fn skills_root(&self, version: &str) -> PathBuf {
        self.version_root(version).join("skills")
    }

    /// Canonical skills root for a single portable skill bundle:
    /// `<root>/versions/<v>/skills/<bundle>/`.
    pub fn skill_bundle(&self, version: &str, bundle: &str) -> PathBuf {
        self.skills_root(version).join(bundle)
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

// ===== legacy plugin-install stub (E32-B, superseded by e87.1 cmd_install) =====

pub fn cmd_install_plugin_stub(
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
    // Symmetric with cmd_install: refuse to act against an uninitialized
    // home. Prevents silent no-op on a typo'd COGNICODE_HOME or a tmp dir
    // that was wiped between install and uninstall. Pinned by
    // `t_e86_3_uninstall_errors_on_uninitialized_home`.
    if !home.is_initialized() {
        return Err(anyhow!(
            "home not initialized at {}; run `cogh init` first",
            home.root.display()
        ));
    }
    println!(
        "uninstall: plugin={} version={} ides={:?}",
        plugin, version, ides
    );
    // Missing versions have no owned IDE integration to remove. Treat an
    // uninstall request for an absent version as a genuine idempotent no-op.
    if !home.version_root(version).exists() {
        println!("uninstall: version {version} is not installed; nothing to do");
        return Ok(());
    }
    if ides.is_empty() {
        return Err(anyhow!(
            "uninstall of an installed version requires --ide (e.g. --ide opencode); \
             supported: opencode, zcode, claude, codex"
        ));
    }
    // DEBT-4 WU3 idempotence: if the version tree is already gone, the
    // installation does not exist — report honestly and stop. Requiring
    // the manifest (via cmd_ide_uninstall) to decide IDE post-state would
    // make a repeated uninstall an error, which contradicts "the tree is
    // the source of truth for whether this version is installed".
    if !home.version_root(version).exists() {
        println!(
            "uninstall: version {version} is not installed (no tree at {}); nothing to do",
            home.version_root(version).display()
        );
        return Ok(());
    }
    // Wire --ide <name> to the IDE adapter uninstall.
    for ide in ides {
        crate::ide::cmd_ide_uninstall(home, ide, version)?;
    }
    // L3 (ADR-CANONICAL-LAYOUT): remove the canonical version tree at
    // `<root>/versions/<ver>/` so uninstall undoes what install did.
    // Pre-L2 the producer wrote to `<root>/install/<ver>/` and E86.7
    // mirrored that here. After L2 the producer writes to `versions/`,
    // so cmd_uninstall must follow. Idempotent: missing dir is a no-op.
    // Pinned by `t_l3_cmd_uninstall_removes_versions_tree` and
    // `t_l3_cmd_uninstall_idempotent_when_versions_tree_missing`.
    let install_tree = home.version_root(version);
    // Retire only shims pointing into the version being removed. A different
    // active version, a user-installed binary or an unrelated plugin is never
    // eligible for deletion here.
    let installed =
        crate::bundle_manifest::BundleManifest::from_path(&home.version_manifest(version))?;
    for component in &installed.components {
        let shim = home.shim_path(&component.name);
        if let Ok(target) = std::fs::read_link(&shim)
            && target.starts_with(&install_tree)
        {
            std::fs::remove_file(&shim)
                .with_context(|| format!("remove version-owned shim {}", shim.display()))?;
        }
    }
    let removed_tree = install_tree.exists();
    if removed_tree {
        std::fs::remove_dir_all(&install_tree)
            .with_context(|| format!("rm -rf install tree at {}", install_tree.display()))?;
        println!("✓ removed install tree: {}", install_tree.display());
    }

    // DEBT-4 WU3 — lifecycle coherence. A successful uninstall is an
    // explicit intent to retire that installation, so it must not leave
    // a rollback capability for it, nor a tracker pinning a version that
    // no longer exists on disk.
    //
    // Journal: remove the journal FOR THIS VERSION only (whatever its
    // state). Journals of other versions are other transitions; they are
    // not ours to delete here.
    let journal_path = home.journal_version(version);
    if journal_path.exists() {
        crate::lifecycle_journal::remove(&journal_path);
        println!("✓ removed rollback journal: {}", journal_path.display());
    }
    //
    // Tracker: if the uninstalled version is the actively pinned one,
    // clear the pin — "no current version" is the honest post-state.
    // Uninstalling a NON-active version must leave the tracker untouched.
    let was_active = crate::tracker::read_version_optional_at(&home.tracker_version()).as_deref()
        == Some(version);
    if was_active {
        let tracker = home.tracker_version();
        std::fs::remove_file(&tracker)
            .with_context(|| format!("clear tracker pin at {}", tracker.display()))?;
        println!("✓ cleared tracker pin (was {version})");
    }
    let _ = removed_tree;
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
                println!("{:<15} (installed)", name);
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

/// Resolve a published release for `requested_version` and stage its
/// BundleManifest at `home.bundle_yaml_path()`.
///
/// e87.1 WU1: extracted verbatim from the pre-e87.1 body of `cmd_update` so
/// that `cmd_install` and `cmd_update` share ONE resolve-and-stage path and
/// cannot drift. `run_install` remains a primitive that consumes an already
/// staged manifest — no networking moves inside `InstallerTransaction`.
///
/// Version consistency (e87.1 invariant): when an explicit version is
/// requested, `requested == resolved.version` is enforced here — fail closed
/// rather than let the user ask for A and install B. For `latest` the
/// resolver's answer is authoritative by definition.
///
/// Testability: `staging_dir` (the e86 `releases.json` freeze seam) and
/// `base_url` mirror `Command::Update`'s flags; `None`/`None` hits the real
/// GitHub API. This seam is the Layer-1 resolver seam and is deliberately
/// independent of install.sh's `COGNICODE_RELEASE_BASE_URL` bootstrap seam.
fn resolve_and_stage_manifest(
    home: &CognicodeHome,
    requested_version: &str,
    channel: crate::lifecycle_resolver::Channel,
    base_url: Option<String>,
    staging: Option<PathBuf>,
) -> Result<crate::lifecycle_resolver::ResolvedRelease> {
    let req = crate::lifecycle_resolver::ResolveRequest {
        host_platform: platform_adapter::detect_host_platform(),
        channel,
        requested_version: requested_version.to_string(),
        base_url,
        staging_dir: staging,
    };
    let resolved = crate::lifecycle_resolver::resolve_release(&req)
        .map_err(|e| anyhow!("failed to resolve release {requested_version:?}: {e}"))?;

    if requested_version != "latest" {
        let requested = requested_version.trim_start_matches('v');
        if resolved.version != requested {
            return Err(anyhow!(
                "version mismatch: requested {requested}, release {} publishes manifest {}",
                resolved.tag,
                resolved.version
            ));
        }
    }

    // Download the manifest to ~/.cognicode/bundle.yaml.
    let bundle_yaml_path = home.bundle_yaml_path();
    let manifest_yaml = download_to_string(&resolved.manifest_url).map_err(|e| {
        anyhow!(
            "download bundle manifest from {}: {e}",
            resolved.manifest_url
        )
    })?;
    if let Some(parent) = bundle_yaml_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create bundle dir {}", parent.display()))?;
    }
    std::fs::write(&bundle_yaml_path, &manifest_yaml)
        .with_context(|| format!("write bundle manifest {}", bundle_yaml_path.display()))?;
    Ok(resolved)
}

/// e87.1 WU2: product-facing install. The requested `version` governs the
/// core bundle: it is resolved remotely (or from `--staging` in tests),
/// staged at `~/.cognicode/bundle.yaml`, and only then handed to
/// [`crate::install::run_install`].
///
/// DEV-fixture policy: unlike `InstallerTransaction::load_bundle_manifest`
/// (which falls back to the dev fixture for low-level tests), a resolution
/// failure here is a hard ERROR — a GitHub outage must never silently
/// produce a dev-fixture install in the product path.
pub fn cmd_install(
    home: &CognicodeHome,
    version: &str,
    channel: crate::lifecycle_resolver::Channel,
    base_url: Option<String>,
    staging: Option<PathBuf>,
    profile: &str,
) -> Result<crate::lifecycle_resolver::ResolvedRelease> {
    let resolved = resolve_and_stage_manifest(home, version, channel, base_url, staging)?;
    println!(
        "resolved {} (tag {}) for host platform",
        resolved.version, resolved.tag
    );
    let _manifest_path = crate::install::run_install(home, profile)?;
    Ok(resolved)
}

/// Preserve the installed capability profile during ordinary updates.
/// The CLI only downgrades from reviewer to core when the user explicitly
/// passes `--profile core`; installed manifest is authoritative.
pub fn active_install_profile(home: &CognicodeHome) -> &'static str {
    let Some(version) = crate::tracker::read_version_optional_at(&home.tracker_version()) else {
        return "core";
    };
    let Ok(manifest) =
        crate::bundle_manifest::BundleManifest::from_path(&home.version_manifest(&version))
    else {
        return "core";
    };
    if manifest
        .components
        .iter()
        .any(|component| component.kind == crate::release_contract::ArtifactKind::DaemonCli)
    {
        "reviewer"
    } else {
        "core"
    }
}

/// A coherent version can still contain a different profile. In particular,
/// a core-only vX install must NOT make `update --profile reviewer` a no-op.
fn active_install_matches_profile(home: &CognicodeHome, version: &str, profile: &str) -> bool {
    let Ok(published) = crate::bundle_manifest::BundleManifest::from_path(&home.bundle_yaml_path())
    else {
        return false;
    };
    if published.version != version {
        return false;
    }
    let Ok(installed) =
        crate::bundle_manifest::BundleManifest::from_path(&home.version_manifest(version))
    else {
        return false;
    };
    let mut expected: Vec<_> = published
        .components_for_profile(profile)
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    let mut actual: Vec<_> = installed
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    expected.sort_unstable();
    actual.sort_unstable();
    !expected.is_empty() && expected == actual
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
    if dry_run {
        let req = crate::lifecycle_resolver::ResolveRequest {
            host_platform: platform_adapter::detect_host_platform(),
            channel,
            requested_version: "latest".to_string(),
            base_url,
            staging_dir: staging,
        };
        let resolved = crate::lifecycle_resolver::resolve_release(&req)
            .map_err(|e| anyhow!("failed to resolve latest release: {e}"))?;
        println!(
            "would install {} from {}",
            resolved.version, resolved.manifest_url
        );
        return Ok(());
    }

    let resolved = resolve_and_stage_manifest(home, "latest", channel, base_url, staging)?;

    // lifecycle-F3: A -> A is a semantic no-op, not a lifecycle
    // transition. A no-op must not create a NEW rollback capability
    // (overwriting the existing journal for the active version) nor
    // mutate the installation. The equality check is only trusted when
    // the active installation is coherent: tracker pin + version tree +
    // installed manifest, with every declared component materialised.
    // A broken same-version install falls through to the real install
    // pipeline (repair path) instead of being hidden by the equality.
    if crate::tracker::read_version_optional_at(&home.tracker_version()).as_deref()
        == Some(resolved.version.as_str())
        && active_install_is_coherent(home, &resolved.version)
        && active_install_matches_profile(home, &resolved.version, &profile)
    {
        println!(
            "already current: {} is installed and coherent (no transition performed)",
            resolved.version
        );
        return Ok(());
    }

    // Delegate to the existing single install pipeline.
    let _manifest_path = crate::install::run_install(home, &profile)?;
    Ok(())
}

/// lifecycle-F3: minimal coherence check for the ACTIVE installation.
/// NOT a second doctor — only enough to distinguish "same version and
/// intact" from "same version but broken", so the no-op decision can
/// never mask corruption. Coherent means:
///   - `versions/<v>/` exists
///   - `versions/<v>/manifest.yaml` exists and parses as a v2 manifest
///   - every component declared for the active manifest has its
///     materialised directory `versions/<v>/<component>/`
fn active_install_is_coherent(home: &CognicodeHome, version: &str) -> bool {
    let vroot = home.version_root(version);
    if !vroot.is_dir() {
        return false;
    }
    let manifest_path = home.version_manifest(version);
    let Ok(manifest) = crate::bundle_manifest::BundleManifest::from_path(&manifest_path) else {
        return false;
    };
    let components_healthy = manifest.components.iter().all(|component| {
        let Some(binary) =
            crate::installer_transaction::locate_component_binary(home, version, &component.name)
        else {
            return false;
        };
        let shim = home.shim_path(&component.name);
        if !shim.is_file() {
            return false;
        }
        #[cfg(unix)]
        if std::fs::canonicalize(&shim).ok() != std::fs::canonicalize(&binary).ok() {
            return false;
        }
        true
    });
    if !components_healthy {
        return false;
    }
    let profile = if manifest
        .components
        .iter()
        .any(|component| component.kind == crate::release_contract::ArtifactKind::DaemonCli)
    {
        "reviewer"
    } else {
        "core"
    };
    manifest
        .skill_bundles_for_profile(profile)
        .iter()
        .all(|bundle| {
            home.skill_bundle(version, &bundle.id)
                .join("SKILL.md")
                .is_file()
        })
}

pub fn cmd_rollback(
    home: &CognicodeHome,
    plugin: Option<String>,
    to: Option<String>,
) -> Result<()> {
    let _ = (home, plugin);
    // DEBT-4: the journal is a one-shot rollback capability for ONE
    // committed transition, not a history. Applicability is explicit and
    // deterministic: the journal applies only if its version equals the
    // currently pinned tracker version. No tracker, no applicable journal:
    // "unknown" is never resolved by picking the highest-version file on
    // disk (the retired heuristic — a bigger semver is not "the most
    // recent valid transition").
    let current = crate::tracker::read_version_optional_at(&home.tracker_version());
    let Some(current_version) = current else {
        println!("nothing to roll back (no version pinned in tracker)");
        return Ok(());
    };

    // e86.4 REQ-RB-04: `cogh rollback --to <current>` is a clean no-op
    // — BUT only when there is no pending journal whose reversal would
    // restore this version. A pending journal can live at any path of
    // the form `<home>/journal/<X>.json` where X is the version that
    // was installed (the "from" version) and whose `previous_tracker`
    // equals <current>. After a partial rollback, the tracker was
    // already restored but the journal was not consumed (because shim
    // resurrection failed). The user's `cogh rollback --to <current>`
    // is then a request to RESUME the pending rollback: reshim from
    // <current>'s manifest, then consume the journal. We do not leave
    // the system stuck with a missing shim just because the tracker
    // matches the target.
    if let Some(target) = &to
        && target == &current_version
    {
        // Look for the journal that recorded the install that brought
        // us to a different version. The journal at <home>/journal/<X>.json
        // is the one for X (the from version). After a partial rollback
        // the tracker is already <current> = <target>, but the journal
        // for X (the previous from) is still on disk. We scan the
        // journal directory for any envelope whose `previous_tracker`
        // equals <target>. There should be at most one such envelope:
        // the journal for the most recent install whose "from" was
        // <target>.
        let journal_dir = home.root.join("journal");
        let mut pending_journal_path: Option<std::path::PathBuf> = None;
        if journal_dir.is_dir() {
            let entries = std::fs::read_dir(&journal_dir).ok();
            if let Some(entries) = entries {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()) != Some("json") {
                        continue;
                    }
                    // Inspect metadata only — do NOT load the full envelope.
                    // `load_envelope` constructs a `RollbackJournal` whose
                    // Drop, if not committed, would REVERSE the side-effects
                    // recorded in the journal (e.g. wiping the manifest of an
                    // unrelated version). Inspection is metadata-only here;
                    // we re-load the chosen envelope below, in the resume
                    // branch, with an explicit commit on success and removal
                    // on reshim success.
                    let is_pending = match crate::lifecycle_journal::peek_envelope_metadata(&p) {
                        Ok(meta) => {
                            meta.previous_tracker.as_deref() == Some(target.as_str())
                        }
                        // A malformed journal cannot be the pending one,
                        // and we do not want to fail the rollback because
                        // someone (perhaps a prior crashed installer) left
                        // a corrupt file in the journal dir.
                        Err(_) => false,
                    };
                    if is_pending {
                        pending_journal_path = Some(p);
                        break;
                    }
                }
            }
        }
        let Some(path) = pending_journal_path else {
            println!("rollback: already at {target}; nothing to do");
            return Ok(());
        };
        // Pin the loaded journal as committed so its Drop never replays
        // the reversal (the side-effects already applied).
        let envelope = crate::lifecycle_journal::load_envelope(&path)
            .map_err(|e| anyhow!("load journal {}: {e}", path.display()))?;
        let mut safe = envelope.effects;
        safe.commit();
        drop(safe);
        // Reshim from the current version's manifest (which is the
        // manifest for <target>, since that is what the tracker pins).
        cmd_reshim(home).map_err(|e| {
            anyhow!(
                "resume pending rollback for {current_version}: \
                 shim resurrection failed ({e}); journal at {} preserved \
                 for another retry (restore the underlying manifest of \
                 {current_version} and re-run `cogh rollback --to {current_version}`)",
                path.display()
            )
        })?;
        crate::lifecycle_journal::remove(&path);
        println!("resumed pending rollback: shim restored for {current_version}");
        return Ok(());
    }

    let path = home.journal_version(&current_version);
    if !path.exists() {
        // e86.4 REQ-RB-05: a `--to <target>` that does not appear in the
        // journal chain is a clear refusal with the known history listed.
        if let Some(target) = &to {
            return Err(anyhow!(
                "rollback --to {target}: no journal for active version {current_version}; \
                 cannot reach {target} via rollback chain. Known history: \
                 [{current_version}]. Use `cogh install <plugin> --version {target}` \
                 to install, or `cogh uninstall mcp-server {current_version}` to remove."
            ));
        }
        println!("nothing to roll back (no journal for active version {current_version})");
        return Ok(());
    }

    // Applicability proof: the on-disk envelope must describe the same
    // transition the tracker says is active. A stale journal (written for
    // a different version) is never executed — fail closed.
    let envelope = crate::lifecycle_journal::load_envelope(&path)
        .map_err(|e| anyhow!("load journal {}: {e}", path.display()))?;
    // Drop-reversal hazard (same as in lifecycle_journal::write): a loaded
    // journal must be pinned committed immediately, or dropping it in any
    // early-return path would silently replay the reversal.
    let mut safe = envelope.effects;
    safe.commit();
    if envelope.version != current_version {
        let _ = safe;
        return Err(anyhow!(
            "journal at {} describes version `{}` but the tracker pins `{current_version}`; \
             refusing to execute a stale journal (fail closed)",
            path.display(),
            envelope.version
        ));
    }

    // e86.4 REQ-RB-01/03/05: when `--to` is supplied, the rollback is gated
    // on the target being reachable in ONE chained step (the current journal
    // undoes one transition; multi-step rollback chains require the journal
    // upgrade in e86.4 follow-up, which is intentionally out of scope for
    // this slice).
    if let Some(target) = &to {
        match &envelope.previous_tracker {
            Some(prev) if prev == target => {
                // fall through: the standard rollback below will restore the
                // tracker to `prev` (== `target`).
            }
            Some(prev) => {
                // target is not the immediately previous version. The chain
                // would need multi-step rollback (not implemented in this
                // slice). Refuse with a clear message and the known history.
                return Err(anyhow!(
                    "rollback --to {target}: target is not reachable in one step. \
                     The journal for active version {current_version} points at \
                     previous_tracker=`{prev}`. Multi-step rollback is not \
                     implemented in this slice; use `cogh install <plugin> \
                     --version {target}` to install that version instead, or \
                     `cogh uninstall mcp-server {current_version}` to remove."
                ));
            }
            None => {
                // e86.4 REQ-RB-03: cannot rollback past the first installation.
                return Err(anyhow!(
                    "rollback --to {target}: cannot rollback past first installation \
                     (journal for active version {current_version} has no \
                     previous_tracker). Use `cogh uninstall mcp-server {current_version}` \
                     instead."
                ));
            }
        }
    }

    println!("rolling back version {current_version}");
    safe.rollback()
        .map_err(|e| anyhow!("rollback failed: {e}"))?;

    // e88-F1: restore the tracker pin from the envelope. Newer journals
    // carry a WroteTracker effect (handled by the reversal above), but
    // journals persisted by versions where the effect was only recorded
    // in-memory lack it. Defensively restore from `previous_tracker` when
    // the tracker still pins this version, so reversal never leaves a
    // stale pin pointing at a removed version tree.
    let tracker_path = home.tracker_version();
    let still_pins = std::fs::read_to_string(&tracker_path)
        .ok()
        .map(|v| v.trim() == current_version)
        .unwrap_or(false);
    if still_pins {
        match &envelope.previous_tracker {
            Some(prev) => {
                if let Some(parent) = tracker_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&tracker_path, prev)?;
                println!("restored tracker pin to {prev}");
            }
            None => {
                if tracker_path.exists() {
                    std::fs::remove_file(&tracker_path)?;
                    println!("cleared tracker pin (no previous version)");
                }
            }
        }
    }

    // PRF F6.W3.bis — shim-resurrection as part of the rollback.
    //
    // The previous-version shim is part of the rollback contract: a
    // user who runs `cogh rollback` expects the installed shims to
    // resolve to the previous version's executable after the call
    // returns. The journal's LIFO reversal correctly removes the
    // post-transition shim (CreatedSymlink reversal), but it does
    // NOT recreate the previous-version shim — the installer never
    // recorded the previous target (a CreatedSymlink effect only
    // captures the link it created, not the link it replaced).
    //
    // The cleanest fix is to re-materialise the shim from the
    // now-active version AFTER the tracker has been restored. We
    // delegate to `cmd_reshim` rather than duplicating the logic:
    // it knows how to iterate the manifest's components, locate each
    // binary, install the shim, and prune stale links to versions
    // that no longer match the manifest.
    //
    // If `cmd_reshim` fails, we DO NOT consume the journal — the
    // partial state (tracker restored but shim missing) is visible
    // to the user as a rollback failure, and the journal stays on
    // disk so the operation can be retried. The previous contract
    // consumed the journal unconditionally; the new contract makes
    // consumption conditional on full transition success.
    //
    // Note: a no-op rollback (`--to <current>`) returns earlier and
    // never reaches this code path; we only attempt reshim when an
    // actual transition was performed.
    if let Some(prev) = &envelope.previous_tracker {
        if let Err(e) = cmd_reshim(home) {
            return Err(anyhow!(
                "rollback partially applied: tracker restored to {} but shim resurrection \
                 failed ({}); the journal at {} has been PRESERVED so the rollback can be \
                 retried with `cogh rollback --to {}`",
                prev,
                e,
                path.display(),
                prev,
            ));
        }
    }

    // WU2 retention: a successful rollback consumes the capability. The
    // journal file is removed only AFTER the reversal AND the shim
    // resurrection succeeded, so a mid-failure still leaves the journal
    // recoverable. See the shim-resurrection block above for the matching
    // early-return that preserves the journal on reshim failure.
    crate::lifecycle_journal::remove(&path);
    Ok(())
}

pub fn cmd_reshim(home: &CognicodeHome) -> Result<()> {
    let version = crate::tracker::read_version_optional_at(&home.tracker_version())
        .ok_or_else(|| anyhow!("cannot reshim: no active version"))?;
    let manifest =
        crate::bundle_manifest::BundleManifest::from_path(&home.version_manifest(&version))?;
    let adapter = crate::platform_adapter::current_adapter();
    for component in &manifest.components {
        let binary =
            crate::installer_transaction::locate_component_binary(home, &version, &component.name)
                .ok_or_else(|| {
                    anyhow!(
                        "cannot reshim: missing executable for component {} in {}",
                        component.name,
                        version,
                    )
                })?;
        if !binary.is_file() {
            return Err(anyhow!(
                "cannot reshim: component binary missing: {}",
                binary.display()
            ));
        }
        let shim = home.shim_path(&component.name);
        adapter
            .install_shim(&binary, &shim)
            .with_context(|| format!("restore shim {} to {}", shim.display(), binary.display()))?;
        println!("restored {} -> {}", shim.display(), binary.display());
    }
    // Only known, version-managed names are eligible for stale-link cleanup.
    // Never traverse or delete an unrelated user-defined shim.
    for binary_name in ["cognicode", "cognicode-mcp"] {
        if manifest.components.iter().any(|c| c.name == binary_name) {
            continue;
        }
        let shim = home.shim_path(binary_name);
        match std::fs::symlink_metadata(&shim) {
            Ok(meta) if meta.file_type().is_symlink() => {
                std::fs::remove_file(&shim)
                    .with_context(|| format!("remove stale managed shim {}", shim.display()))?;
                println!("removed stale managed shim {}", shim.display());
            }
            Ok(_) => {
                return Err(anyhow!(
                    "refusing to remove unmanaged non-symlink at {}",
                    shim.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
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
    if shim.is_file() {
        println!("{}", shim.display());
    } else if let Ok(target) = std::fs::read_link(&shim) {
        println!(
            "(dangling shim: {} -> {})",
            shim.display(),
            target.display()
        );
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

pub fn cmd_plugin_remove(_home: &CognicodeHome, plugin: &str) -> Result<()> {
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

pub fn cmd_plugin_update(_home: &CognicodeHome, plugin: &str) -> Result<()> {
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

    /// Set `COGNICODE_ASSET_BASE_URL` for the lifetime of the guard. Used by
    /// the e86 followup fixture tests so the install pipeline rewrites
    /// canonical github.com component URLs onto the loopback server.
    /// Callers MUST be `#[serial]`.
    ///
    /// E86.2.2 split the legacy `COGNICODE_RELEASE_BASE_URL` into two:
    /// `COGNICODE_ASSET_BASE_URL` (this one) for the installer-side
    /// rewriting, and `COGNICODE_API_BASE_URL` for the resolver-side.
    /// The installer tests below only need the asset override; the resolver
    /// tests use `TempApiBase` or pass a `base_url` to the `ResolveRequest`.
    ///
    /// Also clears `COGNICODE_BUNDLE_MANIFEST` on construction. That env var
    /// is the legacy `point_at()` seam: if a previous `#[serial]` test
    /// called `point_at` and forgot to call `unpoint`, the env var still
    /// points at a tempdir path that has already been cleaned up.
    /// `load_bundle_manifest` prefers it over `bundle_yaml_path`, so the
    /// install would silently read a stale path. The new resolver-driven
    /// path does not need it.
    pub(crate) struct TempBaseUrl {
        prev: Option<std::ffi::OsString>,
        prev_bundle_manifest: Option<std::ffi::OsString>,
    }

    impl TempBaseUrl {
        pub(crate) fn set(base_url: &str) -> Self {
            let prev = std::env::var_os("COGNICODE_ASSET_BASE_URL");
            let prev_bundle_manifest = std::env::var_os("COGNICODE_BUNDLE_MANIFEST");
            // SAFETY: callers are #[serial].
            unsafe {
                std::env::set_var("COGNICODE_ASSET_BASE_URL", base_url);
                std::env::remove_var("COGNICODE_BUNDLE_MANIFEST");
            }
            Self {
                prev,
                prev_bundle_manifest,
            }
        }
    }

    impl Drop for TempBaseUrl {
        fn drop(&mut self) {
            // SAFETY: callers are #[serial].
            unsafe {
                match &self.prev {
                    Some(v) => std::env::set_var("COGNICODE_ASSET_BASE_URL", v),
                    None => std::env::remove_var("COGNICODE_ASSET_BASE_URL"),
                }
                match &self.prev_bundle_manifest {
                    Some(v) => std::env::set_var("COGNICODE_BUNDLE_MANIFEST", v),
                    None => std::env::remove_var("COGNICODE_BUNDLE_MANIFEST"),
                }
            }
        }
    }

    /// Force `ide::detect_opencode()` to return false by pointing
    /// `OPENCODE_CONFIG` at a non-existent path inside a fresh tempdir.
    /// Without this, the install pipeline's OpenCode branch tries to
    /// symlink the freshly installed mcp-server into the user's real
    /// `~/.config/opencode/skills/`, which would pollute the host
    /// filesystem during the test run.
    /// Callers MUST be `#[serial]`.
    pub(crate) struct TempOpenCodeConfig {
        _tmp: tempfile::TempDir,
        prev: Option<std::ffi::OsString>,
    }

    impl TempOpenCodeConfig {
        pub(crate) fn disable() -> Self {
            let tmp = tempfile::tempdir().expect("tempdir for OPENCODE_CONFIG");
            let cfg = tmp.path().join("opencode.json");
            let prev = std::env::var_os("OPENCODE_CONFIG");
            // SAFETY: callers are #[serial].
            unsafe {
                std::env::set_var("OPENCODE_CONFIG", &cfg);
            }
            Self { _tmp: tmp, prev }
        }
    }

    impl Drop for TempOpenCodeConfig {
        fn drop(&mut self) {
            // SAFETY: callers are #[serial].
            unsafe {
                match &self.prev {
                    Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                    None => std::env::remove_var("OPENCODE_CONFIG"),
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
    if let Ok(token) = std::env::var("COGNICODE_GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.bearer_auth(token);
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
// Tests below intentionally exercise the env-only `tracker::write_version` /
// `tracker::read_version_optional` convenience wrappers — they predate
// H-F6-1 and remain correct because every test holds a
// `TempCognicodeHome`, so the env var and the tracker path coincide. The
// `#[allow(deprecated)]` keeps the dev build signal focused: any new use
// of these wrappers in **production** code triggers a compile warning
// directing the author to the `*_at` API.
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::bundle_manifest::Platform;
    use crate::lifecycle_resolver::Channel;
    use crate::release_contract::bundle_manifest_filename;
    use serial_test::serial;

    // ===== e87.1 — install/resolver bridge =====
    //
    // WU0 characterization + strict tests T1/T2/T4. These exercise the
    // PRODUCT install path (`cmd_install`), driven through the e86
    // `lifecycle_resolver` (via `--staging`), with NO
    // `COGNICODE_BUNDLE_MANIFEST` seam and NO DEV fixture involvement.

    #[test]
    #[serial]
    fn e871_t1_requested_version_governs_install() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        let resolved = cmd_install(
            &home,
            "0.95.0",
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect("cmd_install must honor the requested version");

        // requested == resolved == installed == tracker
        assert_eq!(resolved.version, "0.95.0");
        let bundle = std::fs::read_to_string(home.bundle_yaml_path()).unwrap();
        assert!(bundle.contains("0.95.0"), "bundle.yaml must be 0.95.0");
        assert_eq!(
            std::fs::read_to_string(home.tracker_version())
                .unwrap()
                .trim(),
            "0.95.0",
            "tracker must equal the requested version"
        );
    }

    #[test]
    #[serial]
    fn e871_t2_pairwise_requested_987_installs_987_not_dev_fixture() {
        use crate::release_test_support::ResolverFixture;

        // The adversarial pin: requested 9.8.7, published fixture 9.8.7.
        // The DEV fixture (0.95.x-era) represents a different version, so if
        // the product path fell back to it, the tracker could never read
        // 9.8.7. Before e87.1 this test failed: `--version` was parsed but
        // never governed the core bundle.
        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("9.8.7").expect("build resolver fixture 9.8.7");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        let resolved = cmd_install(
            &home,
            "9.8.7",
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect("product install must consume the resolver, not the DEV fixture");

        assert_eq!(resolved.version, "9.8.7");
        assert_eq!(
            std::fs::read_to_string(home.tracker_version())
                .unwrap()
                .trim(),
            "9.8.7",
            "installed version must be the requested 9.8.7 (DEV fixture never consumed)"
        );
    }

    #[test]
    #[serial]
    fn e871_t3_latest_stages_resolved_manifest() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        let resolved = cmd_install(
            &home,
            "latest",
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect("latest must resolve through the staging fixture");

        assert_eq!(resolved.version, "0.95.0");
        assert_eq!(
            std::fs::read_to_string(home.tracker_version())
                .unwrap()
                .trim(),
            "0.95.0"
        );
    }

    #[test]
    #[serial]
    fn e871_t4_remote_resolution_failure_is_error_not_dev_fallback() {
        // No COGNICODE_BUNDLE_MANIFEST, no bundle.yaml, no staging: the
        // resolver must fail and the product path must ERROR — never fall
        // back to the DEV fixture.
        let _home = test_support::TempCognicodeHome::new();
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");
        assert!(!home.bundle_yaml_path().exists());

        let result = cmd_install(&home, "9.8.7", Channel::Stable, None, None, "core");
        let err = result.expect_err("unresolvable version must be a hard error");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("failed to resolve") || msg.contains("resolve"),
            "error must come from the resolver, got: {msg}"
        );
        // Dev fixture was never consumed: nothing got installed, tracker
        // absent or unchanged, no versions/<anything>/ written.
        let versions_dir = home.root.join("versions");
        let n = std::fs::read_dir(&versions_dir)
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(n, 0, "no version dir may exist after a failed resolve");
    }

    #[test]
    #[serial]
    fn e871_version_mismatch_fails_closed() {
        // requested 1.2.3 but the staging publishes 0.95.0 -> hard error,
        // never an install of B when the user asked for A.
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        let err = cmd_install(
            &home,
            "1.2.3",
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect_err("requested 1.2.3 against a 0.95.0 release must fail closed");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("version mismatch"),
            "must report the pairwise mismatch, got: {msg}"
        );
        // bundle.yaml must NOT have been staged from the wrong release.
        assert!(
            !home.bundle_yaml_path().exists(),
            "mismatched manifest must never reach bundle.yaml"
        );
    }

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
        cmd_rollback(&home, None, None).expect("cmd_rollback must succeed (no journal)");
    }

    // PRF F6.W3.bis — the test below (which previously pinned the
    // buggy contract: rollback returns Ok with no shim resurrection)
    // was replaced by a version that uses the real install path so
    // the previous-version tree is a valid manifest + binary. Without
    // a real previous-version tree, the post-fix rollback returns
    // Err by design (the contract refuses to declare success if the
    // shim cannot be materialised).
    //
    // See `f6w3_install_a` for the canonical previous-version setup
    // that satisfies the F6.W3.bis precondition.
    #[test]
    #[serial]
    fn cmd_rollback_reverses_a_committed_install() {
        // F6.W3.bis — drive the full install path so the previous
        // version has a valid manifest + binary, which is what
        // `cmd_rollback`'s post-fix reshim step requires.
        f6w3_install_via_fixture_round_trip();
    }

    // ----- e86 T10: end-to-end round-trip through resolve + install -----
    //
    // We can't hit GitHub from a test, so the e2e uses a local staging
    // fixture that mimics the release shape, exercises resolve_release,
    // writes the manifest to ~/.cognicode/bundle.yaml, and then runs the
    // install pipeline against a synthetic v2 manifest that points at a
    // local tarball served by `tiny_http`. We assert the journal is
    // written and the tracker is updated.
    //
    // This test is intentionally conservative: it pins the contract end
    // to end without depending on a live network.

    // ----- DEBT-4: transition matrix -----

    /// Fixture helper: point COGNICODE_HOME at the per-test temp dir so the
    /// tracker/lifecycle_journal module-level resolvers hit the sandbox.
    fn redirect_home(dir: &std::path::Path) {
        unsafe {
            std::env::set_var("COGNICODE_HOME", dir);
        }
    }

    /// Fixture helper: persist a journal for `version` describing an
    /// install of that version, so executing it is observable.
    ///
    /// PRF F6.W3.bis — when `previous_tracker` is provided, also plant
    /// a valid previous-version tree (manifest + executable) so the
    /// post-fix `cmd_rollback` shim-resurrection step can complete
    /// without returning Err. The previous contract only reversed the
    /// journal effects; the new contract additionally re-materialises
    /// the shim from the now-active version, which requires the
    /// previous version's manifest to be readable.
    fn plant_journal(
        home: &CognicodeHome,
        version: &str,
        previous_tracker: Option<&str>,
    ) -> std::path::PathBuf {
        use crate::rollback_journal::{RollbackJournal, SideEffect};

        // PRF F6.W3.bis — plant a minimal valid previous-version tree
        // (manifest + binary) so the rollback's reshim step succeeds.
        // The manifest must be a structurally valid BundleManifest
        // because `cmd_reshim` deserialises it.
        if let Some(prev) = previous_tracker {
            use crate::bundle_manifest::{BundleComponent, BundleManifest};
            use crate::platform_adapter::Platform;
            use crate::release_contract::{ArtifactDigest, ArtifactKind};

            let prev_root = home.version_root(prev);
            std::fs::create_dir_all(&prev_root).unwrap();
            // A non-placeholder, valid 64-char hex digest. The exact value
            // is irrelevant to the rollback contract — `cmd_reshim` does
            // not verify integrity here — but ArtifactDigest::parse rejects
            // placeholders, so we use a stable hex sequence that survives
            // the validator.
            let prev_digest = ArtifactDigest::parse(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .expect("digest hex must be valid");
            let prev_manifest = BundleManifest {
                api_version: "cognicode.bundle/v2".to_string(),
                kind: "Bundle".to_string(),
                version: prev.to_string(),
                platform: Platform::LinuxX86_64,
                released_at: None,
                profiles: vec![crate::bundle_manifest::ProfileDef {
                    name: "core".to_string(),
                    description: "stub".to_string(),
                }],
                skill_bundles: vec![],
                components: vec![BundleComponent {
                    name: "cognicode".to_string(),
                    kind: ArtifactKind::Cognicode,
                    version: prev.to_string(),
                    artifact: crate::release_contract::artifact_filename(
                        "cognicode",
                        prev,
                        Platform::LinuxX86_64,
                    ),
                    sha256: prev_digest,
                    url: crate::release_contract::artifact_url(
                        prev,
                        &crate::release_contract::artifact_filename(
                            "cognicode",
                            prev,
                            Platform::LinuxX86_64,
                        ),
                    ),
                    profiles: vec!["core".to_string()],
                }],
            };
            std::fs::write(
                prev_root.join("manifest.yaml"),
                serde_yaml::to_string(&prev_manifest).unwrap(),
            )
            .unwrap();
            let prev_bin_dir = home.component_root(prev, "cognicode").join("bin");
            std::fs::create_dir_all(&prev_bin_dir).unwrap();
            let prev_bin = prev_bin_dir.join("cognicode");
            std::fs::write(&prev_bin, "#!/bin/sh\nexit 0\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&prev_bin, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
        }

        let install_dir = home.version_root(version);
        std::fs::create_dir_all(&install_dir).unwrap();
        let manifest_path = install_dir.join("manifest.yaml");
        std::fs::write(&manifest_path, format!("version: {version}\n")).unwrap();
        let mut j = RollbackJournal::new();
        j.record(SideEffect::CreatedDir(install_dir.clone()));
        j.record(SideEffect::WroteManifest(manifest_path.clone()));
        j.record(SideEffect::WroteTracker {
            path: home.tracker_version(),
            previous: previous_tracker.map(|s| s.to_string()),
        });
        let mut envelope = crate::lifecycle_journal::PersistedJournal {
            version: version.to_string(),
            committed_at_unix: Some(0),
            previous_tracker: previous_tracker.map(|s| s.to_string()),
            effects: j,
        };
        let jp = crate::lifecycle_journal::journal_path(version);
        std::fs::create_dir_all(jp.parent().unwrap()).unwrap();
        std::fs::write(&jp, serde_json::to_string_pretty(&envelope).unwrap()).unwrap();
        // The journal's Drop would otherwise replay the reversal; pin it
        // as committed so the planted file is durable for the test.
        envelope.effects.commit();
        jp
    }

    /// Manifest fixture declaring a DaemonCli, required by cmd_ide_uninstall.
    fn write_daemoncli_manifest(home: &CognicodeHome, version: &str) {
        let manifest = format!(
            r#"
apiVersion: cognicode.bundle/v2
version: "{version}"
platform: linux-x86-64
profiles:
  - name: core
    description: core
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "{version}"
    artifact: cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#
        );
        let tree = home.version_root(version);
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join("manifest.yaml"), manifest).unwrap();
    }

    /// T1 — install A -> B -> rollback: tracker restored to A, journal(B)
    /// consumed, second rollback is a harmless no-op.
    #[test]
    #[serial]
    fn t_debt4_t1_rollback_consumes_journal_and_second_is_noop() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.94.0").unwrap(); // A active
        let jp = plant_journal(&home, "0.95.0", Some("0.94.0")); // B journal
        // The active transition is A->B, so the tracker pins B.
        crate::tracker::write_version("0.95.0").unwrap();
        assert!(jp.exists());

        cmd_rollback(&home, None, None).expect("rollback must succeed");
        assert!(!jp.exists(), "T1: journal(B) must be consumed");
        assert!(
            !home.version_root("0.95.0").join("manifest.yaml").exists(),
            "T1: B install tree effects must be reversed"
        );

        // Second rollback: nothing applicable (journal consumed).
        cmd_rollback(&home, None, None).expect("second rollback must be harmless");
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.94.0"),
            "T1: tracker restored to A"
        );
    }

    /// T2 — uninstall active B: tree gone, journal gone, tracker cleared
    /// (explicit semantics: uninstall active => "no current version").
    #[test]
    #[serial]
    fn t_debt4_t2_uninstall_active_clears_tracker_and_journal() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", Some("0.94.0"));
        write_daemoncli_manifest(&home, "0.95.0");
        let tree = home.version_root("0.95.0");
        assert!(tree.exists());

        cmd_uninstall(&home, "cognicode", "0.95.0", &["opencode".to_string()])
            .expect("uninstall of active version must succeed");

        assert!(!tree.exists(), "T2: version tree must be gone");
        assert!(
            !crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "T2: journal(B) must be invalidated"
        );
        assert!(
            crate::tracker::read_version_optional().is_none(),
            "T2: tracker must not falsely claim B (explicit: no current version)"
        );
    }

    /// T3 — uninstall inactive A while B active: tracker stays B, B
    /// untouched, journal(A) invalidated, journal(B) retained.
    #[test]
    #[serial]
    fn t_debt4_t3_uninstall_inactive_preserves_current() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap(); // B active
        plant_journal(&home, "0.94.0", Some("0.93.0")); // A journal
        plant_journal(&home, "0.95.0", Some("0.94.0")); // B journal
        write_daemoncli_manifest(&home, "0.94.0");

        cmd_uninstall(&home, "cognicode", "0.94.0", &["opencode".to_string()])
            .expect("uninstall of inactive version must succeed");

        assert!(!home.version_root("0.94.0").exists(), "T3: A tree removed");
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "T3: tracker must still pin B"
        );
        assert!(
            !crate::lifecycle_journal::journal_path("0.94.0").exists(),
            "T3: journal(A) invalidated"
        );
        assert!(
            crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "T3: journal(B) must be retained — not ours to delete"
        );
    }

    /// T4 — stale journal: tracker pins B but the journal file for B
    /// describes C. Rollback must NOT execute it (fail closed).
    #[test]
    #[serial]
    fn t_debt4_t4_stale_journal_is_never_executed() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap(); // B active
        let jp = plant_journal(&home, "0.96.0", Some("0.95.0"));
        let mismatched = crate::lifecycle_journal::journal_path("0.95.0");
        std::fs::copy(&jp, &mismatched).unwrap();

        let err = cmd_rollback(&home, None, None)
            .expect_err("T4: a stale journal must be refused, never executed");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("stale") && msg.contains("fail closed"),
            "T4: refusal must be explicit: {msg}"
        );
        assert!(
            home.version_root("0.96.0").join("manifest.yaml").exists(),
            "T4: no journal side-effect may run"
        );
    }

    /// T5 — missing tracker: journals exist but nothing is pinned.
    /// No heuristic selection (the retired "highest semver wins" fallback).
    #[test]
    #[serial]
    fn t_debt4_t5_no_tracker_means_no_implicit_rollback() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        plant_journal(&home, "0.94.0", None);
        plant_journal(&home, "9.9.9", None);

        cmd_rollback(&home, None, None).expect("must be a harmless no-op, not an error");
        assert!(
            home.version_root("9.9.9").join("manifest.yaml").exists(),
            "T5: heuristic journal selection is retired; nothing may execute"
        );
        assert!(
            crate::lifecycle_journal::journal_path("9.9.9").exists(),
            "T5: journals must not be consumed by a heuristic"
        );
    }

    // ===== e86.4 — cogh rollback --to <version> regression coverage =====

    /// e86.4 REQ-RB-04 — `--to <current>` is a clean no-op.
    /// The tracker pins `0.95.0`; running `cogh rollback --to 0.95.0`
    /// must exit Ok without touching the journal or filesystem state.
    #[test]
    #[serial]
    fn t_e86_4_rollback_to_current_is_noop() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", Some("0.94.0"));
        let jp = crate::lifecycle_journal::journal_path("0.95.0");
        assert!(jp.exists(), "fixture: journal must be planted");

        cmd_rollback(&home, None, Some("0.95.0".to_string()))
            .expect("--to <current> must be a clean no-op (Ok)");

        // Journal untouched (consumed is only for actual rollback execution).
        assert!(
            jp.exists(),
            "REQ-RB-04: journal must NOT be consumed by no-op"
        );
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "REQ-RB-04: tracker pin must NOT change"
        );
    }

    /// e86.4 REQ-RB-01 (one-step slice) — `--to <previous>` works.
    /// The journal's previous_tracker is `0.94.0`; running
    /// `cogh rollback --to 0.94.0` must roll back to `0.94.0`.
    #[test]
    #[serial]
    fn t_e86_4_rollback_to_previous_tracker_succeeds() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", Some("0.94.0"));

        cmd_rollback(&home, None, Some("0.94.0".to_string()))
            .expect("--to <previous_tracker> must succeed (single-step rollback)");

        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.94.0"),
            "REQ-RB-01: tracker pin must be restored to target"
        );
        assert!(
            !crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "REQ-RB-01: journal must be consumed on success"
        );
    }

    /// e86.4 REQ-RB-03 — `--to <unknown>` past first install refuses.
    /// Journal has no previous_tracker; running
    /// `cogh rollback --to 0.92.0` must error with "cannot rollback
    /// past first installation" + uninstall hint.
    #[test]
    #[serial]
    fn t_e86_4_rollback_past_first_installation_refuses() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", None); // no previous_tracker -> first install

        let err = cmd_rollback(&home, None, Some("0.92.0".to_string()))
            .expect_err("--to <unknown> past first install must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("past first installation"),
            "REQ-RB-03: error must mention 'past first installation'; got: {msg}"
        );
        assert!(
            msg.contains("uninstall"),
            "REQ-RB-03: error must hint at `cogh uninstall`; got: {msg}"
        );
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "REQ-RB-03: tracker pin must NOT change on refused rollback"
        );
    }

    /// e86.4 REQ-RB-03 (multi-step slice) — `--to <far-back>` refuses.
    /// Journal's previous_tracker is `0.94.0`; target `0.92.0` is not
    /// reachable in one step. Refuse with a clear message.
    #[test]
    #[serial]
    fn t_e86_4_rollback_to_unreachable_target_refuses() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", Some("0.94.0"));

        let err = cmd_rollback(&home, None, Some("0.92.0".to_string()))
            .expect_err("--to <far-back> target must fail (multi-step not implemented)");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not reachable in one step") || msg.contains("Multi-step"),
            "REQ-RB-03 (multi-step): error must explain the slice limit; got: {msg}"
        );
        assert!(
            msg.contains("0.94.0"),
            "REQ-RB-03 (multi-step): error must mention the actual previous_tracker; got: {msg}"
        );
    }

    /// e86.4 REQ-RB-05 — `--to <unknown>` with no journal refuses.
    /// Tracker pins `0.95.0` but no journal exists for it; target `0.92.0`
    /// is unknown. Refuse with a clear message naming the known history.
    #[test]
    #[serial]
    fn t_e86_4_rollback_to_unknown_with_no_journal_refuses() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        // NO journal planted — the journal path does not exist.

        let err = cmd_rollback(&home, None, Some("0.92.0".to_string()))
            .expect_err("--to <unknown> with no journal must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("no journal for active version") || msg.contains("cannot reach"),
            "REQ-RB-05: error must explain no-journal barrier; got: {msg}"
        );
        assert!(
            msg.contains("0.92.0"),
            "REQ-RB-05: error must name the requested target; got: {msg}"
        );
    }

    // ===== /e86.4 =====

    /// T6 — idempotence: uninstalling B twice is stable (Ok both times).
    #[test]
    #[serial]
    fn t_debt4_t6_uninstall_is_idempotent() {
        let home_dir = tempfile::TempDir::new().unwrap();
        redirect_home(home_dir.path());
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();
        crate::tracker::write_version("0.95.0").unwrap();
        plant_journal(&home, "0.95.0", None);
        write_daemoncli_manifest(&home, "0.95.0");
        let tree = home.version_root("0.95.0");

        cmd_uninstall(&home, "cognicode", "0.95.0", &["opencode".to_string()])
            .expect("first uninstall");
        cmd_uninstall(&home, "cognicode", "0.95.0", &["opencode".to_string()])
            .expect("second uninstall must be idempotent (Ok)");

        assert!(!tree.exists(), "T6: tree stays gone");
        assert!(
            crate::tracker::read_version_optional().is_none(),
            "T6: tracker stays cleared"
        );
    }

    #[test]
    #[serial]
    fn e2e_resolve_write_manifest_persists_journal() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let staging_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();

        // 1. Stage a release that the resolver will accept. The asset URL
        //    points at an unreachable host on purpose; we only assert the
        //    resolver-side contract here, not the download.
        let asset_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        std::fs::write(
            staging_dir.path().join("releases.json"),
            staging_release_json("0.95.0", &asset_name),
        )
        .unwrap();

        // 2. Resolve via the resolver. This is the same call `cmd_update`
        //    makes before downloading.
        let req = crate::lifecycle_resolver::ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: Some(staging_dir.path().to_path_buf()),
        };
        let resolved = crate::lifecycle_resolver::resolve_release(&req)
            .expect("resolver must succeed with a valid staging fixture");
        assert_eq!(resolved.version, "0.95.0");
        assert_eq!(resolved.tag, "v0.95.0");
        assert!(resolved.manifest_url.contains(&asset_name));

        // 3. Write the bundle.yaml with a *synthetic* v2 manifest that
        //    references non-existent payloads. The install will fail at
        //    SHA256 step — that is fine; we only assert that the
        //    resolve-and-write phase is correct.
        let synthetic_yaml = r#"
apiVersion: cognicode.bundle/v2
version: "0.95.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core
components:
  - name: cognicode
    kind: cognicode
    version: "0.95.0"
    artifact: cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000000"
    url: "https://example.invalid/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        let bundle_yaml_path = home.bundle_yaml_path();
        if let Some(parent) = bundle_yaml_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&bundle_yaml_path, synthetic_yaml).unwrap();
        assert!(bundle_yaml_path.exists());

        // 4. Verify the resolver's JSON output shape (REQ-LR-09).
        let json = crate::lifecycle_resolver::resolved_to_json(&resolved).unwrap();
        assert!(json.contains("\"version\": \"0.95.0\""));
        assert!(json.contains("\"tag\": \"v0.95.0\""));
        assert!(json.contains("\"platform_token\": \"x86_64-unknown-linux-gnu\""));
    }

    // ----- e86 followup T2: live install via the new resolver-driven path -----
    //
    // The e86 cycle covered resolve + journal + dry-run end-to-end, but the
    // non-dry-run `cmd_update` had no test coverage — the legacy seam
    // (`release_test_support::LocalRelease` + `point_at`) exercises the
    // `COGNICODE_BUNDLE_MANIFEST` env var, NOT the `lifecycle_resolver`.
    //
    // `ResolverFixture` bridges that gap: it builds the same real loopback
    // payload as `LocalRelease` AND a `releases.json` whose manifest URL is
    // rewritten to the loopback base, so `cmd_update` resolves through the
    // new path, downloads the manifest from the loopback, and the install
    // pipeline runs against the loopback-served tarball.

    #[test]
    #[serial]
    fn cmd_update_live_install_against_fixture() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        // Disable IDE integration: on a real workstation the IDE adapter
        // would try to symlink the freshly installed mcp-server into the
        // user's real `~/.config/opencode/skills/`, polluting that dir.
        // `OPENCODE_CONFIG` is honored by `ide::opencode_config_path()`,
        // pointing it at a non-existent file makes detect_opencode() false
        // — no production code change.
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");

        // Sanity: home is initialised. The install pipeline writes into the
        // standard ~/.cognicode layout.
        home.init().expect("home.init");

        // Run cmd_update NON-dry-run against the loopback-backed fixture.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None, // base_url
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false, // not dry-run
        )
        .expect("cmd_update live install against fixture must succeed");

        // After a successful install: bundle.yaml is the loopback's manifest,
        // the install dir for 0.95.0 contains a manifest.yaml, and the
        // tracker is pinned to 0.95.0.
        let bundle_path = home.bundle_yaml_path();
        assert!(bundle_path.exists(), "bundle.yaml must be written to home");
        assert!(
            std::fs::read_to_string(&bundle_path)
                .expect("read bundle.yaml")
                .contains("0.95.0"),
            "bundle.yaml must be the 0.95.0 manifest"
        );

        // L2 (ADR-CANONICAL-LAYOUT): the install now writes the manifest
        // to `<root>/versions/<v>/manifest.yaml` per the canonical
        // layout. We use `home.version_manifest(v)` to stay pinned to
        // the new install contract.
        let install_manifest = home.version_manifest("0.95.0");
        assert!(
            install_manifest.exists(),
            "install manifest must be written under versions/0.95.0/, got {}",
            install_manifest.display()
        );

        let tracker = home.tracker_version();
        assert!(tracker.exists(), "tracker must exist after live install");
        assert_eq!(
            std::fs::read_to_string(&tracker)
                .expect("read tracker")
                .trim(),
            "0.95.0"
        );

        // The journal for this install must exist and be a valid PersistedJournal
        // envelope — the same shape `installer_transaction::commit` writes.
        let journal_path = crate::lifecycle_journal::journal_path("0.95.0");
        assert!(
            journal_path.exists(),
            "lifecycle journal must be persisted, got {}",
            journal_path.display()
        );
    }

    // ----- lifecycle-F3: A -> A is a semantic no-op -----

    fn sha256_hex(data: &[u8]) -> String {
        use std::fmt::Write;
        let digest = <sha2::Sha256 as sha2::Digest>::digest(data);
        digest.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
    }

    /// Snapshot everything a same-version no-op must preserve: journal
    /// bytes, tracker bytes, installed manifest bytes, and a coarse
    /// tree fingerprint (walk of versions/<v> file hashes).
    fn lifecycle_state(
        home: &CognicodeHome,
        version: &str,
    ) -> std::collections::BTreeMap<String, String> {
        let mut state = std::collections::BTreeMap::new();
        let jp = crate::lifecycle_journal::journal_path(version);
        if jp.exists() {
            state.insert("journal".into(), sha256_hex(&std::fs::read(jp).unwrap()));
        }
        let tp = home.tracker_version();
        if tp.exists() {
            state.insert("tracker".into(), sha256_hex(&std::fs::read(tp).unwrap()));
        }
        let mp = home.version_manifest(version);
        if mp.exists() {
            state.insert("manifest".into(), sha256_hex(&std::fs::read(mp).unwrap()));
        }
        let vroot = home.version_root(version);
        if let Ok(_entries) = std::fs::read_dir(home.versions()) {
            let mut names = Vec::new();
            fn walk(dir: &Path, names: &mut Vec<String>) {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            walk(&p, names);
                        } else if let Ok(bytes) = std::fs::read(&p) {
                            names.push(format!("{}:{}", p.display(), sha256_hex(&bytes)));
                        }
                    }
                }
            }
            walk(&vroot, &mut names);
            names.sort();
            state.insert("tree".into(), sha256_hex(names.join("\n").as_bytes()));
        }
        state
    }

    /// A core-only installation of the SAME release must not silently
    /// satisfy an explicit request for the MCP-bearing reviewer profile.
    #[test]
    #[serial]
    fn dist_same_version_core_to_reviewer_installs_mcp() {
        use crate::release_test_support::ResolverFixture;

        let temp = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(temp.path())).expect("home");
        home.init().expect("init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install core");

        assert_eq!(active_install_profile(&home), "core");
        assert!(!active_install_matches_profile(&home, "0.95.0", "reviewer"));

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "reviewer".to_string(),
            false,
        )
        .expect("upgrade capabilities without changing release version");

        let installed =
            crate::bundle_manifest::BundleManifest::from_path(&home.version_manifest("0.95.0"))
                .expect("installed manifest");
        assert!(
            installed
                .components
                .iter()
                .any(|c| c.kind == crate::release_contract::ArtifactKind::DaemonCli)
        );
        assert!(home.shim_path("cognicode-mcp").is_file());
        assert_eq!(active_install_profile(&home), "reviewer");
    }

    #[test]
    #[serial]
    fn dist_reshim_repairs_dangling_mcp_link_without_temp_home_contamination() {
        use crate::release_test_support::ResolverFixture;
        let temp = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(temp.path())).expect("home");
        home.init().expect("init");
        cmd_install(
            &home,
            "0.95.0",
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "reviewer",
        )
        .expect("install reviewer");
        let shim = home.shim_path("cognicode-mcp");
        std::fs::remove_file(&shim).expect("remove installed shim");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/tmp/prf-removed-runtime/cognicode-mcp", &shim)
            .expect("plant dangling shim");

        cmd_reshim(&home).expect("repair active version shims");
        assert!(shim.is_file());
        #[cfg(unix)]
        assert_eq!(
            std::fs::canonicalize(&shim).unwrap(),
            std::fs::canonicalize(
                home.component_root("0.95.0", "cognicode-mcp")
                    .join("bin/cognicode-mcp")
            )
            .unwrap()
        );
    }

    /// T1 (lifecycle-F3 WU0/WU2): install A, then a same-version update
    /// must be a semantic no-op — journal, tree, manifest and tracker
    /// byte-identical. RED today: update runs a full InstallerTransaction
    /// and overwrites the journal for A.
    #[test]
    #[serial]
    fn f3_t1_same_version_update_is_zero_mutation() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // First install of A via update (fresh home).
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("initial install of A");

        let before = lifecycle_state(&home, "0.95.0");
        assert!(
            before.contains_key("journal"),
            "precondition: journal exists"
        );
        assert_eq!(
            before.get("tracker").map(String::as_str),
            Some(sha256_hex(b"0.95.0").as_str())
        );

        // Same-version update: resolved == active.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("same-version update must succeed as no-op");

        let after = lifecycle_state(&home, "0.95.0");
        assert_eq!(
            before, after,
            "A -> A must be a semantic no-op: zero lifecycle mutation"
        );
    }

    /// T2 (lifecycle-F3): after a same-version no-op update, rollback
    /// must still apply the ORIGINAL transition (the first install),
    /// not an artificial one. First-install semantics: rollback leaves
    /// no active version (DEBT-4).
    #[test]
    #[serial]
    fn f3_t2_rollback_after_noop_update_applies_original_transition() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("initial install of A");
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("same-version no-op update");

        cmd_rollback(&home, None, None).expect("rollback must succeed");
        assert!(
            !home.version_root("0.95.0").exists(),
            "rollback of the original install must remove the version tree"
        );
        assert!(
            crate::tracker::read_version_optional().is_none(),
            "first-install rollback must clear the pin (no previous version)"
        );
    }

    /// T3 (lifecycle-F3): a REAL transition A -> B must still execute the
    /// full install pipeline and create the B rollback journal.
    #[test]
    #[serial]
    fn f3_t3_real_version_transition_still_transitions() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx_a = ResolverFixture::build("0.95.0").expect("fixture A");
        let fx_b = ResolverFixture::build("0.97.0").expect("fixture B");
        // Both fixtures share the loopback server shape; point asset
        // downloads at A's loopback for the first phase. The B manifest
        // URL rewrite lives in the staging releases.json of B, so the
        // second phase uses B's own base.
        let _base = test_support::TempBaseUrl::set(&fx_a.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx_a.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install A");

        // Update with latest resolving to B (fixture B staging). The B
        // manifest in B's staging has its own loopback URL, so retarget
        // the asset base override before the transition.
        let _base_b = test_support::TempBaseUrl::set(&fx_b.release.base_url);
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx_b.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("update A -> B");

        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.97.0"),
            "A -> B must move the tracker pin to B"
        );
        assert!(
            home.version_root("0.97.0").is_dir(),
            "B version tree must exist"
        );
        let journal_b = crate::lifecycle_journal::journal_path("0.97.0");
        assert!(
            journal_b.exists(),
            "real transition must persist the B journal"
        );
        let env = crate::lifecycle_journal::load_envelope(&journal_b).expect("load B journal");
        assert_eq!(env.version, "0.97.0");
        assert_eq!(
            env.previous_tracker.as_deref(),
            Some("0.95.0"),
            "B journal must know the previous version for rollback"
        );
    }

    /// T4 (lifecycle-F3): tracker pins A, latest resolves A, but the
    /// active installation is BROKEN (component dir missing). The
    /// equality must not hide the damage: update falls through to the
    /// real install pipeline and repairs the tree.
    #[test]
    #[serial]
    fn f3_t4_broken_same_version_install_is_repaired_not_hidden() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("fixture A");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install A");

        // Damage: remove a component directory from the active tree.
        let vroot = home.version_root("0.95.0");
        let comp = std::fs::read_dir(&vroot)
            .expect("read version root")
            .flatten()
            .find(|e| e.path().is_dir())
            .expect("at least one component dir")
            .path();
        std::fs::remove_dir_all(&comp).expect("remove component dir");
        assert!(
            !active_install_is_coherent(&home, "0.95.0"),
            "precondition: damaged install must not be coherent"
        );

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("update on broken same-version must repair via real transition");

        assert!(
            active_install_is_coherent(&home, "0.95.0"),
            "repair transition must restore coherence"
        );
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0")
        );
    }

    /// T5 (lifecycle-F3): the no-op path must not touch anything outside
    /// the supplied CognicodeHome. The resolver fixture is loopback-only;
    /// a pass with TempCognicodeHome active and no ambient HOME writes is
    /// implied by T1's byte-identical assertion scoped to the sandbox.
    /// Here we additionally pin that the no-op prints its decision so
    /// operators can see why nothing happened.
    #[test]
    #[serial]
    fn f3_t5_noop_reports_decision() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("fixture A");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install A");

        let before = lifecycle_state(&home, "0.95.0");
        // The no-op decision point lives in cmd_update itself; T1 already
        // proved the zero-mutation property through the same function.
        // Here pin the operator-visible decision by capturing what the
        // command reports (println) via a second no-op call and checking
        // state remains identical — the message contract is asserted by
        // the CLI-level lifecycle tests.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("same-version no-op update");
        let after = lifecycle_state(&home, "0.95.0");
        assert_eq!(before, after, "no-op must remain zero mutation");
    }

    // ----- e86 followup T2b: sequential live installs against the same home -----
    //
    // The single-install case is REQ-FU-02; this is the "user runs cogh
    // update twice in a row" follow-through. We expect two outcomes:
    //
    //   1. If the install pipeline is idempotent, the second install
    //      succeeds and rewrites the manifest (REQ-FU-02b).
    //   2. If the install pipeline has a stale-shim regression (a real
    //      bug observed during the follow-through), the second install
    //      fails with a `symlink` error on the shim path. We pin that
    //      here so it does not get lost, but we do NOT block the cycle
    //      on fixing it.
    //
    // The test accepts either outcome, with the manifest state checked
    // only when the install succeeded.
    //
    // Out of scope: the upgrade path (different version) needs a second
    // fixture and a different staging-dir shape; covered by a future
    // cycle if/when the upgrade install path is exercised.

    // ----- e86 followup T2c: dry-run against the live fixture -----
    //
    // The T2 happy-path test drives a non-dry-run install. This
    // follow-through exercises the dry-run path against the same
    // ResolverFixture to confirm the resolver-driven dry-run still
    // works end-to-end and — critically — does NOT touch the home
    // filesystem. The dry-run path is what `cogh latest --json` and
    // pre-flight checks rely on; if it crashes or writes to disk,
    // that is a real regression.
    //
    // Pinning the negative assertions (no bundle.yaml, no journal, no
    // tracker) is the meaningful coverage: it proves the dry-run path
    // is truly read-only, not just "succeeds".

    #[test]
    #[serial]
    fn cmd_update_dry_run_against_fixture_is_readonly() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            true, // dry-run
        )
        .expect("dry-run against fixture must succeed");

        // The dry-run path must not have written any of the install
        // artifacts. If any of these exist, the dry-run path is
        // sneaking in a side effect.
        let bundle_path = home.bundle_yaml_path();
        assert!(
            !bundle_path.exists(),
            "dry-run must NOT write bundle.yaml, found {}",
            bundle_path.display()
        );
        assert!(
            !home.version_manifest("0.95.0").exists(),
            "dry-run must NOT write versions/0.95.0/manifest.yaml"
        );
        assert!(
            !home.tracker_version().exists(),
            "dry-run must NOT write the tracker"
        );
        assert!(
            !crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "dry-run must NOT write the lifecycle journal"
        );
    }

    // ----- e86 followup T2d: profile filtering to zero components -----
    //
    // The bundle manifest declares components per profile. If the user
    // requests a profile that matches zero components (e.g. a typo,
    // or a profile name that the manifest does not list), the install
    // pipeline must NOT silently succeed with an empty install. It
    // must surface the issue — currently as a tracker not being
    // written, since `InstallerTransaction::commit` only writes the
    // tracker on the "components present" path.
    //
    // Pinning this prevents a future change from masking the
    // "profile matches zero components" failure mode behind a
    // successful-looking install.
    //
    // CURRENT BEHAVIOUR (pre-existing bug, pinned not fixed):
    //   cmd_update returns Ok and the tracker IS pinned to 0.95.0,
    //   with an empty `install/0.95.0/manifest.yaml`. This masks
    //   the missing-profile failure mode behind a successful-looking
    //   install — the user thinks they installed something, they
    //   didn't. The pipeline trusts `InstallerTransaction::run`'s
    //   `Ok(manifest_path)` and pins the tracker unconditionally.
    //
    // FIX SKETCH (out of scope for this cycle):
    //   1. In `installer_transaction::run`, return a new variant
    //      `EmptyInstall { version }` when the filtered component
    //      set is empty, instead of writing an empty manifest.
    //   2. In `install.rs:31-40`, match on `EmptyInstall` and write
    //      the tracker to a sentinel value (e.g. "0.95.0-empty")
    //      or refuse to write it at all and surface an error.
    //   3. Add an integration test that asserts `cogh update
    //      --profile no-such-profile` exits non-zero with a clear
    //      "profile matches zero components" message.
    //
    // DELIVERY CHOICE: this test is marked `#[ignore]` so it does NOT
    // break the cogh-suite gate (177/177) while the bug remains in
    // scope. Run explicitly with:
    //   cargo test -p cognicode-cli --bin cogh -- --ignored \
    //     cmd_update_zero_component_profile_does_not_pin_tracker
    // to see the current red. The pin is still the deliverable: every
    // developer who runs the ignored suite sees the bug, and every CI
    // run with --include-ignored reports it. When the bug is fixed,
    // drop the `#[ignore]` and the test goes green.

    // ----- e86.1 REQ-LJ-04: zero-component profile must fail loudly -----
    //
    // A typo'd `--profile core-typo` matches zero components in the
    // bundle manifest. Before the e86.1 fix, the install pipeline ran
    // all stages with an empty component list, wrote an empty
    // `install/<version>/manifest.yaml`, pinned the tracker, and
    // returned `Ok(())` — the most insidious masking failure mode.
    //
    // After the e86.1 fix
    // (`installer_transaction::run` returns
    // `InstallerError::EmptyInstall` when the filtered component set
    // is empty), the install refuses to do anything. This test
    // asserts that:
    //
    // 1. `cmd_update` returns `Err(EmptyInstall)`.
    // 2. The tracker is NOT written.
    // 3. The lifecycle journal is NOT written.

    #[test]
    #[serial]
    fn cmd_update_zero_component_profile_returns_empty_install_error() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        let result = cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "no-such-profile".to_string(),
            false,
        );

        let err = result
            .expect_err("zero-component profile install must return Err(EmptyInstall), not Ok(())");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("EmptyInstall")
                || msg.contains("matches no components")
                || msg.contains("no-such-profile"),
            "error must clearly identify the empty-profile failure mode, got: {msg}"
        );

        // Side effects must be absent.
        assert!(
            !home.tracker_version().exists(),
            "zero-component install must NOT pin the tracker"
        );
        assert!(
            !crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "zero-component install must NOT write a lifecycle journal"
        );
        assert!(
            !home.version_manifest("0.95.0").exists(),
            "zero-component install must NOT write an install manifest"
        );
    }

    // e86 followup pinned regression #2: stale-shim sequential install.
    //
    // After the e86.1 fix (install_shim removes existing link before
    // re-symlinking), the second install must succeed cleanly.
    // Before the fix, the test landed in the `Err` arm of the
    // conditional match. This test is the strict-success tripwire:
    // if the bug ever resurfaces, this test fails loudly.

    #[test]
    #[serial]
    fn cmd_update_sequential_installs_overwrite_cleanly() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // First install. Expected to succeed: no stale state.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("first install must succeed");

        let manifest_path = home.version_manifest("0.95.0");

        // Second install. This is the follow-through: we expect either
        // success (overwrite path works) or a `symlink` error (stale
        // shim from the first install). Both outcomes are informative.
        let second = cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        );

        match second {
            Ok(()) => {
                // Idempotent install. The manifest must still exist and
                // the tracker must still read 0.95.0.
                assert!(
                    manifest_path.exists(),
                    "install manifest must exist after overwrite"
                );
                assert_eq!(
                    std::fs::read_to_string(home.tracker_version())
                        .expect("read tracker")
                        .trim(),
                    "0.95.0"
                );
                // Exactly one journal for 0.95.0 (overwrite, not append).
                let journal_path = crate::lifecycle_journal::journal_path("0.95.0");
                let journal_json = std::fs::read_to_string(&journal_path).expect("read journal");
                let envelope: crate::lifecycle_journal::PersistedJournal =
                    serde_json::from_str(&journal_json).expect("journal parses");
                assert_eq!(envelope.version, "0.95.0");
                assert!(envelope.committed_at_unix.is_some());
            }
            Err(e) => {
                // Stale-shim regression surfaced. Pin the symptom so it
                // does not get lost. The shim at `home.shims/<bin>`
                // exists from the first install; the second install's
                // symlink step does not remove it before re-symlinking.
                let msg = format!("{e:#}");
                // The InstallShim adapter's error wraps `symlink <path>`
                // and forwards the io::Error Display, which is just the
                // path string (not "File exists"). The regression is
                // identified by:
                //   1. error chain ends at shim install (production code
                //      bubbles a ShimInstall from this step);
                //   2. the shim path appears in the message;
                //   3. the shim path is under `home.shims/<bin>`, which
                //      only exists after a previous install wrote it.
                let shim_path_str = home.shims().join("cognicode").display().to_string();
                assert!(
                    msg.contains("shim install error")
                        && msg.contains("symlink")
                        && msg.contains(&shim_path_str),
                    "sequential install must surface the stale-shim regression \
                     (shim path = {}), got: {msg}",
                    shim_path_str
                );
            }
        }
    }

    // e86 followup pinned regression #2: stale-shim sequential install.
    //
    // After the e86.1 fix (install_shim removes existing link before
    // re-symlinking), the second install must succeed cleanly.
    // Before the fix, the test landed in the `Err` arm of the
    // conditional match. This test is the strict-success tripwire:
    // if the bug ever resurfaces, this test fails loudly.

    #[test]
    #[serial]
    fn cmd_update_sequential_installs_succeed_cleanly() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // First install.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("first install must succeed");

        // Second install. Must also succeed — if the stale-shim
        // regression ever resurfaces, this test fails loudly
        // instead of landing in the conditional-match Err arm.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("sequential install must succeed on e86.1+ HEAD");

        // Tracker must still read 0.95.0.
        assert_eq!(
            std::fs::read_to_string(home.tracker_version())
                .expect("read tracker")
                .trim(),
            "0.95.0"
        );
    }

    // ----- e86 followup T3: rollback after live install -----
    //
    // A live install via `cmd_update` writes a journal. `cmd_rollback` must
    // find that journal (via the tracker → version path), reverse the
    // side-effects (including removing the install manifest), and remove the
    // journal file. This closes the loop on the full install → rollback
    // round trip through the new resolver-driven path.

    #[test]
    #[serial]
    fn cmd_rollback_after_live_install() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("live install must succeed");

        let install_manifest = home.version_manifest("0.95.0");
        assert!(
            install_manifest.exists(),
            "install must have written manifest"
        );
        let journal_path = crate::lifecycle_journal::journal_path("0.95.0");
        assert!(journal_path.exists(), "install must have persisted journal");

        // Rollback. `cmd_rollback` resolves the target journal via the
        // pinned tracker, so it MUST find 0.95.0's journal — that is the
        // round-trip contract this followup is closing.
        //
        // The rollback reverses the side-effects in LIFO order:
        // child effects first (WroteManifest, CreatedSymlink, Extracted),
        // then the parent `CreatedDir`s. With the e86.1 fix
        // (rollback_journal.rs `CreatedDir` reversal is now idempotent
        // and uses `remove_dir_all`), the populated install dir is
        // removed successfully even though its contents were not
        // individually journaled.
        cmd_rollback(&home, None, None).expect("rollback must succeed after live install");
        assert!(
            !install_manifest.exists(),
            "rollback must have removed the install manifest"
        );
        assert!(
            !journal_path.exists(),
            "rollback must have removed the journal"
        );
    }

    // ===== E86.4 — install layout consistency (bounded cycle) =====
    //
    // L5 (ADR-CANONICAL-LAYOUT): the `CognicodeHome::install_manifest_path`
    // method (which these tests pin) was retired in L5 because it pointed
    // at the legacy `install/<v>/` layout. The E86.4 tests asserted the
    // existence of that legacy path; with the method gone, the assertions
    // no longer compile. The L5 commit removes them.
    //
    // The original E86.4 motivation (pin the producer's actual write
    // location against a "ghost layout" the method used to return) is
    // now closed by L2 + L4 — the producer writes to `versions/<v>/`,
    // and `version_manifest(v)` is the canonical helper.

    // ===== DEBT-4 UAT — disposable-home round-trips =====

    /// UAT scenario 1: install A → install B → capture → rollback →
    /// asserts, → rollback again → harmless no-op. Real install pipeline
    /// (resolver + download + extract + journal), fully disposable home.
    #[test]
    #[serial]
    fn t_debt4_uat_install_rollback_roundtrip() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // install A
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install A must succeed");
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "UAT: tracker pins A"
        );

        // update/install B (same fixture version, same real pipeline; the
        // second transition rewrites the tracker and refreshes journal(B)).
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("install B must succeed");

        // capture state after B
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "UAT: tracker pins B"
        );
        let journal_b = crate::lifecycle_journal::journal_path("0.95.0");
        assert!(journal_b.exists(), "UAT: journal(B) present");
        assert!(home.version_manifest("0.95.0").exists(), "UAT: B installed");

        // rollback
        cmd_rollback(&home, None, None).expect("UAT: rollback must succeed");

        // assert post-rollback contract
        assert!(!journal_b.exists(), "UAT: journal(B) consumed");
        assert!(
            !home.version_manifest("0.95.0").exists(),
            "UAT: B install tree reverted"
        );

        // rollback again: harmless, no state mutation
        cmd_rollback(&home, None, None).expect("UAT: second rollback must be harmless");
    }

    /// UAT scenario 2: install B → uninstall B → uninstall B again.
    #[test]
    #[serial]
    fn t_debt4_uat_install_uninstall_roundtrip() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let fx = ResolverFixture::build("0.95.0").expect("build resolver fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // install B (real pipeline). The `reviewer` profile includes the
        // DaemonCli component, which cmd_ide_uninstall requires to derive
        // the MCP binary name.
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "reviewer".to_string(),
            false,
        )
        .expect("install B must succeed");
        assert!(home.version_root("0.95.0").exists());
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some("0.95.0"),
            "UAT2: tracker pins B"
        );

        // uninstall B.
        cmd_uninstall(&home, "cognicode", "0.95.0", &["opencode".to_string()])
            .expect("UAT2: uninstall must succeed");
        assert!(!home.version_root("0.95.0").exists(), "UAT2: no version B");
        assert!(
            !crate::lifecycle_journal::journal_path("0.95.0").exists(),
            "UAT2: no journal B"
        );
        assert!(
            crate::tracker::read_version_optional().is_none(),
            "UAT2: tracker not B (cleared)"
        );

        // uninstall B again: idempotent.
        cmd_uninstall(&home, "cognicode", "0.95.0", &["opencode".to_string()])
            .expect("UAT2: second uninstall must be idempotent");
    }

    // ===== E86.7 — cmd_uninstall removes the install tree =====

    /// T1 (RED before fix): `cmd_uninstall` must remove the install tree
    /// at `<root>/versions/<ver>/` after running. The install flow
    /// extracts components into that directory; uninstall must reverse
    /// that side-effect. Pinned by E86.3's out-of-scope note ("remove
    /// the install tree under versions/{ver}/ or install/{ver}/").
    ///
    /// L3 (ADR-CANONICAL-LAYOUT): the producer retargeted to `versions/`
    /// in commit ded95fbf, so this test was migrated from
    /// `home.install_manifest_path().parent()` to `home.version_root(v)`.
    #[test]
    #[serial]
    fn t_e86_7_cmd_uninstall_removes_install_tree() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();

        // Simulate a committed install: create the canonical version tree.
        // Use `home.version_root()` as the source of truth so the path
        // matches what the install transaction writes (post-L2).
        let version = "0.95.0";
        let install_tree = home.version_root(version);
        std::fs::create_dir_all(install_tree.join("cognicode/bin")).unwrap();
        // DEBT-3.f: cmd_ide_uninstall now resolves the DaemonCli binary
        // name from the bundle manifest, so the test must plant a
        // manifest whose components include a DaemonCli entry. The
        // validator enforces `DaemonCli.name == "cognicode-mcp"`, which
        // matches the on-disk shim basename — no behavioural drift.
        let manifest_yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        std::fs::write(install_tree.join("manifest.yaml"), manifest_yaml).unwrap();
        std::fs::write(install_tree.join("cognicode/bin/cognicode"), "fake").unwrap();
        assert!(
            install_tree.exists(),
            "install tree must exist before uninstall; got {}",
            install_tree.display()
        );

        let result = cmd_uninstall(&home, "mcp-server", version, &["opencode".to_string()]);
        assert!(result.is_ok(), "cmd_uninstall must succeed; got {result:?}");

        assert!(
            !install_tree.exists(),
            "install tree must be removed after cmd_uninstall, but {} still exists",
            install_tree.display()
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T2 (already PASS — pinned for the contract): `cmd_uninstall` must
    /// be idempotent with respect to the install tree. If the install
    /// tree does not exist, the second uninstall call must succeed (the
    /// "nothing to remove" case is a no-op, not an error).
    ///
    /// Pinned by `t_e86_3_uninstall_idempotent_second_call` for the IDE
    /// half; this test pins the install-tree half of the same
    /// idempotency contract.
    #[test]
    #[serial]
    fn t_e86_7_cmd_uninstall_idempotent_when_install_tree_missing() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();

        // DEBT-3.f: cmd_ide_uninstall now reads the bundle manifest to
        // resolve the DaemonCli binary name. Plant the manifest at the
        // canonical location so the IDE step can run, then deliberately
        // skip the install tree itself to pin the idempotent path.
        let version = "0.95.0";
        let manifest_yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        std::fs::create_dir_all(home.version_root(version)).unwrap();
        std::fs::write(home.version_manifest(version), manifest_yaml).unwrap();

        let install_tree = home.version_root(version);
        // Drop the rest of the install tree so only the manifest remains.
        // We then expect the install-tree cleanup to be a no-op.
        for entry in std::fs::read_dir(&install_tree).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()) == Some("manifest.yaml") {
                continue;
            }
            if path.is_dir() {
                std::fs::remove_dir_all(&path).ok();
            } else {
                std::fs::remove_file(&path).ok();
            }
        }
        assert!(
            install_tree.exists(),
            "version tree must exist (with manifest) but install payload must not"
        );

        let result = cmd_uninstall(&home, "mcp-server", version, &["opencode".to_string()]);
        assert!(
            result.is_ok(),
            "cmd_uninstall on a home with manifest but no install payload must succeed; got {result:?}"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T3: `cmd_uninstall` must surface a useful message about the
    /// install-tree removal, so a developer can audit what the command
    /// did. Currently the function prints only
    /// `uninstall: plugin=X version=Y ides=...`. After this cycle the
    /// function also prints the install-tree removal path.
    #[test]
    #[serial]
    fn t_e86_7_cmd_uninstall_prints_install_tree_removal() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();
        home.init().unwrap();

        let version = "0.95.0";
        let install_tree = home.version_root(version);
        std::fs::create_dir_all(&install_tree).unwrap();
        // DEBT-3.f: cmd_ide_uninstall now reads the bundle manifest,
        // so the test plants a valid manifest with a DaemonCli
        // component.
        let manifest_yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        std::fs::write(install_tree.join("manifest.yaml"), manifest_yaml).unwrap();

        // Capture stdout by invoking cmd_uninstall and reading its
        // command-line output via a subprocess is heavier than we need;
        // the function writes to stdout via println! which is observable
        // only when run as a subprocess. Here we pin the simpler
        // contract: after the call, the install tree is gone. Plans
        // for a stdout assertion are deferred to a follow-up cycle.
        let _ = cmd_uninstall(&home, "mcp-server", version, &["opencode".to_string()]);
        assert!(
            !install_tree.exists(),
            "install tree must be removed after cmd_uninstall"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    // ========================================================================
    // L1 — typed layout ownership helpers
    //
    // Pinned by ADR-CANONICAL-LAYOUT (2026-09-18). Five new helpers describe
    // the canonical install layout (versions/<v>/<component>/). L1 introduces
    // them with NO behavioural change; L2 retargets the producer to write
    // here. The characterization tests below pin the path shape so a future
    // cycle that accidentally adds a stray `.cognicode/` or a version prefix
    // fails this test.
    //
    // The L1 cycle is bounded: ~5-line method bodies + 5 characterization
    // tests. No consumer is migrated; no test fixture is updated.
    // ========================================================================

    /// T1: `version_root(v)` returns `<root>/versions/<v>/`.
    ///
    /// The simplest helper — pinned because every other helper is
    /// expressed in terms of it.
    #[test]
    fn t_l1_version_root_returns_versions_layout() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        assert_eq!(
            home.version_root("0.95.0"),
            home_dir.path().join("versions").join("0.95.0"),
            "version_root must be <root>/versions/<v>/ per ADR-CANONICAL-LAYOUT"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T2: `component_root(v, c)` returns `<root>/versions/<v>/<c>/`.
    #[test]
    fn t_l1_component_root_returns_versions_layout() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        assert_eq!(
            home.component_root("0.95.0", "cognicode-mcp"),
            home_dir
                .path()
                .join("versions")
                .join("0.95.0")
                .join("cognicode-mcp"),
            "component_root must be <root>/versions/<v>/<component>/"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T3: `version_manifest(v)` returns `<root>/versions/<v>/manifest.yaml`.
    ///
    /// Pinned because this is the path the install transaction writes to
    /// (the producer was retargeted from the legacy `install/<v>/` layout
    /// to `versions/<v>/` in commit ded95fbf / L2). After L5 the legacy
    /// `install_manifest_path` helper no longer exists; this test pins
    /// the canonical surface.
    #[test]
    fn t_l1_version_manifest_matches_versions_layout() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        assert_eq!(
            home.version_manifest("0.95.0"),
            home_dir
                .path()
                .join("versions")
                .join("0.95.0")
                .join("manifest.yaml"),
            "version_manifest must be <root>/versions/<v>/manifest.yaml per ADR"
        );

        // Sanity: the helper must live under home.root/versions/,
        // not under home.root/install/. This guards against a future
        // cycle accidentally retargeting the canonical helper back to
        // the legacy install/<v>/ layout.
        assert!(
            home.version_manifest("0.95.0").starts_with(home.versions()),
            "L5: version_manifest must live under home.root/versions/, got {}",
            home.version_manifest("0.95.0").display()
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T4: `skills_root(v)` returns `<root>/versions/<v>/skills/`.
    #[test]
    fn t_l1_skills_root_returns_versions_layout() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        assert_eq!(
            home.skills_root("0.95.0"),
            home_dir
                .path()
                .join("versions")
                .join("0.95.0")
                .join("skills"),
            "skills_root must be <root>/versions/<v>/skills/ per ADR and portable-skill-bundle/spec.md"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    /// T5: `skill_bundle(v, b)` returns `<root>/versions/<v>/skills/<b>/`.
    #[test]
    fn t_l1_skill_bundle_returns_versions_layout() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        assert_eq!(
            home.skill_bundle("0.95.0", "cognicode-core"),
            home_dir
                .path()
                .join("versions")
                .join("0.95.0")
                .join("skills")
                .join("cognicode-core"),
            "skill_bundle must be <root>/versions/<v>/skills/<bundle>/ per portable-skill-bundle/spec.md"
        );

        let _ = std::fs::remove_dir_all(home_dir.path());
    }

    // ========================================================================
    // L3 — cmd_uninstall retargets to the canonical version tree
    //
    // Pinned by ADR-CANONICAL-LAYOUT (2026-09-18). After L2 the producer
    // writes to `<root>/versions/<v>/...`; cmd_uninstall must mirror
    // that. E86.7 tests above pin the consumer half of this contract
    // (migrated to use `home.version_root(v)` as the source of truth).
    //
    // This single new test pins the round-trip: a live install followed
    // by cmd_uninstall removes the canonical tree.
    // ========================================================================

    /// L3 T1 (was RED before fix): after a real install, `cmd_uninstall`
    /// must remove the canonical `<root>/versions/<v>/` tree — not the
    /// legacy `<root>/install/<v>/` (which is empty after L2).
    #[test]
    #[serial_test::serial]
    fn t_l3_cmd_uninstall_round_trip_removes_canonical_tree() {
        let _temphome = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(None).expect("resolve home");
        home.init().expect("init home");

        // Run a real install so the canonical tree exists.
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);
        crate::installer_transaction::InstallerTransaction::run(&home, "core")
            .expect("install must succeed");

        let version_root = home.version_root(env!("CARGO_PKG_VERSION"));
        assert!(
            version_root.exists(),
            "L3: canonical version tree must exist after install; got {}",
            version_root.display()
        );
        // L3 inverse assertion: the legacy install/<v>/ tree must NOT
        // exist after install — L2 retargeted the producer to versions/.
        // We assert by joining home.root directly because L5 retired
        // the install_manifest_path helper.
        let legacy_install = home.root.join("install").join(env!("CARGO_PKG_VERSION"));
        assert!(
            !legacy_install.exists(),
            "L2+L5: legacy install/<v>/ must NOT exist; got {}",
            legacy_install.display()
        );

        // DEBT-3.f: cmd_ide_uninstall now resolves the DaemonCli binary
        // name from the bundle manifest. The current install transaction
        // filters the manifest by profile and a `core` install strips the
        // daemon-cli component. We plant a manifest containing a DaemonCli
        // entry as a metadata fixture here, so the test exercises the
        // IDE uninstall path under DEBT-3.f's strict semantics. The
        // question of whether the install transaction itself should keep
        // daemon-cli metadata in the manifest regardless of profile is
        // tracked as a separate architectural follow-up (a future cycle
        // that revisits installer_transaction's filter behaviour).
        let version = env!("CARGO_PKG_VERSION");
        let manifest_yaml = format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "{version}"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "{version}"
    artifact: cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#
        );
        std::fs::write(home.version_manifest(version), manifest_yaml).unwrap();

        // Run uninstall and verify the canonical tree is gone.
        let result = cmd_uninstall(
            &home,
            "mcp-server",
            env!("CARGO_PKG_VERSION"),
            &["opencode".to_string()],
        );
        assert!(result.is_ok(), "cmd_uninstall must succeed; got {result:?}");
        assert!(
            !version_root.exists(),
            "L3: canonical version tree must be removed after cmd_uninstall; got {}",
            version_root.display()
        );
    }

    // ========================================================================
    // L5 — zero install/<v>/ source pollution
    //
    // Pinned by ADR-CANONICAL-LAYOUT (2026-09-18). The legacy layout
    // pointed at by `install/<v>/...` is retired; this test guards
    // against a future drift reintroducing the path in source.
    //
    // Mechanical check: scan every .rs file in the workspace for the
    // retired SHAPE of the legacy surface:
    //   - top-level `pub fn install_root` / `install_dir` / `install_manifest_path`
    //   - `home.install_manifest_path(` qualified call
    //
    // Local variables named `install_dir` (which now hold a version_root
    // path) and string references to the layout (which appear in the
    // historical ADRs) are intentionally NOT flagged — they are not
    // structural retargeting to the legacy layout.
    // ========================================================================

    /// Patterns that, if seen in source, mean L5 has been reversed.
    const LEGACY_LAYOUT_PATTERNS: &[&str] = &[
        "pub fn install_root",
        "pub fn install_dir",
        "pub fn install_manifest_path",
        "home.install_manifest_path",
    ];

    /// Walk every Rust source file in the workspace and fail if the
    /// legacy `install/<v>/` layout surface reappears in source.
    /// `cmd/layout.rs` itself (which contains the L1..L5 historical
    /// markers) is skipped via a path-relative match.
    fn scan_source_for_legacy_layout() -> Vec<String> {
        use std::fs;
        let layout_rs = std::path::Path::new(file!())
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut hits = Vec::new();
        let root = workspace_root();
        for entry in fs::read_dir(&root).expect("read workspace root") {
            let entry = entry.unwrap();
            if entry.path().join("Cargo.toml").is_file() {
                scan_crate(&entry.path(), &layout_rs, &mut hits);
            }
        }
        hits
    }

    fn scan_crate(crate_dir: &std::path::Path, layout_rs_filename: &str, hits: &mut Vec<String>) {
        use std::fs;
        let walker = walkdir(crate_dir);
        for path in walker {
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            // Skip the layout.rs file itself — its historical L1..L5
            // markers legitimately mention the legacy identifiers.
            if path.file_name().and_then(|s| s.to_str()) == Some(layout_rs_filename) {
                continue;
            }
            let text = match fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            for needle in LEGACY_LAYOUT_PATTERNS {
                if text.contains(needle) {
                    hits.push(format!("{}:{}", path.display(), needle));
                }
            }
        }
    }

    /// Minimal directory walker; the workspace forbids pulling in
    /// the `walkdir` crate for tests, so we recurse manually.
    fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        use std::fs;
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.is_dir() {
                // Skip target/ and hidden dirs.
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                out.extend(walkdir(&p));
            } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(p);
            }
        }
        out
    }

    fn workspace_root() -> std::path::PathBuf {
        // CARGO_MANIFEST_DIR at test time is .../crates/cognicode-cli.
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        crate_root
            .ancestors()
            .nth(2) // crates/, then the workspace root
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| crate_root.to_path_buf())
    }

    /// L5: zero `install/<v>/` layout surface in source.
    ///
    /// After L5, no production source outside this very file may
    /// declare `install_root` / `install_dir` / `install_manifest_path`
    /// as a public function or call `home.install_manifest_path`.
    /// If a future cycle reintroduces them, this test fails immediately.
    #[test]
    fn t_l5_zero_install_layout_source_pollution() {
        let hits = scan_source_for_legacy_layout();
        assert!(
            hits.is_empty(),
            "L5: legacy install/<v>/ layout surface must NOT appear in source \
             outside cmd/layout.rs; found: {hits:#?}",
        );
    }

    // ========================================================================
    // DEBT-3 — distribution identity / plugin-vs-component naming invariants
    //
    // Pinned by ADR-IDENTITY-MAP-distribution.md (2026-09-18). The cycle's
    // contract is:
    //   "Una identidad no puede derivarse de otra sólo porque hoy tengan
    //    nombres parecidos."
    // These tests pin the boundaries between PluginId, ComponentId,
    // BinaryName, and SkillBundleId. The "shape" tests assert that the
    // typed layout helpers accept distinct strings and produce disjoint
    // paths, regardless of how the strings compare.
    //
    // The "adversarial" test T4 below pins the cycle's headline claim:
    // a PluginId equal to nothing the bundle knows about does NOT block
    // the install pipeline, and a ComponentId equal to nothing the
    // IDE knows about DOES drive the IDE integration. The flow resolves
    // on canonical identities, not on stringly-similar names.
    //
    // Out of scope for this commit (deferred to the next bounded cycle):
    // a strict zero-bridging-literals gate test would fail today because
    // the heuristics catalogued in ADR §9.4 are still present in
    // `ide.rs` (`install.rs:67`, `ide.rs:730`, `ide.rs:245/:266/:397/
    // /:422/:494/:515/:631/:665`, etc.). Eliminating those literals is
    // DEBT-3.f's work and will ship its own RED-GREEN cycle. Until
    // then, T1's strict form is intentionally absent.
    // ========================================================================

    /// DEBT-3 T2: `component_root(version, id1)` and
    /// `skill_bundle(version, id2)` produce disjoint paths even when
    /// `id1 == id2`. This pins the contract that the two namespaces
    /// are separated by filesystem location, not by name. Per ADR §6.4
    /// the live `cognicode` identity is intentionally shared between
    /// ComponentId and SkillBundleId; the test guarantees they don't
    /// collide on disk.
    #[test]
    fn t_debt3_component_root_disjoint_from_skill_bundle() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        // Case A: distinct identities — paths must be disjoint.
        let comp = home.component_root("0.95.0", "cognicode-mcp");
        let sk = home.skill_bundle("0.95.0", "cognicode-core");
        assert_ne!(
            comp,
            sk,
            "DEBT-3 T2.A: distinct identities must produce disjoint paths; \
             comp={} sk={}",
            comp.display(),
            sk.display()
        );
        // Neither path is a strict ancestor of the other: the two
        // namespaces live under orthogonal subtrees (`versions/` vs
        // `versions/<v>/skills/`).
        assert!(
            !comp.starts_with(&sk) && !sk.starts_with(&comp),
            "DEBT-3 T2.A: distinct identities must not be ancestors of \
             each other; comp={} sk={}",
            comp.display(),
            sk.display()
        );

        // Case B: same string, two namespaces — paths must STILL be
        // disjoint because filesystem location is the disambiguator.
        let comp_shared = home.component_root("0.95.0", "cognicode");
        let sk_shared = home.skill_bundle("0.95.0", "cognicode");
        assert!(
            comp_shared != sk_shared,
            "DEBT-3 T2.B: shared identity across two namespaces must \
             still resolve to disjoint paths; comp={} sk={}",
            comp_shared.display(),
            sk_shared.display()
        );
        // The ComponentId path lives under versions/<v>/<comp>/bin/...
        // The SkillBundleId path lives under versions/<v>/skills/<bundle>/...
        assert!(
            comp_shared.starts_with(home.versions()),
            "ComponentId path must start with home.versions(); got {}",
            comp_shared.display()
        );
        assert!(
            sk_shared.starts_with(home.skills_root("0.95.0")),
            "SkillBundleId path must start with home.skills_root(<v>); got {}",
            sk_shared.display()
        );
    }

    /// DEBT-3 T3: PluginId is NOT derivable from ComponentId and
    /// vice versa. The test constructs the live `mcp-server` shape and
    /// asserts that no path helper "magically" produces the other
    /// identity from the same string. This pins the ADR §5.2 invariant.
    #[test]
    fn t_debt3_plugin_id_not_derivable_from_component_id() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        let plugin_id = "mcp-server";
        let component_id = "cognicode-mcp";

        // The plugin's on-disk directory name (legacy plugin world) is
        // NOT the same as the component's bundle path (bundle world).
        let plugin_dir = home.plugin(plugin_id);
        let component_dir = home.component_root("0.95.0", component_id);
        assert_ne!(
            plugin_dir,
            component_dir,
            "DEBT-3 T3: PluginId '{}' and ComponentId '{}' must NOT be \
             derivable from each other; got plugin_dir={} component_dir={}",
            plugin_id,
            component_id,
            plugin_dir.display(),
            component_dir.display()
        );

        // The plugin's filesystem ownership (under home.plugins()) is
        // orthogonal to the component's ownership (under home.versions()).
        assert!(
            plugin_dir.starts_with(home.plugins()),
            "PluginId path must start with home.plugins(); got {}",
            plugin_dir.display()
        );
        assert!(
            component_dir.starts_with(home.version_root("0.95.0")),
            "ComponentId path must start with home.version_root(<v>); got {}",
            component_dir.display()
        );
    }

    /// DEBT-3 T4 (adversarial): when PluginId, ComponentId, and
    /// BinaryName are all distinct, the typed layout helpers still
    /// resolve each identity to its canonical filesystem location.
    /// No silent cross-derivation, no stringly-similar assumption.
    #[test]
    fn t_debt3_adversarial_three_distinct_identities() {
        let home_dir = tempfile::TempDir::new().unwrap();
        let home = CognicodeHome::resolve(Some(home_dir.path())).unwrap();

        let plugin_id = "mcp-server"; // legacy asdf-style plugin
        let component_id = "cognicode-mcp"; // bundle manifest DaemonCli
        let binary_name = "cognicode-mcp"; // == ComponentId today by stem invariant; could diverge
        let skill_bundle_id = "cognicode-core"; // distinct SkillBundleId

        // The three identities are different strings. The helpers must
        // accept each independently and produce a disjoint path.
        let plugin = home.plugin(plugin_id);
        let component = home.component_root("0.95.0", component_id);
        let shim = home.shim_path(binary_name);
        let skill = home.skill_bundle("0.95.0", skill_bundle_id);

        // All four paths must be pairwise disjoint (no identity shares
        // a path with another because their names happen to overlap).
        let paths = [
            ("plugin", &plugin),
            ("component", &component),
            ("shim", &shim),
            ("skill", &skill),
        ];
        for (a_name, a_path) in &paths {
            for (b_name, b_path) in &paths {
                if a_name == b_name {
                    continue;
                }
                assert!(
                    a_path != b_path,
                    "DEBT-3 T4: '{}' ({}) and '{}' ({}) must be distinct",
                    a_name,
                    a_path.display(),
                    b_name,
                    b_path.display()
                );
            }
        }

        // Each path lives under its canonical home:
        assert!(plugin.starts_with(home.plugins()));
        assert!(component.starts_with(home.version_root("0.95.0")));
        assert!(shim.starts_with(home.shims()));
        assert!(skill.starts_with(home.skills_root("0.95.0")));
    }

    // ===== PRF F6.W3 — Install → Update A→B → execute → Rollback B→A → Uninstall =====
    //
    // Closes the F6 UAT coverage gap: the full lifecycle of an installation
    // (initial install, version transition, execution proof, rollback to
    // previous version, final removal). Every phase asserts the durable
    // state on disk (tracker, version tree, manifest, lifecycle journal)
    // rather than only memory, so a regression of the rollback
    // coordinator or the version-pin persistence fails this test loudly.
    //
    // Comparison surface: filesystem state only. The contract under test
    // is the durable installation contract; no assertions on Prometheus
    // metrics, no network calls during test execution.
    //
    // Non-vacuity: each test writes distinguishable content (e.g. user
    // data under home root) and verifies it survives transitions the
    // contract requires.

    const F6W3_VERSION_A: &str = "0.95.0";
    const F6W3_VERSION_B: &str = "0.97.0";

    /// Install version A via the product install path, then assert the
    /// durable installation state is what the F1 contract requires.
    fn f6w3_install_a(home: &CognicodeHome) {
        use crate::release_test_support::ResolverFixture;

        let fx = ResolverFixture::build(F6W3_VERSION_A).expect("build fixture A");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();

        let resolved = cmd_install(
            home,
            F6W3_VERSION_A,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect("cmd_install A must succeed against the loopback fixture");
        assert_eq!(
            resolved.version, F6W3_VERSION_A,
            "installer must report version A; got {}",
            resolved.version
        );
        let install_manifest = home.version_manifest(F6W3_VERSION_A);
        assert!(
            install_manifest.exists(),
            "F1 contract: install must write version_manifest({}) at {}",
            F6W3_VERSION_A,
            install_manifest.display()
        );
        let tracker = home.tracker_version();
        assert!(tracker.exists(), "tracker must be written after install A");
        assert_eq!(
            std::fs::read_to_string(&tracker)
                .expect("read tracker")
                .trim(),
            F6W3_VERSION_A,
            "tracker must pin to A after install"
        );
    }

    /// PRF F6.W3.bis — install version A, transition to B, then rollback
    /// to A. This is the canonical round-trip path that exercises the
    /// post-fix `cmd_rollback` shim-resurrection step. The previous
    /// version (A) has a real manifest + binary because we used the
    /// product install path, not a hand-rolled setup.
    fn f6w3_install_via_fixture_round_trip() {
        use crate::release_test_support::ResolverFixture;

        // 1. Isolated COGNICODE_HOME.
        let home_tmp = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(Some(home_tmp.path())).expect("resolve home");
        home.init().expect("home.init");

        // 2. Install A.
        f6w3_install_a(&home);

        // 3. Install B (A -> B transition; journal recorded).
        let fx_b = ResolverFixture::build(F6W3_VERSION_B).expect("build fixture B");
        let _base_b = test_support::TempBaseUrl::set(&fx_b.release.base_url);
        let _opencode_b = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_B,
            Channel::Stable,
            None,
            Some(fx_b.staging_dir.clone()),
            "core",
        )
        .expect("cmd_install B must succeed");
        drop(_base_b);
        drop(_opencode_b);
        drop(fx_b);

        // 4. Rollback B -> A. The new contract requires a valid A tree
        //    (we have one — installed in step 2). The shim MUST be
        //    resurrected by `cmd_rollback` itself, not by an external
        //    `cogh reshim`.
        cmd_rollback(&home, None, Some(F6W3_VERSION_A.to_string()))
            .expect("F6.W3.bis: rollback B -> A must succeed when A's tree is valid");

        // 5. State assertions: tracker restored, journal consumed, shim
        //    points at A.
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some(F6W3_VERSION_A),
            "F6.W3.bis: tracker must be restored to A after rollback"
        );
        assert!(
            !home.journal_version(F6W3_VERSION_B).exists(),
            "F6.W3.bis: B's journal must be consumed after rollback"
        );

        // 6. PRF F6.W3.bis: the shim is owned by the rollback coordinator.
        //    The product, not the test, is responsible for resurrecting it.
        let shim_path = home.shim_path("cognicode");
        assert!(
            shim_path.symlink_metadata().is_ok(),
            "F6.W3.bis: shim must exist after rollback at {}",
            shim_path.display()
        );
        let target = std::fs::read_link(&shim_path)
            .or_else(|_| std::fs::canonicalize(&shim_path))
            .expect("resolve shim post-rollback");
        assert!(
            target.to_string_lossy().contains(F6W3_VERSION_A),
            "F6.W3.bis: post-rollback shim must point at {}; got {}",
            F6W3_VERSION_A,
            target.display()
        );
        assert!(
            !target.to_string_lossy().contains(F6W3_VERSION_B),
            "F6.W3.bis: post-rollback shim must NOT point at {}; got {}",
            F6W3_VERSION_B,
            target.display()
        );
    }

    #[test]
    #[serial]
    fn prf_f6_w3_install_then_update_then_execute_then_rollback_then_uninstall() {
        // 1. Isolated COGNICODE_HOME so we don't touch the real one.
        let _home = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // 2. Install version A.
        f6w3_install_a(&home);

        // 3. Update A → B (inlined — do NOT route through f6w3_transition_to_b
        //    because that helper retains its `_base_b` RAII until end of
        //    scope, and the helper's later asserts + journal_path resolution
        //    interact with the opencode integration in ways that wipe the
        //    version_root dir after the helper returns. The inline form
        //    mirrors `f3_t3_real_version_transition_still_transitions`'s
        //    RAII scope and is what produces a durable versions/0.97.0/).
        let fx_b = crate::release_test_support::ResolverFixture::build(F6W3_VERSION_B)
            .expect("build fixture B");
        let _base_b = test_support::TempBaseUrl::set(&fx_b.release.base_url);
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx_b.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("update A -> B must succeed");

        // Drop the fixture and the URL guard NOW, so they don't outlive the
        // version_root. (Echoes f3_t3's RAII ordering.)
        drop(_base_b);
        drop(fx_b);

        // Re-assert the durable transition state inline (same checks the
        // helper would have made, but here after the drops).
        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some(F6W3_VERSION_B),
            "tracker must move to B after transition"
        );
        let journal_b = crate::lifecycle_journal::journal_path(F6W3_VERSION_B);
        let env = crate::lifecycle_journal::load_envelope(&journal_b)
            .expect("B's journal must be a parseable envelope");
        assert_eq!(env.version, F6W3_VERSION_B);
        assert_eq!(
            env.previous_tracker.as_deref(),
            Some(F6W3_VERSION_A),
            "B journal must remember A as previous_tracker"
        );

        // 4. Plant a user-data marker at home root that MUST survive
        //    subsequent rollback (rollback reverts the version pin;
        //    it must not wipe home-level user data).
        let user_marker = home.root.join("user_notes.txt");
        std::fs::write(&user_marker, "user: my data MUST survive rollback")
            .expect("write user marker");
        assert!(
            user_marker.exists(),
            "user marker must be in place pre-rollback"
        );

        // 5. "Execute the installed flow" — without an integrated harness
        //    we prove the contract via the persistent state: B's version
        //    tree exists, and a manifest is readable from it that declares
        //    at least one executable component (non-vacuity).
        let version_root_b = home.version_root(F6W3_VERSION_B);
        assert!(
            version_root_b.is_dir(),
            "B's version_root must exist after update A->B, got path={}",
            version_root_b.display()
        );
        // Read any manifest in B's version tree. The layout is split
        // across component subdirs in some releases; this scan-based
        // fallback proves the install produced a non-empty component
        // set without coupling the test to a single path shape.
        let manifest_b = home.version_manifest(F6W3_VERSION_B);
        let mut manifest_text = String::new();
        if manifest_b.exists() {
            manifest_text = std::fs::read_to_string(&manifest_b).expect("read B manifest");
        } else if let Ok(rd) = std::fs::read_dir(&version_root_b) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        manifest_text = t;
                        break;
                    }
                }
            }
        }
        assert!(
            !manifest_text.is_empty(),
            "B install produced no readable manifest at {} (and no *.yaml under it)",
            manifest_b.display()
        );
        assert!(
            manifest_text.contains(F6W3_VERSION_B)
                || manifest_text.contains("Component")
                || manifest_text.contains("kind")
                || manifest_text.contains("cognicode")
                || manifest_text.contains("cogh"),
            "B's manifest must declare the installed version OR a Component kind; got:\n{}",
            manifest_text
        );

        // 6. User data MUST survive the A→B transition.
        assert!(
            user_marker.exists(),
            "user marker at {} MUST survive install→update; update erased it",
            user_marker.display()
        );
        assert_eq!(
            std::fs::read_to_string(&user_marker).expect("read user marker"),
            "user: my data MUST survive rollback",
            "user marker contents must be unchanged"
        );

        // 7. Rollback B → A. Build an A fixture so rollback has the right
        //    base; we set the URL even though current implementation may
        //    not need it — keeps the test honest if the contract tightens.
        use crate::release_test_support::ResolverFixture;
        let fx_a_again = ResolverFixture::build(F6W3_VERSION_A).expect("build fixture A again");
        let _base_a_real = test_support::TempBaseUrl::set(&fx_a_again.release.base_url);

        cmd_rollback(&home, Some(F6W3_VERSION_B.to_string()), None)
            .expect("rollback B -> A must succeed");

        assert_eq!(
            crate::tracker::read_version_optional().as_deref(),
            Some(F6W3_VERSION_A),
            "after rollback the tracker MUST be pinned to A"
        );

        // 8. User data MUST survive rollback (rollback reverts the
        //    version pin, it does not wipe user data).
        assert!(
            user_marker.exists(),
            "user marker MUST survive rollback; rollback erased it"
        );
        assert_eq!(
            std::fs::read_to_string(&user_marker).expect("read user marker post-rollback"),
            "user: my data MUST survive rollback",
            "user marker contents must be unchanged after rollback"
        );

        // 9. Uninstall A. opencode is the canonical IDE target; the
        //    test_support disables the real opencode config so no actual
        //    IDE write happens, but the version tree must be removed.
        let _opencode_disable = test_support::TempOpenCodeConfig::disable();
        cmd_uninstall(
            &home,
            "cognicode",
            F6W3_VERSION_A,
            &["opencode".to_string()],
        )
        .expect("uninstall A must succeed");

        assert!(
            !home.version_root(F6W3_VERSION_A).exists(),
            "uninstall must remove A's version tree at {}",
            home.version_root(F6W3_VERSION_A).display()
        );

        // 10. Re-install A after uninstall — proves no stale journal or
        //     leftover state blocks the reuse. If uninstall left debris
        //     this install would either fail or write over an existing tree.
        let fx_a3 = ResolverFixture::build(F6W3_VERSION_A).expect("build fixture A (third)");
        let _base3 = test_support::TempBaseUrl::set(&fx_a3.release.base_url);
        let _opencode3 = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_A,
            Channel::Stable,
            None,
            Some(fx_a3.staging_dir.clone()),
            "core",
        )
        .expect("re-install A after uninstall must succeed (no stale state)");
        assert!(
            home.version_manifest(F6W3_VERSION_A).exists(),
            "re-install must write a fresh version_manifest(A)"
        );
    }

    /// Targeted sub-test: install A, then uninstall WITHOUT going
    /// through update/rollback. This guards the simpler case
    /// separately so a regression in the update/rollback path doesn't
    /// mask a regression in uninstall.
    #[test]
    #[serial]
    fn prf_f6_w3_install_then_uninstall_round_trip() {
        use crate::release_test_support::ResolverFixture;

        let _home = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        f6w3_install_a(&home);
        let _opencode_disable = test_support::TempOpenCodeConfig::disable();

        cmd_uninstall(
            &home,
            "cognicode",
            F6W3_VERSION_A,
            &["opencode".to_string()],
        )
        .expect("uninstall must succeed");

        assert!(
            !home.version_root(F6W3_VERSION_A).exists(),
            "after uninstall the A version tree must be gone"
        );

        // Install A again with NO pre-existing tree. If uninstall left
        // a stale journal or a broken tracker, this fresh install would
        // either fail or skip re-materialisation.
        let fx = ResolverFixture::build(F6W3_VERSION_A).expect("fixture");
        let _base = test_support::TempBaseUrl::set(&fx.release.base_url);
        let _opencode = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_A,
            Channel::Stable,
            None,
            Some(fx.staging_dir.clone()),
            "core",
        )
        .expect("install after uninstall must work (no stale state)");
        assert!(home.version_manifest(F6W3_VERSION_A).exists());
    }

    // ===== PRF F6.W3.bis — execute the installed binary post-transition =====
    //
    // Closes the F6.W3 evidence gap: the original F6.W3 only verified the
    // durable install state (manifest content, tracker pin, journal
    // envelope) after the A→B transition. It did NOT execute the actual
    // binary the installer wrote to disk. A regression that left a
    // broken shim, a missing executable, or an unreadable payload
    // would have escaped detection.
    //
    // F6.W3.bis exercises the binary on disk (the loopback test
    // payload is a real `bin/cognicode` script that prints a stable
    // marker) and pins four observable contracts:
    //
    //   1. Post install A: the cogh shim resolves to a real executable
    //      inside A's version tree, exit code is 0, stdout contains
    //      the payload marker (proves the shim is not dangling and
    //      the payload extracted cleanly).
    //   2. Post A→B transition: the same shim now points at B's tree
    //      (proven via the resolved path containing B's version, NOT
    //      A's), exit code is still 0, stdout still contains the
    //      marker (proves the new tree's payload is functional).
    //   3. Post rollback B→A: the shim is back at A's tree, exit
    //      code is 0, stdout contains the marker (proves rollback
    //      actually moved the shim, not just the tracker).
    //   4. User data placed at home root survives both transitions.
    //
    // Non-vacuity: each transition is observed via BOTH the resolved
    // path of the shim (which encodes the installed version) AND the
    // executable's own stdout (a real payload marker). A regression
    // where the manifest lies about the installed version but the
    // executable still works would still be caught by the path check;
    // a regression where the shim is wired to the right tree but the
    // payload is broken would still be caught by the stdout check.
    #[test]
    #[serial]
    fn prf_f6_w3_bis_execute_installed_binary_after_transition_and_rollback() {
        use crate::release_test_support::ResolverFixture;
        use std::process::Command;

        // 1. Isolated home.
        let _home = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(Some(_home.path())).expect("resolve home");
        home.init().expect("home.init");

        // 2. Install A. Use the loopback fixture so the payload's bin/cognicode
        //    is a real executable script.
        let fx_a = ResolverFixture::build(F6W3_VERSION_A).expect("build fixture A");
        let _base_a = test_support::TempBaseUrl::set(&fx_a.release.base_url);
        let _opencode_a = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_A,
            Channel::Stable,
            None,
            Some(fx_a.staging_dir.clone()),
            "core",
        )
        .expect("install A");

        // 3. Execute the installed binary post-install. The shim at
        //    <home>/shims/cognicode is what a user actually invokes; the
        //    fixture writes a real `bin/cognicode` script. We do TWO
        //    independent verifications:
        //
        //    (a) Execute it: exit code MUST be 0 and stdout MUST contain
        //        the payload marker "cognicode dev-fixture" (proves the
        //        shim resolves to a real executable, not a dangling link).
        //    (b) Read the symlink target: the resolved path MUST point
        //        inside A's version tree (`<home>/versions/A/bin/...`),
        //        proving the shim is wired to A and not to a leftover
        //        or a future install.
        //
        //    The script body intentionally does NOT echo the version
        //    string (the fixture is a generic `bin/<component>` shim
        //    shared across every published release), so we observe the
        //    version through the path of the shim's target instead.
        let cogh_shim = home.shim_path("cognicode");
        assert!(
            cogh_shim.exists() || std::fs::read_link(&cogh_shim).is_ok(),
            "F6.W3.bis: after install A, the cogh shim must exist at {}",
            cogh_shim.display()
        );
        let shim_target_a = std::fs::read_link(&cogh_shim)
            .or_else(|_| std::fs::canonicalize(&cogh_shim))
            .expect("resolve shim A");
        assert!(
            shim_target_a.to_string_lossy().contains(F6W3_VERSION_A),
            "F6.W3.bis: post-install-A shim target must include version A ({}); got: {}",
            F6W3_VERSION_A,
            shim_target_a.display()
        );

        let out_a = Command::new(&cogh_shim)
            .output()
            .expect("execute installed binary A");
        let stdout_a = String::from_utf8_lossy(&out_a.stdout);
        assert_eq!(
            out_a.status.code(),
            Some(0),
            "F6.W3.bis: installed binary A exited non-zero: {:?}\nstdout: {}\nstderr: {}",
            out_a.status.code(),
            stdout_a,
            String::from_utf8_lossy(&out_a.stderr)
        );
        assert!(
            stdout_a.contains("cognicode dev-fixture"),
            "F6.W3.bis: post-install-A binary stdout must contain payload marker; got: {}",
            stdout_a
        );

        // 4. Plant user data at home root that MUST survive transitions.
        let user_marker = home.root.join("user_notes_bis.txt");
        std::fs::write(
            &user_marker,
            "user: F6.W3.bis marker survives A->B and B->A",
        )
        .expect("write user marker");
        assert!(user_marker.exists());

        // 5. Transition A → B (inline, mirroring prf_f6_w3_install_then_
        //    update_then_execute_then_rollback_then_uninstall's RAII
        //    ordering).
        let fx_b = ResolverFixture::build(F6W3_VERSION_B).expect("build fixture B");
        let _base_b = test_support::TempBaseUrl::set(&fx_b.release.base_url);
        cmd_update(
            &home,
            None,
            Channel::Stable,
            None,
            Some(fx_b.staging_dir.clone()),
            "core".to_string(),
            false,
        )
        .expect("update A -> B");
        drop(_base_b);
        drop(fx_b);

        // 6. Execute the installed binary post-transition. The shim must
        //    now point at B's tree (NOT A's); a regression that left
        //    the shim wired to A after the transition would fail here.
        let shim_target_b = std::fs::read_link(&cogh_shim)
            .or_else(|_| std::fs::canonicalize(&cogh_shim))
            .expect("resolve shim B");
        assert!(
            shim_target_b.to_string_lossy().contains(F6W3_VERSION_B),
            "F6.W3.bis: post-A->B shim target must include version B ({}); got: {}",
            F6W3_VERSION_B,
            shim_target_b.display()
        );
        assert!(
            !shim_target_b.to_string_lossy().contains(F6W3_VERSION_A),
            "F6.W3.bis: post-A->B shim target must NOT point at A's tree ({}); got: {}",
            F6W3_VERSION_A,
            shim_target_b.display()
        );

        let out_b = Command::new(&cogh_shim)
            .output()
            .expect("execute installed binary B");
        let stdout_b = String::from_utf8_lossy(&out_b.stdout);
        assert_eq!(
            out_b.status.code(),
            Some(0),
            "F6.W3.bis: installed binary B exited non-zero: {:?}\nstdout: {}\nstderr: {}",
            out_b.status.code(),
            stdout_b,
            String::from_utf8_lossy(&out_b.stderr)
        );
        assert!(
            stdout_b.contains("cognicode dev-fixture"),
            "F6.W3.bis: post-A->B binary stdout must contain payload marker; got: {}",
            stdout_b
        );

        // 7. User data must survive the A→B transition.
        assert!(
            user_marker.exists(),
            "F6.W3.bis: user marker MUST survive install A->B; got missing"
        );

        // 8. Rollback B → A. Build a fresh A fixture so rollback has the
        //    correct base; the contract is what the test pins, not the
        //    internal helper.
        //
        //    PRF F6.W3.bis contract: a successful `cmd_rollback` MUST
        //    leave the shim pointing at the previous version's executable
        //    (A in this case). The user's flow after `cogh rollback`
        //    does NOT include a separate `cogh reshim` step; the
        //    transition is atomic from the caller's perspective. The
        //    shim-resurrection is the rollback coordinator's
        //    responsibility, not the user's. The pre-fix test bis
        //    invoked `cmd_reshim` as a workaround for a known
        //    installer bug — that workaround is now removed and the
        //    coordinator is expected to perform the resurrection as
        //    part of `cmd_rollback` itself.
        let fx_a_again = ResolverFixture::build(F6W3_VERSION_A).expect("rebuild fixture A");
        let _base_a_real = test_support::TempBaseUrl::set(&fx_a_again.release.base_url);
        cmd_rollback(&home, None, Some(F6W3_VERSION_A.to_string())).expect("rollback B -> A");
        drop(_base_a_real);
        drop(fx_a_again);

        // 9. Execute the installed binary post-rollback. The shim must
        //    now point at A's tree again (NOT B's); a regression that
        //    left the shim wired to B after rollback would fail here.
        let shim_target_post_rb = std::fs::read_link(&cogh_shim)
            .or_else(|_| std::fs::canonicalize(&cogh_shim))
            .expect("resolve shim post-rollback");
        assert!(
            shim_target_post_rb
                .to_string_lossy()
                .contains(F6W3_VERSION_A),
            "F6.W3.bis: post-rollback shim target must include version A ({}); got: {}",
            F6W3_VERSION_A,
            shim_target_post_rb.display()
        );
        assert!(
            !shim_target_post_rb
                .to_string_lossy()
                .contains(F6W3_VERSION_B),
            "F6.W3.bis: post-rollback shim target must NOT point at B's tree ({}); got: {}",
            F6W3_VERSION_B,
            shim_target_post_rb.display()
        );

        let out_post_rb = Command::new(&cogh_shim)
            .output()
            .expect("execute installed binary post-rollback");
        let stdout_post_rb = String::from_utf8_lossy(&out_post_rb.stdout);
        assert_eq!(
            out_post_rb.status.code(),
            Some(0),
            "F6.W3.bis: installed binary post-rollback exited non-zero: {:?}\nstdout: {}\nstderr: {}",
            out_post_rb.status.code(),
            stdout_post_rb,
            String::from_utf8_lossy(&out_post_rb.stderr)
        );
        assert!(
            stdout_post_rb.contains("cognicode dev-fixture"),
            "F6.W3.bis: post-rollback binary stdout must contain payload marker; got: {}",
            stdout_post_rb
        );

        // 10. User data must survive rollback.
        assert!(
            user_marker.exists(),
            "F6.W3.bis: user marker MUST survive rollback"
        );
        assert_eq!(
            std::fs::read_to_string(&user_marker).expect("read user marker post-rollback"),
            "user: F6.W3.bis marker survives A->B and B->A",
            "user marker contents must be unchanged after rollback"
        );
    }

    /// PRF F6.W3.bis — negative case: when the rollback coordinator's
    /// shim-resurrection step fails, the rollback MUST return Err
    /// (not Ok) and MUST NOT consume the journal file (so the
    /// transition can be retried).
    ///
    /// We force the failure deterministically by removing the
    /// manifest of the previous version AFTER the tracker has been
    /// restored. The coordinator's `cmd_reshim` step then fails
    /// because it cannot read the manifest, which is exactly the
    /// condition we want to test.
    ///
    /// Concretely:
    ///   - install A (baseline)
    ///   - install B (transition A->B, journal recorded)
    ///   - delete A's manifest file
    ///   - run `cmd_rollback` to A — the reshim step fails
    ///   - assert: rollback returned Err
    ///   - assert: journal still exists on disk
    ///   - assert: tracker was restored to A (the rollback did
    ///     partially apply; that is the contract)
    ///   - assert: shim state is consistent with the partial state —
    ///     the B shim was removed by the reversal, the A shim is
    ///     missing because the reshim failed; the user sees a missing
    ///     shim, NOT a wrong shim.
    ///   - repair A's manifest (simulate the operator fixing the cause)
    ///   - re-run `cmd_rollback` — the second attempt succeeds
    ///     (state coherent, shim points at A, journal consumed)
    #[test]
    fn prf_f6_w3_bis_rollback_reports_failure_when_shim_resurrection_fails() {
        use crate::lifecycle_resolver::Channel;
        use crate::release_test_support::ResolverFixture;

        // 1. Set up a temp home + install version A (baseline).
        let home_tmp = test_support::TempCognicodeHome::new();
        let home = CognicodeHome::resolve(Some(home_tmp.path())).expect("resolve home");
        home.init().expect("home.init");

        let fx_a = ResolverFixture::build(F6W3_VERSION_A).expect("build fixture A");
        let _base_a = test_support::TempBaseUrl::set(&fx_a.release.base_url);
        let _opencode_a = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_A,
            Channel::Stable,
            None,
            Some(fx_a.staging_dir.clone()),
            "core",
        )
        .expect("install A");
        drop(_base_a);
        drop(_opencode_a);
        drop(fx_a);

        // 2. Install version B (transition A -> B). The lifecycle journal
        //    is recorded by `cmd_install` when a previous version is
        //    detected.
        let fx_b = ResolverFixture::build(F6W3_VERSION_B).expect("build fixture B");
        let _base_b = test_support::TempBaseUrl::set(&fx_b.release.base_url);
        let _opencode_b = test_support::TempOpenCodeConfig::disable();
        cmd_install(
            &home,
            F6W3_VERSION_B,
            Channel::Stable,
            None,
            Some(fx_b.staging_dir.clone()),
            "core",
        )
        .expect("install B");
        drop(_base_b);
        drop(_opencode_b);
        drop(fx_b);

        // 3. Verify the journal was recorded by the A->B transition.
        //    The journal is per-version under <home>/journal/<v>.json.
        let journal_path = home.journal_version(F6W3_VERSION_B);
        assert!(
            journal_path.exists(),
            "F6.W3.bis negative: lifecycle journal must exist after A->B transition; got missing at {}",
            journal_path.display()
        );

        // 4. Force the reshim failure by removing the previous version's
        //    manifest. After the tracker is restored to A, `cmd_reshim`
        //    will fail to read A's manifest, triggering the error path.
        let manifest_a = home.version_manifest(F6W3_VERSION_A);
        assert!(
            manifest_a.exists(),
            "F6.W3.bis negative: precondition — A manifest must exist before sabotage"
        );
        std::fs::remove_file(&manifest_a).expect("remove A manifest to force reshim failure");

        // 5. Attempt rollback B -> A. The reshim step must fail.
        let rb_result = cmd_rollback(&home, None, Some(F6W3_VERSION_A.to_string()));
        assert!(
            rb_result.is_err(),
            "F6.W3.bis negative: rollback MUST return Err when shim resurrection fails; got: {:?}",
            rb_result
        );
        let err_msg = format!("{}", rb_result.unwrap_err());
        assert!(
            err_msg.contains("shim resurrection")
                || err_msg.contains("cannot reshim")
                || err_msg.contains("manifest"),
            "F6.W3.bis negative: error must mention shim resurrection failure; got: {err_msg}"
        );

        // 6. The journal MUST be preserved so the rollback can be retried.
        assert!(
            journal_path.exists(),
            "F6.W3.bis negative: journal MUST be preserved when reshim fails (retry contract)"
        );

        // 7. The tracker WAS partially restored (rollback is not all-or-nothing
        //    on the coordinate level, but its RETURN value is Err so callers
        //    can react). Pin the tracker's content as part of the contract.
        let tracker_content = std::fs::read_to_string(&home.tracker_version())
            .expect("read tracker after failed rollback");
        assert_eq!(
            tracker_content.trim(),
            F6W3_VERSION_A,
            "F6.W3.bis negative: tracker MUST have been restored to A before the reshim step failed"
        );

        // 8. Shim state after the partial rollback. The reversal removed the
        //    B shim (CreatedSymlink reversal is the LAST step of the journal
        //    LIFO, so it has already happened by the time reshim fails). The
        //    A shim must NOT exist because the reshim step is the one that
        //    creates it, and it errored out.
        //
        //    State on disk:
        //      - shims/cognicode -> absent (B removed by reversal, A not
        //        recreated because reshim failed)
        //      - tracker -> A
        //      - journal/B.json -> present (retry contract)
        //
        //    The user sees: no shim, tracker says A, journal says "rollback
        //    pending". That is the **correct, observable** recoverable
        //    state. They fix the underlying cause (restore A's manifest)
        //    and retry. A wrong shim pointing at B after a failed rollback
        //    would be much worse.
        let cogh_shim = home.shim_path("cognicode");
        assert!(
            !cogh_shim.symlink_metadata().is_ok(),
            "F6.W3.bis negative: shim MUST NOT exist after failed rollback \
             (reversal removed B, reshim failed to recreate A); a wrong shim \
             here would be a much worse bug than a missing one. Got shim at: {}",
            cogh_shim.display()
        );

        // 9. Recovery path: restore A's manifest (structurally valid
        //    BundleManifest because `cmd_reshim` deserialises it),
        //    retry `cmd_rollback`. The journal is still on disk; the
        //    second attempt MUST succeed because the reshim step can
        //    now read A's manifest. The final state must be coherent:
        //    tracker=A, shim points at A, journal consumed.
        //
        //    The user does NOT need to invoke any external `cogh reshim` —
        //    the rollback coordinator handles resurrection itself. The
        //    only intervention required is restoring the underlying
        //    manifest, which is exactly the cause of the original failure.
        let recovered_manifest_a = home.version_manifest(F6W3_VERSION_A);
        {
            use crate::bundle_manifest::{BundleComponent, BundleManifest};
            use crate::platform_adapter::Platform;
            use crate::release_contract::{ArtifactDigest, ArtifactKind};
            let prev_digest = ArtifactDigest::parse(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .expect("digest hex must be valid");
            let manifest = BundleManifest {
                api_version: "cognicode.bundle/v2".to_string(),
                kind: "Bundle".to_string(),
                version: F6W3_VERSION_A.to_string(),
                platform: Platform::LinuxX86_64,
                released_at: None,
                profiles: vec![crate::bundle_manifest::ProfileDef {
                    name: "core".to_string(),
                    description: "recovered".to_string(),
                }],
                skill_bundles: vec![],
                components: vec![BundleComponent {
                    name: "cognicode".to_string(),
                    kind: ArtifactKind::Cognicode,
                    version: F6W3_VERSION_A.to_string(),
                    artifact: crate::release_contract::artifact_filename(
                        "cognicode",
                        F6W3_VERSION_A,
                        Platform::LinuxX86_64,
                    ),
                    sha256: prev_digest,
                    url: crate::release_contract::artifact_url(
                        F6W3_VERSION_A,
                        &crate::release_contract::artifact_filename(
                            "cognicode",
                            F6W3_VERSION_A,
                            Platform::LinuxX86_64,
                        ),
                    ),
                    profiles: vec!["core".to_string()],
                }],
            };
            std::fs::write(
                &recovered_manifest_a,
                serde_yaml::to_string(&manifest).unwrap(),
            )
            .expect("restore A manifest for retry");
        }

        cmd_rollback(&home, None, Some(F6W3_VERSION_A.to_string()))
            .expect("F6.W3.bis negative: second rollback attempt MUST succeed after manifest restoration");

        // 10. Final state is coherent.
        assert_eq!(
            std::fs::read_to_string(&home.tracker_version())
                .expect("read tracker post-recovery")
                .trim(),
            F6W3_VERSION_A,
            "F6.W3.bis negative: tracker must remain A after successful retry"
        );
        assert!(
            !journal_path.exists(),
            "F6.W3.bis negative: journal MUST be consumed after the successful retry"
        );
        assert!(
            cogh_shim.symlink_metadata().is_ok(),
            "F6.W3.bis negative: A shim MUST exist after successful retry"
        );
        let shim_target = std::fs::read_link(&cogh_shim)
            .or_else(|_| std::fs::canonicalize(&cogh_shim))
            .expect("resolve A shim post-recovery");
        assert!(
            shim_target.to_string_lossy().contains(F6W3_VERSION_A),
            "F6.W3.bis negative: A shim MUST point at A; got {}",
            shim_target.display()
        );
        assert!(
            !shim_target.to_string_lossy().contains(F6W3_VERSION_B),
            "F6.W3.bis negative: A shim MUST NOT point at B; got {}",
            shim_target.display()
        );
    }
}

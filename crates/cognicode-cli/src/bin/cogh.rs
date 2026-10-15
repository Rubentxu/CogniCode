//! `cogh` — CogniCode version manager CLI
//!
//! Implementation of E32-A. Mirrors the asdf-vm pattern (ADR-035).
//! Single-binary Rust CLI for managing CogniCode runtime artifacts:
//! MCP server, sandbox containers, skills, IDE integration.
//! See docs/adr/ADR-034-cognicode-distribution-package.md.
//!
//! ## Lint policy
//!
//! The cogh CLI imports the full `cmd/*` module surface via `#[path = "..."]`
//! so the in-tree `cogh::install`, `cogh::update`, `cogh::rollback` etc.
//! share types and helpers with the `cognicode-release` bin and the
//! integration tests.  Some items (e.g. `LockGuard::new`,
//! `RollbackJournal::{effects,to_json,from_json}`, `journal_path`,
//! `filter_by_profile`, `verify_sha256`, `Layer::Layer0Boot`/`Layer1Runtime`)
//! are referenced from `cognicode-release` and from tests but not from
//! the production `cogh` binary path.  Without the bin-level allow
//! below, `cargo clippy --bin cogh -- -D warnings` would reject every
//! build.  Items still unused after both bins AND tests are exercised
//! are tracked in the dead-code ledger rather than silenced here.
#![allow(dead_code)]
#![allow(unused_imports)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[path = "../cmd/bundle_manifest.rs"]
mod bundle_manifest;
#[path = "../cmd/bundled.rs"]
mod bundled;
#[path = "../cmd/cache.rs"]
mod cache;
#[path = "../cmd/doctor.rs"]
mod doctor;
#[path = "../cmd/error.rs"]
mod error;
#[path = "../cmd/ide.rs"]
mod ide;
#[path = "../cmd/install.rs"]
mod install;
#[path = "../cmd/install_lock.rs"]
mod install_lock;
#[path = "../cmd/installer_transaction.rs"]
mod installer_transaction;
#[path = "../cmd/layout.rs"]
mod layout;
#[path = "../cmd/lifecycle.rs"]
mod lifecycle;
#[path = "../cmd/lifecycle_journal.rs"]
mod lifecycle_journal;
#[path = "../cmd/lifecycle_resolver.rs"]
mod lifecycle_resolver;
#[path = "../cmd/lockfile.rs"]
mod lockfile;
#[path = "../cmd/manifest.rs"]
mod manifest;
#[path = "../cmd/platform_adapter.rs"]
pub mod platform_adapter;
#[path = "../cmd/profile.rs"]
mod profile;
#[path = "../cmd/registry.rs"]
mod registry;
#[path = "../cmd/release_contract.rs"]
mod release_contract;
#[cfg(test)]
#[path = "../cmd/release_factory.rs"]
mod release_factory;
#[cfg(test)]
#[path = "../cmd/release_test_support.rs"]
mod release_test_support;
#[path = "../cmd/rollback_journal.rs"]
mod rollback_journal;
#[path = "../cmd/skill.rs"]
mod skill;
#[path = "../skills.rs"]
mod skills;
#[path = "../cmd/tracker.rs"]
mod tracker;
#[path = "../cmd/version.rs"]
mod version;

use layout::CognicodeHome;

#[derive(Parser, Debug)]
#[command(name = "cogh", version, about = "CogniCode version manager")]
pub struct Cli {
    /// Path to COGNICODE_HOME (defaults to ~/.cognicode)
    #[arg(long, global = true)]
    pub home: Option<std::path::PathBuf>,

    /// Verbose output
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install a plugin at a specific version, or self-install cogh
    /// (B6.W3). When `plugin` is omitted, `--version` controls the
    /// cogh lifecycle and `--apply` is required to actually write.
    Install {
        /// Plugin name (e.g. mcp-server, opencode). Optional in B6.W3:
        /// when omitted, the install is the self-install surface
        /// driven by `--version` / `--rollback` / `--apply`.
        plugin: Option<String>,
        /// Version ref ("latest" or e.g. 0.96.0). For the self-install
        /// surface (no plugin) this is the cogh version to install.
        #[arg(long, default_value = "latest")]
        version: String,
        /// Configure one or more IDEs (opencode, zcode, claude, codex, all)
        #[arg(long, value_delimiter = ',')]
        ide: Vec<String>,
        /// Installation profile (core or reviewer; default reviewer when --ide is supplied)
        #[arg(long)]
        profile: Option<String>,
        /// Channel selector (stable by default; preview not yet published)
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Override the GitHub base URL (for air-gapped installs and tests)
        #[arg(long)]
        base_url: Option<String>,
        /// Read releases.json from this directory instead of hitting the API
        #[arg(long)]
        staging: Option<PathBuf>,
        /// B6.W3: rollback cogh to a previously pinned version.
        /// Mutually exclusive with the plugin install path.
        #[arg(long)]
        rollback: Option<String>,
        /// B6.W3: actually write the install. Without this flag the
        /// self-install surface prints a JSON `InstallPlan` for review
        /// and exits 0 (no filesystem mutation).
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// Uninstall a plugin version
    Uninstall {
        plugin: String,
        #[arg(long)]
        version: String,
        /// Also uninstall IDE configurations
        #[arg(long, value_delimiter = ',')]
        ide: Vec<String>,
    },
    /// List installed plugins and versions
    List {
        /// Show only installed plugins
        #[arg(long)]
        installed: bool,
    },
    /// Show the active version pin
    Current,
    /// Show the latest stable version for a plugin
    Latest {
        /// Plugin name (omit for --all)
        plugin: Option<String>,
        /// Show latest for all plugins
        #[arg(long)]
        all: bool,
        /// Channel selector (stable by default; preview not yet published)
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Override the GitHub base URL (for air-gapped installs and tests)
        #[arg(long)]
        base_url: Option<String>,
        /// Read releases.json from this directory instead of hitting the API
        #[arg(long)]
        staging: Option<PathBuf>,
        /// Emit a JSON object instead of the plain tag
        #[arg(long)]
        json: bool,
    },
    /// Update a plugin to the latest version (respects .cognicode.lock)
    Update {
        /// Plugin name (omit for all)
        plugin: Option<String>,
        /// Channel selector (stable by default; preview not yet published)
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Override the GitHub base URL (for air-gapped installs and tests)
        #[arg(long)]
        base_url: Option<String>,
        /// Read releases.json from this directory instead of hitting the API
        #[arg(long)]
        staging: Option<PathBuf>,
        /// Profile to install (default: preserve the active installed profile)
        #[arg(long)]
        profile: Option<String>,
        /// Resolve the latest release but do not download or install
        #[arg(long)]
        dry_run: bool,
    },
    /// Regenerate the shims directory
    Reshim,
    /// Reverse the last install, or roll back to a specific version (e86.4 REQ-RB-01..05)
    Rollback {
        /// Plugin name (reserved for future use; today the active install is the only target)
        plugin: Option<String>,
        /// Roll back to a specific version instead of the previous one (e86.4 REQ-RB-01).
        /// When omitted, behaves like the legacy `cogh rollback` (undo last transaction).
        #[arg(long)]
        to: Option<String>,
    },
    /// Validate the install + diagnose issues
    Doctor,
    /// Print the resolved path to a binary
    Where { binary: String },
    /// Initialize ~/.cognicode/ with bundled plugins
    Init,
    /// A-015 / CP2.4-onboarding — install, then diagnose the happy path.
    ///
    /// The default version/channel follow the normal stable install policy.
    /// `--staging` provides the hermetic release fixture used by UAT and by
    /// air-gapped operators; without it, resolution uses the configured
    /// release service.
    Setup {
        /// Version ref (`latest` or an explicit release version).
        #[arg(long, default_value = "latest")]
        version: String,
        /// Release channel (stable by default).
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Override the release API and asset origin.
        #[arg(long)]
        base_url: Option<String>,
        /// Read releases.json from this directory instead of using the API.
        #[arg(long)]
        staging: Option<PathBuf>,
        /// Installation profile (reviewer includes the MCP daemon).
        #[arg(long, default_value = "reviewer")]
        profile: String,
    },
    /// Plugin management (add/remove/list)
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Validate a portable skill bundle
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// List agent-first skills from product/skills.json (B6.W4)
    Skills {
        /// Emit a JSON array instead of the plain table
        #[arg(long)]
        json: bool,
    },
    /// IDE-specific operations (detect, install)
    Ide {
        #[command(subcommand)]
        action: IdeAction,
    },
    /// Print cogh + CogniCode version
    Version,
}

#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// Register a plugin (from git-url or registered marketplace)
    Add {
        /// Plugin name
        name: String,
        /// GitHub URL or shorthand (owner/repo)
        #[arg(long)]
        url: Option<String>,
    },
    /// Unregister a plugin
    Remove { name: String },
    /// List registered plugins
    List,
    /// Update a plugin's source repository
    Update { name: String },
}

#[derive(Subcommand, Debug)]
pub enum SkillAction {
    /// Validate a portable skill bundle (directory or .yaml)
    Validate {
        /// Path to the skill bundle directory
        path: std::path::PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum IdeAction {
    /// Detect installed IDEs
    Detect,
    /// Install (integrate) the MCP server + skills into an IDE
    Install {
        /// IDE name (e.g. opencode, zcode, claude, codex)
        ide: String,
        /// Plugin name (e.g. mcp-server)
        #[arg(long)]
        plugin: String,
        /// Version ref
        #[arg(long, default_value = "latest")]
        version: String,
    },
    /// Uninstall (remove) the MCP server + skills from an IDE
    Uninstall {
        /// IDE name
        ide: String,
        /// Plugin version
        #[arg(long)]
        version: String,
    },
}

fn finish_setup(
    home: &CognicodeHome,
    version: &str,
    tag: &str,
    profile: &str,
) -> anyhow::Result<()> {
    let report = doctor::run_doctor(&home.root);
    print!("{report}");
    if !report.is_healthy() {
        return Err(anyhow::anyhow!(
            "setup failed: final doctor report is unhealthy"
        ));
    }
    println!();
    println!(
        "Setup complete: installed {} ({}) with the {} profile.",
        version, tag, profile
    );
    Ok(())
}

fn run_setup(
    home: &CognicodeHome,
    version: &str,
    channel: &str,
    base_url: Option<String>,
    staging: Option<PathBuf>,
    profile: &str,
) -> anyhow::Result<()> {
    layout::cmd_init(home)?;
    let channel = channel
        .parse::<lifecycle_resolver::Channel>()
        .map_err(|e| anyhow::anyhow!("invalid release channel: {e}"))?;
    let resolved = layout::cmd_install(home, version, channel, base_url, staging, profile)?;
    finish_setup(home, &resolved.version, &resolved.tag, profile)
}

/// B6.W3 — `cogh install --version X` / `--rollback Y`.
///
/// Self-install surface for the `cogh` binary itself. The plan is
/// always printed to stdout (JSON when `--apply` is not set, JSON
/// outcome when it is). Without `--apply` we never touch the
/// filesystem: this lets a UI preview the destination and only
/// commit when the user confirms.
fn run_self_install(
    home: &CognicodeHome,
    version: &str,
    rollback: Option<&str>,
    apply: bool,
) -> anyhow::Result<()> {
    use cognicode_cli::lifecycle::{
        InstallAction, execute_install, plan_install, plan_rollback,
    };

    let plan = if let Some(target) = rollback {
        plan_rollback(&home.root, target)
            .map_err(|e| anyhow::anyhow!("lifecycle plan failed: {e}"))?
    } else {
        plan_install(version)
            .map_err(|e| anyhow::anyhow!("lifecycle plan failed: {e}"))?
    };

    if !apply {
        // Dry-run envelope. Always exit 0; the plan is the result.
        let envelope = serde_json::json!({
            "dry_run": true,
            "plan": plan,
            "action": if rollback.is_some() {
                serde_json::json!(InstallAction::Rollback {
                    from: String::new(),
                    to: plan.version.clone(),
                })
            } else {
                serde_json::json!(InstallAction::Install)
            },
            "message": "re-run with --apply to perform the install",
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
        return Ok(());
    }

    // Apply path: copy the current `cogh` binary to the planned
    // install path, then write the tracker so a subsequent
    // `cogh current` reflects the new pin. The source binary is
    // resolved from `current_exe()`; under `cargo test` that
    // resolves to the integration-test binary, which is fine for
    // subprocess coverage.
    let source = std::env::current_exe().map_err(|e| {
        anyhow::anyhow!("could not resolve current cogh binary for self-install: {e}")
    })?;
    let mut outcome = execute_install(&plan, &source)
        .map_err(|e| anyhow::anyhow!("lifecycle apply failed: {e}"))?;
    if rollback.is_some() {
        outcome.action = InstallAction::Rollback {
            from: String::new(),
            to: plan.version.clone(),
        };
    }
    if let Some(parent) = home.tracker_version().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(home.tracker_version(), plan.version.as_bytes())?;
    let envelope = serde_json::json!({
        "dry_run": false,
        "outcome": outcome,
    });
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        let _ = tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(tracing::Level::DEBUG)
                .finish(),
        );
    }

    let home = CognicodeHome::resolve(cli.home.as_deref())?;

    match cli.command {
        Command::Init => layout::cmd_init(&home),
        Command::Setup {
            version,
            channel,
            base_url,
            staging,
            profile,
        } => run_setup(&home, &version, &channel, base_url, staging, &profile),
        Command::Install {
            plugin,
            version,
            ide,
            profile,
            channel,
            base_url,
            staging,
            rollback,
            apply,
        } => {
            // B6.W3: the plugin positional is now optional. When it is
            // absent, the install is the self-install surface
            // (`cogh install --version X` / `--rollback Y`), driven by
            // the new `cognicode_cli::lifecycle` module. When it is
            // present, the legacy plugin install path runs unchanged.
            let plugin_missing = plugin.is_none();
            if plugin_missing || rollback.is_some() {
                return run_self_install(
                    &home,
                    version.as_str(),
                    rollback.as_deref(),
                    apply,
                );
            }
            let plugin = plugin.expect("plugin is required on the legacy path");
            // e87.1: the requested version governs the core bundle. Resolve
            // the published release remotely (or from --staging), stage its
            // BundleManifest, then run the atomic install. A remote failure
            // is a hard error — never a DEV-fixture fallback.
            let channel = channel
                .parse::<lifecycle_resolver::Channel>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let profile = profile
                .unwrap_or_else(|| (if ide.is_empty() { "core" } else { "reviewer" }).to_string());
            let resolved =
                layout::cmd_install(&home, &version, channel, base_url, staging, &profile)?;
            let installed_version = resolved.version;

            // Then dispatch to IDE adapters if --ide was provided
            if !ide.is_empty() {
                // Valid IDEs that dispatch to ide::cmd_ide_install
                let valid_ides = ["opencode", "zcode", "claude", "codex"];

                // Handle --ide all: install to all 4 IDEs
                if ide.contains(&"all".to_string()) {
                    for ide_name in valid_ides {
                        ide::cmd_ide_install(&home, ide_name, &plugin, &installed_version)?;
                    }
                } else {
                    // Check each IDE argument
                    for ide_name in &ide {
                        if !valid_ides.contains(&ide_name.as_str()) {
                            return Err(anyhow::anyhow!(
                                "Unknown IDE '{}'. Valid options: opencode, zcode, claude, codex, all",
                                ide_name
                            ));
                        }
                        ide::cmd_ide_install(&home, ide_name, &plugin, &installed_version)?;
                    }
                }
            }
            Ok(())
        }
        Command::Uninstall {
            plugin,
            version,
            ide,
        } => layout::cmd_uninstall(&home, &plugin, &version, &ide),
        Command::List { installed } => layout::cmd_list(&home, installed),
        Command::Current => layout::cmd_current(&home),
        Command::Latest {
            plugin,
            all,
            channel,
            base_url,
            staging,
            json,
        } => {
            let channel = channel
                .parse::<lifecycle_resolver::Channel>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            layout::cmd_latest(&home, plugin, all, channel, base_url, staging, json)
        }
        Command::Update {
            plugin,
            channel,
            base_url,
            staging,
            profile,
            dry_run,
        } => {
            let channel = channel
                .parse::<lifecycle_resolver::Channel>()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let profile = profile.unwrap_or_else(|| layout::active_install_profile(&home).into());
            layout::cmd_update(&home, plugin, channel, base_url, staging, profile, dry_run)
        }
        Command::Reshim => layout::cmd_reshim(&home),
        Command::Rollback { plugin, to } => layout::cmd_rollback(&home, plugin, to),
        Command::Doctor => layout::cmd_doctor(&home),
        Command::Where { binary } => layout::cmd_where(&home, &binary),
        Command::Version => version::cmd_version(&home),
        Command::Plugin { action } => match action {
            PluginAction::Add { name, url } => {
                layout::cmd_plugin_add(&home, &name, url.as_deref())?;
                println!("Plugin {} added", name);
                Ok(())
            }
            PluginAction::Remove { name } => {
                layout::cmd_plugin_remove(&home, &name)?;
                println!("Plugin {} removed", name);
                Ok(())
            }
            PluginAction::List => {
                layout::cmd_plugin_list(&home)?;
                Ok(())
            }
            PluginAction::Update { name } => {
                layout::cmd_plugin_update(&home, &name)?;
                Ok(())
            }
        },
        Command::Skill { action } => match action {
            SkillAction::Validate { path } => skill::cmd_skill_validate(&path),
        },
        Command::Skills { json } => skills::cmd_skills_list(json)
            .map_err(|e| anyhow::anyhow!("{e}")),
        Command::Ide { action } => match action {
            IdeAction::Detect => ide::cmd_ide_detect(),
            IdeAction::Install {
                ide,
                plugin,
                version,
            } => ide::cmd_ide_install(&home, &ide, &plugin, &version),
            IdeAction::Uninstall { ide, version } => ide::cmd_ide_uninstall(&home, &ide, &version),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release_test_support::ResolverFixture;
    use serial_test::serial;

    #[test]
    #[serial]
    fn setup_installs_reviewer_profile_before_doctor() {
        let fixture = ResolverFixture::build("0.100.0").expect("build release fixture");
        let temp = tempfile::tempdir().expect("create setup home");
        let home = CognicodeHome::resolve(Some(temp.path())).expect("resolve setup home");
        unsafe {
            std::env::set_var("COGNICODE_ASSET_BASE_URL", &fixture.release.base_url);
        }

        run_setup(
            &home,
            "0.100.0",
            "stable",
            Some(fixture.release.base_url.clone()),
            Some(fixture.staging_dir.clone()),
            "reviewer",
        )
        .expect("setup should install and diagnose");

        assert_eq!(
            std::fs::read_to_string(home.tracker_version()).expect("read version pin"),
            "0.100.0"
        );
        assert!(
            home.version_manifest("0.100.0").exists(),
            "setup must materialise the installed bundle manifest"
        );
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
        }
    }

    #[test]
    #[serial]
    fn setup_rejects_an_unhealthy_final_doctor_report() {
        let temp = tempfile::tempdir().expect("create setup home");
        let home = CognicodeHome::resolve(Some(temp.path())).expect("resolve setup home");
        let error = finish_setup(&home, "0.100.0", "v0.100.0", "reviewer")
            .expect_err("setup must fail when doctor reports a core failure");

        assert!(
            error.to_string().contains("unhealthy"),
            "expected an unhealthy setup error, got: {error:#}"
        );
    }
}

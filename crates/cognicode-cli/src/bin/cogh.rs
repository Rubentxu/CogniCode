//! `cogh` — CogniCode version manager CLI
//!
// Implementation of E32-A. Mirrors the asdf-vm pattern (ADR-035).
// Single-binary Rust CLI for managing CogniCode runtime artifacts:
// MCP server, sandbox containers, skills, IDE integration.
// See docs/adr/ADR-034-cognicode-distribution-package.md.

use clap::{Parser, Subcommand};

#[path = "../cmd/error.rs"]
mod error;
#[path = "../cmd/bundled.rs"]
mod bundled;
#[path = "../cmd/bundle_manifest.rs"]
mod bundle_manifest;
#[path = "../cmd/ide.rs"]
mod ide;
#[path = "../cmd/layout.rs"]
mod layout;
#[path = "../cmd/lifecycle.rs"]
mod lifecycle;
#[path = "../cmd/lockfile.rs"]
mod lockfile;
#[path = "../cmd/manifest.rs"]
mod manifest;
#[path = "../cmd/registry.rs"]
mod registry;
#[path = "../cmd/skill.rs"]
mod skill;
#[path = "../cmd/version.rs"]
mod version;
#[path = "../cmd/install.rs"]
mod install;
#[path = "../cmd/installer_transaction.rs"]
mod installer_transaction;
#[path = "../cmd/rollback_journal.rs"]
mod rollback_journal;
#[path = "../cmd/cache.rs"]
mod cache;

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
    /// Install a plugin at a specific version
    Install {
        /// Plugin name (e.g. mcp-server, opencode)
        plugin: String,
        /// Version ref (e.g. 0.92.0, latest)
        #[arg(long, default_value = "latest")]
        version: String,
        /// Configure one or more IDEs (opencode, zcode, claude, codex, all)
        #[arg(long, value_delimiter = ',')]
        ide: Vec<String>,
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
    },
    /// Update a plugin to the latest version (respects .cognicode.lock)
    Update {
        /// Plugin name (omit for all)
        plugin: Option<String>,
    },
    /// Regenerate the shims directory
    Reshim,
    /// Validate the install + diagnose issues
    Doctor,
    /// Print the resolved path to a binary
    Where {
        binary: String,
    },
    /// Initialize ~/.cognicode/ with bundled plugins
    Init,
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
        plugin: String,
        /// Git URL for community plugins
        #[arg(long)]
        from_url: Option<String>,
    },
    /// Unregister a plugin
    Remove { plugin: String },
    /// List registered plugins
    List,
    /// Update a plugin's source repository
    Update { plugin: String },
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
        Command::Install { plugin, version, ide } => {
            layout::cmd_install(&home, &plugin, &version, &ide)
        }
        Command::Uninstall { plugin, version, ide } => {
            layout::cmd_uninstall(&home, &plugin, &version, &ide)
        }
        Command::List { installed } => layout::cmd_list(&home, installed),
        Command::Current => layout::cmd_current(&home),
        Command::Latest { plugin, all } => layout::cmd_latest(&home, plugin, all),
        Command::Update { plugin } => layout::cmd_update(&home, plugin),
        Command::Reshim => layout::cmd_reshim(&home),
        Command::Doctor => layout::cmd_doctor(&home),
        Command::Where { binary } => layout::cmd_where(&home, &binary),
        Command::Version => version::cmd_version(&home),
        Command::Plugin { action } => match action {
            PluginAction::Add { plugin, from_url } => {
                layout::cmd_plugin_add(&home, &plugin, from_url.as_deref())
            }
            PluginAction::Remove { plugin } => layout::cmd_plugin_remove(&home, &plugin),
            PluginAction::List => layout::cmd_plugin_list(&home),
            PluginAction::Update { plugin } => layout::cmd_plugin_update(&home, &plugin),
        },
        Command::Skill { action } => match action {
            SkillAction::Validate { path } => skill::cmd_skill_validate(&path),
        },
        Command::Ide { action } => match action {
            IdeAction::Detect => ide::cmd_ide_detect(),
            IdeAction::Install { ide, plugin, version } => {
                ide::cmd_ide_install(&home, &ide, &plugin, &version)
            }
            IdeAction::Uninstall { ide, version } => {
                ide::cmd_ide_uninstall(&home, &ide, &version)
            }
        },
    }
}

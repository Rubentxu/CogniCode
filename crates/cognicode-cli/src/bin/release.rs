//! `cognicode-release` — the e85 release factory tool.
//!
//! This binary is the executable form of the release contract. It exists so that
//! **no shell script and no workflow YAML ever types a platform token, an
//! artifact filename, or a digest**. Everything is derived from the Rust
//! contract in [`release_contract`].
//!
//! It never touches the network. `generate` and `verify` operate on a local
//! staging directory, so the whole contract can be checked before any GitHub
//! Release exists.
//!
//! ## Usage
//!
//! ```text
//! cognicode-release platforms                      # Tier 1 platform tokens
//! cognicode-release platform-token --platform linux-aarch64
//! cognicode-release name --component cognicode --platform linux-aarch64
//! cognicode-release plan --platform linux-x86-64
//! cognicode-release generate --staging dist --out dist/generated \
//!     --version 0.95.0 --tag v0.95.0 --source-commit "$GITHUB_SHA"
//! cognicode-release verify   --staging dist \
//!     --version 0.95.0 --tag v0.95.0
//! ```

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[path = "../cmd/bundle_manifest.rs"]
mod bundle_manifest;
#[path = "../cmd/release_contract.rs"]
mod release_contract;
#[path = "../cmd/release_factory.rs"]
mod release_factory;

use bundle_manifest::Platform;
use release_contract::{
    TIER1_PLATFORMS, artifact_filename, component_by_stem, parse_platform, platform_token,
    published_components,
};

#[derive(Parser, Debug)]
#[command(
    name = "cognicode-release",
    version,
    about = "CogniCode release factory: derive, generate and verify release artifacts"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the Tier 1 platform Rust target tokens.
    Platforms,
    /// Print the Rust target token for one platform.
    PlatformToken {
        /// Kebab platform name (`linux-x86-64`) or Rust triple.
        #[arg(long)]
        platform: String,
    },
    /// Print the canonical artifact filename for a component.
    Name {
        /// Component name (`cogh`, `cognicode`, `cognicode-mcp`, ...).
        #[arg(long)]
        component: String,
        #[arg(long)]
        platform: String,
        #[arg(long)]
        version: Option<String>,
    },
    /// Print the canonical payload set for a platform (the per-lane ledger).
    Plan {
        #[arg(long)]
        platform: String,
        #[arg(long)]
        version: Option<String>,
    },
    /// Generate BundleManifest v2, ReleaseInventory and SHA256SUMS.
    Generate {
        /// Directory holding the produced payload archives.
        #[arg(long)]
        staging: PathBuf,
        /// Where the generated artifacts are written.
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        source_commit: String,
        /// Platforms to include; defaults to Tier 1.
        #[arg(long = "platform")]
        platforms: Vec<String>,
        /// ISO-8601 timestamp recorded in the manifests.
        #[arg(long)]
        released_at: Option<String>,
    },
    /// Verify a staged release against R1-R9. Runs locally; never publishes.
    Verify {
        #[arg(long)]
        staging: PathBuf,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        tag: String,
        #[arg(long = "platform")]
        platforms: Vec<String>,
    },
}

/// The workspace version, so callers need not repeat it.
fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn resolve_platforms(raw: &[String]) -> anyhow::Result<Vec<Platform>> {
    if raw.is_empty() {
        return Ok(TIER1_PLATFORMS.to_vec());
    }
    raw.iter()
        .map(|r| {
            parse_platform(r).ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown platform `{r}`; expected one of linux-x86-64, linux-aarch64, \
                     mac-os-x86-64, mac-os-aarch64, windows-x86-64, or a Rust triple"
                )
            })
        })
        .collect()
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Platforms => {
            for p in TIER1_PLATFORMS {
                println!("{}", platform_token(p));
            }
            Ok(())
        }

        Command::PlatformToken { platform } => {
            let p = parse_platform(&platform)
                .ok_or_else(|| anyhow::anyhow!("unknown platform `{platform}`"))?;
            println!("{}", platform_token(p));
            Ok(())
        }

        Command::Name {
            component,
            platform,
            version,
        } => {
            let p = parse_platform(&platform)
                .ok_or_else(|| anyhow::anyhow!("unknown platform `{platform}`"))?;
            let spec = component_by_stem(&component)
                .ok_or_else(|| anyhow::anyhow!("unknown component `{component}`"))?;
            let v = version.unwrap_or_else(default_version);
            println!("{}", artifact_filename(spec.kind.stem(), &v, p));
            Ok(())
        }

        Command::Plan { platform, version } => {
            let p = parse_platform(&platform)
                .ok_or_else(|| anyhow::anyhow!("unknown platform `{platform}`"))?;
            let v = version.unwrap_or_else(default_version);
            for spec in published_components() {
                println!("{}", artifact_filename(spec.kind.stem(), &v, p));
            }
            Ok(())
        }

        Command::Generate {
            staging,
            out,
            version,
            tag,
            source_commit,
            platforms,
            released_at,
        } => {
            let v = version.unwrap_or_else(default_version);
            let plats = resolve_platforms(&platforms)?;
            let report = release_factory::generate_release(
                &staging,
                &out,
                &v,
                &tag,
                &source_commit,
                &plats,
                released_at,
            )?;
            println!(
                "generate: OK  version={} tag={} payloads={} manifests={} sha256sums_entries={}",
                report.version,
                report.tag,
                report.payload_count,
                report.manifest_count,
                report.sha256sums_entries
            );
            Ok(())
        }

        Command::Verify {
            staging,
            version,
            tag,
            platforms,
        } => {
            let v = version.unwrap_or_else(default_version);
            let plats = resolve_platforms(&platforms)?;
            let report = release_factory::verify_release(&staging, &v, &tag, &plats)?;
            print!("{}", report.render());
            Ok(())
        }
    }
}

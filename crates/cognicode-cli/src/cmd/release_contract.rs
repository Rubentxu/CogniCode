//! e85 — Canonical release artifact contract, executable.
//!
//! This module is the single source of truth for the vocabulary decided in e84
//! (`openspec/changes/e84-cognicode-distribution-artifact-contract/wu2-*`). It is
//! shared, via `#[path]`, by the `cogh` binary and the `cognicode-release` tool,
//! so there is exactly one implementation of the contract.
//!
//! ## What lives here
//!
//! * the **total** `Platform -> Rust target triple` mapping (e84 R9);
//! * the published **product surface** — which components are Layer 0, which are
//!   Layer 1, which profiles include them (e84 WU3);
//! * **derived** artifact filenames and URLs (e84 R1);
//! * **digest validation**, including placeholder rejection (e84 R5);
//! * the `ProducedArtifact` ledger and the `ReleaseInventory` (e84 WU5/WU6);
//! * `BundleManifest` v2 projection (e84 WU7);
//! * `SHA256SUMS` (e84 WU10);
//! * the executable verification gate for R1–R9 (e84 WU9).
//!
//! ## What deliberately does NOT live here
//!
//! No network access and no publication. The release tool must be runnable
//! locally against a staging directory, with no GitHub Release in existence.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::bundle_manifest::Platform;

/// The repository that hosts the releases.
pub const RELEASE_REPO: &str = "Rubentxu/CogniCode";

/// Base URL for release downloads. The tag is always `v{version}`.
pub const RELEASE_DOWNLOAD_BASE: &str = "https://github.com/Rubentxu/CogniCode/releases/download";

/// Lowest numeric value still considered "not obviously a placeholder".
///
/// A real SHA256 does not have 60 leading zero bits. Anything numerically this
/// small is by construction a hand-written sequence like `0000…0001`.
const MIN_PLAUSIBLE_DIGEST_VALUE: u128 = 0x1_0000_0000_0000_0000;

// ---------------------------------------------------------------------------
// Platform <-> Rust target triple (e84 R9: total, single place)
// ---------------------------------------------------------------------------

/// Every platform the contract knows about, in stable order.
pub const ALL_PLATFORMS: [Platform; 5] = [
    Platform::LinuxX86_64,
    Platform::LinuxAarch64,
    Platform::MacOsX86_64,
    Platform::MacOsAarch64,
    Platform::WindowsX86_64,
];

/// Platforms e85 actually publishes. Tier 1 (e84 WU9).
pub const TIER1_PLATFORMS: [Platform; 2] = [Platform::LinuxX86_64, Platform::LinuxAarch64];

/// The Rust target triple for a platform. Total over all variants.
pub fn platform_token(platform: Platform) -> &'static str {
    match platform {
        Platform::LinuxX86_64 => "x86_64-unknown-linux-gnu",
        Platform::LinuxAarch64 => "aarch64-unknown-linux-gnu",
        Platform::MacOsX86_64 => "x86_64-apple-darwin",
        Platform::MacOsAarch64 => "aarch64-apple-darwin",
        Platform::WindowsX86_64 => "x86_64-pc-windows-msvc",
    }
}

/// Inverse of [`platform_token`]. Total over the five known triples.
pub fn platform_from_token(token: &str) -> Option<Platform> {
    ALL_PLATFORMS
        .iter()
        .copied()
        .find(|p| platform_token(*p) == token)
}

/// Parse either the kebab platform name (`linux-x86-64`) or the Rust triple.
pub fn parse_platform(raw: &str) -> Option<Platform> {
    if let Some(p) = platform_from_token(raw) {
        return Some(p);
    }
    ALL_PLATFORMS.iter().copied().find(|p| p.to_string() == raw)
}

// ---------------------------------------------------------------------------
// Layers and kinds (e84 WU3: Layer 0 vs Layer 1, and product surface)
// ---------------------------------------------------------------------------

/// Which ownership layer an artifact belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// Installing `cogh` itself. Owned by an external channel.
    Layer0Boot,
    /// The CogniCode runtime that `cogh` installs. Owned by `cogh`.
    Layer1Runtime,
    /// Release metadata. Neither layer; never a runtime component.
    Meta,
}

/// What kind of thing an artifact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Cogh,
    Cognicode,
    DaemonCli,
    ExplorerApi,
    BundleManifest,
    ReleaseInventory,
    Checksums,
}

impl ArtifactKind {
    /// Whether this kind is something `cogh` may install.
    pub fn is_installable(self) -> bool {
        matches!(self.layer(), Layer::Layer1Runtime)
    }

    pub fn layer(self) -> Layer {
        match self {
            ArtifactKind::Cogh => Layer::Layer0Boot,
            ArtifactKind::Cognicode | ArtifactKind::DaemonCli | ArtifactKind::ExplorerApi => {
                Layer::Layer1Runtime
            }
            ArtifactKind::BundleManifest
            | ArtifactKind::ReleaseInventory
            | ArtifactKind::Checksums => Layer::Meta,
        }
    }

    /// The canonical filename stem. This is the component name, and it is what
    /// appears in the derived artifact filename.
    pub fn stem(self) -> &'static str {
        match self {
            ArtifactKind::Cogh => "cogh",
            ArtifactKind::Cognicode => "cognicode",
            ArtifactKind::DaemonCli => "cognicode-mcp",
            ArtifactKind::ExplorerApi => "explorer-api",
            ArtifactKind::BundleManifest => "bundle",
            ArtifactKind::ReleaseInventory => "release-inventory",
            ArtifactKind::Checksums => "SHA256SUMS",
        }
    }
}

/// A published product component: the authoritative e85 product surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSpec {
    pub kind: ArtifactKind,
    /// Profiles that include this component. Empty for Layer 0.
    pub profiles: &'static [&'static str],
    /// Whether e85 actually builds and publishes it.
    pub published: bool,
    /// Why it is or is not published.
    pub rationale: &'static str,
}

/// The install profiles e85 publishes.
///
/// `full` is deliberately absent: e84 WU3 says to omit it honestly rather than
/// publish phantom entries for skills/sandbox assets that are not produced.
pub const PUBLISHED_PROFILES: &[(&str, &str)] = &[
    ("core", "Daily CLI"),
    ("reviewer", "Adds the MCP bridge daemon for IDE integration"),
];

/// The product surface. One table, no second copy.
pub const COMPONENTS: &[ComponentSpec] = &[
    ComponentSpec {
        kind: ArtifactKind::Cogh,
        profiles: &[],
        published: true,
        rationale: "Layer 0 bootstrap. Required to install anything at all.",
    },
    ComponentSpec {
        kind: ArtifactKind::Cognicode,
        profiles: &["core", "reviewer"],
        published: true,
        rationale: "The daily CLI. Required Layer 1 component for e85.",
    },
    ComponentSpec {
        kind: ArtifactKind::DaemonCli,
        profiles: &["reviewer"],
        published: true,
        rationale: "MCP bridge daemon; the immediate CLI/MCP goal of e85.",
    },
    ComponentSpec {
        kind: ArtifactKind::ExplorerApi,
        profiles: &["reviewer"],
        published: false,
        rationale: "PUBLIC PRODUCT but not in the e85 Tier-1 surface; ships in a later profile once its own profile semantics are decided.",
    },
];

/// Look up a component by its canonical stem.
pub fn component_by_stem(stem: &str) -> Option<&'static ComponentSpec> {
    COMPONENTS.iter().find(|c| c.kind.stem() == stem)
}

/// The components published in e85, in stable order.
pub fn published_components() -> impl Iterator<Item = &'static ComponentSpec> {
    COMPONENTS.iter().filter(|c| c.published)
}

// ---------------------------------------------------------------------------
// Portable skill bundles (DEBT-2c)
//
// Skill bundles are deliberately NOT `ArtifactKind`s (the ADR forbids
// widening that closed set without a new ADR). They are a parallel
// concept owned by `SkillBundleId` (ADR-IDENTITY-MAP §3.5). This table
// is the single source of truth for which portable skill bundles a
// release ships; the generated manifests declare them by id in
// `skill_bundles[]` and the payloads are named
// `{SkillBundleId}-{version}.tar.gz` (platform-less by design — the
// bundle content is IDE-agnostic and platform-neutral).
// ---------------------------------------------------------------------------

/// One published portable skill bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillBundleSpec {
    /// SkillBundleId. Also the payload filename stem and the directory
    /// under `versions/<v>/skills/` at install time.
    pub id: &'static str,
    /// Profiles that ship this bundle.
    pub profiles: &'static [&'static str],
    /// Whether the release actually publishes it.
    pub published: bool,
}

/// The published skill bundle surface.
pub const SKILL_BUNDLES: &[SkillBundleSpec] = &[
    SkillBundleSpec {
        id: "cognicode",
        profiles: &["core", "reviewer"],
        published: true,
    },
    SkillBundleSpec {
        id: "cognicode-mcp",
        profiles: &["reviewer"],
        published: true,
    },
    SkillBundleSpec {
        id: "cognicode-developer",
        profiles: &[],
        published: false,
    },
];

/// Look up a skill bundle by id.
pub fn skill_bundle_by_id(id: &str) -> Option<&'static SkillBundleSpec> {
    SKILL_BUNDLES.iter().find(|s| s.id == id)
}

/// The published skill bundles, in stable order.
pub fn published_skill_bundles() -> impl Iterator<Item = &'static SkillBundleSpec> {
    SKILL_BUNDLES.iter().filter(|s| s.published)
}

/// Canonical payload filename for a skill bundle at a version.
///
/// Platform-less: portable skill bundles are IDE-agnostic and
/// platform-neutral (`just bundle-skills` produces exactly this shape).
pub fn skill_bundle_filename(id: &str, version: &str) -> String {
    format!("{id}-{version}.tar.gz")
}

/// Whether a filename is a skill bundle payload for `version` (exact,
/// table-derived match — no inference from component stems).
pub fn is_skill_bundle_payload(name: &str, version: &str) -> bool {
    published_skill_bundles().any(|spec| skill_bundle_filename(spec.id, version) == name)
}

// ---------------------------------------------------------------------------
// Derived names and URLs (e84 R1)
// ---------------------------------------------------------------------------

/// The canonical artifact filename for a component at a version on a platform.
///
/// Derived, never typed. One component per artifact (e84 R2).
pub fn artifact_filename(stem: &str, version: &str, platform: Platform) -> String {
    format!("{stem}-{version}-{}.tar.gz", platform_token(platform))
}

/// The canonical GitHub Release download URL for an artifact.
pub fn artifact_url(version: &str, filename: &str) -> String {
    format!("{RELEASE_DOWNLOAD_BASE}/v{version}/{filename}")
}

/// The canonical filename for the generated per-platform bundle manifest.
pub fn bundle_manifest_filename(version: &str, platform: Platform) -> String {
    format!("bundle-{version}-{}.yaml", platform_token(platform))
}

/// The canonical filename for the release inventory.
pub fn release_inventory_filename(version: &str) -> String {
    format!("release-inventory-{version}.json")
}

// ---------------------------------------------------------------------------
// Digests (e84 R5: computed, never literal)
// ---------------------------------------------------------------------------

/// A validated SHA256 digest, lowercase hex, 64 characters, not a placeholder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactDigest(String);

impl ArtifactDigest {
    /// Parse and validate a digest string.
    ///
    /// Rejects: wrong length, non-hex, all-same-character, and any value small
    /// enough to be a hand-written sequence such as `0000…0001`.
    pub fn parse(raw: &str) -> Result<Self> {
        let s = raw.trim().to_ascii_lowercase();
        if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("digest must be 64 lowercase hex chars, got `{raw}`");
        }
        if is_placeholder_digest(&s) {
            bail!(
                "digest `{raw}` is a placeholder, not a computed SHA256; \
                 a manifest may only carry digests computed from the packaged bytes"
            );
        }
        Ok(ArtifactDigest(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ArtifactDigest {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        ArtifactDigest::parse(&value)
    }
}

impl From<ArtifactDigest> for String {
    fn from(value: ArtifactDigest) -> String {
        value.0
    }
}

/// Detect a hand-written placeholder digest.
///
/// Two independent rules, either of which is disqualifying:
///   1. every character identical (`0000…`, `ffff…`);
///   2. numerically tiny, i.e. at least 60 leading zero bits. No honest SHA256
///      has that, and it is exactly the shape of `0000…0001/0002/0003`.
pub fn is_placeholder_digest(hex: &str) -> bool {
    if hex.is_empty() {
        return true;
    }
    let mut chars = hex.chars();
    let first = chars.next().unwrap();
    if chars.all(|c| c == first) {
        return true;
    }
    match u128::from_str_radix(hex, 16) {
        Ok(value) => value < MIN_PLAUSIBLE_DIGEST_VALUE,
        // Longer than 32 hex chars: cannot parse as u128, so not tiny.
        Err(_) => false,
    }
}

/// SHA256 of a file, streamed, lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::Digest as _;
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open {} for hashing", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ---------------------------------------------------------------------------
// ProducedArtifact ledger (e84 WU5) and ReleaseInventory (e84 WU6)
// ---------------------------------------------------------------------------

/// One produced payload artifact, with its digest computed from the final bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducedArtifact {
    pub kind: ArtifactKind,
    /// Canonical component name (== `kind.stem()`).
    pub component_name: String,
    pub version: String,
    pub platform: Platform,
    pub platform_token: String,
    pub filename: String,
    pub sha256: ArtifactDigest,
    /// Byte length, so re-downloads can be sanity-checked cheaply.
    pub size: u64,
    /// Whether `cogh` may install this (Layer 1 only).
    pub installable: bool,
    /// Profiles that include it. Empty for Layer 0 and meta artifacts.
    pub profiles: Vec<String>,
}

impl ProducedArtifact {
    /// The canonical URL a consumer would use.
    pub fn url(&self) -> String {
        artifact_url(&self.version, &self.filename)
    }
}

/// The release-wide inventory: everything published, across both layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInventory {
    pub version: String,
    pub tag: String,
    pub source_commit: String,
    pub artifacts: Vec<ProducedArtifact>,
}

impl ReleaseInventory {
    pub fn for_platform(&self, platform: Platform) -> impl Iterator<Item = &ProducedArtifact> {
        self.artifacts
            .iter()
            .filter(move |a| a.platform == platform)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str(s).context("parse release inventory JSON")
    }
}

// ---------------------------------------------------------------------------
// Staging scan: filename -> ledger
// ---------------------------------------------------------------------------

/// Classify a canonical artifact filename into `(stem, version, platform)`.
///
/// Returns `None` for anything that is not exactly a canonical payload name.
pub fn classify_artifact_filename(name: &str) -> Option<(String, String, Platform)> {
    let stem_with_version = name.strip_suffix(".tar.gz")?;
    // Longest stem first: `cognicode-mcp` must win over `cognicode`, otherwise
    // `cognicode-mcp-0.95.0-<token>` parses as component `cognicode` with
    // version `mcp-0.95.0`.
    let mut stems: Vec<&'static str> = COMPONENTS.iter().map(|c| c.kind.stem()).collect();
    stems.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for stem in stems {
        let prefix = format!("{stem}-");
        let Some(rest) = stem_with_version.strip_prefix(&prefix) else {
            continue;
        };
        // Longest-token-first so `x86_64-unknown-linux-gnu` wins over any
        // shorter suffix that happens to be a substring.
        for platform in ALL_PLATFORMS {
            let token = platform_token(platform);
            if let Some(version) = rest.strip_suffix(&format!("-{token}"))
                && !version.is_empty()
            {
                return Some((stem.to_string(), version.to_string(), platform));
            }
        }
    }
    None
}

/// Build the ledger for one artifact file, hashing its final bytes.
pub fn produced_artifact_from_file(path: &Path, version: &str) -> Result<ProducedArtifact> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("artifact path has no filename")?
        .to_string();

    let (stem, file_version, platform) =
        classify_artifact_filename(&filename).with_context(|| {
            format!(
                "`{filename}` is not a canonical artifact name; \
                 names are derived as {{component}}-{{version}}-{{platform_token}}.tar.gz"
            )
        })?;

    let spec =
        component_by_stem(&stem).with_context(|| format!("`{stem}` is not a known component"))?;

    if file_version != *version {
        bail!(
            "artifact `{filename}` declares version `{file_version}` but the \
             release version is `{version}`"
        );
    }

    let canonical = artifact_filename(spec.kind.stem(), version, platform);
    if canonical != filename {
        bail!("artifact `{filename}` is not canonical; expected `{canonical}`");
    }

    let sha256 = ArtifactDigest::parse(&sha256_file(path)?)?;
    let size = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .len();

    Ok(ProducedArtifact {
        kind: spec.kind,
        component_name: spec.kind.stem().to_string(),
        version: version.to_string(),
        platform,
        platform_token: platform_token(platform).to_string(),
        filename,
        sha256,
        size,
        installable: spec.kind.is_installable(),
        profiles: spec.profiles.iter().map(|p| p.to_string()).collect(),
    })
}

/// Scan a staging directory and build the release-wide inventory.
///
/// Fails loudly on: duplicates, orphans (published component missing for a
/// platform), unknown filenames, and version mismatches.
pub fn build_inventory(
    staging: &Path,
    version: &str,
    tag: &str,
    source_commit: &str,
    expected_platforms: &[Platform],
) -> Result<ReleaseInventory> {
    let mut artifacts: Vec<ProducedArtifact> = Vec::new();
    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();

    let entries = std::fs::read_dir(staging)
        .with_context(|| format!("read staging dir {}", staging.display()))?;

    for entry in entries {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Only payload archives participate in the scan; manifest/inventory
        // files are generated, not scanned. Platform-less skill bundle
        // payloads are their own class (DEBT-2c): recognized by the
        // SKILL_BUNDLES table, not treated as component artifacts.
        if !name.ends_with(".tar.gz") {
            continue;
        }
        if is_skill_bundle_payload(name, version) {
            continue;
        }
        if let Some(previous) = seen.insert(name.to_string(), path.clone()) {
            bail!(
                "duplicate artifact `{name}`: produced twice ({}, {})",
                previous.display(),
                path.display()
            );
        }
        artifacts.push(produced_artifact_from_file(&path, version)?);
    }

    // Orphans: every published component must exist for every expected platform.
    for platform in expected_platforms {
        for spec in published_components() {
            let filename = artifact_filename(spec.kind.stem(), version, *platform);
            if !seen.contains_key(&filename) {
                bail!(
                    "missing artifact `{filename}`: component `{}` is published \
                     but was not produced for platform `{platform}`",
                    spec.kind.stem()
                );
            }
        }
    }

    // Duplicate (component, platform) pairs cannot happen given the filename
    // key, but assert it explicitly because it is a contract invariant.
    let mut pairs: BTreeMap<(String, Platform), usize> = BTreeMap::new();
    for a in &artifacts {
        *pairs
            .entry((a.component_name.clone(), a.platform))
            .or_default() += 1;
    }
    if let Some(((name, platform), count)) = pairs.into_iter().find(|(_, c)| *c > 1) {
        bail!("component `{name}` for platform `{platform}` appears {count} times");
    }

    artifacts.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(ReleaseInventory {
        version: version.to_string(),
        tag: tag.to_string(),
        source_commit: source_commit.to_string(),
        artifacts,
    })
}

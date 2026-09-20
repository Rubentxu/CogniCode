//! `cogh::ide` — IDE adapter plugins (E32-D through E32-G).
//!
// Spec: `openspec/specs/cognicode-ide-adapter/spec.md`.
//!
// Each IDE adapter knows how to:
//! 1. Detect whether the IDE is installed
//! 2. Integrate: patch the IDE's MCP config + copy skill bundles
//! 3. Uninstall: remove the MCP entry + clean up skills
//!
// The adapter is a `cogh` plugin (same as mcp-server). Its
//! `integrate` and `uninstall` steps are implemented in cogh itself
//! (not as a separate plugin binary) — the plugin manifest just
//! declares the steps.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::platform_adapter;

/// A step in an integrate / uninstall recipe.
#[derive(Debug, Clone)]
pub enum Step {
    /// Copy a file or directory tree to a target.
    Copy { source: PathBuf, target: PathBuf },
    /// Recursively remove a directory.
    RmRf { target: PathBuf },
    /// Merge a JSON value into a config at a dot-path key.
    MergeJson {
        target: PathBuf,
        path: Vec<String>,
        value: Value,
    },
    /// Remove a JSON key at a dot-path.
    RemoveFromJson { target: PathBuf, path: Vec<String> },
    /// Create a symbolic link from source to target.
    Symlink { source: PathBuf, target: PathBuf },
}

impl Step {
    /// Execute a single integration step.
    pub fn execute(&self) -> Result<()> {
        match self {
            Step::Copy { source, target } => {
                if source.is_dir() {
                    copy_dir_recursive(source, target)?;
                } else {
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(source, target)?;
                }
                Ok(())
            }
            Step::RmRf { target } => {
                if target.exists() {
                    std::fs::remove_dir_all(target)?;
                }
                Ok(())
            }
            Step::MergeJson {
                target,
                path,
                value,
            } => {
                let mut config = if target.exists() {
                    let text = std::fs::read_to_string(target)?;
                    serde_json::from_str(&text).unwrap_or(json!({}))
                } else {
                    json!({})
                };
                // Navigate to the nested path
                let obj = config.as_object_mut().unwrap();
                let mut current = obj;
                for key in path.iter().take(path.len() - 1) {
                    current = current
                        .entry(key)
                        .or_insert_with(|| json!({}))
                        .as_object_mut()
                        .unwrap();
                }
                if let Some(last_key) = path.last() {
                    current.insert(last_key.clone(), value.clone());
                }
                write_json_atomic(target, &config)?;
                Ok(())
            }
            Step::RemoveFromJson { target, path } => {
                if !target.exists() {
                    return Ok(());
                }
                let text = std::fs::read_to_string(target)?;
                let mut config: Value = serde_json::from_str(&text).unwrap_or(json!({}));
                let obj = config.as_object_mut().unwrap();
                let mut current = obj;
                for key in path.iter().take(path.len() - 1) {
                    current = current
                        .entry(key)
                        .or_insert_with(|| json!({}))
                        .as_object_mut()
                        .unwrap();
                }
                if let Some(last_key) = path.last() {
                    current.remove(last_key);
                }
                write_json_atomic(target, &config)?;
                Ok(())
            }
            Step::Symlink { source, target } => {
                // e74 WU1: route through the platform adapter so that
                // Unix-family hosts symlink and Windows copies. This
                // replaces the previous `cfg(unix)`/`cfg(not(unix))`
                // duplication that mixed business policy with platform
                // mechanics.
                platform_adapter::current_adapter()
                    .link_or_copy(source, target)
                    .map_err(|e| anyhow!("link_or_copy failed: {}", e))?;
                Ok(())
            }
        }
    }
}

/// Detect whether the IDE is installed.
///
/// Uses `Path::is_file()` (not `Path::exists()`) so a directory at the
/// config path is NOT mistakenly detected as a configured IDE. Pinned by
/// `t_e86_5_detect_opencode_returns_false_when_config_is_directory`
/// (RED before E86.5).
pub fn detect_opencode() -> bool {
    OpenCodePaths::resolve().config_file.is_file()
}

/// OpenCode ownership root: the directory the `OPENCODE_CONFIG` file lives in
/// (or would live in by default). Skills are derived from this single root,
/// never from `$HOME` independently — that drift is what produced the
/// E86.2.2 real-PC UAT finding.
///
/// Default location (when `OPENCODE_CONFIG` is unset) is
/// `$HOME/.config/opencode/`. The default config file is
/// `opencode.json` inside that directory.
#[derive(Debug, Clone)]
pub struct OpenCodePaths {
    /// The on-disk path of `opencode.json`. May not exist yet.
    pub config_file: PathBuf,
    /// The directory the config file lives in. Skills directory is derived
    /// from this single root.
    pub config_dir: PathBuf,
    /// The skills directory, sibling-derived from `config_dir` per the
    /// OpenCode on-disk layout.
    pub skills_dir: PathBuf,
}

impl OpenCodePaths {
    /// Resolve the OpenCode ownership root from `OPENCODE_CONFIG` (preferred)
    /// or `$HOME/.config/opencode/` (default).
    ///
    /// Skills are derived from the **same** root the config file lives in,
    /// never from `$HOME` independently. If `OPENCODE_CONFIG` is set, both
    /// config and skills live under its parent directory by design.
    pub fn resolve() -> Self {
        let (config_file, config_dir) = if let Ok(env) = std::env::var("OPENCODE_CONFIG") {
            let path = PathBuf::from(env);
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (path, dir)
        } else {
            let home = std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("~/.config/opencode"));
            let dir = home.join(".config/opencode");
            (dir.join("opencode.json"), dir)
        };
        let skills_dir = config_dir.join("skills");
        Self {
            config_file,
            config_dir,
            skills_dir,
        }
    }
}

pub fn opencode_config_path() -> PathBuf {
    OpenCodePaths::resolve().config_file
}

pub fn opencode_skills_dir() -> PathBuf {
    OpenCodePaths::resolve().skills_dir
}

/// Read the OpenCode config as JSON.
pub fn read_opencode_config() -> Result<Value> {
    let path = opencode_config_path();
    if !path.exists() {
        return Ok(json!({}));
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let v: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

/// Atomic JSON write (tmp + rename).
///
/// DEBT-1: if the file already exists and parses to a JSON value that is
/// *semantically equal* to `value`, the write is skipped entirely — no
/// content change, no mtime bump, no inode churn ("HOME zero-touch" for
/// repeated installs). Semantic equality is structural JSON equality
/// (`serde_json::Value`), not byte equality; key order and formatting
/// differences do not count as changes. A real semantic change still
/// goes through the atomic tmp+rename path.
fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    if path.exists()
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(current) = serde_json::from_str::<Value>(&text)
        && current == *value
    {
        return Ok(());
    }
    let tmp = path.with_extension("json.tmp");
    if let Some(parent) = tmp.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let json_str = serde_json::to_string_pretty(value)
        .with_context(|| format!("serialize {}", path.display()))?;
    std::fs::write(&tmp, json_str).with_context(|| format!("write tmp {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

/// Derive the BinaryName (the JSON / TOML merge key) from the
/// `mcp_command` argument that `cmd_ide_install` already resolves.
///
/// DEBT-3.f: replaces the hardcoded `"cognicode-mcp"` literal that
/// the four IDE integrators (opencode, zcode, claude, codex) used
/// to pass as the merge key when patching `mcp.cognicode-mcp`,
/// `.claude/mcp/cognicode-mcp.json`, or
/// `[mcp_servers.cognicode-mcp]`. The literal silently coupled
/// BinaryName to a specific component name; if a future plugin
/// declared a different binary, the IDE integration would write
/// to the wrong key.
///
/// The shim path is the canonical source of truth for the
/// BinaryName at install time: `cmd_ide_install` already resolves
/// it via `plugin_mcp_binary_name(...)` and embeds it as the
/// basename of `mcp_command[0]`. We extract it from there rather
/// than re-reading the manifest — the manifest-derived name is
/// authoritative, and the shim path is its on-disk reflection.
///
/// Fails loudly when `mcp_command` is empty or the shim path has
/// no usable filename.
fn mcp_merge_key_from_command(mcp_command: &[String]) -> Result<String> {
    let shim_path_str = mcp_command.first().ok_or_else(|| {
        anyhow!(
            "mcp_command must contain at least one element (the shim path); \
             got an empty command"
        )
    })?;
    let shim_path = Path::new(shim_path_str);
    shim_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow!(
                "mcp_command[0] ({}) has no usable filename; cannot derive merge key",
                shim_path.display()
            )
        })
}

/// Build the opencode adapter's `integrate` recipe as steps.
pub fn integrate_opencode(
    skill_path: &Path,
    version: &str,
    mcp_command: &[String],
) -> Result<Vec<Step>> {
    let mut steps = Vec::new();

    // 1. Symlink skill bundle to OpenCode skills directory
    let skills_target = opencode_skills_dir().join(format!("cognicode-{version}"));
    steps.push(Step::Symlink {
        source: skill_path.to_path_buf(),
        target: skills_target,
    });

    // 2. MCP config merge. DEBT-2c: a profile may ship skill bundles
    //    without a DaemonCli component; in that case `mcp_command` is
    //    empty and only the skills symlink is installed — no MCP entry.
    if !mcp_command.is_empty() {
        let config_path = opencode_config_path();
        let mcp_entry = json!({
            "command": mcp_command,
            "enabled": true,
            "type": "stdio",
        });
        // DEBT-3.f: derive the merge key from `mcp_command[0]` (the
        // shim path), not from a hardcoded `"cognicode-mcp"` literal.
        let binary_name = mcp_merge_key_from_command(mcp_command)?;
        steps.push(Step::MergeJson {
            target: config_path,
            path: vec!["mcp".to_string(), binary_name],
            value: mcp_entry,
        });
    }

    Ok(steps)
}

/// Build the opencode adapter's `uninstall` recipe as steps.
pub fn uninstall_opencode(version: &str, binary_name: &str) -> Result<Vec<Step>> {
    let mut steps = Vec::new();

    // 1. Remove skills symlink
    let skills_target = opencode_skills_dir().join(format!("cognicode-{version}"));
    steps.push(Step::RmRf {
        target: skills_target,
    });

    // 2. Remove MCP entry. DEBT-3.f: take the BinaryName from the
    //    bundle manifest's DaemonCli component, not from a
    //    hardcoded `"cognicode-mcp"` literal. The literal silently
    //    coupled BinaryName to a specific component name; the
    //    manifest-derived name is the source of truth.
    let config_path = opencode_config_path();
    steps.push(Step::RemoveFromJson {
        target: config_path,
        path: vec!["mcp".to_string(), binary_name.to_string()],
    });

    Ok(steps)
}

/// Copy a directory tree recursively.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst)
            .with_context(|| format!("rm -rf existing {}", dst.display()))?;
    }
    std::fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for entry in std::fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| format!("copy {}", src_path.display()))?;
        }
    }
    Ok(())
}

// ===== ZCode adapter (E32-E) =====

pub fn detect_zcode() -> bool {
    ZCodePaths::resolve().config_file.is_file()
}

/// ZCode ownership root. Single source of truth for both the config file
/// (driven by `ZCODE_CONFIG`) and the skills directory (derived from the
/// same root). See [`OpenCodePaths`] for the analogous OpenCode struct.
#[derive(Debug, Clone)]
pub struct ZCodePaths {
    pub config_file: PathBuf,
    pub config_dir: PathBuf,
    pub skills_dir: PathBuf,
}

impl ZCodePaths {
    pub fn resolve() -> Self {
        let (config_file, config_dir) = if let Ok(env) = std::env::var("ZCODE_CONFIG") {
            let path = PathBuf::from(env);
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (path, dir)
        } else {
            let home = std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("~/.zcode"));
            let dir = home.join(".zcode");
            (dir.join("v2/config.json"), dir)
        };
        let skills_dir = config_dir.join("skills");
        Self {
            config_file,
            config_dir,
            skills_dir,
        }
    }
}

pub fn zcode_config_path() -> PathBuf {
    ZCodePaths::resolve().config_file
}

pub fn zcode_skills_dir() -> PathBuf {
    ZCodePaths::resolve().skills_dir
}

pub fn read_zcode_config() -> Result<Value> {
    let path = zcode_config_path();
    if !path.exists() {
        return Ok(json!({}));
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let v: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

pub fn integrate_zcode(
    home: &Path,
    plugin: &str,
    version: &str,
    mcp_command: &[String],
) -> Result<()> {
    // 1. Skill copy.
    //
    // DEBT-2: the SkillBundleId comes from the bundle manifest's
    // `skill_bundles[]` declaration, resolved canonically by
    // `crate::bundle_manifest::declared_skill_bundle_dirs` against the
    // version's canonical `skills/` root. The retired heuristic
    // (`<root>/versions/<v>/<plugin>/skills/`, ADR-IDENTITY-MAP
    // §9.4-11) is gone: a bundle not declared in the manifest is never
    // picked up, and a declared-but-missing bundle fails loudly.
    // The `&Path` argument is preserved (instead of taking
    // `&CognicodeHome`) so the `cognicode-release` binary — which does
    // not link `layout.rs` — can keep calling this function with a
    // borrowed path.
    let skills_sources = crate::bundle_manifest::declared_skill_bundle_dirs(
        &home.join("versions").join(version).join("skills"),
        &home.join("versions").join(version).join("manifest.yaml"),
        "core",
    )?;
    let skills_dst = zcode_skills_dir().join(format!("cognicode-{version}"));
    if !skills_sources.is_empty() {
        for skills_src in skills_sources {
            copy_dir_recursive(&skills_src, &skills_dst).with_context(|| {
                format!("copy {} → {}", skills_src.display(), skills_dst.display())
            })?;
            println!("✓ copied skills: {}", skills_dst.display());
        }
    } else {
        println!(
            "(no skill bundles declared for this release; nothing copied to {})",
            skills_dst.display()
        );
    }

    // 2. MCP config merge
    let config_path = zcode_config_path();
    let mut config = read_zcode_config()?;
    let mcp_entry = json!({
        "command": mcp_command,
        "enabled": true,
        "type": "stdio",
    });

    let cfg = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("zcode config.json is not an object"))?;
    let mcp = cfg.entry("mcp").or_insert_with(|| json!({}));
    if !mcp.is_object() {
        return Err(anyhow!(
            "zcode config.json: 'mcp' is not an object (got {})",
            mcp
        ));
    }
    // DEBT-3.f: derive the merge key from `mcp_command[0]`
    // (the shim path), not from a hardcoded `"cognicode-mcp"`
    // literal.
    let binary_name = mcp_merge_key_from_command(mcp_command)?;
    mcp.as_object_mut().unwrap().insert(binary_name, mcp_entry);
    write_json_atomic(&config_path, &config)?;
    println!("✓ patched: {}", config_path.display());

    Ok(())
}

pub fn uninstall_zcode(version: &str, binary_name: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = zcode_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP entry. DEBT-3.f: take the BinaryName from
    //    the bundle manifest's DaemonCli component, not from a
    //    hardcoded `"cognicode-mcp"` literal.
    let config_path = zcode_config_path();
    if config_path.exists() {
        let mut config = read_zcode_config()?;
        let cfg = config
            .as_object_mut()
            .ok_or_else(|| anyhow!("zcode config.json is not an object"))?;
        if let Some(mcp) = cfg.get_mut("mcp")
            && let Some(mcp_obj) = mcp.as_object_mut()
        {
            mcp_obj.remove(binary_name);
        }
        write_json_atomic(&config_path, &config)?;
        println!("✓ unpached: {}", config_path.display());
    }

    Ok(())
}

// ===== Claude Code adapter (E32-F) =====

pub fn detect_claude() -> bool {
    let mcp_dir = claude_mcp_dir();
    mcp_dir.exists()
}

pub fn claude_config_path() -> PathBuf {
    if let Ok(env) = std::env::var("CLAUDE_CONFIG") {
        return PathBuf::from(env);
    }
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return PathBuf::from("~/.claude"),
    };
    home.join(".claude")
}

pub fn claude_mcp_dir() -> PathBuf {
    claude_config_path().join("mcp")
}

pub fn claude_skills_dir() -> PathBuf {
    claude_config_path().join("skills")
}

pub fn read_claude_mcp_entry(name: &str) -> Result<Option<Value>> {
    let path = claude_mcp_dir().join(format!("{name}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let v: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(v))
}

pub fn integrate_claude(
    home: &Path,
    plugin: &str,
    version: &str,
    mcp_command: &[String],
) -> Result<()> {
    // 1. Skill copy.
    //
    let skills_sources = crate::bundle_manifest::declared_skill_bundle_dirs(
        &home.join("versions").join(version).join("skills"),
        &home.join("versions").join(version).join("manifest.yaml"),
        "core",
    )?;
    let skills_dst = claude_skills_dir().join(format!("cognicode-{version}"));
    if !skills_sources.is_empty() {
        for skills_src in skills_sources {
            copy_dir_recursive(&skills_src, &skills_dst).with_context(|| {
                format!("copy {} → {}", skills_src.display(), skills_dst.display())
            })?;
            println!("✓ copied skills: {}", skills_dst.display());
        }
    } else {
        println!(
            "(no skill bundles declared for this release; nothing copied to {})",
            skills_dst.display()
        );
    }

    // 2. MCP config: write `~/.claude/mcp/<binary_name>.json`.
    //    DEBT-3.f: derive the file stem from `mcp_command[0]`
    //    (the shim path), not from a hardcoded `"cognicode-mcp"`
    //    literal.
    let mcp_dir = claude_mcp_dir();
    std::fs::create_dir_all(&mcp_dir).with_context(|| format!("create {}", mcp_dir.display()))?;
    let binary_name = mcp_merge_key_from_command(mcp_command)?;
    let target = mcp_dir.join(format!("{binary_name}.json"));
    let entry = json!({
        "command": mcp_command[0],
        "args": mcp_command.get(1..).unwrap_or(&[]).to_vec(),
    });
    write_json_atomic(&target, &entry)?;
    println!("✓ patched: {}", target.display());

    Ok(())
}

pub fn uninstall_claude(version: &str, binary_name: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = claude_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP file. DEBT-3.f: take the file stem from the
    //    bundle manifest's DaemonCli component, not from a
    //    hardcoded `"cognicode-mcp"` literal.
    let target = claude_mcp_dir().join(format!("{binary_name}.json"));
    if target.exists() {
        std::fs::remove_file(&target).with_context(|| format!("rm {}", target.display()))?;
        println!("✓ removed: {}", target.display());
    }

    Ok(())
}

// ===== Codex adapter (E32-G) =====

pub fn detect_codex() -> bool {
    CodexPaths::resolve().config_file.is_file()
}

/// Codex ownership root. Single source of truth for both the config file
/// (driven by `CODEX_CONFIG`) and the skills directory (derived from the
/// same root). See [`OpenCodePaths`] for the analogous OpenCode struct.
#[derive(Debug, Clone)]
pub struct CodexPaths {
    pub config_file: PathBuf,
    pub config_dir: PathBuf,
    pub skills_dir: PathBuf,
}

impl CodexPaths {
    pub fn resolve() -> Self {
        let (config_file, config_dir) = if let Ok(env) = std::env::var("CODEX_CONFIG") {
            let path = PathBuf::from(env);
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (path, dir)
        } else {
            let home = std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("~/.codex"));
            let dir = home.join(".codex");
            (dir.join("config.toml"), dir)
        };
        let skills_dir = config_dir.join("skills");
        Self {
            config_file,
            config_dir,
            skills_dir,
        }
    }
}

pub fn codex_config_path() -> PathBuf {
    CodexPaths::resolve().config_file
}

pub fn codex_skills_dir() -> PathBuf {
    CodexPaths::resolve().skills_dir
}

pub fn read_codex_config() -> Result<toml::Value> {
    let path = codex_config_path();
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let v: toml::Value = text
        .parse()
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

pub fn integrate_codex(
    home: &Path,
    plugin: &str,
    version: &str,
    mcp_command: &[String],
) -> Result<()> {
    // 1. Skill copy.
    //
    let skills_sources = crate::bundle_manifest::declared_skill_bundle_dirs(
        &home.join("versions").join(version).join("skills"),
        &home.join("versions").join(version).join("manifest.yaml"),
        "core",
    )?;
    let skills_dst = codex_skills_dir().join(format!("cognicode-{version}"));
    if !skills_sources.is_empty() {
        for skills_src in skills_sources {
            copy_dir_recursive(&skills_src, &skills_dst).with_context(|| {
                format!("copy {} → {}", skills_src.display(), skills_dst.display())
            })?;
            println!("✓ copied skills: {}", skills_dst.display());
        }
    } else {
        println!(
            "(no skill bundles declared for this release; nothing copied to {})",
            skills_dst.display()
        );
    }

    // 2. Codex config: TOML, [mcp_servers.<binary_name>] section.
    //    DEBT-3.f: derive the subtable key from `mcp_command[0]`
    //    (the shim path), not from a hardcoded `"cognicode-mcp"`
    //    literal.
    let config_path = codex_config_path();
    let mut config = read_codex_config()?;
    let mcp_table = config
        .as_table_mut()
        .ok_or_else(|| anyhow!("codex config.toml is not a table"))?
        .entry("mcp_servers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let mcp_table = mcp_table
        .as_table_mut()
        .ok_or_else(|| anyhow!("mcp_servers is not a table"))?;
    // Codex convention: each MCP server is a subtable with command + args
    let cmd = mcp_command.first().map(String::as_str).unwrap_or("");
    let args_value = mcp_command
        .get(1..)
        .map(|rest| {
            let arr: Vec<toml::Value> = rest.iter().cloned().map(toml::Value::String).collect();
            toml::Value::Array(arr)
        })
        .unwrap_or_else(|| toml::Value::Array(Vec::new()));
    let server = toml::Value::Table(toml::map::Map::from_iter([
        ("command".to_string(), toml::Value::String(cmd.to_string())),
        ("args".to_string(), args_value),
    ]));
    let binary_name = mcp_merge_key_from_command(mcp_command)?;
    mcp_table.insert(binary_name, server);

    // Atomic write
    let tmp = config_path.with_extension("toml.tmp");
    if let Some(parent) = tmp.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let s = toml::to_string_pretty(&config)
        .with_context(|| format!("serialize {}", config_path.display()))?;
    std::fs::write(&tmp, s).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &config_path)
        .with_context(|| format!("rename {}", config_path.display()))?;
    println!("✓ patched: {}", config_path.display());

    Ok(())
}

pub fn uninstall_codex(version: &str, binary_name: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = codex_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP entry from TOML config. DEBT-3.f: take the
    //    subtable key from the bundle manifest's DaemonCli
    //    component, not from a hardcoded `"cognicode-mcp"` literal.
    let config_path = codex_config_path();
    if config_path.exists() {
        let mut config = read_codex_config()?;
        if let Some(t) = config.as_table_mut()
            && let Some(mcp_servers) = t.get_mut("mcp_servers")
            && let Some(mcp_table) = mcp_servers.as_table_mut()
        {
            mcp_table.remove(binary_name);
        }
        let tmp = config_path.with_extension("toml.tmp");
        if let Some(parent) = tmp.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let s = toml::to_string_pretty(&config)
            .with_context(|| format!("serialize {}", config_path.display()))?;
        std::fs::write(&tmp, s).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &config_path)
            .with_context(|| format!("rename {}", config_path.display()))?;
        println!("✓ unpached: {}", config_path.display());
    }

    Ok(())
}

// ===== CLI handlers =====

pub fn cmd_ide_detect() -> Result<()> {
    println!("Detected IDEs:");
    if detect_opencode() {
        println!("  ✓ opencode ({})", opencode_config_path().display());
    } else {
        println!("  ✗ opencode (config not found)");
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        let zcode = home.join(".zcode/v2/config.json");
        if zcode.exists() {
            println!("  ✓ zcode ({})", zcode.display());
        } else {
            println!("  ✗ zcode (config not found)");
        }
        let claude = home.join(".claude/claude_desktop_config.json");
        if claude.exists() {
            println!(
                "    (claude config present at {}, but adapter is E32-F)",
                claude.display()
            );
        }
        let codex = home.join(".codex/config.json");
        if codex.exists() {
            println!(
                "    (codex config present at {}, but adapter is E32-G)",
                codex.display()
            );
        }
    }
    Ok(())
}

pub fn cmd_ide_install(
    home: &CognicodeHomeSup,
    ide: &str,
    plugin: &str,
    version: &str,
) -> Result<()> {
    // Resolve the MCP command line. The BinaryName comes from the
    // plugin manifest's declared `binaries[].name`, not from a
    // hardcoded `"cognicode-mcp"` literal. The literal silently
    // coupled BinaryName to a specific plugin's choice; if a
    // future plugin declared a different binary, the IDE
    // integration would point at a shim that didn't exist.
    // The plugin manifest is the source of truth; if the plugin
    // declares no binaries, fail loudly.
    let mcp_binary_name =
        crate::manifest::plugin_mcp_binary_name(&home.plugin(plugin).join("plugin.yaml"))?;
    let mcp_command = vec![
        home.shim_path(&mcp_binary_name)
            .to_string_lossy()
            .to_string(),
    ];
    match ide {
        "opencode" => {
            // DEBT-2: `skill_path` is resolved from the bundle
            // manifest's `skill_bundles[]` declaration via the
            // canonical `declared_skill_bundle_dirs` helper against
            // `home.skills_root(version)` — the retired
            // `<plugin>/skills` construction (ADR-IDENTITY-MAP
            // §9.4-11, site :737) is gone.
            let skill_sources = crate::bundle_manifest::declared_skill_bundle_dirs(
                &home.skills_root(version),
                &home.version_manifest(version),
                "core",
            )?;
            if skill_sources.is_empty() {
                return Err(anyhow!(
                    "no skill bundles declared for version {version}; cannot integrate \
                     OpenCode without a declared SkillBundleId"
                ));
            }
            for skill_path in skill_sources {
                let steps = integrate_opencode(&skill_path, version, &mcp_command)?;
                for step in steps {
                    step.execute()?;
                }
            }
            println!("✓ OpenCode integration complete");
            Ok(())
        }
        "zcode" => integrate_zcode(home.root.as_path(), plugin, version, &mcp_command),
        "claude" => integrate_claude(home.root.as_path(), plugin, version, &mcp_command),
        "codex" => integrate_codex(home.root.as_path(), plugin, version, &mcp_command),
        other => Err(anyhow!(
            "IDE '{}' is not supported by cogh yet (opencode/zcode/claude/codex in E32-D/E/F/G)",
            other
        )),
    }
}

pub fn cmd_ide_uninstall(home: &CognicodeHomeSup, ide: &str, version: &str) -> Result<()> {
    // DEBT-3.f: derive the BinaryName from the bundle manifest's
    // DaemonCli component, then pass it down to each uninstall
    // path so they remove the right JSON/TOML/file entry rather
    // than blindly targeting a hardcoded `"cognicode-mcp"` key.
    let binary_name =
        crate::bundle_manifest::daemon_cli_binary_name(&home.version_manifest(version))?;
    match ide {
        "opencode" => {
            let steps = uninstall_opencode(version, &binary_name)?;
            for step in steps {
                step.execute()?;
            }
            println!("✓ OpenCode uninstall complete");
            Ok(())
        }
        "zcode" => uninstall_zcode(version, &binary_name),
        "claude" => uninstall_claude(version, &binary_name),
        "codex" => uninstall_codex(version, &binary_name),
        other => Err(anyhow!(
            "IDE '{}' is not supported by cogh yet (opencode/zcode/claude/codex in E32-D/E/F/G)",
            other
        )),
    }
}

// Re-export the home type so we don't have to import from layout.
pub type CognicodeHomeSup = crate::layout::CognicodeHome;

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// DEBT-1: semantic equality, not byte equality. A file with the same
    /// JSON under different formatting/key order must not be rewritten.
    #[test]
    fn t_debt1_write_json_atomic_semantic_equal_different_formatting_skips() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("cfg.json");
        // Compact, keys in non-canonical order.
        std::fs::write(&target, r#"{"b":1,"a":{"z":true,"y":[1,2]}}"#).unwrap();
        filetime::set_file_mtime(&target, filetime::FileTime::from_unix_time(1, 0)).unwrap();
        let meta_before = std::fs::metadata(&target).unwrap();

        let value: Value = serde_json::from_str(r#"{"a":{"y":[1,2],"z":true},"b":1}"#).unwrap();
        write_json_atomic(&target, &value).expect("write must succeed");

        let meta_after = std::fs::metadata(&target).unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            r#"{"b":1,"a":{"z":true,"y":[1,2]}}"#,
            "semantically equal content must be preserved byte-for-byte"
        );
        assert_eq!(
            meta_before.modified().unwrap(),
            meta_after.modified().unwrap(),
            "semantically equal write must not touch mtime"
        );
    }

    /// DEBT-1: a real semantic change still goes through the atomic
    /// tmp+rename path and lands on disk.
    #[test]
    fn t_debt1_write_json_atomic_real_change_writes_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("cfg.json");
        std::fs::write(&target, r#"{"a":1}"#).unwrap();

        let value: Value = serde_json::from_str(r#"{"a":2}"#).unwrap();
        write_json_atomic(&target, &value).expect("write must succeed");

        let text = std::fs::read_to_string(&target).unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["a"], 2, "semantic change must be persisted");
        assert!(
            !target.with_extension("json.tmp").exists(),
            "atomic rename must not leave the tmp file behind"
        );
    }

    /// DEBT-1: an unparseable existing file is not silently preserved —
    /// it is replaced (can't establish semantic equality).
    #[test]
    fn t_debt1_write_json_atomic_unparseable_existing_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("cfg.json");
        std::fs::write(&target, "not json {{{").unwrap();

        let value: Value = serde_json::from_str(r#"{"a":1}"#).unwrap();
        write_json_atomic(&target, &value).expect("write must succeed");

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(parsed["a"], 1);
    }

    /// DEBT-1 characterization RED: `Step::MergeJson` with an entry that is
    /// already present must be a semantic no-op — the file must not be
    /// touched (mtime, inode, bytes). Current code rewrites unconditionally.
    #[test]
    #[serial]
    fn t_debt1_merge_json_semantic_noop_skips_write() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("opencode.json");
        let original = r#"{
  "mcp": {
    "cognicode-mcp": {
      "command": [
        "/shims/cognicode-mcp"
      ],
      "enabled": true,
      "type": "stdio"
    }
  }
}"#;
        std::fs::write(&target, original).unwrap();

        // Redirect OpenCodePaths at the disposable config for the
        // duration of the test.
        let prev = std::env::var("OPENCODE_CONFIG").ok();
        unsafe {
            std::env::set_var("OPENCODE_CONFIG", &target);
        }

        // Freeze mtime to a known past value so a rewrite is detectable
        // even on filesystems with coarse timestamps.
        let past = filetime::FileTime::from_unix_time(1_000_000_000, 0);
        filetime::set_file_mtime(&target, past).unwrap();
        let meta_before = std::fs::metadata(&target).unwrap();
        let sha_before = crate::release_contract::sha256_file(&target).unwrap();

        let steps = integrate_opencode(
            &tmp.path().join("skills"),
            "0.95.0",
            &["/shims/cognicode-mcp".to_string()],
        )
        .unwrap();
        for step in steps {
            step.execute().unwrap();
        }

        // Restore env before asserting so failures don't leak it.
        match prev {
            Some(v) => unsafe {
                std::env::set_var("OPENCODE_CONFIG", v);
            },
            None => unsafe {
                std::env::remove_var("OPENCODE_CONFIG");
            },
        }

        let meta_after = std::fs::metadata(&target).unwrap();
        let sha_after = crate::release_contract::sha256_file(&target).unwrap();
        assert_eq!(
            sha_before, sha_after,
            "DEBT-1: semantic no-op must not change file content"
        );
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                meta_before.ino(),
                meta_after.ino(),
                "DEBT-1: semantic no-op must not rewrite the file (inode changed)"
            );
        }
        assert_eq!(
            meta_before.modified().unwrap(),
            meta_after.modified().unwrap(),
            "DEBT-1: semantic no-op must not touch mtime"
        );
    }

    /// Plant a version home with a bundle manifest declaring the
    /// `skills-for-claude` skill bundle (pairwise-distinct from the
    /// ComponentId `cognicode-mcp`) plus the bundle dir on disk, so
    /// DEBT-2 integrators can resolve sources from the manifest.
    fn plant_skill_bundle_version(home: &Path, version: &str) {
        let vdir = home.join("versions").join(version);
        std::fs::create_dir_all(vdir.join("skills").join("skills-for-claude")).unwrap();
        let yaml = format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "{version}"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
skill_bundles:
  - id: skills-for-claude
    version: "{version}"
    profiles: [core]
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
        std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
    }

    // ========================================================================
    // DEBT-3.f strict gates — IDE merge key derivation
    //
    // Contract: each integrator (`integrate_opencode`, `integrate_zcode`,
    // `integrate_claude`, `integrate_codex`) MUST derive the JSON/TOML merge
    // key from `mcp_command[0]` (the shim path), NOT from a hardcoded
    // `"cognicode-mcp"` literal. The shim path's basename is the
    // BinaryName, which may legitimately differ from the ComponentId
    // (the validator currently forces equality, but the IDE integration
    // must not bake that assumption in).
    //
    // These tests exercise `mcp_merge_key_from_command` directly with
    // pairwise-distinct identities:
    //
    //     PluginId   = "mcp-server"
    //     ComponentId = "cognicode-mcp"
    //     BinaryName = "renamed-mcp-binary"   (≠ ComponentId by design)
    //
    // Pre-DEBT-3.f the integrator passed `"cognicode-mcp"` (ComponentId)
    // as the merge key, which is wrong because the BinaryName is what
    // the on-disk shim and IDE config file stem need to agree on.
    // ========================================================================

    /// T_strict_merge_key_renamed_binary: a divergent BinaryName
    /// must be returned verbatim from the shim path, with no
    /// cross-reference to ComponentId or PluginId.
    #[test]
    fn t_debt3f_merge_key_renamed_binary() {
        let cmd = vec!["/tmp/home/shims/renamed-mcp-binary".to_string()];
        let key = mcp_merge_key_from_command(&cmd).expect("ok");
        assert_eq!(key, "renamed-mcp-binary");
        assert_ne!(
            key, "cognicode-mcp",
            "merge key must NOT collapse to ComponentId"
        );
        assert_ne!(key, "mcp-server", "merge key must NOT collapse to PluginId");
    }

    /// T_strict_merge_key_legacy_basename: when the shim path
    /// matches the ComponentId basename, the helper still returns it
    /// verbatim (no special-casing).
    #[test]
    fn t_debt3f_merge_key_legacy_basename() {
        let cmd = vec!["/tmp/home/shims/cognicode-mcp".to_string()];
        let key = mcp_merge_key_from_command(&cmd).expect("ok");
        assert_eq!(key, "cognicode-mcp");
    }

    /// T_strict_merge_key_empty_command: empty `mcp_command` is a
    /// programmer error — surface it loudly rather than guess.
    #[test]
    fn t_debt3f_merge_key_empty_command_fails_loudly() {
        let cmd: Vec<String> = vec![];
        let err = mcp_merge_key_from_command(&cmd).expect_err("must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("empty") || msg.contains("at least one"),
            "error must mention the empty-command contract; got: {msg}"
        );
    }

    /// T_strict_merge_key_unusable_filename: a path with no
    /// usable file name (e.g. trailing slash) is a programmer
    /// error — surface it loudly.
    #[test]
    fn t_debt3f_merge_key_unusable_filename_fails_loudly() {
        // A path with no `file_name()` (rare but possible)
        let cmd = vec!["/".to_string()];
        let err = mcp_merge_key_from_command(&cmd).expect_err("must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("no usable filename") || msg.contains("file_name"),
            "error must mention the unusable-filename contract; got: {msg}"
        );
    }

    /// T_strict_e2e_claude_renamed_binary: `integrate_claude` MUST
    /// write its MCP entry under a file whose stem equals the
    /// BinaryName embedded in `mcp_command[0]`. If the BinaryName
    /// diverges from the ComponentId, the file stem must follow the
    /// BinaryName — never the ComponentId, never the PluginId.
    ///
    /// Identities planted:
    ///     PluginId   = "mcp-server"
    ///     ComponentId = "cognicode-mcp"
    ///     BinaryName = "renamed-mcp-binary"
    ///
    /// Pre-DEBT-3.f the integrator wrote `~/.claude/mcp/cognicode-mcp.json`
    /// regardless of binary name; that would silently bind the IDE
    /// config to a ComponentId-derived stem, which is exactly the
    /// heuristic the audit (§9.4-11) calls out as identity-bridging.
    #[test]
    #[serial]
    fn t_debt3f_claude_integrate_uses_binary_name_stem() {
        let tmp = std::env::temp_dir().join(format!("cogh-debt3f-claude-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".claude/mcp")).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // DEBT-2: plant the version manifest + declared skill bundle
        // so the integrator resolves sources from the manifest.
        plant_skill_bundle_version(tmp.as_path(), "0.92.0");

        let mcp_cmd = vec!["/tmp/home/shims/renamed-mcp-binary".to_string()];
        let result = integrate_claude(tmp.as_path(), "mcp-server", "0.92.0", &mcp_cmd);

        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        // The renamed-binary file must exist…
        let renamed = tmp.join(".claude/mcp/renamed-mcp-binary.json");
        assert!(
            renamed.exists(),
            "integrate_claude must write a file under the BinaryName stem; \
             expected {} but file is missing. Pre-DEBT-3.f this would have \
             been ~/.claude/mcp/cognicode-mcp.json (ComponentId alias).",
            renamed.display()
        );

        // …and the legacy ComponentId-stem file must NOT exist.
        let legacy = tmp.join(".claude/mcp/cognicode-mcp.json");
        assert!(
            !legacy.exists(),
            "integrate_claude must NOT write a file under the ComponentId stem; \
             {} should not exist when BinaryName != ComponentId.",
            legacy.display()
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T_strict_e2e_codex_renamed_binary: `integrate_codex` MUST
    /// insert its MCP subtable under a key equal to the BinaryName
    /// embedded in `mcp_command[0]`. The TOML key, not a hardcoded
    /// `"cognicode-mcp"`, must follow the BinaryName.
    #[test]
    #[serial]
    fn t_debt3f_codex_integrate_uses_binary_name_subtable() {
        let tmp = std::env::temp_dir().join(format!("cogh-debt3f-codex-{}", std::process::id()));
        // CodexPaths resolves to $HOME/.codex/, so plant that dir first
        // (otherwise std::fs::write to config.toml below ENOENTs).
        std::fs::create_dir_all(tmp.join(".codex")).unwrap();
        let config = tmp.join(".codex/config.toml");
        std::fs::write(&config, "model = 'test'\n").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // DEBT-2: plant the version manifest + declared skill bundle
        // so the integrator resolves sources from the manifest.
        plant_skill_bundle_version(tmp.as_path(), "0.92.0");

        let mcp_cmd = vec!["/tmp/home/shims/renamed-mcp-binary".to_string()];
        let result = integrate_codex(tmp.as_path(), "mcp-server", "0.92.0", &mcp_cmd);

        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        let parsed: toml::Value = text.parse().unwrap();
        let servers = parsed.get("mcp_servers").and_then(|s| s.as_table());
        assert!(
            servers
                .map(|s| s.contains_key("renamed-mcp-binary"))
                .unwrap_or(false),
            "integrate_codex must insert a `[mcp_servers.<binary_name>]` subtable; \
             got text:\n{text}"
        );
        assert!(
            !text.contains("cognicode-mcp"),
            "integrate_codex must NOT emit a `[mcp_servers.cognicode-mcp]` subtable \
             when BinaryName != ComponentId; got text:\n{text}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T_strict_e2e_zcode_renamed_binary: `integrate_zcode` MUST
    /// insert its MCP entry under a JSON key equal to the BinaryName
    /// embedded in `mcp_command[0]`.
    #[test]
    #[serial]
    fn t_debt3f_zcode_integrate_uses_binary_name_key() {
        let tmp = std::env::temp_dir().join(format!("cogh-debt3f-zcode-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".zcode/v2")).unwrap();
        let config = tmp.join(".zcode/v2/config.json");
        std::fs::write(&config, "{}").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // DEBT-2: plant the version manifest + declared skill bundle
        // so the integrator resolves sources from the manifest.
        plant_skill_bundle_version(tmp.as_path(), "0.92.0");

        let mcp_cmd = vec!["/tmp/home/shims/renamed-mcp-binary".to_string()];
        let result = integrate_zcode(tmp.as_path(), "mcp-server", "0.92.0", &mcp_cmd);

        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert!(
            v["mcp"].get("renamed-mcp-binary").is_some(),
            "integrate_zcode must insert a JSON key equal to BinaryName; got: {text}"
        );
        assert!(
            v["mcp"].get("cognicode-mcp").is_none(),
            "integrate_zcode must NOT insert a JSON key equal to ComponentId when \
             BinaryName != ComponentId; got: {text}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn merge_path_adds_nested_value() {
        let mut v = json!({});
        let path = vec!["mcp".to_string(), "cognicode-mcp".to_string()];
        let entry = json!({"command": ["x"], "enabled": true, "type": "stdio"});

        // Mirror the merge logic
        let obj = v.as_object_mut().unwrap();
        let mcp = obj.entry("mcp").or_insert_with(|| json!({}));
        mcp.as_object_mut()
            .unwrap()
            .insert("cognicode-mcp".to_string(), entry.clone());

        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
        assert_eq!(v["mcp"]["cognicode-mcp"]["enabled"], true);
    }

    #[test]
    fn merge_preserves_existing_mcp_servers() {
        let mut v = json!({
            "mcp": {
                "chronos": {"type": "local"},
                "bastion": {"type": "local"}
            }
        });
        let entry = json!({"command": ["x"], "enabled": true, "type": "stdio"});
        let obj = v.as_object_mut().unwrap();
        let mcp = obj.entry("mcp").or_insert_with(|| json!({}));
        mcp.as_object_mut()
            .unwrap()
            .insert("cognicode-mcp".to_string(), entry);

        // Original entries preserved
        assert_eq!(v["mcp"]["chronos"]["type"], "local");
        assert_eq!(v["mcp"]["bastion"]["type"], "local");
        // New entry added
        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
    }

    #[test]
    fn remove_path_clears_nested_value() {
        let mut v = json!({
            "mcp": {
                "cognicode-mcp": {"type": "stdio"},
                "chronos": {"type": "local"}
            }
        });
        if let Some(mcp) = v.get_mut("mcp")
            && let Some(mcp_obj) = mcp.as_object_mut()
        {
            mcp_obj.remove("cognicode-mcp");
        }
        assert!(v["mcp"].get("cognicode-mcp").is_none());
        assert_eq!(v["mcp"]["chronos"]["type"], "local");
    }

    #[test]
    fn opencode_config_path_default() {
        let p = opencode_config_path();
        assert!(p.ends_with("opencode.json"));
    }

    #[test]
    fn mcp_entry_has_required_fields() {
        let entry = json!({
            "command": ["~/.cognicode/shims/cognicode-mcp"],
            "enabled": true,
            "type": "stdio",
        });
        assert_eq!(entry["type"], "stdio");
        assert_eq!(entry["enabled"], true);
        assert!(entry["command"].is_array());
    }

    #[test]
    #[serial]
    fn integrate_opencode_writes_mcp_entry() {
        let tmp = std::env::temp_dir().join(format!("cogh-oc-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".config/opencode")).unwrap();
        let config = tmp.join(".config/opencode/opencode.json");
        let original = json!({
            "agent": {"foo": {"description": "test"}},
            "mcp": {"chronos": {"type": "local"}}
        });
        std::fs::write(&config, "{\"agent\":{\"foo\":{\"description\":\"test\"}},\"mcp\":{\"chronos\":{\"type\":\"local\"}}}").unwrap();

        // Override HOME for the duration of this test
        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Create fake skill path
        let skill_path = tmp.join(".cognicode/versions/0.92.0/mcp-server/skills");
        std::fs::create_dir_all(&skill_path).unwrap();

        let mcp_cmd = vec!["cognicode-mcp".to_string(), "stdio".to_string()];
        let steps = integrate_opencode(&skill_path, "0.92.0", &mcp_cmd).unwrap();
        for step in steps {
            step.execute().unwrap();
        }

        unsafe {
            std::env::set_var("HOME", &prev_home);
        }

        // Read back the config
        let text = std::fs::read_to_string(&config).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();

        // Original entries preserved
        assert_eq!(v["agent"]["foo"]["description"], "test");
        assert_eq!(v["mcp"]["chronos"]["type"], "local");
        // New entry added
        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
        assert_eq!(v["mcp"]["cognicode-mcp"]["enabled"], true);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn uninstall_opencode_removes_entry() {
        let tmp = std::env::temp_dir().join(format!("cogh-oc-un-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".config/opencode")).unwrap();
        let config = tmp.join(".config/opencode/opencode.json");
        let original = json!({
            "mcp": {
                "cognicode-mcp": {"type": "stdio", "enabled": true},
                "chronos": {"type": "local"}
            }
        });
        std::fs::write(&config, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        let steps = uninstall_opencode("0.92.0", "cognicode-mcp").unwrap();
        for step in steps {
            step.execute().unwrap();
        }

        unsafe {
            std::env::set_var("HOME", &prev_home);
        }

        let text = std::fs::read_to_string(&config).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        // cognicode-mcp removed, chronos preserved
        assert!(v["mcp"].get("cognicode-mcp").is_none());
        assert_eq!(v["mcp"]["chronos"]["type"], "local");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn detect_opencode_finds_config() {
        // Hermetic: give the child a temp HOME with a stub opencode config
        // instead of relying on the developer's real ~/.config/opencode.
        let tmp = tempfile::tempdir().expect("tempdir for HOME");
        let prev_home = std::env::var_os("HOME");
        // SAFETY: #[serial] excludes concurrent env mutation.
        unsafe {
            std::env::set_var("HOME", tmp.path());
        }
        let oc_dir = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
        std::fs::write(oc_dir.join("opencode.json"), "{}").expect("write stub config");

        let detected = detect_opencode();

        unsafe {
            match &prev_home {
                Some(h) => std::env::set_var("HOME", h),
                None => std::env::remove_var("HOME"),
            }
        }
        assert!(detected, "detect_opencode should find the stub config");
    }
    #[test]
    #[serial]
    fn integrate_zcode_creates_mcp_section() {
        let tmp = std::env::temp_dir().join(format!("cogh-zcode-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".zcode/v2")).unwrap();
        let config = tmp.join(".zcode/v2/config.json");
        let original = json!({"provider": {"minimax": {}}});
        std::fs::write(&config, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        // DEBT-2: plant manifest + declared skill bundle for resolution.
        plant_skill_bundle_version(&home, "0.92.0");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string()];
        let result = integrate_zcode(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(v["provider"]["minimax"].is_object(), true);
        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
        assert_eq!(v["mcp"]["cognicode-mcp"]["enabled"], true);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn uninstall_zcode_removes_entry() {
        let tmp = std::env::temp_dir().join(format!("cogh-zcode-un-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".zcode/v2")).unwrap();
        let config = tmp.join(".zcode/v2/config.json");
        let original = json!({
            "mcp": {
                "cognicode-mcp": {"type": "stdio"},
                "other": {"type": "local"}
            }
        });
        std::fs::write(&config, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let result = uninstall_zcode("0.92.0", "cognicode-mcp");
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(v["mcp"].get("cognicode-mcp").is_none());
        assert_eq!(v["mcp"]["other"]["type"], "local");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn zcode_config_path_default() {
        let p = zcode_config_path();
        assert!(p.ends_with("config.json"));
    }
    #[test]
    #[serial]
    fn integrate_claude_writes_mcp_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-claude-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".claude")).unwrap();
        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        // DEBT-2: plant manifest + declared skill bundle for resolution.
        plant_skill_bundle_version(&home, "0.92.0");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string()];
        let result = integrate_claude(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();
        let path = tmp.join(".claude/mcp/cognicode-mcp.json");
        assert!(path.exists());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"], "/bin/cognicode-mcp");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn uninstall_claude_removes_mcp_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-claude-un-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.join(".claude/mcp")).unwrap();
        let path = tmp.join(".claude/mcp/cognicode-mcp.json");
        std::fs::write(&path, r#"{"command":"x"}"#).unwrap();
        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let result = uninstall_claude("0.92.0", "cognicode-mcp");
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn claude_config_path_default() {
        let p = claude_config_path();
        assert!(p.ends_with(".claude"));
    }
    #[test]
    #[serial]
    fn integrate_codex_inserts_mcp_server() {
        let tmp = std::env::temp_dir().join(format!("cogh-codex-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".codex")).unwrap();
        let config = tmp.join(".codex/config.toml");
        std::fs::write(&config, "model = 'test'\n").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        // DEBT-2: plant manifest + declared skill bundle for resolution.
        plant_skill_bundle_version(&home, "0.92.0");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string(), "stdio".to_string()];
        let result = integrate_codex(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        // Parse the TOML to verify semantically — quoting style may
        // change ("test" vs 'test').
        let parsed: toml::Value = text.parse().unwrap();
        let model = parsed.get("model").and_then(|m| m.as_str()).unwrap_or("");
        let has_cognicode_mcp = parsed
            .get("mcp_servers")
            .and_then(|s| s.get("cognicode-mcp"))
            .is_some();
        assert!(has_cognicode_mcp, "cognicode-mcp not in mcp_servers");
        assert_eq!(model, "test", "model field not preserved");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn uninstall_codex_removes_entry() {
        let tmp = std::env::temp_dir().join(format!("cogh-codex-un-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".codex")).unwrap();
        let config = tmp.join(".codex/config.toml");
        let original = r#"model = 'test'
mcp_servers.existing.command = 'x'
mcp_servers.existing.args = ['y']
"#;
        std::fs::write(&config, original).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
        let result = uninstall_codex("0.92.0", "cognicode-mcp");
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        // TOML output may use [mcp_servers.existing] section format
        // instead of inline. Parse it back to verify semantically.
        let parsed: toml::Value = text.parse().unwrap();
        let existing_cmd = parsed
            .get("mcp_servers")
            .and_then(|s| s.get("existing"))
            .and_then(|e| e.get("command"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        assert!(!text.contains("cognicode-mcp"));
        assert_eq!(existing_cmd, "x");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn codex_config_path_default() {
        let p = codex_config_path();
        assert!(p.ends_with("config.toml"));
    }

    #[test]
    fn test_detect_opencode() {
        // Returns false in test environment (no real config)
        let detected = detect_opencode();
        println!("OpenCode detected: {}", detected);
    }

    #[test]
    fn test_integrate_opencode_steps() {
        let skill_path = PathBuf::from("/fake/skills");
        let mcp_cmd = vec!["cognicode-mcp".to_string(), "stdio".to_string()];
        let steps = integrate_opencode(&skill_path, "0.94.9", &mcp_cmd).unwrap();
        assert!(!steps.is_empty());
        assert!(matches!(steps[0], Step::Symlink { .. }));
    }

    #[test]
    fn test_uninstall_opencode_steps() {
        let steps = uninstall_opencode("0.94.9", "cognicode-mcp").unwrap();
        assert!(!steps.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // E86.2.3 — disposable-aware OpenCode integration
    //
    // The bug surfaced by the E86.2.2 real-PC UAT: `opencode_skills_dir()`
    // reads only `$HOME`, ignoring `OPENCODE_CONFIG`. That means setting
    // `OPENCODE_CONFIG` to a disposable path still results in skills being
    // symlinked under the developer's real `~/.config/opencode/skills/`.
    //
    // These tests pin the bug BEFORE the fix.
    // ─────────────────────────────────────────────────────────────────────────

    // RED: explicit OPENCODE_CONFIG must drive the skills symlink target.
    #[test]
    #[serial]
    fn t_e86_2_3_opencode_skills_follow_opencode_config() {
        let tmp = std::env::temp_dir().join(format!("cogh-e86-2-3-oc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let disposable = tmp.join("disposable-opencode");
        std::fs::create_dir_all(&disposable).unwrap();

        // The OPENCODE_CONFIG override points at a file inside `disposable`.
        let config_path = disposable.join("opencode.json");
        std::fs::write(&config_path, "{}").unwrap();

        // A pre-existing "real" OpenCode config at the host's HOME is what
        // the UAT script must NOT touch. We simulate that by setting HOME
        // to a tempdir containing a sentinel `opencode/skills` we can
        // check later for absence of pollution.
        let real_home = tmp.join("real-home");
        std::fs::create_dir_all(real_home.join(".config/opencode/skills")).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_opencode_cfg = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &real_home);
            std::env::set_var("OPENCODE_CONFIG", &config_path);
        }

        let skill_path = tmp.join("skill-src");
        std::fs::create_dir_all(&skill_path).unwrap();
        let mcp_cmd = vec!["cognicode-mcp".to_string(), "stdio".to_string()];
        let steps = integrate_opencode(&skill_path, "0.95.0", &mcp_cmd).unwrap();

        // Collect step targets BEFORE executing — `step.execute()` moves.
        let symlinks: Vec<_> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Symlink { target, .. } => Some(target.clone()),
                _ => None,
            })
            .collect();
        for step in steps {
            step.execute().unwrap();
        }

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_opencode_cfg {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        // Expected: skills symlink lives under `disposable/skills/...`,
        // NOT under `real_home/.config/opencode/skills/`.
        assert_eq!(symlinks.len(), 1, "expected one Symlink step");
        let target = &symlinks[0];
        assert!(
            target.starts_with(&disposable),
            "skills symlink must live under OPENCODE_CONFIG's parent ({}), got {}",
            disposable.display(),
            target.display()
        );
        assert!(
            !target.starts_with(&real_home),
            "skills symlink must NOT live under real HOME ({}), got {}",
            real_home.display(),
            target.display()
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // RED: explicit OPENCODE_CONFIG must drive the skills uninstall target too.
    #[test]
    #[serial]
    fn t_e86_2_3_opencode_uninstall_follows_opencode_config() {
        let tmp = std::env::temp_dir().join(format!("cogh-e86-2-3-oc-un-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let disposable = tmp.join("disposable-opencode");
        std::fs::create_dir_all(disposable.join("skills")).unwrap();
        std::fs::write(disposable.join("opencode.json"), "{}").unwrap();

        // Plant a sentinel symlink under the real HOME that must NOT be
        // removed by the uninstall step.
        let real_home = tmp.join("real-home");
        let real_skills_dir = real_home.join(".config/opencode/skills");
        std::fs::create_dir_all(&real_skills_dir).unwrap();
        let real_target = real_skills_dir.join("cognicode-0.95.0");
        std::os::unix::fs::symlink(&disposable, &real_target).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_opencode_cfg = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &real_home);
            std::env::set_var("OPENCODE_CONFIG", disposable.join("opencode.json"));
        }

        let steps = uninstall_opencode("0.95.0", "cognicode-mcp").unwrap();
        let mut targets = Vec::new();
        for step in steps {
            match &step {
                Step::RmRf { target } => targets.push(target.clone()),
                _ => {}
            }
            step.execute().unwrap();
        }

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_opencode_cfg {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        // The RmRf target must point at the disposable skills, NOT real HOME.
        let rmtargets: Vec<_> = targets.iter().collect();
        assert_eq!(rmtargets.len(), 1, "expected one RmRf step");
        let t = rmtargets[0];
        assert!(
            t.starts_with(&disposable),
            "uninstall target must live under OPENCODE_CONFIG's parent, got {}",
            t.display()
        );
        assert!(
            !t.starts_with(&real_home),
            "uninstall target must NOT live under real HOME, got {}",
            t.display()
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // T1 — explicit OPENCODE_CONFIG controls config + skills end-to-end.
    #[test]
    #[serial]
    fn t_e86_2_3_opencode_resolver_with_explicit_config() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-2-3-resolver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let disposable = tmp.join("oc");
        std::fs::create_dir_all(&disposable).unwrap();
        let config_path = disposable.join("opencode.json");
        std::fs::write(&config_path, r#"{"agent":{"foo":"bar"}}"#).unwrap();

        let prev = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("OPENCODE_CONFIG", &config_path);
        }

        let paths = OpenCodePaths::resolve();

        unsafe {
            match prev {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        assert_eq!(paths.config_file, config_path);
        assert_eq!(paths.config_dir, disposable);
        assert_eq!(paths.skills_dir, disposable.join("skills"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // T3 — default behaviour: with OPENCODE_CONFIG unset, the resolver
    // falls back to `$HOME/.config/opencode/`. Pure path resolution test.
    #[test]
    #[serial]
    fn t_e86_2_3_opencode_resolver_default() {
        let tmp = std::env::temp_dir().join(format!("cogh-e86-2-3-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &tmp);
            std::env::remove_var("OPENCODE_CONFIG");
        }

        let paths = OpenCodePaths::resolve();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        assert_eq!(
            paths.config_file,
            tmp.join(".config/opencode/opencode.json")
        );
        assert_eq!(paths.config_dir, tmp.join(".config/opencode"));
        assert_eq!(paths.skills_dir, tmp.join(".config/opencode/skills"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ===== E86.5 — detect_* uses is_file, not exists =====

    /// T1 (RED before fix): `detect_opencode()` must return FALSE when the
    /// resolved `config_file` is a directory, not a regular file.
    ///
    /// Currently `detect_opencode` uses `Path::exists()`, which returns
    /// true for both files AND directories. The UAT script
    /// (`/tmp/cogh-uat-real-pc.sh`) works around this by pointing
    /// `OPENCODE_CONFIG` at a NON-EXISTENT file path inside a fresh temp
    /// dir; but if a developer's `$HOME/.config/opencode/` exists as a
    /// directory and the config file inside it is missing (e.g. the user
    /// ran opencode once, deleted `opencode.json`, and never recreated
    /// it), `detect_opencode()` would falsely return true and the
    /// install pipeline would try to integrate against a non-existent
    /// config — `Step::MergeJson` would then create a fresh empty config
    /// and `Step::Symlink` would fail with `link_or_copy failed` against
    /// a missing skills source.
    ///
    /// We exhibit the bug by creating a *directory* at the resolved
    /// `config_file` path. `Path::exists()` returns true (the dir
    /// exists) but `Path::is_file()` returns false (it is not a regular
    /// file). With the buggy `exists()` check, `detect_opencode()`
    /// returns true; with the correct `is_file()` check, it returns
    /// false.
    #[test]
    #[serial]
    fn t_e86_5_detect_opencode_returns_false_when_config_is_directory() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-5-detect-oc-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        // IMPORTANT: create a DIRECTORY at the path config_file will
        // resolve to. This models the bug: the path exists, but it is
        // not a file. With `Path::exists()` the function returns true
        // even though there is no config to integrate.
        let fake_config_path = fake_home.join(".config/opencode/opencode.json");
        std::fs::create_dir_all(&fake_config_path).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("OPENCODE_CONFIG");
        }

        // Sanity: the resolved config_file path EXISTS (it is a
        // directory) but is_file() returns false.
        let paths = OpenCodePaths::resolve();
        assert!(
            paths.config_file.exists(),
            "config_file (as dir) should exist for this test; got {}",
            paths.config_file.display()
        );
        assert!(
            !paths.config_file.is_file(),
            "config_file should NOT be a regular file in this test; got {}",
            paths.config_file.display()
        );
        assert!(
            paths.config_file.is_dir(),
            "config_file should be a directory in this test; got {}",
            paths.config_file.display()
        );

        let detected = detect_opencode();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        assert!(
            !detected,
            "detect_opencode must return false when config_file is a directory; got true"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T2: `detect_opencode()` must return TRUE when the resolved
    /// `config_file` is a regular file. This is the happy path and pins
    /// the contract that a real config file is detected.
    #[test]
    #[serial]
    fn t_e86_5_detect_opencode_returns_true_when_config_is_file() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-5-detect-oc-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        let oc_dir = fake_home.join(".config/opencode");
        std::fs::create_dir_all(&oc_dir).unwrap();
        std::fs::write(oc_dir.join("opencode.json"), "{}").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("OPENCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("OPENCODE_CONFIG");
        }

        let detected = detect_opencode();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("OPENCODE_CONFIG", v),
                None => std::env::remove_var("OPENCODE_CONFIG"),
            }
        }

        assert!(
            detected,
            "detect_opencode must return true when opencode.json exists; got false"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ===== E86.6 — detect_zcode / detect_codex use is_file =====

    /// T1 (RED before fix): `detect_zcode()` must return FALSE when the
    /// resolved `config_file` is a directory, not a regular file.
    /// Same shape as `t_e86_5_detect_opencode_returns_false_when_config_is_directory`,
    /// but for the ZCode adapter.
    #[test]
    #[serial]
    fn t_e86_6_detect_zcode_returns_false_when_config_is_directory() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-6-detect-zc-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        // ZCodePaths default config_file: $HOME/.zcode/v2/config.json
        let fake_config_path = fake_home.join(".zcode/v2/config.json");
        std::fs::create_dir_all(&fake_config_path).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("ZCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("ZCODE_CONFIG");
        }

        let paths = ZCodePaths::resolve();
        assert!(
            paths.config_file.exists()
                && !paths.config_file.is_file()
                && paths.config_file.is_dir(),
            "config_file shape precondition failed; got {}",
            paths.config_file.display()
        );

        let detected = detect_zcode();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("ZCODE_CONFIG", v),
                None => std::env::remove_var("ZCODE_CONFIG"),
            }
        }

        assert!(
            !detected,
            "detect_zcode must return false when config_file is a directory; got true"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T2 (happy path): `detect_zcode()` returns true when the resolved
    /// `config_file` is a regular file.
    #[test]
    #[serial]
    fn t_e86_6_detect_zcode_returns_true_when_config_is_file() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-6-detect-zc-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        let zc_dir = fake_home.join(".zcode/v2");
        std::fs::create_dir_all(&zc_dir).unwrap();
        std::fs::write(zc_dir.join("config.json"), "{}").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("ZCODE_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("ZCODE_CONFIG");
        }

        let detected = detect_zcode();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("ZCODE_CONFIG", v),
                None => std::env::remove_var("ZCODE_CONFIG"),
            }
        }

        assert!(
            detected,
            "detect_zcode must return true when config.json exists; got false"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T3 (RED before fix): `detect_codex()` must return FALSE when the
    /// resolved `config_file` is a directory.
    #[test]
    #[serial]
    fn t_e86_6_detect_codex_returns_false_when_config_is_directory() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-6-detect-cdx-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        // CodexPaths default config_file: $HOME/.codex/config.toml
        let fake_config_path = fake_home.join(".codex/config.toml");
        std::fs::create_dir_all(&fake_config_path).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("CODEX_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("CODEX_CONFIG");
        }

        let paths = CodexPaths::resolve();
        assert!(
            paths.config_file.exists()
                && !paths.config_file.is_file()
                && paths.config_file.is_dir(),
            "config_file shape precondition failed; got {}",
            paths.config_file.display()
        );

        let detected = detect_codex();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("CODEX_CONFIG", v),
                None => std::env::remove_var("CODEX_CONFIG"),
            }
        }

        assert!(
            !detected,
            "detect_codex must return false when config_file is a directory; got true"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T4 (happy path): `detect_codex()` returns true when the resolved
    /// `config_file` is a regular file.
    #[test]
    #[serial]
    fn t_e86_6_detect_codex_returns_true_when_config_is_file() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-e86-6-detect-cdx-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_home = tmp.join("fake-home");
        let cdx_dir = fake_home.join(".codex");
        std::fs::create_dir_all(&cdx_dir).unwrap();
        std::fs::write(cdx_dir.join("config.toml"), "").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        let prev_cfg = std::env::var_os("CODEX_CONFIG");
        unsafe {
            std::env::set_var("HOME", &fake_home);
            std::env::remove_var("CODEX_CONFIG");
        }

        let detected = detect_codex();

        unsafe {
            std::env::set_var("HOME", &prev_home);
            match prev_cfg {
                Some(v) => std::env::set_var("CODEX_CONFIG", v),
                None => std::env::remove_var("CODEX_CONFIG"),
            }
        }

        assert!(
            detected,
            "detect_codex must return true when config.toml exists; got false"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

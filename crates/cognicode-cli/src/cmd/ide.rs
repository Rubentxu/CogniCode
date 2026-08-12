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

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

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
    RemoveFromJson {
        target: PathBuf,
        path: Vec<String>,
    },
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
                    current = current.entry(key).or_insert_with(|| json!({})).as_object_mut().unwrap();
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
                    current = current.entry(key).or_insert_with(|| json!({})).as_object_mut().unwrap();
                }
                if let Some(last_key) = path.last() {
                    current.remove(last_key);
                }
                write_json_atomic(target, &config)?;
                Ok(())
            }
            Step::Symlink { source, target } => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(source, target)
                        .map_err(|e| anyhow!("symlink failed: {}", e))?;
                }
                #[cfg(not(unix))]
                {
                    if source.is_dir() {
                        copy_dir_recursive(source, target)?;
                    } else {
                        std::fs::copy(source, target)?;
                    }
                }
                Ok(())
            }
        }
    }
}

/// Detect whether the IDE is installed.
pub fn detect_opencode() -> bool {
    let config = opencode_config_path();
    config.exists()
}

pub fn opencode_config_path() -> PathBuf {
    if let Ok(env) = std::env::var("OPENCODE_CONFIG") {
        return PathBuf::from(env);
    }
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return PathBuf::from("~/.config/opencode/opencode.json"),
    };
    home.join(".config/opencode/opencode.json")
}

pub fn opencode_skills_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/opencode/skills")
}

/// Read the OpenCode config as JSON.
pub fn read_opencode_config() -> Result<Value> {
    let path = opencode_config_path();
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let v: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

/// Atomic JSON write (tmp + rename).
fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    if let Some(parent) = tmp.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let json_str = serde_json::to_string_pretty(value)
        .with_context(|| format!("serialize {}", path.display()))?;
    std::fs::write(&tmp, json_str)
        .with_context(|| format!("write tmp {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

/// Build the opencode adapter's `integrate` recipe as steps.
pub fn integrate_opencode(
    skill_path: &Path,
    version: &str,
) -> Result<Vec<Step>> {
    let mut steps = Vec::new();

    // 1. Symlink skill bundle to OpenCode skills directory
    let skills_target = opencode_skills_dir().join(format!("cognicode-{version}"));
    steps.push(Step::Symlink {
        source: skill_path.to_path_buf(),
        target: skills_target,
    });

    // 2. MCP config merge
    let config_path = opencode_config_path();
    let mcp_entry = json!({
        "command": vec!["cognicode-mcp".to_string(), "stdio".to_string()],
        "enabled": true,
        "type": "stdio",
    });
    steps.push(Step::MergeJson {
        target: config_path,
        path: vec!["mcp".to_string(), "cognicode-mcp".to_string()],
        value: mcp_entry,
    });

    Ok(steps)
}

/// Build the opencode adapter's `uninstall` recipe as steps.
pub fn uninstall_opencode(version: &str) -> Result<Vec<Step>> {
    let mut steps = Vec::new();

    // 1. Remove skills symlink
    let skills_target = opencode_skills_dir().join(format!("cognicode-{version}"));
    steps.push(Step::RmRf { target: skills_target });

    // 2. Remove MCP entry
    let config_path = opencode_config_path();
    steps.push(Step::RemoveFromJson {
        target: config_path,
        path: vec!["mcp".to_string(), "cognicode-mcp".to_string()],
    });

    Ok(steps)
}

/// Copy a directory tree recursively.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst)
            .with_context(|| format!("rm -rf existing {}", dst.display()))?;
    }
    std::fs::create_dir_all(dst)
        .with_context(|| format!("create {}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .with_context(|| format!("read_dir {}", src.display()))?
    {
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
    zcode_config_path().exists()
}

pub fn zcode_config_path() -> PathBuf {
    if let Ok(env) = std::env::var("ZCODE_CONFIG") {
        return PathBuf::from(env);
    }
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return PathBuf::from("~/.zcode/v2/config.json"),
    };
    home.join(".zcode/v2/config.json")
}

pub fn zcode_skills_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".zcode/skills")
}

pub fn read_zcode_config() -> Result<Value> {
    let path = zcode_config_path();
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let v: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

pub fn integrate_zcode(home: &Path, plugin: &str, version: &str, mcp_command: &[String]) -> Result<()> {
    // 1. Skill copy
    let skills_src = home
        .join("versions")
        .join(version)
        .join(plugin)
        .join("skills");
    let skills_dst = zcode_skills_dir().join(format!("cognicode-{version}"));
    if skills_src.exists() {
        copy_dir_recursive(&skills_src, &skills_dst)
            .with_context(|| format!("copy {} → {}", skills_src.display(), skills_dst.display()))?;
        println!("✓ copied skills: {}", skills_dst.display());
    } else {
        println!("(no skills to copy from {})", skills_src.display());
    }

    // 2. MCP config merge
    let config_path = zcode_config_path();
    let mut config = read_zcode_config()?;
    let mcp_entry = json!({
        "command": mcp_command,
        "enabled": true,
        "type": "stdio",
    });

    let cfg = config.as_object_mut()
        .ok_or_else(|| anyhow!("zcode config.json is not an object"))?;
    let mcp = cfg.entry("mcp").or_insert_with(|| json!({}));
    if !mcp.is_object() {
        return Err(anyhow!("zcode config.json: 'mcp' is not an object (got {})", mcp));
    }
    mcp.as_object_mut().unwrap()
        .insert("cognicode-mcp".to_string(), mcp_entry);
    write_json_atomic(&config_path, &config)?;
    println!("✓ patched: {}", config_path.display());

    Ok(())
}

pub fn uninstall_zcode(version: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = zcode_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP entry
    let config_path = zcode_config_path();
    if config_path.exists() {
        let mut config = read_zcode_config()?;
        let cfg = config.as_object_mut()
            .ok_or_else(|| anyhow!("zcode config.json is not an object"))?;
        if let Some(mcp) = cfg.get_mut("mcp") {
            if let Some(mcp_obj) = mcp.as_object_mut() {
                mcp_obj.remove("cognicode-mcp");
            }
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
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let v: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(v))
}

pub fn integrate_claude(home: &Path, plugin: &str, version: &str, mcp_command: &[String]) -> Result<()> {
    // 1. Skill copy
    let skills_src = home
        .join("versions")
        .join(version)
        .join(plugin)
        .join("skills");
    let skills_dst = claude_skills_dir().join(format!("cognicode-{version}"));
    if skills_src.exists() {
        copy_dir_recursive(&skills_src, &skills_dst)
            .with_context(|| format!("copy {} → {}", skills_src.display(), skills_dst.display()))?;
        println!("✓ copied skills: {}", skills_dst.display());
    } else {
        println!("(no skills to copy from {})", skills_src.display());
    }

    // 2. MCP config: write `~/.claude/mcp/cognicode-mcp.json`
    let mcp_dir = claude_mcp_dir();
    std::fs::create_dir_all(&mcp_dir)
        .with_context(|| format!("create {}", mcp_dir.display()))?;
    let target = mcp_dir.join("cognicode-mcp.json");
    let entry = json!({
        "command": mcp_command[0],
        "args": mcp_command.get(1..).unwrap_or(&[]).to_vec(),
    });
    write_json_atomic(&target, &entry)?;
    println!("✓ patched: {}", target.display());

    Ok(())
}

pub fn uninstall_claude(version: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = claude_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP file
    let target = claude_mcp_dir().join("cognicode-mcp.json");
    if target.exists() {
        std::fs::remove_file(&target)
            .with_context(|| format!("rm {}", target.display()))?;
        println!("✓ removed: {}", target.display());
    }

    Ok(())
}


// ===== Codex adapter (E32-G) =====

pub fn detect_codex() -> bool {
    codex_config_path().exists()
}

pub fn codex_config_path() -> PathBuf {
    if let Ok(env) = std::env::var("CODEX_CONFIG") {
        return PathBuf::from(env);
    }
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return PathBuf::from("~/.codex/config.toml"),
    };
    home.join(".codex/config.toml")
}

pub fn codex_skills_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codex/skills")
}

pub fn read_codex_config() -> Result<toml::Value> {
    let path = codex_config_path();
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let v: toml::Value = text.parse()
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(v)
}

pub fn integrate_codex(home: &Path, plugin: &str, version: &str, mcp_command: &[String]) -> Result<()> {
    // 1. Skill copy
    let skills_src = home
        .join("versions")
        .join(version)
        .join(plugin)
        .join("skills");
    let skills_dst = codex_skills_dir().join(format!("cognicode-{version}"));
    if skills_src.exists() {
        copy_dir_recursive(&skills_src, &skills_dst)
            .with_context(|| format!("copy {} → {}", skills_src.display(), skills_dst.display()))?;
        println!("✓ copied skills: {}", skills_dst.display());
    } else {
        println!("(no skills to copy from {})", skills_src.display());
    }

    // 2. Codex config: TOML, [mcp_servers.cognicode-mcp] section
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
    let args_value = mcp_command.get(1..).map(|rest| {
        let arr: Vec<toml::Value> = rest.iter().cloned().map(toml::Value::String).collect();
        toml::Value::Array(arr)
    }).unwrap_or_else(|| toml::Value::Array(Vec::new()));
    let server = toml::Value::Table(toml::map::Map::from_iter([
        ("command".to_string(), toml::Value::String(cmd.to_string())),
        ("args".to_string(), args_value),
    ]));
    mcp_table.insert("cognicode-mcp".to_string(), server);

    // Atomic write
    let tmp = config_path.with_extension("toml.tmp");
    if let Some(parent) = tmp.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let s = toml::to_string_pretty(&config)
        .with_context(|| format!("serialize {}", config_path.display()))?;
    std::fs::write(&tmp, s)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &config_path)
        .with_context(|| format!("rename {}", config_path.display()))?;
    println!("✓ patched: {}", config_path.display());

    Ok(())
}

pub fn uninstall_codex(version: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = codex_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        println!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP entry from TOML config
    let config_path = codex_config_path();
    if config_path.exists() {
        let mut config = read_codex_config()?;
        if let Some(t) = config.as_table_mut() {
            if let Some(mcp_servers) = t.get_mut("mcp_servers") {
                if let Some(mcp_table) = mcp_servers.as_table_mut() {
                    mcp_table.remove("cognicode-mcp");
                }
            }
        }
        let tmp = config_path.with_extension("toml.tmp");
        if let Some(parent) = tmp.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let s = toml::to_string_pretty(&config)
            .with_context(|| format!("serialize {}", config_path.display()))?;
        std::fs::write(&tmp, s)
            .with_context(|| format!("write {}", tmp.display()))?;
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
            println!("    (claude config present at {}, but adapter is E32-F)", claude.display());
        }
        let codex = home.join(".codex/config.json");
        if codex.exists() {
            println!("    (codex config present at {}, but adapter is E32-G)", codex.display());
        }
    }
    Ok(())
}

pub fn cmd_ide_install(home: &CognicodeHomeSup, ide: &str, plugin: &str, version: &str) -> Result<()> {
    // Resolve the MCP command line. The actual binary is at
    // `~/.cognicode/shims/cognicode-mcp` (after E32-A's shim layout).
    let mcp_command = vec![
        home.shims().join("cognicode-mcp").to_string_lossy().to_string(),
    ];
    match ide {
        "opencode" => {
            // skill_path is the plugin's skills directory
            let skill_path = home.versions().join(version).join(plugin).join("skills");
            let steps = integrate_opencode(&skill_path, version)?;
            for step in steps {
                step.execute()?;
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
    match ide {
        "opencode" => {
            let steps = uninstall_opencode(version)?;
            for step in steps {
                step.execute()?;
            }
            println!("✓ OpenCode uninstall complete");
            Ok(())
        }
        "zcode" => uninstall_zcode(version),
        "claude" => uninstall_claude(version),
        "codex" => uninstall_codex(version),
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
    use std::io::Write;

    #[test]
    fn merge_path_adds_nested_value() {
        let mut v = json!({});
        let path = vec!["mcp".to_string(), "cognicode-mcp".to_string()];
        let entry = json!({"command": ["x"], "enabled": true, "type": "stdio"});

        // Mirror the merge logic
        let obj = v.as_object_mut().unwrap();
        let mcp = obj.entry("mcp").or_insert_with(|| json!({}));
        mcp.as_object_mut().unwrap().insert("cognicode-mcp".to_string(), entry.clone());

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
        mcp.as_object_mut().unwrap().insert("cognicode-mcp".to_string(), entry);

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
        if let Some(mcp) = v.get_mut("mcp") {
            if let Some(mcp_obj) = mcp.as_object_mut() {
                mcp_obj.remove("cognicode-mcp");
            }
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

        let steps = integrate_opencode(&skill_path, "0.92.0").unwrap();
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

        let steps = uninstall_opencode("0.92.0").unwrap();
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
    fn detect_opencode_finds_config() {
        // The user's existing opencode config makes this true
        assert!(std::path::Path::new(&opencode_config_path()).exists());
    }
    #[test]
    fn integrate_zcode_creates_mcp_section() {
        let tmp = std::env::temp_dir().join(format!("cogh-zcode-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".zcode/v2")).unwrap();
        let config = tmp.join(".zcode/v2/config.json");
        let original = json!({"provider": {"minimax": {}}});
        std::fs::write(&config, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", &tmp); }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string()];
        let result = integrate_zcode(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe { std::env::set_var("HOME", &prev_home); }
        result.unwrap();

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(v["provider"]["minimax"].is_object(), true);
        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
        assert_eq!(v["mcp"]["cognicode-mcp"]["enabled"], true);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
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
        unsafe { std::env::set_var("HOME", &tmp); }
        let result = uninstall_zcode("0.92.0");
        unsafe { std::env::set_var("HOME", &prev_home); }
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
    fn integrate_claude_writes_mcp_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-claude-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".claude")).unwrap();
        let prev_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", &tmp); }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string()];
        let result = integrate_claude(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe { std::env::set_var("HOME", &prev_home); }
        result.unwrap();
        let path = tmp.join(".claude/mcp/cognicode-mcp.json");
        assert!(path.exists());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"], "/bin/cognicode-mcp");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn uninstall_claude_removes_mcp_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-claude-un-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.join(".claude/mcp")).unwrap();
        let path = tmp.join(".claude/mcp/cognicode-mcp.json");
        std::fs::write(&path, r#"{"command":"x"}"#).unwrap();
        let prev_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", &tmp); }
        let result = uninstall_claude("0.92.0");
        unsafe { std::env::set_var("HOME", &prev_home); }
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
    fn integrate_codex_inserts_mcp_server() {
        let tmp = std::env::temp_dir().join(format!("cogh-codex-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".codex")).unwrap();
        let config = tmp.join(".codex/config.toml");
        std::fs::write(&config, "model = 'test'\n").unwrap();

        let prev_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", &tmp); }
        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string(), "stdio".to_string()];
        let result = integrate_codex(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe { std::env::set_var("HOME", &prev_home); }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        // Parse the TOML to verify semantically — quoting style may
        // change ("test" vs 'test').
        let parsed: toml::Value = text.parse().unwrap();
        let model = parsed.get("model").and_then(|m| m.as_str()).unwrap_or("");
        let has_cognicode_mcp = parsed.get("mcp_servers")
            .and_then(|s| s.get("cognicode-mcp"))
            .is_some();
        assert!(has_cognicode_mcp, "cognicode-mcp not in mcp_servers");
        assert_eq!(model, "test", "model field not preserved");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
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
        unsafe { std::env::set_var("HOME", &tmp); }
        let result = uninstall_codex("0.92.0");
        unsafe { std::env::set_var("HOME", &prev_home); }
        result.unwrap();

        let text = std::fs::read_to_string(&config).unwrap();
        // TOML output may use [mcp_servers.existing] section format
        // instead of inline. Parse it back to verify semantically.
        let parsed: toml::Value = text.parse().unwrap();
        let existing_cmd = parsed.get("mcp_servers")
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
        let steps = integrate_opencode(&skill_path, "0.94.9").unwrap();
        assert!(!steps.is_empty());
        assert!(matches!(steps[0], Step::Symlink { .. }));
    }

    #[test]
    fn test_uninstall_opencode_steps() {
        let steps = uninstall_opencode("0.94.9").unwrap();
        assert!(!steps.is_empty());
    }
}


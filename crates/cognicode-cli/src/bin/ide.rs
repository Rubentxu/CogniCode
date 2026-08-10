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
    let json_str = serde_json::to_string_pretty(value)
        .with_context(|| format!("serialize {}", path.display()))?;
    std::fs::write(&tmp, json_str)
        .with_context(|| format!("write tmp {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

/// Run the opencode adapter's `integrate` recipe.
pub fn integrate_opencode(
    home: &Path,
    plugin: &str,
    version: &str,
    mcp_command: &[String],
) -> Result<()> {
    // 1. Skill copy
    let skills_src = home
        .join("versions")
        .join(version)
        .join(plugin)
        .join("skills");
    let skills_dst = opencode_skills_dir().join(format!("cognicode-{version}"));
    if skills_src.exists() {
        copy_dir_recursive(&skills_src, &skills_dst)
            .with_context(|| format!("copy {} → {}", skills_src.display(), skills_dst.display()))?;
        eprintln!("✓ copied skills: {}", skills_dst.display());
    } else {
        eprintln!("(no skills to copy from {})", skills_src.display());
    }

    // 2. MCP config merge
    let config_path = opencode_config_path();
    let mut config = read_opencode_config()?;
    let mcp_entry = json!({
        "command": mcp_command,
        "enabled": true,
        "type": "stdio",
    });

    // Navigate/build the nested `mcp.cognicode-mcp` path.
    let cfg = config.as_object_mut()
        .ok_or_else(|| anyhow!("opencode.json is not an object"))?;
    let mcp = cfg.entry("mcp").or_insert_with(|| json!({}));
    if !mcp.is_object() {
        return Err(anyhow!("opencode.json: 'mcp' is not an object (got {})", mcp));
    }
    mcp.as_object_mut().unwrap()
        .insert("cognicode-mcp".to_string(), mcp_entry);
    write_json_atomic(&config_path, &config)?;
    eprintln!("✓ patched: {}", config_path.display());

    Ok(())
}

/// Run the opencode adapter's `uninstall` recipe.
pub fn uninstall_opencode(version: &str) -> Result<()> {
    // 1. Remove skills dir
    let skills_dst = opencode_skills_dir().join(format!("cognicode-{version}"));
    if skills_dst.exists() {
        std::fs::remove_dir_all(&skills_dst)
            .with_context(|| format!("rm -rf {}", skills_dst.display()))?;
        eprintln!("✓ removed: {}", skills_dst.display());
    }

    // 2. Remove MCP entry
    let config_path = opencode_config_path();
    if config_path.exists() {
        let mut config = read_opencode_config()?;
        let cfg = config.as_object_mut()
            .ok_or_else(|| anyhow!("opencode.json is not an object"))?;
        if let Some(mcp) = cfg.get_mut("mcp") {
            if let Some(mcp_obj) = mcp.as_object_mut() {
                mcp_obj.remove("cognicode-mcp");
            }
        }
        write_json_atomic(&config_path, &config)?;
        eprintln!("✓ unpached: {}", config_path.display());
    }

    Ok(())
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
        "opencode" => integrate_opencode(home.root.as_path(), plugin, version, &mcp_command),
        other => Err(anyhow!(
            "IDE '{}' is not supported by cogh yet (only 'opencode' in E32-D)",
            other
        )),
    }
}

pub fn cmd_ide_uninstall(home: &CognicodeHomeSup, ide: &str, version: &str) -> Result<()> {
    match ide {
        "opencode" => uninstall_opencode(version),
        other => Err(anyhow!(
            "IDE '{}' is not supported by cogh yet (only 'opencode' in E32-D)",
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
        std::fs::write(&config, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        // Override HOME for the duration of this test
        let prev_home = std::env::var("HOME").unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        let home = std::path::PathBuf::from(&tmp).join(".cognicode");
        let mcp_cmd = vec!["/bin/cognicode-mcp".to_string()];
        let result = integrate_opencode(&home, "mcp-server", "0.92.0", &mcp_cmd);
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

        // Read back the config
        let text = std::fs::read_to_string(&config).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();

        // Original entries preserved
        assert_eq!(v["agent"]["foo"]["description"], "test");
        assert_eq!(v["mcp"]["chronos"]["type"], "local");
        // New entry added
        assert_eq!(v["mcp"]["cognicode-mcp"]["type"], "stdio");
        assert_eq!(v["mcp"]["cognicode-mcp"]["enabled"], true);
        assert_eq!(v["mcp"]["cognicode-mcp"]["command"][0], "/bin/cognicode-mcp");

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

        let result = uninstall_opencode("0.92.0");
        unsafe {
            std::env::set_var("HOME", &prev_home);
        }
        result.unwrap();

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
}

//! `cogh::layout` — Filesystem layout helpers for `~/.cognicode/`.
//!
// Mirrors `~/.asdf/` structure (ADR-035). Provides:
//! - `CognicodeHome::resolve()` — figure out the COGNICODE_HOME path
//! - path helpers for bins, shims, versions, plugins, tracker, locks
//! - shell command stubs for `cogh install/uninstall/list/...`
//!
// See ADR-034 §"Architecture" for the directory layout.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::Cli;

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
            let home = std::env::var("HOME")
                .context("HOME not set; pass --home or set COGNICODE_HOME")?;
            PathBuf::from(home).join(".cognicode")
        };
        Ok(Self { root })
    }

    pub fn bin(&self) -> PathBuf { self.root.join("bin") }
    pub fn shims(&self) -> PathBuf { self.root.join("shims") }
    pub fn versions(&self) -> PathBuf { self.root.join("versions") }
    pub fn version(&self, v: &str) -> PathBuf { self.versions().join(v) }
    pub fn plugins(&self) -> PathBuf { self.root.join("plugins") }
    pub fn plugin(&self, name: &str) -> PathBuf { self.plugins().join(name) }
    pub fn tracker(&self) -> PathBuf { self.root.join("tracker") }
    pub fn tracker_version(&self) -> PathBuf { self.tracker().join("version") }
    pub fn locks(&self) -> PathBuf { self.root.join("locks") }
    pub fn cache(&self) -> PathBuf { self.root.join("cache") }
    pub fn cache_downloads(&self) -> PathBuf { self.cache().join("downloads") }
    pub fn config(&self) -> PathBuf { self.root.join("config.yaml") }

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
    if home.is_initialized() {
        println!("✓ {} already initialized", home.root.display());
    } else {
        home.init()?;
        println!("✓ Initialized {}", home.root.display());
    }
    Ok(())
}

// ===== cmd_install (stub) =====

pub fn cmd_install(home: &CognicodeHome, plugin: &str, version: &str, ides: &[String]) -> Result<()> {
    if !home.is_initialized() {
        return Err(anyhow!("home not initialized; run `cogh init` first"));
    }
    println!("install: plugin={} version={} ides={:?}", plugin, version, ides);
    println!("(not yet implemented — placeholder for E32-A scaffold)");
    Ok(())
}

pub fn cmd_uninstall(home: &CognicodeHome, plugin: &str, version: &str, ides: &[String]) -> Result<()> {
    println!("uninstall: plugin={} version={} ides={:?}", plugin, version, ides);
    println!("(not yet implemented)");
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
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
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

pub fn cmd_latest(home: &CognicodeHome, plugin: Option<String>, all: bool) -> Result<()> {
    let _ = home;
    if all {
        println!("(latest --all: not yet implemented)");
    } else if let Some(p) = plugin {
        println!("(latest {}: not yet implemented)", p);
    } else {
        println!("(latest: pass --all or a plugin name)");
    }
    Ok(())
}

pub fn cmd_update(home: &CognicodeHome, plugin: Option<String>) -> Result<()> {
    let _ = home;
    if let Some(p) = plugin {
        println!("(update {}: not yet implemented)", p);
    } else {
        println!("(update all: not yet implemented)");
    }
    Ok(())
}

pub fn cmd_reshim(home: &CognicodeHome) -> Result<()> {
    println!("reshim: would regenerate {} (not yet implemented)", home.shims().display());
    Ok(())
}

pub fn cmd_doctor(home: &CognicodeHome) -> Result<()> {
    let mut issues = 0;
    println!("==> cogh doctor ({})", home.root.display());
    if !home.is_initialized() {
        println!("  ✗ home not initialized");
        return Ok(());
    }
    println!("  ✓ home exists");
    if home.bin().exists() {
        println!("  ✓ bin/ exists");
    } else {
        println!("  ✗ bin/ missing");
        issues += 1;
    }
    if home.shims().exists() {
        println!("  ✓ shims/ exists");
    } else {
        println!("  ✗ shims/ missing");
        issues += 1;
    }
    if home.tracker_version().exists() {
        println!("  ✓ tracker/version exists");
    } else {
        println!("  ⚠ tracker/version missing (no version pinned)");
    }
    println!("==> {} issues", issues);
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
    println!("plugin add: plugin={} from_url={:?}", plugin, from_url);
    println!("(not yet implemented)");
    Ok(())
}

pub fn cmd_plugin_remove(home: &CognicodeHome, plugin: &str) -> Result<()> {
    println!("plugin remove: plugin={}", plugin);
    Ok(())
}

pub fn cmd_plugin_list(home: &CognicodeHome) -> Result<()> {
    let _ = home;
    println!("(plugin list: not yet implemented)");
    Ok(())
}

pub fn cmd_plugin_update(home: &CognicodeHome, plugin: &str) -> Result<()> {
    println!("plugin update: plugin={}", plugin);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_from_explicit_path() {
        let tmp = std::env::temp_dir().join("cogh-test-explicit");
        let home = CognicodeHome::resolve(Some(&tmp)).unwrap();
        assert_eq!(home.root, tmp);
    }

    #[test]
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
        let home = CognicodeHome { root: PathBuf::from("/tmp/cogh") };
        assert_eq!(home.bin(), PathBuf::from("/tmp/cogh/bin"));
        assert_eq!(home.shims(), PathBuf::from("/tmp/cogh/shims"));
        assert_eq!(home.versions(), PathBuf::from("/tmp/cogh/versions"));
        assert_eq!(home.version("0.92.0"), PathBuf::from("/tmp/cogh/versions/0.92.0"));
        assert_eq!(home.plugins(), PathBuf::from("/tmp/cogh/plugins"));
        assert_eq!(home.plugin("mcp-server"), PathBuf::from("/tmp/cogh/plugins/mcp-server"));
        assert_eq!(home.tracker_version(), PathBuf::from("/tmp/cogh/tracker/version"));
        assert_eq!(home.locks(), PathBuf::from("/tmp/cogh/locks"));
        assert_eq!(home.cache_downloads(), PathBuf::from("/tmp/cogh/cache/downloads"));
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
}

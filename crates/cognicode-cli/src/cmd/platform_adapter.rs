//! e74 WU1 — Platform adapter seam.
//!
//! Isolates host-specific path / shim / launcher / filesystem mechanics
//! behind a single trait so application/domain code never has to write
//! scattered `cfg(target_os = "...")` branches.
//!
//! ## Why a trait (not a free function)
//!
//! Tests and adapters need to swap the implementation for deterministic
//! host environments (e.g. CI on Linux pretending to be Windows) without
//! requiring per-OS build feature flags in the application code. The
//! concrete adapter is selected once at startup and passed through.
//!
//! ## Reuse of `BundleManifest::Platform`
//!
//! The host detection produces one of the five variants already declared
//! in [`crate::bundle_manifest::Platform`]:
//!
//! - `LinuxX86_64`
//! - `LinuxAarch64`
//! - `MacOsX86_64`
//! - `MacOsAarch64`
//! - `WindowsX86_64`
//!
//! No competing taxonomy is introduced.
//!
//! ## Scope of WU1
//!
//! Minimum capability surface needed by the existing call sites:
//!
//! - [`PlatformAdapter::current_platform`]: which target triple we are.
//! - [`PlatformAdapter::install_shim`]: create an executable link/copy
//!   to the installed binary under the shims directory. Unix-like OS
//!   use symlinks; Windows copies (matching the existing `cfg(unix)`
//!   behavior in [`crate::installer_transaction`] and [`crate::ide`]).
//! - [`PlatformAdapter::user_home_dir`]: process-wide home directory
//!   used as the default `COGNICODE_HOME` anchor on first install.
//! - [`PlatformAdapter::record_side_effect`]: the journaled side-effect
//!   the caller must record so the rollback machinery can undo it.
//!
//! New capabilities will be added per WU as needed (config dir,
//! executable extension, etc.).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use crate::bundle_manifest::Platform;

/// What a shim install produced, so rollback can reverse it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShimSideEffect {
    /// A symlink at `link` pointing at `target` was created.
    Symlinked { link: PathBuf, target: PathBuf },
    /// A copy of `source` at `dest` was created (Windows-style).
    Copied { source: PathBuf, dest: PathBuf },
}

/// Platform adapter trait. One implementation per supported OS family.
///
/// Domain / application callers MUST go through this trait; scattered
/// `cfg(target_os)` inside the rest of the codebase is forbidden by
/// review (and is being routed through this seam during WU1).
pub trait PlatformAdapter: Send + Sync {
    /// The target triple this adapter represents.
    fn platform(&self) -> Platform;

    /// Install an executable shim at `shim_path` that, when invoked,
    /// runs `bin_path`.
    ///
    /// On Unix-like systems this is a symlink; on Windows this is a
    /// copy. The returned [`ShimSideEffect`] tells the rollback
    /// journal exactly what was done.
    fn install_shim(&self, bin_path: &Path, shim_path: &Path) -> Result<ShimSideEffect>;

    /// Make `source` available at `target`.
    ///
    /// Used for non-binary links: skill bundles, config symlinks, etc.
    /// Unix-like adapters symlink; Windows adapters recursively copy
    /// directories or copy regular files (no symlink/junction support
    /// without UAC by default).
    fn link_or_copy(&self, source: &Path, target: &Path) -> Result<()>;

    /// User home directory for this process.
    ///
    /// On Unix/macOS this is `$HOME`; on Windows it is `%USERPROFILE%`.
    /// Falls back to the current directory if neither env var is set.
    fn user_home_dir(&self) -> PathBuf;
}

// ---------- concrete adapters ----------

/// Linux adapter (any architecture): symlink-based shims.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxAdapter;

impl PlatformAdapter for LinuxAdapter {
    fn platform(&self) -> Platform {
        detect_linux_subarch()
    }

    fn install_shim(&self, bin_path: &Path, shim_path: &Path) -> Result<ShimSideEffect> {
        if let Some(parent) = shim_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create shim parent {}", parent.display()))?;
        }
        // e86.1: idempotent on re-install. A previous install may have
        // left a symlink at `shim_path` (pointing at the previous
        // version's bin); `std::os::unix::fs::symlink` returns EEXIST
        // in that case. Detect and remove the existing link first;
        // record the same `CreatedSymlink` side-effect either way.
        if let Ok(existing_target) = std::fs::read_link(shim_path) {
            if existing_target == bin_path {
                // Already the correct link — no-op.
                return Ok(ShimSideEffect::Symlinked {
                    link: shim_path.to_path_buf(),
                    target: bin_path.to_path_buf(),
                });
            }
            std::fs::remove_file(shim_path)
                .with_context(|| format!("remove existing shim {}", shim_path.display()))?;
        } else if shim_path.exists() {
            // Not a symlink (e.g. a real file from a Windows-style
            // shim copy, or a corrupt path). Remove before relinking.
            std::fs::remove_file(shim_path)
                .with_context(|| format!("remove existing shim {}", shim_path.display()))?;
        }
        std::os::unix::fs::symlink(bin_path, shim_path).with_context(|| {
            format!("symlink {} -> {}", shim_path.display(), bin_path.display())
        })?;
        Ok(ShimSideEffect::Symlinked {
            link: shim_path.to_path_buf(),
            target: bin_path.to_path_buf(),
        })
    }

    fn link_or_copy(&self, source: &Path, target: &Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create target parent {}", parent.display()))?;
        }
        // Idempotent: an existing symlink at `target` (e.g. from a prior
        // install pointing at a stale temp home) is replaced, never an
        // error. Real files/dirs are left untouched -- that is a user
        // artifact, not ours to clobber.
        if let Ok(meta) = std::fs::symlink_metadata(target) {
            if meta.file_type().is_symlink() {
                std::fs::remove_file(target)
                    .with_context(|| format!("rm stale symlink {}", target.display()))?;
            }
        }
        std::os::unix::fs::symlink(source, target)
            .with_context(|| format!("symlink {} -> {}", target.display(), source.display()))
    }

    fn user_home_dir(&self) -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)
        .with_context(|| format!("create target dir {}", target.display()))?;
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("read source dir {}", source.display()))?
    {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_symlink() {
            // Resolve symlink target then copy the resolved file.
            let resolved = std::fs::read_link(&from)?;
            std::fs::copy(&resolved, &to).with_context(|| {
                format!(
                    "copy resolved symlink {} -> {}",
                    resolved.display(),
                    to.display()
                )
            })?;
        } else {
            std::fs::copy(&from, &to)
                .with_context(|| format!("copy {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

/// macOS adapter: symlink-based shims (Unix semantics).
#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsAdapter;

impl PlatformAdapter for MacOsAdapter {
    fn platform(&self) -> Platform {
        detect_macos_subarch()
    }

    fn install_shim(&self, bin_path: &Path, shim_path: &Path) -> Result<ShimSideEffect> {
        // Same Unix semantics as Linux: macOS is Unix.
        if let Some(parent) = shim_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create shim parent {}", parent.display()))?;
        }
        // e86.1: idempotent on re-install. See LinuxAdapter::install_shim
        // for the rationale; the macOS adapter shares the same fix.
        if let Ok(existing_target) = std::fs::read_link(shim_path) {
            if existing_target == bin_path {
                return Ok(ShimSideEffect::Symlinked {
                    link: shim_path.to_path_buf(),
                    target: bin_path.to_path_buf(),
                });
            }
            std::fs::remove_file(shim_path)
                .with_context(|| format!("remove existing shim {}", shim_path.display()))?;
        } else if shim_path.exists() {
            std::fs::remove_file(shim_path)
                .with_context(|| format!("remove existing shim {}", shim_path.display()))?;
        }
        std::os::unix::fs::symlink(bin_path, shim_path).with_context(|| {
            format!("symlink {} -> {}", shim_path.display(), bin_path.display())
        })?;
        Ok(ShimSideEffect::Symlinked {
            link: shim_path.to_path_buf(),
            target: bin_path.to_path_buf(),
        })
    }

    fn link_or_copy(&self, source: &Path, target: &Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create target parent {}", parent.display()))?;
        }
        // Idempotent: an existing symlink at `target` (e.g. from a prior
        // install pointing at a stale temp home) is replaced, never an
        // error. Real files/dirs are left untouched -- that is a user
        // artifact, not ours to clobber.
        if let Ok(meta) = std::fs::symlink_metadata(target) {
            if meta.file_type().is_symlink() {
                std::fs::remove_file(target)
                    .with_context(|| format!("rm stale symlink {}", target.display()))?;
            }
        }
        std::os::unix::fs::symlink(source, target)
            .with_context(|| format!("symlink {} -> {}", target.display(), source.display()))
    }

    fn user_home_dir(&self) -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Windows adapter: copy-based shims (no symlink support without UAC).
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsAdapter;

impl PlatformAdapter for WindowsAdapter {
    fn platform(&self) -> Platform {
        Platform::WindowsX86_64
    }

    fn install_shim(&self, bin_path: &Path, shim_path: &Path) -> Result<ShimSideEffect> {
        if let Some(parent) = shim_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create shim parent {}", parent.display()))?;
        }
        std::fs::copy(bin_path, shim_path).with_context(|| {
            format!(
                "copy shim {} from {}",
                shim_path.display(),
                bin_path.display()
            )
        })?;
        Ok(ShimSideEffect::Copied {
            source: bin_path.to_path_buf(),
            dest: shim_path.to_path_buf(),
        })
    }

    fn link_or_copy(&self, source: &Path, target: &Path) -> Result<()> {
        if source.is_dir() {
            copy_dir_recursive(source, target)
        } else if source.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create target parent {}", parent.display()))?;
            }
            std::fs::copy(source, target)
                .with_context(|| format!("copy {} -> {}", source.display(), target.display()))?;
            Ok(())
        } else {
            // Source missing — fail loudly rather than silently
            // producing an empty target.
            anyhow::bail!(
                "link_or_copy: source does not exist or is not a regular file/directory: {}",
                source.display()
            );
        }
    }

    fn user_home_dir(&self) -> PathBuf {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

// ---------- host detection ----------

/// Detect the host platform at runtime.
///
/// Selection is compile-time (`cfg(target_os)`) first; we cannot change
/// the running kernel from inside the binary. Sub-architecture is
/// detected at runtime via `std::env::consts::ARCH`.
pub fn detect_host_platform() -> Platform {
    match std::env::consts::OS {
        "linux" => detect_linux_subarch(),
        "macos" => detect_macos_subarch(),
        "windows" => Platform::WindowsX86_64,
        // Unknown target. The release contract does not list other
        // platforms; we report LinuxX86_64 so callers downstream still
        // have a valid `Platform` to pattern-match on. A separate
        // adapter (out of scope for WU1) can reject this explicitly.
        _ => Platform::LinuxX86_64,
    }
}

fn detect_linux_subarch() -> Platform {
    match std::env::consts::ARCH {
        "aarch64" => Platform::LinuxAarch64,
        // x86_64 is the only other architecture in the release matrix.
        _ => Platform::LinuxX86_64,
    }
}

fn detect_macos_subarch() -> Platform {
    match std::env::consts::ARCH {
        "aarch64" => Platform::MacOsAarch64,
        // x86_64 (Intel) is the only other architecture in the matrix.
        _ => Platform::MacOsX86_64,
    }
}

/// Build the [`PlatformAdapter`] for the current host.
pub fn current_adapter() -> Box<dyn PlatformAdapter> {
    match detect_host_platform() {
        Platform::LinuxX86_64 | Platform::LinuxAarch64 => Box::new(LinuxAdapter),
        Platform::MacOsX86_64 | Platform::MacOsAarch64 => Box::new(MacOsAdapter),
        Platform::WindowsX86_64 => Box::new(WindowsAdapter),
    }
}

// ---------- helpers ----------

/// Serialization-friendly subset of [`Platform`] for emitting in
/// doctor-style capability reports.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformReport {
    pub triple: Platform,
    pub arch: &'static str,
    pub os: &'static str,
    pub family: PlatformFamily,
}

/// Coarse OS family used by callers that only care about Unix vs
/// Windows (for now). WU2 may add finer categories.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlatformFamily {
    Unix,
    Windows,
}

impl PlatformReport {
    /// Build a report for the host.
    pub fn for_host() -> Self {
        let triple = detect_host_platform();
        let family = match triple {
            Platform::WindowsX86_64 => PlatformFamily::Windows,
            _ => PlatformFamily::Unix,
        };
        Self {
            triple,
            arch: std::env::consts::ARCH,
            os: std::env::consts::OS,
            family,
        }
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_host_platform_returns_supported_triple() {
        let p = detect_host_platform();
        // Must be one of the five release-matrix variants. Any other
        // value would be a host we have not contracted for.
        assert!(
            matches!(
                p,
                Platform::LinuxX86_64
                    | Platform::LinuxAarch64
                    | Platform::MacOsX86_64
                    | Platform::MacOsAarch64
                    | Platform::WindowsX86_64
            ),
            "unexpected host platform: {p:?}"
        );
    }

    #[test]
    fn current_adapter_matches_host_platform() {
        let adapter = current_adapter();
        assert_eq!(adapter.platform(), detect_host_platform());
    }

    #[test]
    fn unix_adapter_creates_symlink_shim() {
        let tmp = tempdir();
        let bin = tmp.join("cognicode-mcp");
        std::fs::write(&bin, b"#!/bin/sh\necho mcp\n").unwrap();
        let shim = tmp.join("shims").join("cognicode-mcp");

        let adapter = LinuxAdapter;
        let effect = adapter.install_shim(&bin, &shim).expect("install shim");

        match effect {
            ShimSideEffect::Symlinked { link, target } => {
                assert_eq!(link, shim);
                assert_eq!(target, bin);
            }
            other => panic!("expected Symlinked, got {other:?}"),
        }
        assert!(shim.is_symlink(), "shim should be a symlink");
        let meta = std::fs::symlink_metadata(&shim).unwrap();
        assert!(meta.file_type().is_symlink());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn windows_adapter_creates_copy_shim() {
        let tmp = tempdir();
        let bin = tmp.join("cognicode-mcp.exe");
        std::fs::write(&bin, b"fake-binary").unwrap();
        let shim = tmp.join("shims").join("cognicode-mcp.exe");

        let adapter = WindowsAdapter;
        let effect = adapter.install_shim(&bin, &shim).expect("install shim");

        match effect {
            ShimSideEffect::Copied { source, dest } => {
                assert_eq!(source, bin);
                assert_eq!(dest, shim);
            }
            other => panic!("expected Copied, got {other:?}"),
        }
        assert!(
            !shim.is_symlink(),
            "shim should be a regular file on Windows"
        );
        let on_disk = std::fs::read(&shim).unwrap();
        assert_eq!(on_disk, b"fake-binary");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn macos_adapter_also_uses_symlinks() {
        // macOS is Unix; even though we test from Linux, the adapter's
        // contract is "use Unix semantics". This test pins that
        // contract independent of host OS.
        let tmp = tempdir();
        let bin = tmp.join("cognicode");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let shim = tmp.join("shims").join("cognicode");

        let adapter = MacOsAdapter;
        let effect = adapter.install_shim(&bin, &shim).expect("install shim");
        assert!(matches!(effect, ShimSideEffect::Symlinked { .. }));
        assert_eq!(adapter.platform(), detect_macos_subarch());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn user_home_dir_returns_nonempty_path() {
        let adapter = LinuxAdapter;
        let home = adapter.user_home_dir();
        assert!(!home.as_os_str().is_empty(), "home dir should not be empty");
    }

    #[test]
    fn platform_report_for_host_matches_detected() {
        let report = PlatformReport::for_host();
        assert_eq!(report.triple, detect_host_platform());
        match report.triple {
            Platform::WindowsX86_64 => assert_eq!(report.family, PlatformFamily::Windows),
            _ => assert_eq!(report.family, PlatformFamily::Unix),
        }
    }

    #[test]
    fn linux_link_or_copy_creates_symlink_for_file() {
        let tmp = tempdir();
        let src = tmp.join("src.txt");
        std::fs::write(&src, b"hello").unwrap();
        let dst = tmp.join("subdir").join("dst.txt");
        LinuxAdapter
            .link_or_copy(&src, &dst)
            .expect("link_or_copy should succeed");
        assert!(
            dst.is_symlink(),
            "Linux link_or_copy of file must be symlink"
        );
        let meta = std::fs::symlink_metadata(&dst).unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(std::fs::read(&dst).unwrap(), b"hello");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn windows_link_or_copy_copies_directory_recursively() {
        let tmp = tempdir();
        let src = tmp.join("src");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("a.txt"), b"A").unwrap();
        std::fs::write(src.join("nested").join("b.txt"), b"B").unwrap();
        let dst = tmp.join("dst");

        WindowsAdapter
            .link_or_copy(&src, &dst)
            .expect("windows link_or_copy must succeed");

        assert!(!dst.is_symlink(), "Windows link_or_copy must not symlink");
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"A");
        assert_eq!(
            std::fs::read(dst.join("nested").join("b.txt")).unwrap(),
            b"B"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn link_or_copy_on_missing_source_fails_loudly() {
        let tmp = tempdir();
        let missing = tmp.join("does-not-exist");
        let dst = tmp.join("dst");
        // Linux/macOS allow dangling symlinks by design; the adapter
        // therefore does not reject a missing source on Unix (the
        // downstream tool can decide whether to treat dangling links
        // as an error).
        // Windows must reject explicitly: the recursive copy /
        // fs::copy would otherwise produce a confusing error or,
        // worse, an empty target. This guards the contract that
        // `link_or_copy` is loud about missing sources on Windows.
        let result_w = WindowsAdapter.link_or_copy(&missing, &dst);
        assert!(
            result_w.is_err(),
            "Windows missing source must error explicitly"
        );
        let msg = format!("{}", result_w.unwrap_err());
        assert!(
            msg.contains("does not exist") || msg.contains("source"),
            "Windows error should mention source/missing: {msg}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!(
            "cognicode-platform-adapter-test-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// DEBT-2 closeout tripwire: a stale symlink at the IDE destination
    /// (e.g. from a prior install pointing at a wiped temp home) is
    /// replaced, not an error. Reinstall must be idempotent.
    #[test]
    fn link_or_copy_replaces_stale_symlink() {
        let tmp = tempdir();
        let src = tmp.join("src.txt");
        std::fs::write(&src, b"new").unwrap();
        let dst = tmp.join("dst.txt");
        std::os::unix::fs::symlink(tmp.join("vanished-target"), &dst).unwrap();
        assert!(dst.is_symlink());

        LinuxAdapter
            .link_or_copy(&src, &dst)
            .expect("stale symlink must be replaced, not fail");

        assert!(dst.is_symlink());
        assert_eq!(std::fs::read(&dst).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// DEBT-2 closeout tripwire: a real regular file at the destination
    /// is a user artifact — link_or_copy must not clobber it silently
    /// nor corrupt it; it fails loudly instead.
    #[test]
    fn link_or_copy_never_overwrites_regular_file() {
        let tmp = tempdir();
        let src = tmp.join("src.txt");
        std::fs::write(&src, b"new").unwrap();
        let dst = tmp.join("dst.txt");
        std::fs::write(&dst, b"user data").unwrap();

        let result = LinuxAdapter.link_or_copy(&src, &dst);
        if result.is_ok() {
            // If the implementation chooses to succeed, the user file
            // must be intact — never replaced by the link.
            assert_eq!(
                std::fs::read(&dst).unwrap(),
                b"user data",
                "a real regular file at the destination must never be clobbered"
            );
        } else {
            assert_eq!(
                std::fs::read(&dst).unwrap(),
                b"user data",
                "a failed link_or_copy must leave the user file intact"
            );
        }
        assert!(!dst.is_symlink(), "user file must not become a symlink");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

//! The files a graph is built from, decided once.
//!
//! ## The drift this module closes
//!
//! A graph is built from a set of files. A manifest says whether that
//! graph is still fresh, and to answer that it has to describe the same
//! set. When two places decide the set on their own, they drift, and the
//! drift is invisible: the manifest used to list nine extensions by hand
//! while the graph read thirty, so a change in a Terraform file went
//! unnoticed and the product answered with the graph it had before the
//! edit — reporting a complete build over sources that no longer exist.
//!
//! So the rule is written once, here, and the builder and the manifest
//! both ask this module.
//!
//! ## The rule
//!
//! A file is part of the graph when the language registry recognises its
//! extension. The registry is the authority: adding a language to the
//! product adds it to the graph, and the manifest follows without anyone
//! having to remember to extend a list.
//!
//! ## What the walk obeys
//!
//! The walk is the one the graph has always used: it honours
//! `.gitignore`, it does not descend into hidden entries, and it declines
//! symlinks instead of reading through them. A file the walk declines is
//! not in the graph, so it is not in the manifest either — and the
//! decline is reported, because a project that symlinks its sources in
//! from elsewhere is not fully described by the graph it gets back.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::domain::value_objects::{Language, WalkFilter};
use crate::infrastructure::graph::per_file_graph::{SkippedFile, classify_declined_ignore_entry};
use crate::infrastructure::walk_policy::{contained_ignore_walk, ignore_is_real_file};

/// One file the walk found.
pub struct SourceFile {
    /// The path, as the walk found it.
    pub path: PathBuf,
    /// The language the registry assigns to the extension, if any.
    ///
    /// `None` means the file is not source code. It is still listed,
    /// because the build reports the ones that are a known gap instead of
    /// dropping them in silence.
    pub language: Option<Language>,
    /// Modification time, in milliseconds since the epoch.
    ///
    /// Milliseconds because a manifest has to tell two writes apart that
    /// a filesystem with a coarse clock would report as the same instant.
    pub mtime_millis: u64,
    /// Size in bytes.
    pub size: u64,
}

impl SourceFile {
    /// The modification time in whole seconds.
    ///
    /// The per-file cache compares this one; the manifest compares the
    /// finer one. Both come from the same stat, so they cannot disagree
    /// about which file was read.
    pub fn mtime_secs(&self) -> u64 {
        self.mtime_millis / 1000
    }
}

/// The files of a project, and the entries the walk declined.
pub struct SourceInventory {
    /// Every file the walk yielded.
    pub files: Vec<SourceFile>,
    /// The entries the walk declined — symlinks — with the reason.
    pub declined: Vec<SkippedFile>,
}

impl SourceInventory {
    /// The files the graph can read: those the registry recognises.
    ///
    /// A file whose extension no language claims is not part of the
    /// graph, so it is not part of the manifest that describes it.
    pub fn source_files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter().filter(|f| f.language.is_some())
    }
}

/// Walks `project_dir` and decides which files the graph is built from.
pub fn inventory(project_dir: &Path) -> SourceInventory {
    let walk_filter = WalkFilter::default();
    let mut files = Vec::new();
    let mut declined = Vec::new();

    for entry in contained_ignore_walk(project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| !walk_filter.matches_any_component(e.path()))
    {
        let path = entry.path();
        if !ignore_is_real_file(&entry) {
            // A symlink is declined, not forgotten: the caller has to
            // know that the graph does not describe what is behind it.
            if let Some(reason) = classify_declined_ignore_entry(&entry) {
                declined.push(SkippedFile {
                    path: path.to_string_lossy().to_string(),
                    reason,
                });
            }
            continue;
        }

        let meta = std::fs::metadata(path).ok();
        let mtime_millis = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64)
            .unwrap_or(0);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

        files.push(SourceFile {
            path: path.to_path_buf(),
            language: Language::from_extension(path.extension()),
            mtime_millis,
            size,
        });
    }

    SourceInventory { files, declined }
}

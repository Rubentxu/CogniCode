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
//!
//! ## What the walk could not do
//!
//! A walk is not infallible: a directory the process cannot read, an
//! entry that vanishes between the read and the stat. Those are not
//! files it left out on purpose — they are failures, and they leave a
//! hole in the coverage. The inventory keeps them apart from the
//! decisions, because the two are not the same thing: one of them says
//! "there is nothing there to see" and the other says "there might
//! have been something there and we did not see it".
//!
//! A walk that failed silently used to produce a `Complete` build over
//! a tree it had never seen. That is the same lie as reporting a
//! complete build over sources that no longer exist, and it is closed
//! the same way: the failure is counted, and whoever reads the report
//! can see it.

use std::path::{Path, PathBuf};

use crate::domain::value_objects::file_time::{measured_millis, seconds_of_millis};
use crate::domain::value_objects::{Language, WalkFilter};
use crate::infrastructure::graph::per_file_graph::{
    SkipReason, SkippedFile, WalkClassification, classify_declined_ignore_entry,
    classify_walk_extension, ignore_walk_error, walk_error,
};
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
    ///
    /// `None` when the filesystem does not report a time. A `Some(0)` is a
    /// file written at the epoch, which is a measurement; a `None` is a
    /// file whose clock nobody read, which is not one.
    pub mtime_millis: Option<i64>,
    /// Size in bytes.
    pub size: u64,
}

impl SourceFile {
    /// The modification time in whole seconds.
    ///
    /// The per-file cache compares this one; the manifest compares the
    /// finer one. Both come from the same stat, so they cannot disagree
    /// about which file was read.
    pub fn mtime_secs(&self) -> Option<i64> {
        seconds_of_millis(self.mtime_millis)
    }
}

/// The files of a project, and everything the walk said about the rest.
///
/// Three buckets, so a caller can tell a failure from a decision:
///
/// - `files`: the files the walk yielded. Each one's `language` says
///   whether the graph can read it.
/// - `declined`: the entries the walk refused to follow — symlinks. A
///   decision, but one that leaves a hole: the project is not fully
///   described by the graph it gets back.
/// - `errors`: what the traversal could not do — a directory it could
///   not read, an entry it could not measure. Not a decision: a
///   failure, and it has to be counted.
///
/// Whether a file is a source, an honest gap (`.kt`, `.kts`) or a
/// legitimate exclusion (a README, a lockfile) is *not* a fourth
/// bucket: it is a property of the file, and it is decided by the one
/// classifier that owns it. [`SourceInventory::known_gaps`] and
/// [`SourceInventory::legit_exclusions`] are two views of the same
/// files, not two copies of them.
///
/// The holes of a walk —what it declined, what it could not do and what
/// it could not read yet— are composed in one place,
/// [`SourceInventory::coverage_holes`], so every route that publishes
/// them publishes the same ones.
pub struct SourceInventory {
    /// Every file the walk yielded and could measure.
    pub files: Vec<SourceFile>,
    /// The entries the walk declined — symlinks — with the reason.
    pub declined: Vec<SkippedFile>,
    /// The errors the traversal produced, with the reason.
    pub errors: Vec<SkippedFile>,
}

impl SourceInventory {
    /// The files the graph can read: those the registry recognises.
    ///
    /// A file whose extension no language claims is not part of the
    /// graph, so it is not part of the manifest that describes it.
    pub fn source_files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter().filter(|f| f.language.is_some())
    }

    /// The sources in a language we know about but do not support yet
    /// (`.kt`, `.kts`): an honest gap, with the reason that names it.
    ///
    /// A gap travels with its reason so that whoever publishes it does
    /// not have to rebuild it — and so that a gap cannot be read as a
    /// decision.
    pub fn known_gaps(&self) -> Vec<SkippedFile> {
        self.files
            .iter()
            .filter(|f| matches!(classify(f), WalkClassification::KnownUnsupportedGap))
            .map(|f| SkippedFile {
                path: f.path.to_string_lossy().into_owned(),
                reason: SkipReason::UnsupportedExtension(extension_of(&f.path)),
            })
            .collect()
    }

    /// The holes in the coverage of this tree, in one list.
    ///
    /// Three things leave the graph incomplete: what the walk DECLINED
    /// (a symlink it refused to follow), what it could not DO (a
    /// directory it could not read, an entry it could not measure), and
    /// what it could not READ yet (a language nobody supports yet).
    ///
    /// They are one list because a consumer cannot act on three lists:
    /// what it has to know is whether the graph it holds describes the
    /// whole tree. Three routes that compose this list on their own are
    /// three answers waiting to drift apart, so the composition lives
    /// here and the routes call it.
    pub fn coverage_holes(&self) -> Vec<SkippedFile> {
        let mut holes = Vec::with_capacity(self.declined.len() + self.errors.len());
        holes.extend(self.declined.iter().cloned());
        holes.extend(self.errors.iter().cloned());
        holes.extend(self.known_gaps());
        holes
    }

    /// The files the analysis leaves out on purpose: a README, a
    /// lockfile, an image. A decision, not a gap — the coverage is
    /// complete without them, and nothing has to be reported about
    /// them.
    pub fn legit_exclusions(&self) -> Vec<&SourceFile> {
        self.files
            .iter()
            .filter(|f| matches!(classify(f), WalkClassification::LegitExclusion))
            .collect()
    }
}

/// The classification of a file, decided by the classifier that owns it.
///
/// The registry is the same one `SourceFile::language` is read from, so
/// "is this a source?" and "is this a gap?" cannot disagree.
fn classify(file: &SourceFile) -> WalkClassification {
    classify_walk_extension(file.path.extension().and_then(|e| e.to_str()))
}

/// The extension of a path, spelled the way the registry spells it.
fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Walks `project_dir` and decides which files the graph is built from.
pub fn inventory(project_dir: &Path) -> SourceInventory {
    let walk_filter = WalkFilter::default();
    let mut files = Vec::new();
    let mut declined = Vec::new();
    let mut errors = Vec::new();

    for entry in contained_ignore_walk(project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build()
    {
        // An error of the walk is not a file it left out: it is a
        // failure of the traversal, and it leaves a hole. It is kept
        // apart from the decisions so that the caller can count it.
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                errors.push(ignore_walk_error(&err));
                continue;
            }
        };

        let path = entry.path();
        if walk_filter.matches_any_component(path, project_dir) {
            continue;
        }
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

        // A file the walk yields but cannot measure is not a measured
        // file. Publishing it with `mtime 0, size 0` would say "this
        // file was last written at the epoch", which is a fact we do
        // not know; the failure stays a failure.
        match measure(path, std::fs::metadata(path)) {
            Ok(file) => files.push(file),
            Err(err) => {
                errors.push(walk_error(Some(path), &err));
                continue;
            }
        }
    }

    SourceInventory {
        files,
        declined,
        errors,
    }
}

/// The measurement of a file the walk yielded.
///
/// A file the walk delivers and that cannot be measured is not a
/// measured file. Publishing it with `mtime 0, size 0` would say "this
/// file was last written at the epoch", which is a fact we do not
/// know; so the failure is returned as a failure, with its cause, and
/// whoever walks decides what to do with it.
fn measure(
    path: &Path,
    meta: std::io::Result<std::fs::Metadata>,
) -> Result<SourceFile, std::io::Error> {
    let meta = meta?;
    Ok(SourceFile {
        path: path.to_path_buf(),
        language: Language::from_extension(path.extension()),
        // La fecha que el sistema de ficheros publica, o ninguna si no la
        // publica. El fichero se midio; lo que puede faltar es su reloj.
        mtime_millis: measured_millis(meta.modified().ok()),
        size: meta.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decision, not the race.
    ///
    /// The filesystem can only fail to measure a file by removing it
    /// between the read and the stat, and that is not reproducible
    /// without a race. What IS reproducible — and what the contract
    /// asks for — is the decision: a file that could not be measured
    /// does not come out as a measured file with a zeroed timestamp.
    #[test]
    fn a_file_that_cannot_be_measured_is_not_a_measured_file() {
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "EACCES");
        let out = measure(Path::new("src/main.rs"), Err(denied));
        assert!(
            out.is_err(),
            "un fichero que no se puede medir no se publica como medido: {:?}",
            out.map(|f| f.mtime_millis)
        );
    }

    /// The other side of the same decision: a file that CAN be measured
    /// is published with the facts of its measurement, and not with
    /// zeroes.
    #[test]
    fn a_measured_file_publishes_its_measurement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").expect("main.rs");
        let expected_size = std::fs::metadata(&path).expect("metadata").len();

        let file = measure(&path, std::fs::metadata(&path)).expect("the file is measurable");

        assert_eq!(file.size, expected_size);
        assert!(
            file.mtime_millis.is_some_and(|mtime| mtime > 0),
            "una medida real no es cero"
        );
        assert!(
            file.language.is_some(),
            "una fuente que el registro reconoce se publica con su lenguaje"
        );
    }
}

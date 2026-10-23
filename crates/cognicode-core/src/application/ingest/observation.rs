//! Where the product looks, and which files it observes there.
//!
//! The inventory walks a workspace and the watcher watches it. Both ask the
//! same two questions — what the file is, and whether it lives where the
//! product looks — and both get their answers from here, so the two never
//! disagree about what the product reads.

use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use ignore::IncrementalIgnore;

use crate::application::ingest::scan::{classify_file, is_noise_dir, workspace_rules};
use crate::application::ingest::types::FileType;

/// The files of a workspace the product observes.
///
/// The observation is made once, when the workspace is known, and then
/// answers for every file: the inventory asks it while it walks, the
/// watcher asks it when the operating system reports a change, and the
/// endpoint that announces the watcher names what it found.
pub struct Observation {
    root: PathBuf,
    excluded: Mutex<IncrementalIgnore>,
}

impl Observation {
    /// The observation of a workspace.
    pub fn of(root: &Path) -> Self {
        let root = absolute(root);
        let mut matchers = workspace_rules(&root).build_matchers();
        let excluded = matchers
            .pop()
            .expect("a builder with one root has one matcher");
        Observation {
            root,
            excluded: Mutex::new(excluded),
        }
    }

    /// The workspace this observation covers.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether the product observes this path.
    ///
    /// The answer is yes when the product can say what the file is and the
    /// file lives where the product looks: outside the directories the
    /// product does not read into, and outside what the repository itself
    /// excludes. A path outside the workspace is not observed.
    pub fn observes(&self, path: &Path) -> bool {
        let path_in_workspace = absolute(path);
        let relative = match path_in_workspace.strip_prefix(&self.root) {
            Ok(relative) => relative,
            Err(_) => return false,
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        // A directory is not a file the product reads.
        if path.is_dir() {
            return false;
        }
        // A file the product cannot name is a file it does not read.
        if classify_file(path).0 == FileType::Other {
            return false;
        }
        // A file inside a directory the product does not read into is not
        // read either, however well its extension is known.
        if lives_in_a_directory_the_product_does_not_read(relative) {
            return false;
        }
        // What the repository excludes is excluded here too.
        let mut excluded = self
            .excluded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !excluded.matched(relative, false).is_ignore()
    }
}

/// Whether the path lives in a directory the product does not look into.
fn lives_in_a_directory_the_product_does_not_read(relative: &Path) -> bool {
    let Some(parent) = relative.parent() else {
        return false;
    };
    parent.components().any(|component| match component {
        Component::Normal(name) => is_noise_dir(name.to_string_lossy().as_ref()),
        _ => false,
    })
}

/// The path made absolute, without the `.` that a relative path carries.
///
/// The workspace and the files inside it are compared as absolute paths, so
/// that a workspace named relatively and a file named absolutely still
/// meet. The path is not resolved through symlinks: the workspace the
/// caller named is the workspace the files are looked for in.
fn absolute(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(working_directory) => working_directory.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    joined
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}

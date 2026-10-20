//! A reference the graph could not tie to a declaration.
//!
//! Reading a file yields names. Some of them are declarations the file
//! itself holds; the rest point somewhere else: another file of the same
//! project, or nowhere at all. A name that points nowhere is not nothing —
//! it is a fact about what the code says, and dropping it hides from the
//! consumer the very thing it asked about.
//!
//! This value object is that fact: where the reference is written, what it
//! names, what kind of relation it is, and why its far end could not be
//! identified.

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::value_objects::DependencyType;

/// Why the far end of a reference could not be identified.
///
/// The class answers for the scope of the build that reports it: a build
/// that merged three files speaks of those three files, and a build that
/// read the whole project speaks of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingClass {
    /// No declaration in the scope of the build carries that name.
    Unresolved,
    /// More than one does, and the build cannot prove which one the
    /// reference means.
    Ambiguous {
        /// How many declarations carry the name.
        candidates: usize,
    },
}

/// A reference whose far end is not identified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReference {
    /// The file that writes the reference.
    pub origin_file: String,
    /// The line the reference is written on, counting from one.
    pub origin_line: Option<u32>,
    /// The declaration that holds the reference.
    pub source: SymbolId,
    /// The name the reference names, as it was written.
    pub target: String,
    /// What the reference is.
    pub kind: DependencyType,
    /// Why the far end could not be identified.
    pub class: PendingClass,
}

/// The readings of a name that names a path, longest first.
///
/// `aws_s3_bucket.app_bucket.id` can denote the bucket
/// (`aws_s3_bucket.app_bucket`) or something called
/// `aws_s3_bucket.app_bucket.id`, which nobody declares. The longest
/// reading that names a declaration wins; when none does, the whole name
/// stays pending.
///
/// One rule, stated once: the file that writes a reference and the project
/// that reads it agree on what a name can denote.
pub fn readings(name: &str) -> Vec<String> {
    let segments: Vec<&str> = name.split('.').collect();
    (1..=segments.len())
        .rev()
        .map(|take| segments[..take].join("."))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_names_a_path_is_read_from_the_longest_reading() {
        assert_eq!(
            readings("aws_s3_bucket.app_bucket.id"),
            vec![
                "aws_s3_bucket.app_bucket.id".to_string(),
                "aws_s3_bucket.app_bucket".to_string(),
                "aws_s3_bucket".to_string(),
            ]
        );
    }

    #[test]
    fn a_name_that_names_nothing_has_one_reading() {
        assert_eq!(
            readings("var.ami_id"),
            vec!["var.ami_id".to_string(), "var".to_string()]
        );
    }
}

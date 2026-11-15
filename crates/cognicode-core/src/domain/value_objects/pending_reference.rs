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
use crate::domain::value_objects::call_site::CallSite;
use serde::{Deserialize, Serialize};

/// Why the far end of a reference could not be identified.
///
/// The class answers for the scope of the build that reports it: a build
/// that merged three files speaks of those three files, and a build that
/// read the whole project speaks of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// What the scope could say about a refused call.
///
/// The two scopes that build a graph know different things, and whether a
/// refusal is a hole depends on what each of them could prove:
///
/// - The project scope holds every declaration the project has. A call
///   it refuses names a declaration the project does not have anywhere:
///   the qualifier is a type the project does not declare, or no
///   declaration carries the name. Such a call may live outside the
///   project — a function of another crate, a module of Ansible — and the
///   graph says nothing about the world outside it.
/// - The file scope holds only the declarations of the file. A type the
///   file does not declare may live in another file of the project, so
///   the file cannot prove that a call written through it is somebody
///   else's. When the file declares the name the call names, the call
///   may be the file's own: that is a hole in what the graph knows.
///
/// The scope answers what it could prove, and the domain applies the
/// rule: the answer travels with the refusal because the two scopes do
/// not know the same thing. One rule, one place, so that two routes
/// cannot disagree about which refusals are worth reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeAnswer {
    /// The scope proved the call names somebody else's declaration.
    Foreign,
    /// The scope declares the name the call names and could not tie the
    /// call to that declaration.
    PossiblyTheScope,
}

/// A reference whose far end is not identified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl PendingReference {
    /// The reference a refused call leaves behind — when the refusal is a
    /// hole in what the graph knows.
    ///
    /// One rule, stated once, and it is the rule the domain already
    /// speaks in prose: a call whose name the scope declares, several
    /// times or in a way the scope cannot tell apart, is a hole. The
    /// graph does not carry that relation, so the consumer has to hear
    /// about it instead of reading a graph that is missing something as a
    /// graph that has nothing.
    ///
    /// What the scope could prove decides the rest, and the two scopes
    /// prove different things:
    ///
    /// - A call the scope proved to be somebody else's is not a hole:
    ///   what the code calls may live outside the scope — a function of
    ///   another crate, a module of Ansible — and the graph says nothing
    ///   about the world outside it. That call leaves no reference
    ///   behind, and the number of them is not a hole either.
    /// - A call the scope could not prove to be somebody else's, and
    ///   whose name the scope declares, may be the scope's own: the type
    ///   the call is written through may live in another file of the
    ///   project, and the scope cannot tell that from a type of somebody
    ///   else. That call is a hole, and it leaves a reference behind with
    ///   the class [`PendingClass::Unresolved`].
    ///
    /// `None` is the first case, and it is a decision rather than an
    /// oversight: the rule lives in one place so that the routes cannot
    /// disagree about which refusals are worth reporting.
    pub fn of_call(
        site: &CallSite,
        class: PendingClass,
        answer: ScopeAnswer,
    ) -> Option<PendingReference> {
        let class = match class {
            // The scope holds the name and cannot say which declaration
            // the call means: a hole, whatever the scope could prove.
            PendingClass::Ambiguous { candidates } => PendingClass::Ambiguous { candidates },
            // The scope could not identify the far end. Whether that is a
            // hole is what the scope's answer says.
            PendingClass::Unresolved => match answer {
                ScopeAnswer::Foreign => return None,
                ScopeAnswer::PossiblyTheScope => PendingClass::Unresolved,
            },
        };
        Some(PendingReference {
            origin_file: site.origin_file.clone(),
            origin_line: Some(site.line),
            source: site.source.clone(),
            target: site.target.clone(),
            kind: DependencyType::Calls,
            class,
        })
    }
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

    /// A call site, as the reader saw it.
    fn site(target: &str) -> CallSite {
        CallSite {
            source: SymbolId::new("src/a.rs:caller:3"),
            origin_file: "src/a.rs".to_string(),
            line: 3,
            target: target.to_string(),
            type_qualifier: None,
            qualifier_path: None,
            path_head: None,
            receiver: None,
        }
    }

    /// The scope proved the call names somebody else's declaration: the
    /// call leaves no reference behind.
    #[test]
    fn a_call_the_scope_proved_foreign_leaves_no_reference_behind() {
        assert!(
            PendingReference::of_call(
                &site("render"),
                PendingClass::Unresolved,
                ScopeAnswer::Foreign,
            )
            .is_none(),
            "what the code calls may live outside the scope: the graph says nothing about the \
             world outside it"
        );
    }

    /// The scope declares the name and could not tie the call to that
    /// declaration: the refusal is a hole, and it leaves a reference
    /// behind that says which call it is.
    #[test]
    fn a_call_the_scope_could_not_tie_leaves_a_reference_behind() {
        let reference = PendingReference::of_call(
            &site("render"),
            PendingClass::Unresolved,
            ScopeAnswer::PossiblyTheScope,
        )
        .expect("the call may be the scope's own: the hole has to be named");

        assert_eq!(reference.class, PendingClass::Unresolved);
        assert_eq!(reference.target, "render");
        assert_eq!(reference.origin_file, "src/a.rs");
        assert_eq!(reference.origin_line, Some(3));
        assert_eq!(reference.kind, DependencyType::Calls);
    }

    /// A name the scope declares several times is a hole whatever the
    /// scope could prove: the scope holds the name and cannot say which
    /// declaration the call means.
    #[test]
    fn an_ambiguous_call_leaves_a_reference_behind() {
        let reference = PendingReference::of_call(
            &site("render"),
            PendingClass::Ambiguous { candidates: 2 },
            ScopeAnswer::Foreign,
        )
        .expect("the scope holds the name and cannot tell the declarations apart");

        assert_eq!(reference.class, PendingClass::Ambiguous { candidates: 2 });
    }

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

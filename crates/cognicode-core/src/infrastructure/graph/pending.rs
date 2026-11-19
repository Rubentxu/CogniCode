//! Tying a pending reference to a declaration, when the project can prove it.
//!
//! A file that reads a name the reader could not tie to a declaration
//! leaves a fact behind, not a gap in the answer: the reference says what
//! the code states, and the project —which knows every declaration it
//! holds— may still be able to say which one it means.
//!
//! This module is where that second look happens. It resolves a reference
//! against the project-wide index and it never invents a relation: a
//! reference that names one declaration becomes an edge, and a reference
//! that names none, or more than one the project cannot tell apart, stays
//! pending and says why.

use std::path::Path;

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::value_objects::Language;
use crate::domain::value_objects::pending_reference::readings;
use crate::domain::value_objects::{
    DependencyType, PendingClass, PendingReference, TargetResolution,
};
use crate::infrastructure::graph::per_file_graph::GlobalSymbolIndex;

/// What a project-wide lookup made of the references that could not be tied
/// to a declaration.
#[derive(Debug, Clone, Default)]
pub struct PendingResolution {
    /// The relations that could be proven: the declaration that holds the
    /// reference, the declaration it points at, and what the relation is.
    pub proven: Vec<(SymbolId, SymbolId, DependencyType)>,
    /// The references that stayed pending, each with the class that says
    /// why.
    pub pending: Vec<PendingReference>,
}

/// Ties the references that could not be tied to a declaration against the
/// project-wide index.
///
/// A reference is a name the reader heard, and a name that names a path can
/// be read in more than one way. The longest reading that names a
/// declaration wins: when it names exactly one, the relation is proven and
/// becomes an edge; when it names several, the reference stays pending and
/// says how many candidates there were; when it names none, it stays
/// pending as unresolved.
///
/// Only the declarations the writing module can see are candidates. A
/// declaration of a sibling module does not satisfy a reference: the edge
/// would name something the code does not name, and the hole it hides
/// would be a real one.
///
/// Two kinds of reference are not this module's rule. A parent is found by
/// the project-wide rule —the one the per-file route uses— because the
/// declaration that names a parent may live in another module of the
/// project. And a call is not touched at all: the route that
/// read the call already decided which scope applies to it —the
/// project-wide one— and it already said why the call could not be tied
/// to a declaration. This pass would either prove it against a declaration
/// of the caller's own module, inventing the very edge the refusal exists
/// to prevent, or reclassify its motive with a scope the route did not
/// use. A refused call travels through untouched.
pub fn resolve_pending_references(
    pending: Vec<PendingReference>,
    index: &GlobalSymbolIndex,
) -> PendingResolution {
    let mut outcome = PendingResolution::default();
    for reference in pending {
        // A refused call keeps its motive and its candidates: they are the
        // answer of the route that read it, and this pass does not get to
        // change it.
        if reference.kind == DependencyType::Calls {
            outcome.pending.push(reference);
            continue;
        }
        let origin = Path::new(&reference.origin_file);
        let module = origin.parent().unwrap_or(origin);
        // The reference is written in the writer's language, and that
        // language says whether the spelling folds.
        let folds_case = Language::folds_case_in(Some(origin));
        let mut resolution = None;
        if reference.kind == DependencyType::Inherits {
            // A parent is not a reference. The declaration it names lives
            // where the code that declares the parent lives, and the code
            // that implements it may live in another module: the trait of a
            // class is declared in the file that declares it, and the class
            // that implements it in another one. The rule that finds a
            // parent is therefore the project-wide one —the same rule the
            // per-file route uses— and a parent resolved by the rule of
            // the reference would be a parent of the module only: the two
            // routes would answer differently about the same file.
            resolution = Some(index.resolve(&reference.target, Some(origin)));
        } else {
            for reading in readings(&reference.target) {
                // The declarations of the module that writes the reference are
                // the only candidates: infrastructure code is scoped by
                // directory, and a module does not see the declarations of
                // another one —ni de sus hermanos ni de sus padres—. A
                // declaration of a module that the writer cannot see does not
                // satisfy the reference: it would be an edge that nothing
                // proves, and it would hide the hole.
                let in_module = index.candidates_in_module(&reading, module, folds_case);
                if in_module.len() == 1 {
                    resolution = Some(TargetResolution::Resolved(in_module[0].clone()));
                    break;
                }
                if in_module.len() > 1 {
                    resolution = Some(TargetResolution::Ambiguous {
                        name: reading,
                        candidates: in_module,
                    });
                    break;
                }
                // Nothing the writer's module declares carries that reading: a
                // shorter one may still name a declaration.
            }
        }
        match resolution {
            Some(TargetResolution::Resolved(target)) => {
                outcome
                    .proven
                    .push((reference.source.clone(), target, reference.kind));
            }
            Some(TargetResolution::Ambiguous { candidates, .. }) => {
                outcome.pending.push(PendingReference {
                    class: PendingClass::Ambiguous {
                        candidates: candidates.len(),
                    },
                    ..reference
                });
            }
            _ => outcome.pending.push(reference),
        }
    }
    outcome
}

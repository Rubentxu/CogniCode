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
use crate::domain::value_objects::pending_reference::readings;
use crate::domain::value_objects::{DependencyType, PendingClass, PendingReference};
use crate::infrastructure::graph::per_file_graph::{GlobalSymbolIndex, TargetResolution};

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
pub fn resolve_pending_references(
    pending: Vec<PendingReference>,
    index: &GlobalSymbolIndex,
) -> PendingResolution {
    let mut outcome = PendingResolution::default();
    for reference in pending {
        let origin = Path::new(&reference.origin_file);
        let module = origin.parent().unwrap_or(origin);
        let mut resolution = None;
        for reading in readings(&reference.target) {
            let key = reading.to_lowercase();
            // The declarations of the module that writes the reference come
            // first: infrastructure code is scoped by directory, and a name
            // that several modules declare is told apart by the module that
            // writes it.
            let in_module = index.candidates_in_directory(&key, module);
            if in_module.len() == 1 {
                resolution = Some(TargetResolution::Resolved(in_module[0].clone()));
                break;
            }
            if in_module.len() > 1 {
                resolution = Some(TargetResolution::Ambiguous {
                    name: key,
                    candidates: in_module,
                });
                break;
            }
            match index.resolve(&key, Some(origin)) {
                // Nothing carries that reading: a shorter one may still name
                // a declaration.
                TargetResolution::Unresolved(_) => continue,
                other => {
                    resolution = Some(other);
                    break;
                }
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

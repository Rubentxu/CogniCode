//! What the graph did with the relations a build offered it.
//!
//! A build offers relations to a repository, and the repository says
//! which ones it took. The build has to say the same: a relation that
//! was refused is not a relation of the graph, and a number that counts
//! the *offers* is not the number the graph *holds*.
//!
//! One rule, one owner: every path that attaches relations attaches
//! them here, so the build paths and the merge path cannot drift into
//! different answers about what was refused.

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::value_objects::{DependencyType, RejectedRelation};

/// Offers every relation and returns the ones the graph refused to take.
///
/// `attach` is the one that decides: the repository on the build paths,
/// the merged graph on the merge path. Every refusal is kept, so a
/// relation that could not be attached leaves a fact behind instead of
/// a silence.
///
/// How many relations were taken is not returned on purpose: that
/// number counts the *offers*, and the number the graph holds is the
/// one that has to be published. The offers are counted by whoever
/// received them, not by the build that made them.
pub fn attach_relations<E: std::fmt::Display>(
    mut attach: impl FnMut(&SymbolId, &SymbolId, DependencyType) -> Result<(), E>,
    offers: impl IntoIterator<Item = (SymbolId, SymbolId, DependencyType)>,
) -> Vec<RejectedRelation> {
    let mut rejected = Vec::new();
    for (source, target, kind) in offers {
        if let Err(reason) = attach(&source, &target, kind) {
            rejected.push(RejectedRelation {
                source,
                target,
                kind,
                reason: reason.to_string(),
            });
        }
    }
    rejected
}

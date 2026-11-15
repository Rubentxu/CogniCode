//! Who owns a call, and the scope that answers it.
//!
//! The rule that judges a call is written once, and the only thing that
//! differs between the routes is the scope it is judged against: the
//! project when there is an index, the file when there is not.

use super::package;
use super::per_file_graph::GlobalSymbolIndex;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::{CallSite, Receiver, TargetResolution};
use crate::infrastructure::parser::Language;
use crate::infrastructure::parser::naming;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// What a scope knows about the names it declares.
///
/// The scope is the project when there is an index and the file when there
/// is not: that is the whole difference between the routes, and the rule
/// that judges a call is written once.
pub(crate) trait Declares {
    /// Whether the scope declares a name, spelled exactly this way.
    ///
    /// The spelling is the scope's to judge, and `folds_case` says whether
    /// the language that wrote the name folds it.
    ///
    /// The target is the name the call names, when the call names one: a
    /// scope can own a name without declaring it, and what it owns then is
    /// the call written through it. The project index owns nothing that way
    /// — it knows the declarations and nothing else — so it reads the target
    /// as nothing.
    ///
    /// The qualifier path is the path the call's qualifier is written with,
    /// and it is what tells two types of the same name apart. The project
    /// index does not judge it: it knows the declarations of the project and
    /// not the path a call is written through, and a scope that judges one
    /// name without the path of the other answers about a name and not
    /// about the type.
    fn declares(
        &self,
        name: &str,
        target: Option<&str>,
        qualifier_path: Option<&str>,
        folds_case: bool,
    ) -> bool;
}

impl Declares for GlobalSymbolIndex {
    fn declares(
        &self,
        name: &str,
        _target: Option<&str>,
        _qualifier_path: Option<&str>,
        folds_case: bool,
    ) -> bool {
        GlobalSymbolIndex::declares(self, name, folds_case)
    }
}

/// The declarations of one file, as a scope.
struct FileScope<'a> {
    /// The symbols the file declares, by lowercase name.
    local: &'a HashMap<String, Vec<SymbolId>>,
    /// The graph the file's symbols live in.
    graph: &'a CallGraph,
    /// The names the file's impls declare, and the owner of each one.
    ///
    /// A file that implements `Widget` owns the calls written `Widget::new`,
    /// even when it does not declare `Widget`: the type lives in another
    /// file and the impl is here.
    ///
    /// The owner keeps the path it is written with, so the ownership is
    /// the ownership of a TYPE and not of a name: the file that implements
    /// `a::Widget` owns the calls written `a::Widget::new` and not the
    /// ones written `b::Widget::new`, where `b::Widget` is another type
    /// that happens to have the same name.
    impl_owners: &'a HashMap<String, HashSet<String>>,
}

impl Declares for FileScope<'_> {
    fn declares(
        &self,
        name: &str,
        target: Option<&str>,
        qualifier_path: Option<&str>,
        folds_case: bool,
    ) -> bool {
        let declared = self
            .local
            .get(&name.to_lowercase())
            .is_some_and(|candidates| {
                candidates.iter().any(|id| {
                    self.graph.get_symbol(id).is_some_and(|symbol| {
                        Language::answers_to_in(symbol.name(), name, folds_case)
                    })
                })
            });
        if declared {
            return true;
        }
        // The file does not declare the name, but it may own it: the name
        // the call names is an item of an impl of that name, so the call is
        // written through something the file implements.
        //
        // The owner answers when its path and the path of the call's
        // qualifier are the same type: the names are equal and the modules
        // are compatible.
        target.is_some_and(|target| {
            self.impl_owners.get(target).is_some_and(|owners| {
                owners
                    .iter()
                    .any(|owner| naming::owner_answers_to(owner, qualifier_path, folds_case))
            })
        })
    }
}

/// The rule that judges a call, written once.
///
/// Two facts say it, and both are asked here so that no route decides them
/// on its own:
///
/// - A call written `Duration::from_millis(10)` names a method of
///   `Duration`. When the scope declares no `Duration`, that method is not
///   the scope's, and joining it to the scope's own `from_millis` would
///   publish the standard library's call count as the project's.
/// - A call written `serde_json::to_value(1)` names a function of the crate
///   the package pulls. When the package declares that crate from a
///   registry, the call is not the project's, and joining it to the
///   project's own `to_value` would publish a dependency's call count as
///   the project's.
///
/// The scope is the only thing that differs between the routes: the project
/// when there is an index, the file when there is not.
///
/// The facts of a call are distinct inputs, and the rule asks for each of
/// them by name: the name, the qualifier, the path of the qualifier, the
/// head of the path and the receiver.
#[allow(clippy::too_many_arguments)] // the facts of a call, one by one
pub(crate) fn call_is_owned<S: Declares + ?Sized>(
    scope: &S,
    manifests: &mut package::Manifests,
    caller_file: Option<&Path>,
    type_qualifier: Option<&str>,
    qualifier_path: Option<&str>,
    target: Option<&str>,
    path_head: Option<&str>,
    receiver: Option<&Receiver>,
) -> bool {
    // The name the call writes is read in the language the call is written
    // in, and that language says whether the spelling folds.
    let folds_case = Language::folds_case_in(caller_file);
    if type_qualifier
        .is_some_and(|qualifier| !scope.declares(qualifier, target, qualifier_path, folds_case))
    {
        return false;
    }
    if path_head.is_some_and(|head| manifests.is_external_crate(head, caller_file)) {
        return false;
    }
    // A call written on a value names a method of the receiver's type, and
    // the file said whose the receiver is. A receiver that belongs to
    // somebody else is not a call to the project's declaration of the same
    // name: joining them would publish the standard library's call count as
    // the project's.
    //
    // The receiver is judged by what the file said, not by what the scope
    // declares, so that the two routes answer the same question about the
    // same call: the file is the one that knows.
    match receiver {
        Some(Receiver::Crate(crate_name)) => {
            if manifests.is_external_crate(crate_name, caller_file) {
                return false;
            }
        }
        Some(Receiver::Unbound) => return false,
        Some(Receiver::Project) | Some(Receiver::Uncertain) | None => {}
    }
    true
}

/// A call site judged against the file as the whole scope.
///
/// The type qualifier is checked against the declarations of the file the
/// way [`GlobalSymbolIndex::resolve_call`] checks it against the project's:
/// by exact spelling, and only when the call states one.
///
/// The path of the qualifier travels with it, because it is what tells two
/// types of the same name apart: a file that implements `a::Widget` owns
/// the calls written `a::Widget::new` and not the ones written
/// `b::Widget::new`. The route with the index does not need it: it judges
/// against the declarations of the project, where the two types are two
/// declarations of the same name.
///
/// The crate the path starts at is checked the same way the index checks
/// it: against the crates the file's own package pulls from outside the
/// project. A call through one of them is not the project's, and the file
/// that writes it is the only one that says which package it belongs to.
///
/// The rule itself is [`call_is_owned`]: this route and the project-wide one
/// ask the same question, and the scope is the only thing that differs.
///
/// The call is judged as the reader stated it: the [`CallSite`] already
/// says the name, the qualifier, the head of the path and the receiver, so
/// the route does not take them apart and hand them over one by one.
pub(crate) fn resolve_call_within_file(
    site: &CallSite,
    manifests: &mut package::Manifests,
    local: &HashMap<String, Vec<SymbolId>>,
    graph: &CallGraph,
    impl_owners: &HashMap<String, HashSet<String>>,
) -> TargetResolution {
    let scope = FileScope {
        local,
        graph,
        impl_owners,
    };
    if !call_is_owned(
        &scope,
        manifests,
        Some(Path::new(&site.origin_file)),
        site.type_qualifier.as_deref(),
        site.qualifier_path.as_deref(),
        Some(site.target.as_str()),
        site.path_head.as_deref(),
        site.receiver.as_ref(),
    ) {
        return TargetResolution::Unresolved(site.target.clone());
    }
    resolve_within_file(
        &site.target,
        local,
        graph,
        Language::folds_case_in(Some(Path::new(&site.origin_file))),
    )
}

/// The answer the file itself gives about a name.
///
/// Without a project-wide index the file IS the scope, and this is the
/// whole answer: a name the file declares once names that declaration, a
/// name it declares several times is ambiguous, and a name it does not
/// declare names nothing here. The motive is the same one the
/// project-wide resolver gives, because it comes from the same place:
/// `TargetResolution::outcome`.
///
/// The name is the one the call was written with, and the language of the
/// file says whether that spelling folds.
pub(crate) fn resolve_within_file(
    name: &str,
    local: &HashMap<String, Vec<SymbolId>>,
    graph: &CallGraph,
    folds_case: bool,
) -> TargetResolution {
    let candidates: Vec<SymbolId> = match local.get(&name.to_lowercase()) {
        None => Vec::new(),
        Some(bucket) => bucket
            .iter()
            .filter(|id| {
                graph
                    .get_symbol(id)
                    .is_some_and(|symbol| Language::answers_to_in(symbol.name(), name, folds_case))
            })
            .cloned()
            .collect(),
    };
    match candidates.as_slice() {
        [] => TargetResolution::Unresolved(name.to_string()),
        [only] => TargetResolution::Resolved(only.clone()),
        many => TargetResolution::Ambiguous {
            name: name.to_string(),
            candidates: many.to_vec(),
        },
    }
}

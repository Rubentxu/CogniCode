//! Keeping what the last build already proved.
//!
//! A graph build resolves every relationship the readers found: it looks
//! each called name up in the project-wide index and wires it to the
//! declaration it names. That stage is not cheap — on a 400-file project it
//! is about 2.4 seconds, three quarters of the whole build — and it is the
//! SAME work in a cold build, in a warm one and after a single file
//! changed. A consumer that touched one file pays for the whole project.
//!
//! This module owns the answer to one narrow question: **given what
//! changed, which resolutions still hold?**
//!
//! The rule is the rule of IDENTITY. A resolution names a declaration, and
//! a declaration is identified by its path, its name and the line it sits
//! on. Two builds of the same file give that declaration the same
//! identity — the identity depends on what the file says, not on how the
//! file happened to be read — so a resolution still holds when the
//! declaration it names is still the same declaration. A name is touched
//! when some file — changed, new or gone — added the declaration of that
//! name, dropped it, or gave it a different identity.
//!
//! Each half of the rule is necessary:
//!
//! - the caller's own file: the calls it states are read again every
//!   build, and the ones it no longer states simply stop being
//!   relationships;
//! - the name: a file that starts declaring `foo` can turn an ambiguous
//!   call into a resolved one, and a file that stops declaring it does the
//!   opposite. A rule that only looked at the caller's file would keep
//!   answers that the change invalidated.
//!
//! What the change does not touch is kept. Changing the body of a
//! function does not change the identity of that function, so the calls
//! that name it keep their answer; moving it to another line DOES change
//! its identity, so those calls are looked up again — and they have to be,
//! because the answer they kept names an identity that no longer exists.
//!
//! A file that entered or left the build is handled by the same rule: the
//! declarations it brought or took away are touched, and nothing else is.
//! The scope of the build needs no guard of its own, because what a
//! resolution depends on is the set of declarations of the name it looks
//! for and the path of its caller — and the rule above already watches
//! both.
//!
//! A call that names the type it belongs to (`Duration::from_millis`) is
//! judged against the declarations of that type too: the same name called
//! through two types has two answers, so the type is part of what the
//! ledger remembers, and a change that declares or drops the type touches
//! the answers of every call written through it.
//!
//! A call written through a crate (`serde_json::to_value`) is judged against
//! what the package of the caller declares, and so is a call written on a
//! value whose type belongs to a crate. The ledger does not read the
//! manifest to judge the call — the resolver does, and it is the only place
//! that does — but it has to know that the answer depended on it: a
//! dependency added or removed between two builds changes the answer of
//! every call written through that crate. So the ledger tracks the manifest
//! that governs each file, hashed the same way the bytes of a source file
//! are, and a manifest that changed invalidates the answers of the files it
//! governs.
//!
//! The ledger remembers two things between builds: what the last build
//! knew about each file (its content hash and the identity of every
//! declaration it made), and what it resolved for each (caller file, called
//! name, judged type) triple. Both are replaced wholesale by the current
//! build when it ends, so the ledger always describes exactly one build and
//! never a mixture of two.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{
    CallSite, DependencyType, PendingClass, PendingReference, Receiver,
};
use crate::infrastructure::graph::package::{self, GoverningManifests};
use crate::infrastructure::graph::per_file_graph::GlobalSymbolIndex;

/// What one build knows about one file.
#[derive(Debug, Clone)]
pub struct DeclaredFile {
    /// The path of the file, as the build spells it.
    pub path: String,
    /// The hash of the bytes the build read.
    pub hash: [u8; 32],
    /// What the file declares, by lowercased name: the identity of every
    /// declaration that name has in this file.
    ///
    /// Lowercased because that is how the index keys them: a call is
    /// resolved by the name it looks for, and that name is compared
    /// against the name the file declares. A name can have more than one
    /// declaration in a file — two locals called `x`, a function and a
    /// type — so the identity of each one is kept, not just the name.
    pub declarations: BTreeMap<String, BTreeSet<SymbolId>>,
}

impl DeclaredFile {
    /// What one build knows about one file: its hash, and the identity of
    /// every declaration it makes.
    ///
    /// The identity of a declaration is the one its reader gave it — the
    /// path, the name and the line — because that is the identity the
    /// index knows it by, and a rule that compared another identity would
    /// be comparing something the resolver never looked at.
    pub fn of(path: String, hash: [u8; 32], symbols: &[Symbol]) -> Self {
        let mut declarations: BTreeMap<String, BTreeSet<SymbolId>> = BTreeMap::new();
        for symbol in symbols {
            declarations
                .entry(symbol.name().to_lowercase())
                .or_default()
                .insert(SymbolId::new(symbol.fully_qualified_name()));
        }
        Self {
            path,
            hash,
            declarations,
        }
    }
}

/// The answer the last build gave for one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The last build looked this up and it named no declaration — and it
    /// says WHICH refusal it was.
    ///
    /// The reason travels with the answer because the answer is kept: a
    /// build that reuses what the last one proved has to publish the same
    /// reason, or the two builds would answer differently about the same
    /// call.
    Refused(PendingClass),
    /// The last build found this target.
    Found(SymbolId),
}

/// What changed between the build the ledger describes and this one.
#[derive(Debug, Default)]
pub struct Changes {
    /// The names whose declarations are not the same as they were: added,
    /// dropped, or given another identity.
    names: BTreeSet<String>,
    /// The files whose bytes changed, appeared or disappeared.
    changed_files: HashSet<String>,
    /// The files whose governing manifest changed: the manifest appeared,
    /// disappeared, or says something else now.
    changed_governing: HashSet<String>,
}

impl Changes {
    /// Whether the change can have altered the resolution of this name.
    fn touches(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Whether the change altered the content of this file.
    fn changed(&self, path: &str) -> bool {
        self.changed_files.contains(path)
    }

    /// Whether the manifest that governs this file changed.
    ///
    /// The answer to a call can depend on what the package of the caller
    /// declares: a call written through a crate, and a call written on a
    /// value whose type belongs to one. So a manifest that changed can have
    /// altered the answer of every call of the files it governs.
    fn governing_changed(&self, path: &str) -> bool {
        self.changed_governing.contains(path)
    }
}

/// What a call asks: the file it is written in, the lowercased name it
/// calls, the type that qualifies it when the call states one the
/// resolver judges, the crate or module its path starts at, and what the
/// file says about the receiver of a call written on a value.
type Question = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<Receiver>,
);

/// What the last build knew about each file, and what it resolved.
#[derive(Debug, Default)]
pub struct ReuseLedger {
    /// The files the last build covered, by path.
    files: HashMap<String, Remembered>,
    /// The answers of the last build, by what each call asked.
    outcomes: HashMap<Question, Answer>,
    /// The manifests that governed each file in the last build.
    ///
    /// The answer to a call written through a crate depends on what the
    /// package of the caller declares, and the manifest is the only place
    /// that says it. The ledger hashes the manifest the same way it hashes
    /// the bytes of a source file, so it can tell whether the manifest it
    /// reads now is the one it read then.
    governing: HashMap<String, Option<GoverningManifests>>,
}

/// What the ledger remembers about one file.
#[derive(Debug, Clone)]
struct Remembered {
    /// The hash of the bytes the build read.
    hash: [u8; 32],
    /// What the file declared, by lowercased name.
    declarations: BTreeMap<String, BTreeSet<SymbolId>>,
}

impl ReuseLedger {
    /// A ledger that knows nothing: the first build keeps nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// What changed since the build this ledger describes.
    ///
    /// The manifests that govern the files are resolved here — one walk per
    /// directory, one reading per package — and hashed: the ledger has to
    /// know whether the manifest it reads now is the one it read then,
    /// because the answer to a call written through a crate depends on what
    /// the manifest says.
    pub fn changes(&mut self, files: &[DeclaredFile]) -> Changes {
        let current: HashSet<&str> = files.iter().map(|file| file.path.as_str()).collect();

        let mut changes = Changes::default();

        // The manifest that governs each file: one walk per directory, and
        // one reading per package. A project has one package per directory
        // and one manifest per package, so reading the manifest of every
        // directory reads the manifest of every package twice.
        let mut directories: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
        let mut packages: HashMap<PathBuf, Option<GoverningManifests>> = HashMap::new();
        let governing: Vec<(String, Option<GoverningManifests>)> = files
            .iter()
            .map(|file| {
                let path = Path::new(&file.path);
                let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();
                let found = if !package::is_governed_by_a_manifest(path) {
                    None
                } else {
                    let manifest = directories
                        .entry(directory.clone())
                        .or_insert_with(|| package::manifest_of(&directory))
                        .clone();
                    match manifest {
                        Some(manifest) => packages
                            .entry(manifest.clone())
                            .or_insert_with(|| package::governing_manifests_of(&manifest))
                            .clone(),
                        None => None,
                    }
                };
                (file.path.clone(), found)
            })
            .collect();

        for file in files {
            match self.files.get(&file.path) {
                // Unchanged: every declaration it makes is the same one.
                Some(remembered) if remembered.hash == file.hash => {}
                // Changed: a declaration that is not the same one touches
                // the name it answers to.
                Some(remembered) => {
                    for (name, ids) in &file.declarations {
                        match remembered.declarations.get(name) {
                            // Same declarations, same identity: the name
                            // answers exactly as it did.
                            Some(before) if before == ids => {}
                            // A declaration appeared, or the ones that
                            // were there now live elsewhere.
                            _ => {
                                changes.names.insert(name.clone());
                            }
                        }
                    }
                    // A declaration that is gone touches its name too: a
                    // call that named it may no longer be provable.
                    for name in remembered.declarations.keys() {
                        if !file.declarations.contains_key(name) {
                            changes.names.insert(name.clone());
                        }
                    }
                    changes.changed_files.insert(file.path.clone());
                }
                // New: what it declares is new to the project.
                None => {
                    changes.names.extend(file.declarations.keys().cloned());
                    changes.changed_files.insert(file.path.clone());
                }
            }
        }

        // Gone: what it declared is gone with it.
        for (path, remembered) in &self.files {
            if !current.contains(path.as_str()) {
                changes
                    .names
                    .extend(remembered.declarations.keys().cloned());
                changes.changed_files.insert(path.clone());
            }
        }

        // The manifest that governs a file changed: the answers of that
        // file can depend on what its package declares. Only the files of
        // that package are affected: the manifest of one package says
        // nothing about the calls of another.
        for (path, manifest) in &governing {
            if self.governing.get(path) != Some(manifest) {
                changes.changed_governing.insert(path.clone());
            }
        }

        // The manifests this build read are the ones the next build
        // compares against.
        for (path, manifest) in governing {
            self.governing.insert(path, manifest);
        }

        changes
    }

    /// The answer the last build gave, when it can be kept.
    ///
    /// `None` means the answer has to be computed again.
    fn kept(&self, question: &Question, changes: &Changes) -> Option<Answer> {
        let (caller_file, name, type_qualifier, _path_head, _receiver) = question;
        let qualifier_touched = type_qualifier
            .as_deref()
            .is_some_and(|qualifier| changes.touches(&qualifier.to_lowercase()));
        if changes.touches(name) || qualifier_touched || changes.changed(caller_file) {
            return None;
        }
        // The answer to a call can depend on what the package of the caller
        // declares: a call written through a crate path, and a call written
        // on a value whose type belongs to a crate. The ledger does not read
        // the manifest to judge the call, so it does not know which answers
        // depend on it; a manifest that changed therefore invalidates the
        // answers of the files it governs — and only of those.
        if changes.governing_changed(caller_file) {
            return None;
        }
        self.outcomes.get(question).cloned()
    }

    /// Remembers what this build knew, replacing what it knew before.
    ///
    /// The declarations and the answers are taken, not copied: they are
    /// the ledger's from now on, and copying eighteen thousand
    /// identities and thirty-five thousand answers on every build would
    /// cost more than the work the ledger saves.
    pub fn remember(&mut self, files: Vec<DeclaredFile>, resolution: &mut Resolution) {
        self.files = files
            .into_iter()
            .map(|file| {
                (
                    file.path.clone(),
                    Remembered {
                        hash: file.hash,
                        declarations: file.declarations,
                    },
                )
            })
            .collect();
        self.outcomes = std::mem::take(&mut resolution.outcomes);
    }
}

/// The relationships a build resolved, and how much of the work it kept.
#[derive(Debug, Default)]
pub struct Resolution {
    /// The edges the relationships prove.
    pub edges: Vec<(SymbolId, SymbolId, DependencyType)>,
    /// The relationships that named no declaration.
    pub unresolved: usize,
    /// The refusals that are holes in what the graph knows: the calls
    /// whose name the project declares and cannot tie to one of its
    /// declarations.
    ///
    /// They travel with the edges because a refusal that is only counted
    /// is a fact the consumer cannot use: it does not say which call, in
    /// which file and line, nor how many candidates it could have meant.
    pub pending: Vec<PendingReference>,
    /// The relationships whose answer this build kept.
    pub reused: usize,
    /// The relationships whose answer this build computed again.
    pub resolved: usize,
    /// The answers, for the ledger to remember.
    outcomes: HashMap<Question, Answer>,
}

/// Resolves the calls of a build, keeping what the last build proved.
///
/// The calls are the ones the readers found, each with the place it is
/// written and the declaration that holds it: the caller file is what
/// lets the resolver apply the scope-aware rules, and the line is what
/// lets a refusal be pointed at.
pub fn resolve(
    calls: &[CallSite],
    index: &GlobalSymbolIndex,
    ledger: &ReuseLedger,
    changes: &Changes,
) -> Resolution {
    let mut resolution = Resolution::default();

    for site in calls {
        let question: Question = (
            site.origin_file.clone(),
            site.target.to_lowercase(),
            site.type_qualifier.clone(),
            site.path_head.clone(),
            site.receiver.clone(),
        );
        let answer = match ledger.kept(&question, changes) {
            Some(answer) => {
                resolution.reused += 1;
                answer
            }
            None => {
                resolution.resolved += 1;
                look_up(index, &question)
            }
        };
        // The answer goes into the new ledger either way: a kept answer is
        // still an answer, and a ledger that only remembered the freshly
        // computed ones would empty itself on every warm build — the second
        // build after a change would find nothing to keep and would redo
        // the whole project.
        resolution.outcomes.insert(question, answer.clone());

        match answer {
            Answer::Found(target) => {
                resolution
                    .edges
                    .push((site.source.clone(), target, DependencyType::Calls));
            }
            Answer::Refused(class) => {
                resolution.unresolved += 1;
                // The refusal leaves a reference behind when it is a hole
                // in what the graph knows. Whether it is one is the
                // domain's rule, and it lives in one place: this module
                // does not decide it, it only carries what the decision
                // says.
                if let Some(reference) = PendingReference::of_call(site, class) {
                    resolution.pending.push(reference);
                }
            }
        }
    }

    resolution
}

/// Looks one name up in the project-wide index.
///
/// The two refusals mean different things to the consumer — a name the
/// project declares several times, and a name it does not declare at all
/// — and the answer carries which one it was. The reason comes from the
/// resolution itself, so no route decides it on its own.
fn look_up(index: &GlobalSymbolIndex, question: &Question) -> Answer {
    let (caller_file, name_lower, type_qualifier, path_head, receiver) = question;
    let caller_path = std::path::Path::new(caller_file);
    match index
        .resolve_call(
            name_lower,
            type_qualifier.as_deref(),
            path_head.as_deref(),
            Some(caller_path),
            receiver.as_ref(),
        )
        .outcome()
    {
        Ok(target) => Answer::Found(target),
        Err(class) => Answer::Refused(class),
    }
}

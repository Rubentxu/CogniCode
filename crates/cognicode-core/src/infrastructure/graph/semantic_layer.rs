//! The semantic layer: what a language's own reader makes of a file.
//!
//! Some languages have a reader that understands their domain: HCL has
//! Terraform blocks, YAML has Ansible plays. That reader is the authority
//! for those languages, and it is the only one that can name a declaration
//! the way the tools that ask about those declarations do.
//!
//! Two paths build a graph — the per-file cache and the product's own
//! builder — and they used to disagree about which reader they consulted.
//! One of them asked the tree-sitter parser directly and answered with
//! generic names, so the same repository produced two different graphs and
//! the product asked about names the graph had never heard of. This module
//! is the single place that asks the domain reader, so both paths answer
//! the same thing.
//!
//! And the answer says what happened. A reader that ran, a reader that does
//! not apply to this file, and a reader that fell over are three different
//! facts, and a consumer that cannot tell them apart reads a failure as a
//! file that declares nothing. The outcome is what tells them apart.

use std::collections::HashMap;
use std::path::Path;

use crate::application::ingest::extractor::{HandlerOutcome, extract_file_with_outcome};
use crate::application::ingest::types::{FileFacts, FileRelation, SemanticOutcome, TargetRef};
use crate::domain::aggregates::GraphNode;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{
    Declaration, DependencyType, Location, NodeKind, PendingClass, PendingReference, SymbolKind,
};
use crate::infrastructure::graph::per_file_graph::content_hash_64;
use crate::infrastructure::parser::Language;
use crate::infrastructure::parser::language_config::LanguageConfig;

/// What the domain reader makes of one file.
///
/// The reader of the language is the authority for that language, and this
/// is where it is asked. The answer carries what happened: the reader
/// declared something, the reader does not apply to this file, or the reader
/// fell over and the cause says why.
///
/// The last case is the one that used to be lost. A reader that fell over
/// left a file without declarations behind, which is what a file that
/// declares nothing also leaves behind, and the consumer fell back to the
/// generic reader as if the file had simply said nothing. Now the failure
/// is told apart from the absence, and the consumer can say it.
pub fn semantic_facts(language: Language, source: &str, file_path: &str) -> SemanticOutcome {
    semantic_facts_of(LanguageConfig::for_language(language), source, file_path)
}

/// What the given reader makes of one file.
///
/// The reader is a parameter because a reader is what decides what a file
/// means, and a caller that holds one —the registry, or a test that needs a
/// reader that fails— must be able to hand it over.
pub fn semantic_facts_of(
    config: &LanguageConfig,
    source: &str,
    file_path: &str,
) -> SemanticOutcome {
    // A language without a reader of its own is answered by the generic
    // reader, and asking the tree before answering costs a parse that
    // nobody reads.
    if config.semantic_handler.is_none() {
        return SemanticOutcome::NotApplicable;
    }
    let (extracted, outcome) = extract_file_with_outcome(
        config,
        Path::new(file_path),
        source,
        &format!("{:016x}", content_hash_64(source.as_bytes())),
    );
    match outcome {
        // The language has no reader of its own: the generic reader is the
        // one that answers, and this file is not a fact about it.
        HandlerOutcome::Absent => return SemanticOutcome::NotApplicable,
        // The reader fell over. The file is not read, and the reason why is
        // the reader's own.
        HandlerOutcome::Failed(cause) => return SemanticOutcome::Failed(cause),
        HandlerOutcome::Ran => {}
    }
    // The reading itself did not come through: the tree was never built, or
    // the reader said the file is broken. Either way the file was not read,
    // and that is not the same as a file with nothing to declare.
    if let Some(cause) = extracted.error.as_deref() {
        return SemanticOutcome::Failed(cause.to_string());
    }

    // The reader's own identifier for a declaration and the identifier the
    // graph gives a symbol are different grammars, so the edges are
    // resolved through this map and not by comparing strings.
    let mut by_reader_id: HashMap<String, SymbolId> = HashMap::new();
    let mut symbols: Vec<Symbol> = Vec::new();

    for node in &extracted.nodes {
        // The graph carries what the file declares. A symbol of
        // programming is carried as a symbol; a declaration is carried as
        // a declaration, because that is what it is and the tools that
        // ask about infrastructure ask the graph, not the file. A node
        // kind that is neither has nowhere to go, and no place is
        // invented for it.
        let kind = match &node.kind {
            NodeKind::Symbol(kind) => *kind,
            NodeKind::Declaration => SymbolKind::Declaration,
            _ => continue,
        };
        // The node of the file itself is not a declaration. The readers
        // identify it by the source path, and the generic path does not
        // emit it as a symbol either: letting it in would give a
        // repository of infrastructure code one symbol more than any
        // other repository, and would let a file share a name with a
        // symbol in the index.
        if node.id.as_str() == file_path {
            continue;
        }
        let reader_id = node.id.as_str().to_string();
        // The generic reader emits the same declaration twice, once as
        // the block and once as the key that holds it. The reader's
        // identifier is the same in both, so the second one is dropped.
        if by_reader_id.contains_key(&reader_id) {
            continue;
        }
        // The declaration knows where it is: the reader put it there, and
        // the graph is where the tools that ask about it look.
        let mut symbol = Symbol::new(&node.label, kind, location_of(node, file_path));
        // The identity of a declaration is the one its reader gave it.
        // The graph identifies a symbol by its qualified name, and the one
        // that builds it is the generic one — file and symbol. Letting the
        // declaration pass for a generic one made the product ask for the
        // name its reader had given, and the graph answer that it had
        // never heard of it.
        symbol.set_fqn_override(&reader_id);
        // What the reader declared travels with the symbol. The reader is
        // the one that knows whether this block is a resource, a variable
        // or a play, and the graph is the only place from where the tools
        // that ask about the declaration can learn it.
        if let Some(declaration) = declaration_of(node) {
            symbol = symbol.with_declaration(declaration);
        }
        by_reader_id.insert(reader_id.clone(), SymbolId::new(&reader_id));
        symbols.push(symbol);
    }

    // A reader that exists but interpreted nothing says nothing. The
    // generic reader takes over in that case, and the file keeps its
    // symbols.
    if symbols.is_empty() {
        return SemanticOutcome::NotApplicable;
    }

    let mut identified: Vec<FileRelation> = Vec::new();
    let mut pending: Vec<PendingReference> = Vec::new();
    for edge in &extracted.edges {
        let Some(kind) = edge
            .kind
            .strip_prefix("dependency.")
            .and_then(|k| k.parse::<DependencyType>().ok())
        else {
            continue;
        };
        match &edge.target_ref {
            TargetRef::Resolved(target) => {
                let (Some(from), Some(to)) = (
                    by_reader_id.get(edge.source.as_str()),
                    by_reader_id.get(target.as_str()),
                ) else {
                    continue;
                };
                identified.push(FileRelation {
                    caller: from.clone(),
                    callee: to.clone(),
                    kind,
                });
            }
            TargetRef::Unresolved(target) => {
                // A reference that names a declaration of the project is a
                // hole when it cannot be tied to one. A call is not: what
                // the code calls may live outside the project — a function
                // of another crate, a module of Ansible — and the graph
                // says nothing about the world outside it. Neither this
                // path nor the generic one publishes those calls as gaps,
                // and neither counts them yet: separating the calls from
                // the relations is its own work.
                if kind == DependencyType::Calls {
                    continue;
                }
                // The reference names something this file does not declare.
                // No edge is invented for it — nothing proves its far end —
                // but the reference is a fact about what the file says, and
                // the project may still be able to tie it to a declaration in
                // another file. Dropping it here would leave the consumer
                // with a graph that claims to know everything.
                let Some(source) = by_reader_id.get(edge.source.as_str()) else {
                    continue;
                };
                pending.push(PendingReference {
                    origin_file: file_path.to_string(),
                    origin_line: edge.line,
                    source: source.clone(),
                    target: target.clone(),
                    kind,
                    class: PendingClass::Unresolved,
                });
            }
        }
    }

    SemanticOutcome::Handled(FileFacts {
        symbols,
        identified,
        calls: Vec::new(),
        pending,
    })
}

/// Where the reader says the declaration is.
///
/// The reader counts the lines from one, the way the rest of the product
/// counts them, and the graph counts them from zero. A declaration that
/// carries no position carries the position nobody gave it: zero, which
/// says "nowhere in particular" instead of pointing at the first line of
/// the file.
fn location_of(node: &GraphNode, file_path: &str) -> Location {
    let properties = node.properties.as_object();
    let line = properties
        .and_then(|properties| properties.get("line"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let column = properties
        .and_then(|properties| properties.get("column"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    Location::new(file_path, line.saturating_sub(1), column.saturating_sub(1))
}

/// The declaration a reader made, when it made one.
///
/// The reader names the kind of its declaration — and the type it carries,
/// when it carries one — in the properties of the node. A node that names
/// no declaration declares nothing in particular, and no declaration is
/// invented for it.
fn declaration_of(node: &GraphNode) -> Option<Declaration> {
    let properties = node.properties.as_object()?;
    let kind = properties
        .get(Declaration::KIND_PROPERTY)
        .and_then(|value| value.as_str())?;
    match properties
        .get(Declaration::TYPE_PROPERTY)
        .and_then(|value| value.as_str())
    {
        Some(resource_type) => Some(Declaration::with_type(kind, resource_type)),
        None => Some(Declaration::new(kind)),
    }
}

/// The facts of one file, shaped as a graph.
pub fn call_graph_from(facts: &FileFacts) -> CallGraph {
    let mut graph = CallGraph::new();
    for symbol in &facts.symbols {
        graph.add_symbol(symbol.clone());
    }
    for relation in &facts.identified {
        // The reader tied both ends, and both ends are symbols of the
        // file the reader read.
        let attached = graph.add_dependency(&relation.caller, &relation.callee, relation.kind);
        debug_assert!(
            attached.is_ok(),
            "the file graph holds both ends of the relation: {attached:?}"
        );
    }
    graph
}

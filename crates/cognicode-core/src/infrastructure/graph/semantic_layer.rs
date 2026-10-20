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

use std::collections::HashMap;
use std::path::Path;

use crate::application::ingest::extractor::extract_file;
use crate::application::ingest::types::{FileFacts, FileRelation, TargetRef};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{DependencyType, Location, NodeKind};
use crate::infrastructure::graph::per_file_graph::content_hash_64;
use crate::infrastructure::parser::Language;
use crate::infrastructure::parser::language_config::LanguageConfig;

/// What the domain reader makes of one file.
///
/// `None` when the language has no reader of its own, when the reader
/// failed, or when the reader had nothing to say about this file. The
/// last case matters: a file that declares nothing is not a file the
/// domain reader claims. Answering `None` there lets the generic reader
/// take over, instead of leaving the file without symbols.
pub fn semantic_facts(language: Language, source: &str, file_path: &str) -> Option<FileFacts> {
    let config = LanguageConfig::for_language(language);
    config.semantic_handler?;
    let extracted = extract_file(
        config,
        Path::new(file_path),
        source,
        &format!("{:016x}", content_hash_64(source.as_bytes())),
    );
    if extracted.error.is_some() {
        return None;
    }

    // The reader's own identifier for a declaration and the identifier the
    // graph gives a symbol are different grammars, so the edges are
    // resolved through this map and not by comparing strings.
    let mut by_reader_id: HashMap<String, SymbolId> = HashMap::new();
    let mut symbols: Vec<Symbol> = Vec::new();

    for node in &extracted.nodes {
        // A graph models symbols. A node kind that is not a symbol has
        // nowhere to go, and no place is invented for it.
        let NodeKind::Symbol(kind) = &node.kind else {
            continue;
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
        let mut symbol = Symbol::new(&node.label, *kind, Location::new(file_path, 0, 0));
        // The identity of a declaration is the one its reader gave it.
        // The graph identifies a symbol by its qualified name, and the one
        // that builds it is the generic one — file and symbol. Letting the
        // declaration pass for a generic one made the product ask for the
        // name its reader had given, and the graph answer that it had
        // never heard of it.
        symbol.set_fqn_override(&reader_id);
        by_reader_id.insert(reader_id.clone(), SymbolId::new(&reader_id));
        symbols.push(symbol);
    }

    // A reader that exists but interpreted nothing says nothing. The
    // generic reader takes over in that case, and the file keeps its
    // symbols.
    if symbols.is_empty() {
        return None;
    }

    let mut identified: Vec<FileRelation> = Vec::new();
    for edge in &extracted.edges {
        // An edge whose far end was not resolved is not invented: its
        // absence is counted, which is what the generic path already did.
        let TargetRef::Resolved(target) = &edge.target_ref else {
            continue;
        };
        let (Some(from), Some(to)) = (
            by_reader_id.get(edge.source.as_str()),
            by_reader_id.get(target.as_str()),
        ) else {
            continue;
        };
        let Some(kind) = edge
            .kind
            .strip_prefix("dependency.")
            .and_then(|k| k.parse::<DependencyType>().ok())
        else {
            continue;
        };
        identified.push(FileRelation {
            caller: from.clone(),
            callee: to.clone(),
            kind,
        });
    }

    Some(FileFacts {
        symbols,
        identified,
        calls: Vec::new(),
    })
}

/// The facts of one file, shaped as a graph.
pub fn call_graph_from(facts: &FileFacts) -> CallGraph {
    let mut graph = CallGraph::new();
    for symbol in &facts.symbols {
        graph.add_symbol(symbol.clone());
    }
    for relation in &facts.identified {
        let _ = graph.add_dependency(&relation.caller, &relation.callee, relation.kind);
    }
    graph
}

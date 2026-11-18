//! From the changed symbols to what to read around them.
//!
//! The seeds say which symbols the lines of a change fall in. This module
//! selects, from the relations the graph already holds, the symbols a
//! reviewer or an editor reads next, each with the edge that justifies it:
//!
//! - the **contracts** a changed symbol declares that it satisfies;
//! - the **consumers**: the symbols that depend on a changed declaration
//!   through a direct edge, only when the declaration itself changed
//!   (`signature` or `added`), because a body change leaves the contract
//!   its consumers rely on where it was;
//! - the **tests** the reader marked that reach a changed symbol, with the
//!   path of the graph that reaches it.
//!
//! It does not compute what a change affects: the graph owns that question
//! and this reads its edges once. Reachability beyond a direct consumer is
//! only followed to find tests.
//!
//! ## What is not known is said
//!
//! Every relation the graph cannot vouch for becomes a [`Gap`] next to the
//! selection: a reference that could not be tied to a declaration and that
//! names a changed symbol, a changed symbol no test reaches, a hunk outside
//! every symbol, a file that could not be mapped. A gap is not an empty
//! impact; it is the part of the question this answer cannot close.

use std::collections::{BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use super::seed::{ChangeSeed, SeedKind, SeedSymbol};
use crate::application::context_pack::selection::{
    Candidate, ContextSelection, MAX_CANDIDATES, Reason,
};
use crate::application::context_pack::service::ItemRange;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::DependencyType;

/// The most consumers selected for one changed symbol.
pub const MAX_CONSUMERS_PER_SYMBOL: usize = 20;

/// The most tests selected for one changed symbol.
pub const MAX_TESTS_PER_SYMBOL: usize = 10;

/// The most unresolved references listed for one changed symbol.
pub const MAX_PENDING_PER_SYMBOL: usize = 5;

/// A reference the graph could not tie to a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRef {
    /// The file that writes the reference.
    pub file: String,
    /// The line it is written on, counted from one.
    pub line: Option<u32>,
    /// The name it names, as written.
    pub target: String,
    /// Whether several declarations carry the name.
    pub ambiguous: bool,
}

impl PendingRef {
    /// Whether the written name may be `name`: the whole name, or its last
    /// segment after a path or member separator.
    fn may_name(&self, name: &str) -> bool {
        let target = self.target.as_str();
        target == name
            || target
                .rsplit([':', '.'])
                .next()
                .is_some_and(|last| last == name)
    }
}

/// A part of the question the answer could not close.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Gap {
    /// `unmapped_file`, `outside_symbols`, `inexact_diff`, `files_not_read`,
    /// `no_test_found`, `unresolved_reference`, `ambiguous_reference`,
    /// `consumers_truncated`, `tests_truncated` or `candidates_truncated`.
    pub kind: String,
    /// What the gap is about: a path or a symbol id.
    pub subject: String,
    /// What it means for the reader.
    pub detail: String,
}

impl Gap {
    fn new(kind: &str, subject: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            subject: subject.into(),
            detail: detail.into(),
        }
    }
}

/// What was found around one changed symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChangedSymbol {
    /// The identity of the symbol.
    pub id: String,
    /// `added`, `signature` or `body`.
    pub how: String,
    /// The lines of its declaration.
    pub declaration: ItemRange,
    /// How many consumers were selected for it.
    pub consumers: usize,
    /// How many contracts were selected for it.
    pub contracts: usize,
    /// How many tests were selected for it.
    pub tests: usize,
}

/// The selection of a change, and what it leaves open.
#[derive(Debug, Clone, Default)]
pub struct ChangeSelection {
    /// The candidates, in the order they are delivered.
    pub selection: ContextSelection,
    /// The changed symbols and what was found around each.
    pub symbols: Vec<ChangedSymbol>,
    /// What the selection cannot say.
    pub gaps: Vec<Gap>,
}

/// Selects what to read around the changed symbols of `seed`.
///
/// The candidates come in four groups, each in a stable order: the changed
/// symbols, their contracts, their consumers and their tests. A symbol that
/// several groups propose keeps the first reason, so the changed symbol is
/// never listed again as the consumer of another.
pub fn select_change(
    graph: &CallGraph,
    seed: &ChangeSeed,
    pending: &[PendingRef],
) -> ChangeSelection {
    let mut out = ChangeSelection::default();
    out.gaps.extend(file_gaps(seed));

    let mut changed = Vec::new();
    let mut contracts = Vec::new();
    let mut consumers = Vec::new();
    let mut tests = Vec::new();
    for symbol in &seed.seeds {
        changed.push(Candidate {
            id: symbol.id.clone(),
            reason: Reason::ChangedSymbol {
                how: symbol.kind.code().to_string(),
                lines: symbol
                    .changed
                    .iter()
                    .map(|range| (range.start_line, range.end_line))
                    .collect(),
            },
            score: 1.0,
        });
        let mut found = ChangedSymbol {
            id: symbol.id.to_string(),
            how: symbol.kind.code().to_string(),
            declaration: symbol.declaration.clone(),
            consumers: 0,
            contracts: 0,
            tests: 0,
        };
        if symbol.kind != SeedKind::Body {
            let found_contracts = contracts_of(graph, symbol);
            found.contracts = found_contracts.len();
            contracts.extend(found_contracts);
            let (found_consumers, gap) = consumers_of(graph, symbol);
            found.consumers = found_consumers.len();
            consumers.extend(found_consumers);
            out.gaps.extend(gap);
            out.gaps.extend(pending_gaps(graph, symbol, pending));
        }
        let (found_tests, gap) = tests_of(graph, symbol);
        found.tests = found_tests.len();
        if found_tests.is_empty() && !is_test(graph, &symbol.id) {
            out.gaps.push(Gap::new(
                "no_test_found",
                symbol.id.as_str(),
                "no test the graph knows reaches this symbol: this is missing coverage, \
                 not evidence that the change is safe",
            ));
        }
        tests.extend(found_tests);
        out.gaps.extend(gap);
        out.symbols.push(found);
    }

    let mut seen = BTreeSet::new();
    let mut dropped = 0usize;
    for candidate in changed
        .into_iter()
        .chain(contracts)
        .chain(consumers)
        .chain(tests)
    {
        if !seen.insert(candidate.id.clone()) {
            continue;
        }
        if out.selection.candidates.len() == MAX_CANDIDATES {
            dropped += 1;
            continue;
        }
        out.selection.candidates.push(candidate);
    }
    if dropped > 0 {
        out.gaps.push(Gap::new(
            "candidates_truncated",
            "selection",
            format!("{dropped} more candidate(s) than the {MAX_CANDIDATES} a selection holds"),
        ));
    }
    out
}

fn is_test(graph: &CallGraph, id: &SymbolId) -> bool {
    graph.get_symbol(id).is_some_and(|symbol| symbol.is_test())
}

/// The gaps a file leaves, whatever its symbols are.
fn file_gaps(seed: &ChangeSeed) -> Vec<Gap> {
    let mut gaps = Vec::new();
    for file in &seed.files {
        if file.state == "unmapped" {
            let reason = file.reason.as_deref().unwrap_or("unknown");
            let detail = file.detail.as_deref().unwrap_or_default();
            gaps.push(Gap::new(
                "unmapped_file",
                file.path.as_str(),
                format!("{reason}: {detail}; its consumers and tests are not known"),
            ));
        }
        if file.outside_symbols > 0 {
            gaps.push(Gap::new(
                "outside_symbols",
                file.path.as_str(),
                format!(
                    "{} changed hunk(s) fall in no symbol of the graph: what depends on them is not known",
                    file.outside_symbols
                ),
            ));
        }
        if !file.exact {
            gaps.push(Gap::new(
                "inexact_diff",
                file.path.as_str(),
                "the changed lines are a superset of the minimal diff: the file changed too much to diff exactly",
            ));
        }
    }
    if seed.files_not_read > 0 {
        gaps.push(Gap::new(
            "files_not_read",
            "change",
            format!(
                "{} changed file(s) past the bound were not read",
                seed.files_not_read
            ),
        ));
    }
    gaps
}

/// The symbols that `symbol` declares it satisfies.
fn contracts_of(graph: &CallGraph, symbol: &SeedSymbol) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = graph
        .dependencies_with_metadata(&symbol.id)
        .filter(|(_, edge, _, _)| **edge == DependencyType::Inherits)
        .map(|(target, edge, _, _)| Candidate {
            id: target.clone(),
            reason: Reason::ContractOfChange {
                changed: symbol.id.clone(),
                edge: edge.to_string(),
            },
            score: 0.0,
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// The symbols that depend on `symbol` through one edge, and the gap when
/// there are more than the bound.
fn consumers_of(graph: &CallGraph, symbol: &SeedSymbol) -> (Vec<Candidate>, Option<Gap>) {
    let mut found: Vec<Candidate> = graph
        .incoming_dependencies_with_metadata(&symbol.id)
        .into_iter()
        .filter(|(source, edge, _, _)| {
            **source != symbol.id && **edge != DependencyType::Contains && !is_test(graph, source)
        })
        .map(|(source, edge, provenance, confidence)| Candidate {
            id: source.clone(),
            reason: Reason::ConsumerOfChange {
                changed: symbol.id.clone(),
                edge: edge.to_string(),
                provenance: provenance.to_string(),
                confidence,
            },
            score: confidence,
        })
        .collect();
    found.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    found.dedup_by(|b, a| a.id == b.id);
    let total = found.len();
    found.truncate(MAX_CONSUMERS_PER_SYMBOL);
    let gap = (total > found.len()).then(|| {
        Gap::new(
            "consumers_truncated",
            symbol.id.as_str(),
            format!(
                "{total} consumers depend on it and {} are listed",
                found.len()
            ),
        )
    });
    (found, gap)
}

/// The tests that reach `symbol`, nearest first, and the gap when there are
/// more than the bound.
fn tests_of(graph: &CallGraph, symbol: &SeedSymbol) -> (Vec<Candidate>, Option<Gap>) {
    let mut found: Vec<Candidate> = reaching_paths(graph, &symbol.id)
        .into_iter()
        .filter(|(id, _)| is_test(graph, id))
        .map(|(id, path)| Candidate {
            score: 1.0 / path.len().max(1) as f64,
            reason: Reason::TestOfChange {
                changed: symbol.id.clone(),
                path,
            },
            id,
        })
        .collect();
    found.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    let total = found.len();
    found.truncate(MAX_TESTS_PER_SYMBOL);
    let gap = (total > found.len()).then(|| {
        Gap::new(
            "tests_truncated",
            symbol.id.as_str(),
            format!("{total} tests reach it and {} are listed", found.len()),
        )
    });
    (found, gap)
}

/// Every symbol that reaches `target` through the edges of the graph, with
/// a shortest path from it to `target`, the symbol first.
///
/// The graph answers *who* reaches a symbol; the path is what lets a reader
/// check the claim, so it is kept while the dependents are walked, breadth
/// first over the reverse edges (which the graph publishes in id order).
fn reaching_paths(graph: &CallGraph, target: &SymbolId) -> Vec<(SymbolId, Vec<SymbolId>)> {
    let mut next_hop: HashMap<SymbolId, SymbolId> = HashMap::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::from([target.clone()]);
    while let Some(current) = queue.pop_front() {
        for dependent in graph.dependents(&current) {
            if dependent == target || next_hop.contains_key(dependent) {
                continue;
            }
            next_hop.insert(dependent.clone(), current.clone());
            order.push(dependent.clone());
            queue.push_back(dependent.clone());
        }
    }
    order
        .into_iter()
        .map(|start| {
            let mut path = vec![start.clone()];
            let mut at = &start;
            while let Some(next) = next_hop.get(at) {
                path.push(next.clone());
                at = next;
            }
            (start, path)
        })
        .collect()
}

/// The references the graph could not tie to a declaration that may name
/// `symbol`: they are consumers the graph cannot vouch for.
fn pending_gaps(graph: &CallGraph, symbol: &SeedSymbol, pending: &[PendingRef]) -> Vec<Gap> {
    let Some(name) = graph.get_symbol(&symbol.id).map(|found| found.name()) else {
        return Vec::new();
    };
    let matching: Vec<&PendingRef> = pending.iter().filter(|r| r.may_name(name)).collect();
    let mut gaps: Vec<Gap> = matching
        .iter()
        .take(MAX_PENDING_PER_SYMBOL)
        .map(|reference| {
            let at = match reference.line {
                Some(line) => format!("{}:{line}", reference.file),
                None => reference.file.clone(),
            };
            Gap::new(
                if reference.ambiguous {
                    "ambiguous_reference"
                } else {
                    "unresolved_reference"
                },
                symbol.id.as_str(),
                format!(
                    "`{}` written at {at} could not be tied to a declaration: it may consume this symbol",
                    reference.target
                ),
            )
        })
        .collect();
    if matching.len() > MAX_PENDING_PER_SYMBOL {
        gaps.push(Gap::new(
            "unresolved_reference",
            symbol.id.as_str(),
            format!(
                "{} more unresolved reference(s) may name it",
                matching.len() - MAX_PENDING_PER_SYMBOL
            ),
        ));
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::context_pack::change::seed::ChangedFile;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::{Location, SymbolKind};

    struct World {
        graph: CallGraph,
    }

    impl World {
        fn id(&self, name: &str) -> SymbolId {
            self.graph
                .find_by_name(name)
                .into_iter()
                .find(|symbol| symbol.name() == name)
                .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
                .unwrap_or_else(|| panic!("no symbol {name}"))
        }

        fn seed(&self, name: &str, kind: SeedKind) -> SeedSymbol {
            SeedSymbol {
                id: self.id(name),
                file: "src/a.rs".to_string(),
                kind,
                declaration: ItemRange {
                    start_line: 1,
                    end_line: 3,
                },
                changed: Vec::new(),
            }
        }
    }

    fn function(name: &str, file: &str, line: u32) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new(file, line, 0))
    }

    /// `parse` is called by `run` and, through a guess, by `render`; the test
    /// `test_run` reaches it through `run`; `parse_all` implements the
    /// contract `Parse`; `helper` is only called by `run`; `test_lonely`
    /// reaches nothing.
    fn world() -> World {
        let mut graph = CallGraph::new();
        let parse = graph.add_symbol(function("parse", "src/a.rs", 0));
        let run = graph.add_symbol(function("run", "src/a.rs", 10));
        let render = graph.add_symbol(function("render", "src/b.rs", 0));
        let helper = graph.add_symbol(function("helper", "src/a.rs", 20));
        let parse_all = graph.add_symbol(function("parse_all", "src/c.rs", 0));
        let contract = graph.add_symbol(Symbol::new(
            "Parse",
            SymbolKind::Trait,
            Location::new("src/c.rs", 30, 0),
        ));
        let test_run = graph.add_symbol(function("test_run", "tests/run.rs", 0).mark_as_test());
        graph.add_symbol(function("test_lonely", "tests/lonely.rs", 0).mark_as_test());
        for (from, to) in [(&run, &parse), (&run, &helper), (&test_run, &run)] {
            graph
                .add_dependency(from, to, DependencyType::Calls)
                .unwrap();
        }
        graph
            .add_dependency_with_provenance(
                &render,
                &parse,
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.6 },
            )
            .unwrap();
        graph
            .add_dependency(&parse_all, &contract, DependencyType::Inherits)
            .unwrap();
        World { graph }
    }

    fn seed_of(symbols: Vec<SeedSymbol>) -> ChangeSeed {
        ChangeSeed {
            files: Vec::new(),
            seeds: symbols,
            files_not_read: 0,
        }
    }

    fn codes(selection: &ChangeSelection) -> Vec<(String, &'static str)> {
        selection
            .selection
            .candidates
            .iter()
            .map(|c| {
                (
                    c.id.as_str().split(':').nth(1).unwrap().to_string(),
                    c.reason.code(),
                )
            })
            .collect()
    }

    fn gap_kinds(selection: &ChangeSelection) -> Vec<&str> {
        selection.gaps.iter().map(|g| g.kind.as_str()).collect()
    }

    #[test]
    fn a_signature_change_selects_consumers_and_the_tests_that_reach_it() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Signature)]);
        let selected = select_change(&world.graph, &seed, &[]);

        assert_eq!(
            codes(&selected),
            [
                ("parse".to_string(), "changed_symbol"),
                ("run".to_string(), "consumer_of_change"),
                ("render".to_string(), "consumer_of_change"),
                ("test_run".to_string(), "test_of_change"),
            ]
        );
        assert_eq!(selected.symbols[0].consumers, 2);
        assert_eq!(selected.symbols[0].tests, 1);
    }

    #[test]
    fn a_test_that_calls_the_change_directly_is_a_test_and_not_a_consumer() {
        let mut world = world();
        let parse = world.id("parse");
        let direct = world
            .graph
            .add_symbol(function("test_direct", "tests/direct.rs", 0).mark_as_test());
        world
            .graph
            .add_dependency(&direct, &parse, DependencyType::Calls)
            .unwrap();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Signature)]);
        let selected = select_change(&world.graph, &seed, &[]);

        let reasons = codes(&selected);
        assert!(reasons.contains(&("test_direct".to_string(), "test_of_change")));
        assert_eq!(selected.symbols[0].consumers, 2);
        assert_eq!(selected.symbols[0].tests, 2);
    }

    #[test]
    fn a_body_change_selects_the_tests_and_no_consumers() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Body)]);
        let selected = select_change(&world.graph, &seed, &[]);

        assert_eq!(
            codes(&selected),
            [
                ("parse".to_string(), "changed_symbol"),
                ("test_run".to_string(), "test_of_change"),
            ]
        );
    }

    #[test]
    fn a_changed_symbol_says_which_of_its_lines_changed() {
        let world = world();
        let mut parse = world.seed("parse", SeedKind::Body);
        parse.changed = vec![
            ItemRange {
                start_line: 2,
                end_line: 2,
            },
            ItemRange {
                start_line: 5,
                end_line: 7,
            },
        ];
        let selected = select_change(&world.graph, &seed_of(vec![parse]), &[]);

        assert_eq!(
            selected.selection.candidates[0].reason.detail().as_deref(),
            Some("body; changed lines 2, 5-7")
        );
    }

    #[test]
    fn a_narrower_symbol_gets_a_narrower_selection() {
        let world = world();
        let api = select_change(
            &world.graph,
            &seed_of(vec![world.seed("parse", SeedKind::Signature)]),
            &[],
        );
        let private = select_change(
            &world.graph,
            &seed_of(vec![world.seed("helper", SeedKind::Signature)]),
            &[],
        );

        assert!(private.selection.candidates.len() < api.selection.candidates.len());
        assert_eq!(
            codes(&private),
            [
                ("helper".to_string(), "changed_symbol"),
                ("run".to_string(), "consumer_of_change"),
                ("test_run".to_string(), "test_of_change"),
            ]
        );
    }

    #[test]
    fn every_consumer_carries_the_edge_that_justifies_it() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Signature)]);
        let selected = select_change(&world.graph, &seed, &[]);

        let render = selected
            .selection
            .candidates
            .iter()
            .find(|c| c.id == world.id("render"))
            .unwrap();
        let detail = render.reason.detail().unwrap();
        assert!(detail.starts_with("calls "), "{detail}");
        assert!(detail.contains("inferred"), "{detail}");
        assert!(detail.contains("confidence 0.60"), "{detail}");
        let run = selected
            .selection
            .candidates
            .iter()
            .find(|c| c.id == world.id("run"))
            .unwrap();
        assert!(run.reason.detail().unwrap().contains("extracted"));
    }

    #[test]
    fn a_test_carries_the_path_of_the_graph_that_reaches_the_change() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Body)]);
        let selected = select_change(&world.graph, &seed, &[]);

        let test = &selected.selection.candidates[1];
        let Reason::TestOfChange { path, .. } = &test.reason else {
            panic!("not a test")
        };
        assert_eq!(
            path,
            &[world.id("test_run"), world.id("run"), world.id("parse")]
        );
    }

    #[test]
    fn the_contract_a_changed_symbol_satisfies_is_selected_with_its_edge() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse_all", SeedKind::Signature)]);
        let selected = select_change(&world.graph, &seed, &[]);

        assert_eq!(
            codes(&selected)[..2],
            [
                ("parse_all".to_string(), "changed_symbol"),
                ("Parse".to_string(), "contract_of_change"),
            ]
        );
        assert_eq!(selected.symbols[0].contracts, 1);
    }

    #[test]
    fn a_symbol_no_test_reaches_is_missing_coverage_not_a_clean_change() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse_all", SeedKind::Body)]);
        let selected = select_change(&world.graph, &seed, &[]);

        assert_eq!(gap_kinds(&selected), ["no_test_found"]);
        assert!(selected.gaps[0].detail.contains("missing coverage"));
    }

    #[test]
    fn a_changed_test_is_not_reported_as_untested() {
        let world = world();
        let seed = seed_of(vec![world.seed("test_lonely", SeedKind::Body)]);
        let selected = select_change(&world.graph, &seed, &[]);

        assert!(gap_kinds(&selected).is_empty());
    }

    #[test]
    fn references_the_graph_could_not_resolve_are_unknown_consumers() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Signature)]);
        let pending = [
            PendingRef {
                file: "src/d.rs".to_string(),
                line: Some(7),
                target: "Parser::parse".to_string(),
                ambiguous: true,
            },
            PendingRef {
                file: "src/e.rs".to_string(),
                line: None,
                target: "unrelated".to_string(),
                ambiguous: false,
            },
        ];
        let selected = select_change(&world.graph, &seed, &pending);

        assert_eq!(gap_kinds(&selected), ["ambiguous_reference"]);
        assert!(selected.gaps[0].detail.contains("src/d.rs:7"));
    }

    #[test]
    fn a_body_change_does_not_ask_about_unresolved_consumers() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Body)]);
        let pending = [PendingRef {
            file: "src/d.rs".to_string(),
            line: Some(7),
            target: "parse".to_string(),
            ambiguous: false,
        }];

        assert!(select_change(&world.graph, &seed, &pending).gaps.is_empty());
    }

    #[test]
    fn files_that_could_not_be_mapped_are_gaps_and_never_silence() {
        let world = world();
        let mut seed = seed_of(Vec::new());
        seed.files_not_read = 3;
        for (reason, outside, exact) in [(Some("snapshot_differs"), 0, true), (None, 2, false)] {
            seed.files.push(ChangedFile {
                path: "src/x.rs".to_string(),
                change: "modified".to_string(),
                from: None,
                state: if reason.is_some() {
                    "unmapped"
                } else {
                    "mapped"
                }
                .to_string(),
                reason: reason.map(str::to_string),
                detail: None,
                changed_lines: Vec::new(),
                removed_lines: 0,
                outside_symbols: outside,
                symbols: Vec::new(),
                exact,
            });
        }
        let selected = select_change(&world.graph, &seed, &[]);

        assert_eq!(
            gap_kinds(&selected),
            [
                "unmapped_file",
                "outside_symbols",
                "inexact_diff",
                "files_not_read"
            ]
        );
        assert!(selected.selection.candidates.is_empty());
    }

    #[test]
    fn a_changed_symbol_is_never_listed_again_as_a_consumer() {
        let world = world();
        let seed = seed_of(vec![
            world.seed("parse", SeedKind::Signature),
            world.seed("run", SeedKind::Signature),
        ]);
        let selected = select_change(&world.graph, &seed, &[]);

        let runs = selected
            .selection
            .candidates
            .iter()
            .filter(|c| c.id == world.id("run"))
            .count();
        assert_eq!(runs, 1);
        assert_eq!(
            selected.selection.candidates[1].reason.code(),
            "changed_symbol"
        );
    }

    #[test]
    fn consumers_past_the_bound_are_counted_not_dropped_silently() {
        let mut graph = CallGraph::new();
        let target = graph.add_symbol(function("target", "src/a.rs", 0));
        for at in 0..MAX_CONSUMERS_PER_SYMBOL + 5 {
            let caller =
                graph.add_symbol(function(&format!("caller_{at:02}"), "src/b.rs", at as u32));
            graph
                .add_dependency(&caller, &target, DependencyType::Calls)
                .unwrap();
        }
        let seed = seed_of(vec![SeedSymbol {
            id: target,
            file: "src/a.rs".to_string(),
            kind: SeedKind::Signature,
            declaration: ItemRange {
                start_line: 1,
                end_line: 3,
            },
            changed: Vec::new(),
        }]);
        let selected = select_change(&graph, &seed, &[]);

        assert_eq!(selected.symbols[0].consumers, MAX_CONSUMERS_PER_SYMBOL);
        let gap = selected
            .gaps
            .iter()
            .find(|g| g.kind == "consumers_truncated");
        assert!(gap.is_some_and(|g| g.detail.starts_with("25 consumers")));
    }

    #[test]
    fn the_same_change_selects_the_same_list_every_time() {
        let world = world();
        let seed = seed_of(vec![world.seed("parse", SeedKind::Signature)]);
        let first = codes(&select_change(&world.graph, &seed, &[]));
        let second = codes(&select_change(&world.graph, &seed, &[]));

        assert_eq!(first, second);
    }
}

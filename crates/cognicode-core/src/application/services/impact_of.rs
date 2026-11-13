//! The one answer to "what depends on this name".
//!
//! The MCP tool, the CLI command and the workspace session all put this
//! question. They share the answer, so a name resolves to the same symbol and
//! the same risk whichever surface asked.
//!
//! A name means ONE symbol, and the domain's rule decides which
//! ([`crate::domain::services::resolve_target`]): the same rule that answers a call hierarchy or a
//! call path. The full identity (`file:name:line`) names exactly one symbol;
//! a name that several symbols carry is refused together with its
//! candidates, because the impact of "`helper`" is the impact of none of the
//! `helper`s: adding them up publishes the callers of one as if they were
//! the callers of the other, and inflates the risk with them. A name the
//! graph does not hold is [`GraphQueryError::NotFound`], never an empty
//! impact.

use std::collections::BTreeSet;
use std::path::Path;

use crate::application::dto::AffectedTest;
use crate::application::dto::RiskLevel;
use crate::application::services::graph_queries::{GraphQueryError, resolve};
use crate::domain::aggregates::Symbol;
use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::services::command_for;
use crate::domain::value_objects::{Language, published_line};

/// What changing a named symbol reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactOf {
    /// How many symbols the name resolved to.
    ///
    /// Always one: a name that several symbols carry is refused instead of
    /// summed. The field stays because the goal `impact` publishes it.
    pub matched: usize,
    /// Names of the symbols that depend, directly or not, on any of them.
    pub symbols: Vec<String>,
    /// Files those dependents live in.
    pub files: Vec<String>,
    /// Blast radius derived from the number of dependent symbols.
    pub risk: RiskLevel,
    /// The tests that reach the change, and the command that runs each one.
    ///
    /// The tests are the dependents the reader marked as tests: a name that
    /// looks like a test is not one, and a helper of a test is not one
    /// either. Empty is the truth when nothing tests the change.
    pub affected_tests: Vec<AffectedTest>,
}

/// The impact of changing the symbol `name` means in `graph`.
pub fn impact_of(graph: &CallGraph, name: &str) -> Result<ImpactOf, GraphQueryError> {
    let target = resolve(graph, name)?;

    let mut symbols = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut affected_tests: Vec<AffectedTest> = Vec::new();
    for dependent in graph.find_all_dependents(&target) {
        if let Some(symbol) = graph.get_symbol(&dependent) {
            symbols.insert(symbol.name().to_string());
            files.insert(symbol.location().file().to_string());
            if symbol.is_test() {
                affected_tests.push(affected_test(symbol));
            }
        }
    }
    // The order of the answer is part of the answer: the tests come in the
    // order of the file they live in, so the same question gives the same
    // list twice.
    affected_tests.sort_by(|a, b| (&a.file, a.line, &a.name).cmp(&(&b.file, b.line, &b.name)));

    Ok(ImpactOf {
        matched: 1,
        risk: RiskLevel::from_impacted_symbols(symbols.len()),
        symbols: symbols.into_iter().collect(),
        files: files.into_iter().collect(),
        affected_tests,
    })
}

/// The test the answer names, and the command that runs it.
///
/// The command is the one the language's convention says: where the
/// language does not decide its own runner, there is none, and the answer
/// says so rather than inventing one.
fn affected_test(symbol: &Symbol) -> AffectedTest {
    let file = symbol.location().file();
    let command = Language::from_path(Path::new(file))
        .and_then(|language| command_for(language, file, symbol.name()));
    AffectedTest {
        name: symbol.name().to_string(),
        file: file.to_string(),
        line: published_line(symbol.location().line()),
        command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Symbol;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    fn function(name: &str, file: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new(file, 1, 1))
    }

    /// `foo` and `foo_bar`, each called by its own function.
    fn graph() -> CallGraph {
        let mut graph = CallGraph::new();
        let foo = graph.add_symbol(function("foo", "a.rs"));
        let foo_bar = graph.add_symbol(function("foo_bar", "a.rs"));
        let uses_foo = graph.add_symbol(function("uses_foo", "b.rs"));
        let uses_bar = graph.add_symbol(function("uses_bar", "c.rs"));
        graph
            .add_dependency(&uses_foo, &foo, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&uses_bar, &foo_bar, DependencyType::Calls)
            .unwrap();
        graph
    }

    #[test]
    fn a_name_matches_only_the_symbols_that_carry_it() {
        let impact = impact_of(&graph(), "foo").unwrap();

        assert_eq!(impact.matched, 1);
        assert_eq!(impact.symbols, vec!["uses_foo".to_string()]);
        assert_eq!(impact.files, vec!["b.rs".to_string()]);
    }

    #[test]
    fn the_match_ignores_case() {
        let impact = impact_of(&graph(), "FOO_BAR").unwrap();

        assert_eq!(impact.symbols, vec!["uses_bar".to_string()]);
    }

    #[test]
    fn a_name_the_graph_does_not_hold_is_not_an_empty_impact() {
        assert!(impact_of(&graph(), "fo").is_err());
        assert!(impact_of(&graph(), "missing").is_err());
    }

    /// Two functions called `helper`, one per file, each with its own
    /// caller. Returns the graph and the identity of each homonym.
    fn homonyms() -> (CallGraph, String, String) {
        let mut graph = CallGraph::new();
        let helper_a = graph.add_symbol(function("helper", "a.rs"));
        let helper_b = graph.add_symbol(function("helper", "b.rs"));
        let caller_a = graph.add_symbol(function("caller_of_a", "ca.rs"));
        let caller_b = graph.add_symbol(function("caller_of_b", "cb.rs"));
        graph
            .add_dependency(&caller_a, &helper_a, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&caller_b, &helper_b, DependencyType::Calls)
            .unwrap();
        (
            graph,
            helper_a.as_str().to_string(),
            helper_b.as_str().to_string(),
        )
    }

    #[test]
    fn a_name_that_two_symbols_carry_is_refused_with_its_candidates() {
        let (graph, in_a, in_b) = homonyms();

        let refusal = impact_of(&graph, "helper").unwrap_err();

        assert_eq!(
            refusal,
            GraphQueryError::Ambiguous {
                name: "helper".to_string(),
                candidates: vec![in_a, in_b],
            },
            "the impact of `helper` is the impact of NEITHER of them: it says which two"
        );
    }

    #[test]
    fn the_identity_of_one_homonym_reaches_only_its_own_callers() {
        let (graph, in_a, in_b) = homonyms();

        let impact_a = impact_of(&graph, &in_a).unwrap();
        let impact_b = impact_of(&graph, &in_b).unwrap();

        assert_eq!(impact_a.matched, 1);
        assert_eq!(impact_a.symbols, vec!["caller_of_a".to_string()]);
        assert_eq!(impact_a.files, vec!["ca.rs".to_string()]);
        assert_eq!(impact_b.symbols, vec!["caller_of_b".to_string()]);
        assert_eq!(impact_b.files, vec!["cb.rs".to_string()]);
    }

    #[test]
    fn the_refusal_of_a_missing_name_is_not_the_refusal_of_a_homonym() {
        let (graph, _, _) = homonyms();

        assert_eq!(
            impact_of(&graph, "ghost").unwrap_err(),
            GraphQueryError::NotFound {
                name: "ghost".to_string()
            }
        );
    }

    #[test]
    fn a_symbol_nobody_calls_has_low_risk_and_no_dependents() {
        let impact = impact_of(&graph(), "uses_foo").unwrap();

        assert!(impact.symbols.is_empty());
        assert_eq!(impact.risk, RiskLevel::Low);
    }

    #[test]
    fn the_answer_is_ordered() {
        let mut graph = CallGraph::new();
        let target = graph.add_symbol(function("target", "t.rs"));
        for name in ["zeta", "alpha", "mid"] {
            let caller = graph.add_symbol(function(name, "c.rs"));
            graph
                .add_dependency(&caller, &target, DependencyType::Calls)
                .unwrap();
        }

        let impact = impact_of(&graph, "target").unwrap();

        assert_eq!(impact.symbols, ["alpha", "mid", "zeta"]);
        assert_eq!(impact.risk, RiskLevel::Medium);
    }
}

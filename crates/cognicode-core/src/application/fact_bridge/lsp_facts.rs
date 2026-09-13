//! Code-intelligence adapter (E37 design D1).
//!
//! Turns `&dyn CodeIntelligenceProvider` observations for a set of files
//! into canonical relation observations:
//!
//! - all five `ReferenceKind` variants map onto canonical predicates:
//!   `Call` → `core:calls`, `Import` → `core:imports`, and
//!   `Read`/`Write`/`Type` → `core:references` with the kind recorded in
//!   `provenance.detail` as `ref=<Kind>`;
//! - `get_hierarchy` parents → `(type, core:inherits, Text(parent))`.
//!
//! Subjects follow the canonical subject grammar declared in
//! [`super`] (E38.1 CP-3): a subject that denotes a symbol is that
//! symbol's 1-BASED fact-side FQN, so RuntimeObserver facts join the
//! `core:defines` entities the DeterministicAnalyzer emits for the same
//! source. Container references resolve through the per-file extraction
//! context ([`SubjectIndex`]); anything unresolvable keeps the documented
//! non-joinable fallback (the reference-site file path — never an invented
//! entity FQN).
//!
//! Producer: `RuntimeObserver` (design D1 authority table).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::application::fact_bridge::batch_builder::FactBatchBuilder;
use crate::domain::aggregates::Symbol;
use crate::domain::evidence_kernel::fact::ProducerKind;
use crate::domain::evidence_kernel::symbol_fqn::SymbolFqn;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceProvider, Reference, ReferenceKind,
};
use crate::domain::value_objects::SymbolKind;

/// Producer stamped on every provider-derived observation (design D1).
const PRODUCER: ProducerKind = ProducerKind::RuntimeObserver;

/// The per-file extraction context for subject normalization (E38.1 CP-3):
/// maps symbol names onto the symbol's 1-BASED fact-side FQN.
///
/// Built from the `get_symbols` symbols of the walked files (the extraction
/// context). `Symbol::location()` stores the ZERO-BASED tree-sitter
/// `start.row` (the legacy-side convention, `Symbol::from_legacy_side`
/// grammar), so the fact-side FQN re-bases the line onto
/// [`SymbolFqn::from_fact_side`] — the same declared 0-based → 1-based
/// step as the equivalence harness's `normalize_legacy_fqn`.
///
/// Determinism: `SymbolKind::File` symbols are excluded (files are never
/// `core:defines` subjects), and a duplicate name (same-name symbols in one
/// file) tie-breaks on the lexicographically smallest FQN — the same rule
/// as the shared callee resolver (`resolve_callee_identity`).
#[derive(Debug, Clone, Default)]
struct SubjectIndex {
    by_name: BTreeMap<String, String>,
}

impl SubjectIndex {
    /// Builds the index from one file's extraction-context symbols.
    fn from_symbols(symbols: &[Symbol]) -> Self {
        let mut candidates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for symbol in symbols {
            if *symbol.kind() == SymbolKind::File {
                continue; // files are evidence input, not tracked entities
            }
            let location = symbol.location();
            let fqn = SymbolFqn::from_fact_side(
                location.file(),
                symbol.name(),
                // Declared 0-based → 1-based re-base (the `Symbol` legacy
                // line onto the extractor's fact-side convention).
                location.line() + 1,
            )
            .assemble();
            candidates
                .entry(symbol.name().to_string())
                .or_default()
                .insert(fqn);
        }
        Self {
            by_name: candidates
                .into_iter()
                .map(|(name, fqns)| {
                    let fqn = fqns
                        .into_iter()
                        .next()
                        .expect("BTreeSet built from at least one insert");
                    (name, fqn)
                })
                .collect(),
        }
    }

    /// The enclosing symbol's fact-side FQN for a container name, when the
    /// container resolves in this file's extraction context.
    fn resolve(&self, container: &str) -> Option<&str> {
        self.by_name.get(container).map(String::as_str)
    }
}

/// Adds every relation observable through `provider` for `files` into
/// `builder`. Provider errors degrade per file/per query (the observation
/// is skipped), which is deterministic for a given provider state.
pub async fn collect(
    builder: &mut FactBatchBuilder,
    provider: &dyn CodeIntelligenceProvider,
    files: &[PathBuf],
) {
    // One `get_symbols` round per file (unchanged walk semantics); the
    // symbols are kept so EVERY file's observations can resolve containers
    // against EVERY file's extraction context (references may be sited in a
    // different file than the queried symbol's definition).
    let mut walked: Vec<(&PathBuf, Vec<Symbol>)> = Vec::new();
    for path in files {
        match provider.get_symbols(path).await {
            Ok(symbols) => walked.push((path, symbols)),
            Err(_) => continue, // provider degradation: skip the file
        }
    }
    let indexes: BTreeMap<String, SubjectIndex> = walked
        .iter()
        .map(|(path, symbols)| {
            (
                path.to_string_lossy().into_owned(),
                SubjectIndex::from_symbols(symbols),
            )
        })
        .collect();
    for (_path, symbols) in &walked {
        for symbol in symbols {
            collect_references(builder, symbol, provider, &indexes).await;
            collect_hierarchy(builder, symbol, provider).await;
        }
    }
}

/// Resolves one reference observation's subject per the canonical subject
/// grammar (E38.1 CP-3): a container that resolves against the reference
/// site's extraction context becomes the enclosing symbol's 1-based
/// fact-side FQN (joinable); anything else — no container reported, or a
/// container that matches no context symbol — falls back to the reference
/// site's FILE PATH, a documented NON-joinable subject form (files are
/// never `core:defines` subjects, so the fallback can never fabricate or
/// mis-join an entity).
fn reference_subject(reference: &Reference, indexes: &BTreeMap<String, SubjectIndex>) -> String {
    reference
        .container
        .as_deref()
        .and_then(|container| {
            indexes
                .get(reference.location.file())
                .and_then(|index| index.resolve(container))
        })
        .map_or_else(|| reference.location.file().to_string(), str::to_string)
}

/// Maps all five `ReferenceKind` variants onto canonical predicates
/// (design D1).
async fn collect_references(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn CodeIntelligenceProvider,
    indexes: &BTreeMap<String, SubjectIndex>,
) {
    let references: Vec<Reference> = match provider.find_references(symbol.location(), false).await
    {
        Ok(references) => references,
        Err(_) => return, // provider degradation: no reference observations
    };
    for reference in &references {
        let (predicate, detail) = match reference.reference_kind {
            ReferenceKind::Call => ("core:calls", None),
            ReferenceKind::Import => ("core:imports", None),
            ReferenceKind::Read | ReferenceKind::Write | ReferenceKind::Type => (
                "core:references",
                Some(format!(
                    "ref={}",
                    reference_kind_name(reference.reference_kind)
                )),
            ),
        };
        builder
            .add_observation(
                reference_subject(reference, indexes),
                super::relation(predicate),
                symbol.name(),
                PRODUCER,
                detail,
            )
            .expect("RuntimeObserver is never rejected");
    }
}

/// Maps hierarchy parents onto `core:inherits` facts (design D1).
///
/// The subject is the QUERIED symbol itself, so it is always resolvable:
/// its 1-based fact-side FQN (canonical subject grammar, E38.1 CP-3),
/// which joins the tree-sitter defines entity of the same symbol.
async fn collect_hierarchy(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn CodeIntelligenceProvider,
) {
    let hierarchy = match provider.get_hierarchy(symbol.location()).await {
        Ok(hierarchy) => hierarchy,
        Err(_) => return, // provider degradation: no hierarchy observations
    };
    let subject = SymbolFqn::from_fact_side(
        symbol.location().file(),
        symbol.name(),
        // Declared 0-based → 1-based re-base (the `Symbol` legacy line onto
        // the extractor's fact-side convention).
        symbol.location().line() + 1,
    )
    .assemble();
    for parent in &hierarchy.parents {
        builder
            .add_observation(
                subject.clone(),
                super::relation("core:inherits"),
                parent.symbol.name(),
                PRODUCER,
                None,
            )
            .expect("RuntimeObserver is never rejected");
    }
}

/// The serde name of a [`ReferenceKind`] variant (`ref=<Kind>` detail,
/// design D1). Exhaustive so a new variant fails compilation here.
fn reference_kind_name(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Read => "Read",
        ReferenceKind::Write => "Write",
        ReferenceKind::Call => "Call",
        ReferenceKind::Type => "Type",
        ReferenceKind::Import => "Import",
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use async_trait::async_trait;

    use crate::domain::aggregates::Symbol;
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind};
    use crate::domain::evidence_kernel::ids::{EntityId, SnapshotId};
    use crate::domain::traits::code_intelligence::{
        CodeIntelligenceError, DocumentSymbol, HoverInfo, Reference, TypeHierarchy,
        TypeHierarchyNode,
    };
    use crate::domain::value_objects::{Location, SymbolKind};

    use super::*;

    const SNAPSHOT: u64 = 1;

    /// Mock provider returning one function symbol per file, five
    /// references (one per `ReferenceKind`), and one hierarchy parent.
    struct MockObserver;

    fn fixed_symbol() -> Symbol {
        Symbol::new(
            "do_work",
            SymbolKind::Function,
            Location::new("src/lib.rs", 4, 0),
        )
    }

    #[async_trait]
    impl CodeIntelligenceProvider for MockObserver {
        async fn get_symbols(&self, _path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            Ok(vec![fixed_symbol()])
        }

        async fn find_references(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            let at = |file: &str| Location::new(file, 6, 2);
            Ok(vec![
                Reference {
                    location: at("src/lib.rs"),
                    reference_kind: ReferenceKind::Call,
                    container: Some("main".to_string()),
                },
                Reference {
                    location: at("src/lib.rs"),
                    reference_kind: ReferenceKind::Import,
                    container: Some("main".to_string()),
                },
                Reference {
                    location: at("src/lib.rs"),
                    reference_kind: ReferenceKind::Read,
                    container: Some("main".to_string()),
                },
                Reference {
                    location: at("src/lib.rs"),
                    reference_kind: ReferenceKind::Write,
                    container: None,
                },
                Reference {
                    location: at("src/lib.rs"),
                    reference_kind: ReferenceKind::Type,
                    container: None,
                },
            ])
        }

        async fn get_hierarchy(
            &self,
            _location: &Location,
        ) -> Result<TypeHierarchy, CodeIntelligenceError> {
            Ok(TypeHierarchy {
                symbol: Symbol::new(
                    "Widget",
                    SymbolKind::Class,
                    Location::new("src/lib.rs", 10, 0),
                ),
                parents: vec![TypeHierarchyNode {
                    symbol: Symbol::new(
                        "BaseWidget",
                        SymbolKind::Class,
                        Location::new("src/base.rs", 2, 0),
                    ),
                    distance: 1,
                }],
                children: vec![],
            })
        }

        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            Ok(None)
        }

        async fn get_document_symbols(
            &self,
            _path: &Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            Ok(vec![])
        }

        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            Ok(None)
        }
    }

    /// All five `ReferenceKind` variants map onto their canonical
    /// predicates with `ref=<Kind>` details where required, and hierarchy
    /// parents become `core:inherits` facts (design D1).
    #[tokio::test]
    async fn maps_reference_kinds_and_hierarchy_to_canonical_predicates() {
        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];
        builder.add_provider(&MockObserver, &files).await;
        let facts = builder.finish();

        let find = |predicate: &str, object: &str| {
            facts
                .iter()
                .find(|f| {
                    f.predicate.as_str() == predicate
                        && matches!(&f.object, FactValue::Text(t) if t == object)
                })
                .unwrap_or_else(|| panic!("{predicate} → {object} fact missing"))
        };

        // Call → core:calls, no ref detail.
        let calls = find("core:calls", "do_work");
        assert_eq!(calls.provenance.detail, None);
        assert_eq!(calls.provenance.producer, ProducerKind::RuntimeObserver);

        // Import → core:imports, no ref detail.
        let imports = find("core:imports", "do_work");
        assert_eq!(imports.provenance.detail, None);

        // Read/Write/Type → core:references with ref=<Kind> detail. The
        // container "main" resolves against NO context symbol (the file's
        // only symbol is `do_work`), so ALL five reference facts keep the
        // documented non-joinable fallback: the reference site's file.
        let references_facts: Vec<&Fact> = facts
            .iter()
            .filter(|f| f.predicate.as_str() == "core:references")
            .collect();
        assert_eq!(
            references_facts.len(),
            3,
            "Read, Write and Type each map to one core:references fact"
        );
        let details: Vec<Option<&str>> = references_facts
            .iter()
            .map(|f| f.provenance.detail.as_deref())
            .collect();
        assert_eq!(
            details,
            vec![Some("ref=Read"), Some("ref=Type"), Some("ref=Write")],
            "reference kind rides provenance.detail as ref=<Kind>"
        );
        for fact in &references_facts {
            assert_eq!(
                fact.subject,
                EntityId::new(1),
                "unresolvable container and no container both fall back to the \
                 reference site's file path ('src/lib.rs' sorts first)"
            );
        }

        // Hierarchy parents → core:inherits (subject = the queried symbol's
        // 1-based fact-side FQN per the canonical subject grammar, object =
        // the parent name).
        let inherits = find("core:inherits", "BaseWidget");
        assert_eq!(inherits.provenance.producer, ProducerKind::RuntimeObserver);
        assert_eq!(inherits.provenance.detail, None);
        assert_eq!(
            inherits.subject,
            EntityId::new(2),
            "the queried symbol's fact-side FQN 'src/lib.rs:do_work:5' sorts \
             after the file subject"
        );

        // Every fact carries the RuntimeObserver producer (design D1).
        assert!(
            facts
                .iter()
                .all(|f| f.provenance.producer == ProducerKind::RuntimeObserver)
        );
        assert_eq!(
            facts.len(),
            6,
            "1 call + 1 import + 3 references + 1 inherits"
        );
    }

    /// A container that RESOLVES against the extraction context normalizes
    /// to the enclosing symbol's 1-based fact-side FQN (E38.1 CP-3): the
    /// call fact referencing `helper` from inside `main` lands on the SAME
    /// subject as the `main` hierarchy fact (`src/lib.rs:main:5`), proving
    /// the RuntimeObserver observation joined the enclosing symbol.
    #[tokio::test]
    async fn resolvable_container_normalizes_to_enclosing_symbol_fact_side_fqn() {
        struct MainAndHelper;
        #[async_trait]
        impl CodeIntelligenceProvider for MainAndHelper {
            async fn get_symbols(
                &self,
                _path: &Path,
            ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
                let at = |line: u32| Location::new("src/lib.rs", line, 0);
                Ok(vec![
                    Symbol::new("helper", SymbolKind::Function, at(0)),
                    Symbol::new("main", SymbolKind::Function, at(4)),
                ])
            }
            async fn find_references(
                &self,
                location: &Location,
                _include_declaration: bool,
            ) -> Result<Vec<Reference>, CodeIntelligenceError> {
                if location.line() == 0 {
                    return Ok(vec![Reference {
                        location: Location::new("src/lib.rs", 5, 4),
                        reference_kind: ReferenceKind::Call,
                        container: Some("main".to_string()),
                    }]);
                }
                Ok(vec![])
            }
            async fn get_hierarchy(
                &self,
                location: &Location,
            ) -> Result<TypeHierarchy, CodeIntelligenceError> {
                let parents = if location.line() == 4 {
                    vec![TypeHierarchyNode {
                        symbol: Symbol::new(
                            "Base",
                            SymbolKind::Class,
                            Location::new("src/base.rs", 0, 0),
                        ),
                        distance: 1,
                    }]
                } else {
                    vec![]
                };
                Ok(TypeHierarchy {
                    symbol: Symbol::new("queried", SymbolKind::Function, location.clone()),
                    parents,
                    children: vec![],
                })
            }
            async fn get_definition(
                &self,
                _location: &Location,
            ) -> Result<Option<Location>, CodeIntelligenceError> {
                Ok(None)
            }
            async fn get_document_symbols(
                &self,
                _path: &Path,
            ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn hover(
                &self,
                _location: &Location,
            ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
                Ok(None)
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];
        builder.add_provider(&MainAndHelper, &files).await;
        let facts = builder.finish();

        // Both facts share the enclosing symbol's fact-side FQN subject:
        // the container-resolved call fact and the hierarchy fact join.
        let calls = facts
            .iter()
            .find(|f| f.predicate.as_str() == "core:calls")
            .expect("call fact present");
        let inherits = facts
            .iter()
            .find(|f| f.predicate.as_str() == "core:inherits")
            .expect("inherits fact present");
        assert_eq!(
            calls.subject, inherits.subject,
            "container 'main' must normalize to 'src/lib.rs:main:5' — the same \
             subject the hierarchy fact uses for the queried symbol"
        );
    }

    /// Repeated collection from the same provider state yields identical
    /// fact sets (design D3 determinism).
    #[tokio::test]
    async fn repeated_provider_collection_is_identical() {
        let run = || async {
            let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
            let files = vec![PathBuf::from("src/lib.rs")];
            builder.add_provider(&MockObserver, &files).await;
            builder.finish()
        };
        let first = run().await;
        let second = run().await;
        assert!(!first.is_empty());
        assert_eq!(
            first, second,
            "identical provider observations must canonicalize identically"
        );
    }

    /// A provider that fails for a file degrades to no observations for it
    /// (deterministic skip), without failing the batch.
    #[tokio::test]
    async fn provider_errors_degrade_to_no_observations() {
        struct FailingProvider;
        #[async_trait]
        impl CodeIntelligenceProvider for FailingProvider {
            async fn get_symbols(
                &self,
                _path: &Path,
            ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
                Err(CodeIntelligenceError::LspError("unavailable".to_string()))
            }
            async fn find_references(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> Result<Vec<Reference>, CodeIntelligenceError> {
                Err(CodeIntelligenceError::LspError("unavailable".to_string()))
            }
            async fn get_hierarchy(
                &self,
                _location: &Location,
            ) -> Result<TypeHierarchy, CodeIntelligenceError> {
                Err(CodeIntelligenceError::LspError("unavailable".to_string()))
            }
            async fn get_definition(
                &self,
                _location: &Location,
            ) -> Result<Option<Location>, CodeIntelligenceError> {
                Ok(None)
            }
            async fn get_document_symbols(
                &self,
                _path: &Path,
            ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn hover(
                &self,
                _location: &Location,
            ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
                Ok(None)
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/missing.rs")];
        builder.add_provider(&FailingProvider, &files).await;
        assert!(builder.finish().is_empty());
    }
}

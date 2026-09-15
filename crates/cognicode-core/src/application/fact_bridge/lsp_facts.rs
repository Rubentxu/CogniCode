//! Code-intelligence adapter (E37 design D1; LSI M4 design D4/D5).
//!
//! Turns `&dyn TieredCodeIntelligenceProvider` observations for a set of
//! files into canonical relation observations:
//!
//! - all five `ReferenceKind` variants map onto canonical predicates:
//!   `Call` → `core:calls`, `Import` → `core:imports`, and
//!   `Read`/`Write`/`Type` → `core:references` with the kind recorded in
//!   `provenance.detail` as `ref=<Kind>`;
//! - `get_hierarchy` parents → `(type, core:inherits, Text(parent))`.
//!
//! Provenance follows the SERVING TIER of each observation (design D4):
//! the pinned mapping classes S2 LSP observations `Extracted`, S1 local
//! resolver observations `Inferred`, and S0 tree-sitter heuristics
//! `Ambiguous`; the builder appends `tier=<T> provider=<id>` to the detail.
//! A heuristic binding therefore never claims `Extracted`.
//!
//! When every eligible tier of a query is exhausted (design D5), the
//! adapter records an in-memory [`UnresolvedRecord`] naming the site and
//! the exhausted tiers and emits NO fact — no subject, target, or
//! container identity is fabricated.
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

use crate::application::fact_bridge::batch_builder::{FactBatchBuilder, UnresolvedRecord};
use crate::domain::aggregates::Symbol;
use crate::domain::evidence_kernel::fact::ProducerKind;
use crate::domain::evidence_kernel::symbol_fqn::SymbolFqn;
use crate::domain::traits::code_intelligence::{
    PrecisionTier, ProviderDiagnostic, Reference, ReferenceKind, TieredCodeIntelligenceProvider,
    TieredOutcome,
};
use crate::domain::value_objects::SymbolKind;

/// Producer stamped on every provider-derived observation (design D1).
const PRODUCER: ProducerKind = ProducerKind::RuntimeObserver;

/// Query names recorded on [`UnresolvedRecord::query`] (design D5).
const QUERY_GET_SYMBOLS: &str = "get_symbols";
const QUERY_FIND_REFERENCES: &str = "find_references";
const QUERY_GET_HIERARCHY: &str = "get_hierarchy";

/// Canonical provider identity of the tier that served an observation
/// (design D4 detail `provider=<id>`).
///
/// A [`Tiered`](crate::domain::traits::code_intelligence::Tiered) value
/// declares its serving TIER, not its serving provider, so the bridge
/// attributes by tier: S2 → `lsp`, S1 → `local-resolver`, S0 →
/// `tree-sitter` — the stable identities the concrete providers export
/// (`LspIntelligenceProvider::PROVIDER_ID`,
/// `TreesitterFallbackProvider::{PROVIDER_ID, LOCAL_RESOLVER_PROVIDER_ID}`).
/// Reserved tiers have no M4 producer; the mapping stays exhaustive.
fn tier_provider_id(tier: PrecisionTier) -> &'static str {
    match tier {
        PrecisionTier::S2 => "lsp",
        PrecisionTier::S1 => "local-resolver",
        PrecisionTier::S0 => "tree-sitter",
        PrecisionTier::S3 | PrecisionTier::S4 => "reserved",
    }
}

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

/// The queried symbol's 1-based fact-side FQN (canonical subject grammar,
/// E38.1 CP-3): the subject used for hierarchy observations and for the
/// `site` of an exhausted symbol query (design D5).
fn symbol_fact_side_fqn(symbol: &Symbol) -> String {
    SymbolFqn::from_fact_side(
        symbol.location().file(),
        symbol.name(),
        // Declared 0-based → 1-based re-base (the `Symbol` legacy line onto
        // the extractor's fact-side convention).
        symbol.location().line() + 1,
    )
    .assemble()
}

/// Adds every relation observable through `provider` for `files` into
/// `builder`.
///
/// A query whose tiers are all exhausted (design D5) records an in-memory
/// [`UnresolvedRecord`] on the builder and contributes NO fact — provider
/// degradation is never fabricating, so the batch stays honest.
pub async fn collect(
    builder: &mut FactBatchBuilder,
    provider: &dyn TieredCodeIntelligenceProvider,
    files: &[PathBuf],
) {
    // One `get_symbols` round per file (unchanged walk semantics); the
    // symbols are kept so EVERY file's observations can resolve containers
    // against EVERY file's extraction context (references may be sited in a
    // different file than the queried symbol's definition).
    let mut walked: Vec<(&PathBuf, Vec<Symbol>)> = Vec::new();
    for path in files {
        match provider.get_symbols_tiered(path).await {
            TieredOutcome::Served(tiered) => walked.push((path, tiered.value)),
            TieredOutcome::Unresolved(diagnostics) => {
                // Provider degradation: skip the file, record the uncertainty.
                builder.record_unresolved(UnresolvedRecord {
                    site: path.to_string_lossy().into_owned(),
                    query: QUERY_GET_SYMBOLS.to_string(),
                    exhausted_tiers: exhausted_tiers(&diagnostics),
                });
            }
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

/// The attempted tiers named by an exhausted query's diagnostics, in
/// attempt order (design D5).
fn exhausted_tiers(diagnostics: &[ProviderDiagnostic]) -> Vec<PrecisionTier> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.attempted_tier)
        .collect()
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
/// (design D1), attributing every fact to the tier that served the
/// reference query (design D4).
async fn collect_references(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn TieredCodeIntelligenceProvider,
    indexes: &BTreeMap<String, SubjectIndex>,
) {
    let (references, tier) = match provider
        .find_references_tiered(symbol.location(), false)
        .await
    {
        TieredOutcome::Served(tiered) => (tiered.value, tiered.tier),
        TieredOutcome::Unresolved(diagnostics) => {
            // Provider degradation: no reference observations, but the
            // exhausted query is recorded instead of silently dropped.
            builder.record_unresolved(UnresolvedRecord {
                site: symbol_fact_side_fqn(symbol),
                query: QUERY_FIND_REFERENCES.to_string(),
                exhausted_tiers: exhausted_tiers(&diagnostics),
            });
            return;
        }
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
            .add_tiered_observation(
                reference_subject(reference, indexes),
                super::relation(predicate),
                symbol.name(),
                PRODUCER,
                detail,
                tier,
                tier.provenance_class(),
                tier_provider_id(tier),
            )
            .expect("the class is derived from the tier it declares");
    }
}

/// Maps hierarchy parents onto `core:inherits` facts (design D1),
/// attributed to the serving tier (design D4).
///
/// The subject is the QUERIED symbol itself, so it is always resolvable:
/// its 1-based fact-side FQN (canonical subject grammar, E38.1 CP-3),
/// which joins the tree-sitter defines entity of the same symbol.
async fn collect_hierarchy(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn TieredCodeIntelligenceProvider,
) {
    let (hierarchy, tier) = match provider.get_hierarchy_tiered(symbol.location()).await {
        TieredOutcome::Served(tiered) => (tiered.value, tiered.tier),
        TieredOutcome::Unresolved(diagnostics) => {
            // Provider degradation: no hierarchy observations, uncertainty
            // recorded (design D5).
            builder.record_unresolved(UnresolvedRecord {
                site: symbol_fact_side_fqn(symbol),
                query: QUERY_GET_HIERARCHY.to_string(),
                exhausted_tiers: exhausted_tiers(&diagnostics),
            });
            return;
        }
    };
    let subject = symbol_fact_side_fqn(symbol);
    for parent in &hierarchy.parents {
        builder
            .add_tiered_observation(
                subject.clone(),
                super::relation("core:inherits"),
                parent.symbol.name(),
                PRODUCER,
                None,
                tier,
                tier.provenance_class(),
                tier_provider_id(tier),
            )
            .expect("the class is derived from the tier it declares");
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
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::traits::code_intelligence::{
        DocumentSymbol, HoverInfo, ProviderDiagnostic, ProviderOutcome, Reference, TypeHierarchy,
        TypeHierarchyNode,
    };
    use crate::domain::value_objects::{Location, Provenance, SymbolKind};

    use super::*;
    use crate::domain::evidence_kernel::continuity::view::SnapshotEntityView;

    const SNAPSHOT: u64 = 1;

    /// Mock provider serving S0 tree-sitter observations: one function
    /// symbol per file, five references (one per `ReferenceKind`), and one
    /// hierarchy parent.
    struct MockObserver;

    fn fixed_symbol() -> Symbol {
        Symbol::new(
            "do_work",
            SymbolKind::Function,
            Location::new("src/lib.rs", 4, 0),
        )
    }

    #[async_trait]
    impl TieredCodeIntelligenceProvider for MockObserver {
        async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
            TieredOutcome::served(vec![fixed_symbol()], PrecisionTier::S0)
        }

        async fn find_references_tiered(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> TieredOutcome<Vec<Reference>> {
            let at = |file: &str| Location::new(file, 6, 2);
            TieredOutcome::served(
                vec![
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
                ],
                PrecisionTier::S0,
            )
        }

        async fn get_hierarchy_tiered(&self, _location: &Location) -> TieredOutcome<TypeHierarchy> {
            TieredOutcome::served(
                TypeHierarchy {
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
                },
                PrecisionTier::S0,
            )
        }

        async fn get_definition_tiered(
            &self,
            _location: &Location,
        ) -> TieredOutcome<Option<Location>> {
            // The bridge does not consume `get_definition` (design D5).
            TieredOutcome::unresolved(vec![])
        }

        async fn get_document_symbols_tiered(
            &self,
            _path: &Path,
        ) -> TieredOutcome<Vec<DocumentSymbol>> {
            TieredOutcome::served(vec![], PrecisionTier::S0)
        }

        async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
            TieredOutcome::served(None, PrecisionTier::S0)
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

        // Call → core:calls, no ref detail; the S0 tier attestation follows.
        let calls = find("core:calls", "do_work");
        assert_eq!(
            calls.provenance.detail.as_deref(),
            Some("tier=S0 provider=tree-sitter")
        );
        assert_eq!(calls.provenance.producer, ProducerKind::RuntimeObserver);
        assert_eq!(
            calls.provenance.class,
            Provenance::Ambiguous,
            "an S0 tree-sitter binding is never Extracted"
        );

        // Import → core:imports, no ref detail.
        let imports = find("core:imports", "do_work");
        assert_eq!(
            imports.provenance.detail.as_deref(),
            Some("tier=S0 provider=tree-sitter")
        );

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
            vec![
                Some("ref=Read tier=S0 provider=tree-sitter"),
                Some("ref=Type tier=S0 provider=tree-sitter"),
                Some("ref=Write tier=S0 provider=tree-sitter"),
            ],
            "reference kind rides provenance.detail as ref=<Kind>, with the \
             tier attestation appended after it"
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
        assert_eq!(
            inherits.provenance.detail.as_deref(),
            Some("tier=S0 provider=tree-sitter")
        );
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
        // …and every S0 heuristic binding is Ambiguous, NEVER Extracted
        // (spec `provider-tier-provenance`, "Heuristic results never claim
        // Extracted").
        assert!(
            facts
                .iter()
                .all(|f| f.provenance.class == Provenance::Ambiguous),
            "S0 heuristics must never be classed Extracted"
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
        impl TieredCodeIntelligenceProvider for MainAndHelper {
            async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
                let at = |line: u32| Location::new("src/lib.rs", line, 0);
                TieredOutcome::served(
                    vec![
                        Symbol::new("helper", SymbolKind::Function, at(0)),
                        Symbol::new("main", SymbolKind::Function, at(4)),
                    ],
                    PrecisionTier::S0,
                )
            }
            async fn find_references_tiered(
                &self,
                location: &Location,
                _include_declaration: bool,
            ) -> TieredOutcome<Vec<Reference>> {
                if location.line() == 0 {
                    return TieredOutcome::served(
                        vec![Reference {
                            location: Location::new("src/lib.rs", 5, 4),
                            reference_kind: ReferenceKind::Call,
                            container: Some("main".to_string()),
                        }],
                        PrecisionTier::S0,
                    );
                }
                TieredOutcome::served(vec![], PrecisionTier::S0)
            }
            async fn get_hierarchy_tiered(
                &self,
                location: &Location,
            ) -> TieredOutcome<TypeHierarchy> {
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
                TieredOutcome::served(
                    TypeHierarchy {
                        symbol: Symbol::new("queried", SymbolKind::Function, location.clone()),
                        parents,
                        children: vec![],
                    },
                    PrecisionTier::S0,
                )
            }
            async fn get_definition_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<Location>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_document_symbols_tiered(
                &self,
                _path: &Path,
            ) -> TieredOutcome<Vec<DocumentSymbol>> {
                TieredOutcome::served(vec![], PrecisionTier::S0)
            }
            async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
                TieredOutcome::served(None, PrecisionTier::S0)
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

    /// A provider whose every tier is exhausted: each query reports the
    /// attempted tiers' diagnostics and no value.
    struct UnresolvedProvider;

    fn exhausted_diagnostics() -> Vec<ProviderDiagnostic> {
        vec![
            ProviderDiagnostic::new(
                "lsp",
                PrecisionTier::S2,
                ProviderOutcome::Unavailable,
                "server unavailable",
            ),
            ProviderDiagnostic::new(
                "tree-sitter",
                PrecisionTier::S0,
                ProviderOutcome::Error,
                "unreadable file",
            ),
        ]
    }

    #[async_trait]
    impl TieredCodeIntelligenceProvider for UnresolvedProvider {
        async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
            TieredOutcome::unresolved(vec![ProviderDiagnostic::new(
                "lsp",
                PrecisionTier::S2,
                ProviderOutcome::Unavailable,
                "server unavailable",
            )])
        }
        async fn find_references_tiered(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> TieredOutcome<Vec<Reference>> {
            TieredOutcome::unresolved(exhausted_diagnostics())
        }
        async fn get_hierarchy_tiered(&self, _location: &Location) -> TieredOutcome<TypeHierarchy> {
            TieredOutcome::unresolved(exhausted_diagnostics())
        }
        async fn get_definition_tiered(
            &self,
            _location: &Location,
        ) -> TieredOutcome<Option<Location>> {
            TieredOutcome::unresolved(exhausted_diagnostics())
        }
        async fn get_document_symbols_tiered(
            &self,
            _path: &Path,
        ) -> TieredOutcome<Vec<DocumentSymbol>> {
            TieredOutcome::unresolved(exhausted_diagnostics())
        }
        async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
            TieredOutcome::unresolved(exhausted_diagnostics())
        }
    }

    /// Spec scenario "Unresolved site propagates without a fact" (design D5,
    /// task 3.5): a file whose `get_symbols` tiers are all exhausted
    /// contributes NO fact, and the uncertainty names the site (file path)
    /// and the exhausted tiers in attempt order.
    #[tokio::test]
    async fn unresolved_symbol_query_records_site_and_exhausted_tiers_without_a_fact() {
        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/missing.rs")];
        builder.add_provider(&UnresolvedProvider, &files).await;

        let unresolved = builder.take_unresolved();
        assert!(
            builder.finish().is_empty(),
            "an unresolved query must never fabricate a fact"
        );
        assert_eq!(
            unresolved,
            vec![UnresolvedRecord {
                site: "src/missing.rs".to_string(),
                query: "get_symbols".to_string(),
                exhausted_tiers: vec![PrecisionTier::S2],
            }]
        );
    }

    /// The symbol-query variant of the same scenario: `find_references` and
    /// `get_hierarchy` exhaust their tiers for every walked symbol, and each
    /// unresolved record names the queried symbol's 1-based fact-side FQN
    /// as its site — no reference or inherit fact is emitted for them.
    #[tokio::test]
    async fn unresolved_symbol_queries_name_the_symbol_fact_side_fqn() {
        struct SymbolsOnly;
        #[async_trait]
        impl TieredCodeIntelligenceProvider for SymbolsOnly {
            async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
                TieredOutcome::served(vec![fixed_symbol()], PrecisionTier::S0)
            }
            async fn find_references_tiered(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> TieredOutcome<Vec<Reference>> {
                TieredOutcome::unresolved(exhausted_diagnostics())
            }
            async fn get_hierarchy_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<TypeHierarchy> {
                TieredOutcome::unresolved(exhausted_diagnostics())
            }
            async fn get_definition_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<Location>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_document_symbols_tiered(
                &self,
                _path: &Path,
            ) -> TieredOutcome<Vec<DocumentSymbol>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
                TieredOutcome::unresolved(vec![])
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];
        builder.add_provider(&SymbolsOnly, &files).await;

        let unresolved = builder.take_unresolved();
        assert!(
            builder.finish().is_empty(),
            "exhausted symbol queries must not fabricate facts"
        );
        assert_eq!(unresolved.len(), 2, "references + hierarchy both exhaust");
        assert!(unresolved.iter().all(|record| {
            record.site == "src/lib.rs:do_work:5"
                && record.exhausted_tiers == vec![PrecisionTier::S2, PrecisionTier::S0]
        }));
        let queries: Vec<&str> = unresolved
            .iter()
            .map(|record| record.query.as_str())
            .collect();
        assert!(queries.contains(&"find_references"));
        assert!(queries.contains(&"get_hierarchy"));
    }

    /// Spec scenario "Tier decides provenance class" (design D4, task 3.5):
    /// a reference observation SERVED by the S1 local-resolver tier commits
    /// as `Inferred` with the resolver's provider identity in the detail —
    /// the tier, not the adapter, decides the class.
    #[tokio::test]
    async fn tier_decides_provenance_class() {
        struct LocalResolverObserver;
        #[async_trait]
        impl TieredCodeIntelligenceProvider for LocalResolverObserver {
            async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
                TieredOutcome::served(vec![fixed_symbol()], PrecisionTier::S1)
            }
            async fn find_references_tiered(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> TieredOutcome<Vec<Reference>> {
                TieredOutcome::served(
                    vec![Reference {
                        location: Location::new("src/lib.rs", 6, 2),
                        reference_kind: ReferenceKind::Call,
                        container: Some("main".to_string()),
                    }],
                    PrecisionTier::S1,
                )
            }
            async fn get_hierarchy_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<TypeHierarchy> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_definition_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<Location>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_document_symbols_tiered(
                &self,
                _path: &Path,
            ) -> TieredOutcome<Vec<DocumentSymbol>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
                TieredOutcome::unresolved(vec![])
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];
        builder.add_provider(&LocalResolverObserver, &files).await;
        let facts = builder.finish();

        assert_eq!(facts.len(), 1, "one call fact, no hierarchy facts");
        assert_eq!(facts[0].provenance.class, Provenance::Inferred);
        assert_eq!(
            facts[0].provenance.detail.as_deref(),
            Some("tier=S1 provider=local-resolver"),
            "the detail names the serving tier and provider identity"
        );
        assert_ne!(
            facts[0].provenance.class,
            Provenance::Extracted,
            "only the S2 LSP tier produces Extracted facts"
        );
    }

    /// The bridge's tier→provider identity mapping is pinned to the
    /// providers' own stable identity constants (design D4): a rename in
    /// either place must fail this test, never silently drift the detail.
    #[test]
    fn tier_provider_ids_match_the_provider_identity_constants() {
        use crate::infrastructure::lsp::providers::fallback::TreesitterFallbackProvider;
        use crate::infrastructure::lsp::providers::lsp::LspIntelligenceProvider;

        assert_eq!(
            tier_provider_id(PrecisionTier::S2),
            LspIntelligenceProvider::PROVIDER_ID
        );
        assert_eq!(
            tier_provider_id(PrecisionTier::S1),
            TreesitterFallbackProvider::LOCAL_RESOLVER_PROVIDER_ID
        );
        assert_eq!(
            tier_provider_id(PrecisionTier::S0),
            TreesitterFallbackProvider::PROVIDER_ID
        );
    }

    // =========================================================================
    // e42: CP-3 cross-producer edge cases
    // (RETIREMENT-LEDGER: cross-producer join contract)
    // =========================================================================

    /// Edge case: the LSP provider reports a reference whose `container` is
    /// `None` (some LSP servers omit the enclosing-symbol metadata for
    /// module-level references). The bridge MUST fall back to the
    /// reference site's FILE PATH as the subject — never fabricate a fact.
    /// (E38.1 CP-3 deterministic-fallback rule.)
    #[tokio::test]
    async fn cp3_unreported_container_falls_back_to_file_path() {
        struct NoContainerObserver;
        #[async_trait]
        impl TieredCodeIntelligenceProvider for NoContainerObserver {
            async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
                TieredOutcome::served(
                    vec![Symbol::new(
                        "thing",
                        SymbolKind::Function,
                        Location::new("src/lib.rs", 0, 0),
                    )],
                    PrecisionTier::S0,
                )
            }
            async fn find_references_tiered(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> TieredOutcome<Vec<Reference>> {
                TieredOutcome::served(
                    vec![Reference {
                        location: Location::new("src/lib.rs", 10, 4),
                        reference_kind: ReferenceKind::Call,
                        container: None, // <-- no enclosing symbol reported
                    }],
                    PrecisionTier::S0,
                )
            }
            async fn get_hierarchy_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<TypeHierarchy> {
                TieredOutcome::served(
                    TypeHierarchy {
                        symbol: Symbol::new("queried", SymbolKind::Function, _location.clone()),
                        parents: vec![],
                        children: vec![],
                    },
                    PrecisionTier::S0,
                )
            }
            async fn get_definition_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<Location>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_document_symbols_tiered(
                &self,
                _path: &Path,
            ) -> TieredOutcome<Vec<DocumentSymbol>> {
                TieredOutcome::served(vec![], PrecisionTier::S0)
            }
            async fn hover_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<HoverInfo>> {
                TieredOutcome::served(None, PrecisionTier::S0)
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];

        // Pre-declare `thing` as a core:defines entity so the
        // SnapshotEntityView can resolve it. This simulates the
        // DeterministicAnalyzer's contribution to the same snapshot.
        builder
            .add_observation(
                "src/lib.rs:thing:1",
                RelationKind::try_new("core:defines").expect("canonical predicate"),
                "src/lib.rs:thing:1",
                ProducerKind::DeterministicAnalyzer,
                Some("kind=Function".to_string()),
            )
            .expect("deterministic producer is accepted");

        builder.add_provider(&NoContainerObserver, &files).await;
        let facts = builder.finish();

        // Use SnapshotEntityView to verify the call fact's subject is the
        // FILE entity (not a fabricated symbol entity).
        let view = SnapshotEntityView::from_facts(&facts, SnapshotId::new(SNAPSHOT));

        // The call fact MUST exist (reference was served).
        let calls: Vec<&Fact> = facts
            .iter()
            .filter(|f| f.predicate.as_str() == "core:calls")
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "the served reference MUST emit exactly one core:calls fact; got {calls:?}"
        );

        // The `thing` symbol entity MUST exist (it was declared by the
        // pre-declared core:defines) but its `callees` MUST be EMPTY — the
        // call fact was emitted with a different subject (the file path,
        // which is not a core:defines entity; the LSP reference did NOT
        // join `thing`).
        let thing_entity = view
            .entities
            .values()
            .find(|e| e.fqn == "src/lib.rs:thing:1")
            .expect("the `thing` symbol entity must exist");
        assert!(
            thing_entity.callees.is_empty(),
            "with no container reported, the call fact MUST NOT join `thing`; \
             thing.callees was {:?}",
            thing_entity.callees
        );
        // And the call fact's subject EntityId MUST NOT be the thing
        // entity's id (no silent fallback to the only declared symbol).
        assert_ne!(
            calls[0].subject, thing_entity.entity,
            "no-container reference MUST NOT silently join the closest declared symbol; \
             got subject={:?}, thing_entity={:?}",
            calls[0].subject, thing_entity.entity
        );
    }

    /// Edge case: the LSP provider reports a container name that does NOT
    /// match any symbol in the extraction context. The bridge MUST NOT
    /// fabricate an entity; it MUST fall back to the file path.
    /// (E38.1 CP-3: \"no container matches no extraction-context symbol\".)
    #[tokio::test]
    async fn cp3_unresolvable_container_falls_back_to_file_path() {
        struct PhantomContainerObserver;
        #[async_trait]
        impl TieredCodeIntelligenceProvider for PhantomContainerObserver {
            async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
                // Only `real_fn` exists — `phantom_container` will not match.
                TieredOutcome::served(
                    vec![Symbol::new(
                        "real_fn",
                        SymbolKind::Function,
                        Location::new("src/lib.rs", 0, 0),
                    )],
                    PrecisionTier::S0,
                )
            }
            async fn find_references_tiered(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> TieredOutcome<Vec<Reference>> {
                TieredOutcome::served(
                    vec![Reference {
                        location: Location::new("src/lib.rs", 5, 4),
                        reference_kind: ReferenceKind::Call,
                        container: Some("phantom_container".to_string()), // <-- not in index
                    }],
                    PrecisionTier::S0,
                )
            }
            async fn get_hierarchy_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<TypeHierarchy> {
                TieredOutcome::served(
                    TypeHierarchy {
                        symbol: Symbol::new("queried", SymbolKind::Function, _location.clone()),
                        parents: vec![],
                        children: vec![],
                    },
                    PrecisionTier::S0,
                )
            }
            async fn get_definition_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<Location>> {
                TieredOutcome::unresolved(vec![])
            }
            async fn get_document_symbols_tiered(
                &self,
                _path: &Path,
            ) -> TieredOutcome<Vec<DocumentSymbol>> {
                TieredOutcome::served(vec![], PrecisionTier::S0)
            }
            async fn hover_tiered(
                &self,
                _location: &Location,
            ) -> TieredOutcome<Option<HoverInfo>> {
                TieredOutcome::served(None, PrecisionTier::S0)
            }
        }

        let mut builder = FactBatchBuilder::new(SnapshotId::new(SNAPSHOT));
        let files = vec![PathBuf::from("src/lib.rs")];

        // Pre-declare `real_fn` as a core:defines entity (simulates the
        // DeterministicAnalyzer contribution to the same snapshot).
        builder
            .add_observation(
                "src/lib.rs:real_fn:1",
                RelationKind::try_new("core:defines").expect("canonical predicate"),
                "src/lib.rs:real_fn:1",
                ProducerKind::DeterministicAnalyzer,
                Some("kind=Function".to_string()),
            )
            .expect("deterministic producer is accepted");

        builder.add_provider(&PhantomContainerObserver, &files).await;
        let facts = builder.finish();

        // The call fact MUST exist (reference was served).
        let calls: Vec<&Fact> = facts
            .iter()
            .filter(|f| f.predicate.as_str() == "core:calls")
            .collect();
        assert_eq!(calls.len(), 1);

        // The call fact's subject EntityId MUST NOT be the `real_fn`
        // entity's id: an unresolvable container MUST NOT silently join
        // the closest match.
        let view = SnapshotEntityView::from_facts(&facts, SnapshotId::new(SNAPSHOT));
        let real_fn_entity = view
            .entities
            .values()
            .find(|e| e.fqn == "src/lib.rs:real_fn:1")
            .expect("the `real_fn` symbol entity must exist");
        assert_ne!(
            calls[0].subject, real_fn_entity.entity,
            "unresolvable container MUST NOT silently match `real_fn` (CP-3); \
             got subject={:?}, real_fn_entity={:?}",
            calls[0].subject, real_fn_entity.entity
        );
        // `real_fn.callees` MUST be empty (the call did NOT join it).
        assert!(
            real_fn_entity.callees.is_empty(),
            "with an unresolvable container, the call fact MUST NOT join \
             `real_fn`; real_fn.callees was {:?}",
            real_fn_entity.callees
        );

        // Defensive: no entity may carry the `phantom_container` string
        // (confirms no fallback invented an entity).
        for entity in view.entities.values() {
            assert!(
                !entity.fqn.contains("phantom_container") && !entity.name.contains("phantom_container"),
                "no entity may carry the unresolvable container name; \
                 found entity with fqn=`{fqn}`, name=`{name}`",
                fqn = entity.fqn,
                name = entity.name
            );
        }
    }
}

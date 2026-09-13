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
//! The observation subject is the enclosing entity when the provider
//! reports a `container`, else the file containing the reference site; the
//! referenced symbol's name is the object `Text` (cross-entity references
//! stay `Text`, design D3).
//!
//! Producer: `RuntimeObserver` (design D1 authority table).

use std::path::PathBuf;

use crate::application::fact_bridge::batch_builder::FactBatchBuilder;
use crate::domain::aggregates::Symbol;
use crate::domain::evidence_kernel::fact::ProducerKind;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceProvider, Reference, ReferenceKind,
};

/// Producer stamped on every provider-derived observation (design D1).
const PRODUCER: ProducerKind = ProducerKind::RuntimeObserver;

/// Adds every relation observable through `provider` for `files` into
/// `builder`. Provider errors degrade per file/per query (the observation
/// is skipped), which is deterministic for a given provider state.
pub async fn collect(
    builder: &mut FactBatchBuilder,
    provider: &dyn CodeIntelligenceProvider,
    files: &[PathBuf],
) {
    for path in files {
        let symbols = match provider.get_symbols(path).await {
            Ok(symbols) => symbols,
            Err(_) => continue, // provider degradation: skip the file
        };
        for symbol in &symbols {
            collect_references(builder, symbol, provider).await;
            collect_hierarchy(builder, symbol, provider).await;
        }
    }
}

/// Maps all five `ReferenceKind` variants onto canonical predicates
/// (design D1).
async fn collect_references(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn CodeIntelligenceProvider,
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
        let subject = reference
            .container
            .clone()
            .unwrap_or_else(|| reference.location.file().to_string());
        builder
            .add_observation(
                subject,
                super::relation(predicate),
                symbol.name(),
                PRODUCER,
                detail,
            )
            .expect("RuntimeObserver is never rejected");
    }
}

/// Maps hierarchy parents onto `core:inherits` facts (design D1).
async fn collect_hierarchy(
    builder: &mut FactBatchBuilder,
    symbol: &Symbol,
    provider: &dyn CodeIntelligenceProvider,
) {
    let hierarchy = match provider.get_hierarchy(symbol.location()).await {
        Ok(hierarchy) => hierarchy,
        Err(_) => return, // provider degradation: no hierarchy observations
    };
    for parent in &hierarchy.parents {
        builder
            .add_observation(
                symbol.fully_qualified_name(),
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
        // observation subject is the container when the provider reports
        // one, else the file of the reference site.
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
        assert_eq!(
            references_facts[0].subject,
            EntityId::new(1),
            "'main' (container) sorts first among the subjects"
        );
        assert_eq!(
            references_facts[1].subject,
            EntityId::new(2),
            "no container → the reference site's file is the subject"
        );

        // Hierarchy parents → core:inherits (subject = the queried symbol
        // FQN, object = the parent name).
        let inherits = find("core:inherits", "BaseWidget");
        assert_eq!(inherits.provenance.producer, ProducerKind::RuntimeObserver);
        assert_eq!(inherits.provenance.detail, None);
        assert_eq!(inherits.subject, EntityId::new(3), "symbol FQN entity");

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

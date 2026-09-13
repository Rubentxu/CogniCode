//! Canonical fact-batch assembly (E37 design D3) — `FactBatchBuilder`.
//!
//! The builder accumulates raw relation observations from the fact
//! adapters ([`super::tree_sitter_facts`], [`super::lsp_facts`]) and
//! canonicalizes them in [`FactBatchBuilder::finish`]: records sort by
//! `(subject id string, predicate, object)` BEFORE any id exists, subjects
//! map through a snapshot-scoped [`EntityIdTable`] (sorted strings →
//! `EntityId(1..N)`, no hashing), and facts take `FactId(1..M)` in
//! canonical order. Identical inputs therefore produce byte-identical
//! fact sets regardless of adapter walk order (design D3).

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::application::fact_bridge::entity_table::EntityIdTable;
use crate::application::fact_bridge::lsp_facts;
use crate::application::fact_bridge::tree_sitter_facts;
use crate::application::ingest::types::ExtractionResult;
use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
use crate::domain::evidence_kernel::ids::{FactId, SnapshotId};
use crate::domain::evidence_kernel::relation::RelationKind;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::domain::value_objects::Provenance;

use super::FactBridgeError;

/// One relation observation collected before canonical id assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationRecord {
    /// Raw subject id string (legacy FQN for symbols, file path for files).
    subject: String,
    /// Canonical `core:*` predicate.
    predicate: RelationKind,
    /// Raw object text (cross-entity references stay `Text`, design D3).
    object: String,
    /// Free-form provenance detail (e.g. `kind=<SymbolKind>`, `ref=<Kind>`).
    detail: Option<String>,
    /// Which deterministic authority observed the relation (never LLM —
    /// rejected at insertion).
    producer: ProducerKind,
}

/// Stable total order over [`ProducerKind`] (the enum is not `Ord`), used
/// as the final canonical-sort tie-breaker so fact sets stay byte-identical
/// even when two authorities observed the same triple.
fn producer_rank(producer: ProducerKind) -> u8 {
    match producer {
        ProducerKind::DeterministicAdapter => 0,
        ProducerKind::DeterministicAnalyzer => 1,
        ProducerKind::RuntimeObserver => 2,
        ProducerKind::LlmAgent => 3,
        ProducerKind::Human => 4,
    }
}

/// Accumulates relation observations and turns them into one canonical,
/// deterministic fact batch (E37 design D3).
pub struct FactBatchBuilder {
    snapshot: SnapshotId,
    records: Vec<RelationRecord>,
}

impl FactBatchBuilder {
    /// Creates an empty batch for `snapshot`.
    pub fn new(snapshot: SnapshotId) -> Self {
        Self {
            snapshot,
            records: Vec::new(),
        }
    }

    /// Adds every relation derivable from one `extract_file` result
    /// (design D1; producer `DeterministicAnalyzer`). Failed extractions
    /// contribute nothing (error isolation, ADR-023).
    pub fn add_extraction(&mut self, result: &ExtractionResult) {
        tree_sitter_facts::collect(self, result);
    }

    /// Adds every relation observable through a code-intelligence provider
    /// for `files` (design D1; producer `RuntimeObserver`).
    pub async fn add_provider(
        &mut self,
        provider: &dyn CodeIntelligenceProvider,
        files: &[PathBuf],
    ) {
        lsp_facts::collect(self, provider, files).await;
    }

    /// Low-level insertion point for one observation. Returns
    /// [`FactBridgeError::LlmProvenance`] for LLM-agent producers: LLM
    /// output never becomes an extracted fact (design D3 / ADR-040).
    pub fn add_observation(
        &mut self,
        subject: impl Into<String>,
        predicate: RelationKind,
        object: impl Into<String>,
        producer: ProducerKind,
        detail: Option<String>,
    ) -> Result<(), FactBridgeError> {
        if producer == ProducerKind::LlmAgent {
            return Err(FactBridgeError::LlmProvenance);
        }
        self.records.push(RelationRecord {
            subject: subject.into(),
            predicate,
            object: object.into(),
            detail,
            producer,
        });
        Ok(())
    }

    /// Canonicalizes the accumulated observations into `Fact`s (design D3).
    ///
    /// Records sort by `(subject id string, predicate, object)` — with
    /// `(detail, producer)` as deterministic tie-breakers — BEFORE ids
    /// exist. Subject strings then map through a snapshot-scoped
    /// [`EntityIdTable`] and facts take `FactId(1..M)` in canonical order.
    pub fn finish(self) -> Vec<Fact> {
        let snapshot = self.snapshot;
        let mut records = self.records;
        records.sort_by(|a, b| {
            (
                &a.subject,
                a.predicate.as_str(),
                &a.object,
                &a.detail,
                producer_rank(a.producer),
            )
                .cmp(&(
                    &b.subject,
                    b.predicate.as_str(),
                    &b.object,
                    &b.detail,
                    producer_rank(b.producer),
                ))
        });

        let subjects: BTreeSet<&str> = records.iter().map(|r| r.subject.as_str()).collect();
        let table = EntityIdTable::build(snapshot, subjects.into_iter().map(str::to_string));

        records
            .into_iter()
            .enumerate()
            .map(|(i, record)| {
                let subject = table
                    .get(&record.subject)
                    .expect("every record subject was registered in the table");
                Fact::new(
                    FactId::new(i as u64 + 1),
                    subject,
                    record.predicate,
                    FactValue::Text(record.object),
                    snapshot,
                    ProvenanceRecord::new(Provenance::Extracted, record.producer, record.detail),
                )
                .expect("producers were validated at insertion (never LLM)")
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use async_trait::async_trait;

    use crate::application::fact_bridge::relation;
    use crate::application::ingest::extractor::extract_file;
    use crate::domain::aggregates::Symbol;
    use crate::domain::evidence_kernel::continuity::SnapshotEntityView;
    use crate::domain::evidence_kernel::fact::{Fact, FactValue};
    use crate::domain::evidence_kernel::ids::{EntityId, SnapshotId};
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::traits::code_intelligence::{
        CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, HoverInfo, Reference,
        ReferenceKind, TypeHierarchy,
    };
    use crate::domain::value_objects::{Location, SymbolKind};
    use crate::infrastructure::parser::language_config::RUST_CONFIG;

    use super::*;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);

    // -------------------------------------------------------------------------
    // Canonical assembly (design D3)
    // -------------------------------------------------------------------------

    /// `finish()` must sort canonically by (subject, predicate, object)
    /// before assigning ids: `FactId(1..M)` follows the sorted order and
    /// `EntityId(1..N)` follows the sorted subject strings — whatever the
    /// insertion order was.
    #[test]
    fn finish_assigns_canonical_ids_regardless_of_insertion_order() {
        let build = || {
            let mut builder = FactBatchBuilder::new(SNAPSHOT);
            builder
                .add_observation(
                    "z.rs:main:1",
                    relation("core:calls"),
                    "help",
                    ProducerKind::DeterministicAnalyzer,
                    None,
                )
                .expect("deterministic producer");
            builder
                .add_observation(
                    "a.rs",
                    relation("core:contains"),
                    "z.rs:main:1",
                    ProducerKind::DeterministicAnalyzer,
                    None,
                )
                .expect("deterministic producer");
            builder
                .add_observation(
                    "z.rs:main:1",
                    relation("core:defines"),
                    "z.rs:main:1",
                    ProducerKind::DeterministicAnalyzer,
                    Some("kind=Function".to_string()),
                )
                .expect("deterministic producer");
            builder.finish()
        };

        let first = build();
        let second = build();
        assert_eq!(first, second, "same observations must canonicalize equally");
        assert_eq!(first.len(), 3);

        // Sorted subject strings: "a.rs" → 1, "z.rs:main:1" → 2.
        assert_eq!(first[0].id, FactId::new(1));
        assert_eq!(first[0].subject, EntityId::new(1), "a.rs sorts first");
        assert_eq!(
            first[0].predicate,
            RelationKind::try_new("core:contains").expect("valid")
        );
        assert_eq!(first[1].subject, EntityId::new(2));
        assert_eq!(
            first[1].predicate,
            RelationKind::try_new("core:calls").expect("valid")
        );
        assert_eq!(
            first[2].predicate,
            RelationKind::try_new("core:defines").expect("valid")
        );
        for (i, fact) in first.iter().enumerate() {
            assert_eq!(fact.id, FactId::new(i as u64 + 1), "sequential FactIds");
            assert_eq!(fact.snapshot, SNAPSHOT);
        }
    }

    /// The entity table covers record SUBJECTS; cross-entity references in
    /// the object position stay `FactValue::Text` (design D3).
    #[test]
    fn objects_stay_text_and_entities_come_from_subjects() {
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder
            .add_observation(
                "a.rs",
                relation("core:contains"),
                "a.rs:sym:3",
                ProducerKind::DeterministicAnalyzer,
                None,
            )
            .expect("deterministic producer");
        let facts = builder.finish();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].object, FactValue::Text("a.rs:sym:3".to_string()));
        assert!(matches!(facts[0].object, FactValue::Text(_)));
    }

    // -------------------------------------------------------------------------
    // E37 Task 2.5 — LlmAgent rejection at the bridge boundary
    // -------------------------------------------------------------------------

    /// The builder refuses LLM-agent provenance and keeps no trace of the
    /// rejected observation (design D3 / ADR-040).
    #[test]
    fn add_observation_rejects_llm_agent_producer() {
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder
            .add_observation(
                "a.rs",
                relation("core:contains"),
                "a.rs:sym:3",
                ProducerKind::DeterministicAnalyzer,
                None,
            )
            .expect("deterministic producer");

        let err = builder
            .add_observation(
                "a.rs",
                relation("core:calls"),
                "help",
                ProducerKind::LlmAgent,
                None,
            )
            .expect_err("LLM-agent provenance must be rejected");
        assert_eq!(err, FactBridgeError::LlmProvenance);

        let facts = builder.finish();
        assert_eq!(facts.len(), 1, "the rejected observation must not persist");
    }

    // -------------------------------------------------------------------------
    // E37 Task 2.1 RED — spec scenario "Repeated extraction is identical"
    // (specs/projection-equivalence-harness)
    // -------------------------------------------------------------------------

    /// Collects the fixture's Rust sources in sorted order, skipping build
    /// output (`target/`) and hidden directories so the walk is stable.
    fn collect_rust_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if path.is_dir() {
                    if name != "target" && !name.starts_with('.') {
                        stack.push(path);
                    }
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        files.sort();
        files
    }

    /// GIVEN an unchanged golden fixture (`sandbox/fixtures/rust-hello`),
    /// WHEN its extraction adapters run twice, THEN both runs produce
    /// identical fact sets (subject, predicate, object) — and
    /// byte-identical serialized facts, regardless of file walk order.
    #[test]
    fn repeated_extraction_is_identical() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../sandbox/fixtures/rust-hello")
            .canonicalize()
            .expect("fixture root resolves");
        let mut files = collect_rust_files(&fixture);
        assert!(!files.is_empty(), "fixture must contain Rust sources");

        let run = |order: &[PathBuf]| -> Vec<Fact> {
            let mut builder = FactBatchBuilder::new(SNAPSHOT);
            for path in order {
                let source = fs::read_to_string(path).expect("read fixture file");
                let result = extract_file(&RUST_CONFIG, path, &source, "fixture-content");
                builder.add_extraction(&result);
            }
            builder.finish()
        };

        let first = run(&files);
        assert!(!first.is_empty(), "extraction must produce facts");

        // Second run over the SAME fixture in REVERSED walk order: the
        // canonical sort in `finish()` must erase the difference.
        files.reverse();
        let second = run(&files);

        assert_eq!(
            first, second,
            "double-run must yield identical fact sets (subject, predicate, object)"
        );

        // Byte-identical identity (E37 design D3): serialized fact sets match.
        let bytes_first =
            bincode::serde::encode_to_vec(&first, bincode::config::standard()).expect("encode 1");
        let bytes_second =
            bincode::serde::encode_to_vec(&second, bincode::config::standard()).expect("encode 2");
        assert_eq!(
            bytes_first, bytes_second,
            "fact sets must be byte-identical across runs"
        );
    }

    // -------------------------------------------------------------------------
    // E38.1 task 3.1 (CP-3) — dual-producer subject join (RED-first)
    // -------------------------------------------------------------------------

    /// RuntimeObserver stub over `src/join.rs`: reports the same symbols the
    /// tree-sitter extractor derives (0-based `start.row` locations, the
    /// `Symbol` convention) and one Call reference to `unused_fn` sited
    /// inside `main` (container "main"). The source itself never calls
    /// `unused_fn`, so ONLY the RuntimeObserver fact can carry that edge.
    struct JoinObserver;

    #[async_trait]
    impl CodeIntelligenceProvider for JoinObserver {
        async fn get_symbols(&self, _path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            let at = |line: u32| Location::new("src/join.rs", line, 0);
            Ok(vec![
                Symbol::new("helper", SymbolKind::Function, at(0)),
                Symbol::new("unused_fn", SymbolKind::Function, at(4)),
                Symbol::new("main", SymbolKind::Function, at(6)),
            ])
        }

        async fn find_references(
            &self,
            location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            if location.line() == 4 {
                return Ok(vec![Reference {
                    location: Location::new("src/join.rs", 7, 8),
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
            Ok(TypeHierarchy {
                symbol: Symbol::new("queried", SymbolKind::Function, location.clone()),
                parents: vec![],
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

    /// GIVEN one small source file, WHEN the tree-sitter producer (defines
    /// facts) AND the RuntimeObserver producer (reference facts over the
    /// SAME file) feed ONE `FactBatchBuilder` snapshot, THEN every
    /// RuntimeObserver reference fact JOINs the tree-sitter entity of its
    /// enclosing symbol in the [`SnapshotEntityView`] (E38.1 CP-3): the
    /// LSP-only call edge `main → unused_fn` is visible on the `main`
    /// entity. RED-first: with raw container-name subjects the LSP fact
    /// cannot join, so the edge is invisible in the view.
    #[tokio::test]
    async fn lsp_reference_facts_join_tree_sitter_defines_entities() {
        // 0-based tree-sitter rows: helper=0, unused_fn=4, main=6.
        let source = r#"fn helper(x: u32) -> u32 {
    x + 1
}

fn unused_fn() {}

fn main() {
    let v = helper(2);
}
"#;
        let path = Path::new("src/join.rs");

        // Producer 1 (DeterministicAnalyzer): tree-sitter extraction.
        let extraction = extract_file(&RUST_CONFIG, path, source, "hash");
        assert!(
            extraction.error.is_none(),
            "extraction failed: {:?}",
            extraction.error
        );

        // Producer 2 (RuntimeObserver): references over the same file.
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder.add_extraction(&extraction);
        builder
            .add_provider(&JoinObserver, &[path.to_path_buf()])
            .await;
        let facts = builder.finish();

        // The tree-sitter side defines `main` as the 1-based fact-side FQN.
        let defines: Vec<&str> = facts
            .iter()
            .filter(|f| f.predicate.as_str() == "core:defines")
            .filter_map(|f| match &f.object {
                FactValue::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            defines.contains(&"src/join.rs:main:7"),
            "tree-sitter defines facts must use the fact-side grammar: {defines:?}"
        );

        // The view must recover the LSP-ONLY call edge on the `main`
        // entity — proving the RuntimeObserver fact subject joined it.
        let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);
        let main = view
            .entities
            .values()
            .find(|e| e.name == "main")
            .expect("main is a core:defines entity");
        assert!(
            main.callees.contains(&"unused_fn".to_string()),
            "LSP call fact (container 'main') must JOIN the tree-sitter entity \
             src/join.rs:main:7; callees were {:?}",
            main.callees
        );
    }
}

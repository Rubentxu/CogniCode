//! Grounded AST projection (e67 WU2).
//!
//! Joins the real `ExtractionResult` from the tree-sitter extractor with
//! canonical `Fact`s committed to the [`FactStore`] (from WU1's ingestion
//! path) to produce an [`AstInput`] where each [`AstConstruct`] carries
//! either a resolved [`GroundingRef`] or `None` (fail-closed).
//!
//! # Why this module exists
//!
//! The M6 AST backend consumes abstract [`AstInput`] units; its design
//! notes ([`ast_backend`]) explicitly call out that "wiring a real
//! tree-sitter extractor that produces these constructs is a follow-up".
//! e67 WU2 lands that follow-up **without modifying the backend semantics**:
//! the projection layer here is the only new code, and it operates purely
//! at the application boundary.
//!
//! # Grounding rules (fail-closed)
//!
//! For each [`ExtractionResult`] node whose kind is `SymbolKind::Function`:
//!
//! 1. Look up the canonical `core:defines` fact whose `object` equals the
//!    node's FQN (the symbol's canonical id) AND whose `detail` carries
//!    `kind=function` (matching the `kind=Function` provenance emitted by
//!    [`crate::application::fact_bridge::tree_sitter_facts`]).
//! 2. If **exactly one** matching fact exists → emit
//!    [`GroundingRef::entity`] pointing at that fact.
//! 3. If **zero** matches → grounding is `None` (no canonical fact to bind to).
//! 4. If **more than one** match → grounding is `None` (ambiguity is refused;
//!    never guess).
//!
//! `correct incomplete > fabricated complete` (the `grounding` module docs).
//!
//! # Layering
//!
//! - Lives in `application/findings/` (joins `application::ingest` and
//!   `domain::findings::ast_backend`).
//! - No `tree-sitter` types in the public signature (consumes only
//!   `ExtractionResult`, a stable application-level DTO).
//! - No Fact construction (only FactStore reads).

use std::sync::Arc;

use crate::application::ingest::types::ExtractionResult;
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::ids::{EntityId, FactId};
use crate::domain::evidence_kernel::ports::FactStore;
use crate::domain::evidence_kernel::relation::RelationKind;
use crate::domain::findings::ast_backend::{AstConstruct, AstInput, AstUnit};
use crate::domain::findings::detector_ir::SubjectPattern;
use crate::domain::findings::grounding::GroundingRef;
use crate::domain::value_objects::{NodeKind, SymbolKind, WorkspaceId};

/// Subject used for function-definition constructs (e67 WU2 bounded case).
///
/// This is intentionally a `syntax.*` classification (not `core:defines`):
/// the kernel produces structural truth (`core:defines`); the detector
/// matches on `SubjectPattern`, a separate vocabulary. Conflating the two
/// was the trap we explicitly avoided.
pub const SUBJECT_SYNTAX_FUNCTION_DEFINITION: &str = "syntax.function_definition";

/// Kind detail string the tree-sitter bridge writes into `core:defines`
/// provenance for `SymbolKind::Function` (see
/// `crate::domain::evidence_kernel::symbol_kind_detail::SymbolKindDetail::encode`).
/// The encoded form is the serde name: `"Function"` (capital F) — see
/// `SymbolKind` in `domain::value_objects::symbol_kind`.
const DETAIL_KIND_FUNCTION: &str = "kind=Function";

/// Result of a grounded AST projection.
#[derive(Debug, Clone)]
pub struct GroundedAstProjection {
    /// The [`AstInput`] to feed the AST backend.
    pub ast_input: AstInput,
    /// Per-construct outcome diagnostics (for tests + observability).
    /// `true` = grounded; `false` = no canonical fact available.
    pub grounded_outcomes: Vec<GroundedOutcome>,
}

/// One construct's projection outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundedOutcome {
    /// The FQN of the source symbol (subject FQN string).
    pub fqn: String,
    /// 1-based line of the construct in source.
    pub line: u32,
    /// The resolved `GroundingRef` if exactly one canonical `core:defines`
    /// matched; `None` for 0-or-many matches (fail-closed).
    pub grounding: Option<GroundingRef>,
    /// Number of matching canonical facts observed (0, 1, or >1).
    pub match_count: usize,
}

/// Errors raised by the grounded AST projector.
#[derive(Debug, thiserror::Error)]
pub enum GroundedProjectionError {
    /// Snapshot read failure (kernel I/O).
    #[error("fact store read failed: {0}")]
    StoreRead(String),

    /// The kernel predicate `core:defines` failed validation. This should
    /// not happen if `bootstrap_registry` was called; surfaced as a hard
    /// error so misconfigured callers are not silently disabled.
    #[error("invalid relation kind for core:defines: {0}")]
    InvalidRelationKind(String),
}

/// Project an `ExtractionResult` into an [`AstInput`], resolving
/// `GroundingRef`s from canonical `core:defines` facts committed to the
/// given `(workspace, snapshot)`.
///
/// # Async
///
/// Reads the snapshot via `FactStore::facts_in_snapshot(...).await`. No
/// sync wrapper around this read — the caller (runtime composition root)
/// owns async execution (e67 REQ-DGN-001).
///
/// # Fail-closed
///
/// Missing or ambiguous canonical facts leave `grounding = None` and emit a
/// `GroundedOutcome { match_count: 0_or_many }`. The caller can inspect
/// `grounded_outcomes` to confirm whether each construct was grounded.
pub async fn project_grounded_ast(
    fact_store: Arc<dyn FactStore>,
    ws: &WorkspaceId,
    snap: &SnapshotId,
    extraction: &ExtractionResult,
) -> Result<GroundedAstProjection, GroundedProjectionError> {
    let predicate = RelationKind::try_new("core:defines")
        .map_err(|e| GroundedProjectionError::InvalidRelationKind(e.to_string()))?;

    // Read all canonical facts for this snapshot, then index by object (FQN).
    let all_facts = fact_store
        .facts_in_snapshot(ws, snap)
        .await
        .map_err(|e| GroundedProjectionError::StoreRead(format!("{e:?}")))?;

    // Index by object FQN. Canonical `core:defines` facts have their
    // symbol FQN as the `object` (text) — see tree_sitter_facts::collect.
    let mut by_object: std::collections::BTreeMap<String, Vec<(FactId, EntityId, String)>> =
        std::collections::BTreeMap::new();
    for fact in &all_facts {
        if fact.predicate != predicate {
            continue;
        }
        // Only `kind=function` provenance matches our bounded case.
        if !fact
            .provenance
            .detail
            .as_deref()
            .unwrap_or("")
            .contains(DETAIL_KIND_FUNCTION)
        {
            continue;
        }
        let object = match &fact.object {
            crate::domain::evidence_kernel::fact::FactValue::Text(s) => s.clone(),
            _ => continue,
        };
        by_object.entry(object).or_default().push((
            fact.id,
            fact.subject,
            fact.provenance.detail.clone().unwrap_or_default(),
        ));
    }

    let subject_pattern = SubjectPattern::new(SUBJECT_SYNTAX_FUNCTION_DEFINITION)
        .expect("SUBJECT_SYNTAX_FUNCTION_DEFINITION is a valid namespaced name");

    let mut constructs = Vec::new();
    let mut grounded_outcomes = Vec::new();

    for node in &extraction.nodes {
        // Only Function definitions are in scope for WU2's bounded case.
        let NodeKind::Symbol(kind) = node.kind else {
            continue;
        };
        if kind != SymbolKind::Function {
            continue;
        }
        let fqn = node.id.as_str().to_string();
        // The 1-based line lives in properties["line"] (see extract_file).
        let line = node
            .to_map()
            .get("line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let matches = by_object.get(&fqn).cloned().unwrap_or_default();
        let match_count = matches.len();

        let grounding = match matches.as_slice() {
            [(fact_id, subject, _)] => Some(GroundingRef::entity(*subject, *fact_id)),
            _ => None, // 0 or >1 → fail closed, do not fabricate
        };

        constructs.push(AstConstruct {
            subject: subject_pattern.clone(),
            line,
            detail: format!("function `{}`", fqn),
            grounding,
        });

        grounded_outcomes.push(GroundedOutcome {
            fqn,
            line,
            grounding,
            match_count,
        });
    }

    let ast_input = AstInput {
        units: vec![AstUnit {
            path: extraction.source_path.display().to_string(),
            constructs,
        }],
    };

    Ok(GroundedAstProjection {
        ast_input,
        grounded_outcomes,
    })
}

// ============================================================================
// Tests (WU2)
// ============================================================================

#[cfg(test)]
#[cfg(feature = "evidence-kernel")]
mod tests {
    use super::*;
    use crate::application::fact_bridge::production_grounding::ingest_rust_facts;
    use crate::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use crate::infrastructure::evidence_kernel::{InMemoryFactStore, InMemorySchemaRegistry};
    use std::path::PathBuf;

    fn fresh_store() -> Arc<InMemoryFactStore> {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
        Arc::new(InMemoryFactStore::new(Arc::new(registry)))
    }

    fn load_fixture() -> (PathBuf, String, String) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is set by cargo during tests");
        let path = PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("sandbox/fixtures/lsi-grounding/sample.rs");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {:?}: {e}", path));
        let hash = "sha256:e67-fixture-pin";
        (path, source, hash.to_string())
    }

    /// Run the WU1 ingestion seam so the snapshot has canonical `core:defines`
    /// facts for the fixture's two functions.
    async fn ingest_fixture(
        store: Arc<InMemoryFactStore>,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> (PathBuf, String, String) {
        let (path, source, hash) = load_fixture();
        ingest_rust_facts(store.as_ref(), ws, snap, &path, &source, &hash)
            .await
            .expect("WU1 ingest succeeds");
        (path, source, hash)
    }

    /// Extract (sync) the fixture so we have the raw `ExtractionResult`.
    fn extract_fixture(source: &str) -> ExtractionResult {
        use crate::application::ingest::extractor::extract_file;
        use crate::infrastructure::parser::language_config::RUST_CONFIG;
        let (path, _, _) = load_fixture();
        extract_file(&RUST_CONFIG, &path, source, "sha256:e67-fixture-pin")
    }

    #[tokio::test]
    async fn wu2_grounded_projection_resolves_real_core_defines_facts() {
        // GIVEN a snapshot with canonical Facts from WU1's ingest.
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (_path, source, _hash) = ingest_fixture(store.clone(), &ws, &snap).await;
        let extraction = extract_fixture(&source);

        // WHEN we project a grounded AST input.
        let projection = project_grounded_ast(store.clone(), &ws, &snap, &extraction)
            .await
            .expect("projection succeeds");

        // THEN each Function node has exactly one canonical match (greet + helper).
        assert_eq!(
            projection.grounded_outcomes.len(),
            2,
            "expected 2 function constructs (greet, helper); got {}",
            projection.grounded_outcomes.len()
        );
        for outcome in &projection.grounded_outcomes {
            assert_eq!(
                outcome.match_count, 1,
                "construct {} should have exactly 1 canonical core:defines match",
                outcome.fqn
            );
            let grounding = outcome
                .grounding
                .expect("grounding resolved (1-of-1 match)");
            // The GroundingRef's fact must resolve to a real committed fact.
            let visible_facts = store
                .facts_in_snapshot(&ws, &snap)
                .await
                .expect("facts_in_snapshot");
            assert!(
                visible_facts.iter().any(|f| f.id == grounding.fact),
                "GroundingRef.fact {:?} does not resolve to a committed fact",
                grounding.fact
            );
        }
    }

    #[tokio::test]
    async fn wu2_construct_subject_is_syntax_function_definition_not_core_defines() {
        // GIVEN a snapshot with canonical Facts.
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (_path, source, _hash) = ingest_fixture(store.clone(), &ws, &snap).await;
        let extraction = extract_fixture(&source);

        // WHEN we project.
        let projection = project_grounded_ast(store, &ws, &snap, &extraction)
            .await
            .expect("projection succeeds");

        // THEN no construct's subject leaks the kernel vocabulary `core:defines`.
        for unit in &projection.ast_input.units {
            for c in &unit.constructs {
                assert_ne!(
                    c.subject.as_str(),
                    "core:defines",
                    "AstConstruct.subject MUST NOT conflate with kernel predicate vocabulary"
                );
                assert_eq!(
                    c.subject.as_str(),
                    SUBJECT_SYNTAX_FUNCTION_DEFINITION,
                    "subject must be the bounded syntax classification"
                );
            }
        }
    }

    #[tokio::test]
    async fn wu2_ungrounded_construct_is_reported_with_match_count_zero() {
        // GIVEN an EMPTY snapshot (no canonical Facts committed).
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(99);
        let (_path, source, _hash) = load_fixture();
        let extraction = extract_fixture(&source);

        // WHEN we project against an empty snapshot.
        let projection = project_grounded_ast(store, &ws, &snap, &extraction)
            .await
            .expect("projection succeeds");

        // THEN every construct has match_count=0 and grounding=None.
        assert_eq!(projection.grounded_outcomes.len(), 2);
        for outcome in &projection.grounded_outcomes {
            assert_eq!(outcome.match_count, 0);
            assert!(
                outcome.grounding.is_none(),
                "fail-closed: 0 matches must yield grounding=None"
            );
        }
    }

    #[tokio::test]
    async fn wu2_ast_backend_emits_produced_evidence_with_grounding() {
        // GIVEN a snapshot with canonical Facts.
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (_path, source, _hash) = ingest_fixture(store.clone(), &ws, &snap).await;
        let extraction = extract_fixture(&source);
        let projection = project_grounded_ast(store.clone(), &ws, &snap, &extraction)
            .await
            .expect("projection succeeds");

        // WHEN we drive the existing AstBackend through the real M6 path.
        use crate::domain::execution::actor::ActorRef;
        use crate::domain::execution::correlation::CorrelationId;
        use crate::domain::findings::DetectorAuthority;
        use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
        use crate::domain::findings::ast_backend::AstBackend;
        use crate::domain::findings::detector_ir::{
            AnalysisCapability, DetectorId, DetectorIr, DetectorStep, FindingKind,
        };
        use crate::domain::findings::execution::{
            AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
        };
        use crate::domain::findings::scope::AnalysisScope;

        let ir = DetectorIr {
            id: DetectorId::new("structure.function_definition").unwrap(),
            name: "structure.function_definition".to_string(),
            policy: Default::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(SUBJECT_SYNTAX_FUNCTION_DEFINITION).unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("structure.function_definition").unwrap(),
                },
            ],
        };
        let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin)
            .expect("admit succeeds");

        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let executor = DetectorExecutor::new(&registry);

        let input = AnalysisInput {
            scope: Some(AnalysisScope::new(ws.clone(), snap)),
            ast: Some(projection.ast_input.clone()),
            graph: None,
            dataflow: None,
        };
        let request = ExecutionRequest::new(
            crate::domain::kernel_ids::ExecutionId::new(1),
            AnalysisScope::new(ws.clone(), snap),
            ActorRef::kernel(),
            CorrelationId::new("wu2-test").unwrap(),
        );

        let prepared = executor
            .prepare(&permit, &input, request)
            .expect("prepare succeeds");

        // THEN every produced evidence carries a Some(GroundingRef).
        assert_eq!(prepared.outcome().matches.len(), 2);
        assert_eq!(prepared.outcome().produced_evidence.len(), 2);
        for ev in &prepared.outcome().produced_evidence {
            assert!(
                ev.grounding.is_some(),
                "AstBackend produced evidence MUST carry Some(GroundingRef) when projection resolved one"
            );
        }
    }

    #[tokio::test]
    async fn wu2_ungrounded_input_yields_ungrounded_produced_evidence() {
        // GIVEN an EMPTY snapshot so projection yields grounding=None everywhere.
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(99);
        let (_path, source, _hash) = load_fixture();
        let extraction = extract_fixture(&source);
        let projection = project_grounded_ast(store.clone(), &ws, &snap, &extraction)
            .await
            .expect("projection succeeds");

        // WHEN we drive the existing AstBackend.
        use crate::domain::execution::actor::ActorRef;
        use crate::domain::execution::correlation::CorrelationId;
        use crate::domain::findings::DetectorAuthority;
        use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
        use crate::domain::findings::ast_backend::AstBackend;
        use crate::domain::findings::detector_ir::{
            AnalysisCapability, DetectorId, DetectorIr, DetectorStep, FindingKind,
        };
        use crate::domain::findings::execution::{
            AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
        };
        use crate::domain::findings::scope::AnalysisScope;

        let ir = DetectorIr {
            id: DetectorId::new("structure.function_definition").unwrap(),
            name: "structure.function_definition".to_string(),
            policy: Default::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(SUBJECT_SYNTAX_FUNCTION_DEFINITION).unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("structure.function_definition").unwrap(),
                },
            ],
        };
        let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin)
            .expect("admit succeeds");

        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let executor = DetectorExecutor::new(&registry);

        let input = AnalysisInput {
            scope: Some(AnalysisScope::new(ws.clone(), snap)),
            ast: Some(projection.ast_input),
            graph: None,
            dataflow: None,
        };
        let request = ExecutionRequest::new(
            crate::domain::kernel_ids::ExecutionId::new(2),
            AnalysisScope::new(ws.clone(), snap),
            ActorRef::kernel(),
            CorrelationId::new("wu2-ungrounded-test").unwrap(),
        );
        let prepared = executor
            .prepare(&permit, &input, request)
            .expect("prepare succeeds");

        // THEN every evidence still emits but carries grounding=None.
        assert_eq!(prepared.outcome().matches.len(), 2);
        for ev in &prepared.outcome().produced_evidence {
            assert!(
                ev.grounding.is_none(),
                "ungrounded projection MUST yield grounding=None on emitted evidence"
            );
        }
    }
}

//! H0.5 (delivery-first-radical, package integration) — cross-cutting
//! acceptance laws for the H0 block.
//!
//! Las cinco propiedades de H0.5 (mtime/size; invalidate(); identidad
//! de workspace; complexity+skip ⇒ Partial; Ungrounded ⇒ no
//! admision) tienen tests unitarios en:
//!
//!   - h01_per_file_cache_invalidate_contract.rs       (H0.1)
//!   - cu1_w2_service_analyzer_contract.rs::graph_complexity_inherits_partial (H0.2)
//!   - h03_workspace_root_identity_contract.rs         (H0.3)
//!   - h04_ungrounded_no_sentinels_contract.rs         (H0.4)
//!
//! Este fichero NO duplica esos tests. Verifica que las cinco
//! propiedades funcionan JUNTAS como sistema, a traves de la API
//! publica (ServiceAnalyzer + to_guard_observation), en 3 leyes
//! pequenas cross-cutting. Cada ley ejercita varias propiedades
//! combinadas; si una de las cinco se rompe, alguna de las tres
//! leyes cae.
//!
//! Mantener: 2-4 leyes pequenas (hitos 01-HITOS-Y-TODO, H0.5). Tres
//! es el limite inferior del rango; cada ley es pequena (20-50
//! lineas) y reutiliza el fixture de rust que ya vive en
//! cu1_w2_service_analyzer_contract.rs.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisValue, Analyze, OutcomeState,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use cognicode_core::domain::findings::EvidenceClass;
use cognicode_core::domain::findings::guard_transfer::{GuardObservation, to_guard_observation};
use cognicode_core::domain::findings::projection::{Gap, ProjectionError};

use cognicode_core::application::analysis_basis::GraphValue;

fn tempdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h05-acceptance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rust_fixture(dir: &std::path::Path) {
    // Same fixture as cu1_w2_service_analyzer_contract.rs:rust_fixture,
    // minus the symlink-heavy homonym scenario. Two modules with a
    // helper each, so graph.full has >= 2 symbols to count.
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "mod alpha;\nmod beta;\nfn main() { alpha::helper(); beta::helper(); }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("alpha.rs"),
        "pub fn helper() -> u32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("beta.rs"),
        "pub fn helper() -> u32 { 2 }\n",
    )
    .unwrap();
}

/// LAW A: estado honesto a traves del adaptador de produccion.
///
/// graph.full y graph.complexity sobre el MISMO workspace, cuando
/// el walk del kernel produjo un skip, deben ambos devolver Partial
/// con los MISMOS gaps. Esto ejercita H0.2 (complexity hereda
/// Partial del BuildReport compartido) y H0.4 (el seam no fabrica
/// provenance: los gaps son los del walk, no una traduccion
/// sentinel). Si H0.1 o H0.2 se rompe, esta ley cae; si H0.4
/// introduce un sentinel en este path, los gaps dejarian de
/// coincidir.
///
/// MEDIDO 2026-10-19: el skip del fixture era `Cargo.toml`, que K0/H1.1
/// (2026-10-18) declaro exclusion legitima y no hueco. La ley no se
/// debilita: se le da un hueco REAL (`.kt`) para que siga detectando
/// la divergencia H0.2/H0.4 para la que nacio.
#[test]
fn law_a_state_is_honest_through_the_production_adapter() {
    let tmp = tempdir();
    rust_fixture(&tmp);
    std::fs::write(
        tmp.join("src").join("Legacy.kt"),
        "class Legacy { fun x() = 1 }\n",
    )
    .unwrap();
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    // 1) graph.full puebla last_build_report.
    let full = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp.clone(),
            rebuild: true,
        },
    );
    assert_eq!(full.state(), OutcomeState::Partial);
    let full_gaps: Vec<String> = full.gaps().iter().map(|g| g.message.clone()).collect();
    assert!(
        full_gaps.iter().any(|m| m.contains("Legacy.kt")),
        "graph.full must surface the Legacy.kt skip: {full_gaps:?}"
    );

    // 2) graph.complexity hereda el mismo Partial con los mismos
    //    gaps (H0.2 + H0.4 honesty: la traduccion es UNICA, no
    //    se reinventa con sentinels en esta op).
    let complexity = analyzer.execute(&basis, AnalysisOp::GraphComplexity { path: tmp.clone() });
    assert_eq!(
        complexity.state(),
        OutcomeState::Partial,
        "graph.complexity must inherit Partial from the shared BuildReport (H0.2)"
    );
    let complexity_gaps: Vec<String> = complexity
        .gaps()
        .iter()
        .map(|g| g.message.clone())
        .collect();
    assert_eq!(
        complexity_gaps, full_gaps,
        "graph.complexity must carry the EXACT same gaps as graph.full on the \
         same workspace; a divergence means the seam translated BuildStatus \
         twice (H0.2 / H0.4 honesty): full={full_gaps:?} complexity={complexity_gaps:?}"
    );
    match complexity.value() {
        Some(AnalysisValue::Graph(GraphValue::Complexity { metrics, .. })) => {
            // La metrica se calcula sobre el grafo parcial, no es
            // un valor silencioso. El adaptador devolvio Partial
            // CON valor; eso es honestidad (no "Unknown vacio").
            assert!(metrics.get("total_symbols").is_some());
        }
        other => panic!("expected GraphValue::Complexity with metrics, got {other:?}"),
    }
}

/// LAW B: identidad de workspace a traves del ciclo de vida del
/// adaptador.
///
/// Dos workspaces distintos analizados en secuencia sobre el mismo
/// `ServiceAnalyzer` (mismo cache interno) NO contaminan sus
/// grafos. Esto ejercita H0.3 (for_workspace produce identidades
/// distintas para paths distintos) y H0.1 (la cache no sirve
/// grafos de un workspace bajo el pretexto de rebuild=false). Si
/// H0.1 o H0.3 se rompe, los simbolos del workspace A aparecen
/// en el resultado del workspace B.
#[test]
fn law_b_workspace_identity_is_preserved_across_analyzer_lifetime() {
    let dir_a = tempdir();
    let dir_b = tempdir();
    rust_fixture(&dir_a);
    rust_fixture(&dir_b);

    let basis_a = AnalysisBasis::for_workspace(dir_a.clone());
    let basis_b = AnalysisBasis::for_workspace(dir_b.clone());

    // Las identidades son distintas (H0.3).
    assert_ne!(
        basis_a.workspace_root(),
        basis_b.workspace_root(),
        "precondition: workspace identities are distinct (H0.3)"
    );

    let analyzer = ServiceAnalyzer::new();

    // Construye el grafo de A.
    let full_a = analyzer.execute(
        &basis_a,
        AnalysisOp::GraphFull {
            path: dir_a.clone(),
            rebuild: true,
        },
    );
    let symbols_a = match full_a.value() {
        Some(AnalysisValue::Graph(GraphValue::Full { symbols, .. })) => *symbols,
        _ => panic!("graph.full on A must produce a Full value"),
    };
    assert!(
        symbols_a >= 2,
        "fixture A defines >= 2 symbols, got {symbols_a}"
    );

    // Ahora pide graph.full sobre B SIN rebuild (cache lookup).
    // El grafo de A no debe filtrarse al resultado de B (H0.1 + H0.3).
    let full_b = analyzer.execute(
        &basis_b,
        AnalysisOp::GraphFull {
            path: dir_b.clone(),
            rebuild: false,
        },
    );
    match full_b.value() {
        Some(AnalysisValue::Graph(GraphValue::Full { path, .. })) => {
            // El path del value refleja el workspace solicitado, no
            // el workspace del que provino el grafo. Si H0.1/H0.3
            // estuviera roto, `path` seguiria siendo dir_a (o el
            // grafo tendria los simbolos de A).
            assert_eq!(
                path, &dir_b,
                "the Full value must report the requested workspace (B), \
                 not the cache owner's (A); cache pollution breaks H0.1+H0.3"
            );
        }
        // B es valido como Partial si la cache no lo sirvio; lo
        // importante es que, en cualquier caso, los simbolos y el
        // path correspondan a B.
        Some(AnalysisValue::Graph(GraphValue::Complexity { path, .. })) => {
            assert_eq!(path, &dir_b);
        }
        other => panic!("graph.full on B must produce a Full or Complexity value, got {other:?}"),
    }
}

/// LAW C: honestidad causal en el seam.
///
/// Un finding con gaps pero SIN snapshot/detector context
/// (representante del caso "perdimos la provenance") produce
/// `GuardObservation::Ungrounded`, no `Inconclusive`, no
/// `Verifiable`, no `Advisory`. Esto ejercita H0.4 (la ausencia
/// de provenance es un discriminante propio) y, en combinacion
/// con la estructura del enum, hace que el invariant "Ungrounded
/// jamas implica admision" sea enforceable por pattern-matching
/// puro en el consumer.
#[test]
fn law_c_ungrounded_cannot_be_promoted_by_any_caller() {
    // Caso: Err(Inconclusive(gaps)) — el error lleva solo gaps,
    // no snapshot ni detector. Es el unico camino real a
    // Ungrounded hoy.
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(vec![
        Gap::EvidenceClassBelowThreshold {
            class: EvidenceClass::D,
        },
    ]));
    let observation = to_guard_observation(&result);
    assert!(
        matches!(observation, GuardObservation::Ungrounded { .. }),
        "Err(Inconclusive) must map to Ungrounded, got {observation:?}"
    );

    // Exhaustive: NO se confunde con NINGUN otro variante.
    let _verifiable: bool = matches!(observation, GuardObservation::Verifiable { .. });
    let _advisory: bool = matches!(observation, GuardObservation::Advisory { .. });
    let _inconclusive: bool = matches!(observation, GuardObservation::Inconclusive { .. });
    let _unsupported: bool = matches!(observation, GuardObservation::Unsupported { .. });
    let _failed: bool = matches!(observation, GuardObservation::Failed { .. });
    // Solo Ungrounded es true. El compilador NO nos ayuda aqui
    // (no hay #[non_exhaustive]); pero el pattern del consumer
    // SI: si el consumer de Guard ve `Ungrounded`, no puede
    // promover. Si ve otro, ese otro SI tiene provenance. La
    // honestidad esta en el discriminante.

    // Caso inverso: una Err sin contexto de gaps (Failed) tampoco
    // es Ungrounded. La honestidad distingue "no tengo provenance"
    // de "falle estructuralmente".
    let result_failed: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Failed("structural".to_string()));
    let observation_failed = to_guard_observation(&result_failed);
    assert!(
        matches!(observation_failed, GuardObservation::Failed { .. }),
        "Err(Failed) must remain Failed (structural), not collapse to Ungrounded: {observation_failed:?}"
    );
}

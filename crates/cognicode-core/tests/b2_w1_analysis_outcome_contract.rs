//! B2.W1 — Contract for `AnalysisOutcome<T>` and `AnalysisBasis`.
//!
//! Estado pre-B2.W1:
//!   - El CLI devuelve `Result<CommandOutput, _>` con `IndexResult` o
//!     `GraphResult` tipados, pero el resto de sub-comandos cae en
//!     `CommandOutput::Other` (sin tipar).
//!   - El MCP invoca handlers directamente (`handle_build_graph`,
//!     `handle_get_file_symbols`, ...) sin un modelo común que
//!     distinga `Complete|Partial|Unsupported|Unknown|Failed`.
//!   - `CapabilityCompleteness` en `assurance_export/envelope.rs`
//!     tiene el vocabulario (4 variantes: Complete / Partial /
//!     Unknown / Unsupported) pero solo en el envelope de export.
//!
//! Lo que cierra este test:
//!   - `AnalysisOutcome<T>` reusa el vocabulario de
//!     `CapabilityCompleteness` y lo eleva a contrato de
//!     aplicación (no solo del envelope de export).
//!   - `OutcomeState` añade la variante `Failed` que el envelope
//!     assurance-evidence/v1 no tiene (porque un envelope con
//!     `Failed` no se serializa, queda en error path).
//!   - Los constructores pinean invariantes: `Partial` requiere
//!     gaps no vacíos, `Unsupported` requiere reason no vacío,
//!     `Failed` requiere error no vacío. Igual que
//!     `CapabilityCompleteness::partial` y `::unsupported`.
//!   - `AnalysisBasis` canonicaliza el path de workspace (no
//!     acepta `..` ni `~/work` sin canonicalizar).
//!   - El JSON shape es estable: `state` es discriminante, `value`
//!     solo aparece en Complete/Partial, `gaps`/`reason`/`error`
//!     solo en Partial/Unsupported/Failed respectivamente.
//!
//! Política B2.W1: el contrato vive en `application::analysis_basis`
//! y NO se reexporta fuera de `application` excepto en `interface::cli`
//! y `interface::mcp` (que son los dos adapters que lo consumen).
//! El dominio no depende de él; el envelope de assurance-evidence/v1
//! tampoco — su `CapabilityCompleteness` se queda como tipo aparte
//! porque la semántica del envelope difiere (un envelope con gaps es
//! serializable; un `AnalysisOutcome::Partial` también, pero un
//! `AnalysisOutcome::Failed` no).

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisError, AnalysisOutcome, AnalysisProvenance, OutcomeState,
};
use serde_json::Value;
use std::path::PathBuf;

/// OutcomeState es discriminante: 5 variantes. Si una nueva
/// semántica de resultado aparece (e.g. `Cancelled`, `Throttled`)
/// se añade con un test que la pinee; mientras tanto, este assert
/// protege contra la perdida silenciosa de variantes en el JSON.
#[test]
fn outcome_state_serializes_as_discriminator() {
    let cases = [
        (OutcomeState::Complete, "complete"),
        (OutcomeState::Partial, "partial"),
        (OutcomeState::Unknown, "unknown"),
        (OutcomeState::Unsupported, "unsupported"),
        (OutcomeState::Failed, "failed"),
    ];
    let mut seen = std::collections::HashSet::new();
    for (state, expected) in cases {
        let json = serde_json::to_string(&state).expect("OutcomeState serializa");
        let parsed: Value = serde_json::from_str(&json).expect("OutcomeState JSON es valido");
        let actual = parsed
            .as_str()
            .unwrap_or_else(|| panic!("OutcomeState no serializa como string: {json}"));
        assert_eq!(
            actual, expected,
            "discriminante drift: {actual:?} vs {expected:?}"
        );
        assert!(
            seen.insert(actual.to_string()),
            "dos OutcomeState colapsan al mismo discriminante: {actual}"
        );
    }
    assert_eq!(seen.len(), 5, "se esperan exactamente 5 variantes");
}

#[test]
fn complete_outcome_carries_value_without_gaps() {
    let outcome: AnalysisOutcome<u32> = AnalysisOutcome::complete(42);
    assert_eq!(outcome.state(), OutcomeState::Complete);
    assert_eq!(outcome.value(), Some(&42));
    assert!(outcome.gaps().is_empty());
    assert!(outcome.is_producing());
}

#[test]
fn partial_outcome_carries_value_and_non_empty_gaps() {
    let outcome: AnalysisOutcome<u32> = AnalysisOutcome::partial(
        42,
        vec![AnalysisError::gap(
            "relations",
            "Tarjan SCC no se calcula en v1",
        )],
    );
    assert_eq!(outcome.state(), OutcomeState::Partial);
    assert_eq!(outcome.value(), Some(&42));
    assert_eq!(outcome.gaps().len(), 1);
    assert!(outcome.is_producing());
}

#[test]
#[should_panic(expected = "Partial expone al menos un gap")]
fn partial_rejects_empty_gaps() {
    // Pineamos el mismo invariante que `CapabilityCompleteness::partial`:
    // un Partial sin gaps es un Complete mal nombrado.
    let _: AnalysisOutcome<u32> = AnalysisOutcome::partial(42, vec![]);
}

#[test]
#[should_panic(expected = "Unsupported requiere reason no vacio")]
fn unsupported_rejects_blank_reason() {
    let _: AnalysisOutcome<u32> = AnalysisOutcome::unsupported("   ");
}

#[test]
fn unsupported_outcome_carries_reason_no_value() {
    let outcome: AnalysisOutcome<u32> = AnalysisOutcome::unsupported("no calculado en v1");
    assert_eq!(outcome.state(), OutcomeState::Unsupported);
    assert!(outcome.value().is_none());
    assert!(!outcome.is_producing());
    assert_eq!(outcome.reason(), Some("no calculado en v1"));
}

#[test]
fn unknown_outcome_has_no_value_no_gaps() {
    let outcome: AnalysisOutcome<u32> = AnalysisOutcome::unknown();
    assert_eq!(outcome.state(), OutcomeState::Unknown);
    assert!(outcome.value().is_none());
    assert!(outcome.gaps().is_empty());
    assert!(!outcome.is_producing());
}

#[test]
fn failed_outcome_carries_error_no_value() {
    let outcome: AnalysisOutcome<u32> =
        AnalysisOutcome::failed(AnalysisError::io_failure("no se pudo leer el snapshot"));
    assert_eq!(outcome.state(), OutcomeState::Failed);
    assert!(outcome.value().is_none());
    assert!(outcome.error().is_some());
    assert!(!outcome.is_producing());
}

#[test]
#[should_panic(expected = "Failed requiere error no vacio")]
fn failed_rejects_blank_error() {
    let _: AnalysisOutcome<u32> = AnalysisOutcome::failed(AnalysisError::io_failure("   "));
}

#[test]
fn analysis_basis_canonicalizes_workspace_root() {
    // El root debe canonicalizarse: `..` se resuelve en el path
    // real, el resultado es absoluto, y queda libre de `..` como
    // componente. Usamos un path que existe en disco para que
    // `canonicalize` (que sigue symlinks Y resuelve `..`) pueda
    // actuar; un path inexistente se devuelve literal y no se
    // normaliza, eso es la semantica de std::fs::canonicalize.
    //
    // RED-PROBE(cognicode-cd5): `raw` era un path RELATIVO, asi que su
    // resolucion dependia del cwd del proceso de test (cargo usa
    // CARGO_MANIFEST_DIR para los integration tests, pero no es un
    // contrato del harness). Lo hacemos explicito y absoluto para que
    // el expected no dependa de donde se ejecute.
    let cli_dir =
        std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cognicode-cli"))
            .expect("el directorio de prueba debe existir en disco");
    // `raw` apunta a `cognicode-cli` CON un `..` extra que canonicalize
    // debe resolver de vuelta al mismo directorio: asi el `..` se
    // ejercita de verdad y el resultado esperado sigue siendo `cli_dir`.
    let raw = cli_dir.join("../cognicode-cli");
    let basis = AnalysisBasis::for_workspace(raw.clone());
    assert!(
        basis.workspace_root().is_absolute(),
        "root no canonicalizado: {:?}",
        basis.workspace_root()
    );
    assert!(
        !basis.workspace_root().to_string_lossy().contains(".."),
        "root contiene '..': {:?}",
        basis.workspace_root()
    );
    // RED-PROBE(cognicode-cd5): las dos aserciones anteriores las
    // cumple CUALQUIER ruta absoluta, incluida una literal inventada
    // (`Path::new("/tmp/...")` hacia `ok` con el producto roto). Fijamos
    // la identidad: el root debe ser EXACTAMENTE el directorio pedido,
    // no una ruta cualquiera con forma correcta.
    assert_eq!(
        basis.workspace_root(),
        cli_dir,
        "workspace_root() devolvio {:?} en lugar del directorio solicitado {:?}",
        basis.workspace_root(),
        cli_dir
    );
}

#[test]
fn provenance_carries_workspace_operation_elapsed() {
    let provenance =
        AnalysisProvenance::new(PathBuf::from("/work/repo"), "index.build", "entities")
            .with_elapsed_ms(123);
    assert_eq!(provenance.workspace, Some(PathBuf::from("/work/repo")));
    assert_eq!(provenance.operation.as_deref(), Some("index.build"));
    assert_eq!(provenance.capability.as_deref(), Some("entities"));
    assert_eq!(provenance.elapsed_ms, 123);
}

#[test]
fn outcome_serializes_to_expected_json_shape() {
    // El bin CLI y el MCP server emiten este shape por stdout/JSON-RPC.
    // Si cambia, el `tools/list` consumer (MCP) y los scripts de
    // usuario (CLI) rompen en silencio — pineamos el contrato.
    let outcome: AnalysisOutcome<u32> =
        AnalysisOutcome::partial(42, vec![AnalysisError::gap("x", "y")]);
    let json: Value = serde_json::to_value(&outcome).expect("serialize");
    assert_eq!(json["state"], "partial");
    assert_eq!(json["value"], 42);
    assert_eq!(json["gaps"][0]["capability"], "x");
    assert_eq!(json["gaps"][0]["message"], "y");
}

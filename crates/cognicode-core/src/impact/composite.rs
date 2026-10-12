//! B4.W5 — Composite impact query orchestrator.
//!
//! Ver `crate::impact` para el module root y la discusion del
//! shape, la politica de "evidence, not verdict" y la degradacion
//! graciosa de los campos de evidencia.
//!
//! Tipos:
//!   - `CompositeImpactQuery`: el input del agente.
//!   - `CompositeImpactResult`: el output agregado, sin veredicto.
//!   - `execute_composite`: la funcion orquestadora.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};

use crate::findings::Finding;
use crate::interface::catalog::load_tool_catalog;
use crate::interface::intent::ToolRef;
use crate::interface::symbol_identity::SymbolId;

/// Razon canonica, constante y estaticamente anclada, por la que
/// una consulta compuesta nunca produce un veredicto. Es parte del
/// contrato: cualquier consumer (humano o agente) puede comprobar
/// que esta cadena explica por que no debe tratar la salida como
/// una recomendacion autoritativa.
pub const COMPOSITE_VERDICT_REFUSED_REASON: &str =
    "composite queries return evidence, not verdicts; investigate each piece";

/// Consulta compuesta: "que necesito saber si quiero refactorizar
/// ESTE simbolo en ESTE workspace, con (opcional) limite de
/// profundidad?".
///
/// `Serialize` only (mismo principio que `ToolRef` en B3.W2): la
/// cadena fluye del servidor al cliente; deserializar con
/// `&'static str` forzaria `String` y perderia el pin estatico del
/// contrato.
#[derive(Debug, Clone, Serialize)]
pub struct CompositeImpactQuery {
    /// Identidad estable del simbolo candidato (B3.W1 contract).
    pub symbol: SymbolId,
    /// Raiz del workspace sobre el que se ejecuta la investigacion.
    pub workspace_root: PathBuf,
    /// Limite de profundidad opcional para el extractor de impacto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_limit: Option<usize>,
}

/// Resultado de una consulta compuesta: agregacion de evidencia +
/// pointers a tools, SIN veredicto.
///
/// `Serialize` only (mismo principio que `ToolRef` en B3.W2). El
/// consumer (humano o modelo) lee el JSON resultante sin
/// deserializarlo a este tipo Rust.
///
/// Los tres campos de evidencia (`impact`, `architecture`,
/// `findings`) son `None` / `vec![]` mientras los extractores no
/// esten mergeados. Cualquier serializador que vea este shape
/// tiene `verdict_refused == true` para recordar que NO debe
/// presentar el resultado como autoritativo.
#[derive(Debug, Clone, Serialize)]
pub struct CompositeImpactResult {
    /// B4.W1 — `None` mientras el extractor no se conecte.
    /// Forward-compatible via `serde_json::Value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<Value>,
    /// B4.W2 — `None` mientras el extractor no se conecte.
    /// Mismo patron de degradacion graciosa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<Value>,
    /// B4.W3 — `Vec<Finding>` (no Option): "cero hallazgos" es un
    /// resultado valido que pertenece al cable.
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// B3.W3 parity — pointers del catalogo `follow_ups`.
    #[serde(default)]
    pub suggested_follow_ups: Vec<ToolRef>,
    /// B3.W3 parity — pointers del catalogo `prerequisites`.
    #[serde(default)]
    pub suggested_prerequisites: Vec<ToolRef>,
    /// Load-bearing B4.W5 invariant. SIEMPRE `true`.
    pub verdict_refused: bool,
    /// Razon canonica y estatica. Ver
    /// [`COMPOSITE_VERDICT_REFUSED_REASON`].
    pub verdict_refused_reason: &'static str,
}

impl CompositeImpactResult {
    /// Helper privado para fijar la invariante load-bearing en el
    /// constructor: `verdict_refused: true` + razon canonica.
    fn investigative() -> Self {
        Self {
            impact: None,
            architecture: None,
            findings: Vec::new(),
            suggested_follow_ups: Vec::new(),
            suggested_prerequisites: Vec::new(),
            verdict_refused: true,
            verdict_refused_reason: COMPOSITE_VERDICT_REFUSED_REASON,
        }
    }
}

/// Ejecuta la consulta compuesta. Mientras los extractores
/// B4.W1 / W2 / W3 no esten mergeados con este modulo, los
/// campos de evidencia quedan `None` / `vec![]` (degradacion
/// graciosa). Los pointers a tools se extraen del catalogo en
/// cualquier caso.
pub fn execute_composite(query: &CompositeImpactQuery) -> CompositeImpactResult {
    let mut result = CompositeImpactResult::investigative();
    let _ = &query.depth_limit;
    result.impact = None;
    result.architecture = None;
    result.findings = Vec::new();

    let catalog = load_tool_catalog();
    if let Some(descriptor) = catalog
        .iter()
        .find(|d| d.name == query.symbol.qualified_name)
    {
        result.suggested_follow_ups =
            tool_names_to_refs(descriptor.follow_ups.as_deref(), &query.symbol);
        result.suggested_prerequisites =
            tool_names_to_refs(descriptor.prerequisites.as_deref(), &query.symbol);
    }

    result
}

fn tool_names_to_refs(
    names: Option<&[String]>,
    symbol: &SymbolId,
) -> Vec<ToolRef> {
    let Some(names) = names else {
        return Vec::new();
    };
    let catalog = load_tool_catalog();
    let names_set: std::collections::HashSet<&str> =
        catalog.iter().map(|d| d.name.as_str()).collect();

    let mut out = Vec::with_capacity(names.len());
    for name in names {
        if !names_set.contains(name.as_str()) {
            continue;
        }
        out.push(ToolRef {
            tool: name.clone(),
            args: json!({
                "qualified_name": symbol.qualified_name,
                "file_path": symbol.file_path.to_string_lossy(),
                "byte_range_start": symbol.byte_range.start,
                "byte_range_end": symbol.byte_range.end,
            }),
            rationale: COMPOSITE_TOOL_REF_RATIONALE,
        });
    }
    out
}

const COMPOSITE_TOOL_REF_RATIONALE: &str =
    "Composite impact query: this tool appears in the catalog's \
     follow_ups or prerequisites for the queried symbol; run it to \
     advance the investigation.";

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    fn stub_query(symbol_qualified: &str) -> CompositeImpactQuery {
        CompositeImpactQuery {
            symbol: SymbolId {
                qualified_name: symbol_qualified.to_string(),
                file_path: PathBuf::from("src/lib.rs"),
                byte_range: Range { start: 0, end: 10 },
            },
            workspace_root: PathBuf::from("."),
            depth_limit: None,
        }
    }

    #[test]
    fn verdict_refused_reason_is_static_and_exact() {
        let r = execute_composite(&stub_query("x"));
        assert!(r.verdict_refused);
        assert_eq!(r.verdict_refused_reason, COMPOSITE_VERDICT_REFUSED_REASON);
        assert!(COMPOSITE_VERDICT_REFUSED_REASON.contains("evidence"));
        assert!(COMPOSITE_VERDICT_REFUSED_REASON.contains("verdict"));
    }

    #[test]
    fn default_construction_carries_verdict_refused_true() {
        let r = CompositeImpactResult::investigative();
        assert!(r.impact.is_none());
        assert!(r.architecture.is_none());
        assert!(r.findings.is_empty());
        assert!(r.verdict_refused);
        assert_eq!(r.verdict_refused_reason, COMPOSITE_VERDICT_REFUSED_REASON);
    }

    #[test]
    fn follow_ups_pull_only_from_catalog() {
        let r = execute_composite(&stub_query("definitely_not_in_any_catalog"));
        assert!(r.suggested_follow_ups.is_empty());
        assert!(r.suggested_prerequisites.is_empty());
    }

    #[test]
    fn prerequisites_pull_only_from_catalog() {
        let r = execute_composite(&CompositeImpactQuery {
            symbol: SymbolId {
                qualified_name: "analyze_impact".into(),
                file_path: PathBuf::from("src/impact.rs"),
                byte_range: Range { start: 0, end: 10 },
            },
            workspace_root: PathBuf::from("."),
            depth_limit: None,
        });
        if let Some(d) = load_tool_catalog()
            .into_iter()
            .find(|d| d.name == "analyze_impact")
        {
            let expected_count = d
                .prerequisites
                .as_ref()
                .map(|v| v.len())
                .unwrap_or_default();
            assert_eq!(r.suggested_prerequisites.len(), expected_count);
        }
    }

    #[test]
    fn missing_impact_is_none_not_empty_value() {
        let r = execute_composite(&stub_query("any::query"));
        assert!(r.impact.is_none());
    }

    #[test]
    fn missing_architecture_is_none() {
        let r = execute_composite(&stub_query("any::query"));
        assert!(r.architecture.is_none());
    }

    #[test]
    fn suggested_tool_ref_args_include_symbol_identity() {
        let r = execute_composite(&CompositeImpactQuery {
            symbol: SymbolId {
                qualified_name: "analyze_impact".into(),
                file_path: PathBuf::from("src/analyze.rs"),
                byte_range: Range { start: 100, end: 200 },
            },
            workspace_root: PathBuf::from("."),
            depth_limit: None,
        });
        let prereq = r
            .suggested_prerequisites
            .iter()
            .find(|t| t.tool == "build_graph");
        if let Some(p) = prereq {
            let v = serde_json::to_value(&p.args)
                .expect("ToolRef.args is serialisable");
            assert!(v.get("qualified_name").is_some());
        }
    }

    #[test]
    fn impact_field_accepts_arbitrary_json_value() {
        // `CompositeImpactResult` is `Serialize` only (mismo principio
        // que `ToolRef` en B3.W2 — `&'static str` no sobrevive
        // deserializacion). Verificamos que el campo `impact`
        // acepta cualquier shape JSON forward-compatible.
        let mut result = CompositeImpactResult::investigative();
        result.impact = Some(json!({
            "paths": [{"complete": true, "coverage": 1.0}],
            "truncated": false,
            "total_visited": 2
        }));
        let json = serde_json::to_value(&result).expect("serializable");
        let imp = json
            .get("impact")
            .expect("impact must serialise when populated");
        assert_eq!(imp["total_visited"], json!(2));
        // `verdict_refused` sigue siendo la pieza load-bearing del
        // pin: sobrevive a un assignment directo del campo `impact`.
        assert_eq!(json["verdict_refused"], json!(true));
    }

    #[test]
    fn execute_composite_is_deterministic() {
        let q = stub_query("deterministic::test::symbol");
        let r1 = execute_composite(&q);
        let r2 = execute_composite(&q);
        let v1 = serde_json::to_value(&r1).expect("serialisable");
        let v2 = serde_json::to_value(&r2).expect("serialisable");
        assert_eq!(v1, v2, "the same query must produce byte-identical results");
        assert!(r1.verdict_refused && r2.verdict_refused);
    }
}

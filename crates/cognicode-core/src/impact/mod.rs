//! B4.W5 — Composite impact query for agents.
//!
//! Estado pre-B4.W5:
//!   - Un agente que investiga una propuesta de refactor necesita
//!     tres piezas de evidencia: las trayectorias de impacto del
//!     simbolo candidato (B4.W1), las observaciones arquitectonicas
//!     sobre las capas afectadas (B4.W2), y los hallazgos de
//!     calidad para los archivos tocados (B4.W3). Tambien necesita
//!     saber que tools ejecutar a continuacion y cuales debe haber
//!     ejecutado antes.
//!   - Cada evidencia se ofrecia como un comando aislado
//!     (`cogh graph impact`, `cogh check-architecture`,
//!     `cogh findings ...`). El agente tenia que recordar los
//!     nombres y orquestarlos a mano.
//!   - Ninguna combinacion devolvia explicitamente "esto NO es un
//!     veredicto": el agente corria el riesgo de leer la salida como
//!     una recomendacion autoritativa cuando solo era una pieza de
//!     evidencia parcial (R-B4-W5).
//!
//! Lo que aporta este modulo:
//!   - `CompositeImpactQuery` y `CompositeImpactResult`: el shape
//!     estandar para una consulta de impacto compuesta. El bin CLI
//!     y el adapter MCP serializan al MISMO shape; el consumer
//!     (humano o modelo) puede leer cualquiera sin conversion.
//!   - Las piezas de evidencia declaradas por el WU:
//!     `impact`, `architecture`, `findings`,
//!     `suggested_follow_ups`, `suggested_prerequisites`, y la pieza
//!     load-bearing del contrato: `verdict_refused` (siempre `true`)
//!     + `verdict_refused_reason`.
//!   - `execute_composite` orquesta los extractores B4.W1 / W2 / W3
//!     cuando se conectan. Mientras esos extractores no esten
//!     mergeados con este modulo, los campos de evidencia quedan
//!     `None` / `vec![]` (degradacion graciosa sin fallback
//!     inventado).
//!   - Los tool pointers (`suggested_follow_ups` y
//!     `suggested_prerequisites`) se extraen EXCLUSIVAMENTE del
//!     catalogo de la herramienta consultada (B3.W3 parity). No hay
//!     heuristica; si el catalogo no declara los affordances, el
//!     campo es vacio.
//!
//! Politica B4.W5:
//!   - `verdict_refused: true` es la espina del WU; este modulo no
//!     produce veredictos bajo ninguna circunstancia.
//!   - Cuando los extractores B4.W1 / W2 / W3 se conecten, el shape
//!     publico del resultado y este modulo no cambian: solo
//!     `execute_composite` se modifica para poblar los campos.

pub mod composite;
pub mod graph;

pub use composite::{
    CompositeImpactQuery, CompositeImpactResult, COMPOSITE_VERDICT_REFUSED_REASON,
    execute_composite,
};
pub use graph::{
    build_impact_result, Direction, ImpactEdge, ImpactLimits, ImpactPath, ImpactResult,
    Provenance,
};

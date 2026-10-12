//! B3.W2 — Progressive context routing.
//!
//! Estado pre-B3.W2:
//!   - 6 tools (`codebase_map`, `smart_search`, `get_symbol_code`,
//!     `read_file`, `search_content`, `retrieve_and_verify`) son
//!     invocables aisladamente, pero el agente tiene que saber los
//!     seis nombres y combinarlos a mano. Una peticion como
//!     "encuentra la definicion de X y muestrame las primeras 30
//!     lineas" fuerza al agente a reconstruir la cadena
//!     `query_symbol_index` -> `get_symbol_code` -> `read_file`.
//!   - El agente repite trabajo: emite la misma consulta varias
//!     veces hasta dar con la combinacion correcta.
//!
//! Lo que aporta este modulo:
//!   - `Intent`: la intencion del agente tipada (Locate, Summarize,
//!     Verify, Read). Modelo minimo: solo lo necesario para enrutar.
//!     Variantes adicionales se anaden cuando B3.W5 cierre el
//!     shape final del comando `cognicode inspect`.
//!   - `ToolRef`: un puntero serializable a un tool del catalogo,
//!     con sus `args` pre-llenados para que el caller los pase tal
//!     cual al handler (CLI/MCP). No es una llamada: el caller
//!     decide cuando ejecutar.
//!   - `route_intent`: el corazon del WU. Dada una `Intent`
//!     devuelve una lista NO-vacia de `ToolRef`s que llevan a un
//!     primer resultado util. La lista esta ordenada: el caller
//!     puede iterarla y emitir las tools en orden, parando cuando
//!     una respuesta sea suficiente.
//!   - 6 tools cubren los 6 huecos: cada uno aparece en al menos
//!     una variante. Ver `route_intent` y tests
//!     `route_intent_*_returns_<tool>`.
//!
//! Politica B3.W2:
//!   - El routing vive AQUI, no en cada handler. Si un handler
//!     quiere sugerir su propio siguiente paso, emite un `ToolRef`
//!     serializado en su respuesta; no recalcula la cadena.
//!   - El routing es ESTATICO (no lee el estado del workspace). Un
//!     routing dinamico (e.g. "si no hay indice, salta
//!     query_symbol_index") es B3.W3 (affordances tipadas).
//!   - El routing es HONESTO: cada `ToolRef.rationale` explica
//!     POR QUE ese tool aparece. Un agente que sigue la cadena
//!     puede justificar cada paso.
//!
//! Out of scope (B3.W5+):
//!   - Persistir historial de navegacion (siguiente paso depende
//!     de respuestas anteriores).
//!   - Componer multiples `Intent`s en una sola llamada (e.g.
//!     `Intent::Locate { .. }.then(Intent::Read { .. })`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ops::Range;

/// Intencion del agente. El conjunto de variantes es el vocabulario
/// que `cognicode inspect` reconoce en v0.101.13. Anadir una
/// variante aqui implica extender `route_intent` Y los `intent_tags`
/// en `product/tools.json` (paridad pinea por
/// `b3_w2_progressive_context::intent_tags_in_catalog_match_route_intent`).
///
/// English: the user's stated goal.
/// Espanol: la intencion declarada por el agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// "Encuentra donde esta definido X". El agente quiere la
    /// primera localizacion y el codigo asociado.
    Locate { symbol: String },
    /// "Dame un resumen de la estructura de X". El agente quiere
    /// una vista global antes de profundizar.
    Summarize { scope: String },
    /// "Verifica que la afirmacion X es correcta". El agente
    /// quiere evidencia grounded antes de propagar la respuesta.
    Verify { claim: String },
    /// "Lee el archivo X (lineas Y..Z)". El agente quiere el
    /// contenido exacto, no un resumen.
    Read {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<Range<usize>>,
    },
}

/// Un puntero a un tool del catalogo, listo para invocar.
///
/// `tool` es el nombre canonico de `product/tools.json`. `args`
/// es un `serde_json::Value` que el handler puede deserializar
/// directamente. `rationale` explica POR QUE este tool aparece
/// en la cadena: el agente (humano o modelo) puede leerlo para
/// entender si seguir adelante o parar.
///
/// English: a reference to a catalog tool with pre-filled args.
/// Espanol: una referencia a un tool del catalogo con sus args.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRef {
    pub tool: String,
    pub args: Value,
    pub rationale: &'static str,
}

/// Respuesta de `route_intent`. Lleva la intencion original
/// (para que el caller pueda correlacionar con su peticion),
/// el resultado primario del primer tool, y los siguientes
/// pasos sugeridos.
///
/// English: the routing function's reply shape.
/// Espanol: la forma de respuesta del routing.
///
/// Nota: solo `Serialize` (no `Deserialize`) porque la cadena
/// fluye del servidor al cliente. Deserializar con un lifetime
/// no-static forzaria `String` en lugar de `&'static str`
/// para `rationale`, lo que perderia el pin estatico del
/// contrato. Si B3.W5 anade un cliente que necesite
/// deserializar, se cambiara a `String` con un test explicito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntentResponse {
    pub intent: Intent,
    pub primary: Value,
    pub next_steps: Vec<ToolRef>,
}

/// Routing: dada una `Intent`, devuelve los `ToolRef`s
/// ordenados que llevan a un primer resultado util.
///
/// English: map an intent to a list of tool refs to invoke.
/// Espanol: enruta una intencion a una lista de tool refs.
///
/// Politica: la lista NO esta vacia. La unidad de composicion
/// minima es 1 tool (e.g. `Read` enruta solo a `read_file`);
/// las intenciones mas compuestas (Locate) encadenan hasta 5.
/// El caller itera y para cuando una respuesta basta.
pub fn route_intent(intent: &Intent) -> Vec<ToolRef> {
    match intent {
        Intent::Locate { symbol } => route_locate(symbol),
        Intent::Summarize { scope } => route_summarize(scope),
        Intent::Verify { claim } => route_verify(claim),
        Intent::Read { path, range } => route_read(path, range.as_ref()),
    }
}

/// Cadena para `Intent::Locate`. Encadena:
///   1. `query_symbol_index` (definicion: donde esta X?)
///   2. `get_symbol_code` (codigo: que hace X?)
///   3. `smart_search` (uso: donde se usa X?)
///   4. `search_content` (literal: busqueda regex/texto)
///   5. `read_file` (contexto: el archivo que contiene X)
///
/// Justificacion: la mayoria de los agentes que dicen
/// "encuentra X" quieren los cinco: definicion + usos + contexto.
/// El caller puede iterar y parar en (1) si solo necesita la URL.
fn route_locate(symbol: &str) -> Vec<ToolRef> {
    vec![
        ToolRef {
            tool: "query_symbol_index".to_string(),
            args: json!({ "symbol_name": symbol }),
            rationale: "Locate starts with the symbol index: where is X defined?",
        },
        ToolRef {
            tool: "get_symbol_code".to_string(),
            args: json!({ "symbol_name": symbol }),
            rationale: "Once X is located, fetch the source code at the index entry",
        },
        ToolRef {
            tool: "smart_search".to_string(),
            args: json!({ "query": symbol, "limit": 20 }),
            rationale: "Find call sites and references across the workspace",
        },
        ToolRef {
            tool: "search_content".to_string(),
            args: json!({ "query": symbol, "regex": false, "limit": 30 }),
            rationale: "Literal text search to catch strings, comments, docs",
        },
        ToolRef {
            tool: "read_file".to_string(),
            args: json!({ "chunk_size": 200 }),
            rationale: "Read the file surrounding the definition for context",
        },
    ]
}

/// Cadena para `Intent::Summarize`. Encadena:
///   1. `codebase_map` (mapa global: estructura del scope)
///   2. `read_file` (entry points: leer archivos raiz)
///
/// Justificacion: el resumen se construye sobre el mapa. Leer
/// archivos concretos (entry points) lo enriquece, pero la
/// mayoria de los agentes pararan en (1).
fn route_summarize(scope: &str) -> Vec<ToolRef> {
    vec![
        ToolRef {
            tool: "codebase_map".to_string(),
            args: json!({ "scope": scope }),
            rationale: "Summarize starts with the codebase map: structure of the scope",
        },
        ToolRef {
            tool: "read_file".to_string(),
            args: json!({ "chunk_size": 100 }),
            rationale: "Read entry-point files to enrich the structural summary",
        },
    ]
}

/// Cadena para `Intent::Verify`. Encadena:
///   1. `retrieve_and_verify` (grounded evidence: la fuente
///      primaria de la afirmacion)
///   2. `search_content` (literal: corroborar con texto)
///
/// Justificacion: una afirmacion sin grounded evidence es
/// alucinacion. `retrieve_and_verify` es el PRIMER paso
/// obligatorio; `search_content` es el segundo, para corroborar
/// cross-source.
fn route_verify(claim: &str) -> Vec<ToolRef> {
    vec![
        ToolRef {
            tool: "retrieve_and_verify".to_string(),
            args: json!({ "claim": claim }),
            rationale: "Verify must start with grounded evidence retrieval",
        },
        ToolRef {
            tool: "search_content".to_string(),
            args: json!({ "query": claim, "regex": false, "limit": 20 }),
            rationale: "Cross-check the claim against literal text occurrences",
        },
    ]
}

/// Cadena para `Intent::Read`. Encadena:
///   1. `read_file` (contenido exacto: con rango si aplica)
///   2. `search_content` (lineas que coinciden: navegacion
///      intra-archivo)
///
/// Justificacion: leer es leer; la busqueda intra-archivo es
/// opcional. La mayoria de los agentes pararan en (1).
fn route_read(path: &str, range: Option<&Range<usize>>) -> Vec<ToolRef> {
    let mut args = json!({ "path": path });
    if let Some(r) = range {
        args["start_line"] = json!(r.start);
        args["end_line"] = json!(r.end);
    }
    vec![
        ToolRef {
            tool: "read_file".to_string(),
            args,
            rationale: "Read is the primary operation: fetch the file content",
        },
        ToolRef {
            tool: "search_content".to_string(),
            args: json!({
                "path": path,
                "regex": false,
                "limit": 30
            }),
            rationale: "After reading, optionally search for related lines",
        },
    ]
}

#[cfg(test)]
mod tests {
    //! Unit tests for the routing function. The 5+ tests below
    //! pin the minimum contract: every Intent variant routes to
    //! at least one tool, the canonical pairings (Locate ->
    //! query_symbol_index + get_symbol_code, Verify ->
    //! retrieve_and_verify, Read -> read_file) are honored, and
    //! the 6 B3.W2 tools each appear in at least one branch.

    use super::*;

    #[test]
    fn locate_routes_to_query_symbol_index() {
        let route = route_intent(&Intent::Locate {
            symbol: "foo".into(),
        });
        assert!(route.iter().any(|r| r.tool == "query_symbol_index"));
    }

    #[test]
    fn locate_routes_to_get_symbol_code() {
        let route = route_intent(&Intent::Locate {
            symbol: "foo".into(),
        });
        assert!(route.iter().any(|r| r.tool == "get_symbol_code"));
    }

    #[test]
    fn locate_routes_to_smart_search() {
        let route = route_intent(&Intent::Locate {
            symbol: "foo".into(),
        });
        assert!(route.iter().any(|r| r.tool == "smart_search"));
    }

    #[test]
    fn summarize_routes_to_codebase_map() {
        let route = route_intent(&Intent::Summarize {
            scope: ".".into(),
        });
        assert!(route.iter().any(|r| r.tool == "codebase_map"));
    }

    #[test]
    fn verify_routes_to_retrieve_and_verify() {
        let route = route_intent(&Intent::Verify {
            claim: "claim".into(),
        });
        assert!(route.iter().any(|r| r.tool == "retrieve_and_verify"));
    }

    #[test]
    fn verify_routes_to_search_content() {
        // Verify must corroborate cross-source via search_content.
        let route = route_intent(&Intent::Verify {
            claim: "claim".into(),
        });
        assert!(route.iter().any(|r| r.tool == "search_content"));
    }

    #[test]
    fn read_routes_to_read_file() {
        let route = route_intent(&Intent::Read {
            path: "x.rs".into(),
            range: None,
        });
        assert!(route.iter().any(|r| r.tool == "read_file"));
    }

    #[test]
    fn read_with_range_includes_start_and_end_line() {
        let route = route_intent(&Intent::Read {
            path: "x.rs".into(),
            range: Some(Range { start: 10, end: 20 }),
        });
        let reader = route
            .iter()
            .find(|r| r.tool == "read_file")
            .expect("Read -> read_file");
        assert_eq!(reader.args["start_line"], json!(10));
        assert_eq!(reader.args["end_line"], json!(20));
    }

    #[test]
    fn every_variant_routes_to_a_non_empty_vec() {
        // Property: route_intent never returns an empty Vec. A
        // routing that emits no tools would leave the caller with
        // nothing to call; that's a contract failure.
        let cases = vec![
            Intent::Locate { symbol: "x".into() },
            Intent::Summarize {
                scope: ".".into(),
            },
            Intent::Verify {
                claim: "c".into(),
            },
            Intent::Read {
                path: "f.rs".into(),
                range: None,
            },
        ];
        for intent in cases {
            let route = route_intent(&intent);
            assert!(
                !route.is_empty(),
                "route_intent({intent:?}) returned empty Vec"
            );
        }
    }

    #[test]
    fn all_six_b3w2_tools_appear_in_some_branch() {
        // The 6 B3.W2 composable tools must each appear in at
        // least one Intent variant's routing. If one disappears,
        // the scaffold has drifted from the WU scope.
        const TARGET: &[&str] = &[
            "codebase_map",
            "smart_search",
            "get_symbol_code",
            "read_file",
            "search_content",
            "retrieve_and_verify",
        ];
        let intents = vec![
            Intent::Locate {
                symbol: "x".into(),
            },
            Intent::Summarize {
                scope: ".".into(),
            },
            Intent::Verify {
                claim: "c".into(),
            },
            Intent::Read {
                path: "f.rs".into(),
                range: None,
            },
        ];
        let mut seen = std::collections::HashSet::new();
        for intent in &intents {
            for r in route_intent(intent) {
                seen.insert(r.tool.clone());
            }
        }
        for tool in TARGET {
            assert!(
                seen.contains(*tool),
                "{tool} must appear in at least one branch; seen={seen:?}"
            );
        }
    }

    #[test]
    fn intent_serializes_with_kind_discriminator() {
        // JSON contract: each Intent variant serializes with
        // `kind: "locate" | "summarize" | "verify" | "read"`.
        // The bin CLI uses this to deserialize intent strings from
        // the user without a custom parser per variant.
        let locate = Intent::Locate {
            symbol: "X".into(),
        };
        let v = serde_json::to_value(&locate).expect("serialise Locate");
        assert_eq!(v["kind"], "locate");
        assert_eq!(v["symbol"], "X");

        let read = Intent::Read {
            path: "p.rs".into(),
            range: None,
        };
        let v = serde_json::to_value(&read).expect("serialise Read");
        assert_eq!(v["kind"], "read");
        assert_eq!(v["path"], "p.rs");
        // range is None => not serialized (skip_serializing_if).
        assert!(v.get("range").is_none());
    }
}
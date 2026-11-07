//! El sobre de la respuesta: `result`, `basis`, `budget` y los enlaces.
//!
//! Una respuesta que dice **qué encontró**, **sobre qué se apoya**, **qué
//! dejó fuera** y **qué se puede hacer ahora** es una respuesta que un
//! consumidor puede usar sin adivinar. Antes de este módulo el producto
//! publicaba el resultado y poco más: el que quería saber si la respuesta
//! era completa, o qué se había omitido, o cuál era el siguiente paso,
//! tenía que reconstruirlo desde el payload —y a veces no podía, porque el
//! payload no lo decía.
//!
//! ## Un dueño, dos transportes
//!
//! El sobre se construye **una vez**, aquí, a partir de la respuesta; el
//! MCP lo publica en `structuredContent` y el CLI lo imprime como órdenes.
//! Ninguno de los dos vuelve a decidir qué es un agujero, qué es un
//! veredicto ni qué viene después: si lo hicieran, el CLI y el MCP
//! dirían cosas distintas sobre la misma respuesta, que es exactamente lo
//! que este módulo viene a impedir.
//!
//! ## Lo que no se ha medido, no se anuncia
//!
//! Un campo que la respuesta no declara se publica `null`, no `0` ni
//! `complete`. Un cero es una medición; `null` es la ausencia de una
//! medición. El consumidor puede distinguir las dos, y con ellas puede
//! distinguir «la respuesta es completa» de «nadie lo comprobó».
//!
//! ## Los enlaces
//!
//! Cada enlace nombra una tool del catálogo, con los argumentos **rellenos**
//! (lo que el consumidor tiene que pasar), el motivo por el que existe, la
//! precondición que tiene que cumplirse y la autoridad que la tool tiene.
//! Un enlace que nombra una tool que este build no anuncia no se publica:
//! un enlace muerto es peor que no tener enlaces.
//!
//! Los siete casos que producen un enlace: la **ambigüedad** (una
//! referencia que declara un nombre que varios símbolos llevan), el
//! **recorte** (la respuesta dice que está cortada), el **fan-in alto**
//! (muchos dependen de un símbolo), la **cobertura parcial** (la respuesta
//! deja agujeros), el **riesgo** (la respuesta dice que el riesgo es alto), el
//! **plan de cambio** (la respuesta propone cambiar algo) y el **reintento**
//! (un paso de la receta falló).
//!
//! ## Los handles
//!
//! Un handle (`sym:`, `page:`, `plan:`) es una referencia firmada con el
//! mecanismo de [`crate::interface::mcp::cursor`]: los mismos anclajes, el
//! mismo hash, los mismos errores tipados. Un handle manipulado no pasa, y
//! uno de otro workspace o de otra instantánea tampoco.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::interface::catalog::ToolDescriptor;
use crate::interface::mcp::cursor::{self, CursorError, CursorErrorKind};

/// El contexto al que se ancla un handle: el workspace y la instantánea.
///
/// Es el contexto del cursor, con otro nombre: un handle y un cursor se
/// firman igual y caducan igual.
pub use crate::interface::mcp::cursor::CursorContext as HandleContext;

/// La versión de la forma del sobre.
///
/// Cuando la forma cambia, el número cambia: un consumidor que entiende la
/// versión 1 sabe qué puede leer.
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;

/// El fan-in a partir del cual un símbolo se considera caliente.
///
/// El número es del producto, no del consumidor: por debajo de él, mirar
/// los llamantes de un símbolo es ruido.
const FAN_IN_THRESHOLD: usize = 8;

// --- El sobre ---

/// La respuesta, con lo que la sostiene y lo que viene después.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Envelope {
    /// La versión de la forma del sobre.
    pub schema_version: u32,

    /// El veredicto de la respuesta: `complete`, `partial`, `unknown`,
    /// `unsupported` o `failed`.
    ///
    /// La palabra la declara quien midió; el sobre no la inventa.
    pub state: String,

    /// El resultado, tal cual lo publicó la tool.
    ///
    /// Viaja entero y sin reescribir: un sobre que resumiera el resultado
    /// sería un segundo resultado.
    pub result: Value,

    /// Sobre qué se apoya la respuesta.
    pub basis: Basis,

    /// Lo que la respuesta dejó fuera.
    pub budget: Budget,

    /// Lo que se puede hacer ahora.
    #[serde(rename = "_links")]
    pub links: Vec<Link>,
}

/// Lo que sostiene la respuesta.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Basis {
    /// Lo que la respuesta cubre del árbol que se midió.
    ///
    /// `None` cuando la respuesta no dice nada de la cobertura: nadie la
    /// midió.
    pub coverage: Option<CoverageBasis>,

    /// Sobre qué árbol y qué instantánea se calculó.
    ///
    /// `None` cuando la respuesta no lo declara.
    pub freshness: Option<FreshnessBasis>,

    /// Las aristas que un LSP confirmó.
    ///
    /// `None` siempre, hoy: el producto no publica un recuento de aristas
    /// confirmadas por LSP, y un `0` sería una medición que nadie hizo.
    pub lsp_confirmed_edges: Option<usize>,
}

/// La cobertura de la respuesta.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoverageBasis {
    /// El veredicto de la cobertura: `complete`, `partial`, `unknown` o
    /// `unsupported`.
    pub status: String,

    /// Los ficheros que la lectura no pudo leer.
    pub skipped_files: usize,

    /// Las referencias que no se pudieron atar a una declaración.
    pub pending_references: usize,

    /// Las relaciones que el repositorio rechazó.
    pub rejected_relationships: usize,
}

/// La frescura de la respuesta.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FreshnessBasis {
    /// El workspace canónico del que se calculó.
    pub workspace: String,

    /// El digest del manifiesto de fuentes: el mismo árbol da el mismo
    /// digest, y cualquier cambio lo cambia.
    pub snapshot: String,

    /// De dónde salió el grafo: `built`, `cache` o `daemon`.
    pub graph_source: Option<String>,
}

/// Lo que la respuesta dejó fuera.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Budget {
    /// Cuántos elementos quedaron fuera, cuando la respuesta lo dice.
    pub omitted: Option<usize>,

    /// Por qué quedaron fuera, cuando la respuesta lo dice.
    pub reason: Option<String>,

    /// El handle firmado por el que se sigue leyendo.
    pub page: Option<String>,
}

/// Un enlace: lo que se puede hacer ahora, con los argumentos rellenos.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Link {
    /// Qué clase de enlace es: `resolve-ambiguity`, `continue`,
    /// `inspect-callers`, `reindex`, `inspect-risk`, `preview-change` o
    /// `retry`.
    pub rel: String,

    /// La tool que responde al enlace.
    pub tool: String,

    /// Los argumentos con los que hay que llamarla.
    pub required_args: Map<String, Value>,

    /// Por qué existe este enlace.
    pub reason: String,

    /// Lo que tiene que ser verdad antes de llamar a la tool.
    pub precondition: Option<String>,

    /// La autoridad de la tool: `read`, `mutating`, `execute` o `network`.
    pub authority: String,

    /// El handle firmado que el enlace lleva, cuando lo lleva.
    pub handle: Option<String>,
}

// --- El constructor ---

/// Construye el sobre de una respuesta.
///
/// Los argumentos de la llamada se necesitan para los enlaces que se
/// derivan de ellos (continuar una respuesta cortada repite la llamada con
/// un argumento más), y el contexto ancla los handles que se firman.
pub fn envelope_for(
    tool: &str,
    args: &Map<String, Value>,
    result: &Value,
    ctx: &HandleContext,
) -> Envelope {
    Envelope {
        schema_version: ENVELOPE_SCHEMA_VERSION,
        state: state_of(result),
        result: result.clone(),
        basis: basis_of(result),
        budget: budget_of(tool, args, result, ctx),
        links: links_for(tool, args, result, ctx),
    }
}

/// El veredicto que la respuesta declara.
///
/// Lo primero es la palabra que la respuesta trae (`status`, `outcome`,
/// `state`): si está, esa es. Si no, la respuesta que deja agujeros es
/// `partial` —el agujero **es** el veredicto— y la que no dice nada es
/// `complete`: no afirma nada que la contradiga.
fn state_of(result: &Value) -> String {
    // El veredicto puede venir en la respuesta o en su bloque de
    // cobertura: depende de qué forma declare la respuesta.
    let coverage = result.get("coverage").filter(|c| c.is_object());
    for key in ["status", "outcome", "state"] {
        if let Some(word) = result
            .get(key)
            .and_then(Value::as_str)
            .or_else(|| coverage.and_then(|c| c.get(key)).and_then(Value::as_str))
            && matches!(
                word,
                "complete" | "partial" | "unknown" | "unsupported" | "failed"
            )
        {
            return word.to_string();
        }
    }
    match holes_of(result) {
        Some(holes) if holes.total() > 0 => "partial".to_string(),
        _ => "complete".to_string(),
    }
}

/// Los agujeros que la respuesta declara.
///
/// `None` cuando la respuesta no dice nada de la cobertura: nadie la midió.
fn holes_of(result: &Value) -> Option<Holes> {
    // Los agujeros viven en la respuesta o en su bloque de cobertura:
    // la respuesta los declara donde los declara.
    let source = result
        .get("coverage")
        .filter(|c| c.is_object())
        .unwrap_or(result);
    let skipped = count_of(source, &["skipped_files"]);
    let pending = count_of(source, &["pending_references"]);
    let rejected = count_of(source, &["rejected_relationships", "rejected_relations"]);
    if skipped.is_none() && pending.is_none() && rejected.is_none() {
        return None;
    }
    Some(Holes {
        skipped: skipped.unwrap_or(0),
        pending: pending.unwrap_or(0),
        rejected: rejected.unwrap_or(0),
    })
}

/// Los agujeros de una respuesta.
struct Holes {
    skipped: usize,
    pending: usize,
    rejected: usize,
}

impl Holes {
    /// Cuántos agujeros deja la respuesta.
    fn total(&self) -> usize {
        self.skipped + self.pending + self.rejected
    }
}

/// El número de elementos de un campo, si la respuesta lo declara.
fn count_of(result: &Value, names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|name| result.get(*name).and_then(Value::as_array))
        .map(|items| items.len())
}

/// La palabra, en singular o en plural, según lo que se cuenta.
///
/// El motivo de un enlace se lee en una terminal y en un cliente: es una
/// frase, no un contador. Una razón que dice «1 files» es una razón
/// escrita por una máquina que no cuenta.
fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// La base de la respuesta.
fn basis_of(result: &Value) -> Basis {
    Basis {
        coverage: holes_of(result).map(|holes| CoverageBasis {
            status: state_of(result),
            skipped_files: holes.skipped,
            pending_references: holes.pending,
            rejected_relationships: holes.rejected,
        }),
        freshness: freshness_of(result),
        lsp_confirmed_edges: None,
    }
}

/// La frescura que la respuesta declara en su `basis`.
fn freshness_of(result: &Value) -> Option<FreshnessBasis> {
    let basis = result.get("basis")?.as_object()?;
    let workspace = basis.get("workspace")?.as_str()?.to_string();
    let snapshot = basis
        .get("source_manifest_digest")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let graph_source = basis
        .get("graph_source")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(FreshnessBasis {
        workspace,
        snapshot,
        graph_source,
    })
}

/// El presupuesto de la respuesta.
fn budget_of(tool: &str, args: &Map<String, Value>, result: &Value, ctx: &HandleContext) -> Budget {
    let omitted = result
        .get("omitted")
        .or_else(|| result.get("omitted_count"))
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let reason = result
        .get("truncation_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    let page = continuation_of(tool, args, result, ctx)
        .map(|(_, value)| mint_handle(HandleKind::Page, ctx, &value));
    Budget {
        omitted,
        reason,
        page,
    }
}

// --- Los enlaces ---

/// Los enlaces de la respuesta.
///
/// Cada regla produce **un** enlace: la respuesta puede tener muchos
/// agujeros, y el consumidor necesita saber que hay un reindexado, no
/// reindexar tres veces. El motivo dice cuántos hay.
fn links_for(
    tool: &str,
    args: &Map<String, Value>,
    result: &Value,
    ctx: &HandleContext,
) -> Vec<Link> {
    [
        ambiguity_link(result, ctx),
        continue_link(tool, args, result, ctx),
        callers_link(result, ctx),
        reindex_link(result, ctx),
        risk_link(result, ctx),
        preview_link(result, ctx),
        retry_link(tool, args, result),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// El reintento: un paso de la receta falló, y la receta se puede volver a
/// correr.
///
/// El enlace lleva los argumentos de la llamada —el nombre de la receta y sus
/// parámetros— sin el `plan`: reintentar no es planificar. Un consumidor que
/// ve un `partial` sabe qué volver a correr y con qué.
fn retry_link(tool: &str, args: &Map<String, Value>, result: &Value) -> Option<Link> {
    if tool != "recipe" {
        return None;
    }
    let failure = result.get("failure")?.as_object()?;
    let goal = failure.get("goal")?.as_str()?;
    let reason = failure.get("reason")?.as_str()?;
    let mut filled = args.clone();
    filled.remove("plan");
    Some(link(
        "retry",
        "recipe",
        filled,
        format!("the step `{goal}` failed: {reason}"),
        None,
    ))
}

/// La ambigüedad: una referencia declara un nombre que varios símbolos
/// llevan, y el código de esa referencia es lo que la resuelve.
fn ambiguity_link(result: &Value, ctx: &HandleContext) -> Option<Link> {
    let pending = result.get("pending_references")?.as_array()?;
    let ambiguous: Vec<&Value> = pending
        .iter()
        .filter(|entry| entry.get("reason").and_then(Value::as_str) == Some("ambiguous"))
        .collect();
    let entry = ambiguous.first()?;
    let file = entry.get("file")?.as_str()?.to_string();
    let line = entry.get("line").and_then(Value::as_u64)?;
    let target = entry.get("target").and_then(Value::as_str)?;

    let mut args = Map::new();
    args.insert("file".into(), json!(file));
    args.insert("line".into(), json!(line));
    args.insert("col".into(), json!(1));

    Some(link(
        "resolve-ambiguity",
        "get_symbol_code",
        args,
        format!(
            "the answer names {} ambiguous {}",
            ambiguous.len(),
            plural(ambiguous.len(), "reference")
        ),
        Some(mint_handle(HandleKind::Sym, ctx, &json!(target))),
    ))
}

/// El recorte: la respuesta dice que está cortada, y dice por dónde sigue.
fn continue_link(
    tool: &str,
    args: &Map<String, Value>,
    result: &Value,
    ctx: &HandleContext,
) -> Option<Link> {
    let cut = result.get("truncated").and_then(Value::as_bool) == Some(true)
        || result.get("has_more").and_then(Value::as_bool) == Some(true);
    if !cut {
        return None;
    }
    let (name, value) = continuation_of(tool, args, result, ctx)?;
    let mut filled = args.clone();
    filled.insert(name.clone(), value.clone());
    Some(link(
        "continue",
        tool,
        filled,
        format!("the answer was cut; it continues from {}", value),
        Some(mint_handle(HandleKind::Page, ctx, &value)),
    ))
}

/// El fan-in alto: un símbolo del que dependen muchos.
///
/// La respuesta dice cuántos; el enlace dice a mirar quién la llama.
fn callers_link(result: &Value, ctx: &HandleContext) -> Option<Link> {
    let fan_in = result.get("fan_in").and_then(Value::as_u64)? as usize;
    if fan_in < FAN_IN_THRESHOLD {
        return None;
    }
    let symbol = subject_of(result)?;

    let mut args = Map::new();
    args.insert("symbol_name".into(), json!(symbol));
    args.insert("direction".into(), json!("incoming"));

    Some(link(
        "inspect-callers",
        "get_call_hierarchy",
        args,
        format!("the symbol has {} {}", fan_in, plural(fan_in, "caller")),
        Some(mint_handle(HandleKind::Sym, ctx, &json!(symbol))),
    ))
}

/// La cobertura parcial: la respuesta no lo cubre todo, y el reindexado es
/// lo que la cubre.
fn reindex_link(result: &Value, ctx: &HandleContext) -> Option<Link> {
    let holes = holes_of(result)?;
    if holes.total() == 0 {
        return None;
    }
    let mut args = Map::new();
    args.insert("directory".into(), json!(ctx.workspace));
    Some(link(
        "reindex",
        "build_graph",
        args,
        format!(
            "{} {}, {} {} and {} {} were left out of the answer",
            holes.skipped,
            plural(holes.skipped, "file"),
            holes.pending,
            plural(holes.pending, "reference"),
            holes.rejected,
            plural(holes.rejected, "relation")
        ),
        None,
    ))
}

/// El riesgo: la respuesta dice que el riesgo es alto, y dice qué toca.
fn risk_link(result: &Value, _ctx: &HandleContext) -> Option<Link> {
    let risk = result.get("risk_level").and_then(Value::as_str)?;
    let risk = risk.to_lowercase();
    let risk = risk.as_str();
    if !matches!(risk, "high" | "critical") {
        return None;
    }
    let impacted = result.get("impacted_files")?.as_array()?;
    let first = impacted.first()?.as_str()?;

    let mut args = Map::new();
    args.insert("file_path".into(), json!(first));

    Some(link(
        "inspect-risk",
        "get_file_symbols",
        args,
        format!(
            "the risk level is {risk} and it touches {} {}",
            impacted.len(),
            plural(impacted.len(), "file")
        ),
        None,
    ))
}

/// El plan de cambio: la respuesta propone cambiar algo, y lo que se puede
/// mirar antes de tocarlo es qué rompería.
fn preview_link(result: &Value, ctx: &HandleContext) -> Option<Link> {
    let recommendations = result.get("recommendations")?.as_array()?;
    if recommendations.is_empty() {
        return None;
    }
    let subject = subject_of(result)?;

    let mut args = Map::new();
    args.insert("symbol_name".into(), json!(subject));

    Some(link(
        "preview-change",
        "analyze_impact",
        args,
        format!(
            "the answer proposes {} {}; this is what the change would touch",
            recommendations.len(),
            plural(recommendations.len(), "change")
        ),
        Some(mint_handle(HandleKind::Plan, ctx, &json!(subject))),
    ))
}

/// El sujeto del que habla la respuesta.
///
/// El nombre del campo lo dice la respuesta: `symbol`, `node`, `file`,
/// `path`, `file_path` o `name`. Una respuesta que no nombra de qué habla
/// no produce enlace: un enlace con un sujeto inventado es un enlace a
/// otra cosa.
fn subject_of(result: &Value) -> Option<String> {
    ["symbol", "node", "file", "path", "file_path", "name"]
        .iter()
        .find_map(|name| result.get(*name).and_then(Value::as_str))
        .map(str::to_string)
}

/// La continuación que la respuesta publica, y el argumento por el que se
/// sigue.
///
/// El argumento tiene que existir en el esquema de la tool y el valor tiene
/// que encajar con él: un enlace que inventa un argumento es un enlace que
/// la tool no entiende.
fn continuation_of(
    tool: &str,
    _args: &Map<String, Value>,
    result: &Value,
    _ctx: &HandleContext,
) -> Option<(String, Value)> {
    let schema = catalogue().get(tool)?.input_schema.as_object()?;
    let properties = schema.get("properties")?.as_object()?;
    for name in ["continuation_token", "cursor", "offset", "start_line"] {
        let declared = properties.get(name)?;
        let value = result
            .get(name)
            .or_else(|| result.get(format!("next_{name}")))?;
        if !type_fits(declared, value) {
            continue;
        }
        return Some((name.to_string(), value.clone()));
    }
    None
}

/// Whether a value fits the type the schema declares.
///
/// The type can be a list (`["string", "null"]`): an argument that accepts
/// `null` accepts a string too.
fn type_fits(declared: &Value, value: &Value) -> bool {
    let types: Vec<&str> = match declared.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => return false,
    };
    types.iter().any(|expected| match (*expected, value) {
        ("string", Value::String(_)) => true,
        ("integer", Value::Number(n)) => n.is_i64() || n.is_u64(),
        ("number", Value::Number(_)) => true,
        ("array", Value::Array(_)) => true,
        ("object", Value::Object(_)) => true,
        ("boolean", Value::Bool(_)) => true,
        ("null", Value::Null) => true,
        _ => false,
    })
}

/// Un enlace, con lo que el catálogo dice de la tool que nombra.
///
/// Una tool que este build no anuncia no produce enlace: el enlace tiene
/// que poder resolverse, o no es un enlace.
fn link(
    rel: &str,
    tool: &str,
    required_args: Map<String, Value>,
    reason: String,
    handle: Option<String>,
) -> Link {
    let descriptor = catalogue().get(tool);
    Link {
        rel: rel.to_string(),
        tool: tool.to_string(),
        required_args,
        reason,
        precondition: descriptor
            .and_then(|d| d.prerequisites.clone())
            .map(|p| p.join(", "))
            .filter(|p| !p.is_empty()),
        authority: descriptor
            .map(|d| d.authority.clone())
            .unwrap_or_else(|| "read".to_string()),
        handle,
    }
}

/// El catálogo que este build anuncia, leído una vez.
///
/// Los enlaces nombran tools: el catálogo es el sitio donde se resuelve si
/// una tool se puede llamar, y se consulta el **activo** —una tool detrás
/// de una feature desactivada tampoco se puede llamar aquí—.
fn catalogue() -> &'static HashMap<String, ToolDescriptor> {
    static CATALOGUE: OnceLock<HashMap<String, ToolDescriptor>> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        crate::interface::catalog::load_tool_catalog_active()
            .into_iter()
            .map(|descriptor| (descriptor.name.clone(), descriptor))
            .collect()
    })
}

/// Las líneas «qué viene después» que la respuesta publica.
///
/// Una línea por enlace, y la línea dice a quién hay que llamar, con qué
/// argumentos y por qué. Un agente que lee la respuesta como texto ve el
/// paso siguiente sin tener que analizar nada.
pub fn follow_up_lines(envelope: &Envelope) -> Vec<String> {
    envelope
        .links
        .iter()
        .map(|link| {
            let arguments =
                serde_json::to_string(&link.required_args).unwrap_or_else(|_| "{}".to_string());
            let handle = link
                .handle
                .as_ref()
                .map(|handle| format!(" [{handle}]"))
                .unwrap_or_default();
            format!(
                "Siguiente: {} {}{} — {}",
                link.tool, arguments, handle, link.reason
            )
        })
        .collect()
}

// --- Los handles ---

/// De qué es un handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    /// Un símbolo.
    Sym,
    /// Una página.
    Page,
    /// Un plan.
    Plan,
}

impl HandleKind {
    /// El prefijo con el que viaja el handle.
    pub fn prefix(self) -> &'static str {
        match self {
            HandleKind::Sym => "sym",
            HandleKind::Page => "page",
            HandleKind::Plan => "plan",
        }
    }

    /// El prefijo que se entiende, si lo hay.
    pub fn parse(prefix: &str) -> Option<Self> {
        match prefix {
            "sym" => Some(HandleKind::Sym),
            "page" => Some(HandleKind::Page),
            "plan" => Some(HandleKind::Plan),
            _ => None,
        }
    }
}

/// Un handle verificado.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedHandle {
    /// De qué es el handle.
    pub kind: HandleKind,
    /// A qué apunta.
    pub payload: Value,
}

/// Firma un handle para un contexto.
///
/// El handle es el prefijo de su clase y el sobre firmado: `sym:…`,
/// `page:…`, `plan:…`.
pub fn mint_handle(kind: HandleKind, ctx: &HandleContext, payload: &Value) -> String {
    format!(
        "{}:{}",
        kind.prefix(),
        cursor::encode_signed(ctx, kind.prefix(), payload)
    )
}

/// Verifica un handle contra un contexto.
///
/// Un handle manipulado, de otro workspace o de otra instantánea vuelve
/// con su error tipado: `Garbled`, `WorkspaceMismatch` o
/// `SnapshotExpired`. Un prefijo que no es de los nuestros tampoco es un
/// handle: es `Garbled`.
pub fn verify_handle(handle: &str, ctx: &HandleContext) -> Result<VerifiedHandle, CursorError> {
    let (prefix, token) = handle.split_once(':').ok_or_else(|| CursorError {
        kind: CursorErrorKind::Garbled,
        reason: "the handle has no class: it is not a handle".to_string(),
    })?;
    let kind = HandleKind::parse(prefix).ok_or_else(|| CursorError {
        kind: CursorErrorKind::Garbled,
        reason: format!("`{prefix}` is not a class of handle this server speaks"),
    })?;
    let signed = cursor::verify_signed(token, ctx)?;
    Ok(VerifiedHandle {
        kind,
        payload: signed.payload,
    })
}

/// Resuelve los handles que viajan en los argumentos.
///
/// Un handle es una promesa firmada contra este workspace y esta
/// instantánea. Uno que fue reescrito, o que pertenece a otro árbol, se
/// rechaza con su error tipado — **antes** de que la tool corra, no
/// después.
pub fn resolve_handles(
    arguments: &mut Map<String, Value>,
    context: &HandleContext,
) -> Result<(), CursorError> {
    for value in arguments.values_mut() {
        let Some(text) = value.as_str().map(str::to_string) else {
            continue;
        };
        let Some((prefix, _)) = text.split_once(':') else {
            continue;
        };
        if HandleKind::parse(prefix).is_none() {
            continue;
        }
        *value = resolve_handle(&Value::String(text), context)?;
    }
    Ok(())
}

/// Resuelve un valor que puede ser un handle.
///
/// Un valor que no es un handle se devuelve tal cual: la resolución no
/// obliga a nada.
pub fn resolve_handle(value: &Value, ctx: &HandleContext) -> Result<Value, CursorError> {
    let Some(text) = value.as_str() else {
        return Ok(value.clone());
    };
    if !text.contains(':') {
        return Ok(value.clone());
    }
    let Some((prefix, _)) = text.split_once(':') else {
        return Ok(value.clone());
    };
    if HandleKind::parse(prefix).is_none() {
        return Ok(value.clone());
    }
    Ok(verify_handle(text, ctx)?.payload)
}

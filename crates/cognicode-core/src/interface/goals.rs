//! El registro de goals: una declaración, cuatro superficies.
//!
//! Un goal es lo que un consumidor **pide** (`status`, `find`, `trace`), no
//! lo que el producto **calcula** (`find_usages`, `trace_path`). Hoy esas dos
//! cosas son la misma, y por eso un nombre de tool es un nombre de propósito
//! y una tool nueva obliga a elegir un nombre nuevo sin que nadie sepa qué
//! nombres existen.
//!
//! El registro separa las dos. Cada goal declara **una vez** su nombre, su
//! título, su descripción con ejemplos, su autoridad, su fase mínima, su
//! dueño y sus argumentos; y de esa declaración salen las cuatro superficies
//! por las que un consumidor puede ver al goal:
//!
//! | Superficie | Quién la ve | De dónde sale |
//! |---|---|---|
//! | el esquema MCP | el cliente MCP | [`mcp_descriptor`] |
//! | el comando clap | quien escribe en la terminal | `clap_command` (en `cognicode-cli`) |
//! | la entrada de `product/tools.json` | quien lee el catálogo | [`tools_json_entry`] |
//! | la ayuda | quien escribe `cognicode --help` | [`help_text`] |
//!
//! ## Los argumentos tienen un dueño
//!
//! El struct que lee los argumentos **es** la declaración de los
//! argumentos: de él se deriva el `inputSchema` que el MCP publica, y ese
//! mismo esquema es el que el comando clap expone y el que el catálogo publica.
//! El registro no vuelve a escribir los argumentos: si los escribiera, el
//! esquema tendría dos dueños y podría mentir.
//!
//! ## Lo que este registro NO es
//!
//! - **No es el mecanismo de publicación.** El registro declara los goals;
//!   qué se publica en cada cable lo decide
//!   [`crate::interface::surface`] (los perfiles `default`, `full` y
//!   `legacy`).
//! - **No retira ninguna tool publicada.** Un goal se publica *además* de las
//!   tools en el perfil `full`, y en el perfil `legacy` las tools se
//!   publican como alias con el aviso que nombra al goal que las sustituye.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

use crate::interface::catalog::ToolDescriptor;

/// Lo que un goal puede hacer con el workspace.
///
/// El vocabulario es el del catálogo (`read` | `mutating`), no uno nuevo: un
/// cliente que ya lee la autoridad de una tool lee aquí la misma palabra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalAuthority {
    /// Sólo lee.
    Read,
    /// Escribe en el workspace.
    Mutating,
}

impl GoalAuthority {
    /// La palabra que el catálogo y el cable ya usan.
    pub fn as_str(self) -> &'static str {
        match self {
            GoalAuthority::Read => "read",
            GoalAuthority::Mutating => "mutating",
        }
    }
}

/// La fase del producto en la que un goal ya puede responder.
///
/// Las fases son las del producto (`discover`, `index`, `link`, `analyze`):
/// un goal declara la más temprana en la que su respuesta es verdad, y no
/// una más temprana — un goal que se apoya en el grafo no puede responder en
/// `discover` aunque el fichero esté ahí.
///
/// El orden de las fases es el del producto, y el perfil por defecto lo usa:
/// publica los goals que responden antes del análisis completo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GoalPhase {
    /// Se ven los ficheros y los símbolos que declaran.
    Discover,
    /// El índice responde: los símbolos se buscan sin recorrer el árbol.
    Index,
    /// Las aristas están unidas: se sabe quién llama a quién.
    Link,
    /// El análisis completo responde: impacto, calidad y caminos.
    Analyze,
}

impl GoalPhase {
    /// Las fases del producto, en orden.
    pub const ALL: [GoalPhase; 4] = [
        GoalPhase::Discover,
        GoalPhase::Index,
        GoalPhase::Link,
        GoalPhase::Analyze,
    ];

    /// El nombre de la fase.
    pub fn as_str(self) -> &'static str {
        match self {
            GoalPhase::Discover => "discover",
            GoalPhase::Index => "index",
            GoalPhase::Link => "link",
            GoalPhase::Analyze => "analyze",
        }
    }

    /// Lo que la fase significa, en una frase.
    pub fn description(self) -> &'static str {
        match self {
            GoalPhase::Discover => "Se ven los ficheros del workspace y los símbolos que declaran.",
            GoalPhase::Index => {
                "El índice responde: los símbolos se buscan por nombre sin recorrer el árbol."
            }
            GoalPhase::Link => "Las aristas están unidas: se sabe quién llama a quién.",
            GoalPhase::Analyze => {
                "El análisis completo responde: impacto, calidad y caminos calientes."
            }
        }
    }
}

/// El contexto en el que un goal se ejecuta.
///
/// Un goal necesita dos cosas del mundo: el workspace que analiza y **dónde
/// guarda el producto lo que deriva de él**. Las dos vienen aquí, así que el
/// goal no adivina ninguna de las dos y la terminal y el cable le dan el
/// mismo contexto.
///
/// La caché es `None` cuando el entorno no dice dónde hay una: el producto
/// entonces no guarda nada en vez de escribir en un sitio inventado.
#[derive(Debug, Clone, Copy)]
pub struct GoalEnvironment<'a> {
    /// El workspace que se analiza.
    pub workspace: &'a Path,
    /// Dónde guarda el producto lo que deriva del workspace.
    pub cache_root: Option<&'a Path>,
}

impl<'a> GoalEnvironment<'a> {
    /// El contexto de un goal.
    pub fn new(workspace: &'a Path, cache_root: Option<&'a Path>) -> Self {
        Self {
            workspace,
            cache_root,
        }
    }
}

/// Un argumento de un goal, tal y como el esquema lo publica.
///
/// El tipo lo produce el esquema derivado del struct que lee los
/// argumentos; no se escribe a mano, para que el nombre, la descripción y
/// la obligatoriedad tengan un solo dueño.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalArgument {
    /// El nombre del argumento.
    pub name: String,
    /// Lo que hace el argumento, en una frase.
    pub description: String,
    /// Si el llamante tiene que mandarlo.
    pub required: bool,
}

/// La declaración de un goal: el único sitio donde vive.
///
/// Los campos son estáticos a propósito: la declaración se compila con el
/// binario, así que un goal no puede aparecer en el catálogo sin que el
/// código que lo sirve esté en el mismo binario.
#[derive(Debug, Clone)]
pub struct GoalSpec {
    /// El nombre del goal: el que el consumidor escribe.
    pub name: &'static str,
    /// El título corto, para las listas.
    pub title: &'static str,
    /// La descripción, con un ejemplo dentro.
    pub description: &'static str,
    /// Los ejemplos de uso, tal y como los escribe el consumidor.
    pub examples: &'static [&'static str],
    /// Lo que el goal puede hacer con el workspace.
    pub authority: GoalAuthority,
    /// La fase más temprana en la que el goal ya responde.
    pub minimum_phase: GoalPhase,
    /// Quién responde por el goal.
    pub owner: &'static str,
    /// Si el trabajo del goal incluye el grafo del workspace.
    ///
    /// La declaración decide la ventana del despacho: una herramienta que
    /// declara `requires_graph` recibe la ventana del grafo, porque su
    /// trabajo incluye construirlo. Un goal que construye el grafo y dice
    /// que no lo necesita recibe el plazo por defecto, y ese plazo depende
    /// del tamaño del árbol —en este repositorio el grafo tarda 28 s en
    /// construirse, y el plazo por defecto son 30 s.
    ///
    /// La verdad la tiene el trabajo del goal, no la declaración: lo que
    /// hace cada goal se mide (una llamada en frío por goal, con su caché
    /// propia) en `a_goal_declares_the_graph_it_needs`, y esta declaración
    /// tiene que coincidir con esa medida.
    pub requires_graph: bool,
    /// Los nombres legados que este goal sustituye.
    ///
    /// El goal no borra un nombre publicado: lo declara su sustituto, y el
    /// perfil `legacy` publica el nombre viejo con el aviso que nombra a
    /// este goal. La lista la escribe el goal, no el perfil: el perfil
    /// derivaría de ella, así que un nombre puede tener un sustituto
    /// declarado y ninguno inventado.
    pub replaces: &'static [&'static str],
    /// La implementación del goal: lee sus argumentos y devuelve su respuesta.
    ///
    /// El goal se implementa una vez y las cuatro superficies la comparten:
    /// el MCP la llama con el contexto de la sesión y los argumentos del
    /// cable; el CLI la llama con el directorio actual y los argumentos que
    /// el comando clap leyó.
    pub run: fn(&GoalEnvironment, &Value) -> Result<Value, String>,
    /// La respuesta del goal en texto.
    ///
    /// El texto lo compone el goal, no el comando: el MCP no lo publica, pero
    /// la terminal sí, y las dos formas de la respuesta tienen que salir de la
    /// misma declaración.
    pub render: fn(&Value) -> String,
}

/// El goal `status`: el estado del producto y del workspace.
pub const STATUS: GoalSpec = GoalSpec {
    name: "status",
    title: "El estado del producto y del workspace",
    description: "Di qué ofrece este build y qué se ve del workspace: las fases del \
producto, los lenguajes detectados, la cobertura de la lectura y los goals con sus \
plantillas de argumentos. Un cliente lo llama antes de decidir; un humano lo llama \
para saber si el workspace está donde cree. Ejemplos: `cognicode status`, \
`cognicode status --format json`.",
    examples: &["cognicode status", "cognicode status --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Discover,
    owner: "interface::goals::status",
    requires_graph: false,
    replaces: &[],
    run: status::run,
    render: status::render,
};

/// El goal `context`: el workspace como contexto de trabajo.
pub const CONTEXT: GoalSpec = GoalSpec {
    name: "context",
    title: "El workspace como contexto de trabajo",
    description: "Di qué hay en el workspace: los ficheros que lo componen, los \
lenguajes que se detectan y la cobertura de la lectura. Es el goal que se llama \
antes de tocar nada. Ejemplos: `cognicode context`, `cognicode context --format \
json`.",
    examples: &["cognicode context", "cognicode context --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Discover,
    owner: "interface::goals::context",
    requires_graph: false,
    replaces: &["list_files"],
    run: context::run,
    render: context::render,
};

/// El goal `find`: dónde está un símbolo.
pub const FIND: GoalSpec = GoalSpec {
    name: "find",
    title: "Dónde está un símbolo",
    description: "Resuelve un nombre de símbolo a los sitios donde está declarado, \
con su fichero y su línea. El nombre se compara como lo lee el lenguaje de cada \
declaración: en un lenguaje que distingue mayúsculas, `Ok` y `ok` son dos nombres, \
y en uno que las pliega —PHP, PowerShell y Fortran— son el mismo. La respuesta dice \
con qué comparación encontró las declaraciones: `exact` cuando todas están \
escritas como el nombre buscado, y `case-insensitive` cuando alguna contesta sólo \
porque el lenguaje que la escribió pliega la grafía. Es la pregunta que se hace un \
homónimo: el mismo nombre puede estar en dos ficheros. Ejemplos: `cognicode find \
--symbol helper`, `cognicode find --symbol helper --format json`.",
    examples: &[
        "cognicode find --symbol helper",
        "cognicode find --symbol helper --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Index,
    owner: "interface::goals::find",
    requires_graph: true,
    replaces: &["query_symbol_index", "find_references"],
    run: find::run,
    render: find::render,
};

/// El goal `inspect`: qué declara un fichero.
pub const INSPECT: GoalSpec = GoalSpec {
    name: "inspect",
    title: "Qué declara un fichero",
    description: "Lee un fichero y dice qué símbolos declara, con su tipo y su línea, o \
—con `--revision`— los hechos que se ensamblan de una revisión Git y su cobertura. \
No necesita el grafo: parsea el fichero que se le nombra. Se da exactamente uno de \
`--file` y `--revision`. Ejemplos: `cognicode inspect --file src/lib.rs`, \
`cognicode inspect --revision HEAD --format json`.",
    examples: &[
        "cognicode inspect --file src/lib.rs",
        "cognicode inspect --file src/lib.rs --format json",
        "cognicode inspect --revision HEAD --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Discover,
    owner: "interface::goals::inspect",
    requires_graph: false,
    replaces: &["get_file_symbols", "get_symbol_code"],
    run: inspect::run,
    render: inspect::render,
};

/// El goal `trace`: quién llama a un símbolo y a quién llama.
pub const TRACE: GoalSpec = GoalSpec {
    name: "trace",
    title: "Quién llama a un símbolo",
    description: "Sigue el grafo de llamadas alrededor de un símbolo: quién lo llama, \
a quién llama y a qué distancia. Ejemplos: `cognicode trace --symbol helper`, \
`cognicode trace --symbol helper --direction callers`.",
    examples: &[
        "cognicode trace --symbol helper",
        "cognicode trace --symbol helper --direction callers",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Link,
    owner: "interface::goals::trace",
    requires_graph: true,
    replaces: &["get_call_hierarchy", "trace_path"],
    run: trace::run,
    render: trace::render,
};

/// El goal `impact`: qué rompe un cambio.
pub const IMPACT: GoalSpec = GoalSpec {
    name: "impact",
    title: "Qué rompe un cambio",
    description: "Dice a qué símbolos y a qué ficheros afecta cambiar un símbolo, y \
con qué riesgo. Es la pregunta que se hace antes de tocar nada. Ejemplos: \
`cognicode impact --symbol helper`, `cognicode impact --symbol helper --format json`.",
    examples: &[
        "cognicode impact --symbol helper",
        "cognicode impact --symbol helper --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::impact",
    requires_graph: true,
    replaces: &["analyze_impact"],
    run: impact::run,
    render: impact::render,
};

/// El goal `overview`: cómo es el proyecto.
pub const OVERVIEW: GoalSpec = GoalSpec {
    name: "overview",
    title: "Cómo es el proyecto",
    description: "Resume el proyecto que hay en el workspace: cuántos símbolos tiene, \
cuántas aristas, cuántos ficheros y en qué lenguajes. Ejemplos: \
`cognicode overview`, `cognicode overview --format json`.",
    examples: &["cognicode overview", "cognicode overview --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::overview",
    requires_graph: true,
    replaces: &["project_overview", "codebase_map"],
    run: overview::run,
    render: overview::render,
};

/// El goal `audit`: qué está mal en el proyecto.
pub const AUDIT: GoalSpec = GoalSpec {
    name: "audit",
    title: "Qué está mal en el proyecto",
    description: "Mira el proyecto y dice qué encuentra: el código que no llega a \
usarse y los ciclos de llamadas. Ejemplos: `cognicode audit`, `cognicode audit \
--format json`.",
    examples: &["cognicode audit", "cognicode audit --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::audit",
    requires_graph: true,
    replaces: &["detect_god_functions", "solid_audit"],
    run: audit::run,
    render: audit::render,
};

/// El goal `compare`: qué cambió entre dos revisiones.
pub const COMPARE: GoalSpec = GoalSpec {
    name: "compare",
    title: "Qué cambió entre dos revisiones",
    description: "Compara los ficheros versionados entre dos revisiones Git ascendentes y publica por separado los cambios staged HEAD→index y las rutas dirty/untracked del working tree (sin afirmar contenido no leído). Ensambla los hechos de cada revisión (y guarda su snapshot y el read-set del detector sobre la base si hay caché) y publica el impacto y la reutilización de trabajo previo que esos hechos y los read-sets guardados sostienen: `evaluated`, `partial` (cota inferior) o `not_evaluated` con su motivo. Publica además los hallazgos de cada detector que escribe read-set (definiciones de función, definiciones de tipo y buckets S3 sin bloqueo de acceso público) sobre la revisión candidata (`head_findings`, una entrada por detector) con su estado `reused` (solo si está probado que ningún cambio toca lo que el detector leyó; la evidencia sigue siendo de la base) o `rerun`, y el motivo; sin caché o sin read-set es siempre `rerun`. Ejemplo: `cognicode compare --base-ref main --head-ref HEAD`.",
    examples: &["cognicode compare --base-ref main --head-ref HEAD"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::compare",
    requires_graph: false,
    replaces: &[],
    run: compare::run,
    render: compare::render,
};

/// El goal `change`: el cambio que se va a hacer.
pub const CHANGE: GoalSpec = GoalSpec {
    name: "change",
    title: "El cambio que se va a hacer",
    description: "Calcula el cambio de un símbolo a otro nombre: los ficheros que \
tocaría, los símbolos que afecta y con qué riesgo. Es el goal que escribe, y por \
eso no aparece en el modo de sólo lectura. Ejemplos: `cognicode change --symbol \
helper --new-name helper2`.",
    examples: &[
        "cognicode change --symbol helper --new-name helper2",
        "cognicode change --symbol helper --new-name helper2 --format json",
    ],
    authority: GoalAuthority::Mutating,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::change",
    requires_graph: true,
    replaces: &["safe_refactor"],
    run: change::run,
    render: change::render,
};

/// El goal `recipe`: una pregunta compuesta que el producto sabe responder.
///
/// Una receta son **varios goals en un orden**, con lo que se puede
/// reutilizar de la corrida anterior. La respuesta dice qué corrió, qué vino
/// de la caché, cuánto costó y, si algo falló, qué falló y por qué.
///
/// El goal es de sólo lectura: una receta **no** escribe en el workspace, y
/// una receta del workspace tampoco puede invocar un goal que escriba.
pub const RECIPE: GoalSpec = GoalSpec {
    name: "recipe",
    title: "Una receta de goals, con su caché y su presupuesto",
    description: "Corre una receta: varios goals en el orden de sus fases, \
memoizados por lo que el árbol ha cambiado, con los pasos independientes en \
paralelo. Con `plan: true` dice qué correría, qué está en caché y lo que \
costaría, sin ejecutar nada. Ejemplos: `cognicode recipe --name onboard`, \
`cognicode recipe --name pre-change --params '{\"symbol\":\"helper\"}'`, \
`cognicode recipe --name onboard --plan`.",
    examples: &[
        "cognicode recipe --name onboard",
        "cognicode recipe --name pre-change --params '{\"symbol\":\"helper\"}'",
        "cognicode recipe --name onboard --plan",
    ],
    authority: GoalAuthority::Read,
    // La receta necesita el análisis completo: sus pasos son goals de las
    // cuatro fases, y el último de ellos no responde antes de `analyze`.
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::recipe",
    requires_graph: true,
    replaces: &[],
    run: recipe::run,
    render: recipe::render,
};

/// El goal `verify`: el snapshot guardado de una revisión frente a su reconstrucción.
pub const VERIFY: GoalSpec = GoalSpec {
    name: "verify",
    title: "El snapshot guardado de una revisión frente a una reconstrucción completa",
    description: "Reconstruye desde cero los hechos de una revisión Git y los compara con el snapshot guardado para ella; no escribe nada. Responde `verified`, `mismatch`, `partial` (faltan hechos de algún fichero analizable) o `not_evaluated` (sin caché o sin snapshot guardado), y nunca presenta un estado incompleto como verificado. Ejemplo: `cognicode verify --revision HEAD`.",
    examples: &[
        "cognicode verify --revision HEAD",
        "cognicode verify --revision HEAD --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::verify",
    requires_graph: false,
    replaces: &[],
    run: verify::run,
    render: verify::render,
};

/// El goal `security`: el pack de seguridad sobre Terraform y Helm.
pub const SECURITY: GoalSpec = GoalSpec {
    name: "security",
    title: "El pack de seguridad sobre Terraform y Helm",
    description: "Corre el pack de seguridad sobre las raíces Terraform y los charts Helm del proyecto y publica un veredicto por unidad y familia: `found`, `not_observed`, `unknown` o `unsupported`, siempre distintos (un `unknown` o `unsupported` nunca se presenta como limpio). Para Helm dice si el chart se renderizó y con qué cobertura; sin el motor `helm` el chart queda `not_rendered` y sus comprobaciones `unknown`. Publica la matriz de capacidades con sus límites (`all_languages` la amplía a todos los lenguajes). Es un informe de candidatos, no una puerta. Ejemplos: `cognicode security`, `cognicode security --format json`.",
    examples: &["cognicode security", "cognicode security --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::security",
    requires_graph: false,
    replaces: &[],
    run: security::run,
    render: security::render,
};

/// The `preflight` goal: the facts delta and cost of in-memory edits.
pub const PREFLIGHT: GoalSpec = GoalSpec {
    name: "preflight",
    title: "The facts delta and cost of edits over a revision, without writing",
    description: "Lay in-memory edits over a Git revision and report the facts they would add and remove and what producing them costs, before anything is written. `edits` maps a repository-relative path to the complete new text of the file, or to null to delete it; `base_ref` defaults to HEAD. The workspace, the fact store and the revision snapshots are never written: the candidate revision exists only in the answer, under a handle scoped to `session`, and it is never a real revision handle. The delta reports the same `impact_status` as `compare`: a file no reader claims, or one that cannot be parsed, makes it `partial`. Edits are whole-file text. Example: `cognicode preflight --edits '{\"src/lib.rs\":\"fn a() {}\\n\"}'`.",
    examples: &[
        "cognicode preflight --edits '{\"src/lib.rs\":\"fn a() {}\\n\"}'",
        "cognicode preflight --base-ref main --edits '{\"src/old.rs\":null}' --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::preflight",
    requires_graph: false,
    replaces: &[],
    run: preflight::run,
    render: preflight::render,
};

/// The `dependencies` goal: the resolved Cargo dependency inventory of a revision.
pub const DEPENDENCIES: GoalSpec = GoalSpec {
    name: "dependencies",
    title: "The resolved Cargo dependency inventory of a revision",
    description: "Read the tracked Cargo.toml and Cargo.lock files of a Git revision, offline, and report per workspace unit the packages the lockfile pins (version, source, SHA-256 checksum or pinned Git commit, Package URL identity), the member declarations that asked for them (kind, target, requirement, features) and whether the lockfile agrees with the manifests: `consistent`, `stale` (a declaration, member or edge disagrees), `partial` (some requirement or file could not be checked), `no_lockfile` or `no_manifest`. The answer is anchored to the revision and to the digest of the lockfile bytes, and states which of the declared, resolved, observed and distributed layers were collected: observed and distributed never are. `dependents_of` (`name` or `name@version`) lists the transitive dependents and the workspace members they affect; `base_ref` adds the packages added, removed and updated in the lockfile since that revision; `path` restricts the units. Cargo, build scripts and the network are never run, so resolved features and the kind of edges between non-member packages are not reported. Example: `cognicode dependencies --dependents-of serde`.",
    examples: &[
        "cognicode dependencies",
        "cognicode dependencies --dependents-of serde --format json",
        "cognicode dependencies --base-ref main",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::dependencies",
    requires_graph: false,
    replaces: &[],
    run: dependencies::run,
    render: dependencies::render,
};

/// El goal `pack`: los símbolos que responden a una petición, con su evidencia.
pub const PACK: GoalSpec = GoalSpec {
    name: "pack",
    title: "Los símbolos que responden a una petición, con evidencia",
    description: "Devuelve un paquete de contexto de símbolos: para cada uno su identidad \
única, el rango de fuente, el digest del fichero, el motivo por el que se eligió y la \
instantánea sobre la que se calculó. Un nombre que varios símbolos declaran se reporta \
como ambigüedad y no se resuelve; se pasa el id del símbolo. El presupuesto \
(`--max-tokens`) acota el texto que se renderiza y se mide sobre ese mismo texto: con \
un tokenizador configurado el recuento es exacto para él, y sin él es una cota \
superior declarada, nunca exacta. El paquete dice qué entregó y qué dejó fuera, y no \
afirma cobertura semántica: nadie la evalúa. Si no cabe todo, devuelve un \
`continuation_token` para la página siguiente de la misma instantánea. Ejemplos: \
`cognicode pack --symbols src/lib.rs:helper:10`, `cognicode pack --intent \"parse \
config\" --max-tokens 800`.",
    examples: &[
        "cognicode pack --symbols src/lib.rs:helper:10",
        "cognicode pack --intent \"parse config\" --max-tokens 800",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Analyze,
    owner: "interface::goals::pack",
    requires_graph: true,
    replaces: &[],
    run: pack::run,
    render: pack::render,
};

/// El registro: los goals que este build declara.
static REGISTRY: [GoalSpec; 16] = [
    STATUS,
    CONTEXT,
    FIND,
    INSPECT,
    TRACE,
    IMPACT,
    OVERVIEW,
    AUDIT,
    COMPARE,
    VERIFY,
    CHANGE,
    RECIPE,
    SECURITY,
    PREFLIGHT,
    DEPENDENCIES,
    PACK,
];

/// Los goals declarados, en orden de registro.
pub fn registry() -> &'static [GoalSpec] {
    &REGISTRY
}

/// El goal con ese nombre, si el registro lo declara.
pub fn find(name: &str) -> Option<&'static GoalSpec> {
    REGISTRY.iter().find(|goal| goal.name == name)
}

/// El `inputSchema` del goal: el derivado del struct que lee sus argumentos.
///
/// Es el mismo esquema que publica el MCP y el mismo que describe los
/// argumentos del comando clap; el registro no lo reescribe.
pub fn input_schema(goal: &GoalSpec) -> Map<String, Value> {
    match goal.name {
        "status" => crate::interface::schema::schema_of::<status::StatusInput>(),
        "context" => crate::interface::schema::schema_of::<context::ContextInput>(),
        "find" => crate::interface::schema::schema_of::<find::FindInput>(),
        "inspect" => crate::interface::schema::schema_of::<inspect::InspectInput>(),
        "trace" => crate::interface::schema::schema_of::<trace::TraceInput>(),
        "impact" => crate::interface::schema::schema_of::<impact::ImpactInput>(),
        "overview" => crate::interface::schema::schema_of::<overview::OverviewInput>(),
        "audit" => crate::interface::schema::schema_of::<audit::AuditInput>(),
        "compare" => crate::interface::schema::schema_of::<compare::CompareInput>(),
        "verify" => crate::interface::schema::schema_of::<verify::VerifyInput>(),
        "change" => crate::interface::schema::schema_of::<change::ChangeInput>(),
        "recipe" => crate::interface::schema::schema_of::<recipe::RecipeInput>(),
        "security" => crate::interface::schema::schema_of::<security::SecurityInput>(),
        "preflight" => crate::interface::schema::schema_of::<preflight::PreflightInput>(),
        "dependencies" => crate::interface::schema::schema_of::<dependencies::DependenciesInput>(),
        "pack" => crate::interface::schema::schema_of::<pack::PackInput>(),
        _ => Map::new(),
    }
}

/// Los argumentos del goal, tal y como los publica su esquema.
pub fn arguments(goal: &GoalSpec) -> Vec<GoalArgument> {
    let schema = input_schema(goal);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = required_names(&schema);

    properties
        .into_iter()
        .map(|(name, property)| GoalArgument {
            description: property
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            required: required.contains(&name.as_str()),
            name,
        })
        .collect()
}

/// La ayuda del goal: su descripción y sus ejemplos.
///
/// Es el `long_about` del comando clap, así que la ayuda y el comando no
/// pueden decir cosas distintas.
pub fn help_text(goal: &GoalSpec) -> String {
    let mut text = String::from(goal.description);
    text.push_str("\n\nEjemplos:\n");
    for example in goal.examples {
        text.push_str("  ");
        text.push_str(example);
        text.push('\n');
    }
    text
}

/// El descriptor MCP del goal: la forma que el catálogo y el cable comparten.
///
/// Se deriva de la misma entrada que publica `product/tools.json`, así que
/// el descriptor y la entrada del catálogo no pueden discrepar.
pub fn mcp_descriptor(goal: &GoalSpec) -> ToolDescriptor {
    serde_json::from_value(tools_json_entry(goal))
        .expect("the derived goal entry is a valid tool descriptor")
}

/// La entrada que el goal ocupa en `product/tools.json`.
///
/// Los campos espejo (`input_schema`, `runtime_metadata`) son los que publica
/// el runtime; los demás los declara el goal. El maestro se escribe a mano,
/// pero sus bloques espejo tienen que ser estos: un check lo compara byte a
/// byte.
///
/// ## Por qué los goals no publican `output_schema`
///
/// MEDIDO el 2026-11-07 sobre este árbol: la lista del perfil por defecto
/// —los cinco goals, sin `outputSchema`— pesa **4.290 bytes** (4.350 por el
/// cable, con las tres features). El mismo esquema del sobre, publicado bajo
/// cada goal, la dejaría entre **26.420** (con el contrato más pequeño que el
/// producto publica) y **65.890** bytes (con el más grande), y el presupuesto
/// es de 6 KB. El goal no anuncia un `outputSchema`: lo que anuncia es **qué
/// pregunta responde** (su `inputSchema`) y **cómo se llama**. El sobre es el
/// mismo para todos.
///
/// El maestro y el runtime tienen que decir lo mismo sobre esto, y lo
/// comprueba `a012_structured_output`.
pub fn tools_json_entry(goal: &GoalSpec) -> Value {
    let mutates = goal.authority == GoalAuthority::Mutating;
    let mut entry = serde_json::json!({
        "name": goal.name,
        "title": goal.title,
        "description": goal.description,
        "category": "goal",
        "authority": goal.authority.as_str(),
        "stability": "stable",
        "estimated_latency_ms": 30,
        "requirements": {
            "graph": goal.requires_graph,
            "persistence": false,
            "cache": null,
            "network": null
        },
        "requirements_status": "not_declared",
        "public_profiles": ["core", "reviewer"],
        "runtime_metadata": {
            "stability": "stable",
            "category": "goal",
            "requires_graph": goal.requires_graph,
            "requires_persistence": false,
            "estimated_latency_ms": 30,
            "authority": goal.authority.as_str(),
            "mutates_workspace": mutates,
            "tool_version": env!("CARGO_PKG_VERSION")
        },
        "input_schema": Value::Object(input_schema(goal)),
        // El campo existe y vale `null`: el maestro declara los trece
        // campos de cada entrada, y `null` es la respuesta honesta para
        // «no hay contrato» — la misma que dan las tools sin esquema.
        "output_schema": Value::Null,
        "precision_floor": "ast-heuristic",
        "required_args": required_args(goal),
        "limitations": limitations(goal)
    });

    // Un campo que no tiene nada que decir no se publica: un
    // `required_args` vacío dice «no hay argumentos obligatorios» y una
    // lista vacía dice lo mismo de otra manera, y el maestro prefiere la
    // ausencia (es lo que hacen las tools sin argumentos obligatorios).
    if required_args(goal).is_empty() {
        entry
            .as_object_mut()
            .expect("the derived entry is an object")
            .remove("required_args");
    }

    entry
}

/// El `outputSchema` del goal: **ninguno**.
///
/// Los goals no publican un `outputSchema` (el porqué está en
/// [`tools_json_entry`]): el perfil por defecto tiene que caber en 6 KB, y
/// medido el 2026-11-07 sobre este árbol, el esquema del sobre bajo cada
/// goal la dejaría entre 26.420 y 65.890 bytes. La función se queda para
/// quien pregunte, y contesta la verdad: un mapa vacío.
pub fn output_schema(_goal: &GoalSpec) -> Map<String, Value> {
    Map::new()
}

/// Los argumentos que el llamante tiene que mandar.
fn required_args(goal: &GoalSpec) -> Vec<String> {
    arguments(goal)
        .into_iter()
        .filter(|argument| argument.required)
        .map(|argument| argument.name)
        .collect()
}

/// Lo que el goal dice que no hace, en sus propias palabras.
///
/// Un goal que escribe tiene que decirlo: quien lee la lista de
/// `tools/list` tiene que saber, **antes** de llamar, que la llamada toca
/// el workspace. Un goal que sólo lee dice lo otro: que su respuesta no
/// añade un cálculo al que el dueño ya hacía.
///
/// Y un goal que necesita el grafo tiene que decir que lo necesita: la
/// limitación es lo que un cliente lee **antes** de llamar, y sin ella
/// llama a un goal que todavía no puede responder.
fn limitations(goal: &GoalSpec) -> Vec<String> {
    let mut limitations = if goal.authority == GoalAuthority::Mutating {
        vec![
            "El goal tiene autoridad de escritura sobre el workspace: es el único \
             que puede llegar a escribir, y por eso no se anuncia en el modo de \
             sólo lectura. Hoy calcula el cambio y lo publica; aplicarlo es una \
             acción aparte."
                .to_string(),
        ]
    } else if goal.name == "compare" {
        vec![
            "The goal reports committed SCM changes, staged HEAD-to-index changes, and dirty/untracked working-tree paths. It does not read dirty file contents. Impact and prior-work reuse come from committed revision facts and stored execution read-sets: they are partial when facts are missing for some file and not_evaluated when there is nothing to compare. The head findings come from the three detectors that write read-sets, function definitions, type definitions and unprotected S3 buckets, one entry each. The other detectors do not write read-sets yet (the security pack, the dataflow detectors, the detectors imported from legacy rules, and the legacy quality rules), so their findings are not published here and their absence is not a clean result. Head findings are reused only when proven unaffected; with no cache or no read-set for the base they are always a rerun, and over a partial candidate they are a lower bound."
                .to_string(),
        ]
    } else if goal.name == "dependencies" {
        vec![
            "Reads tracked Cargo.toml and Cargo.lock bytes offline: resolved features and the kind of edges between non-member packages are not reported."
                .to_string(),
            "The observed and distributed layers are not collected; only the Cargo ecosystem is read."
                .to_string(),
        ]
    } else if goal.name == "pack" {
        vec![
            "The selection is by identity, name weight and direct relation: a pack that delivered every candidate is still not a statement about the code it did not select."
                .to_string(),
            "The budget measures the rendered text, not the structured answer."
                .to_string(),
        ]
    } else {
        vec![
            "El goal se monta sobre el dueño que ya calcula esto: su respuesta no \
             añade un cálculo nuevo."
                .to_string(),
        ]
    };
    if goal.requires_graph {
        limitations.push(
            "El goal trabaja sobre el grafo del workspace: necesita que `build_graph` \
             lo tenga construido y al día."
                .to_string(),
        );
    }
    limitations
}

/// Los nombres de los argumentos que el llamante tiene que mandar.
pub fn required_names(schema: &Map<String, Value>) -> Vec<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// El goal como lo publica la respuesta de `status`: su identidad, su
/// autoridad y sus argumentos.
pub fn goal_info(goal: &GoalSpec) -> GoalInfo {
    GoalInfo {
        name: goal.name.to_string(),
        title: goal.title.to_string(),
        description: goal.description.to_string(),
        examples: goal.examples.iter().map(|e| e.to_string()).collect(),
        authority: goal.authority.as_str().to_string(),
        minimum_phase: goal.minimum_phase.as_str().to_string(),
        owner: goal.owner.to_string(),
        arguments: arguments(goal),
    }
}

/// Un goal tal y como lo describe la respuesta de `status`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalInfo {
    /// El nombre del goal.
    pub name: String,
    /// El título corto.
    pub title: String,
    /// La descripción del goal.
    pub description: String,
    /// Los ejemplos de uso.
    pub examples: Vec<String>,
    /// Lo que el goal puede hacer: `read` o `mutating`.
    pub authority: String,
    /// La fase más temprana en la que el goal ya responde.
    pub minimum_phase: String,
    /// Quién responde por el goal.
    pub owner: String,
    /// Los argumentos del goal y su plantilla.
    pub arguments: Vec<GoalArgument>,
}

pub mod audit;
pub mod change;
pub mod common;
pub mod compare;
pub mod context;
pub mod dependencies;
pub mod find;
pub mod impact;
pub mod inspect;
pub mod overview;
pub mod pack;
pub mod preflight;
pub mod recipe;
mod revision_support;
pub mod security;
pub mod status;
pub mod trace;
pub mod verify;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_declares_the_goal_with_its_whole_declaration() {
        let goal = find("status").expect("the registry declares the goal `status`");
        assert!(!goal.title.is_empty(), "the goal has no title");
        assert!(
            goal.description.contains("Ejemplos:") || goal.description.contains("ejemplo"),
            "the description carries no example: {}",
            goal.description
        );
        assert!(!goal.examples.is_empty(), "the goal declares no example");
        assert!(!goal.owner.is_empty(), "the goal has no owner");
    }

    #[test]
    fn every_phase_has_a_name_and_a_description() {
        for phase in GoalPhase::ALL {
            assert!(!phase.as_str().is_empty());
            assert!(!phase.description().is_empty());
        }
    }

    #[test]
    fn compare_goal_is_declared_as_a_read_only_revision_comparison() {
        let goal = find("compare").expect("the registry declares the compare goal");
        let schema = input_schema(goal);
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("compare publishes input properties");

        assert_eq!(goal.authority, GoalAuthority::Read);
        assert!(!goal.requires_graph);
        assert!(properties.contains_key("base_ref"));
        assert!(properties.contains_key("head_ref"));
    }
}

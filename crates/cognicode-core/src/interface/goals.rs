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
//! argumentos: de él se deriva el `inputSchema` que el MCP publica
//! ([`crate::interface::mcp::input_contracts`]), y ese mismo esquema es el
//! que el comando clap expone y el que el catálogo publica. El registro no
//! vuelve a escribir los argumentos: si los escribiera, el esquema tendría
//! dos dueños y podría mentir.
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
    replaces: &["list_files"],
    run: context::run,
    render: context::render,
};

/// El goal `find`: dónde está un símbolo.
pub const FIND: GoalSpec = GoalSpec {
    name: "find",
    title: "Dónde está un símbolo",
    description: "Resuelve un nombre de símbolo a los sitios donde está declarado, \
con su fichero y su línea. Es la pregunta que se hace un homónimo: el mismo \
nombre puede estar en dos ficheros. Ejemplos: `cognicode find --symbol helper`, \
`cognicode find --symbol helper --format json`.",
    examples: &[
        "cognicode find --symbol helper",
        "cognicode find --symbol helper --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Index,
    owner: "interface::goals::find",
    replaces: &["query_symbol_index", "find_references"],
    run: find::run,
    render: find::render,
};

/// El goal `inspect`: qué declara un fichero.
pub const INSPECT: GoalSpec = GoalSpec {
    name: "inspect",
    title: "Qué declara un fichero",
    description: "Lee un fichero y dice qué símbolos declara, con su tipo y su línea. \
No necesita el grafo: parsea el fichero que se le nombra. Ejemplos: \
`cognicode inspect --file src/lib.rs`, `cognicode inspect --file src/lib.rs \
--format json`.",
    examples: &[
        "cognicode inspect --file src/lib.rs",
        "cognicode inspect --file src/lib.rs --format json",
    ],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Discover,
    owner: "interface::goals::inspect",
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
    replaces: &["detect_god_functions", "solid_audit"],
    run: audit::run,
    render: audit::render,
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
    replaces: &[],
    run: recipe::run,
    render: recipe::render,
};

/// El registro: los goals que este build declara.
static REGISTRY: [GoalSpec; 10] = [
    STATUS, CONTEXT, FIND, INSPECT, TRACE, IMPACT, OVERVIEW, AUDIT, CHANGE, RECIPE,
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
    crate::interface::mcp::input_contracts::input_schema_for(goal.name).unwrap_or_default()
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
            "graph": false,
            "persistence": false,
            "cache": null,
            "network": null
        },
        "requirements_status": "not_declared",
        "public_profiles": ["core", "reviewer"],
        "runtime_metadata": {
            "stability": "stable",
            "category": "goal",
            "requires_graph": false,
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
pub fn output_schema(goal: &GoalSpec) -> Map<String, Value> {
    crate::interface::mcp::output_contracts::output_schema_for(goal.name).unwrap_or_default()
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
fn limitations(goal: &GoalSpec) -> Vec<String> {
    if goal.authority == GoalAuthority::Mutating {
        vec![
            "El goal tiene autoridad de escritura sobre el workspace: es el único \
             que puede llegar a escribir, y por eso no se anuncia en el modo de \
             sólo lectura. Hoy calcula el cambio y lo publica; aplicarlo es una \
             acción aparte."
                .to_string(),
        ]
    } else {
        vec![
            "El goal se monta sobre el dueño que ya calcula esto: su respuesta no \
             añade un cálculo nuevo."
                .to_string(),
        ]
    }
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
pub mod context;
pub mod find;
pub mod impact;
pub mod inspect;
pub mod overview;
pub mod recipe;
pub mod status;
pub mod trace;

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
}

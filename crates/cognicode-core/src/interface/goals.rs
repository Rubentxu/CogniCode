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
//! | el comando clap | quien escribe en la terminal | [`clap_command`] |
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
//! - **No es el vocabulario de los nueve goals.** Los nueve goals (status,
//!   context, find, inspect, trace, impact, overview, audit, change) son de
//!   TASK-84; aquí se declara el mechanism y el primer goal montado.
//! - **No retira ninguna tool publicada.** Un goal se publica *además* de las
//!   tools, y las tools siguen siendo las que sirven hoy. Retirar un nombre
//!   publicado es otra unidad, con su contrato de deprecación.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// La implementación del goal: lee sus argumentos y devuelve su respuesta.
    ///
    /// El goal se implementa una vez y las cuatro superficies la comparten:
    /// el MCP la llama con el `workspace` de la sesión y los argumentos del
    /// cable; el CLI la llama con el directorio actual y los argumentos que
    /// el comando clap leyó.
    pub run: fn(&Path, &Value) -> Result<Value, String>,
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
producto, los lenguajes detectados y los goals con sus plantillas de argumentos. \
Un cliente lo llama antes de decidir; un humano lo llama para saber si el \
workspace está donde cree. Ejemplos: `cognicode status`, `cognicode status \
--format json`.",
    examples: &["cognicode status", "cognicode status --format json"],
    authority: GoalAuthority::Read,
    minimum_phase: GoalPhase::Discover,
    owner: "interface::goals::status",
    run: status::run,
    render: status::render,
};

/// El registro: los goals que este build declara.
static REGISTRY: [GoalSpec; 1] = [STATUS];

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
/// Los campos espejo (`input_schema`, `output_schema`, `runtime_metadata`)
/// son los que publica el runtime; los demás los declara el goal. El
/// maestro se escribe a mano, pero sus bloques espejo tienen que ser
/// estos: un check lo compara byte a byte.
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
        "output_schema": Value::Object(output_schema(goal)),
        "precision_floor": "ast-heuristic",
        "required_args": required_args(goal),
        "limitations": [
            "El estado es del producto y del workspace, no del análisis: la cobertura \
    del workspace todavía no se publica."
        ]
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

/// El `outputSchema` del goal: el derivado del tipo que serializa su respuesta.
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

/// El comando clap del goal.
///
/// El comando se construye aquí y no como una variante de un enum: un enum
/// lo escribiría una persona, y entonces el nombre del goal, su descripción
/// y sus argumentos vivirían en dos sitios. Aquí salen de la declaración.
///
/// Los tipos y los valores por defecto salen del esquema, que es el derivado
/// del struct que lee los argumentos: el flag de la terminal y el campo del
/// cable son el mismo argumento con dos nombres.
pub fn clap_command(goal: &GoalSpec) -> clap::Command {
    let schema = input_schema(goal);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = required_names(&schema);

    let mut command = clap::Command::new(goal.name)
        .about(goal.title)
        .long_about(help_text(goal))
        // El formato lo elige quien escribe, y lo elige en el comando: es
        // la convencion de los comandos que tienen su propia forma de
        // responder (`capabilities`, `doctor`, `find-usages`). El `--format`
        // de la raiz no los alcanza.
        .arg(
            clap::Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .help("Output format: `text` (default) or `json`.")
                .value_parser(["text", "json"])
                .default_value("text"),
        );

    for (name, property) in properties {
        let description = property
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut arg = clap::Arg::new(name.clone())
            // El flag se escribe con guiones, como los demas flags del
            // producto; el nombre del argumento (el que viaja por el cable)
            // sigue siendo el del esquema.
            .long(name.replace('_', "-"))
            .value_name(name.to_uppercase())
            .help(description)
            .value_parser(value_parser_for(&property));
        if required.contains(&name.as_str()) {
            arg = arg.required(true);
        }
        if let Some(default) = property.get("default") {
            arg = arg.default_value(default_value_for(default));
        }
        command = command.arg(arg);
    }

    command
}

/// Los argumentos que el comando clap leyó, como los lee el goal.
///
/// El tipo lo decide el esquema: el flag de la terminal y el campo del cable
/// son el mismo argumento con dos nombres, y el goal los lee con el mismo
/// struct en las dos superficies.
pub fn arguments_from_matches(goal: &GoalSpec, matches: &clap::ArgMatches) -> Value {
    let schema = input_schema(goal);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut object = serde_json::Map::new();
    for (name, property) in properties {
        let value = match argument_type(&property) {
            Some("boolean") => matches
                .try_get_one::<bool>(&name)
                .ok()
                .flatten()
                .map(|value| Value::Bool(*value)),
            Some("integer") => matches
                .try_get_one::<i64>(&name)
                .ok()
                .flatten()
                .map(|value| Value::from(*value)),
            Some("number") => matches
                .try_get_one::<f64>(&name)
                .ok()
                .flatten()
                .map(|value| Value::from(*value)),
            _ => matches
                .try_get_one::<String>(&name)
                .ok()
                .flatten()
                .map(|value| Value::String(value.clone())),
        };
        if let Some(value) = value {
            object.insert(name, value);
        }
    }

    Value::Object(object)
}

/// El tipo de un argumento, tal y como lo publica el esquema.
///
/// Un tipo puede ser una lista (`["string", "null"]`): el `null` es la
/// ausencia, y el tipo que se puede mandar es el otro.
fn argument_type(property: &Value) -> Option<&str> {
    match property.get("type") {
        Some(Value::String(name)) => Some(name.as_str()),
        Some(Value::Array(names)) => names
            .iter()
            .filter_map(Value::as_str)
            .find(|name| *name != "null"),
        _ => None,
    }
}

/// El parser de clap que corresponde al tipo del argumento.
///
/// Los enteros y los decimales tienen su propio parser con rango, y ese
/// parser se convierte al de clap; los otros tres ya son el de clap.
fn value_parser_for(property: &Value) -> clap::builder::ValueParser {
    match argument_type(property) {
        Some("boolean") => clap::value_parser!(bool),
        Some("integer") => clap::value_parser!(i64).into(),
        Some("number") => clap::value_parser!(f64).into(),
        _ => clap::value_parser!(String),
    }
}

/// El valor por defecto del argumento, como lo escribe clap.
fn default_value_for(default: &Value) -> String {
    match default {
        Value::Bool(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

/// Los nombres de los argumentos que el llamante tiene que mandar.
fn required_names(schema: &Map<String, Value>) -> Vec<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Los comandos clap de todos los goals del registro.
pub fn clap_commands() -> Vec<clap::Command> {
    registry().iter().map(clap_command).collect()
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

pub mod status;

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

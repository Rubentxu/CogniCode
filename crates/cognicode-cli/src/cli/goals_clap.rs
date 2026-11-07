//! The clap projection of the goal registry.
//!
//! The registry (`cognicode_core::interface::goals`) owns what a goal is; this
//! module only turns a declaration into a terminal command and reads the
//! matches back as the arguments the goal expects.

use clap::{Arg, ArgMatches, Command};
use cognicode_core::interface::goals::{
    GoalSpec, help_text, input_schema, registry, required_names,
};
use serde_json::Value;

/// El comando clap del goal.
///
/// El comando se construye aquí y no como una variante de un enum: un enum
/// lo escribiría una persona, y entonces el nombre del goal, su descripción
/// y sus argumentos vivirían en dos sitios. Aquí salen de la declaración.
///
/// Los tipos y los valores por defecto salen del esquema, que es el derivado
/// del struct que lee los argumentos: el flag de la terminal y el campo del
/// cable son el mismo argumento con dos nombres.
pub fn clap_command(goal: &GoalSpec) -> Command {
    let schema = input_schema(goal);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = required_names(&schema);

    let mut command = Command::new(goal.name)
        .about(goal.title)
        .long_about(help_text(goal))
        // El formato lo elige quien escribe, y lo elige en el comando: es
        // la convencion de los comandos que tienen su propia forma de
        // responder (`capabilities`, `doctor`, `find-usages`). El `--format`
        // de la raiz no los alcanza.
        .arg(
            Arg::new("format")
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
        let mut arg = Arg::new(name.clone())
            // El flag se escribe con guiones, como los demas flags del
            // producto; el nombre del argumento (el que viaja por el cable)
            // sigue siendo el del esquema.
            .long(name.replace('_', "-"))
            .value_name(name.to_uppercase())
            .help(description)
            .value_parser(value_parser_for(&property));
        // Un booleano se escribe con su valor o sin él: `--plan` y
        // `--plan true` son el mismo argumento, y el primero es el que se
        // escribe. El valor explícito sigue funcionando, así que lo que se
        // publicaba antes no se rompe.
        if argument_type(&property) == Some("boolean") {
            arg = arg.num_args(0..=1).default_missing_value("true");
        }
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
pub fn arguments_from_matches(goal: &GoalSpec, matches: &ArgMatches) -> Value {
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
            // Un objeto en la terminal llega como **texto** JSON, porque un
            // flag lleva texto. Se lee aquí para que la terminal y el cable
            // publiquen el mismo valor —un objeto— y no el mismo texto: el
            // sobre de la respuesta tiene que ser el mismo en las dos
            // superficies. Un texto que no es JSON se deja como texto, y el
            // goal que lo lee dice qué le pasa.
            Some("object") => matches
                .try_get_one::<String>(&name)
                .ok()
                .flatten()
                .map(|text| {
                    serde_json::from_str::<Value>(text)
                        .unwrap_or_else(|_| Value::String(text.clone()))
                }),
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

/// Los comandos clap de todos los goals del registro.
pub fn clap_commands() -> Vec<Command> {
    registry().iter().map(clap_command).collect()
}

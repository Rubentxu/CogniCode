//! Los enlaces del sobre, como órdenes que la terminal entiende.
//!
//! Un enlace dice «esto es lo que viene después». En el cable eso es una
//! tool y sus argumentos; en la terminal eso tiene que ser una **orden**,
//! porque una orden que la terminal no entiende no se puede copiar. Esta
//! módulo es la proyección: el enlace es el mismo, el transporte cambia.
//!
//! Los enlaces que el CLI no sabe convertir no se imprimen como orden: se
//! quedan en el JSON de la respuesta, que es donde un cliente MCP los
//! encuentra. Imprimir media orden sería peor que no imprimir ninguna.

use serde_json::{Map, Value};

use crate::interface::hateoas::{Envelope, Link};

/// Las órdenes que los enlaces del sobre piden.
///
/// Solo las que el CLI sabe convertir: una orden que la terminal no
/// entiende no es copiable.
pub fn commands(envelope: &Envelope) -> Vec<String> {
    envelope.links.iter().filter_map(command_for).collect()
}

/// Las órdenes, con el motivo al lado.
///
/// El motivo importa: una orden sin motivo es una orden que el usuario no
/// puede juzgar.
pub fn annotated_commands(envelope: &Envelope) -> Vec<(String, String)> {
    envelope
        .links
        .iter()
        .filter_map(|link| command_for(link).map(|command| (command, link.reason.clone())))
        .collect()
}

/// La orden que responde a un enlace, si el CLI la tiene.
fn command_for(link: &Link) -> Option<String> {
    let args = &link.required_args;
    match link.tool.as_str() {
        "build_graph" => Some(format!(
            "cognicode index build {}",
            word(args, "directory")?
        )),
        "get_symbol_code" => Some(format!(
            "cognicode index symbol-code {} {} {}",
            word(args, "file")?,
            number(args, "line")?,
            number(args, "col")?
        )),
        "get_call_hierarchy" => Some(format!(
            "cognicode graph hierarchy {} --direction in",
            word(args, "symbol_name")?
        )),
        "get_file_symbols" => Some(format!(
            "cognicode index outline {}",
            word(args, "file_path")?
        )),
        "analyze_impact" => Some(format!(
            "cognicode graph impact {}",
            word(args, "symbol_name")?
        )),
        _ => None,
    }
}

/// Un valor de texto del enlace.
fn word(args: &Map<String, Value>, name: &str) -> Option<String> {
    let value = args.get(name)?.as_str()?;
    Some(quote(value))
}

/// Un valor numérico del enlace.
fn number(args: &Map<String, Value>, name: &str) -> Option<String> {
    let value = args.get(name)?.as_i64()?;
    Some(value.to_string())
}

/// Un valor con espacios es un valor entre comillas.
///
/// Copiar una orden con un camino partido en dos no es copiar una orden.
fn quote(value: &str) -> String {
    if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

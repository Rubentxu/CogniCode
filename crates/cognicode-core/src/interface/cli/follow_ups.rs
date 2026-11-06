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
        // El enlace dice «reconstruye el grafo de este directorio». La
        // orden que lo hace es la que construye el grafo de llamadas —
        // `index build` construye el índice de símbolos, que es otra
        // cosa — y `--rebuild` porque sin él la terminal sirve la
        // caché y el hueco seguiría ahí.
        "build_graph" => Some(format!(
            "cognicode graph full --rebuild {}",
            word(args, "directory")?
        )),
        "get_symbol_code" => Some(format!(
            "cognicode index symbol-code {} {} {}",
            word(args, "file")?,
            number(args, "line")?,
            number(args, "col")?
        )),
        "get_call_hierarchy" => Some(format!(
            "cognicode graph hierarchy {} --direction {}",
            word(args, "symbol_name")?,
            direction(args)?
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

/// La dirección del enlace, en el vocabulario de la terminal.
///
/// El catálogo dice `incoming` y `outgoing`; la terminal dice `in` y
/// `out`. La dirección se **lee** del enlace: una orden que dice una
/// dirección que el enlace no dice es una orden que responde a otra
/// pregunta, y una dirección que la terminal no entiende no se
/// inventa — el enlace se queda en el JSON, donde un cliente lo
/// encuentra.
fn direction(args: &Map<String, Value>) -> Option<String> {
    match args.get("direction")?.as_str()? {
        "incoming" => Some("in".to_string()),
        "outgoing" => Some("out".to_string()),
        "both" => Some("both".to_string()),
        _ => None,
    }
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

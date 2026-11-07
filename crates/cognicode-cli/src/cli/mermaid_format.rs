//! The names the command line publishes for the formats of the graph.
//!
//! The graph owns the vocabulary: the domain's `MermaidFormat` names the two
//! forms the renderer produces. The command line owns the names a user
//! writes, and the table below is the whole of the translation. A name the
//! graph cannot publish has no row, and a name without a row is refused.

use cognicode_core::domain::value_objects::MermaidFormat;

/// The names the command publishes, and the format each one asks for.
///
/// The names are the ones the command has always published: `txt` and `png`
/// are how it has always been written, and a script that writes them keeps
/// working. What changed is that a name now says what the graph publishes
/// instead of travelling into the domain as a word.
pub const PUBLISHED: [(&str, MermaidFormat); 4] = [
    ("code", MermaidFormat::Code),
    ("txt", MermaidFormat::Code),
    ("svg", MermaidFormat::Svg),
    ("png", MermaidFormat::Svg),
];

/// The name the command uses when the user does not choose one.
pub const DEFAULT: &str = "txt";

/// The format the named format asks the graph for.
///
/// A name the table does not hold is a refusal, and the refusal names the
/// names the command does accept: a refusal that does not say what is
/// possible leaves the user to guess.
pub fn resolve(name: &str) -> Result<MermaidFormat, String> {
    PUBLISHED
        .iter()
        .find(|(published, _)| *published == name)
        .map(|(_, format)| *format)
        .ok_or_else(|| {
            format!(
                "unknown mermaid format '{name}': the accepted formats are {}",
                names().join(", ")
            )
        })
}

/// The names the command accepts.
pub fn names() -> Vec<&'static str> {
    PUBLISHED.iter().map(|(name, _)| *name).collect()
}

/// The help that names the formats the command accepts.
///
/// The help is what a user reads before writing the command, so it is built
/// from the same table the command resolves with: the names it names are
/// the names the command takes.
pub fn help() -> String {
    let sources: Vec<&str> = PUBLISHED
        .iter()
        .filter(|(_, format)| format.is_source())
        .map(|(name, _)| *name)
        .collect();
    let drawings: Vec<&str> = PUBLISHED
        .iter()
        .filter(|(_, format)| !format.is_source())
        .map(|(name, _)| *name)
        .collect();

    format!(
        "Output format: {} (the Mermaid source) or {} (the rendered diagram)",
        sources.join(", "),
        drawings.join(", ")
    )
}

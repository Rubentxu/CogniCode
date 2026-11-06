//! The formats a Mermaid diagram is published in.
//!
//! The vocabulary of the diagram belongs to the graph: this is the type that
//! names the two forms the renderer produces, and every surface reads it —
//! the shape the server publishes, the options the renderer takes, and the
//! command line that asks for one of them.
//!
//! A format the graph cannot publish is a refusal where the arguments
//! arrive, not a word compared in silence.

use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

/// The format a Mermaid diagram is published in.
///
/// The type is what the server reads, so the published shape names the
/// values: the shape is derived from this type, and the values a client can
/// read are the values the server accepts.
///
/// The two variants are the two forms the renderer produces — the source,
/// which is the Mermaid text, and the diagram, which is the drawing. They
/// carry no documentation of their own on purpose: a schema generator turns
/// a documented variant into a list of constants, and a published shape that
/// a machine has to read is better as a plain list of the names it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MermaidFormat {
    Code,
    Svg,
}

impl MermaidFormat {
    /// Every format the graph can publish.
    pub const ALL: [MermaidFormat; 2] = [MermaidFormat::Code, MermaidFormat::Svg];

    /// The name the wire carries.
    pub fn as_str(&self) -> &'static str {
        match self {
            MermaidFormat::Code => "code",
            MermaidFormat::Svg => "svg",
        }
    }

    /// The format the diagram is published in.
    ///
    /// A theme asks for the rendered diagram: the caller that names a theme
    /// wants to see the drawing, not the source. The rule lives with the
    /// vocabulary, so no surface has to know it on its own.
    pub fn resolve(requested: Option<Self>, themed: bool) -> Self {
        match requested {
            Some(format) => format,
            None if themed => Self::Svg,
            None => Self::Code,
        }
    }

    /// Whether the diagram is published as the source.
    ///
    /// The source is the form a reader can take in: the rendered diagram is
    /// a drawing, and a drawing is not read character by character.
    pub fn is_source(&self) -> bool {
        matches!(self, Self::Code)
    }
}

/// A name the graph does not publish is refused, and the refusal says so.
///
/// The parsing lives here, so the wire and the command line agree on what a
/// format is by construction rather than by coincidence.
impl FromStr for MermaidFormat {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "code" => Ok(Self::Code),
            "svg" => Ok(Self::Svg),
            other => Err(format!(
                "unknown mermaid format '{other}': the graph publishes code or svg"
            )),
        }
    }
}

/// The wire carries the name, and the name is read here.
///
/// The published shape is derived from the type, so the values a client can
/// read are the values the server accepts.
impl<'de> Deserialize<'de> for MermaidFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_the_name_the_wire_carries() {
        assert_eq!(MermaidFormat::Code.as_str(), "code");
        assert_eq!(MermaidFormat::Svg.as_str(), "svg");
    }

    #[test]
    fn the_vocabulary_is_the_two_forms_the_renderer_produces() {
        assert_eq!(MermaidFormat::ALL.len(), 2);
        assert_eq!(
            MermaidFormat::ALL
                .iter()
                .map(|format| format.as_str())
                .collect::<Vec<_>>(),
            vec!["code", "svg"]
        );
    }

    #[test]
    fn a_name_the_graph_does_not_publish_is_refused() {
        let refusal = "png"
            .parse::<MermaidFormat>()
            .expect_err("png is not a format");

        assert!(
            refusal.contains("code") && refusal.contains("svg"),
            "the refusal does not name the formats the graph publishes: {refusal}"
        );
    }

    /// A theme asks for the rendered diagram; nothing else does.
    #[test]
    fn a_theme_asks_for_the_rendered_diagram() {
        assert_eq!(
            MermaidFormat::resolve(None, true),
            MermaidFormat::Svg,
            "a theme asks for the rendered diagram"
        );
        assert_eq!(
            MermaidFormat::resolve(None, false),
            MermaidFormat::Code,
            "without a theme the source is published"
        );
    }

    /// What the caller asks for is what is published.
    #[test]
    fn what_the_caller_asks_for_is_what_is_published() {
        assert_eq!(
            MermaidFormat::resolve(Some(MermaidFormat::Code), true),
            MermaidFormat::Code,
            "the source was asked for, and the source is published"
        );
        assert_eq!(
            MermaidFormat::resolve(Some(MermaidFormat::Svg), false),
            MermaidFormat::Svg,
            "the rendered diagram was asked for, and it is published"
        );
    }
}

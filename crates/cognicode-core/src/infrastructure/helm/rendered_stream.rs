//! Parse the multi-document YAML stream that `helm template` prints.

use serde_json::Value;

use crate::domain::iac::helm::RenderedDocument;

const SOURCE_PREFIX: &str = "# Source: ";

/// Split a rendered stream into documents, keeping the `# Source:` comment the
/// engine writes before each one. Document indices count empty documents too,
/// so an index names a position in the stream.
///
/// # Errors
/// The message names the document that is not valid YAML.
pub fn parse_rendered_stream(text: &str) -> Result<Vec<RenderedDocument>, String> {
    let mut documents = Vec::new();
    for (index, chunk) in split_documents(text).into_iter().enumerate() {
        let template = chunk
            .lines()
            .find_map(|line| line.strip_prefix(SOURCE_PREFIX))
            .map(|source| source.trim().to_string());
        if chunk
            .lines()
            .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
        {
            continue;
        }
        let value: Value = serde_norway::from_str(&chunk)
            .map_err(|error| format!("document {index} is not valid YAML: {error}"))?;
        documents.push(RenderedDocument {
            template,
            index,
            value,
        });
    }
    Ok(documents)
}

fn split_documents(text: &str) -> Vec<String> {
    let mut chunks = vec![String::new()];
    for line in text.lines() {
        if line.trim_end() == "---" {
            chunks.push(String::new());
        } else if let Some(current) = chunks.last_mut() {
            current.push_str(line);
            current.push('\n');
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_keep_their_source_and_stream_position() {
        let text = "---\n# Source: demo/templates/sa.yaml\napiVersion: v1\nkind: ServiceAccount\nmetadata:\n  name: a\n---\n# Source: demo/templates/svc.yaml\nkind: Service\nmetadata:\n  name: b\n";
        let docs = parse_rendered_stream(text).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].template.as_deref(), Some("demo/templates/sa.yaml"));
        assert_eq!(docs[1].index, 2);
        assert_eq!(docs[1].value["metadata"]["name"], "b");
    }

    #[test]
    fn comment_only_documents_are_not_objects_but_still_count_as_positions() {
        let text = "---\n# Source: demo/templates/empty.yaml\n---\n# Source: demo/templates/a.yaml\nkind: Pod\nmetadata:\n  name: p\n";
        let docs = parse_rendered_stream(text).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].index, 2);
    }

    #[test]
    fn invalid_yaml_is_an_error_naming_the_document() {
        let error = parse_rendered_stream("---\nkind: [unclosed\n").unwrap_err();
        assert!(error.contains("document 1"), "{error}");
    }
}

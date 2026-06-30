//! Mermaid diagram utilities.
//!
//! Shared helpers for Mermaid diagram generation — ID sanitisation and
//! deduplication.

/// Sanitise a string into a valid Mermaid identifier.
///
/// Replaces special characters (`:`, `/`, `(`, `)`, `.`, `-`, ` `) with underscores,
/// then collapses consecutive underscores.
pub fn sanitize_id(id: &str) -> String {
    let mut result = String::with_capacity(id.len());
    let mut last_was_underscore = false;
    for ch in id.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            result.push(ch);
            last_was_underscore = ch == '_';
        } else {
            if !last_was_underscore {
                result.push('_');
            }
            last_was_underscore = true;
        }
    }
    // Trim leading/trailing underscores
    result.trim_matches('_').to_string()
}

/// Deduplicate a list of IDs by appending `_2`, `_3`, … suffixes as needed.
///
/// Preserves insertion order. The first occurrence of a base ID is unchanged;
/// subsequent duplicates get numeric suffixes. IDs are sanitised before deduplication.
pub fn deduplicate_ids<T: AsRef<str>>(ids: &[T]) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    ids.iter()
        .map(|id| {
            let raw = id.as_ref();
            let base = sanitize_id(raw);
            let counter = seen.entry(base.clone()).or_insert(0);
            *counter += 1;
            if *counter == 1 {
                base
            } else {
                format!("{}_{}", base, counter)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // sanitize_id
    // ------------------------------------------------------------------------

    #[test]
    fn sanitize_id_colons() {
        assert_eq!(sanitize_id("symbol:foo:bar"), "symbol_foo_bar");
    }

    #[test]
    fn sanitize_id_slashes() {
        assert_eq!(sanitize_id("path/to/file"), "path_to_file");
    }

    #[test]
    fn sanitize_id_parens() {
        assert_eq!(sanitize_id("fn (arg)"), "fn_arg");
    }

    #[test]
    fn sanitize_id_dots() {
        assert_eq!(sanitize_id("crate.module"), "crate_module");
    }

    #[test]
    fn sanitize_id_alphanumeric_unchanged() {
        assert_eq!(sanitize_id("valid_id_123"), "valid_id_123");
    }

    #[test]
    fn sanitize_id_leading_trailing_underscores_trimmed() {
        assert_eq!(sanitize_id("_foo_"), "foo");
    }

    // ------------------------------------------------------------------------
    // deduplicate_ids
    // ------------------------------------------------------------------------

    #[test]
    fn deduplicate_ids_no_dupes() {
        let ids = ["a", "b", "c"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn deduplicate_ids_one_dup() {
        let ids = ["a", "b", "a"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "a_2"]);
    }

    #[test]
    fn deduplicate_ids_multiple_dups() {
        let ids = ["a", "b", "a", "a", "c", "b"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "a_2", "a_3", "c", "b_2"]);
    }
}

//! A reader of a language that falls over leaves the file unread.
//!
//! The consumer does not fall back to the generic reader: a file whose
//! reader fell over was not read, and saying so is what keeps the hole
//! visible in the coverage instead of publishing half a file.

#[cfg(test)]
mod body {
    use crate::application::ingest::types::ExtractionResult;
    use crate::application::services::analysis_service::construction::read_file_facts_with;
    use crate::domain::traits::ParseError;
    use crate::infrastructure::parser::Language;
    use crate::infrastructure::parser::language_config::{HCL_CONFIG, LanguageConfig};

    /// A reader that cannot read.
    fn a_reader_that_falls_over(
        _source_path: &str,
        _source_hash: &str,
        _source: &str,
        _tree: &tree_sitter::Tree,
        _result: &ExtractionResult,
    ) -> ExtractionResult {
        panic!("this reader cannot read what it was handed");
    }

    /// A reader that falls over is an error with its cause, not a file with
    /// nothing in it.
    #[test]
    fn a_reader_that_falls_over_is_an_error_with_its_cause() {
        let config = LanguageConfig {
            language: Language::Hcl,
            ts_language: HCL_CONFIG.ts_language,
            type_ref_walker: HCL_CONFIG.type_ref_walker,
            semantic_handler: Some(a_reader_that_falls_over),
        };

        match read_file_facts_with(&config, "resource \"a\" \"b\" {}\n", "main.tf") {
            Err(ParseError::InvalidSource(cause)) => assert!(
                cause.contains("this reader cannot read what it was handed"),
                "the error carries the reason the reader gave: {cause}"
            ),
            Err(other) => panic!("the failure is reported as a cause: {other:?}"),
            Ok(facts) => panic!(
                "a file whose reader fell over was read as {} symbols",
                facts.symbols.len()
            ),
        }
    }
}

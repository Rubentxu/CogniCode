//! LSP protocol implementation helpers

use crate::domain::value_objects::{Location as DomainLocation, SourceRange};
use lsp_types::{Position, Range, TextDocumentIdentifier, TextDocumentPositionParams, Url};

/// Converts a domain Location to an LSP Position
pub fn domain_location_to_lsp(location: &DomainLocation) -> Position {
    Position::new(
        location.line().saturating_sub(1),
        location.column().saturating_sub(1),
    )
}

/// Converts an LSP Position to a domain Location
pub fn lsp_position_to_domain(position: Position, uri: String) -> DomainLocation {
    DomainLocation::new(uri, position.line + 1, position.character + 1)
}

/// Converts a domain SourceRange to an LSP Range
pub fn domain_range_to_lsp(range: &SourceRange) -> Range {
    Range::new(
        Position::new(
            range.start().line().saturating_sub(1),
            range.start().column().saturating_sub(1),
        ),
        Position::new(
            range.end().line().saturating_sub(1),
            range.end().column().saturating_sub(1),
        ),
    )
}

/// Creates a TextDocumentPositionParams from a location
pub fn location_to_text_document_position(location: &DomainLocation) -> TextDocumentPositionParams {
    let url = Url::parse(location.file()).unwrap_or_else(|_| Url::parse("file://").unwrap());
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier::new(url),
        position: domain_location_to_lsp(location),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_location_to_lsp_converts_correctly() {
        let location = DomainLocation::new("/path/to/file.rs", 10, 5);
        let position = domain_location_to_lsp(&location);
        assert_eq!(position.line, 9);
        assert_eq!(position.character, 4);
    }

    #[test]
    fn test_domain_location_to_lsp_handles_zero_values() {
        let location = DomainLocation::new("/path/to/file.rs", 0, 0);
        let position = domain_location_to_lsp(&location);
        assert_eq!(position.line, 0);
        assert_eq!(position.character, 0);
    }

    #[test]
    fn test_lsp_position_to_domain_converts_correctly() {
        let position = Position::new(9, 4);
        let location = lsp_position_to_domain(position, "/path/to/file.rs".to_string());
        assert_eq!(location.line(), 10);
        assert_eq!(location.column(), 5);
        assert_eq!(location.file(), "/path/to/file.rs");
    }

    #[test]
    fn test_lsp_position_to_domain_handles_zero() {
        let position = Position::new(0, 0);
        let location = lsp_position_to_domain(position, "/path/to/file.rs".to_string());
        assert_eq!(location.line(), 1);
        assert_eq!(location.column(), 1);
    }

    #[test]
    fn test_domain_range_to_lsp_converts_correctly() {
        let start = DomainLocation::new("/path/to/file.rs", 5, 3);
        let end = DomainLocation::new("/path/to/file.rs", 10, 8);
        let range = SourceRange::new(start, end);
        let lsp_range = domain_range_to_lsp(&range);
        assert_eq!(lsp_range.start.line, 4);
        assert_eq!(lsp_range.start.character, 2);
        assert_eq!(lsp_range.end.line, 9);
        assert_eq!(lsp_range.end.character, 7);
    }

    #[test]
    fn test_domain_range_to_lsp_single_line() {
        let start = DomainLocation::new("/path/to/file.rs", 1, 0);
        let end = DomainLocation::new("/path/to/file.rs", 1, 5);
        let range = SourceRange::new(start, end);
        let lsp_range = domain_range_to_lsp(&range);
        assert_eq!(lsp_range.start.line, 0);
        assert_eq!(lsp_range.start.character, 0);
        assert_eq!(lsp_range.end.line, 0);
        assert_eq!(lsp_range.end.character, 4);
    }

    #[test]
    fn test_location_to_text_document_position_with_valid_uri() {
        let location = DomainLocation::new("file:///path/to/file.rs", 5, 10);
        let params = location_to_text_document_position(&location);
        assert_eq!(params.text_document.uri.as_str(), "file:///path/to/file.rs");
        assert_eq!(params.position.line, 4);
        assert_eq!(params.position.character, 9);
    }

    #[test]
    fn test_location_to_text_document_position_falls_back_on_invalid_path() {
        let location = DomainLocation::new("/path/to/file.rs", 5, 10);
        let params = location_to_text_document_position(&location);
        assert_eq!(params.text_document.uri.as_str(), "file:///");
        assert_eq!(params.position.line, 4);
        assert_eq!(params.position.character, 9);
    }
}

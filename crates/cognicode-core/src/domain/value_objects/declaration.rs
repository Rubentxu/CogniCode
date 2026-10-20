//! What a declaration is, in the vocabulary of the language that declared it.
//!
//! A graph models symbols, and a symbol alone says little about what it
//! is: a Terraform block and a Rust function are both symbols. The reader
//! that understands the language is the one that knows the difference — it
//! is the one that decided the block was a `resource` of type
//! `aws_s3_bucket` — and this value object is where that decision is
//! kept, so the graph can answer with it instead of with the name of the
//! type that happened to hold it.
//!
//! The domain does not enumerate these names: a reader names its own
//! declarations, and the graph carries that name.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What a symbol declares.
///
/// The kind is the name the reader gave the declaration: `resource`, `data`,
/// `variable`, `module`, `output`, `play`, `task`, `handler`. The type is
/// the name the declaration itself carries — `aws_s3_bucket` — when it
/// carries one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Declaration {
    kind: String,
    /// Absent when the declaration names no type of its own.
    #[serde(default)]
    resource_type: Option<String>,
}

impl Declaration {
    /// The node property a reader fills to name the kind of its declaration.
    pub const KIND_PROPERTY: &'static str = "iac_kind";

    /// The node property a reader fills to name the type of its declaration.
    pub const TYPE_PROPERTY: &'static str = "iac_type";

    /// A declaration of the given kind.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            resource_type: None,
        }
    }

    /// A declaration of the given kind that carries a type.
    pub fn with_type(kind: impl Into<String>, resource_type: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            resource_type: Some(resource_type.into()),
        }
    }

    /// The kind of declaration, in the vocabulary of the language that
    /// declared it.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The type the declaration carries, when it carries one.
    pub fn resource_type(&self) -> Option<&str> {
        self.resource_type.as_deref()
    }

    /// The type of the declaration; its kind when it carries no type.
    ///
    /// A bucket declares `aws_s3_bucket`; a variable declares no type and
    /// is a `variable`. One rule, one answer.
    pub fn type_or_kind(&self) -> &str {
        self.resource_type.as_deref().unwrap_or(&self.kind)
    }
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_or_kind())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declaration_that_names_a_type_answers_with_the_type() {
        let declaration = Declaration::with_type("resource", "aws_s3_bucket");
        assert_eq!(declaration.kind(), "resource");
        assert_eq!(declaration.resource_type(), Some("aws_s3_bucket"));
        assert_eq!(declaration.type_or_kind(), "aws_s3_bucket");
    }

    #[test]
    fn a_declaration_that_names_no_type_answers_with_its_kind() {
        let declaration = Declaration::new("variable");
        assert_eq!(declaration.resource_type(), None);
        assert_eq!(declaration.type_or_kind(), "variable");
    }

    #[test]
    fn the_answer_is_the_same_one_the_display_shows() {
        let declaration = Declaration::with_type("resource", "aws_s3_bucket");
        assert_eq!(declaration.to_string(), declaration.type_or_kind());
    }
}

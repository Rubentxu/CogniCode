//! Aggregate Root for code symbols
//!
//! Represents a symbol (function, class, variable, etc.) in the codebase.

use std::fmt;

use super::super::value_objects::{Location, SymbolKind};

/// Aggregate Root representing a code symbol
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    name: String,
    kind: SymbolKind,
    location: Location,
    signature: Option<FunctionSignature>,
    fqn: String,
}

impl Symbol {
    /// Creates a new Symbol with the given name, kind, and location
    pub fn new(name: impl Into<String>, kind: SymbolKind, location: Location) -> Self {
        let name = name.into();
        let fqn = format!("{}:{}:{}", location.file(), name, location.line());
        Self {
            name,
            kind,
            location,
            signature: None,
            fqn,
        }
    }

    /// Creates a new Symbol with a signature (for callable symbols)
    pub fn with_signature(mut self, signature: FunctionSignature) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Returns the name of this symbol
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the kind of this symbol
    pub fn kind(&self) -> &SymbolKind {
        &self.kind
    }

    /// Returns the location of this symbol
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// Returns the signature if this is a callable symbol
    pub fn signature(&self) -> Option<&FunctionSignature> {
        self.signature.as_ref()
    }

    /// Returns the fully qualified name of this symbol
    pub fn fully_qualified_name(&self) -> &str {
        &self.fqn
    }

    #[doc(hidden)]
    pub fn set_fqn_override(&mut self, fqn: &str) {
        self.fqn = fqn.to_string();
    }

    /// Returns true if this symbol is callable (function, method, constructor)
    pub fn is_callable(&self) -> bool {
        self.kind.is_callable()
    }

    /// Returns true if this symbol is a type definition (class, struct, enum, trait, etc.)
    pub fn is_type_definition(&self) -> bool {
        self.kind.is_type_definition()
    }

    /// Returns true if this symbol has a signature
    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) at {}", self.name, self.kind, self.location)?;
        if let Some(sig) = &self.signature {
            write!(f, " {}", sig)?;
        }
        Ok(())
    }
}

/// Represents the signature of a callable symbol (function, method, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: Option<String>,
    is_async: bool,
}

impl FunctionSignature {
    /// Creates a new FunctionSignature
    pub fn new(parameters: Vec<Parameter>, return_type: Option<String>, is_async: bool) -> Self {
        Self {
            parameters,
            return_type,
            is_async,
        }
    }

    /// Returns the list of parameters
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    /// Returns the return type if specified
    pub fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }

    /// Returns true if this is an async function
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Returns the number of parameters
    pub fn arity(&self) -> usize {
        self.parameters.len()
    }

    /// Returns true if this function has variadic parameters
    pub fn is_variadic(&self) -> bool {
        self.parameters.iter().any(|p| p.is_variadic)
    }
}

impl fmt::Display for FunctionSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_async {
            write!(f, "async ")?;
        }
        write!(f, "(")?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", param)?;
        }
        write!(f, ")")?;
        if let Some(return_type) = &self.return_type {
            write!(f, " -> {}", return_type)?;
        }
        Ok(())
    }
}

/// Represents a parameter in a function signature
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parameter {
    name: String,
    type_annotation: Option<String>,
    is_variadic: bool,
}

impl Parameter {
    /// Creates a new Parameter
    pub fn new(name: impl Into<String>, type_annotation: Option<String>) -> Self {
        Self {
            name: name.into(),
            type_annotation,
            is_variadic: false,
        }
    }

    /// Creates a new variadic Parameter
    pub fn variadic(name: impl Into<String>, type_annotation: Option<String>) -> Self {
        Self {
            name: name.into(),
            type_annotation,
            is_variadic: true,
        }
    }

    /// Returns the parameter name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type annotation if specified
    pub fn type_annotation(&self) -> Option<&str> {
        self.type_annotation.as_deref()
    }

    /// Returns true if this is a variadic parameter
    pub fn is_variadic(&self) -> bool {
        self.is_variadic
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(type_annotation) = &self.type_annotation {
            write!(f, ": {}", type_annotation)?;
        }
        if self.is_variadic {
            write!(f, "...")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_new() {
        let location = Location::new("test.rs", 10, 5);
        let symbol = Symbol::new("MyClass", SymbolKind::Class, location);
        assert_eq!(symbol.name(), "MyClass");
        assert_eq!(symbol.kind(), &SymbolKind::Class);
    }

    #[test]
    fn test_symbol_with_signature() {
        let location = Location::new("test.rs", 10, 5);
        let signature = FunctionSignature::new(
            vec![Parameter::new("x", Some("i32".to_string()))],
            Some("i32".to_string()),
            false,
        );
        let symbol = Symbol::new("add", SymbolKind::Function, location).with_signature(signature);
        assert!(symbol.has_signature());
        assert!(symbol.is_callable());
    }

    #[test]
    fn test_symbol_not_callable() {
        let location = Location::new("test.rs", 10, 5);
        let symbol = Symbol::new("MyClass", SymbolKind::Class, location);
        assert!(!symbol.is_callable());
        assert!(symbol.is_type_definition());
    }

    #[test]
    fn test_function_signature_display() {
        let signature = FunctionSignature::new(
            vec![
                Parameter::new("x", Some("i32".to_string())),
                Parameter::variadic("args", Some("String".to_string())),
            ],
            Some("Result".to_string()),
            true,
        );
        let display = format!("{}", signature);
        assert!(display.contains("async"));
        assert!(display.contains("x: i32"));
        assert!(display.contains("args: String..."));
        assert!(display.contains("-> Result"));
    }

    #[test]
    fn test_parameter_display() {
        let param = Parameter::new("x", Some("i32".to_string()));
        assert_eq!(format!("{}", param), "x: i32");

        let variadic_param = Parameter::variadic("args", None);
        assert_eq!(format!("{}", variadic_param), "args...");
    }

    #[test]
    fn test_symbol_display() {
        let location = Location::new("test.rs", 10, 5);
        let signature = FunctionSignature::new(vec![], Some("void".to_string()), false);
        let symbol = Symbol::new("main", SymbolKind::Function, location).with_signature(signature);
        let display = format!("{}", symbol);
        assert!(display.contains("main"));
        assert!(display.contains("function"));
        assert!(display.contains("test.rs:10:5"));
    }
}

//! Implementation of the `#[aix_tool]` attribute macro for MCP tool registration.
//!
//! This macro generates tool definitions for the MCP registry, reducing boilerplate
//! when defining MCP tools.
//!
//! # Example
//!
//! ```ignore
//! #[aix_tool(
//!     name = "build_graph",
//!     description = "Build the call graph for a project directory",
//!     input_schema = BuildGraphInput
//! )]
//! pub async fn handle_build_graph(
//!     ctx: &HandlerContext,
//!     input: BuildGraphInput
//! ) -> HandlerResult<BuildGraphOutput> {
//!     // ... implementation
//! }
//! ```
//!
//! The macro generates:
//! - A `ToolDef<name>` struct containing the tool metadata
//! - A `TOOL_DEF_{name}` constant for registration
//!
//! The generated ToolDef provides:
//! - `name`: The tool's identifier
//! - `description`: Human-readable description
//! - `input_type_name`: The type name of the input schema (as a string)
//!
//! To use this macro:
//! 1. Apply `#[derive(serde::Deserialize)]` to your input type (for JSON parsing)
//! 2. Apply the `#[aix_tool]` attribute to your handler function
//! 3. Use the generated `TOOL_DEF_<NAME>` constant for registration

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemFn, LitStr, Token, Type};

/// Parsed attributes for `#[aix_tool]`
struct AixToolAttrs {
    name: String,
    description: String,
    input_schema: Type,
}

impl syn::parse::Parse for AixToolAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut input_schema = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _separator = if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                "="
            } else if input.peek(Token![:]) {
                input.parse::<Token![:]>()?;
                ":"
            } else {
                return Err(syn::Error::new(key.span(), "expected `=` or `:` after key"));
            };
            let key_str = key.to_string();

            match key_str.as_str() {
                "name" => {
                    let value: LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "description" => {
                    let value: LitStr = input.parse()?;
                    description = Some(value.value());
                }
                "input_schema" => {
                    let value: Type = input.parse()?;
                    input_schema = Some(value);
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("Unknown key: {}", key_str),
                    ))
                }
            }

            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
        }

        Ok(Self {
            name: name.ok_or_else(|| syn::Error::new(input.span(), "Missing `name` field"))?,
            description: description
                .ok_or_else(|| syn::Error::new(input.span(), "Missing `description` field"))?,
            input_schema: input_schema
                .ok_or_else(|| syn::Error::new(input.span(), "Missing `input_schema` field"))?,
        })
    }
}

/// Generate the ToolDef struct and registration constant
fn generate_tool_def(attrs: &AixToolAttrs, fn_name: &Ident) -> TokenStream {
    let name_literal = LitStr::new(&attrs.name, fn_name.span());
    let desc_literal = LitStr::new(&attrs.description, fn_name.span());
    let const_name = Ident::new(
        &format!("TOOL_DEF_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );
    let input_type = &attrs.input_schema;

    // Create a valid CamelCase Rust identifier from the tool name
    let snake_name = fn_name.to_string();
    let camel_name = to_camel_case(&snake_name);
    let struct_name = Ident::new(&camel_name, fn_name.span());

    quote! {
        /// Tool definition for #name_literal
        #[derive(Debug, Clone)]
        #[allow(non_camel_case_types)]
        pub struct #struct_name {
            /// The tool's unique identifier
            pub name: &'static str,
            /// Human-readable description of what the tool does
            pub description: &'static str,
            /// The type name of the input schema
            pub input_type_name: &'static str,
        }

        /// Registration constant for tool #name_literal
        pub const #const_name: #struct_name = #struct_name {
            name: #name_literal,
            description: #desc_literal,
            input_type_name: stringify!(#input_type),
        };
    }
}

/// Convert snake_case to CamelCase
fn to_camel_case(snake: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in snake.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    // Capitalize first character
    if let Some(first) = result.chars().next() {
        if first.is_ascii_lowercase() {
            let mut s = result.clone();
            s.remove(0);
            s.insert(0, first.to_ascii_uppercase());
            result = s;
        }
    }
    result
}

/// Implementation of the `#[aix_tool]` attribute macro
pub fn derive_aix_tool(
    attr: proc_macro2::TokenStream,
    input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    // Parse attribute tokens
    let attrs = syn::parse2::<AixToolAttrs>(attr).expect("Failed to parse #[aix_tool] attributes");

    // Parse the function
    let input_fn = syn::parse2::<ItemFn>(input).expect("Failed to parse function");
    let fn_name = &input_fn.sig.ident;

    // Generate tool definition
    let tool_def = generate_tool_def(&attrs, fn_name);

    // The expanded code
    quote! {
        #input_fn

        #tool_def
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_parse_attrs() {
        let input = quote! {
            name = "test_tool",
            description = "A test tool",
            input_schema = TestInput
        };
        let attrs = syn::parse2::<AixToolAttrs>(input).unwrap();
        assert_eq!(attrs.name, "test_tool");
        assert_eq!(attrs.description, "A test tool");
    }
}

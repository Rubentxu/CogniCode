//! Newtype macro for creating wrapper types with consistent derives.
//!
//! # Syntax
//!
//! ```ignore
//! #[newtype]
//! #[newtype(derive(Clone, Eq, PartialEq, Hash))]
//! pub struct UserId(i64);
//! ```
//!
//! The macro auto-derives: `Debug`, `Display`, `From`, `Serialize`, `Deserialize`
//! Opt-in derives via `#[newtype(derive(...))]`: `Clone`, `Copy`, `Eq`, `Ord`, `Hash`, `Default`

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Fields, Type};

/// Parse extra derives from `#[newtype(derive(Clone, Eq))]`
fn parse_extra_derives(attrs: &[syn::Attribute]) -> Vec<Ident> {
    for attr in attrs {
        // Check if this is a #[newtype(...)] attribute
        let path = attr.path();
        if path.is_ident("newtype") {
            // attr.meta is already a Meta value
            if let syn::Meta::List(meta_list) = &attr.meta {
                // meta_list.tokens contains the content inside #[newtype(...)]
                let tokens = meta_list.tokens.clone();

                // Parse the tokens as a bracketed group
                let mut iter = tokens.into_iter().peekable();
                while let Some(token) = iter.next() {
                    let ident = match token {
                        proc_macro2::TokenTree::Ident(i) => i,
                        _ => continue,
                    };

                    if ident == "derive" {
                        // Look for the parentheses group
                        if let Some(proc_macro2::TokenTree::Group(g)) = iter.next() {
                            let content = g.stream().to_string();
                            return content
                                .split(',')
                                .filter(|s| !s.trim().is_empty())
                                .map(|s| Ident::new(s.trim(), proc_macro2::Span::call_site()))
                                .collect();
                        }
                    }
                }
            }
        }
    }
    Vec::new()
}

/// Extract the inner type from a newtype struct (the wrapped type).
fn extract_inner_type(data: &Data) -> Type {
    match data {
        Data::Struct(ref struct_data) => {
            if let Fields::Unnamed(ref fields) = struct_data.fields {
                if let Some(field) = fields.unnamed.first() {
                    return field.ty.clone();
                }
            }
            parse_quote! { () }
        }
        _ => parse_quote! { () },
    }
}

/// Entry point for the `#[newtype]` derive macro.
pub fn derive_newtype(input: TokenStream2) -> TokenStream2 {
    // Parse as DeriveInput directly - syn handles attribute parsing
    let input: DeriveInput = match syn::parse2(input.clone()) {
        Ok(input) => input,
        Err(e) => {
            // Convert syn::Error to token stream
            let compile_error = syn::Error::to_compile_error(&e);
            return TokenStream2::from(quote! { #compile_error });
        }
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let inner_type = extract_inner_type(&input.data);
    let extra_derives = parse_extra_derives(&input.attrs);

    // Auto-derives: Debug, Display, From, Serialize, Deserialize
    let auto_derives: Vec<Ident> = vec![
        quote::format_ident!("Debug"),
        quote::format_ident!("Display"),
        quote::format_ident!("Serialize"),
        quote::format_ident!("Deserialize"),
    ];

    let expanded = if extra_derives.is_empty() {
        quote! {
            #[derive(#(#auto_derives),*)]
            #input

            impl #impl_generics From<#inner_type> for #name #ty_generics #where_clause {
                fn from(inner: #inner_type) -> Self {
                    Self(inner)
                }
            }
        }
    } else {
        quote! {
            #[derive(#(#auto_derives),*, #(#extra_derives),*)]
            #input

            impl #impl_generics From<#inner_type> for #name #ty_generics #where_clause {
                fn from(inner: #inner_type) -> Self {
                    Self(inner)
                }
            }
        }
    };

    TokenStream2::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newtype_parsing_empty_derive() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct UserId(i64);
        })
        .unwrap();

        let extra = parse_extra_derives(&input.attrs);
        assert!(extra.is_empty());
    }

    #[test]
    fn test_newtype_parsing_with_derive() {
        let input: DeriveInput = syn::parse2(quote! {
            #[newtype(derive(Clone, Eq))]
            pub struct UserId(i64);
        })
        .unwrap();

        let extra = parse_extra_derives(&input.attrs);
        assert_eq!(extra.len(), 2);
        assert!(extra.iter().any(|i| i == "Clone"));
        assert!(extra.iter().any(|i| i == "Eq"));
    }

    #[test]
    fn test_newtype_parsing_with_single_derive() {
        let input: DeriveInput = syn::parse2(quote! {
            #[newtype(derive(Clone))]
            pub struct ResourceHandle(String);
        })
        .unwrap();

        let extra = parse_extra_derives(&input.attrs);
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0], "Clone");
    }

    #[test]
    fn test_extract_inner_type_simple() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct UserId(i64);
        })
        .unwrap();

        let inner = extract_inner_type(&input.data);
        let inner_str = quote! { #inner }.to_string();
        assert!(inner_str.contains("i64"));
    }

    #[test]
    fn test_extract_inner_type_string() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct ResourceHandle(String);
        })
        .unwrap();

        let inner = extract_inner_type(&input.data);
        let inner_str = quote! { #inner }.to_string();
        assert!(inner_str.contains("String"));
    }

    #[test]
    fn test_extract_inner_type_option() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct OptionalId(Option<i64>);
        })
        .unwrap();

        let inner = extract_inner_type(&input.data);
        let inner_str = quote! { #inner }.to_string();
        assert!(inner_str.contains("Option"));
        assert!(inner_str.contains("i64"));
    }

    #[test]
    fn test_derive_newtype_output_has_debug() {
        // Test that the macro generates derive for Debug
        let tokens = quote! { pub struct UserId(i64); };
        let output = derive_newtype(tokens);
        let output_str = output.to_string();
        // The output should contain Debug derive
        assert!(output_str.contains("Debug"));
    }

    #[test]
    fn test_derive_newtype_output_has_display() {
        let output = derive_newtype(quote! { pub struct UserId(i64); });
        let output_str = output.to_string();
        // The output should contain Display derive
        assert!(output_str.contains("Display"));
    }

    #[test]
    fn test_derive_newtype_output_has_serialize() {
        let output = derive_newtype(quote! { pub struct UserId(i64); });
        let output_str = output.to_string();
        // The output should contain Serialize derive
        assert!(output_str.contains("Serialize"));
    }

    #[test]
    fn test_derive_newtype_output_has_deserialize() {
        let output = derive_newtype(quote! { pub struct UserId(i64); });
        let output_str = output.to_string();
        // The output should contain Deserialize derive
        assert!(output_str.contains("Deserialize"));
    }

    #[test]
    fn test_derive_newtype_output_has_from_impl() {
        let output = derive_newtype(quote! { pub struct UserId(i64); });
        let output_str = output.to_string();
        // The output should contain From<i64> for UserId implementation
        assert!(output_str.contains("From"));
        assert!(output_str.contains("i64"));
        assert!(output_str.contains("UserId"));
    }

    #[test]
    fn test_derive_newtype_with_extra_derives() {
        let output = derive_newtype(quote! {
            #[newtype(derive(Clone, Eq, Hash))]
            pub struct UserId(i64);
        });
        let output_str = output.to_string();
        // Should contain both auto-derives and extra derives
        assert!(output_str.contains("Debug"));
        assert!(output_str.contains("Clone"));
        assert!(output_str.contains("Eq"));
        assert!(output_str.contains("Hash"));
    }

    #[test]
    fn test_derive_newtype_compiles_without_errors() {
        // This test just verifies the derive doesn't panic
        let result =
            std::panic::catch_unwind(|| derive_newtype(quote! { pub struct TestType(String); }));
        assert!(result.is_ok());
    }

    #[test]
    fn test_derive_newtype_with_unit_type() {
        // Test edge case with unit type
        let result = std::panic::catch_unwind(|| derive_newtype(quote! { pub struct Empty(()); }));
        assert!(result.is_ok());
    }
}

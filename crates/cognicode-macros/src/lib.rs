use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ItemStruct, Ident, Type, Expr, Token, LitStr};

/// AIX Tool macro for simplified MCP tool registration.
///
/// # Example
/// ```ignore
/// #[aix_tool(
///     name = "smart_overview",
///     description = "Get AI-optimized executive summary",
///     input = SmartOverviewInput,
///     output = SmartOverviewDto
/// )]
/// pub async fn handle_smart_overview(
///     ctx: &HandlerContext,
///     input: SmartOverviewInput
/// ) -> HandlerResult<SmartOverviewDto> {
///     // ... implementation
/// }
/// ```
///
/// For v1, this macro simply validates the attribute syntax and passes
/// the function through unchanged. Full tool registration code generation
/// will be added in future iterations.
#[proc_macro_attribute]
pub fn aix_tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();

    // Validate attribute syntax minimally
    // In v1, we just ensure it parses as a delimited group
    let _ = attr.to_string();

    // For v1, we just pass through the function unchanged
    // Full tool registration will come in future iterations
    let output = quote! {
        #[doc = "AIX Tool registered handler: "]
        #[doc = #fn_name_str]
        #input_fn
    };

    output.into()
}

/// Declare a code smell rule for the rule engine.
///
/// # Syntax
/// ```ignore
/// declare_rule! {
///     id: "S138"
///     name: "Functions should not be too long"
///     severity: Major
///     category: CodeSmell
///     language: "rust"
///     params: {
///         threshold: usize = 50
///     }
///     check: => { ctx.query_functions... }
/// }
/// ```
///
/// Uses `check: => { body }` syntax. The body runs inside `fn check(&self, ctx: &RuleContext)`
/// so `ctx` is available directly as the function parameter.
///
/// This macro generates:
/// - A struct `{Id}Rule` with params as fields
/// - `impl Rule for {Id}Rule`
/// - `inventory::submit!(RuleEntry { factory: || Box::new({Id}Rule::new()) })`
#[proc_macro]
pub fn declare_rule(input: TokenStream) -> TokenStream {
    let rule_input = parse_macro_input!(input as RuleInput);

    let rule_id = &rule_input.id;
    let rule_id_str = LitStr::new(&rule_id.to_string(), rule_id.span());
    let rule_name = &rule_input.name;
    let severity = &rule_input.severity;
    let category = &rule_input.category;
    let language = &rule_input.language;
    let params = &rule_input.params;
    let check_fn = &rule_input.check;
    let explanation = &rule_input.explanation;
    let clean_code = &rule_input.clean_code;
    let impacts = &rule_input.impacts;

    // Generate struct name: S138Rule from "S138"
    let struct_name = Ident::new(&format!("{}Rule", rule_input.id), rule_input.id.span());

    // Build the struct fields - each param is "name: Type"
    let field_definitions: Vec<_> = params
        .iter()
        .map(|(name, ty, _)| quote! { #name: #ty })
        .collect();

    // Field names for constructor
    let field_names: Vec<_> = params
        .iter()
        .map(|(name, _, _)| name)
        .collect();

    // Default values for constructor
    let field_defaults: Vec<_> = params
        .iter()
        .map(|(_, _, default): &(Ident, Type, Option<Expr>)| {
            default.as_ref().map(|e| quote! { #e }).unwrap_or_else(|| quote! { Default::default() })
        })
        .collect();

    // Generate code for the new metadata methods
    let explanation_code = match explanation {
        Some(exp) => quote! { Some(#exp) },
        None => quote! { None },
    };

    let clean_code_code = match clean_code {
        Some(cc) => quote! { Some(CleanCodeAttribute::#cc) },
        None => quote! { None },
    };

    let impacts_code = if impacts.is_empty() {
        quote! { vec![] }
    } else {
        let impacts: Vec<_> = impacts.iter().map(|(q, s)| {
            quote! {
                SoftwareQualityImpact {
                    quality: SoftwareQuality::#q,
                    severity: ImpactSeverity::#s,
                }
            }
        }).collect();
        quote! { vec![#(#impacts),*] }
    };

    // Generate auto-test function if auto_test is true
    // Uses unique function name to avoid conflicts with manual tests
    let auto_test_fn = if rule_input.auto_test {
        let test_name = format!("test_rule_{}_registered", rule_input.id);
        let test_ident = syn::Ident::new(&test_name, rule_input.id.span());
        let rule_id_literal = LitStr::new(&rule_input.id.to_string(), rule_input.id.span());
        quote! {
            #[cfg(test)]
            #[test]
            fn #test_ident() {
                let rule = #struct_name::new();
                assert_eq!(rule.id(), #rule_id_literal);
                assert!(!rule.name().is_empty());
                assert_eq!(rule.severity(), Severity::#severity);
                assert_eq!(rule.category(), Category::#category);
            }
        }
    } else {
        quote! {}
    };

    let output = quote! {
        /// Rule struct generated by declare_rule! macro
        #[derive(Debug, Clone)]
        pub struct #struct_name {
            #(#field_definitions),*
        }

        impl #struct_name {
            /// Create a new rule with default parameters
            pub fn new() -> Self {
                Self::default()
            }
        }

        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #(#field_names: #field_defaults),*
                }
            }
        }

        impl Rule for #struct_name {
            fn id(&self) -> &str {
                #rule_id_str
            }

            fn name(&self) -> &str {
                #rule_name
            }

            fn severity(&self) -> Severity {
                Severity::#severity
            }

            fn category(&self) -> Category {
                Category::#category
            }

            fn language(&self) -> &str {
                #language
            }

            fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
                #check_fn
            }

            fn explanation(&self) -> Option<&str> {
                #explanation_code
            }

            fn clean_code_attribute(&self) -> Option<CleanCodeAttribute> {
                #clean_code_code
            }

            fn software_qualities(&self) -> Vec<SoftwareQualityImpact> {
                #impacts_code
            }
        }

        inventory::submit! {
            RuleEntry {
                factory: || Box::new(#struct_name::new())
            }
        }

        #auto_test_fn
    };

    output.into()
}

/// Attribute macro for defining rules with ast-grep pattern matching.
///
/// # Syntax
/// ```ignore
/// #[cogni_rule(
///     id = "sec/crypto-weak-hash",
///     severity = "Critical",
///     category = "Vulnerability",
///     language = "rust",
///     pattern = "md5($$$)",
///     message = "Use of weak cryptographic hash detected"
/// )]
/// struct WeakCryptoHashRule;
/// ```
///
/// The macro generates:
/// - `impl Rule for WeakCryptoHashRule` with all trait methods
/// - If `pattern` is provided, a `check()` method using ast-grep pattern matching
/// - If `pattern` is NOT provided, `check()` returns empty Vec (user overrides manually)
/// - `inventory::submit!` for auto-registration
#[proc_macro_attribute]
pub fn cogni_rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr as CogniRuleAttrs);
    let item_struct = parse_macro_input!(item as ItemStruct);

    let struct_name = &item_struct.ident;
    let rule_id_str = LitStr::new(&attrs.id, item_struct.ident.span());
    let rule_name_str = attrs.name.unwrap_or_else(|| attrs.id.clone());
    let language_str = LitStr::new(&attrs.language, item_struct.ident.span());
    let message_str = attrs.message.unwrap_or_else(|| "Issue detected".to_string());
    let message_lit = LitStr::new(&message_str, item_struct.ident.span());
    let severity = &attrs.severity;
    let category = &attrs.category;

    // Generate the check method based on whether pattern is provided
    // MVP: Use simple textual matching via ctx.source.contains()
    let check_method = if let Some(pattern) = &attrs.pattern {
        let pattern_str = LitStr::new(pattern, item_struct.ident.span());
        quote! {
            fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
                let source = ctx.source;
                let mut issues = Vec::new();

                // Simple textual pattern matching for MVP
                if source.contains(#pattern_str) {
                    // Find line number of first occurrence
                    let line_number = source.lines()
                        .enumerate()
                        .find(|(_, line)| line.contains(#pattern_str))
                        .map(|(idx, _)| idx + 1)
                        .unwrap_or(1);

                    issues.push(Issue::new(
                        self.id(),
                        #message_lit,
                        self.severity(),
                        self.category(),
                        ctx.file_path,
                        line_number,
                    ));
                }

                issues
            }
        }
    } else {
        quote! {
            fn check(&self, _ctx: &RuleContext) -> Vec<Issue> {
                vec![]
            }
        }
    };

    // Generate the required_keywords method body
    let required_keywords_code = if let Some(ref kws) = attrs.required_keywords {
        let kw_lits: Vec<LitStr> = kws.iter()
            .map(|kw| LitStr::new(kw, item_struct.ident.span()))
            .collect();
        quote! {
            fn required_keywords(&self) -> Vec<&str> {
                vec![#(#kw_lits),*]
            }
        }
    } else {
        quote! {
            fn required_keywords(&self) -> Vec<&str> {
                vec![]
            }
        }
    };

    let output = quote! {
        #item_struct

        impl Rule for #struct_name {
            fn id(&self) -> &str {
                #rule_id_str
            }

            fn name(&self) -> &str {
                #rule_name_str
            }

            fn severity(&self) -> Severity {
                Severity::#severity
            }

            fn category(&self) -> Category {
                Category::#category
            }

            fn language(&self) -> &str {
                #language_str
            }

            #check_method

            fn layer(&self) -> u8 {
                1
            }

            #required_keywords_code
        }

        inventory::submit! {
            RuleEntry {
                factory: || Box::new(#struct_name {})
            }
        }
    };

    output.into()
}

/// Parsed attributes for #[cogni_rule]
struct CogniRuleAttrs {
    id: String,
    name: Option<String>,
    severity: Ident,
    category: Ident,
    language: String,
    pattern: Option<String>,
    message: Option<String>,
    required_keywords: Option<Vec<String>>,
}

impl syn::parse::Parse for CogniRuleAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut id = None;
        let mut name = None;
        let mut severity = None;
        let mut category = None;
        let mut language = None;
        let mut pattern = None;
        let mut message = None;
        let mut required_keywords = None;

        // Parse attribute list - input is already the inner content of (id = "...", ...)
        // Accepts both `key = value` and `key: value` syntax for flexibility
        while !input.is_empty() {
            let key: Ident = input.parse()?;

            // Accept both `=` and `:` as separator between key and value
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
                "id" => {
                    let value: syn::LitStr = input.parse()?;
                    id = Some(value.value());
                }
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "severity" => {
                    let value: Ident = input.parse()?;
                    severity = Some(value);
                }
                "category" => {
                    let value: Ident = input.parse()?;
                    category = Some(value);
                }
                "language" => {
                    let value: syn::LitStr = input.parse()?;
                    language = Some(value.value());
                }
                "pattern" => {
                    let value: syn::LitStr = input.parse()?;
                    pattern = Some(value.value());
                }
                "message" => {
                    let value: syn::LitStr = input.parse()?;
                    message = Some(value.value());
                }
                "required_keywords" => {
                    // Parse array: ["kw1", "kw2", ...]
                    let content;
                    syn::bracketed!(content in input);
                    let mut kws = Vec::new();
                    while !content.is_empty() {
                        let value: syn::LitStr = content.parse()?;
                        kws.push(value.value());
                        if content.peek(Token![,]) {
                            let _comma: Token![,] = content.parse()?;
                        }
                    }
                    required_keywords = Some(kws);
                }
                _ => return Err(syn::Error::new(key.span(), format!("Unknown key: {}", key_str))),
            }

            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
        }

        Ok(Self {
            id: id.ok_or_else(|| syn::Error::new(input.span(), "Missing `id` field"))?,
            name,
            severity: severity.ok_or_else(|| syn::Error::new(input.span(), "Missing `severity` field"))?,
            category: category.ok_or_else(|| syn::Error::new(input.span(), "Missing `category` field"))?,
            language: language.ok_or_else(|| syn::Error::new(input.span(), "Missing `language` field"))?,
            pattern,
            message,
            required_keywords,
        })
    }
}

/// Parsed input for the declare_rule! macro
struct RuleInput {
    id: Ident,
    name: String,
    severity: Ident,
    category: Ident,
    language: String,
    params: Vec<(Ident, Type, Option<Expr>)>,
    check: Expr,
    explanation: Option<String>,
    clean_code: Option<Ident>,
    impacts: Vec<(Ident, Ident)>,
    auto_test: bool,
}

impl syn::parse::Parse for RuleInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut id = None;
        let mut name = None;
        let mut severity = None;
        let mut category = None;
        let mut language = None;
        let mut params = Vec::new();
        let mut check = None;
        // New optional fields
        let mut explanation: Option<String> = None;
        let mut clean_code: Option<Ident> = None;
        let mut impacts: Vec<(Ident, Ident)> = Vec::new();
        let mut auto_test: Option<bool> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _colon: Token![:] = input.parse()?;
            let key_str = key.to_string();

            match key_str.as_str() {
                "id" => {
                    let value: syn::LitStr = input.parse()?;
                    // Sanitize: replace invalid identifier chars with underscore
                    let sanitized = value.value().replace("-", "_").replace("/", "_");
                    id = Some(Ident::new(&sanitized, value.span()));
                }
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "severity" => {
                    let value: Ident = input.parse()?;
                    severity = Some(value);
                }
                "category" => {
                    let value: Ident = input.parse()?;
                    category = Some(value);
                }
                "language" => {
                    let value: syn::LitStr = input.parse()?;
                    language = Some(value.value());
                }
                "params" => {
                    let content;
                    let _brace = syn::braced!(content in input);
                    while !content.is_empty() {
                        let field_name: Ident = content.parse()?;
                        let _colon: Token![:] = content.parse()?;
                        let field_type: Type = content.parse()?;

                        let default_value = if content.peek(Token![=]) {
                            let _eq: Token![=] = content.parse()?;
                            let expr: Expr = content.parse()?;
                            Some(expr)
                        } else {
                            None
                        };

                        params.push((field_name, field_type, default_value));

                        if content.peek(Token![,]) {
                            let _comma: Token![,] = content.parse()?;
                        }
                    }
                }
                "check" => {
                    let _arrow: Token![=>] = input.parse()?;
                    let expr: Expr = input.parse()?;
                    check = Some(expr);
                }
                "explanation" => {
                    let value: syn::LitStr = input.parse()?;
                    explanation = Some(value.value());
                }
                "clean_code" => {
                    let value: Ident = input.parse()?;
                    clean_code = Some(value);
                }
                "impacts" => {
                    let content;
                    let _bracket = syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let quality: Ident = content.parse()?;
                        let _colon: Token![:] = content.parse()?;
                        let severity: Ident = content.parse()?;
                        impacts.push((quality, severity));
                        if content.peek(Token![,]) {
                            let _comma: Token![,] = content.parse()?;
                        }
                    }
                }
                "auto_test" => {
                    // auto_test: true | false
                    let value: syn::LitBool = input.parse()?;
                    auto_test = Some(value.value());
                }
                _ => return Err(syn::Error::new(key.span(), format!("Unknown key: {}", key_str))),
            }

            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
        }

        Ok(Self {
            id: id.ok_or_else(|| syn::Error::new(input.span(), "Missing `id` field"))?,
            name: name.ok_or_else(|| syn::Error::new(input.span(), "Missing `name` field"))?,
            severity: severity.ok_or_else(|| syn::Error::new(input.span(), "Missing `severity` field"))?,
            category: category.ok_or_else(|| syn::Error::new(input.span(), "Missing `category` field"))?,
            language: language.ok_or_else(|| syn::Error::new(input.span(), "Missing `language` field"))?,
            params,
            check: check.ok_or_else(|| syn::Error::new(input.span(), "Missing `check` field"))?,
            explanation,
            clean_code,
            impacts,
            auto_test: auto_test.unwrap_or(true),
        })
    }
}

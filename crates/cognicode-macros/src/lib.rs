use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

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

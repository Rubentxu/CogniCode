use super::{HandlerError, HandlerResult};

/// Calculates complexity metrics for a function. A parse failure is an error
/// and a function that does not exist is `NotFound`: neither is reported as a
/// measurement.
pub(super) fn calculate_function_complexity(
    parser: &crate::infrastructure::parser::TreeSitterParser,
    source: &str,
    function_name: Option<&str>,
) -> HandlerResult<(u32, u32, u32, u32, u32)> {
    let tree = parser.parse_tree(source).map_err(|e| {
        HandlerError::App(crate::application::error::AppError::AnalysisError(format!(
            "Parse error: {e}"
        )))
    })?;
    crate::infrastructure::complexity::function_metrics(
        &tree,
        source,
        function_name,
        parser.language().symbols().function_types,
    )
    .ok_or_else(|| {
        HandlerError::NotFound(format!(
            "Function not found: {}",
            function_name.unwrap_or("<first function>")
        ))
    })
}

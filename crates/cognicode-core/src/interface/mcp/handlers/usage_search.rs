//! Where a handler gets the usages of a symbol: off the runtime that serves it.
//!
//! ## Why it has its own home
//!
//! The search walks the tree and reads the files of it: hundreds of
//! milliseconds of disk that never yield. The handlers are `async`, so
//! doing that on the task that serves them holds the runtime's thread
//! for as long as it takes, and the server answers nothing meanwhile —
//! not even a call that needs no tree.
//!
//! Worse, the deadline that is supposed to cut the call short cannot
//! fire while the poll is stuck inside the search: a deadline is
//! observed when the future is polled again, and the poll is inside the
//! search until it finishes. The answer then arrives after the deadline
//! that should have replaced it, and the caller is left with a timeout
//! and a search whose work was thrown away.
//!
//! The search itself is the application's `find_symbol_usages_with_coverage`,
//! with the budget the context names. This module only moves it off the
//! runtime, and every handler that needs a search goes through it: a
//! handler that called the application directly would bring the stall
//! back.
//!
//! ## What lives here
//!
//! The two handlers that answer "where is this symbol used", and the
//! call that moves the search off the runtime. They live together
//! because the move is part of their contract: a handler that reached
//! for the application on its own would bring the stall back.

use crate::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use crate::application::services::usage_search::UsageSearch;
use crate::interface::mcp::schemas::{
    ContextLines, FindUsagesInput, FindUsagesOutput, FindUsagesWithContextInput,
    FindUsagesWithContextOutput, UsageWithContextEntry,
};
use crate::interface::mcp::security::InputValidator;
use std::path::PathBuf;
use std::sync::Arc;

use super::{HandlerContext, HandlerError, HandlerResult};

/// Handler for find_usages tool
#[cognicode_macros::aix_tool(
    name = "find_usages",
    description = "Find all usages of a symbol across the project.",
    input_schema = FindUsagesInput
)]
pub async fn handle_find_usages(
    ctx: &HandlerContext,
    input: FindUsagesInput,
) -> HandlerResult<FindUsagesOutput> {
    ctx.validator.validate_query(&input.symbol_name)?;

    let search = search_off_the_runtime(
        ctx.analysis_service.clone(),
        UsageSearchParams {
            project_dir: ctx.working_dir.clone(),
            symbol_name: input.symbol_name.clone(),
            include_declaration: input.include_declaration,
            context_lines: input.context_lines,
            first_only_definition: true,
            budget: ctx.usage_search_budget,
        },
    )
    .await?;

    Ok(FindUsagesOutput::of_search(input.symbol_name, search))
}

/// Handler for find_usages_with_context tool
///
/// Migrated to use Validation capability trait directly.
pub async fn handle_find_usages_with_context(
    validator: Arc<InputValidator>,
    working_dir: PathBuf,
    input: FindUsagesWithContextInput,
) -> HandlerResult<FindUsagesWithContextOutput> {
    validator.validate_query(&input.symbol)?;

    let search = search_off_the_runtime(
        Arc::new(AnalysisService::new()),
        UsageSearchParams {
            project_dir: working_dir,
            symbol_name: input.symbol.clone(),
            include_declaration: input.include_declaration,
            context_lines: Some(input.context_lines as usize),
            first_only_definition: false,
            budget: crate::application::services::usage_search::USAGE_SEARCH_BUDGET,
        },
    )
    .await?;

    let total = search.usages.len();

    let usage_entries: Vec<UsageWithContextEntry> = search
        .usages
        .into_iter()
        .map(|u| UsageWithContextEntry {
            file: u.file,
            line: u.line,
            column: u.column,
            context: u.context,
            context_lines: u.context_lines.map_or(
                ContextLines {
                    before: vec![],
                    current: String::new(),
                    after: vec![],
                },
                |c| ContextLines {
                    before: c.before,
                    current: c.current,
                    after: c.after,
                },
            ),
            is_definition: u.is_definition,
        })
        .collect();

    Ok(FindUsagesWithContextOutput {
        symbol: input.symbol,
        usages: usage_entries,
        total,
        coverage: crate::interface::coverage::CoverageInfo::of(&search.coverage),
    })
}

/// The usages of a symbol in a tree, found on a thread that may block.
pub(super) async fn search_off_the_runtime(
    service: Arc<AnalysisService>,
    params: UsageSearchParams,
) -> HandlerResult<UsageSearch> {
    tokio::task::spawn_blocking(move || service.find_symbol_usages_with_coverage(params))
        .await
        .map_err(|e| HandlerError::Internal(format!("spawn_blocking panicked: {e}")))?
        .map_err(HandlerError::App)
}

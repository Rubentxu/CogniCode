//! The usage search: every occurrence of a symbol across the source tree,
//! and what the search covered.
//!
//! The walk is the inventory's, so the files are the ones the graph is built
//! from, with the security blocklist and the performance skips applied by the
//! one walk that owns them. A file the walk lists and the search cannot read
//! is a hole in the answer, not a file without usages.
//!
//! The search has a budget. A tree can be bigger than a consumer can wait
//! for, and the cost of the answer depends on the tree and not on the
//! question, so the search stops at its budget and says what it stopped at:
//! `partial`, the number of files it left out, and the reason. An answer
//! that covers a part of the tree never reads as an answer about the whole
//! tree.

use crate::application::analysis_basis::Budget;
use crate::application::error::AppResult;
use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::analyze::Coverage;
use crate::domain::value_objects::{published_column, published_line};
use std::time::Instant;

/// The budget the product's own surfaces give the usage search.
///
/// One number for the two of them: the CLI and the MCP answer the same
/// question about the same tree, and two numbers would be two answers to
/// the same question. Ten seconds is long enough for a real repository
/// (the search of this one takes a quarter of a second) and short enough
/// that a consumer does not wait forever for a tree nobody can read in
/// time. Fifty thousand files is the other side of the same bound: it is
/// a tree twenty times bigger than this one, and the time limit is the
/// one that stops first.
pub const USAGE_SEARCH_BUDGET: Budget = Budget {
    max_elapsed: Some(std::time::Duration::from_secs(10)),
    max_entities: Some(50_000),
};

/// Why the search has to stop, if it has to.
///
/// The two bounds are the two the domain's budget names: the files it may
/// read and the time it may take. The reason names the bound that was
/// reached, because a consumer that knows "the search stopped" cannot tell
/// whether the tree was too big or the machine was too slow.
fn budget_stopped(budget: &Budget, started: Instant, read: usize) -> Option<String> {
    if let Some(max) = budget.max_entities
        && read >= max
    {
        return Some(format!("the file budget ({max}) was exhausted"));
    }
    if let Some(max) = budget.max_elapsed
        && started.elapsed() >= max
    {
        return Some(format!("the time budget ({max:?}) was exhausted"));
    }
    None
}

#[derive(Debug, Clone)]
pub struct UsageSearchParams {
    pub project_dir: std::path::PathBuf,
    pub symbol_name: String,
    pub include_declaration: bool,
    pub context_lines: Option<usize>,
    pub first_only_definition: bool,
    /// How much of the tree the search may read.
    ///
    /// The search reads the files of the tree until it has read them
    /// all or until the budget stops it. A search that stops says so —
    /// `partial`, the number of files it left out, and the reason — so
    /// that an answer that covers a part of the tree never reads as an
    /// answer about the whole tree.
    ///
    /// The budget is consulted between files, so one enormous file is
    /// read whole: the bound is on the number of files and on the time
    /// the search spends, not on the size of a single read.
    pub budget: Budget,
}

#[derive(Debug, Clone)]
pub struct UsageResult {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub is_definition: bool,
    pub context_lines: Option<ContextData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextData {
    pub before: Vec<String>,
    pub current: String,
    pub after: Vec<String>,
}

/// Gets surrounding context lines for a given 0-indexed line
fn context_data(source: &str, line: usize, context_size: usize) -> ContextData {
    let lines: Vec<&str> = source.lines().collect();

    let before_start = line.saturating_sub(context_size);
    let after_end = (line + 1 + context_size).min(lines.len());

    let before: Vec<String> = (before_start..line)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    let current = lines.get(line).unwrap_or(&"").to_string();

    let after: Vec<String> = ((line + 1)..after_end)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    ContextData {
        before,
        current,
        after,
    }
}

/// What a usage search found and what it covered.
///
/// The usages are the answer; the coverage says how much of the tree the
/// answer is about. They travel together because a list of usages with no
/// word about the files that could not be read is a list that reads as
/// complete.
#[derive(Debug, Clone)]
pub struct UsageSearch {
    /// Every occurrence of the symbol the search could read.
    pub usages: Vec<UsageResult>,
    /// What the search covered, in the vocabulary every other answer uses.
    pub coverage: Coverage,
    /// How many files the search did not read because its budget stopped
    /// it. `None` when the search read the whole tree.
    pub omitted: Option<usize>,
    /// Why the search stopped before the end of the tree. `None` when it
    /// read the whole tree.
    pub truncation_reason: Option<String>,
}

/// One place where an identifier occurs in a file: what the reader of the
/// file hands to the search, with positions as the reader counts them
/// (0-based).
pub(crate) struct Occurrence {
    pub line: u32,
    pub column: u32,
    /// The line of code the identifier is on.
    pub context: String,
    /// Whether this occurrence is the name a declaration declares.
    ///
    /// The reader of the file knows it: it is the node the declaration
    /// writes its name in. The search does not guess it from the text
    /// around the occurrence —a line that says `fn` is not a
    /// declaration, and a name that nothing declares is not one either—.
    pub is_declaration: bool,
}

/// What reading one file said about an identifier.
///
/// The source travels with the occurrences because the search cuts the
/// lines around each one from it: the text the occurrences were found in
/// is the text the context comes from.
pub(crate) struct FileOccurrences {
    pub source: String,
    pub occurrences: Vec<Occurrence>,
}

impl AnalysisService {
    /// Finds all textual/AST occurrences of a symbol across the project
    /// source tree, skipping dependency/build/cache directories.
    ///
    /// The usages only: a caller that publishes the answer asks
    /// [`AnalysisService::find_symbol_usages_with_coverage`], which says
    /// what the answer does not cover.
    pub fn find_symbol_usages(&self, params: UsageSearchParams) -> AppResult<Vec<UsageResult>> {
        Ok(self.find_symbol_usages_with_coverage(params)?.usages)
    }

    /// Finds all textual/AST occurrences of a symbol across the project
    /// source tree, and says what the search covered.
    ///
    /// A file the walk lists and the search cannot read, or the parser
    /// cannot take, is not a file without usages: it is a hole, and it is
    /// named in the coverage with its cause instead of vanishing in a
    /// `continue`. The holes of the walk itself (a symlink it declined, a
    /// directory it could not read, a language nobody supports yet) are the
    /// inventory's, composed in the one place that owns them.
    pub fn find_symbol_usages_with_coverage(
        &self,
        params: UsageSearchParams,
    ) -> AppResult<UsageSearch> {
        let mut usages = Vec::new();
        let mut seen_first_definition = false;
        // The files are the tree's: the same ones the graph is built from,
        // with the security blocklist and the performance skips applied by
        // the one walk that owns them.
        let tree = self.tree_state(&params.project_dir);
        let mut holes = tree.holes;
        let total_files = tree.files.len();
        let started = Instant::now();
        let mut read = 0usize;
        // The budget is consulted before the file is read, so a search
        // that stops does not read one file more than it can afford.
        let mut truncation: Option<String> = None;
        for file in &tree.files {
            if let Some(bound) = budget_stopped(&params.budget, started, read) {
                let reason =
                    format!("the search stopped after {read} of {total_files} files ({bound})");
                truncation = Some(reason.clone());
                // The file the search did not reach is a hole: it was
                // not read, and neither were the files after it.
                holes.push(crate::application::services::analyze::budget_hole(
                    file.path.to_string_lossy().into_owned(),
                    reason,
                ));
                break;
            }
            let path = file.path.as_path();
            let found = self.identifier_in_file(path, file.language, &params.symbol_name);
            read += 1;
            let FileOccurrences {
                source,
                occurrences,
            } = match found {
                Ok(found) => found,
                Err(hole) => {
                    holes.push(hole);
                    continue;
                }
            };

            for occ in occurrences {
                // Whether this occurrence is the declaration is the
                // reader's answer, not a guess about the words around it:
                // the first occurrence that mentions `fn` is not the
                // declaration, it is the first line that happens to say
                // `fn`.
                let declares = occ.is_declaration;

                let is_definition = if params.first_only_definition {
                    declares && !seen_first_definition
                } else {
                    declares
                };

                if declares && !seen_first_definition {
                    seen_first_definition = true;
                }

                if !params.include_declaration && is_definition {
                    continue;
                }

                let context_lines = params
                    .context_lines
                    .map(|ctx| context_data(&source, occ.line as usize, ctx));

                usages.push(UsageResult {
                    file: path.to_string_lossy().into_owned(),
                    line: published_line(occ.line),
                    column: published_column(occ.column),
                    context: occ.context.clone(),
                    is_definition,
                    context_lines,
                });
            }
        }

        // The tree holds something to search when at least one file in a
        // language the product reads was in the walk.
        let coverage =
            crate::application::services::analyze::scan_coverage(holes, !tree.files.is_empty());
        let omitted = truncation.as_ref().map(|_| total_files - read);
        Ok(UsageSearch {
            usages,
            coverage,
            omitted,
            truncation_reason: truncation,
        })
    }
}

//! The usage search: every occurrence of a symbol across the source tree,
//! and what the search covered.
//!
//! The walk is the inventory's, so the files are the ones the graph is built
//! from, with the security blocklist and the performance skips applied by the
//! one walk that owns them. A file the walk lists and the search cannot read
//! is a hole in the answer, not a file without usages.

use crate::application::error::AppResult;
use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::analyze::Coverage;
use crate::domain::value_objects::{published_column, published_line};
use crate::infrastructure::parser::TreeSitterParser;

#[derive(Debug, Clone)]
pub struct UsageSearchParams {
    pub project_dir: std::path::PathBuf,
    pub symbol_name: String,
    pub include_declaration: bool,
    pub context_lines: Option<usize>,
    pub first_only_definition: bool,
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
        use crate::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};

        let mut usages = Vec::new();
        let mut seen_first_definition = false;
        // The files are the inventory's: the same ones the graph is built
        // from, with the security blocklist and the performance skips
        // applied by the one walk that owns them.
        let inventory = crate::infrastructure::graph::source_files::inventory(&params.project_dir);
        let mut holes = inventory.coverage_holes();
        let mut searched_files = 0usize;
        for file in inventory.source_files() {
            let path = file.path.as_path();
            let Some(language) = file.language else {
                continue;
            };
            searched_files += 1;

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(error) => {
                    holes.push(crate::infrastructure::graph::source_files::unreadable(
                        path, &error,
                    ));
                    continue;
                }
            };

            let parser = match TreeSitterParser::new(language) {
                Ok(p) => p,
                Err(error) => {
                    holes.push(SkippedFile {
                        path: path.to_string_lossy().into_owned(),
                        reason: SkipReason::Parse(error.to_string()),
                    });
                    continue;
                }
            };

            let occurrences =
                match parser.find_all_occurrences_of_identifier(&source, &params.symbol_name) {
                    Ok(occurrences) => occurrences,
                    Err(error) => {
                        holes.push(SkippedFile {
                            path: path.to_string_lossy().into_owned(),
                            reason: SkipReason::Parse(error.to_string()),
                        });
                        continue;
                    }
                };

            for occ in occurrences {
                let has_def_keyword = occ.context.contains("def ")
                    || occ.context.contains("class ")
                    || occ.context.contains("struct ")
                    || occ.context.contains("fn ")
                    || occ.context.contains("function ")
                    || occ.context.contains("const ")
                    || occ.context.contains("let ");

                let is_definition = if params.first_only_definition {
                    has_def_keyword && !seen_first_definition
                } else {
                    has_def_keyword
                };

                if has_def_keyword && !seen_first_definition {
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
            crate::application::services::analyze::scan_coverage(holes, searched_files > 0);
        Ok(UsageSearch { usages, coverage })
    }
}

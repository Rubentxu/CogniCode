//! Deterministic content search and its revision-bound continuation.

use super::*;

fn tree_revision(
    entries: &[ignore::DirEntry],
    root: &Path,
    bound: Option<&Bound>,
) -> Option<String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for entry in entries {
        if bound.is_some_and(Bound::reached) {
            return None;
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            let name = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy();
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            let bytes = fs::read(entry.path()).ok()?;
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Some(format!("{:x}", hash.finalize()))
}

fn current_revision(walker: &WalkBuilder, root: &Path, bound: Option<&Bound>) -> Option<String> {
    let mut entries = Vec::new();
    for entry in walker.build() {
        if bound.is_some_and(Bound::reached) {
            return None;
        }
        entries.push(entry.ok()?);
    }
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    tree_revision(&entries, root, bound)
}

impl FileOperationsService {
    /// The same search, with a bound the walk answers to.
    ///
    /// The walk stops when the bound passes, and the answer says that it
    /// stopped: a walk that was cut is not a walk that read everything, and
    /// the answer must not publish it as one. The hole it leaves says which
    /// bound was reached.
    pub(crate) fn search_content_until(
        &self,
        input: SearchContentRequest,
        bound: Option<Bound>,
    ) -> AppResult<SearchContentResult> {
        self.search_content_with_hook(input, bound, || {})
    }

    fn search_content_with_hook(
        &self,
        input: SearchContentRequest,
        bound: Option<Bound>,
        after_revision: impl FnOnce(),
    ) -> AppResult<SearchContentResult> {
        let search_path = input
            .path
            .as_ref()
            .map(|p| self.validate_path(p))
            .unwrap_or_else(|| Ok(self.workspace_root.clone()))?;

        let max_results = input.max_results.unwrap_or(50);
        if max_results == 0 {
            return Err(AppError::InvalidParameter(
                "max_results must be greater than zero".into(),
            ));
        }
        if input.cursor.is_some() && input.offset.is_some() {
            return Err(AppError::InvalidParameter(
                "cursor and offset cannot be combined".into(),
            ));
        }
        let context_lines = input.context_lines.unwrap_or(2) as usize;
        let case_insensitive = input.case_insensitive.unwrap_or(false);
        let is_regex = input.regex.unwrap_or(true);

        // Validate regex pattern if regex mode is enabled
        if is_regex && let Err(msg) = Self::validate_regex_pattern(&input.pattern) {
            return Err(AppError::InvalidParameter(format!(
                "Invalid regex pattern: {}",
                msg
            )));
        }

        // Build walker with gitignore awareness
        //
        // `require_git(false)`: the `.gitignore` of a project is the
        // project's own declaration of what is out of bounds, and
        // whether somebody has run `git init` is not the project's
        // business. `ignore` applies the git rules only inside a
        // repository by default, so a tree that nobody has committed
        // yet was read whole. The law lives in
        // `infrastructure::walk_policy`; the application cannot name
        // that module (CR-06), so the setting whose default is the
        // wrong way round is written here.
        let mut walker = WalkBuilder::new(&search_path);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .parents(true)
            .require_git(false);

        let mut entries = Vec::new();
        let mut walk_errors = Vec::new();
        for result in walker.build() {
            if bound.as_ref().is_some_and(|bound| bound.reached()) {
                break;
            }
            match result {
                Ok(entry) => entries.push(entry),
                Err(error) => walk_errors.push(error),
            }
        }
        entries.sort_by(|a, b| a.path().cmp(b.path()));

        // Revision captures content, not metadata; incomplete walks have no cursor.
        let revision = if walk_errors.is_empty() && !bound.as_ref().is_some_and(Bound::reached) {
            tree_revision(&entries, Path::new(&search_path), bound.as_ref())
        } else {
            None
        };
        after_revision();
        let definition = sha256_hex(
            format!(
                "{:?}",
                (
                    &search_path,
                    &input.pattern,
                    &input.file_glob,
                    is_regex,
                    case_insensitive,
                    context_lines
                )
            )
            .as_bytes(),
        );
        let start = if let Some(cursor) = &input.cursor {
            let parts: Vec<_> = cursor.split(':').collect();
            if parts.len() != 4 || parts[0] != "v1" {
                return Err(AppError::InvalidParameter("invalid search cursor".into()));
            }
            if revision.as_deref() != Some(parts[1]) {
                return Err(AppError::InvalidParameter(
                    "search tree revision changed or could not be read".into(),
                ));
            }
            if parts[2] != definition {
                return Err(AppError::InvalidParameter(
                    "search cursor does not match the search definition".into(),
                ));
            }
            parts[3]
                .parse::<usize>()
                .map_err(|_| AppError::InvalidParameter("invalid search cursor offset".into()))?
        } else {
            input.offset.unwrap_or(0)
        };
        let limit = start.saturating_add(max_results).saturating_add(1);

        // Prepared once for the tree. A pattern that cannot match (an empty
        // literal) still walks, so the files are counted, and reads none.
        let pattern = ContentPattern::new(&input.pattern, is_regex, case_insensitive);
        let mut indexed = IndexedWalk::over(self.text_index.as_ref(), &input.pattern, is_regex);

        let mut matches = Vec::new();
        let mut files_scanned = 0;
        let mut unread: Vec<_> = walk_errors.iter().map(walk_failure).collect();
        // Whether the limit stopped the walk, and why it stopped it. The
        // walk that stops early did not cover the tree, and an answer that
        // says it did is the lie this product does not tell: the tree may
        // hold more matches, and the caller cannot know.
        let mut truncation: Option<String> = None;
        // One owner for the wording of the cut: the two places that stop
        // the walk publish the same sentence, so the answer names the same
        // limit wherever the walk was stopped.
        let truncation_reason = || {
            format!(
                "the search stopped at max_results={max_results}: the tree may hold more matches"
            )
        };

        for entry in entries {
            if matches.len() >= limit {
                truncation = Some(truncation_reason());
                break;
            }

            // The walk answers to a bound: when it passes, the walk stops
            // and the answer says that it stopped. A walk that reads the
            // whole tree of a big repository is the one part of this work
            // that nothing else bounds.
            if let Some(bound) = &bound
                && bound.reached()
            {
                break;
            }

            if let Some(hole) = declined_symlink(&entry) {
                unread.push(hole);
                continue;
            }

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Apply glob filter if provided
            if let Some(glob) = &input.file_glob
                && !path_matches_glob(path, glob)
            {
                continue;
            }

            files_scanned += 1;

            // Skip binary files
            if Self::is_binary_file(path) {
                continue;
            }

            let Some(pattern) = &pattern else {
                continue;
            };

            let content = match indexed.read_text(path, entry.metadata().ok().as_ref()) {
                FileRead::Text(content) => content,
                FileRead::Skipped | FileRead::NotText => continue,
                FileRead::Failed(error) => {
                    unread.push(read_failure(path, &error));
                    continue;
                }
            };

            // Search for pattern
            let search_matches = pattern.find_in(&content, context_lines);

            for (line_num, col, text, context) in search_matches {
                if matches.len() >= limit {
                    truncation = Some(truncation_reason());
                    break;
                }
                matches.push(ContentMatch {
                    file: path.to_string_lossy().to_string(),
                    line: line_num,
                    col,
                    text,
                    context,
                });
            }
        }

        if let Some(bound) = &bound
            && bound.reached()
        {
            unread.push(budget_cut(bound.detail()));
        }
        matches.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
        let end = start.saturating_add(max_results);
        let has_more = matches.len() > end;
        if has_more {
            truncation = Some(truncation_reason());
        }
        if let Some(before) = revision.as_ref()
            && !bound.as_ref().is_some_and(Bound::reached)
            && current_revision(&walker, Path::new(&search_path), bound.as_ref()).as_ref()
                != Some(before)
        {
            return Err(AppError::InvalidParameter(
                "search tree revision changed during search".into(),
            ));
        }
        let next_cursor = revision
            .as_ref()
            .filter(|_| {
                has_more && unread.is_empty() && !bound.as_ref().is_some_and(Bound::reached)
            })
            .map(|revision| format!("v1:{revision}:{definition}:{end}"));
        let matches: Vec<_> = matches.into_iter().skip(start).take(max_results).collect();
        let total = matches.len();

        // The limit stopped the walk: the answer is about a part of the
        // tree, and it says so. The hole is what turns the coverage from
        // "complete" into "partial" — a search that stopped early did not
        // cover the tree it was asked about.
        if let Some(reason) = &truncation {
            unread.push(budget_cut(reason));
        }

        let complete = truncation.is_none() && unread.is_empty() && input.file_glob.is_none();
        let text_index = indexed.finish(Path::new(&search_path), complete);

        Ok(SearchContentResult {
            matches,
            total,
            files_scanned,
            unread,
            text_index,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
    use crate::infrastructure::verification::RustVerifier;
    use crate::interface::mcp::security::InputValidator;
    use std::sync::Arc;

    #[test]
    fn mutation_between_revision_and_search_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let path = root.join("a.rs");
        fs::write(&path, "beta beta").unwrap();
        let service = FileOperationsService::new(
            root.to_string_lossy().to_string(),
            Arc::new(InputValidator::new().with_workspace(vec![root])),
            Arc::new(RustVerifier::new()),
            Arc::new(TreeSitterSyntaxAnalysis::new()),
        );
        let mut request = SearchContentRequest::new("beta");
        request.regex = Some(false);
        request.max_results = Some(1);
        let error = service
            .search_content_with_hook(request, None, || {
                fs::write(&path, "zeta beta").unwrap();
            })
            .unwrap_err();
        assert!(error.to_string().contains("revision"), "{error}");
    }
}

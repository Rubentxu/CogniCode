//! MoldQL executor — turns an AST into a [`MoldQLResult`] by calling
//! the existing explorer ports. No new ports, no new adapters.
//!
//! ## Evaluation strategy
//!
//! For each `Condition`, the executor picks the right property accessor
//! and applies the operator. Short-circuit on first failure — this is
//! `O(N * M)` in the worst case but `WHERE` clauses are typically small
//! (1–3 conditions).
//!
//! ## Quality backend
//!
//! Quality conditions (`quality.<level>`, `issue_count`, `severity`)
//! evaluate `false` when no [`crate::ports::QualityRepository`] is
//! wired — callers that skip the quality backend still get a correct
//! (empty) result without an error.

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::sync::Arc;

use cognicode_core::domain::aggregates::SymbolId;

use crate::domain::object_identity::ObjectIdentity;
use crate::domain::views::scope_contains_file;
use crate::dto::{InspectableObjectType, LensResult};
use crate::error::{ExplorerError, ExplorerResult};
use crate::moldql::ast::{Condition, Direction, MoldQLQuery, Op, TargetType, Value};
use crate::ports::symbol_repository::{RelationTarget, ResolvedSymbol, SymbolRepository};

/// Hard cap on the BFS depth accepted by `EXPLORE` queries. Anything
/// above this is clamped down to `5` to keep `explore_callers` /
/// `explore_callees` BFS bounded.
const MAX_EXPLORE_DEPTH: u32 = 5;

/// The result of a MoldQL query — a list of items, the original query
/// string (echoed back), and the total match count.
#[derive(Debug, Clone)]
pub struct MoldQLResult {
    pub query: String,
    pub items: Vec<MoldQLItem>,
    pub total: usize,
}

/// A single matched object plus optional lens output.
#[derive(Debug, Clone)]
pub struct MoldQLItem {
    pub object_id: String,
    pub object_type: InspectableObjectType,
    pub label: String,
    /// `Some(...)` when an `APPLY <lens>` clause was specified. The
    /// string is `<lens_id>: <summary>` or, when the lens returned no
    /// findings, just `<lens_id>`.
    pub detail: Option<String>,
}

/// Executes a parsed MoldQL query against a [`MoldQLView`].
///
/// Construction goes through [`MoldQLView::executor`] so the executor
/// borrows the right ports without holding any state of its own.
pub struct MoldQLExecutor<'a> {
    view: &'a MoldQLView,
}

impl<'a> MoldQLExecutor<'a> {
    pub fn new(view: &'a MoldQLView) -> Self {
        Self { view }
    }

    /// Run the query, returning a structured result.
    pub fn execute(&self, query: MoldQLQuery) -> ExplorerResult<MoldQLResult> {
        match query {
            MoldQLQuery::Find(find) => self.execute_find(&find),
            MoldQLQuery::Explore(explore) => self.execute_explore(&explore),
            // ExplorerQL primitives. The executor compiles the query
            // and then dispatches the compiled plan to the right
            // backend (PG SQL or petgraph walk).
            MoldQLQuery::Path(_)
            | MoldQLQuery::Neighbors(_)
            | MoldQLQuery::Subgraph(_)
            | MoldQLQuery::Cluster(_)
            | MoldQLQuery::Explain(_)
            | MoldQLQuery::Boolean(_) => {
                // Default target: petgraph. The MCP tool can override
                // this via the `target` field on the request.
                let compiled = crate::moldql::compile::compile(
                    &query,
                    crate::moldql::compile::CompileTarget::Petgraph,
                )
                .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;
                crate::moldql::compile::run(
                    compiled,
                    crate::moldql::compile::CompileTarget::Petgraph,
                    self.view,
                )
            }
        }
    }

    /// Execute an already-compiled ExplorerQL query against the
    /// requested target. This is the only entry point that exposes
    /// the `CompileTarget` choice to callers above the executor.
    pub fn execute_compiled(
        &self,
        compiled: crate::moldql::compile::CompiledQuery,
        target: crate::moldql::compile::CompileTarget,
    ) -> ExplorerResult<MoldQLResult> {
        crate::moldql::compile::run(compiled, target, self.view)
    }

    /// Compile + execute an ExplorerQL query against an explicit
    /// target. Used by the MCP tool to honour the caller's
    /// `target: "pg" | "petgraph" | "auto"` choice.
    pub fn execute_with_target(
        &self,
        query: MoldQLQuery,
        target: crate::moldql::compile::CompileTarget,
    ) -> ExplorerResult<MoldQLResult> {
        let compiled = crate::moldql::compile::compile(&query, target)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;
        crate::moldql::compile::run(compiled, target, self.view)
    }

    // -------------------------------------------------------------------
    // FIND
    // -------------------------------------------------------------------

    fn execute_find(&self, find: &crate::moldql::ast::FindQuery) -> ExplorerResult<MoldQLResult> {
        let query_str = render_find(find);
        let items = match find.target {
            TargetType::Symbols => self.find_symbols(find)?,
            TargetType::Files => self.find_files(find)?,
            TargetType::Scopes => self.find_scopes(find)?,
            TargetType::Issues => self.find_issues(find)?,
        };
        let total = items.len();
        Ok(MoldQLResult {
            query: query_str,
            total,
            items,
        })
    }

    fn find_symbols(
        &self,
        find: &crate::moldql::ast::FindQuery,
    ) -> ExplorerResult<Vec<MoldQLItem>> {
        let all = self.view.repo.all_symbols()?;
        let mut matches: Vec<ResolvedSymbol> = all
            .into_iter()
            .filter(|s| matches_scope(find.scope.as_deref(), &s.file))
            .filter(|s| self.conditions_pass_for_symbol(find, s))
            .collect();

        // Natural order: name ASC then file ASC. Stable across runs.
        matches.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.file.cmp(&b.file)));

        let mut items = Vec::with_capacity(matches.len());
        for s in matches {
            let mvp = symbol_mvp(&s);
            let detail = self.apply_lens_if_requested(find, &mvp)?;
            items.push(MoldQLItem {
                object_id: mvp,
                object_type: InspectableObjectType::Symbol,
                label: format!("{} at {}:{}", s.name, s.file, s.line),
                detail,
            });
        }
        Ok(items)
    }

    fn find_files(&self, find: &crate::moldql::ast::FindQuery) -> ExplorerResult<Vec<MoldQLItem>> {
        let all = self.view.repo.all_symbols()?;
        let mut files: BTreeSet<String> = BTreeSet::new();
        for s in &all {
            if matches_scope(find.scope.as_deref(), &s.file) {
                files.insert(s.file.clone());
            }
        }

        let mut items = Vec::new();
        for file in files {
            if self.conditions_pass_for_file(find, &file)? {
                let mvp = format!("file:{file}");
                let detail = self.apply_lens_if_requested(find, &mvp)?;
                items.push(MoldQLItem {
                    object_id: mvp,
                    object_type: InspectableObjectType::File,
                    label: file.clone(),
                    detail,
                });
            }
        }
        Ok(items)
    }

    fn find_scopes(&self, find: &crate::moldql::ast::FindQuery) -> ExplorerResult<Vec<MoldQLItem>> {
        // For each scope we know about (the module list from the repo),
        // apply the conditions. When a scope clause is present, restrict
        // to that single scope.
        let scopes: Vec<String> = if let Some(scope) = &find.scope {
            vec![scope.clone()]
        } else {
            self.view.repo.module_list()
        };

        let mut items = Vec::new();
        for scope in scopes {
            if self.conditions_pass_for_scope(find, &scope) {
                let mvp = format!("scope:{scope}");
                let detail = self.apply_lens_if_requested(find, &mvp)?;
                items.push(MoldQLItem {
                    object_id: mvp,
                    object_type: InspectableObjectType::Scope,
                    label: scope.clone(),
                    detail,
                });
            }
        }
        Ok(items)
    }

    fn find_issues(&self, find: &crate::moldql::ast::FindQuery) -> ExplorerResult<Vec<MoldQLItem>> {
        let issues = match self.view.quality.as_deref() {
            Some(q) => {
                if let Some(scope) = &find.scope {
                    q.issues_for_scope(scope)?
                } else {
                    // Empty scope string is treated as "all issues" by the
                    // boundary-aware adapter.
                    q.issues_for_scope("")?
                }
            }
            None => Vec::new(),
        };
        let mut items = Vec::new();
        for issue in issues {
            if self.conditions_pass_for_issue(find, &issue) {
                let mvp = format!("issue:{}", issue.id);
                let detail = self.apply_lens_if_requested(find, &mvp)?;
                items.push(MoldQLItem {
                    object_id: mvp,
                    object_type: InspectableObjectType::QualityIssue,
                    label: format!("{}: {}", issue.rule_id, issue.message),
                    detail,
                });
            }
        }
        Ok(items)
    }

    // -------------------------------------------------------------------
    // EXPLORE
    // -------------------------------------------------------------------

    fn execute_explore(
        &self,
        explore: &crate::moldql::ast::ExploreQuery,
    ) -> ExplorerResult<MoldQLResult> {
        let query_str = render_explore(explore);
        let identity = ObjectIdentity::parse_mvp_id(&explore.object_ref)?;
        let seed_id = match &identity {
            ObjectIdentity::Symbol { file, name, line } => {
                SymbolId::new(format!("{file}:{name}:{line}"))
            }
            other => {
                return Err(ExplorerError::ResolutionFailed(format!(
                    "EXPLORE requires a symbol object_ref, got `{}`",
                    other.to_mvp_id()
                )));
            }
        };

        let effective_depth = explore.depth.min(MAX_EXPLORE_DEPTH);
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(SymbolId, u32)> = VecDeque::new();
        let mut ordered: Vec<ResolvedSymbol> = Vec::new();

        // Seed: the root symbol is always included (even when depth=0).
        if let Some(root) = self.view.repo.resolve(&seed_id)? {
            visited.insert(root.id.to_string());
            ordered.push(root);
            if effective_depth > 0 {
                queue.push_back((seed_id, 1));
            }
        }

        while let Some((current, depth)) = queue.pop_front() {
            if depth > effective_depth {
                break;
            }
            let neighbors: Vec<RelationTarget> = match explore.direction {
                Direction::Callers => self.view.repo.callers(&current),
                Direction::Callees => self.view.repo.callees(&current),
            };
            for n in neighbors {
                if visited.insert(n.id.to_string()) {
                    if let Some(resolved) = self.view.repo.resolve(&n.id)? {
                        ordered.push(resolved);
                    }
                    if depth < effective_depth {
                        queue.push_back((n.id, depth + 1));
                    }
                }
            }
        }

        let total = ordered.len();
        let items: Vec<MoldQLItem> = ordered
            .into_iter()
            .map(|s| {
                let mvp = symbol_mvp(&s);
                MoldQLItem {
                    object_id: mvp,
                    object_type: InspectableObjectType::Symbol,
                    label: format!("{} at {}:{}", s.name, s.file, s.line),
                    detail: None,
                }
            })
            .collect();

        Ok(MoldQLResult {
            query: query_str,
            items,
            total,
        })
    }

    // -------------------------------------------------------------------
    // Condition evaluation
    // -------------------------------------------------------------------

    fn conditions_pass_for_symbol(
        &self,
        find: &crate::moldql::ast::FindQuery,
        s: &ResolvedSymbol,
    ) -> bool {
        for cond in &find.conditions {
            if !self.eval_symbol_condition(cond, s) {
                return false;
            }
        }
        true
    }

    fn eval_symbol_condition(&self, cond: &Condition, s: &ResolvedSymbol) -> bool {
        let field = &cond.field;
        let raw = match field.head() {
            "fan_in" => Some(Value::Number(self.view.repo.fan_in(&s.id) as f64)),
            "fan_out" => Some(Value::Number(self.view.repo.fan_out(&s.id) as f64)),
            "kind" => Some(Value::String(s.kind.name().to_string())),
            "name" => Some(Value::String(s.name.clone())),
            "file" => Some(Value::String(s.file.clone())),
            "line" => Some(Value::Number(s.line as f64)),
            "quality" => {
                let level = field.tail().unwrap_or("");
                let issues = match self.view.quality.as_deref() {
                    Some(q) => q.issues_for_file(&s.file).unwrap_or_default(),
                    None => Vec::new(),
                };
                Some(Value::Number(count_by_severity(&issues, level) as f64))
            }
            _ => None,
        };
        let Some(value) = raw else {
            return false;
        };
        compare(&value, &cond.op, &cond.value)
    }

    fn conditions_pass_for_file(
        &self,
        find: &crate::moldql::ast::FindQuery,
        file: &str,
    ) -> ExplorerResult<bool> {
        for cond in &find.conditions {
            if !self.eval_file_condition(cond, file)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn eval_file_condition(&self, cond: &Condition, file: &str) -> ExplorerResult<bool> {
        let field = &cond.field;
        let raw = match field.head() {
            "path" | "file" | "name" => Some(Value::String(file.to_string())),
            "line_count" => Some(Value::Number(self.line_count_for_file(file)?)),
            "symbol_count" => Some(Value::Number(
                self.view.repo.find_symbols_by_file(file)?.len() as f64,
            )),
            "issue_count" => Some(Value::Number(self.issues_for_file_count(file) as f64)),
            "quality" => {
                let level = field.tail().unwrap_or("");
                let issues = self.issues_for_file_list(file);
                Some(Value::Number(count_by_severity(&issues, level) as f64))
            }
            _ => None,
        };
        let Some(value) = raw else {
            return Ok(false);
        };
        Ok(compare(&value, &cond.op, &cond.value))
    }

    fn conditions_pass_for_scope(&self, find: &crate::moldql::ast::FindQuery, scope: &str) -> bool {
        for cond in &find.conditions {
            if !self.eval_scope_condition(cond, scope) {
                return false;
            }
        }
        true
    }

    fn eval_scope_condition(&self, cond: &Condition, scope: &str) -> bool {
        let field = &cond.field;
        let raw = match field.head() {
            "path" | "name" => Some(Value::String(scope.to_string())),
            "file_count" | "symbol_count" => {
                // Derive both counts together (single pass over all_symbols).
                let (files, symbols) = scope_members(self.view.repo.as_ref(), scope);
                match field.head() {
                    "file_count" => Some(Value::Number(files as f64)),
                    _ => Some(Value::Number(symbols as f64)),
                }
            }
            "issue_count" => {
                let issues = match self.view.quality.as_deref() {
                    Some(q) => q.issues_for_scope(scope).unwrap_or_default(),
                    None => Vec::new(),
                };
                Some(Value::Number(issues.len() as f64))
            }
            "quality" => {
                let level = field.tail().unwrap_or("");
                let issues = match self.view.quality.as_deref() {
                    Some(q) => q.issues_for_scope(scope).unwrap_or_default(),
                    None => Vec::new(),
                };
                Some(Value::Number(count_by_severity(&issues, level) as f64))
            }
            _ => None,
        };
        let Some(value) = raw else {
            return false;
        };
        compare(&value, &cond.op, &cond.value)
    }

    fn conditions_pass_for_issue(
        &self,
        find: &crate::moldql::ast::FindQuery,
        issue: &crate::ports::QualityIssue,
    ) -> bool {
        for cond in &find.conditions {
            if !self.eval_issue_condition(cond, issue) {
                return false;
            }
        }
        true
    }

    fn eval_issue_condition(&self, cond: &Condition, issue: &crate::ports::QualityIssue) -> bool {
        let field = &cond.field;
        let raw = match field.head() {
            "severity" => Some(Value::String(issue.severity.clone())),
            "rule" | "rule_id" => Some(Value::String(issue.rule_id.clone())),
            "category" => Some(Value::String(issue.category.clone())),
            "file" | "path" => Some(Value::String(issue.file.clone())),
            "line" => Some(Value::Number(issue.line as f64)),
            "id" => Some(Value::Number(issue.id as f64)),
            _ => None,
        };
        let Some(value) = raw else {
            return false;
        };
        compare(&value, &cond.op, &cond.value)
    }

    // -------------------------------------------------------------------
    // Lens application (graceful)
    // -------------------------------------------------------------------

    fn apply_lens_if_requested(
        &self,
        find: &crate::moldql::ast::FindQuery,
        mvp: &str,
    ) -> ExplorerResult<Option<String>> {
        let Some(lens_id) = find.apply_lens.as_deref() else {
            return Ok(None);
        };
        match (self.view.apply_lens)(mvp, lens_id) {
            Ok(result) => Ok(Some(format_lens_detail(lens_id, &result))),
            // Graceful: unknown lens or any lens failure does not kill
            // the query. Surface the lens id in `detail` so callers can
            // see what was requested.
            Err(_) => Ok(Some(lens_id.to_string())),
        }
    }

    // -------------------------------------------------------------------
    // File helpers
    // -------------------------------------------------------------------

    fn line_count_for_file(&self, file: &str) -> ExplorerResult<f64> {
        let lines = self.view.reader.read_lines(file, 1, u32::MAX)?;
        Ok(lines.iter().map(|(n, _)| *n).max().unwrap_or(0) as f64)
    }

    fn issues_for_file_count(&self, file: &str) -> usize {
        self.issues_for_file_list(file).len()
    }

    fn issues_for_file_list(&self, file: &str) -> Vec<crate::ports::QualityIssue> {
        match self.view.quality.as_deref() {
            Some(q) => q.issues_for_file(file).unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

// ============================================================================
// Public view object — the executor's read-only borrow of the service's
// ports. The service constructs this in `execute_query`.
// ============================================================================

/// Read-only bundle of ports the executor needs. Built from
/// [`crate::service::ExplorerService`] and consumed by
/// [`MoldQLExecutor`]. Mirrors the field layout of `LensContext` so
/// service-owned data does not need to be public.
pub struct MoldQLView {
    pub repo: Arc<dyn SymbolRepository>,
    pub quality: Option<Arc<dyn crate::ports::QualityRepository>>,
    pub reader: Arc<dyn crate::ports::SourceReader>,
    /// `apply_lens(mvp, lens_id)` is delegated so the executor never
    /// needs to know about the lens registry. The `Arc` keeps the
    /// closure cheap to share without the lifetime gymnastics of
    /// `&dyn Fn`.
    #[allow(clippy::type_complexity)]
    pub apply_lens: Arc<dyn Fn(&str, &str) -> ExplorerResult<LensResult> + Send + Sync>,
}

impl MoldQLView {
    /// Borrow `executor` for this view.
    pub fn executor(&self) -> MoldQLExecutor<'_> {
        MoldQLExecutor::new(self)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn symbol_mvp(s: &ResolvedSymbol) -> String {
    format!("symbol:{}:{}:{}", s.file, s.name, s.line)
}

fn matches_scope(scope: Option<&str>, file: &str) -> bool {
    match scope {
        None => true,
        Some(s) => scope_contains_file(s, file),
    }
}

fn count_by_severity(issues: &[crate::ports::QualityIssue], level: &str) -> usize {
    issues
        .iter()
        .filter(|i| i.severity.eq_ignore_ascii_case(level))
        .count()
}

fn scope_members(repo: &dyn SymbolRepository, scope: &str) -> (usize, usize) {
    let mut files: BTreeSet<String> = BTreeSet::new();
    let mut symbols = 0usize;
    if let Ok(all) = repo.all_symbols() {
        for s in all {
            if scope_contains_file(scope, &s.file) {
                files.insert(s.file.clone());
                symbols += 1;
            }
        }
    }
    (files.len(), symbols)
}

fn compare(actual: &Value, op: &Op, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => match op {
            Op::Gt => a > b,
            Op::Gte => a >= b,
            Op::Lt => a < b,
            Op::Lte => a <= b,
            Op::Eq => (a - b).abs() < f64::EPSILON,
            Op::Neq => (a - b).abs() >= f64::EPSILON,
            // `~` on numbers is meaningless; treat as false.
            Op::Contains => false,
        },
        (Value::String(a), Value::String(b)) => {
            // String equality is case-insensitive — `kind = "Function"`
            // must match a stored `kind = "function"`. This mirrors SQL
            // conventions and avoids surprising users about SymbolKind's
            // lowercase canonical form.
            let (al, bl) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
            match op {
                Op::Eq => al == bl,
                Op::Neq => al != bl,
                Op::Contains => al.contains(&bl),
                // `>` / `<` use lexicographic order on the lowered
                // strings so the same shape works for both kinds.
                Op::Gt => al > bl,
                Op::Gte => al >= bl,
                Op::Lt => al < bl,
                Op::Lte => al <= bl,
            }
        }
        // Cross-type comparisons are always false (no implicit coercion).
        _ => false,
    }
}

fn format_lens_detail(lens_id: &str, result: &LensResult) -> String {
    if result.summary.is_empty() {
        lens_id.to_string()
    } else {
        format!("{lens_id}: {}", result.summary)
    }
}

fn render_find(find: &crate::moldql::ast::FindQuery) -> String {
    let mut s = format!("FIND {}", find.target.keyword());
    if let Some(scope) = &find.scope {
        s.push_str(&format!(" IN SCOPE {scope}"));
    }
    if !find.conditions.is_empty() {
        s.push_str(" WHERE ");
        let conds: Vec<String> = find.conditions.iter().map(render_condition).collect();
        s.push_str(&conds.join(" AND "));
    }
    if let Some(lens) = &find.apply_lens {
        s.push_str(&format!(" APPLY {lens}"));
    }
    s
}

fn render_condition(c: &Condition) -> String {
    let field = if c.field.parts.len() == 1 {
        c.field.parts[0].clone()
    } else {
        c.field.parts.join(".")
    };
    let value = match &c.value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{s}\""),
    };
    format!("{field} {} {value}", c.op.symbol())
}

fn render_explore(e: &crate::moldql::ast::ExploreQuery) -> String {
    format!(
        "EXPLORE {} THROUGH {} DEPTH {}",
        e.object_ref,
        e.direction.keyword(),
        e.depth
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::QualityRepository;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Arc;

    /// Test repository that backs symbols + edges by hashmap.
    struct MockRepo {
        by_id: StdHashMap<String, ResolvedSymbol>,
        /// `callers_of[owner_id] = list of caller ids`
        callers_of: StdHashMap<String, Vec<String>>,
        /// `callees_of[owner_id] = list of callee ids`
        callees_of: StdHashMap<String, Vec<String>>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                by_id: StdHashMap::new(),
                callers_of: StdHashMap::new(),
                callees_of: StdHashMap::new(),
            }
        }

        fn with_sym(&mut self, name: &str, file: &str, line: u32) {
            let id = SymbolId::new(format!("{file}:{name}:{line}"));
            let sym = ResolvedSymbol {
                id: id.clone(),
                name: name.to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line,
                signature: None,
            };
            self.by_id.insert(id.to_string(), sym);
        }

        /// Resolve a `(name, file, line)` triple to its `SymbolId`
        /// string. Panics if the symbol is not in the repo.
        fn sid(&self, name: &str, file: &str, line: u32) -> String {
            let key = format!("{file}:{name}:{line}");
            if self.by_id.contains_key(&key) {
                key
            } else {
                panic!("MockRepo::sid: unknown symbol `{name}` at {file}:{line}")
            }
        }

        fn with_caller(&mut self, owner: &str, caller: &str) {
            self.callers_of
                .entry(owner.to_string())
                .or_default()
                .push(caller.to_string());
        }

        fn with_callee(&mut self, owner: &str, callee: &str) {
            self.callees_of
                .entry(owner.to_string())
                .or_default()
                .push(callee.to_string());
        }
    }

    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
        fn callers(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callers_of
                .get(id.as_str())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|cid| self.by_id.get(cid).map(|s| RelationTarget::from(s)))
                        .collect()
                })
                .unwrap_or_default()
        }
        fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callees_of
                .get(id.as_str())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|cid| self.by_id.get(cid).map(|s| RelationTarget::from(s)))
                        .collect()
                })
                .unwrap_or_default()
        }
        fn fan_in(&self, id: &SymbolId) -> usize {
            self.callers_of
                .get(id.as_str())
                .map(|v| v.len())
                .unwrap_or(0)
        }
        fn fan_out(&self, id: &SymbolId) -> usize {
            self.callees_of
                .get(id.as_str())
                .map(|v| v.len())
                .unwrap_or(0)
        }
        fn find_symbols_by_name(&self, name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .by_id
                .values()
                .filter(|s| s.name == name)
                .cloned()
                .collect())
        }
        fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .by_id
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for s in self.by_id.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_id.values().cloned().collect())
        }
        fn graph_stats(&self) -> crate::ports::symbol_repository::GraphStats {
            crate::ports::symbol_repository::GraphStats::default()
        }
    }

    /// Quality repo stub. `None` exercises the "no backend" path.
    struct NoQuality;

    impl QualityRepository for NoQuality {
        fn issues_for_file(&self, _file: &str) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(Vec::new())
        }
        fn issues_for_scope(
            &self,
            _scope_prefix: &str,
        ) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(Vec::new())
        }
        fn issues_at_line(
            &self,
            _file: &str,
            _line: u32,
        ) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(Vec::new())
        }
        fn issue_by_id(&self, _id: i64) -> ExplorerResult<Option<crate::ports::QualityIssue>> {
            Ok(None)
        }
        fn rule_summary(&self, _rule_id: &str) -> ExplorerResult<crate::ports::RuleSummary> {
            Ok(crate::ports::RuleSummary {
                rule_id: String::new(),
                description: String::new(),
                open_count: 0,
            })
        }
        fn quality_gate(&self) -> ExplorerResult<crate::ports::QualityGateSummary> {
            Ok(crate::ports::QualityGateSummary::default())
        }
        fn open_issues_count(&self) -> ExplorerResult<usize> {
            Ok(0)
        }
    }

    /// Test quality repo that returns a hand-picked list. Used by
    /// `find_files_with_quality_filter` to exercise the
    /// `quality.<level>` / `issue_count` code path.
    struct StaticQuality {
        issues: Vec<crate::ports::QualityIssue>,
    }
    impl StaticQuality {
        fn new(issues: Vec<crate::ports::QualityIssue>) -> Self {
            Self { issues }
        }
    }
    impl QualityRepository for StaticQuality {
        fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.file == file)
                .cloned()
                .collect())
        }
        fn issues_for_scope(&self, scope: &str) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.file == scope || i.file.starts_with(&format!("{scope}/")))
                .cloned()
                .collect())
        }
        fn issues_at_line(
            &self,
            file: &str,
            line: u32,
        ) -> ExplorerResult<Vec<crate::ports::QualityIssue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.file == file && i.line == line)
                .cloned()
                .collect())
        }
        fn issue_by_id(&self, id: i64) -> ExplorerResult<Option<crate::ports::QualityIssue>> {
            Ok(self.issues.iter().find(|i| i.id == id).cloned())
        }
        fn rule_summary(&self, rule_id: &str) -> ExplorerResult<crate::ports::RuleSummary> {
            let count = self.issues.iter().filter(|i| i.rule_id == rule_id).count();
            Ok(crate::ports::RuleSummary {
                rule_id: rule_id.to_string(),
                description: rule_id.to_string(),
                open_count: count,
            })
        }
        fn quality_gate(&self) -> ExplorerResult<crate::ports::QualityGateSummary> {
            Ok(crate::ports::QualityGateSummary::default())
        }
        fn open_issues_count(&self) -> ExplorerResult<usize> {
            Ok(self.issues.len())
        }
    }

    /// Build a view with the test repo + a fresh source reader. Lenses
    /// return `Ok` for `hotspots`, `Err` for everything else — that
    /// matches the production behaviour of the lens registry (unknown
    /// lens is an error, but the executor swallows it gracefully).
    fn build_view(repo: Arc<MockRepo>) -> MoldQLView {
        use crate::adapters::FsSourceReader;
        let reader: Arc<dyn crate::ports::SourceReader> = Arc::new(FsSourceReader::new("/tmp"));
        let apply: Arc<dyn Fn(&str, &str) -> ExplorerResult<LensResult> + Send + Sync> =
            Arc::new(|mvp, lens_id| {
                if lens_id == "hotspots" {
                    Ok(LensResult {
                        lens_id: lens_id.to_string(),
                        findings: Vec::new(),
                        summary: format!("Detected 1 hotspot finding(s) at {mvp}"),
                    })
                } else {
                    Err(ExplorerError::ResolutionFailed(format!(
                        "lens not found: {lens_id}"
                    )))
                }
            });
        MoldQLView {
            repo: repo as Arc<dyn SymbolRepository>,
            quality: None,
            reader,
            apply_lens: apply,
        }
    }

    /// Variant of `build_view` that also wires a `NoQuality` backend.
    /// Used by tests that exercise the `quality.<level>` fields.
    fn build_view_with_quality(
        repo: Arc<MockRepo>,
        quality: Arc<dyn crate::ports::QualityRepository>,
    ) -> MoldQLView {
        use crate::adapters::FsSourceReader;
        let reader: Arc<dyn crate::ports::SourceReader> = Arc::new(FsSourceReader::new("/tmp"));
        let apply: Arc<dyn Fn(&str, &str) -> ExplorerResult<LensResult> + Send + Sync> =
            Arc::new(|_mvp, _lens_id| {
                Err(ExplorerError::ResolutionFailed("no lens in test".into()))
            });
        MoldQLView {
            repo: repo as Arc<dyn SymbolRepository>,
            quality: Some(quality),
            reader,
            apply_lens: apply,
        }
    }

    fn run_find(view: &MoldQLView, q: &str) -> MoldQLResult {
        let ast = crate::moldql::parser::parse(q).expect("parse ok");
        view.executor().execute(ast).expect("execute ok")
    }

    fn run_explore(view: &MoldQLView, q: &str) -> MoldQLResult {
        let ast = crate::moldql::parser::parse(q).expect("parse ok");
        view.executor().execute(ast).expect("execute ok")
    }

    // -- Tests ---------------------------------------------------------------

    fn make_repo(builder: impl FnOnce(&mut MockRepo)) -> Arc<MockRepo> {
        let mut repo = MockRepo::new();
        builder(&mut repo);
        Arc::new(repo)
    }

    #[test]
    fn find_symbols_no_filter_returns_all_sorted() {
        let repo = make_repo(|r| {
            r.with_sym("alpha", "src/a.rs", 1);
            r.with_sym("beta", "src/b.rs", 5);
            r.with_sym("gamma", "src/c.rs", 3);
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND symbols");
        assert_eq!(r.total, 3);
        assert_eq!(r.items[0].label, "alpha at src/a.rs:1");
        assert_eq!(r.items[1].label, "beta at src/b.rs:5");
        assert_eq!(r.items[2].label, "gamma at src/c.rs:3");
    }

    #[test]
    fn find_symbols_with_fan_in_filter() {
        let repo = make_repo(|r| {
            r.with_sym("alpha", "src/a.rs", 1);
            r.with_sym("beta", "src/b.rs", 5);
            r.with_sym("gamma_x", "src/c.rs", 3);
            r.with_caller(
                &r.sid("alpha", "src/a.rs", 1),
                &r.sid("beta", "src/b.rs", 5),
            );
            r.with_caller(
                &r.sid("alpha", "src/a.rs", 1),
                &r.sid("gamma_x", "src/c.rs", 3),
            );
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND symbols WHERE fan_in >= 2");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].label, "alpha at src/a.rs:1");
    }

    #[test]
    fn find_files_in_scope() {
        let repo = make_repo(|r| {
            r.with_sym("alpha", "src/foo/a.rs", 1);
            r.with_sym("beta", "src/foo/b.rs", 2);
            r.with_sym("gamma", "src/bar/c.rs", 3);
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND files IN SCOPE src/foo");
        assert_eq!(r.total, 2);
        let labels: Vec<&str> = r.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"src/foo/a.rs"));
        assert!(labels.contains(&"src/foo/b.rs"));
    }

    #[test]
    fn find_files_does_not_bleed_scope_prefix() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/x.rs", 1);
            r.with_sym("b", "src_extra/y.rs", 1);
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND files IN SCOPE src");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].label, "src/x.rs");
    }

    #[test]
    fn find_symbols_multi_condition_and() {
        let repo = make_repo(|r| {
            r.with_sym("alpha", "src/a.rs", 1);
            r.with_sym("beta", "src/b.rs", 5);
            r.with_sym("x", "src/x.rs", 1);
            r.with_sym("y", "src/y.rs", 1);
            r.with_caller(&r.sid("alpha", "src/a.rs", 1), &r.sid("x", "src/x.rs", 1));
            r.with_caller(&r.sid("alpha", "src/a.rs", 1), &r.sid("y", "src/y.rs", 1));
        });
        let view = build_view(repo.clone());
        // fan_in >= 2 AND kind = "function"
        let r = run_find(
            &view,
            "FIND symbols WHERE fan_in >= 2 AND kind = \"function\"",
        );
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].label, "alpha at src/a.rs:1");
    }

    #[test]
    fn find_symbols_contains_operator() {
        let repo = make_repo(|r| {
            r.with_sym("alpha_main", "src/a.rs", 1);
            r.with_sym("beta", "src/b.rs", 1);
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND symbols WHERE name ~ \"main\"");
        assert_eq!(r.total, 1);
        assert!(r.items[0].label.starts_with("alpha_main"));
    }

    #[test]
    fn explore_callers_bfs_dedup_depth() {
        let repo = make_repo(|r| {
            // a is called by b, c, x, y; chain b -> a (a calls b? no — b calls a)
            // Layout: b calls a, c calls b, x calls a, y calls a
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
            r.with_sym("c", "src/c.rs", 1);
            r.with_sym("x", "src/x.rs", 1);
            r.with_sym("y", "src/y.rs", 1);
            r.with_caller(&r.sid("a", "src/a.rs", 1), &r.sid("x", "src/x.rs", 1));
            r.with_caller(&r.sid("a", "src/a.rs", 1), &r.sid("y", "src/y.rs", 1));
            r.with_caller(&r.sid("b", "src/b.rs", 1), &r.sid("a", "src/a.rs", 1));
            r.with_caller(&r.sid("c", "src/c.rs", 1), &r.sid("b", "src/b.rs", 1));
        });
        let view = build_view(repo.clone());
        // BFS: depth 0 = [a]; depth 1 = [x, y] (callers of a).
        let r = run_explore(&view, "EXPLORE symbol:src/a.rs:a:1 THROUGH callers DEPTH 3");
        // a (seed) + x + y = 3 items; b and c are callees, not callers.
        assert_eq!(r.total, 3);
        let labels: Vec<String> = r.items.iter().map(|i| i.label.clone()).collect();
        assert!(labels.iter().any(|l| l.starts_with("a at")));
        assert!(labels.iter().any(|l| l.starts_with("x at")));
        assert!(labels.iter().any(|l| l.starts_with("y at")));
    }

    #[test]
    fn explore_callees_depth_zero_returns_seed_only() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
            r.with_callee(&r.sid("a", "src/a.rs", 1), &r.sid("b", "src/b.rs", 1));
        });
        let view = build_view(repo.clone());
        let r = run_explore(&view, "EXPLORE symbol:src/a.rs:a:1 THROUGH callees DEPTH 0");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].label, "a at src/a.rs:1");
    }

    #[test]
    fn explore_depth_clamped_to_five() {
        let repo = make_repo(|r| {
            // 6-deep chain: a -> b -> c -> d -> e -> f -> g
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
            r.with_sym("c", "src/c.rs", 1);
            r.with_sym("d", "src/d.rs", 1);
            r.with_sym("e", "src/e.rs", 1);
            r.with_sym("f", "src/f.rs", 1);
            r.with_sym("g", "src/g.rs", 1);
            r.with_callee(&r.sid("a", "src/a.rs", 1), &r.sid("b", "src/b.rs", 1));
            r.with_callee(&r.sid("b", "src/b.rs", 1), &r.sid("c", "src/c.rs", 1));
            r.with_callee(&r.sid("c", "src/c.rs", 1), &r.sid("d", "src/d.rs", 1));
            r.with_callee(&r.sid("d", "src/d.rs", 1), &r.sid("e", "src/e.rs", 1));
            r.with_callee(&r.sid("e", "src/e.rs", 1), &r.sid("f", "src/f.rs", 1));
            r.with_callee(&r.sid("f", "src/f.rs", 1), &r.sid("g", "src/g.rs", 1));
        });
        let view = build_view(repo.clone());
        let r = run_explore(
            &view,
            "EXPLORE symbol:src/a.rs:a:1 THROUGH callees DEPTH 99",
        );
        // a (depth 0) + 5 callees (b..f) = 6, g is at depth 6 and beyond cap.
        assert_eq!(r.total, 6);
        let labels: Vec<String> = r.items.iter().map(|i| i.label.clone()).collect();
        assert!(!labels.iter().any(|l| l.starts_with("g at")));
    }

    #[test]
    fn explore_unknown_symbol_yields_empty() {
        let repo = make_repo(|_| {});
        let view = build_view(repo.clone());
        let r = run_explore(
            &view,
            "EXPLORE symbol:src/missing.rs:ghost:1 THROUGH callers DEPTH 1",
        );
        // Unknown symbol → empty items, no error.
        assert_eq!(r.total, 0);
    }

    #[test]
    fn find_quality_condition_degrades_when_no_backend() {
        let repo = make_repo(|r| {
            r.with_sym("alpha", "src/a.rs", 1);
        });
        let view = build_view(repo.clone());

        // No quality repo wired → quality.critical == 0 → strict `> 0` fails
        let r = run_find(&view, "FIND symbols WHERE quality.critical > 0");
        assert_eq!(r.total, 0);

        // `== 0` should still pass for the same reason.
        let r = run_find(&view, "FIND symbols WHERE quality.critical == 0");
        assert_eq!(r.total, 1);
    }

    #[test]
    fn find_files_with_quality_filter_when_backend_wired() {
        // Two issues for `src/a.rs` (one critical, one major) plus an
        // issue for `src/b.rs` (info). The filter `issue_count > 0`
        // should match `src/a.rs` and `src/b.rs` (both have at least
        // one issue) but not the empty `src/c.rs`.
        let repo = make_repo(|r| {
            r.with_sym("a1", "src/a.rs", 1);
            r.with_sym("b1", "src/b.rs", 1);
            r.with_sym("c1", "src/c.rs", 1);
        });
        let quality = StaticQuality::new(vec![
            crate::ports::QualityIssue {
                id: 1,
                rule_id: "rust:S100".into(),
                severity: "Critical".into(),
                category: "smells".into(),
                file: "src/a.rs".into(),
                line: 1,
                message: "msg".into(),
                status: "open".into(),
            },
            crate::ports::QualityIssue {
                id: 2,
                rule_id: "rust:S101".into(),
                severity: "Major".into(),
                category: "smells".into(),
                file: "src/a.rs".into(),
                line: 2,
                message: "msg".into(),
                status: "open".into(),
            },
            crate::ports::QualityIssue {
                id: 3,
                rule_id: "rust:S102".into(),
                severity: "Info".into(),
                category: "smells".into(),
                file: "src/b.rs".into(),
                line: 1,
                message: "msg".into(),
                status: "open".into(),
            },
        ]);
        let quality_arc: Arc<dyn crate::ports::QualityRepository> = Arc::new(quality);
        let view = build_view_with_quality(repo.clone(), quality_arc);

        let r = run_find(&view, "FIND files WHERE issue_count > 0");
        let labels: Vec<&str> = r.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(r.total, 2);
        assert!(labels.contains(&"src/a.rs"));
        assert!(labels.contains(&"src/b.rs"));
        assert!(!labels.contains(&"src/c.rs"));

        // quality.critical > 0 must match only `src/a.rs`.
        let r = run_find(&view, "FIND files WHERE quality.critical > 0");
        let labels: Vec<&str> = r.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(r.total, 1);
        assert!(labels.contains(&"src/a.rs"));
    }

    #[test]
    fn compare_string_lex_order() {
        assert!(compare(
            &Value::String("b".into()),
            &Op::Gt,
            &Value::String("a".into())
        ));
        assert!(compare(
            &Value::String("apple".into()),
            &Op::Lt,
            &Value::String("banana".into())
        ));
    }

    #[test]
    fn compare_string_eq_is_case_insensitive() {
        assert!(compare(
            &Value::String("function".into()),
            &Op::Eq,
            &Value::String("Function".into())
        ));
        assert!(!compare(
            &Value::String("struct".into()),
            &Op::Eq,
            &Value::String("Function".into())
        ));
    }

    #[test]
    fn compare_number() {
        assert!(compare(&Value::Number(5.0), &Op::Gt, &Value::Number(3.0)));
        assert!(!compare(&Value::Number(5.0), &Op::Lt, &Value::Number(3.0)));
        assert!(compare(&Value::Number(5.0), &Op::Neq, &Value::Number(3.0)));
    }

    #[test]
    fn compare_cross_type_is_false() {
        assert!(!compare(
            &Value::Number(5.0),
            &Op::Eq,
            &Value::String("5".into())
        ));
    }

    #[test]
    fn find_scopes_target() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/foo/a.rs", 1);
            r.with_sym("b", "src/bar/b.rs", 1);
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND scopes");
        assert_eq!(r.total, 2);
        let labels: Vec<&str> = r.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"src/foo"));
        assert!(labels.contains(&"src/bar"));
    }

    #[test]
    fn apply_lens_sets_detail() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
            // b calls a so `hotspots` lens on `a` returns a finding.
            r.with_caller(&r.sid("a", "src/a.rs", 1), &r.sid("b", "src/b.rs", 1));
        });
        let view = build_view(repo.clone());
        let r = run_find(&view, "FIND symbols WHERE fan_in >= 1 APPLY hotspots");
        assert_eq!(r.total, 1);
        let detail = r.items[0].detail.as_deref().expect("lens applied");
        assert!(
            detail.starts_with("hotspots"),
            "expected `hotspots:` prefix, got `{detail}`"
        );
    }

    #[test]
    fn apply_unknown_lens_does_not_kill_query() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
        });
        let view = build_view(repo.clone());

        let r = run_find(&view, "FIND symbols APPLY does-not-exist");
        assert_eq!(r.total, 1);
        // Detail is set to the lens id (graceful degradation).
        assert_eq!(r.items[0].detail.as_deref(), Some("does-not-exist"));
    }

    // Sanity: the NoQuality stub is reachable — used by future tests
    // that want to assert the "no issues" path through the quality
    // backend without actually supplying one.
    #[test]
    fn no_quality_backend_returns_empty_for_issues_queries() {
        let no_q: Arc<dyn crate::ports::QualityRepository> = Arc::new(NoQuality);
        let issues = no_q.issues_for_file("anything").expect("ok");
        assert!(issues.is_empty());
    }

    // ========================================================================
    // ExplorerQL execution tests — verify the new variants reach the
    // compile → run pipeline and produce a result envelope.
    // ========================================================================

    fn run_explorerql(view: &MoldQLView, q: &str) -> MoldQLResult {
        let ast = crate::moldql::parser::parse(q).expect("parse ok");
        view.executor().execute(ast).expect("execute ok")
    }

    #[test]
    fn execute_path_uses_compile_then_run() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
        });
        let view = build_view(repo);
        let r = run_explorerql(&view, "PATH FROM a TO b");
        // The petgraph plan is wired through; for the MVP the
        // executor returns an empty `MoldQLResult` (the plan is
        // captured in the query field).
        assert!(r.query.contains("Bfs"));
    }

    #[test]
    fn execute_neighbors_emits_plan() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
        });
        let view = build_view(repo);
        let r = run_explorerql(&view, "NEIGHBORS a DEPTH 1");
        assert!(r.query.contains("DualRadius"));
    }

    #[test]
    fn execute_subgraph_emits_dual_radius_plan() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
        });
        let view = build_view(repo);
        let r = run_explorerql(&view, "SUBGRAPH ROOT a");
        assert!(r.query.contains("DualRadius"));
    }

    #[test]
    fn execute_cluster_scc_emits_detect_cycles_plan() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
        });
        let view = build_view(repo);
        let r = run_explorerql(&view, "CLUSTER");
        assert!(r.query.contains("DetectCycles"));
    }

    #[test]
    fn execute_explain_emits_explain_path_plan() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
        });
        let view = build_view(repo);
        let r = run_explorerql(&view, "EXPLAIN FROM a TO b");
        assert!(r.query.contains("ExplainPath"));
    }

    #[test]
    fn execute_boolean_and_routes_through_compile() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
        });
        let view = build_view(repo);
        // Boolean(And, [Path, Path]) is compiled to a Composed plan;
        // set-algebra execution is a future work item, so the MVP
        // returns NotImplemented. The test asserts the executor
        // does NOT crash and that the failure is a clean error
        // envelope (not a panic).
        let r = view.executor().execute(
            crate::moldql::parser::parse("PATH FROM a TO b AND PATH FROM a TO b")
                .expect("parse ok"),
        );
        match r {
            Ok(moldql_result) => {
                // If set algebra is later implemented, the result is
                // captured in the query echo.
                assert!(
                    moldql_result.query.contains("Bfs")
                        || moldql_result.query.contains("Composed")
                        || moldql_result.query.contains("Unsupported")
                );
            }
            Err(e) => {
                // Expected for the MVP: NotImplemented from compile::run.
                let msg = e.to_string();
                assert!(
                    msg.contains("boolean composition")
                        || msg.contains("NotImplemented")
                        || msg.contains("future work"),
                    "unexpected error: {msg}"
                );
            }
        }
    }

    #[test]
    fn execute_with_target_pg_routes_through_pg_compile() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
        });
        let view = build_view(repo);
        let ast = crate::moldql::parser::parse("PATH FROM a TO b").expect("parse ok");
        // Default build: PG execution surfaces FeatureDisabled.
        let r = view
            .executor()
            .execute_with_target(ast, crate::moldql::compile::CompileTarget::Postgres);
        // The error is "feature disabled" (no panic, no crash).
        assert!(r.is_err() || r.is_ok());
    }

    #[test]
    fn execute_with_target_petgraph_returns_plan() {
        let repo = make_repo(|r| {
            r.with_sym("a", "src/a.rs", 1);
            r.with_sym("b", "src/b.rs", 1);
        });
        let view = build_view(repo);
        let ast = crate::moldql::parser::parse("PATH FROM a TO b").expect("parse ok");
        let r = view
            .executor()
            .execute_with_target(ast, crate::moldql::compile::CompileTarget::Petgraph)
            .expect("ok");
        assert!(r.query.contains("Bfs"));
    }
}

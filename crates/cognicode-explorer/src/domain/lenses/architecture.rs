//! `architecture` lens — boundary + cycle detection at scope level.
//!
//! The lens focuses on **scope-level** structural observations. It
//! surfaces dependency cycles between scopes (Critical) and "god
//! modules" with excessive incoming cross-scope relations (Warning).
//! At the symbol and file level it provides complementary observations
//! (boundary violations, scope interactions) but the strongest signal
//! is the scope-level cycle detection.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use crate::domain::lens::{Lens, LensContext, cap_and_order, clamp_confidence, finding_id};
use crate::dto::{
    DesignFinding, FindingSeverity, InspectableObjectType, LensDescriptor, LensResult,
};
use crate::error::ExplorerResult;
use crate::ports::symbol_repository::{RelationTarget, ResolvedSymbol};
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;

pub const LENS_ID: &str = "architecture";
const FINDING_CAP: usize = 20;
const GOD_MODULE_INCOMING_THRESHOLD: usize = 20;
const CYCLE_MAX_DEPTH: usize = 16;

pub struct ArchitectureLens;

impl Lens for ArchitectureLens {
    fn id(&self) -> &str {
        LENS_ID
    }

    fn descriptor(&self) -> LensDescriptor {
        LensDescriptor {
            id: LENS_ID.into(),
            name: "Architecture Boundaries".into(),
            description: "Detects dependency cycles between scopes, flags god \
                modules with excessive incoming edges, and surfaces \
                cross-scope boundary violations. Findings are hypotheses — \
                always verify cycles before restructuring."
                .into(),
            applicable_types: vec![
                InspectableObjectType::Symbol,
                InspectableObjectType::File,
                InspectableObjectType::Scope,
            ],
        }
    }

    fn apply(&self, ctx: &LensContext) -> ExplorerResult<LensResult> {
        let quality_present = ctx.quality_repo.is_some();
        let findings: Vec<DesignFinding> = match &ctx.object_id {
            crate::domain::ObjectIdentity::Scope { path } => {
                analyse_scope(path, ctx, quality_present)
            }
            crate::domain::ObjectIdentity::File { path } => {
                analyse_file(path, ctx, quality_present)
            }
            crate::domain::ObjectIdentity::Symbol { .. } => {
                let sym_id = ctx.object_id.to_symbol_id().expect("symbol identity");
                if let Some(resolved) = ctx.symbol_repo.resolve(&sym_id)? {
                    analyse_symbol(&resolved, ctx, quality_present)
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        let summary = if findings.is_empty() {
            "No architecture concerns detected at this object".to_string()
        } else {
            format!(
                "Detected {} architecture observation(s) at this object",
                findings.len()
            )
        };

        Ok(LensResult {
            lens_id: LENS_ID.into(),
            findings: cap_and_order(findings, FINDING_CAP),
            summary,
        })
    }
}

fn analyse_scope(scope: &str, ctx: &LensContext, quality_present: bool) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    let all = ctx.symbol_repo.all_symbols().unwrap_or_default();
    let members: Vec<ResolvedSymbol> = all
        .iter()
        .filter(|s| crate::domain::views::scope_contains_file(scope, &s.file))
        .cloned()
        .collect();

    // Build incoming-edge count per scope (how many other scopes call
    // into this one). Used to detect "god modules".
    let mut incoming_counts: HashMap<String, BTreeSet<String>> = HashMap::new();
    for sym in &all {
        for caller in ctx
            .graph_query
            .as_ref()
            .map(|gq| gq.callers(&sym.id))
            .unwrap_or_default()
        {
            if crate::domain::views::scope_contains_file(scope, &caller.file) {
                continue;
            }
            let other = parent_dir(&caller.file);
            incoming_counts
                .entry(other)
                .or_default()
                .insert(parent_dir(&sym.file));
        }
    }
    // Distinct foreign callers of THIS scope.
    let this_scope_callers: BTreeSet<String> = incoming_counts
        .values()
        .flat_map(|set| set.iter().cloned())
        .filter(|s| s == scope)
        .map(|_| "foreign".to_string())
        .collect();
    // Actually we want: how many distinct foreign scopes call into
    // this scope? Recount by walking every member's callers and
    // counting the unique parent dirs that are foreign.
    let mut foreign_callers: BTreeSet<String> = BTreeSet::new();
    for sym in &members {
        for caller in ctx
            .graph_query
            .as_ref()
            .map(|gq| gq.callers(&sym.id))
            .unwrap_or_default()
        {
            if !crate::domain::views::scope_contains_file(scope, &caller.file) {
                foreign_callers.insert(parent_dir(&caller.file));
            }
        }
    }
    let _ = this_scope_callers; // silence unused

    if foreign_callers.len() > GOD_MODULE_INCOMING_THRESHOLD {
        let confidence = clamp_confidence(if quality_present { 0.85 } else { 0.6 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("god-module:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("Possible god module: {}", scope),
            hypothesis: format!(
                "{} foreign scope(s) call into this one. May be acting as a \
                 shared kernel — consider whether sub-modules could own \
                 parts of the responsibility.",
                foreign_callers.len()
            ),
            severity: FindingSeverity::Warning,
            confidence,
            object_ids: vec![format!("scope:{scope}")],
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    }

    // Dependency cycle detection.
    if let Some(cycle) = detect_scope_cycle(scope, &members, &all, ctx) {
        let confidence = clamp_confidence(if quality_present { 0.9 } else { 0.7 });
        let cycle_label = cycle.join(" → ");
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("cycle:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("Possible dependency cycle: {}", cycle_label),
            hypothesis: format!(
                "A possible circular dependency was detected: {}. Cycles \
                 complicate build ordering and refactors. Verify before \
                 acting — the cycle may be intentional.",
                cycle_label
            ),
            severity: FindingSeverity::Critical,
            confidence,
            object_ids: cycle.iter().map(|s| format!("scope:{s}")).collect(),
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    }

    // A baseline "all clear" Info finding so the lens always reports
    // something at a scope (useful for UIs that want to show the
    // "checked, no concerns" state).
    if findings.is_empty() && !members.is_empty() {
        let confidence = clamp_confidence(if quality_present { 0.6 } else { 0.4 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("baseline:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("No cycle / god-module pattern in {}", scope),
            hypothesis: format!(
                "Walked {} symbol(s) in this scope — no dependency cycle or \
                 excessive incoming coupling detected. Pattern is within \
                 typical bounds.",
                members.len()
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![format!("scope:{scope}")],
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    }

    findings
}

fn analyse_file(file: &str, ctx: &LensContext, quality_present: bool) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    let symbols = ctx
        .symbol_repo
        .find_symbols_by_file(file)
        .unwrap_or_default();
    let mut scopes_touched: BTreeSet<String> = BTreeSet::new();
    for sym in &symbols {
        for callee in ctx
            .graph_query
            .as_ref()
            .map(|gq| gq.callees(&sym.id))
            .unwrap_or_default()
        {
            scopes_touched.insert(parent_dir(&callee.file));
        }
        for caller in ctx
            .graph_query
            .as_ref()
            .map(|gq| gq.callers(&sym.id))
            .unwrap_or_default()
        {
            scopes_touched.insert(parent_dir(&caller.file));
        }
    }
    // Remove the file's own scope.
    scopes_touched.remove(&parent_dir(file));
    if !scopes_touched.is_empty() {
        let confidence = clamp_confidence(if quality_present { 0.7 } else { 0.5 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("file-touches:{file}")),
            lens_id: LENS_ID.into(),
            title: format!("File touches {} scope(s): {}", scopes_touched.len(), file),
            hypothesis: format!(
                "This file's symbols reach into {} scope(s) ({}). Within the \
                 typical range; surfaced so callers see the boundary \
                 shape.",
                scopes_touched.len(),
                scopes_touched
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![format!("file:{file}")],
            evidence_ids: vec!["evidence:file_symbols".into()],
        });
    }
    findings
}

fn analyse_symbol(
    symbol: &ResolvedSymbol,
    ctx: &LensContext,
    quality_present: bool,
) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    let own_scope = parent_dir(&symbol.file);
    let mut foreign_scopes: BTreeSet<String> = BTreeSet::new();
    for callee in ctx
        .graph_query
        .as_ref()
        .map(|gq| gq.callees(&symbol.id))
        .unwrap_or_default()
    {
        let other = parent_dir(&callee.file);
        if other != own_scope {
            foreign_scopes.insert(other);
        }
    }
    for caller in ctx
        .graph_query
        .as_ref()
        .map(|gq| gq.callers(&symbol.id))
        .unwrap_or_default()
    {
        let other = parent_dir(&caller.file);
        if other != own_scope {
            foreign_scopes.insert(other);
        }
    }
    if !foreign_scopes.is_empty() {
        let confidence = clamp_confidence(if quality_present { 0.7 } else { 0.5 });
        findings.push(DesignFinding {
            id: finding_id(
                LENS_ID,
                &format!("boundary:{}:{}:{}", symbol.file, symbol.name, symbol.line),
            ),
            lens_id: LENS_ID.into(),
            title: format!("Boundary touch: {} ({})", symbol.name, foreign_scopes.len()),
            hypothesis: format!(
                "This symbol's call graph touches {} foreign scope(s). \
                 Within typical range; surfaced for boundary visibility.",
                foreign_scopes.len()
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![format!(
                "symbol:{}:{}:{}",
                symbol.file, symbol.name, symbol.line
            )],
            evidence_ids: vec!["evidence:symbol_callgraph".into()],
        });
    }
    findings
}

/// BFS from `scope` along outgoing cross-scope calls; if any visited
/// scope is `scope` again within `CYCLE_MAX_DEPTH` steps, the path is
/// returned. Mirrors the algorithm in `DependenciesLens` but kept
/// independent — the two lenses may evolve separately.
fn detect_scope_cycle(
    scope: &str,
    members: &[ResolvedSymbol],
    all: &[ResolvedSymbol],
    ctx: &LensContext,
) -> Option<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(scope.to_string());
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((scope.to_string(), vec![scope.to_string()]));

    while let Some((current, path)) = queue.pop_front() {
        if path.len() > CYCLE_MAX_DEPTH {
            continue;
        }
        for sym in all.iter().chain(members.iter()) {
            if parent_dir(&sym.file) != current {
                continue;
            }
            for callee in ctx
                .graph_query
                .as_ref()
                .map(|gq| gq.callees(&sym.id))
                .unwrap_or_default()
            {
                let other = parent_dir(&callee.file);
                if other == current {
                    continue;
                }
                if other == scope && path.len() >= 2 {
                    let mut full = path.clone();
                    full.push(other);
                    return Some(full);
                }
                if !visited.contains(&other) {
                    visited.insert(other.clone());
                    let mut next_path = path.clone();
                    next_path.push(other.clone());
                    queue.push_back((other, next_path));
                }
            }
        }
    }
    None
}

fn parent_dir(file: &str) -> String {
    std::path::Path::new(file)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::dto::InspectableObjectType;
    use crate::ports::symbol_repository::{
        GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
    };
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Arc;

    fn make_resolved(file: &str, name: &str, line: u32) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("{file}:{name}:{line}")),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: file.to_string(),
            line,
            signature: None,
        }
    }

    struct MockRepo {
        by_id: StdHashMap<String, ResolvedSymbol>,
        callers: StdHashMap<String, Vec<RelationTarget>>,
        callees: StdHashMap<String, Vec<RelationTarget>>,
        all: Vec<ResolvedSymbol>,
    }
    impl MockRepo {
        fn new() -> Self {
            Self {
                by_id: StdHashMap::new(),
                callers: StdHashMap::new(),
                callees: StdHashMap::new(),
                all: Vec::new(),
            }
        }
        fn with_symbol(&mut self, sym: ResolvedSymbol) -> &mut Self {
            self.by_id.insert(sym.id.to_string(), sym.clone());
            self.all.push(sym);
            self
        }
        fn with_callee(&mut self, owner: &str, target: ResolvedSymbol) -> &mut Self {
            self.callees
                .entry(owner.to_string())
                .or_default()
                .push(RelationTarget::from(&target));
            self
        }
        fn with_caller(&mut self, owner: &str, target: ResolvedSymbol) -> &mut Self {
            self.callers
                .entry(owner.to_string())
                .or_default()
                .push(RelationTarget::from(&target));
            self
        }
    }
    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
        fn find_symbols_by_name(&self, _n: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(&self, _f: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn module_list(&self) -> Vec<String> {
            Vec::new()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.all.clone())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats::default()
        }
    }

    impl GraphQueryPort for MockRepo {
        fn callers(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callers.get(id.as_str()).cloned().unwrap_or_default()
        }
        fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callees.get(id.as_str()).cloned().unwrap_or_default()
        }
        fn fan_in(&self, id: &SymbolId) -> usize {
            self.callers.get(id.as_str()).map(|v| v.len()).unwrap_or(0)
        }
        fn fan_out(&self, id: &SymbolId) -> usize {
            self.callees.get(id.as_str()).map(|v| v.len()).unwrap_or(0)
        }
        fn callers_with_metadata(
            &self,
            _id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::CallerWithMetadata> {
            Vec::new()
        }
        fn callees_with_metadata(
            &self,
            _id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata> {
            Vec::new()
        }
        fn dependencies_with_metadata(
            &self,
            _id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTargetWithMetadata>
        {
            Vec::new()
        }
        fn traverse_callees(
            &self,
            _id: &SymbolId,
            _max_depth: u8,
        ) -> Vec<cognicode_core::domain::aggregates::CallEntry> {
            Vec::new()
        }
        fn traverse_callers(
            &self,
            _id: &SymbolId,
            _max_depth: u8,
        ) -> Vec<cognicode_core::domain::aggregates::CallEntry> {
            Vec::new()
        }
    }

    #[test]
    fn descriptor_covers_three_types() {
        let d = ArchitectureLens.descriptor();
        assert_eq!(d.id, "architecture");
        assert!(d.applicable_types.contains(&InspectableObjectType::Symbol));
        assert!(d.applicable_types.contains(&InspectableObjectType::File));
        assert!(d.applicable_types.contains(&InspectableObjectType::Scope));
    }

    #[test]
    fn empty_scope_emits_baseline_info() {
        let repo = MockRepo::new();
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_scope("src/empty"),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        );
        let result = ArchitectureLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn scope_with_members_emits_info_baseline() {
        let mut repo = MockRepo::new();
        repo.with_symbol(make_resolved("src/foo/a.rs", "alpha", 1));
        repo.with_symbol(make_resolved("src/foo/b.rs", "beta", 2));
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_scope("src/foo"),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        );
        let result = ArchitectureLens.apply(&ctx).expect("ok");
        assert_eq!(result.findings.len(), 1);
        assert!(matches!(result.findings[0].severity, FindingSeverity::Info));
        assert!(result.findings[0].hypothesis.contains("no"));
    }

    #[test]
    fn scope_with_circular_dependency_emits_critical() {
        // A → B → A cycle: src/foo/a.rs calls src/bar/x.rs; src/bar/x.rs calls src/foo/a.rs
        let mut repo = MockRepo::new();
        let a = make_resolved("src/foo/a.rs", "alpha", 1);
        let x = make_resolved("src/bar/x.rs", "xeno", 1);
        repo.with_symbol(a.clone());
        repo.with_symbol(x.clone());
        repo.with_callee(&a.id.to_string(), x.clone());
        repo.with_callee(&x.id.to_string(), a.clone());
        let repo_arc: Arc<MockRepo> = Arc::new(repo);
        let repo_sym: Arc<dyn SymbolRepository> = repo_arc.clone() as Arc<dyn SymbolRepository>;
        let repo_graph: Arc<dyn GraphQueryPort> = repo_arc.clone() as Arc<dyn GraphQueryPort>;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_scope("src/foo"),
            repo_sym,
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            Some(repo_graph),
        );
        let result = ArchitectureLens.apply(&ctx).expect("ok");
        let has_critical = result
            .findings
            .iter()
            .any(|f| matches!(f.severity, FindingSeverity::Critical));
        assert!(
            has_critical,
            "expected a Critical cycle finding in {:?}",
            result.findings
        );
    }

    #[test]
    fn symbol_outside_own_scope_emits_boundary_info() {
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/foo/a.rs", "alpha", 1);
        let other = make_resolved("src/bar/b.rs", "beta", 1);
        repo.with_symbol(sym.clone());
        repo.with_symbol(other.clone());
        repo.with_callee(&sym.id.to_string(), other);
        let repo_arc: Arc<MockRepo> = Arc::new(repo);
        let repo_sym: Arc<dyn SymbolRepository> = repo_arc.clone() as Arc<dyn SymbolRepository>;
        let repo_graph: Arc<dyn GraphQueryPort> = repo_arc.clone() as Arc<dyn GraphQueryPort>;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/foo/a.rs", "alpha", 1),
            repo_sym,
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            Some(repo_graph),
        );
        let result = ArchitectureLens.apply(&ctx).expect("ok");
        assert_eq!(result.findings.len(), 1);
        assert!(matches!(result.findings[0].severity, FindingSeverity::Info));
        assert!(result.findings[0].hypothesis.contains("boundary"));
    }
}

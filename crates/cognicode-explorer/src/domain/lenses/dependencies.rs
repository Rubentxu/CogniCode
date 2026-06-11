//! `dependencies` lens — coupling + circular-dependency hints.
//!
//! The lens is a hypothesis, not a verdict: a Warning "high fan-out" is
//! an invitation to look, not a refactor mandate. The lens focuses on
//! call-graph fan-out / fan-in counts and cross-scope coupling.
//!
//! Severity mapping:
//! - `fan_out > 10` on a symbol → Warning (likely a god function).
//! - Circular cross-scope dependency → Critical.
//! - High cross-scope coupling (> 20) on a scope → Warning.
//! - All other cases → Info (the relationship exists; not necessarily
//!   a problem).

use std::collections::{HashSet, VecDeque};

use crate::domain::lens::{Lens, LensContext, cap_and_order, clamp_confidence, finding_id};
use crate::dto::{
    DesignFinding, FindingSeverity, InspectableObjectType, LensDescriptor, LensResult,
};
use crate::error::ExplorerResult;
use crate::ports::symbol_repository::ResolvedSymbol;

pub const LENS_ID: &str = "dependencies";
const FINDING_CAP: usize = 20;
const HIGH_FAN_OUT_THRESHOLD: usize = 10;
const HIGH_CROSS_SCOPE_THRESHOLD: usize = 20;
const HIGH_CROSS_FILE_THRESHOLD: usize = 5;
const CYCLE_MAX_DEPTH: usize = 16;

pub struct DependenciesLens;

impl Lens for DependenciesLens {
    fn id(&self) -> &str {
        LENS_ID
    }

    fn descriptor(&self) -> LensDescriptor {
        LensDescriptor {
            id: LENS_ID.into(),
            name: "Coupling Analysis".into(),
            description: "Surfaces coupling between symbols, files, and scopes. \
                Highlights high fan-out, cross-file/cross-scope coupling, and \
                potential circular dependencies. Findings are hypotheses — \
                verify before refactoring."
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
            crate::domain::ObjectIdentity::Symbol { .. } => {
                let sym_id = ctx.object_id.to_symbol_id().expect("symbol identity");
                if let Some(resolved) = ctx.symbol_repo.resolve(&sym_id)? {
                    analyse_symbol(&resolved, ctx, quality_present)
                } else {
                    Vec::new()
                }
            }
            crate::domain::ObjectIdentity::File { path } => {
                analyse_file(path, ctx, quality_present)
            }
            crate::domain::ObjectIdentity::Scope { path } => {
                analyse_scope(path, ctx, quality_present)
            }
            _ => Vec::new(),
        };

        let summary = if findings.is_empty() {
            "No coupling concerns detected at this object".to_string()
        } else {
            format!(
                "Detected {} coupling observation(s) at this object",
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

fn analyse_symbol(
    symbol: &ResolvedSymbol,
    ctx: &LensContext,
    quality_present: bool,
) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    let fan_in = ctx.symbol_repo.fan_in(&symbol.id);
    let fan_out = ctx.symbol_repo.fan_out(&symbol.id);
    let symbol_object_id = format!("symbol:{}:{}:{}", symbol.file, symbol.name, symbol.line);

    if fan_out > HIGH_FAN_OUT_THRESHOLD {
        let severity = if fan_out >= HIGH_FAN_OUT_THRESHOLD * 2 {
            FindingSeverity::Critical
        } else {
            FindingSeverity::Warning
        };
        let confidence = clamp_confidence(if quality_present { 0.85 } else { 0.6 });
        findings.push(DesignFinding {
            id: finding_id(
                LENS_ID,
                &format!("fanout:{}:{}:{}", symbol.file, symbol.name, symbol.line),
            ),
            lens_id: LENS_ID.into(),
            title: format!("High fan-out: {} ({} callees)", symbol.name, fan_out),
            hypothesis: format!(
                "This symbol may be over-reaching — it calls {} distinct \
                 callees. Could indicate a dispatcher or a place to extract \
                 smaller helpers. Verify the count is intentional.",
                fan_out
            ),
            severity,
            confidence,
            object_ids: vec![symbol_object_id.clone()],
            evidence_ids: vec!["evidence:symbol_callgraph".into()],
        });
    } else if fan_out > 0 {
        let confidence = clamp_confidence(if quality_present { 0.7 } else { 0.5 });
        findings.push(DesignFinding {
            id: finding_id(
                LENS_ID,
                &format!(
                    "fanout-info:{}:{}:{}",
                    symbol.file, symbol.name, symbol.line
                ),
            ),
            lens_id: LENS_ID.into(),
            title: format!("Coupling: {} ({} callees)", symbol.name, fan_out),
            hypothesis: format!(
                "{} callees is within the typical range — included for \
                 completeness so callers can see the relationship shape.",
                fan_out
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![symbol_object_id.clone()],
            evidence_ids: vec!["evidence:symbol_callgraph".into()],
        });
    }

    if fan_in > 0 && findings.is_empty() {
        // Add a one-line Info only if we haven't already produced one —
        // avoids noisy duplicate findings on symbols with both fan_in and
        // moderate fan_out.
        let confidence = clamp_confidence(if quality_present { 0.65 } else { 0.45 });
        findings.push(DesignFinding {
            id: finding_id(
                LENS_ID,
                &format!("fanin-info:{}:{}:{}", symbol.file, symbol.name, symbol.line),
            ),
            lens_id: LENS_ID.into(),
            title: format!("Callers: {} ({} incoming)", symbol.name, fan_in),
            hypothesis: format!(
                "{} caller(s) rely on this symbol — the relationship is \
                 normal; surfaced so the coupling shape is visible.",
                fan_in
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![symbol_object_id],
            evidence_ids: vec!["evidence:symbol_callgraph".into()],
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
    let mut cross_files: HashSet<String> = HashSet::new();
    for sym in &symbols {
        for callee in ctx.symbol_repo.callees(&sym.id) {
            if callee.file != file {
                cross_files.insert(callee.file.clone());
            }
        }
        for caller in ctx.symbol_repo.callers(&sym.id) {
            if caller.file != file {
                cross_files.insert(caller.file.clone());
            }
        }
    }
    let cross_count = cross_files.len();
    let file_object_id = format!("file:{file}");

    if cross_count > HIGH_CROSS_FILE_THRESHOLD {
        let severity = if cross_count >= HIGH_CROSS_FILE_THRESHOLD * 2 {
            FindingSeverity::Warning
        } else {
            FindingSeverity::Warning
        };
        let confidence = clamp_confidence(if quality_present { 0.8 } else { 0.55 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("cross-file:{file}")),
            lens_id: LENS_ID.into(),
            title: format!("Cross-file coupling: {} ({} file(s))", file, cross_count),
            hypothesis: format!(
                "This file's symbols call into {} distinct other file(s). \
                 Worth checking whether the boundary is intentional or an \
                 accumulation of small dependencies.",
                cross_count
            ),
            severity,
            confidence,
            object_ids: vec![file_object_id],
            evidence_ids: vec!["evidence:file_symbols".into()],
        });
    } else if cross_count > 0 {
        let confidence = clamp_confidence(if quality_present { 0.65 } else { 0.45 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("cross-file-info:{file}")),
            lens_id: LENS_ID.into(),
            title: format!("Cross-file calls: {} ({} file(s))", file, cross_count),
            hypothesis: format!(
                "{} cross-file relationship(s) detected — within the \
                 typical range; surfaced for visibility.",
                cross_count
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![file_object_id],
            evidence_ids: vec!["evidence:file_symbols".into()],
        });
    }
    findings
}

fn analyse_scope(scope: &str, ctx: &LensContext, quality_present: bool) -> Vec<DesignFinding> {
    // Build a scope→scope edge map by walking every member symbol's
    // callees/callers. We bucket by parent directory of the foreign
    // file, which is how the existing dependencies view also slices it.
    let all = ctx.symbol_repo.all_symbols().unwrap_or_default();
    let mut members: Vec<ResolvedSymbol> = Vec::new();
    for sym in &all {
        if crate::domain::views::scope_contains_file(scope, &sym.file) {
            members.push(sym.clone());
        }
    }

    // Scope → scope edge counts (outgoing from `scope`).
    let mut outgoing: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut incoming: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for sym in &members {
        for callee in ctx.symbol_repo.callees(&sym.id) {
            if crate::domain::views::scope_contains_file(scope, &callee.file) {
                continue;
            }
            let other = parent_dir(&callee.file);
            *outgoing.entry(other).or_insert(0) += 1;
        }
        for caller in ctx.symbol_repo.callers(&sym.id) {
            if crate::domain::views::scope_contains_file(scope, &caller.file) {
                continue;
            }
            let other = parent_dir(&caller.file);
            *incoming.entry(other).or_insert(0) += 1;
        }
    }

    let total_incoming: usize = incoming.values().sum();
    let mut findings = Vec::new();

    // Circular dependency detection (BFS, depth-limited).
    if let Some(cycle) = detect_scope_cycle(scope, &members, ctx) {
        let confidence = clamp_confidence(if quality_present { 0.9 } else { 0.7 });
        let cycle_label = cycle.join(" → ");
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("cycle:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("Possible cycle: {}", cycle_label),
            hypothesis: format!(
                "A possible dependency cycle was detected: {}. Circular \
                 dependencies can cause deployment pain and unexpected \
                 side effects. Verify whether the cycle is real before \
                 restructuring.",
                cycle_label
            ),
            severity: FindingSeverity::Critical,
            confidence,
            object_ids: cycle.iter().map(|s| format!("scope:{s}")).collect(),
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    }

    if total_incoming > HIGH_CROSS_SCOPE_THRESHOLD {
        let confidence = clamp_confidence(if quality_present { 0.8 } else { 0.55 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("high-incoming:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("High incoming cross-scope coupling: {}", scope),
            hypothesis: format!(
                "{} scope(s) call into this one. Could indicate a \
                 shared-kernel scope that is now load-bearing — review \
                 before splitting the module.",
                incoming.len()
            ),
            severity: FindingSeverity::Warning,
            confidence,
            object_ids: vec![format!("scope:{scope}")],
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    } else if !outgoing.is_empty() || !incoming.is_empty() {
        let confidence = clamp_confidence(if quality_present { 0.7 } else { 0.5 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("coupling-summary:{scope}")),
            lens_id: LENS_ID.into(),
            title: format!("Cross-scope coupling: {}", scope),
            hypothesis: format!(
                "Connected to {} scope(s) outgoing and {} incoming — \
                 within the typical range; surfaced for visibility.",
                outgoing.len(),
                incoming.len()
            ),
            severity: FindingSeverity::Info,
            confidence,
            object_ids: vec![format!("scope:{scope}")],
            evidence_ids: vec!["evidence:scope_dependencies".into()],
        });
    }

    findings
}

/// BFS from `scope` along outgoing cross-scope calls; if any visited
/// scope is `scope` again within `CYCLE_MAX_DEPTH` steps, the path is
/// returned.
fn detect_scope_cycle(
    scope: &str,
    members: &[ResolvedSymbol],
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
        // Walk every symbol whose parent dir is `current`.
        for sym in members
            .iter()
            .chain(ctx.symbol_repo.all_symbols().unwrap_or_default().iter())
        {
            if parent_dir(&sym.file) != current {
                continue;
            }
            for callee in ctx.symbol_repo.callees(&sym.id) {
                let other = parent_dir(&callee.file);
                if other == current {
                    continue;
                }
                if other == scope && path.len() >= 2 {
                    // Found a cycle: scope → ... → scope
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
    }
    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
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

    #[test]
    fn descriptor_covers_symbol_file_scope() {
        let d = DependenciesLens.descriptor();
        assert!(d.applicable_types.contains(&InspectableObjectType::Symbol));
        assert!(d.applicable_types.contains(&InspectableObjectType::File));
        assert!(d.applicable_types.contains(&InspectableObjectType::Scope));
    }

    #[test]
    fn symbol_with_high_fan_out_emits_warning() {
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 1);
        let owner = sym.id.to_string();
        repo.with_symbol(sym.clone());
        for i in 0..12 {
            let callee = make_resolved(&format!("src/c{i}.rs"), "c", 1);
            repo.with_callee(&owner, callee);
        }
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 1),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
        );
        let result = DependenciesLens.apply(&ctx).expect("ok");
        assert_eq!(result.findings.len(), 1);
        assert!(matches!(
            result.findings[0].severity,
            FindingSeverity::Warning
        ));
        assert!(result.findings[0].hypothesis.contains("over-reaching"));
    }

    #[test]
    fn symbol_with_low_fan_out_emits_info() {
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 1);
        let owner = sym.id.to_string();
        repo.with_symbol(sym.clone());
        // 3 callees → below HIGH_FAN_OUT_THRESHOLD but > 0 → Info.
        for i in 0..3 {
            let callee = make_resolved(&format!("src/c{i}.rs"), "c", 1);
            repo.with_callee(&owner, callee);
        }
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 1),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
        );
        let result = DependenciesLens.apply(&ctx).expect("ok");
        assert_eq!(result.findings.len(), 1);
        assert!(matches!(result.findings[0].severity, FindingSeverity::Info));
    }

    #[test]
    fn symbol_with_no_coupling_emits_no_finding() {
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 1);
        repo.with_symbol(sym.clone());
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 1),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
        );
        let result = DependenciesLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn file_with_no_cross_file_calls_emits_no_finding() {
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_file("src/empty.rs"),
            Arc::new(MockRepo::new()),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
        );
        let result = DependenciesLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn scope_with_no_cross_scope_calls_emits_no_finding() {
        let mut repo = MockRepo::new();
        repo.with_symbol(make_resolved("src/foo/a.rs", "alpha", 1));
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_scope("src/foo"),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
        );
        let result = DependenciesLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }
}

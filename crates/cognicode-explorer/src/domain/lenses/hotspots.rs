//! `hotspots` lens — risk-scored symbol / file / scope ranking.
//!
//! Hypothesis framing: a "hotspot" is a symbol, file, or scope where
//! high call-graph centrality AND/OR open quality issues cluster. The
//! lens never claims "this is bad" — it surfaces the cluster and
//! invites the reader to look.
//!
//! Risk formula:
//!   risk = fan_in * 0.4 + weighted_issue_count * 0.6
//!
//! where `weighted_issue_count` is the sum of `severity_weight` for
//! every open issue at the symbol's file/line (symbol case) or the
//! file's total (file case) or the scope's total (scope case).
//!
//! Confidence is higher when quality data is present (the lens has
//! more signals to combine).

use crate::domain::lens::{
    Lens, LensContext, cap_and_order, clamp_confidence, finding_id, severity_weight,
};
use crate::dto::{
    DesignFinding, FindingSeverity, InspectableObjectType, LensDescriptor, LensResult,
};
use crate::error::ExplorerResult;
use crate::ports::symbol_repository::{RelationTarget, ResolvedSymbol};
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;

/// Lens id — also the `lens_id` every finding carries.
pub const LENS_ID: &str = "hotspots";

/// Cap on findings produced per apply, per the spec.
const FINDING_CAP: usize = 20;

/// Threshold above which a symbol is considered "risky enough" to
/// produce a finding. Below it the lens stays silent (no noise).
const SYMBOL_RISK_THRESHOLD: f32 = 2.0;

/// Threshold for file-level aggregation. File risk = sum of its
/// symbols' risks + file-level quality issues.
const FILE_RISK_THRESHOLD: f32 = 5.0;

/// Threshold for scope-level aggregation.
const SCOPE_RISK_THRESHOLD: f32 = 10.0;

/// The hotspots lens.
pub struct HotspotsLens;

impl Lens for HotspotsLens {
    fn id(&self) -> &str {
        LENS_ID
    }

    fn descriptor(&self) -> LensDescriptor {
        LensDescriptor {
            id: LENS_ID.into(),
            name: "Risk Hotspots".into(),
            description: "Surfaces symbols, files, and scopes where high call-graph \
                centrality clusters with open quality issues. Findings are \
                hypotheses, not verdicts — review the evidence before acting."
                .into(),
            applicable_types: vec![
                InspectableObjectType::Symbol,
                InspectableObjectType::File,
                InspectableObjectType::Scope,
            ],
        }
    }

    fn apply(&self, ctx: &LensContext) -> ExplorerResult<LensResult> {
        let mut findings = Vec::new();
        let quality_present = ctx.quality_repo.is_some();

        match &ctx.object_id {
            crate::domain::ObjectIdentity::Symbol { .. } => {
                let sym_id = ctx.object_id.to_symbol_id().expect("symbol identity");
                if let Some(resolved) = ctx.symbol_repo.resolve(&sym_id)? {
                    let (risk, weighted) = symbol_risk(&resolved, ctx);
                    if risk >= SYMBOL_RISK_THRESHOLD {
                        findings.push(build_symbol_finding(
                            &resolved,
                            risk,
                            weighted,
                            quality_present,
                        ));
                    }
                }
            }
            crate::domain::ObjectIdentity::File { path } => {
                findings.extend(analyse_file(path, ctx));
            }
            crate::domain::ObjectIdentity::Scope { path } => {
                findings.extend(analyse_scope(path, ctx));
            }
            // Issue / Rule / Workspace: lens is not meaningful here.
            _ => {}
        }

        let summary = if findings.is_empty() {
            "No hotspots detected at this object".to_string()
        } else {
            format!(
                "Detected {} hotspot finding(s) at this object",
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

fn symbol_risk(symbol: &ResolvedSymbol, ctx: &LensContext) -> (f32, f32) {
    let fan_in = ctx
        .graph_query
        .as_ref()
        .map(|gq| gq.fan_in(&symbol.id))
        .unwrap_or(0) as f32;
    let weighted_issues: f32 = match &ctx.quality_repo {
        Some(q) => q
            .issues_at_line(&symbol.file, symbol.line)
            .unwrap_or_default()
            .iter()
            .map(|i| severity_weight(&i.severity))
            .sum(),
        None => 0.0,
    };
    let risk = fan_in * 0.4 + weighted_issues * 0.6;
    (risk, weighted_issues)
}

fn build_symbol_finding(
    symbol: &ResolvedSymbol,
    risk: f32,
    weighted: f32,
    quality_present: bool,
) -> DesignFinding {
    let severity = if risk >= 8.0 {
        FindingSeverity::Critical
    } else if risk >= 4.0 {
        FindingSeverity::Warning
    } else {
        FindingSeverity::Info
    };
    let confidence = clamp_confidence(if quality_present { 0.85 } else { 0.55 });
    let fan_in = risk / 0.4 - weighted * 1.5; // approximate reverse for the hypothesis text
    let _ = fan_in; // silence; the hypothesis is qualitative
    let hypothesis = format!(
        "May be a risk hotspot — risk score {:.2} combines fan-in with {} \
         quality finding(s) at this location. Worth a closer look.",
        risk, weighted as u32
    );
    let object_id = format!("symbol:{}:{}:{}", symbol.file, symbol.name, symbol.line);
    DesignFinding {
        id: finding_id(
            LENS_ID,
            &format!("{}:{}:{}", symbol.file, symbol.name, symbol.line),
        ),
        lens_id: LENS_ID.into(),
        title: format!(
            "Hotspot: {} at {}:{}",
            symbol.name, symbol.file, symbol.line
        ),
        hypothesis,
        severity,
        confidence,
        object_ids: vec![object_id],
        evidence_ids: vec![
            "evidence:symbol_callgraph".into(),
            "evidence:symbol_quality".into(),
        ],
    }
}

fn analyse_file(path: &str, ctx: &LensContext) -> Vec<DesignFinding> {
    let symbols = ctx
        .symbol_repo
        .find_symbols_by_file(path)
        .unwrap_or_default();
    let quality_present = ctx.quality_repo.is_some();
    let file_issues: f32 = match &ctx.quality_repo {
        Some(q) => q
            .issues_for_file(path)
            .unwrap_or_default()
            .iter()
            .map(|i| severity_weight(&i.severity))
            .sum(),
        None => 0.0,
    };

    // Sum symbol risks; treat as the file's risk score.
    let mut total_risk = file_issues * 0.6; // file-level issues contribute
    let mut hot_symbols: Vec<(ResolvedSymbol, f32, f32)> = Vec::new();
    for sym in &symbols {
        let (r, w) = symbol_risk(sym, ctx);
        total_risk += r;
        if r >= SYMBOL_RISK_THRESHOLD {
            hot_symbols.push((sym.clone(), r, w));
        }
    }

    if total_risk < FILE_RISK_THRESHOLD && hot_symbols.is_empty() {
        return Vec::new();
    }

    let mut findings: Vec<DesignFinding> = Vec::new();
    // One file-level finding summarising the cluster.
    let file_severity = if total_risk >= 15.0 {
        FindingSeverity::Critical
    } else if total_risk >= 8.0 {
        FindingSeverity::Warning
    } else {
        FindingSeverity::Info
    };
    let confidence = clamp_confidence(if quality_present { 0.8 } else { 0.5 });
    let file_object_id = format!("file:{path}");
    findings.push(DesignFinding {
        id: finding_id(LENS_ID, &format!("file:{path}")),
        lens_id: LENS_ID.into(),
        title: format!("Hotspot file: {}", path),
        hypothesis: format!(
            "This file may concentrate risk — {} symbol(s) scored above the \
             hotspot threshold and {} weighted quality issue(s) live here. \
             Review before refactors touch it.",
            hot_symbols.len(),
            file_issues as u32
        ),
        severity: file_severity,
        confidence,
        object_ids: vec![file_object_id.clone()],
        evidence_ids: vec![
            "evidence:file_overview".into(),
            "evidence:file_quality".into(),
        ],
    });

    // Up to 5 individual symbol findings (capped by FINDING_CAP downstream).
    for (sym, r, w) in hot_symbols.into_iter().take(5) {
        findings.push(build_symbol_finding(&sym, r, w, quality_present));
    }

    findings
}

fn analyse_scope(scope_path: &str, ctx: &LensContext) -> Vec<DesignFinding> {
    // Membership: use scope_contains_file semantics — anchor on `/`.
    let mut member_files: Vec<String> = Vec::new();
    let all = ctx.symbol_repo.all_symbols().unwrap_or_default();
    for sym in &all {
        if crate::domain::views::scope_contains_file(scope_path, &sym.file)
            && !member_files.contains(&sym.file)
        {
            member_files.push(sym.file.clone());
        }
    }
    let quality_present = ctx.quality_repo.is_some();
    let scope_issues: f32 = match &ctx.quality_repo {
        Some(q) => q
            .issues_for_scope(scope_path)
            .unwrap_or_default()
            .iter()
            .map(|i| severity_weight(&i.severity))
            .sum(),
        None => 0.0,
    };

    let mut total_risk = scope_issues * 0.6;
    let mut hot_files: Vec<(String, f32)> = Vec::new();
    for file in &member_files {
        let mut file_risk = 0.0_f32;
        if let Some(q) = &ctx.quality_repo {
            file_risk += q
                .issues_for_file(file)
                .unwrap_or_default()
                .iter()
                .map(|i| severity_weight(&i.severity))
                .sum::<f32>()
                * 0.6;
        }
        // Add per-symbol risks.
        let symbols = ctx
            .symbol_repo
            .find_symbols_by_file(file)
            .unwrap_or_default();
        for sym in &symbols {
            let (r, _) = symbol_risk(sym, ctx);
            file_risk += r;
        }
        total_risk += file_risk;
        if file_risk >= FILE_RISK_THRESHOLD {
            hot_files.push((file.clone(), file_risk));
        }
    }

    if total_risk < SCOPE_RISK_THRESHOLD && hot_files.is_empty() {
        return Vec::new();
    }

    let scope_severity = if total_risk >= 30.0 {
        FindingSeverity::Critical
    } else if total_risk >= 15.0 {
        FindingSeverity::Warning
    } else {
        FindingSeverity::Info
    };
    let confidence = clamp_confidence(if quality_present { 0.75 } else { 0.45 });
    let scope_object_id = format!("scope:{scope_path}");
    let mut findings: Vec<DesignFinding> = vec![DesignFinding {
        id: finding_id(LENS_ID, &format!("scope:{scope_path}")),
        lens_id: LENS_ID.into(),
        title: format!("Hotspot scope: {}", scope_path),
        hypothesis: format!(
            "This scope may concentrate risk — {} file(s) exceeded the \
             per-file threshold; total weighted quality issues ≈ {}. Worth \
             scoping a refactor here.",
            hot_files.len(),
            scope_issues as u32
        ),
        severity: scope_severity,
        confidence,
        object_ids: vec![scope_object_id],
        evidence_ids: vec![
            "evidence:scope_overview".into(),
            "evidence:scope_quality".into(),
        ],
    }];

    // Top-5 hot files, summarised.
    hot_files.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (file, risk) in hot_files.into_iter().take(5) {
        let sev = if risk >= 15.0 {
            FindingSeverity::Critical
        } else if risk >= 8.0 {
            FindingSeverity::Warning
        } else {
            FindingSeverity::Info
        };
        let conf = clamp_confidence(if quality_present { 0.8 } else { 0.5 });
        findings.push(DesignFinding {
            id: finding_id(LENS_ID, &format!("scope-file:{file}")),
            lens_id: LENS_ID.into(),
            title: format!("Hotspot file inside scope: {}", file),
            hypothesis: format!(
                "Inside the scope, this file is the strongest hotspot — risk \
                 score {:.2} combines symbol centrality and quality issues. \
                 Consider as a starting point for cleanup.",
                risk
            ),
            severity: sev,
            confidence: conf,
            object_ids: vec![format!("file:{file}")],
            evidence_ids: vec![
                "evidence:file_overview".into(),
                "evidence:file_quality".into(),
            ],
        });
    }

    findings
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::dto::InspectableObjectType;
    use crate::ports::quality_repository::{
        IssueFilter, QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
    };
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
        file_symbols: StdHashMap<String, Vec<ResolvedSymbol>>,
        all: Vec<ResolvedSymbol>,
    }
    impl MockRepo {
        fn new() -> Self {
            Self {
                by_id: StdHashMap::new(),
                callers: StdHashMap::new(),
                file_symbols: StdHashMap::new(),
                all: Vec::new(),
            }
        }
        fn with_symbol(&mut self, sym: ResolvedSymbol) -> &mut Self {
            self.by_id.insert(sym.id.to_string(), sym.clone());
            self.file_symbols
                .entry(sym.file.clone())
                .or_default()
                .push(sym.clone());
            self.all.push(sym);
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
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.file_symbols.get(file).cloned().unwrap_or_default())
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
        fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn fan_in(&self, id: &SymbolId) -> usize {
            self.callers.get(id.as_str()).map(|v| v.len()).unwrap_or(0)
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
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

    struct MockQuality {
        by_line: StdHashMap<(String, u32), Vec<QualityIssue>>,
        by_file: StdHashMap<String, Vec<QualityIssue>>,
        by_scope: StdHashMap<String, Vec<QualityIssue>>,
    }
    impl MockQuality {
        fn new() -> Self {
            Self {
                by_line: StdHashMap::new(),
                by_file: StdHashMap::new(),
                by_scope: StdHashMap::new(),
            }
        }
    }
    impl QualityRepository for MockQuality {
        fn issues_for_file(&self, f: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.by_file.get(f).cloned().unwrap_or_default())
        }
        fn issues_for_scope(&self, s: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.by_scope.get(s).cloned().unwrap_or_default())
        }
        fn issues_at_line(&self, f: &str, l: u32) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self
                .by_line
                .get(&(f.to_string(), l))
                .cloned()
                .unwrap_or_default())
        }
        fn issue_by_id(&self, _id: i64) -> ExplorerResult<Option<QualityIssue>> {
            Ok(None)
        }
        fn rule_summary(&self, _r: &str) -> ExplorerResult<RuleSummary> {
            unimplemented!()
        }
        fn quality_gate(&self) -> ExplorerResult<QualityGateSummary> {
            unimplemented!()
        }
        fn open_issues_count(&self) -> ExplorerResult<usize> {
            Ok(0)
        }
        fn issues_for_workspace(
            &self,
            _workspace_id: Option<&str>,
            filter: &IssueFilter,
        ) -> ExplorerResult<Vec<QualityIssue>> {
            let mut out: Vec<QualityIssue> = self
                .by_file
                .values()
                .chain(self.by_scope.values())
                .chain(self.by_line.values())
                .flat_map(|v| v.iter().cloned())
                .filter(|i| filter.severity.as_deref().is_none_or(|s| i.severity == s))
                .filter(|i| filter.category.as_deref().is_none_or(|c| i.category == c))
                .filter(|i| filter.status.as_deref().is_none_or(|s| i.status == s))
                .filter(|i| match &filter.file_prefix {
                    None => true,
                    Some(p) => i.file == *p || i.file.starts_with(&format!("{p}/")),
                })
                .collect();
            if let Some(n) = filter.limit {
                out.truncate(n);
            }
            Ok(out)
        }
    }

    fn make_issue(file: &str, line: u32, sev: &str) -> QualityIssue {
        QualityIssue {
            id: 1,
            rule_id: "test".into(),
            severity: sev.into(),
            category: "test".into(),
            file: file.into(),
            line,
            message: "test".into(),
            status: "open".into(),
        }
    }

    fn ctx_for(object_id: crate::domain::ObjectIdentity, repo: Arc<MockQuality>) -> LensContext {
        let repo_sym: Arc<dyn SymbolRepository> = Arc::new(MockRepo::new());
        let quality: Option<Arc<dyn QualityRepository>> =
            if repo.by_line.is_empty() && repo.by_file.is_empty() && repo.by_scope.is_empty() {
                None
            } else {
                Some(repo as Arc<dyn QualityRepository>)
            };
        // Wrap quality as Arc<dyn QualityRepository> only if Some
        let quality_arc: Option<Arc<dyn QualityRepository>> = quality.map(|q| {
            let arc: Arc<dyn QualityRepository> = q;
            arc
        });
        LensContext::new(
            object_id,
            repo_sym,
            quality_arc,
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        )
    }

    #[test]
    fn descriptor_lists_three_object_types() {
        let lens = HotspotsLens;
        let d = lens.descriptor();
        assert_eq!(d.id, "hotspots");
        assert!(d.applicable_types.contains(&InspectableObjectType::Symbol));
        assert!(d.applicable_types.contains(&InspectableObjectType::File));
        assert!(d.applicable_types.contains(&InspectableObjectType::Scope));
    }

    #[test]
    fn applies_to_default_delegates_to_descriptor() {
        let lens = HotspotsLens;
        assert!(lens.applies_to(&InspectableObjectType::Symbol));
        assert!(lens.applies_to(&InspectableObjectType::File));
        assert!(lens.applies_to(&InspectableObjectType::Scope));
        assert!(!lens.applies_to(&InspectableObjectType::QualityIssue));
    }

    #[test]
    fn symbol_below_threshold_emits_no_finding() {
        // A symbol with no callers and no issues → risk 0 → below threshold.
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 1);
        repo.with_symbol(sym.clone());
        let quality = MockQuality::new();
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 1),
            Arc::new(repo),
            Some(Arc::new(quality) as Arc<dyn QualityRepository>),
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        );
        let result = HotspotsLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
        assert!(result.summary.contains("No hotspots"));
    }

    #[test]
    fn symbol_with_high_fan_in_and_issues_emits_finding() {
        // Build a symbol with 8 callers → fan_in contribution 8*0.4 = 3.2.
        // Add a Blocker issue at the symbol's line → 3.0 * 0.6 = 1.8.
        // Total risk = 5.0, above threshold.
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 5);
        let owner = sym.id.to_string();
        repo.with_symbol(sym.clone());
        // Add 8 fake callers (use the same target as caller — fan_in is what
        // we care about).
        for i in 0..8 {
            let caller = make_resolved(&format!("src/c{i}.rs"), "caller", 1);
            repo.with_caller(&owner, caller);
        }
        let mut quality = MockQuality::new();
        quality.by_line.insert(
            ("src/a.rs".to_string(), 5u32),
            vec![make_issue("src/a.rs", 5, "Blocker")],
        );
        let quality_arc: Arc<dyn QualityRepository> = Arc::new(quality);
        let repo_arc: Arc<MockRepo> = Arc::new(repo);
        let repo_sym: Arc<dyn SymbolRepository> = repo_arc.clone() as Arc<dyn SymbolRepository>;
        let repo_graph: Arc<dyn GraphQueryPort> = repo_arc.clone() as Arc<dyn GraphQueryPort>;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 5),
            repo_sym,
            Some(quality_arc),
            Arc::new(FsSourceReader::new("/tmp")),
            Some(repo_graph),
        );
        let result = HotspotsLens.apply(&ctx).expect("ok");
        assert_eq!(result.findings.len(), 1, "exactly one finding");
        let f = &result.findings[0];
        assert_eq!(f.lens_id, "hotspots");
        assert!(f.hypothesis.contains("May be a risk hotspot"));
        // 5.0 → Warning (>=4.0)
        assert!(matches!(
            f.severity,
            FindingSeverity::Warning | FindingSeverity::Critical
        ));
        assert!(f.confidence > 0.7, "quality present → higher confidence");
    }

    #[test]
    fn file_with_hot_symbols_emits_finding() {
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/main.rs", "alpha", 1);
        let owner = sym.id.to_string();
        repo.with_symbol(sym.clone());
        // Give alpha 10 callers → fan_in 10 → contribution 4.0.
        for i in 0..10 {
            let caller = make_resolved(&format!("src/c{i}.rs"), "c", 1);
            repo.with_caller(&owner, caller);
        }
        let mut quality = MockQuality::new();
        // Add file-level issue weight to push above FILE_RISK_THRESHOLD.
        quality.by_file.insert(
            "src/main.rs".to_string(),
            vec![
                make_issue("src/main.rs", 1, "Critical"),
                make_issue("src/main.rs", 2, "Major"),
            ],
        );
        let quality_arc: Arc<dyn QualityRepository> = Arc::new(quality);
        let repo_arc: Arc<MockRepo> = Arc::new(repo);
        let repo_sym: Arc<dyn SymbolRepository> = repo_arc.clone() as Arc<dyn SymbolRepository>;
        let repo_graph: Arc<dyn GraphQueryPort> = repo_arc.clone() as Arc<dyn GraphQueryPort>;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_file("src/main.rs"),
            repo_sym,
            Some(quality_arc),
            Arc::new(FsSourceReader::new("/tmp")),
            Some(repo_graph),
        );
        let result = HotspotsLens.apply(&ctx).expect("ok");
        assert!(!result.findings.is_empty(), "file-level finding expected");
        let titles: Vec<&str> = result.findings.iter().map(|f| f.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("Hotspot file")));
    }

    #[test]
    fn quality_absent_does_not_error_and_lowers_confidence() {
        // No quality repo wired → lens degrades, emits Info-level findings
        // when the symbol is hot enough on fan-in alone.
        let mut repo = MockRepo::new();
        let sym = make_resolved("src/a.rs", "alpha", 1);
        let owner = sym.id.to_string();
        repo.with_symbol(sym.clone());
        // 10 callers → fan_in 10 → risk 4.0 (above 2.0).
        for i in 0..10 {
            let caller = make_resolved(&format!("src/c{i}.rs"), "c", 1);
            repo.with_caller(&owner, caller);
        }
        let repo_arc: Arc<MockRepo> = Arc::new(repo);
        let repo_sym: Arc<dyn SymbolRepository> = repo_arc.clone() as Arc<dyn SymbolRepository>;
        let repo_graph: Arc<dyn GraphQueryPort> = repo_arc.clone() as Arc<dyn GraphQueryPort>;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_symbol("src/a.rs", "alpha", 1),
            repo_sym,
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            Some(repo_graph),
        );
        let result = HotspotsLens
            .apply(&ctx)
            .expect("ok — no quality is not an error");
        assert_eq!(result.findings.len(), 1);
        // Without quality, confidence should be < 0.7.
        assert!(
            result.findings[0].confidence < 0.7,
            "confidence lowered: {}",
            result.findings[0].confidence
        );
    }

    #[test]
    fn scope_with_no_members_emits_no_finding() {
        let repo = MockRepo::new();
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_scope("src/empty"),
            Arc::new(repo),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        );
        let result = HotspotsLens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn unrelated_object_types_are_no_ops() {
        // Issue / Rule identities should be ignored by the hotspots lens.
        let lens = HotspotsLens;
        let ctx = LensContext::new(
            crate::domain::ObjectIdentity::new_quality_issue(42),
            Arc::new(MockRepo::new()),
            None,
            Arc::new(FsSourceReader::new("/tmp")),
            None,
        );
        let result = lens.apply(&ctx).expect("ok");
        assert!(result.findings.is_empty());
    }
}

//! QualityIssue compatibility projection (M6.3).
//!
//! Legacy quality issues (`domain::ports::quality_store::QualityIssue`)
//! are lifted into the evidence-backed [`Finding`] model so the Explorer /
//! MCP quality surface is not forced to fork onto a second finding type
//! (umbrella M6 deliverable "legacy QualityIssue projection").
//!
//! ## Policy (documented, deliberate)
//!
//! Legacy rows carry no graded evidence and no causal flow — only a file
//! and line. The projection is therefore **conservative**:
//!
//! - `evidence_class = C` (partial static evidence), never `A`/`B`;
//! - `evidence` is left **empty** (no fabricated handles);
//! - the causal chain carries only the `location` step;
//! - the detector execution ref is `legacy.quality@legacy` recorded with
//!   **`Candidate` authority at execution** — legacy issues never inherit
//!   blocking authority;
//! - [`FindingOrigin::LegacyQuality`] preserves `issue_id` + `rule_id` so
//!   the finding can be traced back to the legacy row.
//!
//! Consequently a projected finding is **not** [`Finding::is_explainable`]
//! and **cannot block** the new gates — a re-evidence pass must upgrade it
//! before it participates.
//!
//! Pure domain: no I/O, no `sqlx`, no `tokio`.

use crate::domain::ports::quality_store::QualityIssue;

use super::detector_ir::{DetectorAuthority, DetectorExecutionRef, DetectorId, FindingKind};
use super::digest::DetectorDigest;
use super::finding::{
    CausalStep, CausalStepKind, EvidenceClass, Finding, FindingId, FindingOrigin, FindingSeverity,
    FindingStatus, RiskLevel,
};

/// Namespace used for projected quality findings (`quality.<category>`).
pub const QUALITY_NAMESPACE: &str = "quality";

/// Synthetic detector id for legacy quality rows.
pub const LEGACY_DETECTOR_ID: &str = "legacy.quality";

/// Synthetic detector version for legacy quality rows.
pub const LEGACY_DETECTOR_VERSION: &str = "legacy";

/// Sanitize an arbitrary label into a non-empty `ns.name` segment.
///
/// Lowercases, maps non-alphanumerics to `_`, collapses runs, trims
/// leading/trailing `_`. Falls back to `uncategorized`.
fn sanitize_segment(input: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "uncategorized".to_string()
    } else {
        out
    }
}

/// Map a legacy severity string to the display severity.
fn severity_from_str(value: &str) -> FindingSeverity {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" | "crit" | "blocker" | "error" | "high" => FindingSeverity::Critical,
        "warning" | "warn" | "major" | "medium" => FindingSeverity::Warning,
        _ => FindingSeverity::Info,
    }
}

/// Map display severity to a default risk level.
fn risk_for(severity: FindingSeverity) -> RiskLevel {
    match severity {
        FindingSeverity::Critical => RiskLevel::High,
        FindingSeverity::Warning => RiskLevel::Medium,
        FindingSeverity::Info => RiskLevel::Low,
    }
}

/// Map a legacy status string to a distinct finding status.
///
/// `false_positive`, `wontfix` and `suppressed` are **not** collapsed:
/// they are a false positive, an accepted risk and a suppression
/// respectively.
fn status_from_str(value: &str) -> FindingStatus {
    match value.trim().to_ascii_lowercase().as_str() {
        "accepted" | "confirmed" | "acknowledged" => FindingStatus::Accepted,
        "fixed" | "resolved" | "closed" | "done" => FindingStatus::Fixed,
        "false_positive" | "falsepositive" => FindingStatus::FalsePositive,
        "wontfix" | "risk_accepted" | "riskaccepted" | "accept_risk" => FindingStatus::RiskAccepted,
        "suppressed" | "ignored" | "exempt" => FindingStatus::Suppressed,
        _ => FindingStatus::Open,
    }
}

/// Project a legacy [`QualityIssue`] into a [`Finding`].
pub fn project_quality_issue(issue: &QualityIssue) -> Result<Finding, super::FindingError> {
    let kind = FindingKind::new(format!(
        "{}.{}",
        QUALITY_NAMESPACE,
        sanitize_segment(&issue.category)
    ))?;

    let severity = severity_from_str(&issue.severity);

    // The legacy detector never held blocking authority.
    let d =
        DetectorDigest::from_content(&format!("{LEGACY_DETECTOR_ID}@{LEGACY_DETECTOR_VERSION}"));
    let detector = DetectorExecutionRef::new(
        DetectorId::new(LEGACY_DETECTOR_ID)?,
        LEGACY_DETECTOR_VERSION,
        DetectorAuthority::Candidate,
        super::detector_ir::DetectorDigests {
            logic: d.clone(),
            policy: d.clone(),
            semantic: d.clone(),
            instance: d,
        },
        None,
    )?;

    let location = format!("{}:{}", issue.file_path, issue.line);
    let causal_chain = vec![CausalStep::new(CausalStepKind::Location, location)?];

    let mut finding = Finding {
        id: FindingId::new(format!("quality-{}", issue.id))?,
        kind,
        origin: FindingOrigin::LegacyQuality {
            issue_id: issue.id,
            rule_id: issue.rule_id.clone(),
        },
        severity,
        risk: risk_for(severity),
        evidence_class: EvidenceClass::C,
        evidence: Vec::new(),
        detector,
        status: status_from_str(&issue.status),
        message: issue.message.clone(),
        causal_chain,
    };

    // Normalize an empty message so the projection never yields an
    // unvalidatable finding; preserve the legacy rule id as context.
    if finding.message.trim().is_empty() {
        finding.message = format!("legacy quality issue for rule `{}`", issue.rule_id);
    }

    finding.validate()?;
    Ok(finding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(severity: &str, category: &str, status: &str, message: &str) -> QualityIssue {
        QualityIssue {
            id: 7,
            rule_id: "rule-1".to_string(),
            severity: severity.to_string(),
            category: category.to_string(),
            file_path: "src/main.rs".to_string(),
            line: 42,
            message: message.to_string(),
            status: status.to_string(),
        }
    }

    #[test]
    fn projects_identity_kind_and_origin() {
        let f = project_quality_issue(&issue("critical", "security", "open", "boom")).unwrap();
        assert_eq!(f.id.as_str(), "quality-7");
        assert_eq!(f.kind.as_str(), "quality.security");
        assert_eq!(
            f.origin,
            FindingOrigin::LegacyQuality {
                issue_id: 7,
                rule_id: "rule-1".to_string(),
            }
        );
    }

    #[test]
    fn sanitizes_category_into_namespace_segment() {
        let f = project_quality_issue(&issue("info", "Code Smell/Coupling", "open", "x")).unwrap();
        assert_eq!(f.kind.as_str(), "quality.code_smell_coupling");
    }

    #[test]
    fn empty_category_falls_back_to_uncategorized() {
        let f = project_quality_issue(&issue("info", "", "open", "x")).unwrap();
        assert_eq!(f.kind.as_str(), "quality.uncategorized");
    }

    #[test]
    fn maps_severity_to_display_and_risk() {
        let critical = project_quality_issue(&issue("critical", "c", "open", "x")).unwrap();
        assert_eq!(critical.severity, FindingSeverity::Critical);
        assert_eq!(critical.risk, RiskLevel::High);

        let warning = project_quality_issue(&issue("warning", "c", "open", "x")).unwrap();
        assert_eq!(warning.severity, FindingSeverity::Warning);
        assert_eq!(warning.risk, RiskLevel::Medium);

        let info = project_quality_issue(&issue("info", "c", "open", "x")).unwrap();
        assert_eq!(info.severity, FindingSeverity::Info);
        assert_eq!(info.risk, RiskLevel::Low);
    }

    #[test]
    fn unknown_severity_defaults_to_info() {
        let f = project_quality_issue(&issue("nonsense", "c", "open", "x")).unwrap();
        assert_eq!(f.severity, FindingSeverity::Info);
    }

    #[test]
    fn status_dispositions_are_distinct() {
        // wontfix != false positive.
        assert_eq!(
            project_quality_issue(&issue("info", "c", "wontfix", "x"))
                .unwrap()
                .status,
            FindingStatus::RiskAccepted
        );
        assert_eq!(
            project_quality_issue(&issue("info", "c", "false_positive", "x"))
                .unwrap()
                .status,
            FindingStatus::FalsePositive
        );
        assert_eq!(
            project_quality_issue(&issue("info", "c", "suppressed", "x"))
                .unwrap()
                .status,
            FindingStatus::Suppressed
        );
        assert_eq!(
            project_quality_issue(&issue("info", "c", "accepted", "x"))
                .unwrap()
                .status,
            FindingStatus::Accepted
        );
        assert_eq!(
            project_quality_issue(&issue("info", "c", "resolved", "x"))
                .unwrap()
                .status,
            FindingStatus::Fixed
        );
        assert_eq!(
            project_quality_issue(&issue("info", "c", "whatever", "x"))
                .unwrap()
                .status,
            FindingStatus::Open
        );
    }

    #[test]
    fn carries_location_as_causal_step() {
        let f = project_quality_issue(&issue("info", "c", "open", "x")).unwrap();
        assert_eq!(f.causal_chain.len(), 1);
        assert_eq!(f.causal_chain[0].kind, CausalStepKind::Location);
        assert_eq!(f.causal_chain[0].detail, "src/main.rs:42");
    }

    #[test]
    fn projected_finding_is_not_explainable_and_cannot_block() {
        let f = project_quality_issue(&issue("critical", "security", "open", "boom")).unwrap();
        assert!(f.evidence.is_empty());
        assert!(!f.is_explainable());
        assert_eq!(
            f.detector.authority_at_execution,
            DetectorAuthority::Candidate
        );
        let gate = super::super::FindingGate::new(EvidenceClass::C, RiskLevel::Low);
        assert!(!f.can_block(&gate), "projected findings must not block");
    }

    #[test]
    fn empty_message_is_replaced_with_rule_context() {
        let f = project_quality_issue(&issue("info", "c", "open", "   ")).unwrap();
        assert!(f.message.contains("rule-1"));
        assert!(f.validate().is_ok());
    }

    #[test]
    fn projected_finding_round_trips() {
        let f = project_quality_issue(&issue("warning", "design", "accepted", "x")).unwrap();
        let json = serde_json::to_string(&f).unwrap();
        let parsed: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, f);
    }
}

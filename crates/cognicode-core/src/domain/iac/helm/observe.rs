//! Projects a chart analysis into the structural facts the security pack
//! consumes.
//!
//! This is the seam between "what a render shows" (the Helm model) and "what a
//! policy does about it" (the findings pack and the gate that owns it). A
//! violation found in rendered objects becomes a
//! [`StructuralFact::Misconfiguration`]. A check that could not decide, and
//! every check of a render that is not complete, becomes a
//! [`StructuralFact::UnknownValue`]: it never turns into a finding, but it
//! keeps a negative from reading as clean.
//!
//! Rendered documents have no source lines. A finding sits on line 1 of the
//! template the engine reported for its document, and the object, field path,
//! chart version and values digest that fixed it are in the fact's text.

use std::collections::BTreeMap;

use super::analysis::{ChartAnalysis, CheckVerdict, RenderCoverage};
use super::checks::{HelmCheck, HelmFinding};
use crate::domain::findings::GroundingRef;
use crate::domain::findings::semantic_model::{
    ObservedUnit, SourceLanguage, StructuralFact, StructuralFamily, StructuralObservation,
};

/// The structural family that carries a check's findings.
///
/// Hardening gaps that a runtime would otherwise close (running as root,
/// privilege escalation, a mutable image) are advisory; privileged and host
/// access are permission findings; a published service is public exposure.
pub fn structural_family(check: HelmCheck) -> StructuralFamily {
    match check {
        HelmCheck::PrivilegedContainer | HelmCheck::HostAccess | HelmCheck::BroadRbac => {
            StructuralFamily::InfraPermissions
        }
        HelmCheck::ExternalExposure => StructuralFamily::InfraPublicExposure,
        HelmCheck::PrivilegeEscalation | HelmCheck::RunsAsRoot | HelmCheck::MutableImage => {
            StructuralFamily::InfraWorkloadHardening
        }
    }
}

/// One observed unit per place the analysis speaks about, in path order.
///
/// The chart itself (`<chart>/Chart.yaml`) is always a unit, so a chart that
/// rendered nothing, or nothing wrong, still gets an explicit verdict instead
/// of silence. Each template that produced a finding is a further unit.
///
/// When the render is not complete, or a check could not decide, every unit
/// carries an unknown for that check: nothing observed there is a clean
/// result. `grounding` maps a path and line to the canonical fact the line was
/// projected from; without it a finding is explainable but cannot block.
pub fn observed_units(
    analysis: &ChartAnalysis,
    grounding: &dyn Fn(&str, u32) -> Option<GroundingRef>,
) -> Vec<ObservedUnit> {
    let chart_path = format!("{}/Chart.yaml", analysis.chart);
    let mut by_path: BTreeMap<String, Vec<StructuralObservation>> = BTreeMap::new();
    by_path.insert(chart_path.clone(), Vec::new());
    for finding in analysis.findings() {
        let path = finding
            .origin
            .template
            .clone()
            .unwrap_or_else(|| chart_path.clone());
        by_path.entry(path).or_default();
    }
    let undecided = undecided_checks(analysis);
    by_path
        .into_iter()
        .map(|(path, mut facts)| {
            facts.extend(
                analysis
                    .findings()
                    .filter(|f| f.origin.template.as_deref().unwrap_or(&chart_path) == path)
                    .map(|f| observation(f, &path, grounding)),
            );
            facts.extend(
                undecided
                    .iter()
                    .map(|(check, causes)| unknown(*check, analysis, causes)),
            );
            ObservedUnit {
                path,
                language: SourceLanguage::Helm,
                facts,
            }
        })
        .collect()
}

fn observation(
    finding: &HelmFinding,
    path: &str,
    grounding: &dyn Fn(&str, u32) -> Option<GroundingRef>,
) -> StructuralObservation {
    let origin = &finding.origin;
    StructuralObservation {
        line: 1,
        grounding: grounding(path, 1),
        fact: StructuralFact::Misconfiguration {
            family: structural_family(finding.check),
            check: finding.check.id().to_string(),
            resource: finding.object.to_string(),
            attribute: finding.location.clone(),
            detail: format!(
                "{} [chart {} {}, template {}, values {}]",
                finding.detail,
                origin.chart,
                origin.chart_version,
                origin.template.as_deref().unwrap_or("unreported"),
                origin.values_digest
            ),
        },
    }
}

fn unknown(check: HelmCheck, analysis: &ChartAnalysis, causes: &[String]) -> StructuralObservation {
    StructuralObservation {
        line: 1,
        grounding: None,
        fact: StructuralFact::UnknownValue {
            family: structural_family(check),
            resource: format!("chart/{}", analysis.chart),
            attribute: check.id().to_string(),
            causes: causes.to_vec(),
        },
    }
}

/// The checks whose result cannot be read as clean, with why. A render that is
/// not complete leaves every check undecided, even one that found something:
/// what else it would have found is not known.
fn undecided_checks(analysis: &ChartAnalysis) -> Vec<(HelmCheck, Vec<String>)> {
    let incomplete = !matches!(analysis.coverage, RenderCoverage::Complete);
    analysis
        .results
        .iter()
        .filter_map(|result| match &result.verdict {
            CheckVerdict::Unknown { reasons } => Some((result.check, reasons.clone())),
            _ if incomplete => Some((result.check, vec!["the render is not complete".to_string()])),
            _ => None,
        })
        .collect()
}

//! The text form of the `security` answer.

use super::{
    FamilyVerdict, GoalSecurity, HelmReport, MatrixRow, TerraformReport, UnitReport, VerdictKind,
};

/// The word a verdict is printed with. The four are distinct on purpose.
fn verdict_word(kind: VerdictKind) -> &'static str {
    match kind {
        VerdictKind::Found => "FOUND",
        VerdictKind::NotObserved => "NOT OBSERVED",
        VerdictKind::Unknown => "UNKNOWN",
        VerdictKind::Unsupported => "UNSUPPORTED",
    }
}

pub(super) fn render_text(answer: &GoalSecurity) -> String {
    let mut text = String::new();
    text.push_str(&answer.summary);
    text.push('\n');
    for report in &answer.terraform {
        terraform(&mut text, report);
    }
    for report in &answer.helm {
        helm(&mut text, report);
    }
    for skipped in &answer.unanalysed {
        text.push_str(&format!(
            "\nNOT ANALYSED {} {}: {}\n",
            skipped.kind, skipped.path, skipped.reason
        ));
    }
    text.push_str(&format!(
        "\ncapability matrix (pack {}):\n",
        answer.pack_version
    ));
    for row in &answer.matrix {
        matrix_row(&mut text, row);
    }
    text.push_str("\nlimits:\n");
    for limit in &answer.limits {
        text.push_str(&format!("  - {limit}\n"));
    }
    text
}

fn terraform(text: &mut String, report: &TerraformReport) {
    let coverage = &report.coverage;
    text.push_str(&format!(
        "\nterraform {}: coverage {} ({} of {} resource(s) assessed)\n",
        report.root,
        if coverage.complete {
            "complete"
        } else {
            "incomplete"
        },
        coverage.assessed,
        coverage.resources
    ));
    for (label, values) in [
        (
            "resource types no check models",
            &coverage.not_evaluated_types,
        ),
        ("modules not opened", &coverage.unresolved_modules),
        ("sources not read", &coverage.unread),
        ("variables nothing binds", &coverage.unbound_variables),
    ] {
        if !values.is_empty() {
            text.push_str(&format!("  {label}: {}\n", values.join(", ")));
        }
    }
    units(text, &report.units);
}

fn helm(text: &mut String, report: &HelmReport) {
    let render = match (&report.render.documents, &report.render.cause) {
        (Some(documents), _) => format!("rendered ({documents} document(s))"),
        (None, Some(cause)) => format!("not rendered ({cause})"),
        (None, None) => "not rendered".to_string(),
    };
    text.push_str(&format!(
        "\nhelm {} (chart {} {}): {render}; coverage {}\n",
        report.path, report.chart, report.version, report.coverage.state
    ));
    for reason in &report.coverage.reasons {
        text.push_str(&format!("  undecided: {reason}\n"));
    }
    for limit in &report.limits {
        text.push_str(&format!("  limit: {limit}\n"));
    }
    units(text, &report.units);
}

fn units(text: &mut String, units: &[UnitReport]) {
    for unit in units {
        text.push_str(&format!("  {}\n", unit.path));
        for family in &unit.families {
            family_line(text, family);
        }
    }
}

fn family_line(text: &mut String, family: &FamilyVerdict) {
    text.push_str(&format!(
        "    {:<12} {}: {}\n",
        verdict_word(family.verdict),
        family.family,
        family.detail
    ));
    for unresolved in &family.unresolved {
        text.push_str(&format!("      undecided: {unresolved}\n"));
    }
    for finding in &family.findings {
        text.push_str(&format!(
            "      - [{} risk, evidence {}] {}\n",
            finding.risk, finding.evidence_class, finding.message
        ));
    }
    for diagnostic in &family.diagnostics {
        text.push_str(&format!("      note: {diagnostic}\n"));
    }
}

fn matrix_row(text: &mut String, row: &MatrixRow) {
    match (&row.backend, &row.reason) {
        (Some(backend), _) => text.push_str(&format!(
            "  {} {}: supported ({}, backend {}, evidence ceiling {}, {}, {})\n",
            row.language,
            row.family,
            row.algorithm,
            backend,
            row.evidence_ceiling.as_deref().unwrap_or("none"),
            row.nature,
            if row.can_block {
                "can block once grounded"
            } else {
                "never blocks at this ceiling"
            }
        )),
        (None, reason) => text.push_str(&format!(
            "  {} {}: unsupported ({})\n",
            row.language,
            row.family,
            reason.as_deref().unwrap_or("no reason given")
        )),
    }
    for limit in &row.limits {
        text.push_str(&format!("      limit: {limit}\n"));
    }
}

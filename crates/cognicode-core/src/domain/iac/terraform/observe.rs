//! Projects an analysis into the structural facts the security pack consumes.
//!
//! This is the seam between "what the configuration says" (this model) and
//! "what a policy does about it" (the findings pack and the gate that owns it).
//! A check that concluded from known values becomes a
//! [`StructuralFact::Misconfiguration`]; a check that could not decide becomes
//! a [`StructuralFact::UnknownValue`], which can never turn into a finding but
//! keeps a negative from reading as clean. Hardcoded secrets are not decided
//! here: the literals are handed to the pack's own secret detector as
//! [`StructuralFact::StringBinding`], so there is one definition of what a
//! secret looks like.

use std::collections::{BTreeMap, BTreeSet};

use super::analysis::TfAnalysis;
use super::graph::Def;
use super::ir::TfExpr;
use super::rules::{CheckResult, Outcome, RiskFamily};
use super::value::TfValue;
use crate::domain::findings::GroundingRef;
use crate::domain::findings::semantic_model::{
    ObservedUnit, SourceLanguage, StructuralFact, StructuralFamily, StructuralObservation,
};

/// The structural family that carries a risk family's findings.
pub fn structural_family(family: RiskFamily) -> StructuralFamily {
    match family {
        RiskFamily::PublicExposure => StructuralFamily::InfraPublicExposure,
        RiskFamily::Encryption => StructuralFamily::InfraEncryption,
        RiskFamily::Permissions => StructuralFamily::InfraPermissions,
    }
}

/// One observed unit per file the analysis looked at, in path order. A file
/// is included when one of its resources was assessed or it holds a literal
/// bound to a name; its facts are in line order.
///
/// `grounding` maps a file and line to the canonical fact that line was
/// projected from. Without it a finding is explainable but cannot block.
pub fn observed_units(
    analysis: &TfAnalysis,
    grounding: &dyn Fn(&str, u32) -> Option<GroundingRef>,
) -> Vec<ObservedUnit> {
    let mut by_file: BTreeMap<String, Vec<StructuralObservation>> = BTreeMap::new();
    for result in analysis.results() {
        let facts = by_file.entry(result.file.clone()).or_default();
        if let Some(fact) = fact_of(result) {
            facts.push(StructuralObservation {
                line: result.line,
                grounding: grounding(&result.file, result.line),
                fact,
            });
        }
    }
    let mut seen = BTreeSet::new();
    for node in analysis.graph().nodes() {
        let Some((name, text)) = literal_binding(&node.def, &node.name) else {
            continue;
        };
        if !seen.insert((node.file.clone(), node.line, name.clone())) {
            continue;
        }
        by_file
            .entry(node.file.clone())
            .or_default()
            .push(StructuralObservation {
                line: node.line,
                grounding: grounding(&node.file, node.line),
                fact: StructuralFact::StringBinding { name, value: text },
            });
    }
    by_file
        .into_iter()
        .map(|(path, mut facts)| {
            facts.sort_by_key(|f| f.line);
            ObservedUnit {
                path,
                language: SourceLanguage::Terraform,
                facts,
            }
        })
        .collect()
}

fn fact_of(result: &CheckResult) -> Option<StructuralFact> {
    let family = structural_family(result.family);
    match &result.outcome {
        Outcome::Violation { detail } => Some(StructuralFact::Misconfiguration {
            family,
            check: result.check.to_string(),
            resource: result.resource.clone(),
            attribute: result.attribute.clone(),
            detail: detail.clone(),
        }),
        Outcome::Unverified { causes } => Some(StructuralFact::UnknownValue {
            family,
            resource: result.resource.clone(),
            attribute: result.attribute.clone(),
            causes: causes.iter().map(ToString::to_string).collect(),
        }),
        Outcome::Satisfied => None,
    }
}

/// The name and text of a node whose source is a string literal. Values that
/// flow in from elsewhere are not literals in the source and are not secrets
/// committed at this line.
fn literal_binding(def: &Def, name: &str) -> Option<(String, String)> {
    let expr = match def {
        Def::Expr(expr) => expr,
        Def::Variable {
            default: Some(expr),
            ..
        } => expr,
        _ => return None,
    };
    match expr {
        TfExpr::Literal(TfValue::Str(text)) => Some((name.to_string(), text.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::eval::Bindings;
    use super::super::ir::{
        ResourceMode, TfAttribute, TfBody, TfModule, TfProject, TfProvider, TfResource, TfVariable,
    };
    use super::*;
    use crate::domain::findings::semantic_model::{
        StructuralClassification, classify_structural, is_hardcoded_secret,
    };

    fn lit(text: &str) -> TfExpr {
        TfExpr::Literal(TfValue::Str(text.into()))
    }

    fn attr(name: &str, expr: TfExpr, line: u32) -> TfAttribute {
        TfAttribute {
            name: name.into(),
            expr,
            line,
        }
    }

    fn project() -> TfProject {
        let mut module = TfModule::default();
        module.resources.insert(
            "aws_s3_bucket.b".into(),
            TfResource {
                mode: ResourceMode::Managed,
                kind: "aws_s3_bucket".into(),
                name: "b".into(),
                body: TfBody {
                    attributes: vec![attr("acl", lit("public-read"), 3)],
                    blocks: vec![],
                },
                file: "main.tf".into(),
                line: 1,
            },
        );
        module.resources.insert(
            "aws_ebs_volume.d".into(),
            TfResource {
                mode: ResourceMode::Managed,
                kind: "aws_ebs_volume".into(),
                name: "d".into(),
                body: TfBody::default(),
                file: "disks.tf".into(),
                line: 1,
            },
        );
        module.variables.insert(
            "db_password".into(),
            TfVariable {
                name: "db_password".into(),
                default: Some(lit("s3cr3t-Value-42")),
                sensitive: false,
                file: "variables.tf".into(),
                line: 5,
            },
        );
        module.providers.push(TfProvider {
            name: "aws".into(),
            body: TfBody {
                attributes: vec![attr("secret_key", lit("AKIAFAKEFAKEFAKE1234"), 8)],
                blocks: vec![],
            },
            file: "providers.tf".into(),
            line: 6,
        });
        TfProject {
            modules: BTreeMap::from([(String::new(), module)]),
            ..Default::default()
        }
    }

    fn units() -> Vec<ObservedUnit> {
        let analysis = TfAnalysis::run(&project(), "", Bindings::none());
        observed_units(&analysis, &|_, line| {
            Some(GroundingRef::fact(crate::domain::kernel_ids::FactId::new(
                u64::from(line),
            )))
        })
    }

    fn unit<'a>(units: &'a [ObservedUnit], path: &str) -> &'a ObservedUnit {
        units.iter().find(|u| u.path == path).expect("unit exists")
    }

    #[test]
    fn a_verified_risk_unknown_value_and_literal_each_land_in_their_file() {
        let units = units();
        let paths: Vec<&str> = units.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(
            paths,
            ["disks.tf", "main.tf", "providers.tf", "variables.tf"]
        );
        assert!(
            units
                .iter()
                .all(|u| u.language == SourceLanguage::Terraform)
        );

        let main = unit(&units, "main.tf");
        assert!(matches!(
            main.facts[0].fact,
            StructuralFact::Misconfiguration {
                family: StructuralFamily::InfraPublicExposure,
                ..
            }
        ));
        assert_eq!(main.facts[0].line, 3);
        assert!(main.facts[0].grounding.is_some());

        let disks = unit(&units, "disks.tf");
        assert!(matches!(
            disks.facts[0].fact,
            StructuralFact::UnknownValue {
                family: StructuralFamily::InfraEncryption,
                ..
            }
        ));
    }

    #[test]
    fn secret_literals_reach_the_shared_secret_detector_and_are_never_echoed() {
        let units = units();
        for (path, name) in [
            ("variables.tf", "db_password"),
            ("providers.tf", "secret_key"),
        ] {
            let u = unit(&units, path);
            let StructuralFact::StringBinding { name: bound, value } = &u.facts[0].fact else {
                panic!("expected a binding in {path}");
            };
            assert_eq!(bound, name);
            assert!(is_hardcoded_secret(bound, value));
            let StructuralClassification::Modeled(ast) =
                classify_structural(StructuralFamily::HardcodedSecret, u)
            else {
                panic!("secrets are modeled for terraform");
            };
            let constructs = &ast.units[0].constructs;
            assert_eq!(constructs.len(), 1);
            assert!(!constructs[0].detail.contains(value.as_str()));
        }
    }

    #[test]
    fn a_value_that_flows_in_is_not_a_committed_literal() {
        let mut project = project();
        let module = project.modules.get_mut("").unwrap();
        module.variables.get_mut("db_password").unwrap().default = None;
        let analysis = TfAnalysis::run(
            &project,
            "",
            Bindings::none().with("db_password", TfValue::Str("s3cr3t-Value-42".into())),
        );
        let units = observed_units(&analysis, &|_, _| None);
        assert!(
            units.iter().all(|u| u.path != "variables.tf"),
            "a bound input is not a literal in the source"
        );
    }
}

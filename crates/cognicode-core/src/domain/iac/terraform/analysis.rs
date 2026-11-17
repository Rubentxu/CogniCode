//! The analysis of one Terraform project: the graph, its evaluation under
//! some bindings, the risk checks, and what the analysis did not cover.
//!
//! [`TfAnalysis::rebind`] answers "what if this variable changes?" without
//! starting over: only the nodes that depend on a changed variable are
//! re-evaluated, and only the resources that own such a node are re-assessed;
//! every other projection is carried over untouched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::eval::{Bindings, Evaluation};
use super::graph::{Def, FlowEdge, NodeAddr, NodeKind, TfGraph, UnresolvedModule};
use super::ir::{ResourceMode, TfProject, UnreadSource};
use super::rules::{self, CheckResult};
use super::value::{TfValue, UnknownCause};

/// What an analysis could not look at. Anything listed here is "not
/// evaluated", never "clean".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Coverage {
    /// Resource and data source instances in the graph.
    pub resources: usize,
    /// Those a check looked at.
    pub assessed: usize,
    /// Instances a known `count = 0` or empty `for_each` removes.
    pub absent: Vec<String>,
    /// Managed resource types no check models.
    pub not_evaluated_types: BTreeSet<String>,
    /// Module calls the analysis could not open.
    pub unresolved_modules: Vec<UnresolvedModule>,
    /// Sources of the project that were not read.
    pub unread: Vec<UnreadSource>,
}

impl Coverage {
    /// Whether every resource was either assessed or known not to exist, and
    /// every module was opened.
    pub fn is_complete(&self) -> bool {
        self.not_evaluated_types.is_empty()
            && self.unresolved_modules.is_empty()
            && self.unread.is_empty()
    }
}

/// How a rebind reused the previous analysis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RebindReport {
    /// Nodes whose value was recomputed.
    pub recomputed: BTreeSet<NodeAddr>,
    /// Resources that were re-assessed because one of their nodes changed.
    pub reassessed: BTreeSet<NodeAddr>,
    /// Resources whose previous results were carried over.
    pub reused: BTreeSet<NodeAddr>,
}

/// Why a node has the value it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    /// The node.
    pub address: NodeAddr,
    /// Its evaluated value.
    pub value: TfValue,
    /// The reads below it: every edge from it down to the variables, locals,
    /// outputs and resources it ultimately depends on.
    pub flow: Vec<FlowEdge>,
    /// When the value is unknown, why.
    pub causes: BTreeSet<UnknownCause>,
}

/// The result of analysing a project.
#[derive(Debug, Clone)]
pub struct TfAnalysis {
    graph: Arc<TfGraph>,
    bindings: Bindings,
    evaluation: Evaluation,
    checks: BTreeMap<NodeAddr, Vec<CheckResult>>,
    absent: BTreeSet<NodeAddr>,
    unread: Arc<Vec<UnreadSource>>,
}

impl TfAnalysis {
    /// Analyses the module directory `root` of `project` under `bindings`.
    pub fn run(project: &TfProject, root: &str, bindings: Bindings) -> Self {
        let graph = Arc::new(TfGraph::build(project, root));
        let evaluation = Evaluation::compute(&graph, &bindings);
        let mut analysis = Self {
            graph,
            bindings,
            evaluation,
            checks: BTreeMap::new(),
            absent: BTreeSet::new(),
            unread: Arc::new(project.unread.clone()),
        };
        let every: BTreeSet<NodeAddr> = analysis
            .graph
            .resources()
            .iter()
            .map(|r| r.addr.clone())
            .collect();
        analysis.assess(&every);
        analysis
    }

    /// The analysis under new bindings, plus what was reused.
    #[must_use]
    pub fn rebind(&self, bindings: Bindings) -> (Self, RebindReport) {
        let (evaluation, recomputed) = self.evaluation.rebind(&self.graph, &bindings);
        let mut next = Self {
            graph: Arc::clone(&self.graph),
            bindings,
            evaluation,
            checks: self.checks.clone(),
            absent: self.absent.clone(),
            unread: Arc::clone(&self.unread),
        };
        let reassessed: BTreeSet<NodeAddr> = self
            .graph
            .resources()
            .iter()
            .filter(|r| self.owns_recomputed(&r.addr, &recomputed))
            .map(|r| r.addr.clone())
            .collect();
        next.assess(&reassessed);
        let reused = self
            .graph
            .resources()
            .iter()
            .map(|r| r.addr.clone())
            .filter(|addr| !reassessed.contains(addr))
            .collect();
        (
            next,
            RebindReport {
                recomputed,
                reassessed,
                reused,
            },
        )
    }

    fn owns_recomputed(&self, resource: &NodeAddr, recomputed: &BTreeSet<NodeAddr>) -> bool {
        recomputed.contains(resource)
            || match self.graph.node(resource).map(|n| &n.def) {
                Some(Def::Aggregate { members, .. }) => {
                    members.iter().any(|m| recomputed.contains(m))
                }
                _ => false,
            }
    }

    fn assess(&mut self, resources: &BTreeSet<NodeAddr>) {
        for instance in self.graph.resources() {
            if !resources.contains(&instance.addr) {
                continue;
            }
            if rules::is_absent(instance, &self.evaluation) {
                self.absent.insert(instance.addr.clone());
            } else {
                self.absent.remove(&instance.addr);
            }
            let results = rules::assess(instance, &self.graph, &self.evaluation);
            self.checks.insert(instance.addr.clone(), results);
        }
    }

    /// The graph.
    pub fn graph(&self) -> &TfGraph {
        &self.graph
    }

    /// The bindings this analysis was run under.
    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// The evaluated value of a node.
    pub fn value(&self, addr: &NodeAddr) -> Option<&TfValue> {
        self.evaluation.get(addr)
    }

    /// Every check result, in resource address order.
    pub fn results(&self) -> impl Iterator<Item = &CheckResult> {
        self.checks.values().flatten()
    }

    /// The check results of one resource instance.
    pub fn results_of(&self, resource: &NodeAddr) -> &[CheckResult] {
        self.checks.get(resource).map_or(&[], Vec::as_slice)
    }

    /// Why `addr` has its value.
    pub fn explain(&self, addr: &NodeAddr) -> Option<Explanation> {
        self.graph.node(addr)?;
        let value = self.evaluation.get(addr)?.clone();
        Some(Explanation {
            address: addr.clone(),
            causes: value.causes(),
            value,
            flow: self.graph.flow_from(addr),
        })
    }

    /// What the analysis did not cover.
    pub fn coverage(&self) -> Coverage {
        let mut coverage = Coverage {
            resources: self.graph.resources().len(),
            unresolved_modules: self.graph.unresolved_modules().to_vec(),
            unread: self.unread.to_vec(),
            ..Coverage::default()
        };
        for instance in self.graph.resources() {
            if self.absent.contains(&instance.addr) {
                coverage.absent.push(instance.addr.to_string());
                continue;
            }
            if rules::is_modeled(&instance.resource.kind) {
                coverage.assessed += 1;
            } else if instance.resource.mode == ResourceMode::Managed {
                coverage
                    .not_evaluated_types
                    .insert(instance.resource.kind.clone());
            }
        }
        coverage
    }

    /// Variables of the root module the bindings do not bind.
    pub fn unbound_variables(&self) -> Vec<NodeAddr> {
        self.graph
            .nodes()
            .filter(|n| {
                n.kind == NodeKind::Variable && matches!(n.def, Def::Variable { root: true, .. })
            })
            .filter(|n| {
                self.evaluation
                    .get(&n.addr)
                    .is_some_and(TfValue::is_unknown)
            })
            .map(|n| n.addr.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::ir::{
        Reference, Step, TfAttribute, TfBody, TfExpr, TfModule, TfModuleCall, TfResource,
        TfVariable,
    };
    use super::super::rules::Outcome;
    use super::*;

    fn var(name: &str) -> TfExpr {
        TfExpr::Ref(Reference {
            root: "var".into(),
            steps: vec![Step::Attr(name.into())],
            line: 1,
        })
    }

    fn resource(kind: &str, name: &str, attributes: Vec<(&str, TfExpr)>) -> TfResource {
        TfResource {
            mode: ResourceMode::Managed,
            kind: kind.into(),
            name: name.into(),
            body: TfBody {
                attributes: attributes
                    .into_iter()
                    .map(|(n, expr)| TfAttribute {
                        name: n.into(),
                        expr,
                        line: 4,
                    })
                    .collect(),
                blocks: vec![],
            },
            file: "main.tf".into(),
            line: 2,
        }
    }

    fn variable(name: &str) -> TfVariable {
        TfVariable {
            name: name.into(),
            default: None,
            sensitive: false,
            file: "variables.tf".into(),
            line: 1,
        }
    }

    fn project() -> TfProject {
        let mut root = TfModule::default();
        root.variables.insert("acl".into(), variable("acl"));
        root.variables.insert("size".into(), variable("size"));
        for r in [
            resource("aws_s3_bucket", "data", vec![("acl", var("acl"))]),
            resource(
                "aws_ebs_volume",
                "disk",
                vec![
                    ("encrypted", TfExpr::Literal(TfValue::Bool(false))),
                    ("size", var("size")),
                ],
            ),
            resource("aws_lambda_function", "fn", vec![]),
        ] {
            root.resources.insert(r.address(), r);
        }
        TfProject {
            modules: BTreeMap::from([(String::new(), root)]),
            ..Default::default()
        }
    }

    #[test]
    fn rebinding_reassesses_only_the_resources_that_depend_on_the_change() {
        let before = TfAnalysis::run(
            &project(),
            "",
            Bindings::none().with("acl", TfValue::Str("private".into())),
        );
        let (after, report) =
            before.rebind(Bindings::none().with("acl", TfValue::Str("public-read".into())));

        assert_eq!(
            report.reassessed,
            BTreeSet::from([NodeAddr::new("aws_s3_bucket.data")])
        );
        assert!(
            report
                .reused
                .contains(&NodeAddr::new("aws_ebs_volume.disk"))
        );
        assert!(
            report
                .reused
                .contains(&NodeAddr::new("aws_lambda_function.fn"))
        );
        assert_eq!(
            after.results_of(&NodeAddr::new("aws_ebs_volume.disk")),
            before.results_of(&NodeAddr::new("aws_ebs_volume.disk")),
            "an unrelated projection is kept as is"
        );
        let bucket = after.results_of(&NodeAddr::new("aws_s3_bucket.data"));
        assert!(matches!(bucket[0].outcome, Outcome::Violation { .. }));
        let fresh = TfAnalysis::run(&project(), "", after.bindings().clone());
        let all = |a: &TfAnalysis| a.results().cloned().collect::<Vec<_>>();
        assert_eq!(all(&after), all(&fresh), "incremental equals from scratch");
    }

    #[test]
    fn binding_a_variable_reassesses_only_the_resources_that_read_it() {
        let base = TfAnalysis::run(&project(), "", Bindings::none());
        let (_, report) = base.rebind(Bindings::none().with("size", TfValue::Number("10".into())));
        // `size` is read by an attribute no check looks at, yet the resource
        // that owns the node is still the only one re-assessed.
        assert_eq!(
            report.reassessed,
            BTreeSet::from([NodeAddr::new("aws_ebs_volume.disk")])
        );
        assert!(report.reused.contains(&NodeAddr::new("aws_s3_bucket.data")));
    }

    #[test]
    fn coverage_lists_what_was_not_evaluated_instead_of_calling_it_clean() {
        let analysis = TfAnalysis::run(&project(), "", Bindings::none());
        let coverage = analysis.coverage();
        assert_eq!(coverage.resources, 3);
        assert_eq!(coverage.assessed, 2);
        assert_eq!(
            coverage.not_evaluated_types,
            BTreeSet::from(["aws_lambda_function".to_string()])
        );
        assert!(!coverage.is_complete());
        assert_eq!(analysis.unbound_variables().len(), 2);
    }

    #[test]
    fn an_unopened_module_is_reported_in_coverage() {
        let mut root = TfModule::default();
        root.calls.insert(
            "net".into(),
            TfModuleCall {
                name: "net".into(),
                source: "terraform-aws-modules/vpc/aws".into(),
                args: vec![],
                file: "main.tf".into(),
                line: 1,
            },
        );
        let project = TfProject {
            modules: BTreeMap::from([(String::new(), root)]),
            ..Default::default()
        };
        let coverage = TfAnalysis::run(&project, "", Bindings::none()).coverage();
        assert_eq!(coverage.unresolved_modules.len(), 1);
        assert!(!coverage.is_complete());
    }

    #[test]
    fn explain_gives_the_value_the_flow_and_the_causes() {
        let analysis = TfAnalysis::run(&project(), "", Bindings::none());
        let explanation = analysis
            .explain(&NodeAddr::new("aws_s3_bucket.data#acl"))
            .expect("the attribute exists");
        assert!(explanation.value.is_unknown());
        assert_eq!(explanation.flow.len(), 1);
        assert_eq!(explanation.flow[0].to, NodeAddr::new("var.acl"));
        assert_eq!(explanation.causes.len(), 1);
        assert!(analysis.explain(&NodeAddr::new("nope")).is_none());
    }
}

//! Static evaluation over the graph, and its incremental update.
//!
//! The evaluator claims a value only when the configuration itself fixes it:
//! literals, templates whose parts are all known, conditionals with a known
//! condition, and the flow of values through variables, locals, outputs and
//! module arguments. Everything else (provider-computed attributes, functions
//! other than `jsonencode`, operators, `for` expressions, runtime bindings)
//! is [`TfValue::Unknown`] with the cause recorded. Nothing is guessed, and
//! nothing is executed.
//!
//! A root variable is known only when an input binds it. A declared default
//! is a hint in the unknown's cause, unless the caller opts into
//! [`Bindings::apply_defaults`], which states that the defaults are the
//! inputs being analysed.

use std::collections::{BTreeMap, BTreeSet};

use super::graph::{Def, NodeAddr, NodeKind, Target, TfGraph};
use super::ir::{Reference, Step, TfExpr};
use super::value::{TfValue, UnknownCause};

/// The inputs of the root module.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bindings {
    values: BTreeMap<String, TfValue>,
    apply_defaults: bool,
}

impl Bindings {
    /// No input bound.
    pub fn none() -> Self {
        Self::default()
    }

    /// Binds a root variable.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: TfValue) -> Self {
        self.values.insert(name.into(), value);
        self
    }

    /// Treats each declared default as the input of its variable. A variable
    /// that no input binds and that has no default stays unknown.
    #[must_use]
    pub fn apply_defaults(mut self) -> Self {
        self.apply_defaults = true;
        self
    }

    /// The bound variables.
    pub fn values(&self) -> &BTreeMap<String, TfValue> {
        &self.values
    }
}

/// The value of an expression that is a constant by its own text: literals,
/// and lists and objects of them. `None` for anything that refers to
/// something, which is what an input file may not do.
pub fn constant(expr: &TfExpr) -> Option<TfValue> {
    match expr {
        TfExpr::Literal(value) => Some(value.clone()),
        TfExpr::List(items) => items
            .iter()
            .map(constant)
            .collect::<Option<Vec<_>>>()
            .map(TfValue::List),
        TfExpr::Object(entries) => {
            let mut map = BTreeMap::new();
            for (key, value) in entries {
                let TfValue::Str(key) = constant(key)? else {
                    return None;
                };
                map.insert(key, constant(value)?);
            }
            Some(TfValue::Object(map))
        }
        _ => None,
    }
}

/// The value of every node of a graph under some bindings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Evaluation {
    values: BTreeMap<NodeAddr, TfValue>,
}

impl Evaluation {
    /// Evaluates every node.
    pub fn compute(graph: &TfGraph, bindings: &Bindings) -> Self {
        let mut evaluator = Evaluator::new(graph, bindings, BTreeMap::new());
        for node in graph.nodes() {
            evaluator.value_of(&node.addr);
        }
        Self {
            values: evaluator.values,
        }
    }

    /// The evaluation under new bindings, recomputing only what depends on a
    /// root variable whose value changed. Also returns the nodes recomputed,
    /// so a caller can tell what was reused.
    pub fn rebind(&self, graph: &TfGraph, bindings: &Bindings) -> (Self, BTreeSet<NodeAddr>) {
        let changed: Vec<NodeAddr> = graph
            .nodes()
            .filter(|n| {
                n.kind == NodeKind::Variable && matches!(n.def, Def::Variable { root: true, .. })
            })
            .filter(|n| {
                let fresh = Evaluator::new(graph, bindings, BTreeMap::new()).value_of(&n.addr);
                self.values.get(&n.addr) != Some(&fresh)
            })
            .map(|n| n.addr.clone())
            .collect();
        let invalid = graph.invalidated(&changed);
        let kept: BTreeMap<NodeAddr, TfValue> = self
            .values
            .iter()
            .filter(|(addr, _)| !invalid.contains(*addr))
            .map(|(a, v)| (a.clone(), v.clone()))
            .collect();
        let mut evaluator = Evaluator::new(graph, bindings, kept);
        for addr in &invalid {
            evaluator.value_of(addr);
        }
        let recomputed = evaluator.computed.clone();
        (
            Self {
                values: evaluator.values,
            },
            recomputed,
        )
    }

    /// The value of a node.
    pub fn get(&self, addr: &NodeAddr) -> Option<&TfValue> {
        self.values.get(addr)
    }
}

struct Evaluator<'g> {
    graph: &'g TfGraph,
    bindings: &'g Bindings,
    values: BTreeMap<NodeAddr, TfValue>,
    in_progress: BTreeSet<NodeAddr>,
    /// The nodes this evaluator computed, as opposed to found in its cache.
    computed: BTreeSet<NodeAddr>,
}

impl<'g> Evaluator<'g> {
    fn new(graph: &'g TfGraph, bindings: &'g Bindings, cache: BTreeMap<NodeAddr, TfValue>) -> Self {
        Self {
            graph,
            bindings,
            values: cache,
            in_progress: BTreeSet::new(),
            computed: BTreeSet::new(),
        }
    }

    fn value_of(&mut self, addr: &NodeAddr) -> TfValue {
        if let Some(known) = self.values.get(addr) {
            return known.clone();
        }
        if !self.in_progress.insert(addr.clone()) {
            return TfValue::unknown(UnknownCause::ReferenceCycle {
                address: addr.as_str().to_string(),
            });
        }
        let value = self.compute(addr);
        self.in_progress.remove(addr);
        self.values.insert(addr.clone(), value.clone());
        self.computed.insert(addr.clone());
        value
    }

    fn compute(&mut self, addr: &NodeAddr) -> TfValue {
        let Some(node) = self.graph.node(addr) else {
            return TfValue::unknown(UnknownCause::UnsupportedExpression);
        };
        let scope = node.scope.clone();
        match node.def.clone() {
            Def::Expr(expr) => self.eval(&scope, &expr),
            Def::Absent => TfValue::unknown(UnknownCause::UnsupportedExpression),
            Def::Variable {
                name,
                root,
                supplied,
                default,
            } => self.variable(addr, &scope, &name, root, supplied, default),
            Def::Aggregate { .. } => TfValue::unknown(UnknownCause::ComputedAttribute {
                address: addr.as_str().to_string(),
                attribute: None,
            }),
            Def::Hole { cause, .. } => TfValue::unknown(cause),
        }
    }

    fn variable(
        &mut self,
        addr: &NodeAddr,
        scope: &str,
        name: &str,
        root: bool,
        supplied: Option<(TfExpr, String)>,
        default: Option<TfExpr>,
    ) -> TfValue {
        if root {
            if let Some(bound) = self.bindings.values.get(name) {
                return bound.clone();
            }
            return match default {
                Some(expr) if self.bindings.apply_defaults => self.eval(scope, &expr),
                other => TfValue::unknown(UnknownCause::UnboundVariable {
                    address: addr.as_str().to_string(),
                    default_available: other.is_some(),
                }),
            };
        }
        match (supplied, default) {
            (Some((expr, parent)), _) => self.eval(&parent, &expr),
            (None, Some(expr)) => self.eval(scope, &expr),
            (None, None) => TfValue::unknown(UnknownCause::UnboundVariable {
                address: addr.as_str().to_string(),
                default_available: false,
            }),
        }
    }

    fn eval(&mut self, scope: &str, expr: &TfExpr) -> TfValue {
        match expr {
            TfExpr::Literal(value) => value.clone(),
            TfExpr::Ref(reference) => self.reference(scope, reference),
            TfExpr::Template(parts) => self.template(scope, parts),
            TfExpr::Conditional {
                condition,
                then,
                otherwise,
            } => match self.eval(scope, condition) {
                TfValue::Bool(true) => self.eval(scope, then),
                TfValue::Bool(false) => self.eval(scope, otherwise),
                other => unknown_or_unsupported(&other),
            },
            TfExpr::List(items) => {
                TfValue::List(items.iter().map(|i| self.eval(scope, i)).collect())
            }
            TfExpr::Object(entries) => self.object(scope, entries),
            TfExpr::Call { function, args } => self.call(scope, function, args),
            TfExpr::Opaque { cause, refs } => {
                let mut causes = BTreeSet::from([cause.clone()]);
                for reference in refs {
                    causes.extend(self.reference(scope, reference).causes());
                }
                TfValue::unknown_from(causes)
            }
        }
    }

    fn template(&mut self, scope: &str, parts: &[TfExpr]) -> TfValue {
        let mut text = String::new();
        let mut causes = BTreeSet::new();
        for part in parts {
            let value = self.eval(scope, part);
            if !value.is_known() {
                causes.extend(value.causes());
            } else if let Some(piece) = value.template_text().or_else(|| value.to_json_text()) {
                text.push_str(&piece);
            } else {
                causes.insert(UnknownCause::UnsupportedExpression);
            }
        }
        if causes.is_empty() {
            TfValue::Str(text)
        } else {
            TfValue::unknown_from(causes)
        }
    }

    fn object(&mut self, scope: &str, entries: &[(TfExpr, TfExpr)]) -> TfValue {
        let mut map = BTreeMap::new();
        let mut causes = BTreeSet::new();
        for (key, value) in entries {
            match self.eval(scope, key).template_text() {
                Some(key) => {
                    map.insert(key, self.eval(scope, value));
                }
                None => {
                    causes.insert(UnknownCause::UnsupportedExpression);
                    causes.extend(self.eval(scope, key).causes());
                }
            }
        }
        if causes.is_empty() {
            TfValue::Object(map)
        } else {
            TfValue::unknown_from(causes)
        }
    }

    fn call(&mut self, scope: &str, function: &str, args: &[TfExpr]) -> TfValue {
        let values: Vec<TfValue> = args.iter().map(|a| self.eval(scope, a)).collect();
        if function == "jsonencode" && values.len() == 1 {
            return TfValue::Json(Box::new(values.into_iter().next().unwrap_or(TfValue::Null)));
        }
        let mut causes = BTreeSet::from([UnknownCause::FunctionCall {
            name: function.to_string(),
        }]);
        for value in &values {
            causes.extend(value.causes());
        }
        TfValue::unknown_from(causes)
    }

    fn reference(&mut self, scope: &str, reference: &Reference) -> TfValue {
        match self.graph.resolve(scope, reference) {
            Target::Runtime(name) => TfValue::unknown(UnknownCause::RuntimeBinding { name }),
            Target::Undeclared(text) => {
                TfValue::unknown(UnknownCause::UndeclaredReference { reference: text })
            }
            Target::Expanded(call) => TfValue::unknown(UnknownCause::ExpandedInstances {
                address: call.as_str().to_string(),
            }),
            Target::Node { addr, consumed } => {
                let kind = self.graph.node(&addr).map(|n| n.kind);
                match kind {
                    Some(NodeKind::Resource) => {
                        let attribute = reference.steps.get(consumed).and_then(|s| match s {
                            Step::Attr(name) => Some(name.clone()),
                            Step::Index(_) => None,
                        });
                        TfValue::unknown(UnknownCause::ComputedAttribute {
                            address: addr.as_str().to_string(),
                            attribute,
                        })
                    }
                    _ => {
                        let base = self.value_of(&addr);
                        self.apply_steps(
                            scope,
                            base,
                            &reference.steps[consumed.min(reference.steps.len())..],
                        )
                    }
                }
            }
        }
    }

    fn apply_steps(&mut self, scope: &str, base: TfValue, steps: &[Step]) -> TfValue {
        let mut current = base;
        for step in steps {
            current = match (&current, step) {
                (TfValue::Unknown(_), _) => return current,
                (TfValue::Object(map), Step::Attr(name)) => {
                    map.get(name).cloned().unwrap_or_else(|| {
                        TfValue::unknown(UnknownCause::UnresolvedAccess {
                            access: format!("attribute `{name}`"),
                        })
                    })
                }
                (_, Step::Attr(name)) => TfValue::unknown(UnknownCause::UnresolvedAccess {
                    access: format!("attribute `{name}`"),
                }),
                (_, Step::Index(index)) => {
                    let index = self.eval(scope, index);
                    index_into(&current, &index)
                }
            };
        }
        current
    }
}

fn index_into(base: &TfValue, index: &TfValue) -> TfValue {
    match (base, index) {
        (_, TfValue::Unknown(causes)) => TfValue::Unknown(causes.clone()),
        (TfValue::List(items), TfValue::Number(_)) => index
            .as_i64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| items.get(i))
            .cloned()
            .unwrap_or_else(|| unresolved_index(index)),
        (TfValue::Object(map), TfValue::Str(key)) => map
            .get(key)
            .cloned()
            .unwrap_or_else(|| unresolved_index(index)),
        _ => unresolved_index(index),
    }
}

fn unresolved_index(index: &TfValue) -> TfValue {
    TfValue::unknown(UnknownCause::UnresolvedAccess {
        access: format!("index {}", index.to_json_text().unwrap_or_default()),
    })
}

/// A condition or operand that is not what the construct needs: unknown stays
/// unknown with its own causes; a known value of the wrong type is a construct
/// the evaluator does not model.
fn unknown_or_unsupported(value: &TfValue) -> TfValue {
    let causes = value.causes();
    if causes.is_empty() {
        TfValue::unknown(UnknownCause::UnsupportedExpression)
    } else {
        TfValue::unknown_from(causes)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ir::{
        ResourceMode, TfAttribute, TfBody, TfLocal, TfModule, TfModuleCall, TfOutput, TfProject,
        TfResource, TfVariable,
    };
    use super::*;

    fn lit(text: &str) -> TfExpr {
        TfExpr::Literal(TfValue::Str(text.into()))
    }

    fn var(name: &str) -> TfExpr {
        TfExpr::Ref(Reference {
            root: "var".into(),
            steps: vec![Step::Attr(name.into())],
            line: 1,
        })
    }

    fn local(name: &str) -> TfExpr {
        TfExpr::Ref(Reference {
            root: "local".into(),
            steps: vec![Step::Attr(name.into())],
            line: 1,
        })
    }

    fn variable(name: &str, default: Option<TfExpr>) -> TfVariable {
        TfVariable {
            name: name.into(),
            default,
            sensitive: false,
            file: "variables.tf".into(),
            line: 1,
        }
    }

    fn a(name: &str, expr: TfExpr) -> TfAttribute {
        TfAttribute {
            name: name.into(),
            expr,
            line: 1,
        }
    }

    fn root_with(locals: Vec<(&str, TfExpr)>, vars: Vec<TfVariable>) -> TfProject {
        let mut m = TfModule::default();
        for v in vars {
            m.variables.insert(v.name.clone(), v);
        }
        for (name, expr) in locals {
            m.locals.insert(
                name.into(),
                TfLocal {
                    name: name.into(),
                    expr,
                    file: "main.tf".into(),
                    line: 1,
                },
            );
        }
        TfProject {
            modules: BTreeMap::from([(String::new(), m)]),
            ..Default::default()
        }
    }

    fn eval_local(project: &TfProject, bindings: &Bindings, name: &str) -> TfValue {
        let graph = TfGraph::build(project, "");
        Evaluation::compute(&graph, bindings)
            .get(&NodeAddr::new(format!("local.{name}")))
            .cloned()
            .expect("the local exists")
    }

    #[test]
    fn an_unbound_root_variable_is_unknown_and_its_default_is_only_a_hint() {
        let project = root_with(
            vec![("x", var("env"))],
            vec![variable("env", Some(lit("prod")))],
        );
        let value = eval_local(&project, &Bindings::none(), "x");
        assert_eq!(
            value,
            TfValue::unknown(UnknownCause::UnboundVariable {
                address: "var.env".into(),
                default_available: true
            })
        );
    }

    #[test]
    fn a_bound_variable_flows_through_locals_and_templates() {
        let project = root_with(
            vec![
                ("env", var("env")),
                (
                    "name",
                    TfExpr::Template(vec![lit("app-"), local("env"), lit("-bucket")]),
                ),
            ],
            vec![variable("env", None)],
        );
        let value = eval_local(
            &project,
            &Bindings::none().with("env", TfValue::Str("prod".into())),
            "name",
        );
        assert_eq!(value, TfValue::Str("app-prod-bucket".into()));
    }

    #[test]
    fn applying_defaults_is_an_explicit_choice_and_does_not_invent_missing_ones() {
        let project = root_with(
            vec![("a", var("with_default")), ("b", var("without"))],
            vec![
                variable("with_default", Some(lit("d"))),
                variable("without", None),
            ],
        );
        let bindings = Bindings::none().apply_defaults();
        assert_eq!(
            eval_local(&project, &bindings, "a"),
            TfValue::Str("d".into())
        );
        assert!(eval_local(&project, &bindings, "b").is_unknown());
    }

    #[test]
    fn a_conditional_follows_a_known_condition_and_is_unknown_otherwise() {
        let cond = |c: TfExpr| TfExpr::Conditional {
            condition: Box::new(c),
            then: Box::new(lit("yes")),
            otherwise: Box::new(lit("no")),
        };
        let project = root_with(vec![("x", cond(var("flag")))], vec![variable("flag", None)]);
        assert_eq!(
            eval_local(
                &project,
                &Bindings::none().with("flag", TfValue::Bool(false)),
                "x"
            ),
            TfValue::Str("no".into())
        );
        let unbound = eval_local(&project, &Bindings::none(), "x");
        assert!(unbound.is_unknown());
        assert_eq!(unbound.causes().len(), 1);
    }

    #[test]
    fn computed_attributes_functions_and_operators_stay_unknown_with_their_cause() {
        let computed = TfExpr::Ref(Reference {
            root: "aws_s3_bucket".into(),
            steps: vec![Step::Attr("b".into()), Step::Attr("arn".into())],
            line: 1,
        });
        let mut project = root_with(
            vec![
                ("arn", computed),
                (
                    "joined",
                    TfExpr::Call {
                        function: "join".into(),
                        args: vec![lit(","), TfExpr::List(vec![lit("a")])],
                    },
                ),
                (
                    "each",
                    TfExpr::Ref(Reference {
                        root: "each".into(),
                        steps: vec![Step::Attr("value".into())],
                        line: 1,
                    }),
                ),
                (
                    "missing",
                    TfExpr::Ref(Reference {
                        root: "var".into(),
                        steps: vec![Step::Attr("nope".into())],
                        line: 1,
                    }),
                ),
            ],
            vec![],
        );
        project.modules.get_mut("").unwrap().resources.insert(
            "aws_s3_bucket.b".into(),
            TfResource {
                mode: ResourceMode::Managed,
                kind: "aws_s3_bucket".into(),
                name: "b".into(),
                body: TfBody::default(),
                file: "main.tf".into(),
                line: 1,
            },
        );
        let b = Bindings::none();
        assert_eq!(
            eval_local(&project, &b, "arn").causes(),
            BTreeSet::from([UnknownCause::ComputedAttribute {
                address: "aws_s3_bucket.b".into(),
                attribute: Some("arn".into())
            }])
        );
        assert_eq!(
            eval_local(&project, &b, "joined").causes(),
            BTreeSet::from([UnknownCause::FunctionCall {
                name: "join".into()
            }])
        );
        assert_eq!(
            eval_local(&project, &b, "each").causes(),
            BTreeSet::from([UnknownCause::RuntimeBinding {
                name: "each".into()
            }])
        );
        assert_eq!(
            eval_local(&project, &b, "missing").causes(),
            BTreeSet::from([UnknownCause::UndeclaredReference {
                reference: "var.nope".into()
            }])
        );
    }

    #[test]
    fn jsonencode_keeps_known_parts_and_marks_unknown_leaves() {
        let doc = TfExpr::Call {
            function: "jsonencode".into(),
            args: vec![TfExpr::Object(vec![
                (lit("Effect"), lit("Allow")),
                (lit("Resource"), var("target")),
            ])],
        };
        let project = root_with(vec![("p", doc)], vec![variable("target", None)]);
        let unbound = eval_local(&project, &Bindings::none(), "p");
        assert!(!unbound.is_known());
        let TfValue::Json(inner) = &unbound else {
            panic!("expected a document, got {unbound:?}");
        };
        let TfValue::Object(map) = &**inner else {
            panic!("expected an object")
        };
        assert_eq!(map["Effect"], TfValue::Str("Allow".into()));
        assert!(map["Resource"].is_unknown());

        let bound = eval_local(
            &project,
            &Bindings::none().with("target", TfValue::Str("*".into())),
            "p",
        );
        assert_eq!(
            bound.to_json_text().as_deref(),
            Some(r#"{"Effect":"Allow","Resource":"*"}"#)
        );
    }

    #[test]
    fn indexes_and_attributes_resolve_inside_known_values_only() {
        let rules = TfExpr::List(vec![TfExpr::Object(vec![(
            lit("port"),
            TfExpr::Literal(TfValue::Number("22".into())),
        )])]);
        let access = TfExpr::Ref(Reference {
            root: "local".into(),
            steps: vec![
                Step::Attr("rules".into()),
                Step::Index(Box::new(TfExpr::Literal(TfValue::Number("0".into())))),
                Step::Attr("port".into()),
            ],
            line: 1,
        });
        let out_of_range = TfExpr::Ref(Reference {
            root: "local".into(),
            steps: vec![
                Step::Attr("rules".into()),
                Step::Index(Box::new(TfExpr::Literal(TfValue::Number("5".into())))),
            ],
            line: 1,
        });
        let project = root_with(
            vec![("rules", rules), ("port", access), ("far", out_of_range)],
            vec![],
        );
        assert_eq!(
            eval_local(&project, &Bindings::none(), "port"),
            TfValue::Number("22".into())
        );
        assert!(eval_local(&project, &Bindings::none(), "far").is_unknown());
    }

    #[test]
    fn a_reference_cycle_is_unknown_not_a_hang() {
        let project = root_with(vec![("a", local("b")), ("b", local("a"))], vec![]);
        assert!(eval_local(&project, &Bindings::none(), "a").is_unknown());
    }

    #[test]
    fn module_arguments_and_outputs_carry_values_across_the_boundary() {
        let mut child = TfModule::default();
        child
            .variables
            .insert("size".into(), variable("size", Some(lit("small"))));
        child.outputs.insert(
            "out".into(),
            TfOutput {
                name: "out".into(),
                value: Some(var("size")),
                sensitive: false,
                file: "o.tf".into(),
                line: 1,
            },
        );
        let mut root = TfModule::default();
        root.variables.insert("sz".into(), variable("sz", None));
        let call = |args: Vec<TfAttribute>| TfModuleCall {
            name: "m".into(),
            source: "./m".into(),
            args,
            file: "main.tf".into(),
            line: 1,
        };
        root.calls
            .insert("m".into(), call(vec![a("size", var("sz"))]));
        root.locals.insert(
            "seen".into(),
            TfLocal {
                name: "seen".into(),
                expr: TfExpr::Ref(Reference {
                    root: "module".into(),
                    steps: vec![Step::Attr("m".into()), Step::Attr("out".into())],
                    line: 1,
                }),
                file: "main.tf".into(),
                line: 1,
            },
        );
        let mut project = TfProject {
            modules: BTreeMap::from([(String::new(), root), ("m".into(), child)]),
            ..Default::default()
        };
        let bound = Bindings::none().with("sz", TfValue::Str("large".into()));
        assert_eq!(
            eval_local(&project, &bound, "seen"),
            TfValue::Str("large".into())
        );
        assert!(eval_local(&project, &Bindings::none(), "seen").is_unknown());

        // An omitted argument falls back to the child's own default: exact.
        project
            .modules
            .get_mut("")
            .unwrap()
            .calls
            .insert("m".into(), call(vec![]));
        assert_eq!(
            eval_local(&project, &Bindings::none(), "seen"),
            TfValue::Str("small".into())
        );
    }

    #[test]
    fn rebinding_recomputes_only_what_depends_on_the_changed_variable() {
        let project = root_with(
            vec![
                ("from_a", var("a")),
                ("from_b", var("b")),
                ("chain", local("from_a")),
                ("constant", lit("fixed")),
            ],
            vec![variable("a", None), variable("b", None)],
        );
        let graph = TfGraph::build(&project, "");
        let before = Bindings::none()
            .with("a", TfValue::Str("1".into()))
            .with("b", TfValue::Str("x".into()));
        let base = Evaluation::compute(&graph, &before);

        let after = before.clone().with("a", TfValue::Str("2".into()));
        let (next, recomputed) = base.rebind(&graph, &after);

        let expected: BTreeSet<NodeAddr> = ["var.a", "local.from_a", "local.chain"]
            .into_iter()
            .map(NodeAddr::new)
            .collect();
        assert_eq!(
            recomputed, expected,
            "exactly the variable and its dependents"
        );
        assert_eq!(
            next,
            Evaluation::compute(&graph, &after),
            "incremental equals from scratch"
        );
        assert_eq!(
            next.get(&NodeAddr::new("local.chain")),
            Some(&TfValue::Str("2".into()))
        );
        assert_eq!(
            next.get(&NodeAddr::new("local.from_b")),
            base.get(&NodeAddr::new("local.from_b"))
        );
    }

    #[test]
    fn rebinding_to_the_same_inputs_recomputes_nothing() {
        let project = root_with(vec![("x", var("a"))], vec![variable("a", None)]);
        let graph = TfGraph::build(&project, "");
        let bindings = Bindings::none().with("a", TfValue::Bool(true));
        let base = Evaluation::compute(&graph, &bindings);
        let (next, recomputed) = base.rebind(&graph, &bindings);
        assert!(recomputed.is_empty());
        assert_eq!(next, base);
    }
}

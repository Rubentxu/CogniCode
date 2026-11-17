//! The dependency graph of a Terraform project.
//!
//! Every value-bearing thing becomes a node: a variable, a local, an output,
//! an attribute of a resource (nested blocks included), a provider attribute,
//! plus one aggregate node per resource and per module call. An edge says
//! "this node's expression reads that node". The graph is built from the IR
//! alone, following only local modules the project already contains, and each
//! instance of a module gets its own nodes, so `module.a.var.x` and
//! `module.b.var.x` are different facts.
//!
//! Addresses are Terraform-style strings: `var.region`,
//! `module.vpc.var.cidr`, `aws_s3_bucket.b#acl`,
//! `aws_security_group.sg#ingress[0].cidr_blocks`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use super::ir::{
    Reference, TfBlock, TfBody, TfExpr, TfModule, TfModuleCall, TfProject, TfResource,
    resolve_local_source,
};
use super::value::UnknownCause;

/// The most module instances the graph builds. A project that fans out past
/// it keeps the remaining calls as unresolved modules instead of growing
/// without bound.
pub const MAX_MODULE_INSTANCES: usize = 512;

/// The address of a node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeAddr(String);

impl NodeAddr {
    /// An address from its text.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeKind {
    /// An input variable.
    Variable,
    /// A local value.
    Local,
    /// An output.
    Output,
    /// An attribute of a resource, data source, module call or provider.
    Attribute,
    /// A resource or data source as a whole.
    Resource,
    /// A module call as a whole.
    ModuleCall,
}

/// How a node gets its value.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Def {
    /// The value of an expression, in the node's scope.
    Expr(TfExpr),
    /// An input variable.
    Variable {
        /// The declared name.
        name: String,
        /// Whether it belongs to the root module (bound from outside).
        root: bool,
        /// The argument a module call passes, and the scope it is written in.
        supplied: Option<(TfExpr, String)>,
        /// The declared default.
        default: Option<TfExpr>,
    },
    /// A resource, data source or loaded module call: computed by the
    /// provider or by the instantiated module, never by this analysis.
    Aggregate {
        /// The nodes it is made of.
        members: Vec<NodeAddr>,
        /// Whether `count` or `for_each` expands it into instances.
        expanded: bool,
    },
    /// A module the analysis cannot open.
    Hole {
        /// Why.
        cause: UnknownCause,
        /// The arguments passed to it, in the scope of the call.
        args: Vec<TfExpr>,
    },
    /// A literal that is not an expression: unreachable by reference.
    Absent,
}

/// A node of the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Its address.
    pub addr: NodeAddr,
    /// What it stands for.
    pub kind: NodeKind,
    /// The module instance prefix its expression is written in (`""` or
    /// `module.a.`).
    pub scope: String,
    /// The attribute or declaration name (`acl`, `region`).
    pub name: String,
    /// File that declares it.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    pub(super) def: Def,
    /// The nodes it reads.
    pub deps: BTreeSet<NodeAddr>,
}

/// A resource instance in the graph: the declaration plus the scope it lives
/// in.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceInstance {
    /// Address of the aggregate node.
    pub addr: NodeAddr,
    /// The module instance prefix.
    pub scope: String,
    /// The declaration.
    pub resource: TfResource,
}

/// A module the analysis could not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedModule {
    /// Address of the call.
    pub address: NodeAddr,
    /// The `source` as written.
    pub source: String,
}

/// An edge of the reference flow: `from` reads `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlowEdge {
    /// The reader.
    pub from: NodeAddr,
    /// The thing read.
    pub to: NodeAddr,
}

/// What a traversal denotes, in the scope it is written in.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Target {
    /// A node, plus how many leading steps named it.
    Node { addr: NodeAddr, consumed: usize },
    /// A module output read through an expanded call.
    Expanded(NodeAddr),
    /// A name Terraform binds at run time.
    Runtime(String),
    /// Nothing the configuration declares.
    Undeclared(String),
}

/// The graph.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TfGraph {
    nodes: BTreeMap<NodeAddr, Node>,
    dependents: BTreeMap<NodeAddr, BTreeSet<NodeAddr>>,
    resources: Vec<ResourceInstance>,
    unresolved: Vec<UnresolvedModule>,
}

impl TfGraph {
    /// Builds the graph of the module directory `root` of `project`. A root
    /// that the project does not contain yields an empty graph.
    pub fn build(project: &TfProject, root: &str) -> Self {
        let mut builder = Builder {
            project,
            graph: Self::default(),
            instances: 0,
        };
        if project.modules.contains_key(root) {
            builder.instantiate(root, "", None, &mut vec![root.to_string()]);
        }
        let mut graph = builder.graph;
        graph.link();
        graph
    }

    /// The node at `addr`.
    pub fn node(&self, addr: &NodeAddr) -> Option<&Node> {
        self.nodes.get(addr)
    }

    /// Every node, in address order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// The resource and data source instances.
    pub fn resources(&self) -> &[ResourceInstance] {
        &self.resources
    }

    /// The module calls the analysis could not open.
    pub fn unresolved_modules(&self) -> &[UnresolvedModule] {
        &self.unresolved
    }

    /// The nodes that read `addr`, directly.
    pub fn readers_of(&self, addr: &NodeAddr) -> impl Iterator<Item = &NodeAddr> {
        self.dependents.get(addr).into_iter().flatten()
    }

    /// `roots` plus everything that transitively reads them: exactly the nodes
    /// whose value may change when a root does.
    pub fn invalidated<'a>(
        &self,
        roots: impl IntoIterator<Item = &'a NodeAddr>,
    ) -> BTreeSet<NodeAddr> {
        let mut seen: BTreeSet<NodeAddr> = BTreeSet::new();
        let mut queue: VecDeque<NodeAddr> = roots.into_iter().cloned().collect();
        while let Some(addr) = queue.pop_front() {
            if !seen.insert(addr.clone()) {
                continue;
            }
            queue.extend(self.readers_of(&addr).cloned());
        }
        seen
    }

    /// The reference flow below `from`: every edge reachable through reads,
    /// in a stable order.
    pub fn flow_from(&self, from: &NodeAddr) -> Vec<FlowEdge> {
        let mut edges = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([from.clone()]);
        while let Some(addr) = queue.pop_front() {
            if !seen.insert(addr.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&addr) {
                for dep in &node.deps {
                    edges.insert(FlowEdge {
                        from: addr.clone(),
                        to: dep.clone(),
                    });
                    queue.push_back(dep.clone());
                }
            }
        }
        edges.into_iter().collect()
    }

    /// The shortest chain of reads from `from` down to `to`, both included.
    pub fn path(&self, from: &NodeAddr, to: &NodeAddr) -> Option<Vec<NodeAddr>> {
        let mut previous: BTreeMap<NodeAddr, NodeAddr> = BTreeMap::new();
        let mut seen = BTreeSet::from([from.clone()]);
        let mut queue = VecDeque::from([from.clone()]);
        while let Some(addr) = queue.pop_front() {
            if &addr == to {
                let mut chain = vec![addr.clone()];
                let mut cursor = &addr;
                while let Some(prev) = previous.get(cursor) {
                    chain.push(prev.clone());
                    cursor = prev;
                }
                chain.reverse();
                return Some(chain);
            }
            for dep in self.nodes.get(&addr).into_iter().flat_map(|n| &n.deps) {
                if seen.insert(dep.clone()) {
                    previous.insert(dep.clone(), addr.clone());
                    queue.push_back(dep.clone());
                }
            }
        }
        None
    }

    /// What a traversal written in `scope` denotes.
    pub(super) fn resolve(&self, scope: &str, reference: &Reference) -> Target {
        let attrs = reference.attr_prefix();
        let text = reference.text();
        let node_at = |suffix: String| {
            let addr = NodeAddr::new(format!("{scope}{suffix}"));
            self.nodes.contains_key(&addr).then_some(addr)
        };
        match reference.root.as_str() {
            "each" | "count" | "self" | "path" | "terraform" => {
                Target::Runtime(reference.root.clone())
            }
            "var" => attrs
                .first()
                .and_then(|name| node_at(format!("var.{name}")))
                .map_or(Target::Undeclared(text), |addr| Target::Node {
                    addr,
                    consumed: 1,
                }),
            "local" => attrs
                .first()
                .and_then(|name| node_at(format!("local.{name}")))
                .map_or(Target::Undeclared(text), |addr| Target::Node {
                    addr,
                    consumed: 1,
                }),
            "module" => self.resolve_module(scope, &attrs, text),
            "data" => match attrs.as_slice() {
                [kind, name, ..] => node_at(format!("data.{kind}.{name}")).map_or(
                    Target::Undeclared(text),
                    |addr| Target::Node { addr, consumed: 2 },
                ),
                _ => Target::Undeclared(text),
            },
            kind => attrs
                .first()
                .and_then(|name| node_at(format!("{kind}.{name}")))
                .map_or(Target::Undeclared(text), |addr| Target::Node {
                    addr,
                    consumed: 1,
                }),
        }
    }

    fn resolve_module(&self, scope: &str, attrs: &[&str], text: String) -> Target {
        let Some(name) = attrs.first() else {
            return Target::Undeclared(text);
        };
        let call = NodeAddr::new(format!("{scope}module.{name}"));
        let Some(call_node) = self.nodes.get(&call) else {
            return Target::Undeclared(text);
        };
        match (&call_node.def, attrs.get(1)) {
            (Def::Hole { .. }, _) | (_, None) => Target::Node {
                addr: call,
                consumed: 1,
            },
            (Def::Aggregate { expanded: true, .. }, Some(_)) => Target::Expanded(call),
            (_, Some(output)) => {
                let addr = NodeAddr::new(format!("{scope}module.{name}.output.{output}"));
                if self.nodes.contains_key(&addr) {
                    Target::Node { addr, consumed: 2 }
                } else {
                    Target::Undeclared(text)
                }
            }
        }
    }

    /// Computes the reads of every node, then the reverse index.
    fn link(&mut self) {
        let resolved: Vec<(NodeAddr, BTreeSet<NodeAddr>)> = self
            .nodes
            .values()
            .map(|node| (node.addr.clone(), self.deps_of(node)))
            .collect();
        for (addr, deps) in resolved {
            for dep in &deps {
                self.dependents
                    .entry(dep.clone())
                    .or_default()
                    .insert(addr.clone());
            }
            if let Some(node) = self.nodes.get_mut(&addr) {
                node.deps = deps;
            }
        }
    }

    fn deps_of(&self, node: &Node) -> BTreeSet<NodeAddr> {
        let in_scope = |scope: &str, expr: &TfExpr| -> BTreeSet<NodeAddr> {
            expr.references()
                .into_iter()
                .filter_map(|r| match self.resolve(scope, r) {
                    Target::Node { addr, .. } | Target::Expanded(addr) => Some(addr),
                    Target::Runtime(_) | Target::Undeclared(_) => None,
                })
                .collect()
        };
        match &node.def {
            Def::Expr(expr) => in_scope(&node.scope, expr),
            Def::Variable {
                supplied, default, ..
            } => match (supplied, default) {
                (Some((expr, parent)), _) => in_scope(parent, expr),
                (None, Some(expr)) => in_scope(&node.scope, expr),
                (None, None) => BTreeSet::new(),
            },
            Def::Aggregate { members, .. } => members.iter().cloned().collect(),
            Def::Hole { args, .. } => args.iter().flat_map(|a| in_scope(&node.scope, a)).collect(),
            Def::Absent => BTreeSet::new(),
        }
    }
}

struct Builder<'p> {
    project: &'p TfProject,
    graph: TfGraph,
    instances: usize,
}

/// The call that instantiates a module and the scope it is written in.
struct CallSite<'c> {
    call: &'c TfModuleCall,
    parent_scope: &'c str,
}

impl Builder<'_> {
    fn add(&mut self, node: Node) {
        self.graph.nodes.insert(node.addr.clone(), node);
    }

    fn instantiate(
        &mut self,
        key: &str,
        scope: &str,
        site: Option<CallSite<'_>>,
        stack: &mut Vec<String>,
    ) {
        self.instances += 1;
        let project = self.project;
        let Some(module) = project.modules.get(key) else {
            return;
        };
        self.declare_variables(module, scope, site.as_ref());
        for local in module.locals.values() {
            self.add(Node {
                addr: NodeAddr::new(format!("{scope}local.{}", local.name)),
                kind: NodeKind::Local,
                scope: scope.to_string(),
                name: local.name.clone(),
                file: local.file.clone(),
                line: local.line,
                def: Def::Expr(local.expr.clone()),
                deps: BTreeSet::new(),
            });
        }
        for output in module.outputs.values() {
            self.add(Node {
                addr: NodeAddr::new(format!("{scope}output.{}", output.name)),
                kind: NodeKind::Output,
                scope: scope.to_string(),
                name: output.name.clone(),
                file: output.file.clone(),
                line: output.line,
                def: output.value.clone().map_or(Def::Absent, Def::Expr),
                deps: BTreeSet::new(),
            });
        }
        for resource in module.resources.values() {
            self.declare_resource(resource, scope);
        }
        for (index, provider) in module.providers.iter().enumerate() {
            let base = format!("{scope}provider.{}[{index}]", provider.name);
            self.declare_body(
                &provider.body,
                &base,
                scope,
                &provider.file,
                &mut String::new(),
                &mut Vec::new(),
            );
        }
        for call in module.calls.values() {
            self.declare_call(key, scope, call, stack);
        }
    }

    fn declare_variables(&mut self, module: &TfModule, scope: &str, site: Option<&CallSite<'_>>) {
        for variable in module.variables.values() {
            let supplied = site.and_then(|s| {
                s.call
                    .args
                    .iter()
                    .find(|a| a.name == variable.name)
                    .map(|a| (a.expr.clone(), s.parent_scope.to_string()))
            });
            self.add(Node {
                addr: NodeAddr::new(format!("{scope}var.{}", variable.name)),
                kind: NodeKind::Variable,
                scope: scope.to_string(),
                name: variable.name.clone(),
                file: variable.file.clone(),
                line: variable.line,
                def: Def::Variable {
                    name: variable.name.clone(),
                    root: site.is_none(),
                    supplied,
                    default: variable.default.clone(),
                },
                deps: BTreeSet::new(),
            });
        }
    }

    fn declare_resource(&mut self, resource: &TfResource, scope: &str) {
        let addr = NodeAddr::new(format!("{scope}{}", resource.address()));
        let mut members = Vec::new();
        let base = addr.as_str().to_string();
        self.declare_body(
            &resource.body,
            &base,
            scope,
            &resource.file,
            &mut String::new(),
            &mut members,
        );
        let expanded = resource.body.attribute("count").is_some()
            || resource.body.attribute("for_each").is_some();
        self.add(Node {
            addr: addr.clone(),
            kind: NodeKind::Resource,
            scope: scope.to_string(),
            name: resource.name.clone(),
            file: resource.file.clone(),
            line: resource.line,
            def: Def::Aggregate { members, expanded },
            deps: BTreeSet::new(),
        });
        self.graph.resources.push(ResourceInstance {
            addr,
            scope: scope.to_string(),
            resource: resource.clone(),
        });
    }

    /// One node per attribute of the body, nested blocks flattened into
    /// `kind[i].attr` paths.
    fn declare_body(
        &mut self,
        body: &TfBody,
        base: &str,
        scope: &str,
        file: &str,
        path: &mut String,
        members: &mut Vec<NodeAddr>,
    ) {
        for attribute in &body.attributes {
            let addr = NodeAddr::new(format!("{base}#{path}{}", attribute.name));
            members.push(addr.clone());
            self.add(Node {
                addr,
                kind: NodeKind::Attribute,
                scope: scope.to_string(),
                name: attribute.name.clone(),
                file: file.to_string(),
                line: attribute.line,
                def: Def::Expr(attribute.expr.clone()),
                deps: BTreeSet::new(),
            });
        }
        let mut seen: BTreeMap<(String, bool), usize> = BTreeMap::new();
        for block in &body.blocks {
            let ordinal = seen
                .entry((block.kind.clone(), block.dynamic.is_some()))
                .or_insert(0);
            let segment = block_segment(block, *ordinal);
            *ordinal += 1;
            if let Some(for_each) = &block.dynamic {
                let addr = NodeAddr::new(format!("{base}#{path}{segment}.for_each"));
                members.push(addr.clone());
                self.add(Node {
                    addr,
                    kind: NodeKind::Attribute,
                    scope: scope.to_string(),
                    name: "for_each".to_string(),
                    file: file.to_string(),
                    line: block.line,
                    def: Def::Expr((**for_each).clone()),
                    deps: BTreeSet::new(),
                });
            }
            let mark = path.len();
            path.push_str(&segment);
            path.push('.');
            self.declare_body(&block.body, base, scope, file, path, members);
            path.truncate(mark);
        }
    }

    fn declare_call(
        &mut self,
        key: &str,
        scope: &str,
        call: &TfModuleCall,
        stack: &mut Vec<String>,
    ) {
        let addr = NodeAddr::new(format!("{scope}module.{}", call.name));
        let target = resolve_local_source(key, &call.source)
            .filter(|k| self.project.modules.contains_key(k))
            .filter(|k| !stack.contains(k))
            .filter(|_| self.instances < MAX_MODULE_INSTANCES);
        let expanded = call
            .args
            .iter()
            .any(|a| a.name == "count" || a.name == "for_each");
        let meta: Vec<NodeAddr> = call
            .args
            .iter()
            .filter(|a| a.name == "count" || a.name == "for_each")
            .map(|a| {
                let meta_addr = NodeAddr::new(format!("{addr}#{}", a.name));
                self.add(Node {
                    addr: meta_addr.clone(),
                    kind: NodeKind::Attribute,
                    scope: scope.to_string(),
                    name: a.name.clone(),
                    file: call.file.clone(),
                    line: a.line,
                    def: Def::Expr(a.expr.clone()),
                    deps: BTreeSet::new(),
                });
                meta_addr
            })
            .collect();

        let def = match target {
            Some(child_key) => {
                let child_scope = format!("{addr}.");
                let before: BTreeSet<NodeAddr> = self.graph.nodes.keys().cloned().collect();
                stack.push(child_key.clone());
                self.instantiate(
                    &child_key,
                    &child_scope,
                    Some(CallSite {
                        call,
                        parent_scope: scope,
                    }),
                    stack,
                );
                stack.pop();
                let mut members: Vec<NodeAddr> = self
                    .graph
                    .nodes
                    .keys()
                    .filter(|a| !before.contains(*a) && a.as_str().starts_with(&child_scope))
                    .cloned()
                    .collect();
                members.extend(meta);
                Def::Aggregate { members, expanded }
            }
            None => {
                self.graph.unresolved.push(UnresolvedModule {
                    address: addr.clone(),
                    source: call.source.clone(),
                });
                Def::Hole {
                    cause: UnknownCause::UnresolvedModule {
                        address: addr.as_str().to_string(),
                        source: call.source.clone(),
                    },
                    args: call.args.iter().map(|a| a.expr.clone()).collect(),
                }
            }
        };
        self.add(Node {
            addr,
            kind: NodeKind::ModuleCall,
            scope: scope.to_string(),
            name: call.name.clone(),
            file: call.file.clone(),
            line: call.line,
            def,
            deps: BTreeSet::new(),
        });
    }
}

/// The path segment of a nested block: `ingress[0]` or `dynamic.ingress[0]`.
pub fn block_segment(block: &TfBlock, ordinal: usize) -> String {
    if block.dynamic.is_some() {
        format!("dynamic.{}[{ordinal}]", block.kind)
    } else {
        format!("{}[{ordinal}]", block.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ir::{ResourceMode, Step, TfAttribute, TfLocal, TfOutput, TfVariable};
    use super::super::value::TfValue;
    use super::*;

    fn reference(root: &str, attrs: &[&str]) -> TfExpr {
        TfExpr::Ref(Reference {
            root: root.into(),
            steps: attrs.iter().map(|a| Step::Attr((*a).into())).collect(),
            line: 1,
        })
    }

    fn attr(name: &str, expr: TfExpr) -> TfAttribute {
        TfAttribute {
            name: name.into(),
            expr,
            line: 1,
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

    fn bucket(acl: TfExpr) -> TfResource {
        TfResource {
            mode: ResourceMode::Managed,
            kind: "aws_s3_bucket".into(),
            name: "b".into(),
            body: TfBody {
                attributes: vec![attr("acl", acl)],
                blocks: vec![],
            },
            file: "main.tf".into(),
            line: 3,
        }
    }

    /// root: local.acl <- var.acl ; aws_s3_bucket.b#acl <- local.acl
    fn small_project() -> TfProject {
        let mut root = TfModule::default();
        root.variables.insert("acl".into(), variable("acl"));
        root.variables.insert("other".into(), variable("other"));
        root.locals.insert(
            "acl".into(),
            TfLocal {
                name: "acl".into(),
                expr: reference("var", &["acl"]),
                file: "main.tf".into(),
                line: 1,
            },
        );
        root.resources.insert(
            "aws_s3_bucket.b".into(),
            bucket(reference("local", &["acl"])),
        );
        root.outputs.insert(
            "name".into(),
            TfOutput {
                name: "name".into(),
                value: Some(reference("var", &["other"])),
                sensitive: false,
                file: "outputs.tf".into(),
                line: 1,
            },
        );
        TfProject {
            modules: BTreeMap::from([(String::new(), root)]),
            ..Default::default()
        }
    }

    #[test]
    fn attributes_read_the_nodes_their_expressions_name() {
        let graph = TfGraph::build(&small_project(), "");
        let acl = graph.node(&NodeAddr::new("aws_s3_bucket.b#acl")).unwrap();
        assert_eq!(acl.deps, BTreeSet::from([NodeAddr::new("local.acl")]));
        let local = graph.node(&NodeAddr::new("local.acl")).unwrap();
        assert_eq!(local.deps, BTreeSet::from([NodeAddr::new("var.acl")]));
        let bucket = graph.node(&NodeAddr::new("aws_s3_bucket.b")).unwrap();
        assert!(bucket.deps.contains(&NodeAddr::new("aws_s3_bucket.b#acl")));
    }

    #[test]
    fn invalidation_is_the_variable_and_what_transitively_reads_it() {
        let graph = TfGraph::build(&small_project(), "");
        let invalid = graph.invalidated([&NodeAddr::new("var.acl")]);
        let expected: BTreeSet<NodeAddr> = [
            "var.acl",
            "local.acl",
            "aws_s3_bucket.b#acl",
            "aws_s3_bucket.b",
        ]
        .into_iter()
        .map(NodeAddr::new)
        .collect();
        assert_eq!(invalid, expected);
        assert!(!invalid.contains(&NodeAddr::new("var.other")));
        assert!(!invalid.contains(&NodeAddr::new("output.name")));
    }

    #[test]
    fn the_flow_and_the_path_explain_how_a_variable_reaches_an_attribute() {
        let graph = TfGraph::build(&small_project(), "");
        let from = NodeAddr::new("aws_s3_bucket.b#acl");
        let to = NodeAddr::new("var.acl");
        assert_eq!(
            graph.path(&from, &to),
            Some(vec![from.clone(), NodeAddr::new("local.acl"), to.clone()])
        );
        assert_eq!(graph.path(&to, &from), None, "reads are directional");
        assert_eq!(graph.flow_from(&from).len(), 2);
    }

    fn module_project() -> TfProject {
        let mut child = TfModule::default();
        child.variables.insert("cidr".into(), variable("cidr"));
        child.outputs.insert(
            "cidr_out".into(),
            TfOutput {
                name: "cidr_out".into(),
                value: Some(reference("var", &["cidr"])),
                sensitive: false,
                file: "modules/net/outputs.tf".into(),
                line: 1,
            },
        );
        let mut root = TfModule::default();
        root.variables
            .insert("net_cidr".into(), variable("net_cidr"));
        root.calls.insert(
            "net".into(),
            TfModuleCall {
                name: "net".into(),
                source: "./modules/net".into(),
                args: vec![attr("cidr", reference("var", &["net_cidr"]))],
                file: "main.tf".into(),
                line: 1,
            },
        );
        root.resources.insert(
            "aws_vpc.v".into(),
            TfResource {
                mode: ResourceMode::Managed,
                kind: "aws_vpc".into(),
                name: "v".into(),
                body: TfBody {
                    attributes: vec![attr(
                        "cidr_block",
                        reference("module", &["net", "cidr_out"]),
                    )],
                    blocks: vec![],
                },
                file: "main.tf".into(),
                line: 5,
            },
        );
        TfProject {
            modules: BTreeMap::from([(String::new(), root), ("modules/net".into(), child)]),
            ..Default::default()
        }
    }

    #[test]
    fn a_local_module_gets_its_own_nodes_and_its_arguments_flow_in() {
        let graph = TfGraph::build(&module_project(), "");
        let var = graph.node(&NodeAddr::new("module.net.var.cidr")).unwrap();
        assert_eq!(var.deps, BTreeSet::from([NodeAddr::new("var.net_cidr")]));
        let attr = graph.node(&NodeAddr::new("aws_vpc.v#cidr_block")).unwrap();
        assert_eq!(
            attr.deps,
            BTreeSet::from([NodeAddr::new("module.net.output.cidr_out")])
        );
        let invalid = graph.invalidated([&NodeAddr::new("var.net_cidr")]);
        assert!(invalid.contains(&NodeAddr::new("aws_vpc.v#cidr_block")));
        assert!(graph.unresolved_modules().is_empty());
    }

    #[test]
    fn a_remote_or_missing_module_is_a_hole_that_keeps_its_inputs_visible() {
        let mut project = module_project();
        project.modules.remove("modules/net");
        let graph = TfGraph::build(&project, "");
        assert_eq!(graph.unresolved_modules().len(), 1);
        assert_eq!(graph.unresolved_modules()[0].source, "./modules/net");
        let hole = graph.node(&NodeAddr::new("module.net")).unwrap();
        assert_eq!(hole.deps, BTreeSet::from([NodeAddr::new("var.net_cidr")]));
    }

    #[test]
    fn a_module_that_calls_itself_stops_at_the_cycle() {
        let mut looped = TfModule::default();
        looped.calls.insert(
            "again".into(),
            TfModuleCall {
                name: "again".into(),
                source: "./".into(),
                args: vec![],
                file: "main.tf".into(),
                line: 1,
            },
        );
        let project = TfProject {
            modules: BTreeMap::from([(String::new(), looped)]),
            ..Default::default()
        };
        let graph = TfGraph::build(&project, "");
        assert_eq!(graph.unresolved_modules().len(), 1);
    }

    #[test]
    fn nested_blocks_become_indexed_attribute_nodes_and_dynamic_blocks_keep_their_for_each() {
        let mut resource = bucket(TfExpr::Literal(TfValue::Str("private".into())));
        resource.body.blocks = vec![
            TfBlock {
                kind: "ingress".into(),
                labels: vec![],
                dynamic: None,
                body: TfBody {
                    attributes: vec![attr("port", TfExpr::Literal(TfValue::Number("22".into())))],
                    blocks: vec![],
                },
                line: 5,
            },
            TfBlock {
                kind: "ingress".into(),
                labels: vec![],
                dynamic: Some(Box::new(reference("var", &["rules"]))),
                body: TfBody {
                    attributes: vec![attr("port", reference("each", &["value"]))],
                    blocks: vec![],
                },
                line: 9,
            },
        ];
        let mut root = TfModule::default();
        root.variables.insert("rules".into(), variable("rules"));
        root.resources.insert(resource.address(), resource);
        let graph = TfGraph::build(
            &TfProject {
                modules: BTreeMap::from([(String::new(), root)]),
                ..Default::default()
            },
            "",
        );
        assert!(
            graph
                .node(&NodeAddr::new("aws_s3_bucket.b#ingress[0].port"))
                .is_some()
        );
        let for_each = graph
            .node(&NodeAddr::new(
                "aws_s3_bucket.b#dynamic.ingress[0].for_each",
            ))
            .unwrap();
        assert_eq!(for_each.deps, BTreeSet::from([NodeAddr::new("var.rules")]));
        assert!(
            graph
                .node(&NodeAddr::new("aws_s3_bucket.b#dynamic.ingress[0].port"))
                .is_some()
        );
    }

    #[test]
    fn building_from_a_root_the_project_lacks_is_empty() {
        let graph = TfGraph::build(&TfProject::default(), "");
        assert_eq!(graph.nodes().count(), 0);
    }
}

//! The Terraform IR: what a configuration declares, as data.
//!
//! Resources, data sources, variables, locals, outputs, module calls and
//! providers, with every attribute kept as an expression ([`TfExpr`]) instead
//! of a value. Nothing here is evaluated; [`super::graph`] and
//! [`super::eval`] decide what the expressions mean, and they only claim a
//! value when the configuration itself determines it.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::value::{TfValue, UnknownCause};

/// One step of a traversal after its root: `.name` or `[expr]`.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// `.name`.
    Attr(String),
    /// `[expr]`.
    Index(Box<TfExpr>),
}

/// A traversal such as `aws_s3_bucket.b.id` or `var.rules[0].port`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The root name: `var`, `local`, `module`, `data`, `each`, a resource
    /// type, and so on.
    pub root: String,
    /// What follows the root.
    pub steps: Vec<Step>,
    /// 1-based source line.
    pub line: u32,
}

impl Reference {
    /// The leading attribute names, up to the first index.
    pub fn attr_prefix(&self) -> Vec<&str> {
        self.steps
            .iter()
            .map_while(|s| match s {
                Step::Attr(name) => Some(name.as_str()),
                Step::Index(_) => None,
            })
            .collect()
    }

    /// The traversal as written, with indexes elided to `[..]`.
    pub fn text(&self) -> String {
        let mut text = self.root.clone();
        for step in &self.steps {
            match step {
                Step::Attr(name) => {
                    text.push('.');
                    text.push_str(name);
                }
                Step::Index(_) => text.push_str("[..]"),
            }
        }
        text
    }
}

/// An expression, read but not evaluated.
#[derive(Debug, Clone, PartialEq)]
pub enum TfExpr {
    /// A literal written in the source: `"x"`, `true`, `3`, `null`. A literal
    /// is the only thing the secrets rule may call hardcoded.
    Literal(TfValue),
    /// A traversal.
    Ref(Reference),
    /// A string template; literal parts are `Literal(Str)`.
    Template(Vec<TfExpr>),
    /// `cond ? then : otherwise`.
    Conditional {
        /// The condition.
        condition: Box<TfExpr>,
        /// Taken when true.
        then: Box<TfExpr>,
        /// Taken when false.
        otherwise: Box<TfExpr>,
    },
    /// A tuple `[a, b]`.
    List(Vec<TfExpr>),
    /// An object `{ k = v }`.
    Object(Vec<(TfExpr, TfExpr)>),
    /// A function call.
    Call {
        /// Function name.
        function: String,
        /// Arguments.
        args: Vec<TfExpr>,
    },
    /// A construct the analysis does not evaluate. The references inside it
    /// stay visible, so the dependencies they create are still tracked.
    Opaque {
        /// Why it is not evaluated.
        cause: UnknownCause,
        /// The traversals it contains, outside names it binds itself.
        refs: Vec<Reference>,
    },
}

impl TfExpr {
    /// Every traversal in the expression, nested ones included.
    pub fn references(&self) -> Vec<&Reference> {
        let mut out = Vec::new();
        self.collect_references(&mut out);
        out
    }

    fn collect_references<'a>(&'a self, out: &mut Vec<&'a Reference>) {
        match self {
            Self::Literal(_) => {}
            Self::Ref(reference) => reference.collect_into(out),
            Self::Template(parts) | Self::List(parts) => {
                parts.iter().for_each(|p| p.collect_references(out));
            }
            Self::Conditional {
                condition,
                then,
                otherwise,
            } => {
                condition.collect_references(out);
                then.collect_references(out);
                otherwise.collect_references(out);
            }
            Self::Object(entries) => entries.iter().for_each(|(k, v)| {
                k.collect_references(out);
                v.collect_references(out);
            }),
            Self::Call { args, .. } => args.iter().for_each(|a| a.collect_references(out)),
            Self::Opaque { refs, .. } => refs.iter().for_each(|r| r.collect_into(out)),
        }
    }
}

impl Reference {
    fn collect_into<'a>(&'a self, out: &mut Vec<&'a Reference>) {
        out.push(self);
        for step in &self.steps {
            if let Step::Index(expr) = step {
                expr.collect_references(out);
            }
        }
    }
}

/// `name = expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfAttribute {
    /// Attribute name.
    pub name: String,
    /// Its expression.
    pub expr: TfExpr,
    /// 1-based source line.
    pub line: u32,
}

/// The body of a block: attributes and nested blocks, in source order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TfBody {
    /// Attributes.
    pub attributes: Vec<TfAttribute>,
    /// Nested blocks.
    pub blocks: Vec<TfBlock>,
}

impl TfBody {
    /// The attribute named `name`.
    pub fn attribute(&self, name: &str) -> Option<&TfAttribute> {
        self.attributes.iter().find(|a| a.name == name)
    }

    /// The nested blocks of `kind`, in source order.
    pub fn blocks_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a TfBlock> {
        self.blocks.iter().filter(move |b| b.kind == kind)
    }
}

/// A nested block, e.g. `ingress { .. }` or `dynamic "ingress" { .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfBlock {
    /// The block type. For a `dynamic` block, the name it generates.
    pub kind: String,
    /// Header labels.
    pub labels: Vec<String>,
    /// `Some(for_each)` for a `dynamic` block; its `content` is the body.
    pub dynamic: Option<Box<TfExpr>>,
    /// The body (the `content` of a `dynamic` block).
    pub body: TfBody,
    /// 1-based source line.
    pub line: u32,
}

/// Managed resource or data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceMode {
    /// `resource "type" "name"`.
    Managed,
    /// `data "type" "name"`.
    Data,
}

/// A resource or data source.
#[derive(Debug, Clone, PartialEq)]
pub struct TfResource {
    /// Managed or data.
    pub mode: ResourceMode,
    /// The type, e.g. `aws_s3_bucket`.
    pub kind: String,
    /// The local name.
    pub name: String,
    /// The body.
    pub body: TfBody,
    /// The file that declares it, relative to the project.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

impl TfResource {
    /// The in-module address: `aws_s3_bucket.b` or `data.aws_ami.u`.
    pub fn address(&self) -> String {
        match self.mode {
            ResourceMode::Managed => format!("{}.{}", self.kind, self.name),
            ResourceMode::Data => format!("data.{}.{}", self.kind, self.name),
        }
    }
}

/// `variable "name" { .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfVariable {
    /// Name.
    pub name: String,
    /// The default, when declared.
    pub default: Option<TfExpr>,
    /// Whether it is declared `sensitive = true`.
    pub sensitive: bool,
    /// File.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

/// One entry of a `locals` block.
#[derive(Debug, Clone, PartialEq)]
pub struct TfLocal {
    /// Name.
    pub name: String,
    /// Value expression.
    pub expr: TfExpr,
    /// File.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

/// `output "name" { value = .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfOutput {
    /// Name.
    pub name: String,
    /// The `value` expression, when present.
    pub value: Option<TfExpr>,
    /// Whether it is declared `sensitive = true`.
    pub sensitive: bool,
    /// File.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

/// `module "name" { source = .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfModuleCall {
    /// Name.
    pub name: String,
    /// The `source` as written; empty when the call has none.
    pub source: String,
    /// Input arguments, `count` and `for_each` included; `source`, `version`,
    /// `providers` and `depends_on` are not inputs.
    pub args: Vec<TfAttribute>,
    /// File.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

/// `provider "name" { .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct TfProvider {
    /// Provider name.
    pub name: String,
    /// Its configuration.
    pub body: TfBody,
    /// File.
    pub file: String,
    /// 1-based source line.
    pub line: u32,
}

/// The declarations of one module directory, merged over its files.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TfModule {
    /// Variables by name.
    pub variables: BTreeMap<String, TfVariable>,
    /// Locals by name.
    pub locals: BTreeMap<String, TfLocal>,
    /// Outputs by name.
    pub outputs: BTreeMap<String, TfOutput>,
    /// Resources and data sources by in-module address.
    pub resources: BTreeMap<String, TfResource>,
    /// Module calls by name.
    pub calls: BTreeMap<String, TfModuleCall>,
    /// Provider configurations.
    pub providers: Vec<TfProvider>,
    /// Declarations made a second time. Terraform rejects such a module, so
    /// the first declaration is the one kept and the repeats are listed here.
    pub duplicates: Vec<TfDuplicate>,
}

/// A declaration that repeats one the module already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TfDuplicate {
    /// What was declared: `resource`, `variable`, `local`, `output` or
    /// `module`.
    pub kind: &'static str,
    /// The name or in-module address.
    pub name: String,
    /// The file of the repeat.
    pub file: String,
    /// 1-based line of the repeat.
    pub line: u32,
}

/// Keeps the first declaration under `key`; a repeat is listed in
/// `duplicates` and dropped.
fn declare<V>(
    map: &mut BTreeMap<String, V>,
    duplicates: &mut Vec<TfDuplicate>,
    kind: &'static str,
    key: String,
    value: V,
    place: impl FnOnce(&V) -> (String, u32),
) {
    match map.entry(key) {
        Entry::Occupied(taken) => {
            let (file, line) = place(&value);
            duplicates.push(TfDuplicate {
                kind,
                name: taken.key().clone(),
                file,
                line,
            });
        }
        Entry::Vacant(free) => {
            free.insert(value);
        }
    }
}

impl TfModule {
    /// Declares a variable; a second one of the same name is a duplicate.
    pub fn declare_variable(&mut self, variable: TfVariable) {
        let place = |v: &TfVariable| (v.file.clone(), v.line);
        declare(
            &mut self.variables,
            &mut self.duplicates,
            "variable",
            variable.name.clone(),
            variable,
            place,
        );
    }

    /// Declares a local; a second one of the same name is a duplicate.
    pub fn declare_local(&mut self, local: TfLocal) {
        let place = |v: &TfLocal| (v.file.clone(), v.line);
        declare(
            &mut self.locals,
            &mut self.duplicates,
            "local",
            local.name.clone(),
            local,
            place,
        );
    }

    /// Declares an output; a second one of the same name is a duplicate.
    pub fn declare_output(&mut self, output: TfOutput) {
        let place = |v: &TfOutput| (v.file.clone(), v.line);
        declare(
            &mut self.outputs,
            &mut self.duplicates,
            "output",
            output.name.clone(),
            output,
            place,
        );
    }

    /// Declares a resource or data source; a second one at the same address
    /// is a duplicate.
    pub fn declare_resource(&mut self, resource: TfResource) {
        let place = |v: &TfResource| (v.file.clone(), v.line);
        declare(
            &mut self.resources,
            &mut self.duplicates,
            "resource",
            resource.address(),
            resource,
            place,
        );
    }

    /// Declares a module call; a second one of the same name is a duplicate.
    pub fn declare_call(&mut self, call: TfModuleCall) {
        let place = |v: &TfModuleCall| (v.file.clone(), v.line);
        declare(
            &mut self.calls,
            &mut self.duplicates,
            "module",
            call.name.clone(),
            call,
            place,
        );
    }

    /// Adds the declarations of another file of the same directory. What the
    /// directory already declares is kept, and a repeat is listed in
    /// [`TfModule::duplicates`].
    pub fn absorb(&mut self, other: TfModule) {
        other
            .variables
            .into_values()
            .for_each(|v| self.declare_variable(v));
        other
            .locals
            .into_values()
            .for_each(|v| self.declare_local(v));
        other
            .outputs
            .into_values()
            .for_each(|v| self.declare_output(v));
        other
            .resources
            .into_values()
            .for_each(|v| self.declare_resource(v));
        other.calls.into_values().for_each(|v| self.declare_call(v));
        self.providers.extend(other.providers);
        self.duplicates.extend(other.duplicates);
    }

    /// Whether the file declared nothing.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// The directory a local module `source` names, relative to the project root.
///
/// Purely lexical: nothing is read. `None` for a source that is not a local
/// path (registry, git, URL) and for a path that climbs out of the project
/// root, which the analysis never follows.
pub fn resolve_local_source(from_dir: &str, source: &str) -> Option<String> {
    if !(source.starts_with("./") || source.starts_with("../") || source == "." || source == "..") {
        return None;
    }
    let mut parts: Vec<&str> = from_dir.split('/').filter(|p| !p.is_empty()).collect();
    for segment in source.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// A project: the module directories read, keyed by their path relative to
/// the root (`""` is the root module).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TfProject {
    /// Directory key to module.
    pub modules: BTreeMap<String, TfModule>,
    /// Sources that exist in the project and were not read into the IR.
    pub unread: Vec<UnreadSource>,
}

/// A source the project has and the analysis did not read: the declarations
/// in it are missing from the IR, so the analysis is not complete without it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnreadSource {
    /// Path relative to the project root.
    pub path: String,
    /// Why it was not read.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_source_resolves_lexically_inside_the_project() {
        assert_eq!(
            resolve_local_source("", "./modules/vpc"),
            Some("modules/vpc".into())
        );
        assert_eq!(
            resolve_local_source("envs/prod", "../../modules/vpc"),
            Some("modules/vpc".into())
        );
        assert_eq!(
            resolve_local_source("modules/a", "../b"),
            Some("modules/b".into())
        );
        assert_eq!(resolve_local_source("a", ".."), Some(String::new()));
    }

    #[test]
    fn a_source_that_climbs_out_of_the_root_or_is_not_local_is_not_followed() {
        assert_eq!(resolve_local_source("", "../outside"), None);
        assert_eq!(resolve_local_source("a", "../../outside"), None);
        assert_eq!(
            resolve_local_source("", "terraform-aws-modules/vpc/aws"),
            None
        );
        assert_eq!(
            resolve_local_source("", "git::https://example.com/x.git"),
            None
        );
        assert_eq!(resolve_local_source("", "/etc/modules"), None);
    }

    #[test]
    fn references_include_those_hidden_in_indexes_and_opaque_parts() {
        let reference = |root: &str| Reference {
            root: root.into(),
            steps: vec![Step::Attr("x".into())],
            line: 1,
        };
        let indexed = Reference {
            root: "var".into(),
            steps: vec![
                Step::Attr("rules".into()),
                Step::Index(Box::new(TfExpr::Ref(reference("local")))),
            ],
            line: 1,
        };
        let expr = TfExpr::Template(vec![
            TfExpr::Literal(TfValue::Str("a".into())),
            TfExpr::Ref(indexed),
            TfExpr::Opaque {
                cause: UnknownCause::ForExpression,
                refs: vec![reference("module")],
            },
        ]);
        let roots: Vec<&str> = expr.references().iter().map(|r| r.root.as_str()).collect();
        assert_eq!(roots, vec!["var", "local", "module"]);
    }
}

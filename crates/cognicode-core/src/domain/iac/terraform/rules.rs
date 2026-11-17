//! Risk checks over the evaluated graph: public exposure, encryption and
//! permissions.
//!
//! A check never decides from a value it does not have. Each reads the
//! evaluated attributes and answers with one of three outcomes: a
//! [`Outcome::Violation`] only when every fact it needs is known, an
//! [`Outcome::Unverified`] naming why a needed value is unknown, or
//! [`Outcome::Satisfied`]. A resource whose `count`/`for_each` is unknown
//! may not exist, so its violations are downgraded to unverified; one whose
//! `count` is a known zero does not exist and is not assessed at all.
//!
//! Secrets are not a check here: the secret detector of the findings model
//! decides what looks like a hardcoded secret, and the observation layer
//! feeds it the literals this IR keeps.

use std::collections::BTreeSet;

use super::eval::Evaluation;
use super::graph::{NodeAddr, ResourceInstance, TfGraph, block_segment};
use super::ir::{TfBlock, TfBody};
use super::value::{TfValue, UnknownCause};

/// The risk a check looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskFamily {
    /// A resource reachable from outside its intended boundary.
    PublicExposure,
    /// Data left unencrypted.
    Encryption,
    /// Access granted beyond need.
    Permissions,
}

impl RiskFamily {
    /// A stable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::PublicExposure => "public_exposure",
            Self::Encryption => "encryption",
            Self::Permissions => "permissions",
        }
    }
}

/// What a check concluded.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The risk is verifiable from known values.
    Violation {
        /// What is wrong, without echoing secrets.
        detail: String,
    },
    /// The check could not decide: a value it needs is unknown.
    Unverified {
        /// Why.
        causes: BTreeSet<UnknownCause>,
    },
    /// The known values rule the risk out.
    Satisfied,
}

/// One check's result for one resource instance.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// The risk.
    pub family: RiskFamily,
    /// The stable check id.
    pub check: &'static str,
    /// The resource instance address.
    pub resource: String,
    /// The attribute path the check looked at.
    pub attribute: String,
    /// File of the declaration.
    pub file: String,
    /// 1-based line of the attribute or block.
    pub line: u32,
    /// What it concluded.
    pub outcome: Outcome,
}

/// Resource types some check models. Types outside it are reported as not
/// evaluated rather than as clean.
pub const MODELED_TYPES: &[&str] = &[
    "aws_s3_bucket",
    "aws_s3_bucket_acl",
    "aws_s3_bucket_public_access_block",
    "aws_security_group",
    "aws_security_group_rule",
    "aws_vpc_security_group_ingress_rule",
    "aws_db_instance",
    "aws_ebs_volume",
    "aws_efs_file_system",
    "aws_iam_policy",
    "aws_iam_role_policy",
    "aws_iam_user_policy",
    "aws_iam_group_policy",
    "aws_iam_policy_document",
];

/// Whether some check models `kind`.
pub fn is_modeled(kind: &str) -> bool {
    MODELED_TYPES.contains(&kind)
}

/// Assesses one resource instance.
pub fn assess(
    instance: &ResourceInstance,
    graph: &TfGraph,
    evaluation: &Evaluation,
) -> Vec<CheckResult> {
    let resource = &instance.resource;
    let mut ctx = Ctx {
        instance,
        graph,
        existence: existence(instance, evaluation),
        results: Vec::new(),
    };
    if matches!(ctx.existence, Existence::Absent) || !is_modeled(&resource.kind) {
        return Vec::new();
    }
    let view = View {
        base: instance.addr.as_str(),
        path: String::new(),
        body: &resource.body,
        evaluation,
        graph,
    };
    match resource.kind.as_str() {
        "aws_s3_bucket" | "aws_s3_bucket_acl" => s3_acl(&mut ctx, &view),
        "aws_s3_bucket_public_access_block" => s3_public_access_block(&mut ctx, &view),
        "aws_security_group" => security_group(&mut ctx, &view),
        "aws_security_group_rule" => security_group_rule(&mut ctx, &view),
        "aws_vpc_security_group_ingress_rule" => vpc_ingress_rule(&mut ctx, &view),
        "aws_db_instance" => db_instance(&mut ctx, &view),
        "aws_ebs_volume" | "aws_efs_file_system" => encrypted_flag(&mut ctx, &view),
        "aws_iam_policy"
        | "aws_iam_role_policy"
        | "aws_iam_user_policy"
        | "aws_iam_group_policy" => {
            iam_policy_attribute(&mut ctx, &view);
        }
        "aws_iam_policy_document" => iam_policy_document(&mut ctx, &view),
        _ => {}
    }
    ctx.results
}

/// Whether a known `count` of zero or an empty `for_each` means the resource
/// is never created.
pub fn is_absent(instance: &ResourceInstance, evaluation: &Evaluation) -> bool {
    matches!(existence(instance, evaluation), Existence::Absent)
}

enum Existence {
    Certain,
    Absent,
    Conditional(BTreeSet<UnknownCause>),
}

fn existence(instance: &ResourceInstance, evaluation: &Evaluation) -> Existence {
    let mut causes = BTreeSet::new();
    for meta in ["count", "for_each"] {
        if instance.resource.body.attribute(meta).is_none() {
            continue;
        }
        let addr = NodeAddr::new(format!("{}#{meta}", instance.addr));
        match evaluation.get(&addr) {
            Some(TfValue::Number(_))
                if evaluation.get(&addr).and_then(TfValue::as_i64) == Some(0) =>
            {
                return Existence::Absent;
            }
            Some(TfValue::List(items)) if items.is_empty() => return Existence::Absent,
            Some(TfValue::Object(map)) if map.is_empty() => return Existence::Absent,
            Some(value) if !value.is_known() => causes.extend(value.causes()),
            _ => {}
        }
    }
    if causes.is_empty() {
        Existence::Certain
    } else {
        Existence::Conditional(causes)
    }
}

struct Ctx<'a> {
    instance: &'a ResourceInstance,
    graph: &'a TfGraph,
    existence: Existence,
    results: Vec<CheckResult>,
}

impl Ctx<'_> {
    fn push(
        &mut self,
        family: RiskFamily,
        check: &'static str,
        attribute: impl Into<String>,
        line: u32,
        outcome: Outcome,
    ) {
        let outcome = match (outcome, &self.existence) {
            (Outcome::Violation { .. }, Existence::Conditional(causes)) => Outcome::Unverified {
                causes: causes.clone(),
            },
            (other, _) => other,
        };
        self.results.push(CheckResult {
            family,
            check,
            resource: self.instance.addr.to_string(),
            attribute: attribute.into(),
            file: self.instance.resource.file.clone(),
            line,
            outcome,
        });
    }

    fn line_of(&self, view: &View<'_>, name: &str) -> u32 {
        view.line(name, self.graph)
            .unwrap_or(self.instance.resource.line)
    }
}

/// The evaluated attributes of one body: the resource's own or a nested
/// block's.
struct View<'a> {
    base: &'a str,
    path: String,
    body: &'a TfBody,
    evaluation: &'a Evaluation,
    graph: &'a TfGraph,
}

impl<'a> View<'a> {
    fn value(&self, name: &str) -> Option<&'a TfValue> {
        self.body.attribute(name)?;
        self.evaluation
            .get(&NodeAddr::new(format!("{}#{}{name}", self.base, self.path)))
    }

    fn line(&self, name: &str, graph: &TfGraph) -> Option<u32> {
        graph
            .node(&NodeAddr::new(format!("{}#{}{name}", self.base, self.path)))
            .map(|n| n.line)
    }

    /// The static nested blocks of `kind` with their path, and whether any
    /// `dynamic` block generates the same kind.
    fn blocks(&self, kind: &str) -> (Vec<(String, &'a TfBlock, View<'a>)>, Vec<&'a TfBlock>) {
        let mut statics = Vec::new();
        let mut dynamics = Vec::new();
        let (mut ordinal_static, mut ordinal_dynamic) = (0usize, 0usize);
        for block in self.body.blocks.iter().filter(|b| b.kind == kind) {
            let ordinal = if block.dynamic.is_some() {
                &mut ordinal_dynamic
            } else {
                &mut ordinal_static
            };
            let segment = block_segment(block, *ordinal);
            *ordinal += 1;
            if block.dynamic.is_some() {
                dynamics.push(block);
            } else {
                let view = View {
                    base: self.base,
                    path: format!("{}{segment}.", self.path),
                    body: &block.body,
                    evaluation: self.evaluation,
                    graph: self.graph,
                };
                statics.push((segment, block, view));
            }
        }
        (statics, dynamics)
    }
}

fn provider_default(attribute: &str) -> TfValue {
    TfValue::unknown(UnknownCause::ProviderDefault {
        attribute: attribute.to_string(),
    })
}

/// A flag Terraform accepts as a bool or as the strings `"true"`/`"false"`.
fn flag(value: &TfValue) -> Option<bool> {
    match value {
        TfValue::Bool(b) => Some(*b),
        TfValue::Str(s) if s == "true" => Some(true),
        TfValue::Str(s) if s == "false" => Some(false),
        _ => None,
    }
}

/// Three-valued truth: the answer, or the reasons it is not known.
#[derive(Debug, Clone, PartialEq)]
enum Tri {
    Yes,
    No,
    Maybe(BTreeSet<UnknownCause>),
}

impl Tri {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Yes, Self::Yes) => Self::Yes,
            (Self::Maybe(mut a), Self::Maybe(b)) => {
                a.extend(b);
                Self::Maybe(a)
            }
            (Self::Maybe(a), Self::Yes) | (Self::Yes, Self::Maybe(a)) => Self::Maybe(a),
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Yes, _) | (_, Self::Yes) => Self::Yes,
            (Self::No, Self::No) => Self::No,
            (Self::Maybe(mut a), Self::Maybe(b)) => {
                a.extend(b);
                Self::Maybe(a)
            }
            (Self::Maybe(a), Self::No) | (Self::No, Self::Maybe(a)) => Self::Maybe(a),
        }
    }

    fn outcome(self, detail: impl FnOnce() -> String) -> Outcome {
        match self {
            Self::Yes => Outcome::Violation { detail: detail() },
            Self::No => Outcome::Satisfied,
            Self::Maybe(causes) => Outcome::Unverified { causes },
        }
    }
}

/// Whether a string, or any string in a list, satisfies `pred`.
fn any_string(value: Option<&TfValue>, pred: &dyn Fn(&str) -> bool) -> Tri {
    match value {
        None | Some(TfValue::Null) => Tri::No,
        Some(TfValue::Unknown(causes)) => Tri::Maybe(causes.clone()),
        Some(TfValue::Str(s)) => {
            if pred(s) {
                Tri::Yes
            } else {
                Tri::No
            }
        }
        Some(TfValue::List(items)) => items
            .iter()
            .fold(Tri::No, |acc, item| acc.or(any_string(Some(item), pred))),
        Some(_) => Tri::No,
    }
}

fn truth(value: &TfValue, expected: bool) -> Tri {
    match value {
        TfValue::Unknown(causes) => Tri::Maybe(causes.clone()),
        other => match flag(other) {
            Some(b) if b == expected => Tri::Yes,
            _ => Tri::No,
        },
    }
}

// --- public exposure -------------------------------------------------------

const PUBLIC_ACLS: &[&str] = &["public-read", "public-read-write", "authenticated-read"];

fn s3_acl(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let Some(value) = view.value("acl") else {
        return;
    };
    let tri = any_string(Some(value), &|s| PUBLIC_ACLS.contains(&s));
    let acl = value.as_str().unwrap_or_default().to_string();
    let line = ctx.line_of(view, "acl");
    ctx.push(
        RiskFamily::PublicExposure,
        "s3_public_acl",
        "acl",
        line,
        tri.outcome(|| format!("the ACL `{acl}` grants access outside the account")),
    );
}

fn s3_public_access_block(ctx: &mut Ctx<'_>, view: &View<'_>) {
    for name in [
        "block_public_acls",
        "block_public_policy",
        "ignore_public_acls",
        "restrict_public_buckets",
    ] {
        let line = ctx.line_of(view, name);
        let outcome = match view.value(name) {
            Some(value) => truth(value, false).outcome(|| format!("`{name}` is explicitly false")),
            None => Outcome::Unverified {
                causes: provider_default(name).causes(),
            },
        };
        ctx.push(
            RiskFamily::PublicExposure,
            "s3_public_access_block",
            name,
            line,
            outcome,
        );
    }
}

fn is_world_v4(cidr: &str) -> bool {
    cidr == "0.0.0.0/0"
}

fn is_world_v6(cidr: &str) -> bool {
    cidr == "::/0"
}

const SSH: i64 = 22;
const RDP: i64 = 3389;

fn port_of(value: Option<&TfValue>, attribute: &str) -> Result<i64, BTreeSet<UnknownCause>> {
    match value {
        None => Err(provider_default(attribute).causes()),
        Some(TfValue::Unknown(causes)) => Err(causes.clone()),
        Some(other) => other
            .as_i64()
            .or_else(|| other.as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(|| BTreeSet::from([UnknownCause::UnsupportedExpression])),
    }
}

/// Whether an ingress rule opens every port, or SSH/RDP, over TCP.
fn sensitive_service(
    protocol: Option<&TfValue>,
    from: Option<&TfValue>,
    to: Option<&TfValue>,
) -> Tri {
    let all = match protocol {
        Some(TfValue::Str(p)) if p == "-1" || p == "all" => Tri::Yes,
        Some(TfValue::Unknown(causes)) => Tri::Maybe(causes.clone()),
        None => Tri::Maybe(provider_default("protocol").causes()),
        _ => Tri::No,
    };
    let tcp = match protocol {
        Some(TfValue::Str(p)) if p == "tcp" || p == "6" => Tri::Yes,
        Some(TfValue::Unknown(causes)) => Tri::Maybe(causes.clone()),
        None => Tri::Maybe(provider_default("protocol").causes()),
        _ => Tri::No,
    };
    let ports = match (port_of(from, "from_port"), port_of(to, "to_port")) {
        (Ok(from), Ok(to)) => {
            if (from..=to).contains(&SSH) || (from..=to).contains(&RDP) {
                Tri::Yes
            } else {
                Tri::No
            }
        }
        (Err(mut a), Err(b)) => {
            a.extend(b);
            Tri::Maybe(a)
        }
        (Err(a), Ok(_)) | (Ok(_), Err(a)) => Tri::Maybe(a),
    };
    all.or(tcp.and(ports))
}

fn security_group(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let (statics, dynamics) = view.blocks("ingress");
    for (segment, block, inner) in &statics {
        let world = any_string(inner.value("cidr_blocks"), &is_world_v4)
            .or(any_string(inner.value("ipv6_cidr_blocks"), &is_world_v6));
        let service = sensitive_service(
            inner.value("protocol"),
            inner.value("from_port"),
            inner.value("to_port"),
        );
        ctx.push(
            RiskFamily::PublicExposure,
            "open_ingress",
            format!("{segment}.cidr_blocks"),
            block.line,
            world
                .and(service)
                .outcome(|| "ingress from anywhere reaches every port, SSH or RDP".to_string()),
        );
    }
    for block in dynamics {
        ctx.push(
            RiskFamily::PublicExposure,
            "open_ingress",
            format!("dynamic.{}", block.kind),
            block.line,
            Outcome::Unverified {
                causes: BTreeSet::from([UnknownCause::DynamicBlock]),
            },
        );
    }
}

fn security_group_rule(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let ingress = match view.value("type") {
        Some(value) => truth(&ingress_marker(value), true),
        None => Tri::Maybe(provider_default("type").causes()),
    };
    let world = any_string(view.value("cidr_blocks"), &is_world_v4)
        .or(any_string(view.value("ipv6_cidr_blocks"), &is_world_v6));
    let service = sensitive_service(
        view.value("protocol"),
        view.value("from_port"),
        view.value("to_port"),
    );
    let line = ctx.line_of(view, "cidr_blocks");
    ctx.push(
        RiskFamily::PublicExposure,
        "open_ingress",
        "cidr_blocks",
        line,
        ingress
            .and(world)
            .and(service)
            .outcome(|| "ingress from anywhere reaches every port, SSH or RDP".to_string()),
    );
}

/// `type = "ingress"` as a truth value, keeping unknowns.
fn ingress_marker(value: &TfValue) -> TfValue {
    match value {
        TfValue::Str(s) => TfValue::Bool(s == "ingress"),
        other => other.clone(),
    }
}

fn vpc_ingress_rule(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let world = any_string(view.value("cidr_ipv4"), &is_world_v4)
        .or(any_string(view.value("cidr_ipv6"), &is_world_v6));
    let service = sensitive_service(
        view.value("ip_protocol"),
        view.value("from_port"),
        view.value("to_port"),
    );
    let line = ctx.line_of(view, "cidr_ipv4");
    ctx.push(
        RiskFamily::PublicExposure,
        "open_ingress",
        "cidr_ipv4",
        line,
        world
            .and(service)
            .outcome(|| "ingress from anywhere reaches every port, SSH or RDP".to_string()),
    );
}

fn db_instance(ctx: &mut Ctx<'_>, view: &View<'_>) {
    if let Some(value) = view.value("publicly_accessible") {
        let line = ctx.line_of(view, "publicly_accessible");
        ctx.push(
            RiskFamily::PublicExposure,
            "db_publicly_accessible",
            "publicly_accessible",
            line,
            truth(value, true)
                .outcome(|| "the database accepts connections from the internet".to_string()),
        );
    }
    let line = ctx.line_of(view, "storage_encrypted");
    let outcome = match view.value("storage_encrypted") {
        Some(value) => {
            truth(value, false).outcome(|| "`storage_encrypted` is explicitly false".to_string())
        }
        None => {
            // These arguments make the instance inherit encryption from its
            // source, so the missing flag decides nothing.
            let inherits = ["kms_key_id", "replicate_source_db", "snapshot_identifier"]
                .iter()
                .any(|name| view.body.attribute(name).is_some());
            if inherits {
                Outcome::Unverified {
                    causes: provider_default("storage_encrypted").causes(),
                }
            } else {
                Outcome::Violation {
                    detail: "`storage_encrypted` is not set and defaults to unencrypted"
                        .to_string(),
                }
            }
        }
    };
    ctx.push(
        RiskFamily::Encryption,
        "storage_encrypted",
        "storage_encrypted",
        line,
        outcome,
    );
}

// --- encryption ------------------------------------------------------------

fn encrypted_flag(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let line = ctx.line_of(view, "encrypted");
    let outcome = match view.value("encrypted") {
        Some(value) => {
            truth(value, false).outcome(|| "`encrypted` is explicitly false".to_string())
        }
        None => Outcome::Unverified {
            causes: provider_default("encrypted").causes(),
        },
    };
    ctx.push(
        RiskFamily::Encryption,
        "storage_encrypted",
        "encrypted",
        line,
        outcome,
    );
}

// --- permissions -----------------------------------------------------------

fn iam_policy_attribute(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let Some(value) = view.value("policy") else {
        return;
    };
    let line = ctx.line_of(view, "policy");
    let outcome = match value.as_document() {
        Some(document) => document_outcome(&document),
        None if value.is_unknown() => Outcome::Unverified {
            causes: value.causes(),
        },
        None => Outcome::Unverified {
            causes: BTreeSet::from([UnknownCause::UnsupportedExpression]),
        },
    };
    ctx.push(
        RiskFamily::Permissions,
        "iam_wildcard_admin",
        "policy",
        line,
        outcome,
    );
}

fn document_outcome(document: &TfValue) -> Outcome {
    let statements = match document {
        TfValue::Unknown(causes) => {
            return Outcome::Unverified {
                causes: causes.clone(),
            };
        }
        TfValue::Object(map) => match map.get("Statement") {
            Some(TfValue::List(items)) => items.iter().collect::<Vec<_>>(),
            Some(single @ TfValue::Object(_)) => vec![single],
            Some(TfValue::Unknown(causes)) => {
                return Outcome::Unverified {
                    causes: causes.clone(),
                };
            }
            _ => return Outcome::Satisfied,
        },
        _ => {
            return Outcome::Unverified {
                causes: BTreeSet::from([UnknownCause::UnsupportedExpression]),
            };
        }
    };
    let verdict = statements.into_iter().fold(Tri::No, |acc, statement| {
        acc.or(statement_is_admin(statement))
    });
    verdict.outcome(|| "a statement allows every action on every resource".to_string())
}

fn statement_is_admin(statement: &TfValue) -> Tri {
    let TfValue::Object(map) = statement else {
        return match statement {
            TfValue::Unknown(causes) => Tri::Maybe(causes.clone()),
            _ => Tri::No,
        };
    };
    let allow = any_string(map.get("Effect"), &|s| s == "Allow");
    let star = |key: &str| any_string(map.get(key), &|s| s == "*");
    allow.and(star("Action")).and(star("Resource"))
}

fn iam_policy_document(ctx: &mut Ctx<'_>, view: &View<'_>) {
    let (statements, dynamics) = view.blocks("statement");
    for (segment, block, inner) in &statements {
        let allow = match inner.value("effect") {
            None => Tri::Yes,
            Some(value) => any_string(Some(value), &|s| s == "Allow"),
        };
        let star = |name: &str| any_string(inner.value(name), &|s| s == "*");
        ctx.push(
            RiskFamily::Permissions,
            "iam_wildcard_admin",
            segment.clone(),
            block.line,
            allow
                .and(star("actions"))
                .and(star("resources"))
                .outcome(|| "a statement allows every action on every resource".to_string()),
        );
    }
    for block in dynamics {
        ctx.push(
            RiskFamily::Permissions,
            "iam_wildcard_admin",
            "dynamic.statement",
            block.line,
            Outcome::Unverified {
                causes: BTreeSet::from([UnknownCause::DynamicBlock]),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::eval::Bindings;
    use super::super::ir::TfExpr;
    use super::super::ir::{
        Reference, ResourceMode, Step, TfAttribute, TfModule, TfProject, TfResource, TfVariable,
    };
    use super::*;

    fn s(text: &str) -> TfExpr {
        TfExpr::Literal(TfValue::Str(text.into()))
    }

    fn n(value: i64) -> TfExpr {
        TfExpr::Literal(TfValue::Number(value.to_string()))
    }

    fn list(items: Vec<TfExpr>) -> TfExpr {
        TfExpr::List(items)
    }

    fn var(name: &str) -> TfExpr {
        TfExpr::Ref(Reference {
            root: "var".into(),
            steps: vec![Step::Attr(name.into())],
            line: 1,
        })
    }

    fn attr(name: &str, expr: TfExpr) -> TfAttribute {
        TfAttribute {
            name: name.into(),
            expr,
            line: 7,
        }
    }

    fn block(kind: &str, attributes: Vec<TfAttribute>) -> TfBlock {
        TfBlock {
            kind: kind.into(),
            labels: vec![],
            dynamic: None,
            body: TfBody {
                attributes,
                blocks: vec![],
            },
            line: 9,
        }
    }

    fn results(
        kind: &str,
        attributes: Vec<TfAttribute>,
        blocks: Vec<TfBlock>,
        bindings: &Bindings,
    ) -> Vec<CheckResult> {
        let mut module = TfModule::default();
        module.variables.insert(
            "v".into(),
            TfVariable {
                name: "v".into(),
                default: None,
                sensitive: false,
                file: "v.tf".into(),
                line: 1,
            },
        );
        module.resources.insert(
            format!("{kind}.r"),
            TfResource {
                mode: ResourceMode::Managed,
                kind: kind.into(),
                name: "r".into(),
                body: TfBody { attributes, blocks },
                file: "main.tf".into(),
                line: 3,
            },
        );
        let project = TfProject {
            modules: BTreeMap::from([(String::new(), module)]),
            ..Default::default()
        };
        let graph = TfGraph::build(&project, "");
        let evaluation = Evaluation::compute(&graph, bindings);
        graph
            .resources()
            .iter()
            .flat_map(|r| assess(r, &graph, &evaluation))
            .collect()
    }

    fn outcomes(results: &[CheckResult]) -> Vec<&Outcome> {
        results.iter().map(|r| &r.outcome).collect()
    }

    fn is_violation(o: &Outcome) -> bool {
        matches!(o, Outcome::Violation { .. })
    }

    fn is_unverified(o: &Outcome) -> bool {
        matches!(o, Outcome::Unverified { .. })
    }

    #[test]
    fn a_public_acl_is_a_violation_only_when_the_value_is_known() {
        let none = Bindings::none();
        let literal = results(
            "aws_s3_bucket",
            vec![attr("acl", s("public-read"))],
            vec![],
            &none,
        );
        assert!(is_violation(&literal[0].outcome));
        assert_eq!(literal[0].line, 7);
        assert_eq!(literal[0].check, "s3_public_acl");

        let private = results(
            "aws_s3_bucket",
            vec![attr("acl", s("private"))],
            vec![],
            &none,
        );
        assert_eq!(outcomes(&private), vec![&Outcome::Satisfied]);

        let via_var = results("aws_s3_bucket", vec![attr("acl", var("v"))], vec![], &none);
        assert!(
            is_unverified(&via_var[0].outcome),
            "an unbound variable is not a violation"
        );

        let bound = Bindings::none().with("v", TfValue::Str("public-read-write".into()));
        let resolved = results("aws_s3_bucket", vec![attr("acl", var("v"))], vec![], &bound);
        assert!(
            is_violation(&resolved[0].outcome),
            "the same flow becomes verifiable once bound"
        );
    }

    #[test]
    fn ingress_from_anywhere_is_judged_on_the_port_and_the_protocol() {
        let none = Bindings::none();
        let rule = |cidr: TfExpr, from: i64, to: i64, proto: &str| {
            block(
                "ingress",
                vec![
                    attr("cidr_blocks", cidr),
                    attr("from_port", n(from)),
                    attr("to_port", n(to)),
                    attr("protocol", s(proto)),
                ],
            )
        };
        let world = || list(vec![s("0.0.0.0/0")]);
        let ssh = results(
            "aws_security_group",
            vec![],
            vec![rule(world(), 22, 22, "tcp")],
            &none,
        );
        assert!(is_violation(&ssh[0].outcome));
        assert_eq!(ssh[0].attribute, "ingress[0].cidr_blocks");

        let https = results(
            "aws_security_group",
            vec![],
            vec![rule(world(), 443, 443, "tcp")],
            &none,
        );
        assert_eq!(outcomes(&https), vec![&Outcome::Satisfied]);

        let range = results(
            "aws_security_group",
            vec![],
            vec![rule(world(), 0, 65535, "tcp")],
            &none,
        );
        assert!(
            is_violation(&range[0].outcome),
            "a range that contains 22 is open"
        );

        let all = results(
            "aws_security_group",
            vec![],
            vec![rule(world(), 0, 0, "-1")],
            &none,
        );
        assert!(is_violation(&all[0].outcome));

        let private = results(
            "aws_security_group",
            vec![],
            vec![rule(list(vec![s("10.0.0.0/8")]), 22, 22, "tcp")],
            &none,
        );
        assert_eq!(outcomes(&private), vec![&Outcome::Satisfied]);

        let unknown_cidr = results(
            "aws_security_group",
            vec![],
            vec![rule(var("v"), 22, 22, "tcp")],
            &none,
        );
        assert!(is_unverified(&unknown_cidr[0].outcome));
    }

    #[test]
    fn a_dynamic_ingress_block_is_unverified_not_clean() {
        let dynamic = TfBlock {
            dynamic: Some(Box::new(var("v"))),
            ..block(
                "ingress",
                vec![attr("cidr_blocks", list(vec![s("0.0.0.0/0")]))],
            )
        };
        let found = results(
            "aws_security_group",
            vec![],
            vec![dynamic],
            &Bindings::none(),
        );
        assert_eq!(
            found[0].outcome,
            Outcome::Unverified {
                causes: BTreeSet::from([UnknownCause::DynamicBlock])
            }
        );
    }

    #[test]
    fn resources_that_may_not_exist_are_not_blamed_and_absent_ones_are_skipped() {
        let open = || vec![attr("acl", s("public-read")), attr("count", var("v"))];
        let unknown_count = results("aws_s3_bucket", open(), vec![], &Bindings::none());
        assert!(is_unverified(&unknown_count[0].outcome));

        let zero = Bindings::none().with("v", TfValue::Number("0".into()));
        assert!(results("aws_s3_bucket", open(), vec![], &zero).is_empty());

        let one = Bindings::none().with("v", TfValue::Number("1".into()));
        assert!(is_violation(
            &results("aws_s3_bucket", open(), vec![], &one)[0].outcome
        ));
    }

    #[test]
    fn encryption_separates_explicit_false_from_provider_defaults() {
        let none = Bindings::none();
        let off = results(
            "aws_ebs_volume",
            vec![attr("encrypted", TfExpr::Literal(TfValue::Bool(false)))],
            vec![],
            &none,
        );
        assert!(is_violation(&off[0].outcome));
        let unset = results("aws_ebs_volume", vec![], vec![], &none);
        assert!(is_unverified(&unset[0].outcome));
        let on = results(
            "aws_ebs_volume",
            vec![attr("encrypted", TfExpr::Literal(TfValue::Bool(true)))],
            vec![],
            &none,
        );
        assert_eq!(outcomes(&on), vec![&Outcome::Satisfied]);

        let db = results("aws_db_instance", vec![], vec![], &none);
        assert!(
            db.iter()
                .any(|r| r.check == "storage_encrypted" && is_violation(&r.outcome))
        );
        let replica = results(
            "aws_db_instance",
            vec![attr("replicate_source_db", s("source"))],
            vec![],
            &none,
        );
        assert!(replica.iter().all(|r| !is_violation(&r.outcome)));
    }

    #[test]
    fn an_admin_policy_is_a_violation_and_unknown_leaves_keep_it_unverified() {
        let doc = |resource: TfExpr| TfExpr::Call {
            function: "jsonencode".into(),
            args: vec![TfExpr::Object(vec![(
                s("Statement"),
                list(vec![TfExpr::Object(vec![
                    (s("Effect"), s("Allow")),
                    (s("Action"), s("*")),
                    (s("Resource"), resource),
                ])]),
            )])],
        };
        let admin = results(
            "aws_iam_policy",
            vec![attr("policy", doc(s("*")))],
            vec![],
            &Bindings::none(),
        );
        assert!(is_violation(&admin[0].outcome));
        let scoped = results(
            "aws_iam_policy",
            vec![attr("policy", doc(s("arn:aws:s3:::b/*")))],
            vec![],
            &Bindings::none(),
        );
        assert_eq!(outcomes(&scoped), vec![&Outcome::Satisfied]);
        let unknown = results(
            "aws_iam_policy",
            vec![attr("policy", doc(var("v")))],
            vec![],
            &Bindings::none(),
        );
        assert!(is_unverified(&unknown[0].outcome));
        let opaque = results(
            "aws_iam_policy",
            vec![attr(
                "policy",
                TfExpr::Call {
                    function: "file".into(),
                    args: vec![s("p.json")],
                },
            )],
            vec![],
            &Bindings::none(),
        );
        assert!(
            is_unverified(&opaque[0].outcome),
            "a file() policy is not read, so it is not clean"
        );
    }

    #[test]
    fn types_no_check_models_produce_no_results() {
        let found = results(
            "aws_lambda_function",
            vec![attr("memory_size", n(128))],
            vec![],
            &Bindings::none(),
        );
        assert!(found.is_empty());
        assert!(!is_modeled("aws_lambda_function"));
        assert!(is_modeled("aws_security_group"));
    }
}

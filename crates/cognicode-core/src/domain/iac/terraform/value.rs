//! Terraform values as the analysis knows them: either known, or unknown with
//! a recorded reason.
//!
//! The analysis never evaluates HCL the way `terraform plan` does. A value is
//! [`TfValue::Unknown`] unless the configuration itself, read statically,
//! determines it. Every unknown carries the set of causes that made it so, and
//! composite values union the causes of their parts, so an unknown is always
//! explainable and never silently turns into a guess.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Query keys that carry a credential in a module source (`?token=..`,
/// `?aws_access_key_secret=..`).
const SECRET_QUERY_KEYS: &[&str] = &["token", "key", "secret", "pass", "pwd", "sig", "auth", "cred"];

/// A module `source` that is safe to show: the userinfo of a URL
/// (`user:password@`) is dropped and the value of a query parameter whose
/// name looks like a credential is replaced. Everything else, the host and
/// the path included, is kept so the source is still recognisable. A source
/// with nothing to hide comes back as written, and redacting twice changes
/// nothing more.
pub fn redact_source(source: &str) -> String {
    let (location, query) = match source.split_once('?') {
        Some((location, query)) => (location, Some(query)),
        None => (source, None),
    };
    let mut out = match location.find("://").map(|at| at + "://".len()) {
        Some(start) => {
            let end = location[start..]
                .find('/')
                .map_or(location.len(), |slash| start + slash);
            let host = location[start..end]
                .rsplit_once('@')
                .map_or(&location[start..end], |(_, host)| host);
            format!("{}{host}{}", &location[..start], &location[end..])
        }
        None => location.to_string(),
    };
    if let Some(query) = query {
        let params: Vec<String> = query
            .split('&')
            .map(|param| match param.split_once('=') {
                Some((key, _)) if is_secret_key(key) => format!("{key}=REDACTED"),
                _ => param.to_string(),
            })
            .collect();
        out.push('?');
        out.push_str(&params.join("&"));
    }
    out
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    SECRET_QUERY_KEYS.iter().any(|secret| key.contains(secret))
}

/// Why a value cannot be known without running Terraform or supplying input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnknownCause {
    /// A root variable that no input bound. `default_available` records that a
    /// default exists; it is a hint, never a value.
    UnboundVariable {
        /// Address of the variable node.
        address: String,
        /// Whether the variable declares a default.
        default_available: bool,
    },
    /// An attribute that the provider computes (`aws_s3_bucket.b.arn`).
    ComputedAttribute {
        /// Address of the referenced resource or data source.
        address: String,
        /// The attribute read, when the reference names one.
        attribute: Option<String>,
    },
    /// A resource or module that expands to several instances.
    ExpandedInstances {
        /// Address of the expanded block.
        address: String,
    },
    /// A function the analysis does not model.
    FunctionCall {
        /// Function name.
        name: String,
    },
    /// An arithmetic, comparison or logical operation.
    Operation,
    /// A `for` expression.
    ForExpression,
    /// A splat expression (`[*]`).
    Splat,
    /// A name bound at run time by Terraform (`each`, `count`, `self`, `path`).
    RuntimeBinding {
        /// The root name.
        name: String,
    },
    /// A module whose source is not available to the analysis.
    UnresolvedModule {
        /// Address of the module call.
        address: String,
        /// The `source` text; credentials are removed when it is shown.
        source: String,
    },
    /// A reference to something the configuration does not declare.
    UndeclaredReference {
        /// The traversal as written.
        reference: String,
    },
    /// A reference cycle; Terraform would reject the configuration.
    ReferenceCycle {
        /// A node on the cycle.
        address: String,
    },
    /// A template directive (`%{ if }`, `%{ for }`).
    TemplateDirective,
    /// A string with escape sequences the analysis does not decode.
    StringEscape,
    /// A `dynamic` block: the number and content of its blocks is computed.
    DynamicBlock,
    /// An index or attribute access that does not resolve in the known value.
    UnresolvedAccess {
        /// What was accessed.
        access: String,
    },
    /// The configuration does not set the attribute and the effective value is
    /// decided outside it (a provider or account default).
    ProviderDefault {
        /// The attribute that is not set.
        attribute: String,
    },
    /// A construct the reader does not model.
    UnsupportedExpression,
}

impl fmt::Display for UnknownCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundVariable {
                address,
                default_available,
            } => write!(
                f,
                "`{address}` has no bound value{}",
                if *default_available {
                    " (a default exists but is not assumed)"
                } else {
                    ""
                }
            ),
            Self::ComputedAttribute { address, attribute } => match attribute {
                Some(a) => write!(f, "`{address}.{a}` is computed by the provider"),
                None => write!(f, "`{address}` is computed by the provider"),
            },
            Self::ExpandedInstances { address } => {
                write!(f, "`{address}` expands to instances decided at plan time")
            }
            Self::FunctionCall { name } => write!(f, "function `{name}` is not evaluated"),
            Self::Operation => write!(f, "operators are not evaluated"),
            Self::ForExpression => write!(f, "`for` expressions are not evaluated"),
            Self::Splat => write!(f, "splat expressions are not evaluated"),
            Self::RuntimeBinding { name } => write!(f, "`{name}` is bound at run time"),
            Self::UnresolvedModule { address, source } => {
                write!(
                    f,
                    "module `{address}` (source `{}`) is not available",
                    redact_source(source)
                )
            }
            Self::UndeclaredReference { reference } => {
                write!(f, "`{reference}` is not declared in the configuration")
            }
            Self::ReferenceCycle { address } => write!(f, "`{address}` is on a reference cycle"),
            Self::TemplateDirective => write!(f, "template directives are not evaluated"),
            Self::StringEscape => write!(f, "string escapes are not decoded"),
            Self::DynamicBlock => write!(f, "a `dynamic` block is expanded at plan time"),
            Self::UnresolvedAccess { access } => write!(f, "{access} does not resolve"),
            Self::ProviderDefault { attribute } => write!(
                f,
                "`{attribute}` is not set; the effective value is decided outside the configuration"
            ),
            Self::UnsupportedExpression => write!(f, "the expression is not modeled"),
        }
    }
}

/// A value of the configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TfValue {
    /// `null`.
    Null,
    /// A boolean.
    Bool(bool),
    /// A number, kept as written so no precision is lost.
    Number(String),
    /// A string.
    Str(String),
    /// A tuple or list.
    List(Vec<TfValue>),
    /// An object or map.
    Object(BTreeMap<String, TfValue>),
    /// The result of `jsonencode`: a document whose leaves may still be
    /// unknown, which is why it is kept structured instead of as text.
    Json(Box<TfValue>),
    /// A value the analysis does not know.
    Unknown(BTreeSet<UnknownCause>),
}

impl TfValue {
    /// An unknown with one cause.
    pub fn unknown(cause: UnknownCause) -> Self {
        Self::Unknown(BTreeSet::from([cause]))
    }

    /// An unknown with the union of `causes`; known inputs contribute none.
    pub fn unknown_from(causes: BTreeSet<UnknownCause>) -> Self {
        debug_assert!(!causes.is_empty(), "an unknown needs a cause");
        Self::Unknown(causes)
    }

    /// Whether the value is wholly known: no unknown anywhere inside it.
    pub fn is_known(&self) -> bool {
        self.causes().is_empty()
    }

    /// Whether the value itself, not a part of it, is unknown.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }

    /// The union of the causes of every unknown inside the value.
    pub fn causes(&self) -> BTreeSet<UnknownCause> {
        let mut out = BTreeSet::new();
        self.collect_causes(&mut out);
        out
    }

    fn collect_causes(&self, out: &mut BTreeSet<UnknownCause>) {
        match self {
            Self::Unknown(causes) => out.extend(causes.iter().cloned()),
            Self::List(items) => items.iter().for_each(|i| i.collect_causes(out)),
            Self::Object(map) => map.values().for_each(|v| v.collect_causes(out)),
            Self::Json(inner) => inner.collect_causes(out),
            Self::Null | Self::Bool(_) | Self::Number(_) | Self::Str(_) => {}
        }
    }

    /// The string, when the value is a known string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, when the value is a known boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The integer, when the value is a known whole number.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => n.parse().ok(),
            _ => None,
        }
    }

    /// The text a scalar takes inside a string template, when it has one.
    ///
    /// Terraform converts numbers and booleans to strings in templates;
    /// collections are an error there, so they have no text.
    pub fn template_text(&self) -> Option<String> {
        match self {
            Self::Str(s) => Some(s.clone()),
            Self::Number(n) => Some(n.clone()),
            Self::Bool(b) => Some(b.to_string()),
            Self::Json(_) | Self::Null | Self::List(_) | Self::Object(_) | Self::Unknown(_) => None,
        }
    }

    /// The JSON text of a wholly known value, with object keys sorted and no
    /// whitespace, as `jsonencode` renders it. `None` when anything inside is
    /// unknown.
    pub fn to_json_text(&self) -> Option<String> {
        if !self.is_known() {
            return None;
        }
        let mut out = String::new();
        self.write_json(&mut out);
        Some(out)
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(&b.to_string()),
            Self::Number(n) => out.push_str(n),
            Self::Str(s) => out.push_str(&serde_json::Value::String(s.clone()).to_string()),
            Self::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            Self::Object(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::Value::String(k.clone()).to_string());
                    out.push(':');
                    v.write_json(out);
                }
                out.push('}');
            }
            Self::Json(inner) => inner.write_json(out),
            Self::Unknown(_) => out.push_str("null"),
        }
    }

    /// The structured document behind a policy-like value: a `jsonencode`
    /// result as is, or a known string parsed as JSON. `None` when the value
    /// is a string that is not JSON or is not a document at all.
    pub fn as_document(&self) -> Option<TfValue> {
        match self {
            Self::Json(inner) => Some((**inner).clone()),
            Self::Str(text) => serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .map(|v| Self::from_json(&v)),
            _ => None,
        }
    }

    /// A known value from parsed JSON.
    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(*b),
            serde_json::Value::Number(n) => Self::Number(n.to_string()),
            serde_json::Value::String(s) => Self::Str(s.clone()),
            serde_json::Value::Array(items) => {
                Self::List(items.iter().map(Self::from_json).collect())
            }
            serde_json::Value::Object(map) => Self::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Self::from_json(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn computed() -> UnknownCause {
        UnknownCause::ComputedAttribute {
            address: "aws_s3_bucket.b".into(),
            attribute: Some("arn".into()),
        }
    }

    #[test]
    fn a_composite_is_known_only_when_every_part_is() {
        let known = TfValue::List(vec![TfValue::Bool(true), TfValue::Str("a".into())]);
        assert!(known.is_known());

        let partial = TfValue::List(vec![TfValue::Bool(true), TfValue::unknown(computed())]);
        assert!(!partial.is_known());
        assert!(!partial.is_unknown(), "the list itself is a known shape");
        assert_eq!(partial.causes(), BTreeSet::from([computed()]));
    }

    #[test]
    fn a_json_document_with_an_unknown_leaf_has_no_text_but_keeps_its_structure() {
        let doc = TfValue::Json(Box::new(TfValue::Object(BTreeMap::from([
            ("a".into(), TfValue::Number("1".into())),
            ("b".into(), TfValue::unknown(computed())),
        ]))));
        assert_eq!(doc.to_json_text(), None);
        assert_eq!(doc.causes(), BTreeSet::from([computed()]));
        assert!(doc.as_document().is_some());
    }

    #[test]
    fn json_text_is_compact_with_sorted_keys() {
        let doc = TfValue::Object(BTreeMap::from([
            (
                "b".into(),
                TfValue::List(vec![TfValue::Bool(true), TfValue::Null]),
            ),
            ("a".into(), TfValue::Str("x\"y".into())),
        ]));
        assert_eq!(
            doc.to_json_text().as_deref(),
            Some(r#"{"a":"x\"y","b":[true,null]}"#)
        );
    }

    #[test]
    fn a_known_string_that_is_json_is_a_document_and_other_strings_are_not() {
        let policy = TfValue::Str(r#"{"Statement":[{"Effect":"Allow"}]}"#.into());
        let doc = policy.as_document().expect("valid JSON text");
        assert!(doc.is_known());
        assert_eq!(TfValue::Str("not json".into()).as_document(), None);
        assert_eq!(TfValue::Bool(true).as_document(), None);
    }

    #[test]
    fn only_scalars_have_template_text() {
        assert_eq!(
            TfValue::Number("3".into()).template_text().as_deref(),
            Some("3")
        );
        assert_eq!(
            TfValue::Bool(false).template_text().as_deref(),
            Some("false")
        );
        assert_eq!(TfValue::List(vec![]).template_text(), None);
        assert_eq!(TfValue::Null.template_text(), None);
    }
}

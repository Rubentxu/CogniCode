//! What a Helm template *states* about itself, without rendering it.
//!
//! A template is Go-template text, not YAML: reading it as a manifest would
//! invent objects whose fields depend on values and functions. This module
//! only lowers the template's actions to facts: which `.Values` paths it
//! reads, whether each read is guarded, and which constructs make a faithful
//! render depend on something outside the chart (cluster lookups, randomness,
//! other templates). The concrete objects come only from a real render.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// A construct that a template uses and that the analysis does not evaluate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Construct {
    /// `include "name"` or `template "name"`: output comes from a named template.
    Include(String),
    /// `tpl`: a string rendered as a template at run time.
    Tpl,
    /// `lookup`: reads the live cluster; empty under `helm template`.
    Lookup,
    /// A random, time or key-generating function: two renders differ.
    Randomness,
    /// `.Capabilities`: depends on the cluster version and API set supplied.
    ClusterCapabilities,
    /// `.Files`: reads files packaged with the chart.
    Files,
    /// The whole `.Values` object is used (`toYaml .Values`): the set of read
    /// keys cannot be listed.
    WholeValues,
}

impl Construct {
    /// Whether a render that uses this construct may differ from what the
    /// chart yields on a cluster or on a second run.
    pub fn limits_render_fidelity(&self) -> bool {
        matches!(
            self,
            Self::Lookup | Self::Randomness | Self::ClusterCapabilities | Self::Tpl
        )
    }

    /// A short human description for reports.
    pub fn describe(&self) -> String {
        match self {
            Self::Include(name) => format!("includes template `{name}`"),
            Self::Tpl => "renders a string as a template (`tpl`)".to_string(),
            Self::Lookup => "reads the live cluster (`lookup`), empty when rendered offline".into(),
            Self::Randomness => "uses random, time or key-generating functions".to_string(),
            Self::ClusterCapabilities => "depends on `.Capabilities`".to_string(),
            Self::Files => "reads packaged files (`.Files`)".to_string(),
            Self::WholeValues => "uses the whole `.Values` object".to_string(),
        }
    }
}

/// One read of a `.Values` path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValueRef {
    /// Dotted path below `.Values` (`image.tag`).
    pub path: String,
    /// 1-based line of the action in the template.
    pub line: u32,
    /// The read sits in a condition or is wrapped by a defaulting function, so
    /// an unset value is a legitimate branch rather than a gap.
    pub guarded: bool,
    /// The read is wrapped by `required`: an unset value fails the render.
    pub required: bool,
}

/// The static facts of one file below `templates/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateModel {
    /// Path relative to the chart root (`templates/deployment.yaml`).
    pub path: String,
    /// Names bound by `define` blocks.
    pub defines: Vec<String>,
    /// Every `.Values` read, in source order.
    pub value_refs: Vec<ValueRef>,
    /// Constructs the analysis does not evaluate.
    pub constructs: BTreeSet<Construct>,
    /// A helper file (`_helpers.tpl`, `NOTES.txt`): it defines text, it is not
    /// rendered into a manifest by itself.
    pub is_helper: bool,
}

impl TemplateModel {
    /// Lower `source` (the text of the template at `path`) to its facts.
    pub fn analyze(path: &str, source: &str) -> Self {
        let mut model = Self {
            path: path.to_string(),
            defines: Vec::new(),
            value_refs: Vec::new(),
            constructs: BTreeSet::new(),
            is_helper: is_helper_path(path),
        };
        for action in actions(source) {
            model.absorb(&action);
        }
        model
    }

    fn absorb(&mut self, action: &Action) {
        let words = words(&action.body);
        let Some(first) = words.first().map(String::as_str) else {
            return;
        };
        if (first == "define" || first == "block")
            && let Some(name) = string_literals(&action.body).into_iter().next()
        {
            self.defines.push(name);
        }
        if (matches!(first, "template" | "block") || words.iter().any(|w| w == "include"))
            && let Some(name) = string_literals(&action.body).into_iter().next()
        {
            self.constructs.insert(Construct::Include(name));
        }
        self.absorb_functions(&words);
        let guarded = is_guard(&words);
        let required = words.iter().any(|w| w == "required");
        for path in values_paths(&action.body) {
            match path {
                Some(path) => self.value_refs.push(ValueRef {
                    path,
                    line: action.line,
                    guarded,
                    required,
                }),
                None => {
                    self.constructs.insert(Construct::WholeValues);
                }
            }
        }
    }

    fn absorb_functions(&mut self, words: &[String]) {
        for word in words {
            match word.as_str() {
                "tpl" => self.constructs.insert(Construct::Tpl),
                "lookup" => self.constructs.insert(Construct::Lookup),
                "randAlphaNum" | "randAlpha" | "randNumeric" | "randAscii" | "uuidv4" | "now"
                | "genCA" | "genPrivateKey" | "genSelfSignedCert" | "genSignedCert" => {
                    self.constructs.insert(Construct::Randomness)
                }
                w if w.starts_with(".Capabilities") => {
                    self.constructs.insert(Construct::ClusterCapabilities)
                }
                w if w.starts_with(".Files") => self.constructs.insert(Construct::Files),
                _ => false,
            };
        }
    }

    /// Constructs of this template that limit the fidelity of a render.
    pub fn fidelity_limits(&self) -> impl Iterator<Item = &Construct> {
        self.constructs
            .iter()
            .filter(|c| c.limits_render_fidelity())
    }
}

/// Whether a file below `templates/` is rendered into manifests on its own.
pub fn is_helper_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.starts_with('_') || name.eq_ignore_ascii_case("NOTES.txt")
}

/// Whether `path` is a file that Helm renders into manifest documents.
pub fn is_manifest_template(path: &str) -> bool {
    path.starts_with("templates/")
        && !is_helper_path(path)
        && [".yaml", ".yml", ".json"]
            .iter()
            .any(|ext| path.ends_with(ext))
}

struct Action {
    body: String,
    line: u32,
}

/// The non-comment `{{ ... }}` actions of `source`, with trim markers removed.
fn actions(source: &str) -> Vec<Action> {
    let mut found = Vec::new();
    let mut rest = source;
    let mut line = 1u32;
    while let Some(open) = rest.find("{{") {
        line += newlines(&rest[..open]);
        let after = &rest[open + 2..];
        let (end, consumed) = action_end(after);
        let raw = &after[..end];
        let body = raw
            .trim_start_matches('-')
            .trim_end_matches('-')
            .trim()
            .to_string();
        if !body.starts_with("/*") && !body.is_empty() {
            found.push(Action { body, line });
        }
        line += newlines(&after[..consumed.min(after.len())]);
        rest = &after[consumed.min(after.len())..];
    }
    found
}

fn newlines(text: &str) -> u32 {
    text.bytes().filter(|b| *b == b'\n').count() as u32
}

/// Index of the closing `}}` (ignoring braces inside string literals) and the
/// number of bytes consumed including it. An unterminated action consumes the
/// rest of the text.
fn action_end(text: &str) -> (usize, usize) {
    if text.trim_start_matches('-').trim_start().starts_with("/*") {
        return text.find("*/").map_or((text.len(), text.len()), |idx| {
            let tail = &text[idx + 2..];
            tail.find("}}")
                .map_or((text.len(), text.len()), |c| (idx + 2 + c, idx + 2 + c + 2))
        });
    }
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    for (pos, &(idx, ch)) in bytes.iter().enumerate() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if ch == '\\' && q != '`' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                }
            }
            None => {
                if matches!(ch, '"' | '`' | '\'') {
                    quote = Some(ch);
                } else if ch == '}' && bytes.get(pos + 1).is_some_and(|&(_, n)| n == '}') {
                    return (idx, idx + 2);
                }
            }
        }
    }
    (text.len(), text.len())
}

/// Whitespace-separated words with parentheses and pipes split off and string
/// literals dropped, so a word inside a string never counts as a function.
fn words(body: &str) -> Vec<String> {
    let mut cleaned = String::with_capacity(body.len());
    let mut quote: Option<char> = None;
    for ch in body.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if matches!(ch, '"' | '`' | '\'') => {
                quote = Some(ch);
                cleaned.push(' ');
            }
            None if matches!(ch, '(' | ')' | '|' | ',' | ':' | '=') => cleaned.push(' '),
            None => cleaned.push(ch),
        }
    }
    cleaned.split_whitespace().map(str::to_string).collect()
}

fn string_literals(body: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut current: Option<(char, String)> = None;
    for ch in body.chars() {
        match &mut current {
            Some((q, text)) if ch == *q => {
                literals.push(std::mem::take(text));
                current = None;
            }
            Some((_, text)) => text.push(ch),
            None if matches!(ch, '"' | '`') => current = Some((ch, String::new())),
            None => {}
        }
    }
    literals
}

const GUARD_WORDS: &[&str] = &[
    "if", "with", "default", "hasKey", "empty", "coalesce", "ternary", "and", "or", "not",
    "kindIs", "typeIs", "required",
];

fn is_guard(words: &[String]) -> bool {
    let conditional = words
        .iter()
        .take(2)
        .any(|w| w == "if" || w == "with" || w == "range");
    conditional
        || words
            .iter()
            .any(|w| w != "required" && GUARD_WORDS.contains(&w.as_str()))
}

/// `Some(path)` for a `.Values.a.b` read; `None` for a bare `.Values`.
fn values_paths(body: &str) -> Vec<Option<String>> {
    let text = strip_strings(body);
    let mut found = Vec::new();
    let mut search = 0;
    while let Some(at) = text[search..].find(".Values") {
        let start = search + at;
        let end = start + ".Values".len();
        search = end;
        let boundary_before = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        if !boundary_before {
            continue;
        }
        let mut path: Vec<&str> = Vec::new();
        let mut cursor = end;
        while text[cursor..].starts_with('.') {
            let tail = &text[cursor + 1..];
            let len = tail
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            if len == 0 {
                break;
            }
            path.push(&tail[..len]);
            cursor += 1 + len;
        }
        found.push(if path.is_empty() {
            None
        } else {
            Some(path.join("."))
        });
        search = cursor;
    }
    found
}

fn strip_strings(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut quote: Option<char> = None;
    for ch in body.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if matches!(ch, '"' | '`' | '\'') => {
                quote = Some(ch);
                out.push(' ');
            }
            None => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(source: &str) -> Vec<(String, bool, bool)> {
        TemplateModel::analyze("templates/a.yaml", source)
            .value_refs
            .into_iter()
            .map(|r| (r.path, r.guarded, r.required))
            .collect()
    }

    #[test]
    fn plain_reads_are_unguarded_and_conditions_are_guards() {
        let source = "replicas: {{ .Values.replicas }}\n{{- if .Values.ingress.enabled }}\nhost: {{ .Values.ingress.host | default \"x\" }}\n{{- end }}";
        assert_eq!(
            refs(source),
            vec![
                ("replicas".into(), false, false),
                ("ingress.enabled".into(), true, false),
                ("ingress.host".into(), true, false),
            ]
        );
    }

    #[test]
    fn required_marks_the_read_and_is_not_a_guard() {
        let source = r#"pw: {{ required "set it" .Values.auth.password }}"#;
        assert_eq!(refs(source), vec![("auth.password".into(), false, true)]);
    }

    #[test]
    fn text_in_strings_and_comments_is_not_a_read() {
        let source = "{{/* .Values.ignored */}}\nname: {{ \"{{ .Values.alsoIgnored }}\" }}{{ .Values.real }}";
        assert_eq!(refs(source), vec![("real".into(), false, false)]);
    }

    #[test]
    fn lines_are_tracked_across_multiline_actions() {
        let source = "a: 1\n{{- if and\n  .Values.x\n  .Values.y }}\nb: 2\n{{ .Values.z }}";
        let model = TemplateModel::analyze("templates/a.yaml", source);
        let lines: Vec<u32> = model.value_refs.iter().map(|r| r.line).collect();
        assert_eq!(lines, vec![2, 2, 6]);
    }

    #[test]
    fn constructs_that_leave_the_chart_are_named() {
        let source = r#"{{ include "app.name" . }}{{ lookup "v1" "Secret" "ns" "s" }}{{ randAlphaNum 8 }}{{ tpl .Values.extra . }}{{ toYaml .Values }}"#;
        let model = TemplateModel::analyze("templates/a.yaml", source);
        assert!(
            model
                .constructs
                .contains(&Construct::Include("app.name".into()))
        );
        for expected in [
            Construct::Lookup,
            Construct::Randomness,
            Construct::Tpl,
            Construct::WholeValues,
        ] {
            assert!(model.constructs.contains(&expected), "{expected:?}");
        }
        assert_eq!(model.fidelity_limits().count(), 3);
    }

    #[test]
    fn defines_are_collected_and_helpers_are_not_manifest_templates() {
        let model = TemplateModel::analyze(
            "templates/_helpers.tpl",
            r#"{{- define "app.name" -}}x{{- end -}}"#,
        );
        assert_eq!(model.defines, vec!["app.name".to_string()]);
        assert!(model.is_helper);
        assert!(!is_manifest_template("templates/_helpers.tpl"));
        assert!(!is_manifest_template("templates/NOTES.txt"));
        assert!(is_manifest_template("templates/deployment.yaml"));
        assert!(!is_manifest_template("values.yaml"));
    }

    #[test]
    fn an_unterminated_action_does_not_panic_or_read_past_the_end() {
        let model = TemplateModel::analyze("templates/a.yaml", "a: {{ .Values.x");
        assert!(model.value_refs.len() <= 1);
    }
}

//! Classification: observed calls in, dataflow statements with subjects out.
//!
//! The classifier is the only place that decides *which arguments of a sink
//! reach it*. That decision is what separates a parameterized query (the value
//! sits in a bound position) from a concatenation (the value sits in the SQL
//! text), without any flow logic of its own.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::catalog::{
    ApiModel, LanguageAdapter, ModelRole, SemanticModelCatalog, SourceLanguage, Weakness,
};
use super::jvm::JvmContext;
use super::rule::{SANITIZER_SUBJECT, USER_INPUT_SUBJECT};
use crate::domain::findings::{
    DataflowFunction, DataflowInput, DataflowLocation, DataflowStatement, GroundingRef,
    SubjectPattern,
};

/// One argument of an observed call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallArg {
    /// A compile-time constant: it carries no variable.
    Literal,
    /// An expression over variables (a variable, a concatenation, a format).
    Value {
        /// The variables the expression reads.
        uses: Vec<String>,
    },
}

impl CallArg {
    fn uses(&self) -> &[String] {
        match self {
            Self::Literal => &[],
            Self::Value { uses } => uses,
        }
    }
}

/// A call as observed in source: callee, arguments and the variable receiving
/// the result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedCall {
    /// Fully qualified callee, as the language adapter spells it.
    pub callee: String,
    /// Arguments in order.
    pub args: Vec<CallArg>,
    /// Variable defined by the call result, if any.
    pub def: Option<String>,
}

/// What an observation is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    /// A call.
    Call(ObservedCall),
    /// A plain assignment (including string building).
    Assign {
        /// Variable defined.
        def: String,
        /// Variables read.
        uses: Vec<String>,
    },
}

/// One observed statement, in program order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// 1-based source line.
    pub line: u32,
    /// The canonical fact this was observed from, when the pipeline recorded
    /// one. Without it the statement may explain a path but cannot open the
    /// gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingRef>,
    /// The statement itself.
    pub kind: ObservationKind,
}

impl Observation {
    /// An ungrounded observation at `line`.
    pub fn new(line: u32, kind: ObservationKind) -> Self {
        Self {
            line,
            grounding: None,
            kind,
        }
    }

    /// The same observation bound to a canonical fact.
    pub fn grounded(mut self, grounding: GroundingRef) -> Self {
        self.grounding = Some(grounding);
        self
    }
}

/// One function as observed in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedFunction {
    /// Function identity.
    pub id: String,
    /// Source path.
    pub path: String,
    /// The language whose adapter classifies the calls.
    pub language: SourceLanguage,
    /// Statements in program order.
    pub observations: Vec<Observation>,
    /// Imports and classpath supertypes at the call sites, for JVM languages.
    /// Optional: when absent, only fully qualified callees resolve and the
    /// rest are reported as unmodeled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm: Option<JvmContext>,
}

/// A function classified against a language adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeledFunction {
    function: DataflowFunction,
    unmodeled_callees: Vec<String>,
    context_resolved: Vec<(String, String)>,
}

impl ModeledFunction {
    /// The dataflow view to hand to a dataflow backend.
    pub fn dataflow_input(&self) -> DataflowInput {
        DataflowInput {
            functions: vec![self.function.clone()],
        }
    }

    /// The classified function.
    pub fn function(&self) -> &DataflowFunction {
        &self.function
    }

    /// Callees the adapter does not know, sorted and deduplicated. A flow may
    /// hide behind any of them.
    pub fn unmodeled_callees(&self) -> &[String] {
        &self.unmodeled_callees
    }

    /// Callees the JVM context resolved, as `(as written, as cataloged)`,
    /// sorted and deduplicated. They explain why a call that is not spelled
    /// fully qualified was still modeled.
    pub fn context_resolved(&self) -> &[(String, String)] {
        &self.context_resolved
    }
}

/// The outcome of classifying a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// The language is modeled; the function carries subjects.
    Modeled(ModeledFunction),
    /// No adapter exists for the language: nothing was analysed.
    Unsupported {
        /// The language without coverage.
        language: SourceLanguage,
    },
}

impl SemanticModelCatalog {
    /// Classify `function` for `weakness` with the adapter of its language.
    pub fn classify(&self, weakness: Weakness, function: &ObservedFunction) -> Classification {
        let Some(adapter) = self.adapter(function.language) else {
            return Classification::Unsupported {
                language: function.language,
            };
        };

        let mut unmodeled = BTreeSet::new();
        let mut context_resolved = BTreeSet::new();
        let mut statements = Vec::with_capacity(function.observations.len());
        for (index, observation) in function.observations.iter().enumerate() {
            let (defs, uses, subject) = match &observation.kind {
                ObservationKind::Assign { def, uses } => (vec![def.clone()], dedup(uses), None),
                ObservationKind::Call(call) => {
                    let defs: Vec<String> = call.def.iter().cloned().collect();
                    let model =
                        lookup_model(adapter, function, &call.callee, &mut context_resolved);
                    match model.map(ApiModel::role) {
                        None => {
                            unmodeled.insert(call.callee.clone());
                            (defs, all_uses(call), None)
                        }
                        Some(ModelRole::Source) => (defs, all_uses(call), Some(USER_INPUT_SUBJECT)),
                        Some(ModelRole::Sink {
                            weakness: w,
                            text_arg,
                        }) if w == weakness => {
                            // Only the dangerous argument reaches the sink:
                            // bound values are data, not code.
                            let text = call.args.get(text_arg).map(CallArg::uses).unwrap_or(&[]);
                            (defs, dedup(text), Some(weakness.sink_subject()))
                        }
                        Some(ModelRole::Sanitizer { neutralises })
                            if neutralises.contains(&weakness) =>
                        {
                            (defs, all_uses(call), Some(SANITIZER_SUBJECT))
                        }
                        // Inert, or a sink/sanitizer of another weakness.
                        Some(_) => (defs, all_uses(call), None),
                    }
                }
            };
            statements.push(DataflowStatement {
                id: index as u64 + 1,
                defs,
                uses,
                subjects: subject.into_iter().map(subject_pattern).collect(),
                location: DataflowLocation {
                    path: function.path.clone(),
                    line: observation.line,
                },
                grounding: observation.grounding,
            });
        }

        Classification::Modeled(ModeledFunction {
            function: DataflowFunction {
                id: function.id.clone(),
                statements,
            },
            unmodeled_callees: unmodeled.into_iter().collect(),
            context_resolved: context_resolved.into_iter().collect(),
        })
    }
}

/// The model of `callee`: exact first, then through the JVM context when the
/// function carries one.
fn lookup_model<'a>(
    adapter: &'a LanguageAdapter,
    function: &ObservedFunction,
    callee: &str,
    resolved: &mut BTreeSet<(String, String)>,
) -> Option<&'a ApiModel> {
    if let Some(model) = adapter.lookup(callee) {
        return Some(model);
    }
    if !function.language.is_jvm() {
        return None;
    }
    let qualified = function
        .jvm
        .as_ref()?
        .resolve(callee, |candidate| adapter.lookup(candidate).is_some())?;
    resolved.insert((callee.to_string(), qualified.clone()));
    adapter.lookup(&qualified)
}

fn subject_pattern(name: &str) -> SubjectPattern {
    // The subject names are crate constants validated by this module's tests.
    SubjectPattern::new(name).expect("constant subject names are valid namespaced names")
}

fn all_uses(call: &ObservedCall) -> Vec<String> {
    dedup(call.args.iter().flat_map(CallArg::uses))
}

fn dedup<'a>(uses: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    uses.into_iter()
        .filter(|u| seen.insert(u.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::rule::SQL_EXECUTION_SUBJECT;
    use super::*;

    fn call(callee: &str, args: Vec<CallArg>, def: Option<&str>) -> Observation {
        Observation::new(
            1,
            ObservationKind::Call(ObservedCall {
                callee: callee.to_string(),
                args,
                def: def.map(str::to_string),
            }),
        )
    }

    fn val(uses: &[&str]) -> CallArg {
        CallArg::Value {
            uses: uses.iter().map(|u| u.to_string()).collect(),
        }
    }

    fn classify(language: SourceLanguage, observations: Vec<Observation>) -> Classification {
        SemanticModelCatalog::builtin().classify(
            Weakness::SqlInjection,
            &ObservedFunction {
                id: "f".to_string(),
                path: "f".to_string(),
                language,
                observations,
                jvm: None,
            },
        )
    }

    fn modeled(classification: Classification) -> ModeledFunction {
        match classification {
            Classification::Modeled(m) => m,
            other => panic!("expected Modeled, got {other:?}"),
        }
    }

    fn subjects(statement: &DataflowStatement) -> Vec<&str> {
        statement.subjects.iter().map(|s| s.as_str()).collect()
    }

    #[test]
    fn only_the_text_argument_of_a_sink_is_used() {
        let m = modeled(classify(
            SourceLanguage::Go,
            vec![call(
                "database/sql.DB.Query",
                vec![CallArg::Literal, val(&["id"])],
                Some("rows"),
            )],
        ));
        let sink = &m.function().statements[0];
        assert!(
            sink.uses.is_empty(),
            "a bound value must not reach the sink"
        );
        assert_eq!(subjects(sink), vec![SQL_EXECUTION_SUBJECT]);
    }

    #[test]
    fn a_value_in_the_text_argument_reaches_the_sink() {
        let m = modeled(classify(
            SourceLanguage::Go,
            vec![call("database/sql.DB.Query", vec![val(&["q", "q"])], None)],
        ));
        assert_eq!(m.function().statements[0].uses, vec!["q".to_string()]);
    }

    #[test]
    fn sources_and_sanitizers_get_their_subjects() {
        let m = modeled(classify(
            SourceLanguage::Java,
            vec![
                call(
                    "javax.servlet.http.HttpServletRequest.getParameter",
                    vec![CallArg::Literal],
                    Some("id"),
                ),
                call("java.lang.Integer.parseInt", vec![val(&["id"])], Some("n")),
            ],
        ));
        let statements = &m.function().statements;
        assert_eq!(subjects(&statements[0]), vec![USER_INPUT_SUBJECT]);
        assert_eq!(statements[0].defs, vec!["id".to_string()]);
        assert_eq!(subjects(&statements[1]), vec![SANITIZER_SUBJECT]);
        assert_eq!(statements[1].uses, vec!["id".to_string()]);
        assert!(m.unmodeled_callees().is_empty());
    }

    #[test]
    fn unknown_callees_are_listed_and_stay_plain_statements() {
        let m = modeled(classify(
            SourceLanguage::Rust,
            vec![
                call("app::z", vec![val(&["a"])], Some("b")),
                call("app::a", vec![], None),
                call("app::z", vec![], None),
            ],
        ));
        assert_eq!(
            m.unmodeled_callees(),
            ["app::a".to_string(), "app::z".to_string()]
        );
        let first = &m.function().statements[0];
        assert!(first.subjects.is_empty());
        assert_eq!(first.uses, vec!["a".to_string()]);
        assert_eq!(first.defs, vec!["b".to_string()]);
    }

    #[test]
    fn statement_ids_follow_program_order_and_keep_location_and_grounding() {
        let grounding = GroundingRef::entity(
            crate::domain::kernel_ids::EntityId::new(9),
            crate::domain::kernel_ids::FactId::new(90),
        );
        let observed = Observation::new(
            42,
            ObservationKind::Assign {
                def: "x".to_string(),
                uses: vec!["y".to_string()],
            },
        )
        .grounded(grounding);
        let m = modeled(classify(
            SourceLanguage::Rust,
            vec![call("app::f", vec![], None), observed],
        ));
        let second = &m.function().statements[1];
        assert_eq!(second.id, 2);
        assert_eq!(second.location.line, 42);
        assert_eq!(second.grounding, Some(grounding));
        assert_eq!(m.function().statements[0].grounding, None);
    }

    #[test]
    fn a_language_without_adapter_is_unsupported() {
        assert_eq!(
            classify(SourceLanguage::Python, vec![call("x", vec![], None)]),
            Classification::Unsupported {
                language: SourceLanguage::Python
            }
        );
    }

    fn jvm_function(language: SourceLanguage, jvm: Option<JvmContext>) -> ObservedFunction {
        ObservedFunction {
            id: "f".to_string(),
            path: "f".to_string(),
            language,
            observations: vec![call("Statement.executeQuery", vec![val(&["q"])], None)],
            jvm,
        }
    }

    #[test]
    fn a_jvm_context_resolves_a_simple_name_into_the_catalog() {
        let ctx = JvmContext::new().with_import("Statement", "java.sql.Statement");
        for language in [SourceLanguage::Java, SourceLanguage::Kotlin] {
            let m = modeled(SemanticModelCatalog::builtin().classify(
                Weakness::SqlInjection,
                &jvm_function(language, Some(ctx.clone())),
            ));
            assert_eq!(
                subjects(&m.function().statements[0]),
                vec![SQL_EXECUTION_SUBJECT]
            );
            assert!(m.unmodeled_callees().is_empty());
            assert_eq!(
                m.context_resolved(),
                [(
                    "Statement.executeQuery".to_string(),
                    "java.sql.Statement.executeQuery".to_string()
                )]
            );
        }
    }

    #[test]
    fn without_a_jvm_context_the_same_call_stays_unmodeled() {
        let m = modeled(SemanticModelCatalog::builtin().classify(
            Weakness::SqlInjection,
            &jvm_function(SourceLanguage::Java, None),
        ));
        assert_eq!(
            m.unmodeled_callees(),
            ["Statement.executeQuery".to_string()]
        );
        assert!(m.context_resolved().is_empty());
    }

    #[test]
    fn a_jvm_context_never_resolves_calls_of_other_languages() {
        let ctx = JvmContext::new().with_import("Statement", "java.sql.Statement");
        let m = modeled(SemanticModelCatalog::builtin().classify(
            Weakness::SqlInjection,
            &jvm_function(SourceLanguage::Go, Some(ctx)),
        ));
        assert_eq!(m.unmodeled_callees().len(), 1);
    }

    #[test]
    fn the_sink_subject_follows_the_weakness_asked_for() {
        let call = vec![call("os/exec.Command", vec![val(&["c"])], None)];
        let catalog = SemanticModelCatalog::builtin();
        let function = ObservedFunction {
            id: "f".to_string(),
            path: "f".to_string(),
            language: SourceLanguage::Go,
            observations: call,
            jvm: None,
        };
        let subjects_for = |weakness| {
            let m = modeled(catalog.classify(weakness, &function));
            m.function().statements[0]
                .subjects
                .iter()
                .map(|s| s.as_str().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            subjects_for(Weakness::CommandInjection),
            vec![Weakness::CommandInjection.sink_subject().to_string()]
        );
        // A command sink is plain data when the question is SQL injection.
        assert!(subjects_for(Weakness::SqlInjection).is_empty());
    }

    #[test]
    fn a_numeric_sanitizer_neutralises_every_weakness_but_a_basename_only_path() {
        let catalog = SemanticModelCatalog::builtin();
        let run = |callee: &str, weakness| {
            let function = ObservedFunction {
                id: "f".to_string(),
                path: "f".to_string(),
                language: SourceLanguage::Go,
                observations: vec![call(callee, vec![val(&["x"])], Some("y"))],
                jvm: None,
            };
            let m = modeled(catalog.classify(weakness, &function));
            m.function().statements[0].subjects.len()
        };
        assert_eq!(run("strconv.Atoi", Weakness::CommandInjection), 1);
        assert_eq!(run("path/filepath.Base", Weakness::PathTraversal), 1);
        assert_eq!(run("path/filepath.Base", Weakness::SqlInjection), 0);
    }
}

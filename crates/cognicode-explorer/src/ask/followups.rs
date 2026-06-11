//! Deterministic follow-up generation for [`crate::ask`].
//!
//! The router MUST emit 1-3 [`FollowUp`] entries per successful
//! response. Follow-ups are context-aware (a forward reach response
//! gets an inverse-direction follow-up; a path response gets a
//! "what does X depend on?" follow-up; etc.) and are deterministic
//! for a given `(category, entities, primary_result)` triple.

use serde_json::Value;

use crate::ask::patterns::QuestionCategory;
use crate::mcp::FollowUp;

/// Build the follow-up list for a successful dispatch. Always
/// returns at least one entry; the spec requires `suggested_follow_ups`
/// to be non-empty for every successful response.
pub fn generate_follow_ups(
    category: QuestionCategory,
    entities: &[String],
    primary_result: &Value,
) -> Vec<FollowUp> {
    let mut out: Vec<FollowUp> = Vec::new();
    match category {
        QuestionCategory::PathBetween => {
            // Spec: "a path response MUST include a follow-up
            // 'what does X depend on?'" for each endpoint.
            for entity in entities {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: format!("related_inverse:what does `{entity}` depend on?"),
                    kind: Some("related_inverse".to_string()),
                });
            }
            // If the primary_result reports an empty path, suggest
            // widening the search radius (spec: "try broader search
            // radius" follow-up when no path exists).
            if is_empty_path(primary_result) {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: "hint:try broader search radius".to_string(),
                    kind: Some("hint".to_string()),
                });
            }
        }
        QuestionCategory::ForwardReach => {
            // Spec: inverse-direction follow-up after forward reach.
            if let Some(x) = entities.first() {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: format!("related_inverse:who calls `{x}`?"),
                    kind: Some("related_inverse".to_string()),
                });
            }
            // Leaf: no outgoing calls — suggest inspecting the body.
            if is_empty_edges(primary_result) {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: "hint:inspect the function body".to_string(),
                    kind: Some("hint".to_string()),
                });
            }
        }
        QuestionCategory::BackwardReach => {
            if let Some(x) = entities.first() {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: format!("related_inverse:what does `{x}` call?"),
                    kind: Some("related_inverse".to_string()),
                });
            }
            if is_empty_edges(primary_result) {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: format!("hint:try `{}` without namespace", x_label(entities)),
                    kind: Some("hint".to_string()),
                });
            }
        }
        QuestionCategory::CodeQuality => {
            if let Some(x) = entities.first() {
                out.push(FollowUp {
                    tool: "explorer_inspect_object".to_string(),
                    reason: format!("inspect `{}` to see the smells in context", x),
                    kind: Some("inspect".to_string()),
                });
            }
        }
        QuestionCategory::Architecture => {
            out.push(FollowUp {
                tool: "cognicode_ask".to_string(),
                reason: "related:where should I start?".to_string(),
                kind: Some("related".to_string()),
            });
        }
        QuestionCategory::WorkspaceOverview => {
            out.push(FollowUp {
                tool: "cognicode_ask".to_string(),
                reason: "related:any cycles in the workspace?".to_string(),
                kind: Some("related".to_string()),
            });
        }
        QuestionCategory::ComponentCluster => {
            if has_no_component(primary_result) {
                out.push(FollowUp {
                    tool: "cognicode_ask".to_string(),
                    reason: "related:show me the architecture shape".to_string(),
                    kind: Some("related".to_string()),
                });
            }
        }
        QuestionCategory::GenericDescription => {
            // Spec: when no pattern matched, surface a
            // `no_pattern_match` follow-up. We always include it
            // for the fallback category — the spec §"Unmatched
            // question returns low-confidence fallback" is what
            // makes this a contract.
            out.push(FollowUp {
                tool: "cognicode_ask".to_string(),
                reason: "no_pattern_match:try phrasing as 'path between X and Y' \
                         or 'who calls X?'"
                    .to_string(),
                kind: Some("no_pattern_match".to_string()),
            });
        }
    }
    // Spec: at least one follow-up per successful response. If the
    // category produced none (e.g. a category with no entity), add a
    // generic "explore more" hint.
    if out.is_empty() {
        out.push(FollowUp {
            tool: "cognicode_ask".to_string(),
            reason: "hint:ask a more specific question".to_string(),
            kind: Some("hint".to_string()),
        });
    }
    out
}

fn is_empty_path(v: &Value) -> bool {
    v.get("path")
        .and_then(|p| p.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(false)
}

fn is_empty_edges(v: &Value) -> bool {
    v.get("edges")
        .and_then(|e| e.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(false)
}

fn has_no_component(v: &Value) -> bool {
    v.get("component_id").map(|c| c.is_null()).unwrap_or(false)
}

fn x_label(entities: &[String]) -> String {
    entities.first().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn path_between_emits_dependency_follow_up_per_endpoint() {
        let ups = generate_follow_ups(
            QuestionCategory::PathBetween,
            &["a".to_string(), "b".to_string()],
            &json!({"path": ["a","x","b"], "length": 3}),
        );
        // One per endpoint, no "broader search" hint.
        assert_eq!(ups.len(), 2);
        for u in &ups {
            assert_eq!(u.kind.as_deref(), Some("related_inverse"));
        }
    }

    #[test]
    fn path_between_empty_path_emits_broader_search_hint() {
        let ups = generate_follow_ups(
            QuestionCategory::PathBetween,
            &["a".to_string(), "b".to_string()],
            &json!({"path": [], "length": 0}),
        );
        assert!(
            ups.iter()
                .any(|u| u.reason.contains("broader search radius")),
            "got: {:?}",
            ups
        );
    }

    #[test]
    fn forward_reach_emits_inverse_follow_up() {
        let ups = generate_follow_ups(
            QuestionCategory::ForwardReach,
            &["foo".to_string()],
            &json!({"root": "foo", "edges": [{"from":"foo","to":"bar","kind":"calls"}]}),
        );
        assert!(
            ups.iter().any(|u| u.reason.contains("who calls `foo`?")),
            "got: {:?}",
            ups
        );
    }

    #[test]
    fn generic_description_emits_no_pattern_match_follow_up() {
        let ups = generate_follow_ups(
            QuestionCategory::GenericDescription,
            &[],
            &json!({"summary": "x", "kind": "function", "location": "x.rs"}),
        );
        assert!(
            ups.iter()
                .any(|u| u.kind.as_deref() == Some("no_pattern_match"))
        );
    }

    #[test]
    fn generate_follow_ups_is_deterministic() {
        let a = generate_follow_ups(
            QuestionCategory::BackwardReach,
            &["f".to_string()],
            &json!({"root": "f", "edges": []}),
        );
        let b = generate_follow_ups(
            QuestionCategory::BackwardReach,
            &["f".to_string()],
            &json!({"root": "f", "edges": []}),
        );
        // Two calls with identical inputs MUST return identical
        // serialized output (spec §"deterministic for a given pair").
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn at_least_one_follow_up_for_every_category() {
        // Spec: `suggested_follow_ups` MUST be non-empty for every
        // successful response. Run every category with empty entities
        // and assert we get ≥ 1 follow-up.
        let cats = [
            QuestionCategory::PathBetween,
            QuestionCategory::ForwardReach,
            QuestionCategory::BackwardReach,
            QuestionCategory::CodeQuality,
            QuestionCategory::Architecture,
            QuestionCategory::WorkspaceOverview,
            QuestionCategory::ComponentCluster,
            QuestionCategory::GenericDescription,
        ];
        for c in cats {
            let ups = generate_follow_ups(c, &[], &json!({}));
            assert!(!ups.is_empty(), "category {:?} produced no follow-ups", c);
        }
    }
}

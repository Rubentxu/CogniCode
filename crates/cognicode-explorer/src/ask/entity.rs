//! Entity extraction for [`crate::ask`].
//!
//! Pulls backtick-quoted tokens out of the question and (when wired
//! to an `ExplorerService`) runs `spotter_search` to disambiguate
//! candidates whose `confidence >= 0.6`.

use serde::Serialize;

use crate::facades::SearchService;
use crate::mcp::FollowUp;

/// A symbol-like token pulled from a free-form question, with the
/// `spotter_search` results attached.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractedEntity {
    /// Raw token text as it appeared between backticks.
    pub raw: String,
    /// `spotter_search` hits with `score >= 0.6`. Empty when
    /// `spotter_search` returned nothing.
    pub candidates: Vec<EntityCandidate>,
}

/// One spotter hit: the resolved MVP id and its score.
#[derive(Debug, Clone, Serialize)]
pub struct EntityCandidate {
    pub object_id: String,
    pub score: f32,
}

/// Extract backtick-quoted tokens from a (lowercased) question
/// without contacting the service. Used by [`crate::ask::AskRouter::classify`].
pub(crate) fn extract_backtick_tokens(question: &str) -> Vec<String> {
    use regex::Regex;
    let Ok(re) = Regex::new(r"`([^`]+)`") else {
        return Vec::new();
    };
    re.captures_iter(question)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Extract backtick tokens, then resolve each against the explorer
/// service. Returns the parsed entities (one per token) and any
/// disambiguation follow-ups that should be surfaced in the envelope.
pub async fn extract_entities(
    question: &str,
    search: &dyn SearchService,
) -> (Vec<ExtractedEntity>, Vec<FollowUp>) {
    let tokens = extract_backtick_tokens(question);
    let mut entities = Vec::with_capacity(tokens.len());
    let mut follow_ups: Vec<FollowUp> = Vec::new();

    for token in tokens {
        let hits = search.spotter_search(&token, None).await.unwrap_or_default();
        // The spec threshold is 0.6.
        let candidates: Vec<EntityCandidate> = hits
            .into_iter()
            .filter(|h| h.score >= 0.6)
            .map(|h| EntityCandidate {
                object_id: h.object.id,
                score: h.score,
            })
            .collect();

        if candidates.is_empty() {
            // No spotter matches — record the empty result and emit a
            // `no_entity_match` follow-up so the agent can correct the
            // token.
            follow_ups.push(FollowUp {
                tool: "cognicode_ask".to_string(),
                reason: format!("no_entity_match:{token}"),
                kind: Some("no_entity_match".to_string()),
            });
        } else if candidates.len() > 1 {
            // Multiple high-confidence candidates — let the user
            // disambiguate. The dispatcher still proceeds with the
            // top-1 candidate.
            follow_ups.push(FollowUp {
                tool: "cognicode_ask".to_string(),
                reason: format!("entity_disambiguation:{token}"),
                kind: Some("entity_disambiguation".to_string()),
            });
        }

        entities.push(ExtractedEntity {
            raw: token,
            candidates,
        });
    }

    (entities, follow_ups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_backtick_tokens_returns_no_matches_for_plain_text() {
        let v = extract_backtick_tokens("no backticks here at all");
        assert!(v.is_empty());
    }

    #[test]
    fn extract_backtick_tokens_returns_one_match() {
        let v = extract_backtick_tokens("what does `validate` do?");
        assert_eq!(v, vec!["validate".to_string()]);
    }

    #[test]
    fn extract_backtick_tokens_returns_many_matches() {
        let v = extract_backtick_tokens("path between `parse` and `render`");
        assert_eq!(v, vec!["parse".to_string(), "render".to_string()]);
    }

    #[test]
    fn extract_backtick_tokens_skips_empty_backticks() {
        let v = extract_backtick_tokens("what does `` do?");
        // Empty inner match — the regex requires `+`, so `` does not
        // match. Should be empty.
        assert!(v.is_empty());
    }
}

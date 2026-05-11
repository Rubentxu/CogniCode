//! Layer-0 Preflight Filter using Aho-Corasick
//!
//! Pre-flight is an ultra-fast filter that runs BEFORE AST parsing.
//! It uses Aho-Corasick over all rule keywords to determine which rules
//! are eligible for a given source file.
//!
//! Rules without keywords are always eligible.
//! Rules with keywords are eligible if any of their keywords are present in source.

use aho_corasick::AhoCorasick;
use std::collections::HashSet;
use crate::rules::types::Rule;

/// Layer-0 preflight filter using Aho-Corasick automaton.
///
/// Determines which rules are eligible for a given source file
/// based on keyword presence.
pub struct PreflightFilter {
    /// The Aho-Corasick automaton built from all rule keywords
    automaton: AhoCorasick,
    /// All keywords in order (index matches automaton pattern ID)
    all_keywords: Vec<String>,
    /// Maps each rule index to its keyword indices in all_keywords (empty = always eligible)
    rule_keyword_indices: Vec<Vec<usize>>,
    /// Total number of rules
    rule_count: usize,
    /// Whether we have any keywords at all (optimization)
    has_any_keywords: bool,
}

impl PreflightFilter {
    /// Build a new PreflightFilter from a slice of rules.
    ///
    /// Collects all unique keywords from `rule.required_keywords()` and builds
    /// a single Aho-Corasick automaton for efficient multi-pattern matching.
    pub fn new(rules: &[Box<dyn Rule>]) -> Self {
        let mut all_keywords: Vec<String> = Vec::new();
        let mut rule_keyword_indices: Vec<Vec<usize>> = Vec::with_capacity(rules.len());

        for rule in rules.iter() {
            let keywords: Vec<&str> = rule.required_keywords();
            let mut indices = Vec::with_capacity(keywords.len());
            for kw in keywords {
                // Find or add keyword to all_keywords
                if let Some(idx) = all_keywords.iter().position(|k| k == kw) {
                    indices.push(idx);
                } else {
                    indices.push(all_keywords.len());
                    all_keywords.push(kw.to_string());
                }
            }
            rule_keyword_indices.push(indices);
        }

        // Build automaton (case-sensitive matching)
        // Note: we pass string slices from owned strings, which is safe because
        // all_keywords owns the strings and we don't move them
        let keyword_refs: Vec<&str> = all_keywords.iter().map(|s| s.as_str()).collect();
        let automaton = AhoCorasick::new(&keyword_refs)
            .expect("Aho-Corasick automaton build failed");

        let has_any_keywords = !all_keywords.is_empty();

        Self {
            automaton,
            all_keywords,
            rule_keyword_indices,
            rule_count: rules.len(),
            has_any_keywords,
        }
    }

    /// Returns indices of rules that are eligible for the given source.
    ///
    /// A rule is eligible if:
    /// - It has no keywords (always eligible), OR
    /// - At least one of its keywords is found in the source
    pub fn eligible_rule_indices(&self, source: &str) -> Vec<usize> {
        // Fast path: if no keywords exist, all rules are eligible
        if !self.has_any_keywords {
            return (0..self.rule_count).collect();
        }

        // Find all matches in the source and collect matched pattern indices
        let matched_pattern_indices: HashSet<usize> = self.automaton
            .find_iter(source)
            .map(|m| m.pattern().as_usize())
            .collect();

        // Determine eligible rules
        let mut eligible = Vec::new();
        for idx in 0..self.rule_count {
            let keyword_indices = &self.rule_keyword_indices[idx];

            // Rules with no keywords are always eligible
            if keyword_indices.is_empty() {
                eligible.push(idx);
                continue;
            }

            // Rules with keywords are eligible if any keyword matches
            if keyword_indices.iter().any(|&kw_idx| matched_pattern_indices.contains(&kw_idx)) {
                eligible.push(idx);
            }
        }

        eligible
    }

    /// Returns true if the filter has any keywords configured.
    pub fn has_keywords(&self) -> bool {
        self.has_any_keywords
    }

    /// Returns the number of rules this filter manages.
    pub fn rule_count(&self) -> usize {
        self.rule_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::{Rule, Severity, Category};
    use crate::rules::types::RuleContext;
    use std::path::Path;

    /// Mock rule for testing preflight
    struct MockRule {
        id: &'static str,
        keywords: Vec<&'static str>,
    }

    impl MockRule {
        fn new(id: &'static str, keywords: Vec<&'static str>) -> Self {
            Self { id, keywords }
        }
    }

    impl Rule for MockRule {
        fn id(&self) -> &str { self.id }
        fn name(&self) -> &str { self.id }
        fn severity(&self) -> Severity { Severity::Minor }
        fn category(&self) -> Category { Category::CodeSmell }
        fn language(&self) -> &str { "rust" }
        fn check(&self, _ctx: &RuleContext) -> Vec<crate::rules::types::Issue> { vec![] }
        fn required_keywords(&self) -> Vec<&str> { self.keywords.clone() }
    }

    fn make_dyn_rule(rule: MockRule) -> Box<dyn Rule> {
        Box::new(rule)
    }

    #[test]
    fn test_rules_without_keywords_are_always_eligible() {
        let rules: Vec<Box<dyn Rule>> = vec![
            make_dyn_rule(MockRule::new("always_run", vec![])),
            make_dyn_rule(MockRule::new("also_always", vec![])),
        ];

        let filter = PreflightFilter::new(&rules);

        // Should be eligible even with empty source
        let eligible = filter.eligible_rule_indices("");
        assert_eq!(eligible, vec![0, 1]);

        // Should still be eligible with any source
        let eligible = filter.eligible_rule_indices("fn main() {}");
        assert_eq!(eligible, vec![0, 1]);
    }

    #[test]
    fn test_rules_with_keywords_only_run_when_keyword_appears() {
        let rules: Vec<Box<dyn Rule>> = vec![
            make_dyn_rule(MockRule::new("md5_rule", vec!["md5"])),
            make_dyn_rule(MockRule::new("sha_rule", vec!["sha1", "sha256"])),
            make_dyn_rule(MockRule::new("always_rule", vec![])),
        ];

        let filter = PreflightFilter::new(&rules);

        // Source with md5 - should match md5_rule and always_rule
        let eligible = filter.eligible_rule_indices("use md5::compute();");
        assert!(eligible.contains(&0)); // md5_rule
        assert!(!eligible.contains(&1)); // sha_rule
        assert!(eligible.contains(&2)); // always_rule

        // Source with sha256 - should match sha_rule and always_rule
        let eligible = filter.eligible_rule_indices("let hash = sha256::hash(data);");
        assert!(!eligible.contains(&0)); // md5_rule
        assert!(eligible.contains(&1)); // sha_rule
        assert!(eligible.contains(&2)); // always_rule

        // Source with both - should match all keyword rules
        let eligible = filter.eligible_rule_indices("md5 and sha256 both present");
        assert!(eligible.contains(&0)); // md5_rule
        assert!(eligible.contains(&1)); // sha_rule
        assert!(eligible.contains(&2)); // always_rule

        // Source with neither - should only match always_rule
        let eligible = filter.eligible_rule_indices("let x = 1;");
        assert!(!eligible.contains(&0)); // md5_rule
        assert!(!eligible.contains(&1)); // sha_rule
        assert!(eligible.contains(&2)); // always_rule
    }

    #[test]
    fn test_mixed_rule_sets_behave_correctly() {
        let rules: Vec<Box<dyn Rule>> = vec![
            make_dyn_rule(MockRule::new("r1", vec!["TODO"])),
            make_dyn_rule(MockRule::new("r2", vec!["FIXME", "BUG"])),
            make_dyn_rule(MockRule::new("r3", vec![])),
            make_dyn_rule(MockRule::new("r4", vec!["HACK"])),
        ];

        let filter = PreflightFilter::new(&rules);

        // Source with TODO and BUG
        let source = "fn foo() { // TODO: fix this\n// BUG: infinite loop";
        let eligible = filter.eligible_rule_indices(source);
        assert!(eligible.contains(&0)); // r1 (TODO)
        assert!(eligible.contains(&1)); // r2 (BUG)
        assert!(eligible.contains(&2)); // r3 (always)
        assert!(!eligible.contains(&3)); // r4 (HACK)

        // Source with only HACK
        let source = "// HACK: workaround here";
        let eligible = filter.eligible_rule_indices(source);
        assert!(!eligible.contains(&0)); // r1 (TODO)
        assert!(!eligible.contains(&1)); // r2 (FIXME/BUG)
        assert!(eligible.contains(&2)); // r3 (always)
        assert!(eligible.contains(&3)); // r4 (HACK)
    }

    #[test]
    fn test_case_sensitive_matching() {
        let rules: Vec<Box<dyn Rule>> = vec![
            make_dyn_rule(MockRule::new("case_rule", vec!["Md5"])),
        ];

        let filter = PreflightFilter::new(&rules);

        // Case-sensitive: lowercase 'md5' should NOT match 'Md5' keyword
        let eligible = filter.eligible_rule_indices("use md5::compute();");
        assert!(!eligible.contains(&0)); // case doesn't match

        // 'Md5' should match
        let eligible = filter.eligible_rule_indices("use Md5::compute();");
        assert!(eligible.contains(&0));
    }

    #[test]
    fn test_empty_rules_vector() {
        let rules: Vec<Box<dyn Rule>> = vec![];
        let filter = PreflightFilter::new(&rules);

        assert!(filter.eligible_rule_indices("any source").is_empty());
        assert!(!filter.has_keywords());
        assert_eq!(filter.rule_count(), 0);
    }

    #[test]
    fn test_single_rule_with_multiple_keywords() {
        let rules: Vec<Box<dyn Rule>> = vec![
            make_dyn_rule(MockRule::new("multi_kw", vec!["aes", "des", "rc4"])),
        ];

        let filter = PreflightFilter::new(&rules);

        // Any keyword should make it eligible
        let eligible = filter.eligible_rule_indices("rc4 encryption");
        assert!(eligible.contains(&0));

        let eligible = filter.eligible_rule_indices("des cipher");
        assert!(eligible.contains(&0));

        let eligible = filter.eligible_rule_indices("aes-gcm");
        assert!(eligible.contains(&0));

        // No matching keyword
        let eligible = filter.eligible_rule_indices("blowfish");
        assert!(!eligible.contains(&0));
    }
}

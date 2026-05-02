//! Rule store — in-memory CRUD with optional persistence
//!
//! Manages governance rules that map to Cedar policies. Each rule has metadata
//! (name, description, tags) and the raw Cedar policy text.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AxiomError, AxiomResult};

/// Unique rule identifier
pub type RuleId = String;

/// A governance rule with metadata and Cedar policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier
    pub id: RuleId,
    /// Human-readable name
    pub name: String,
    /// Description of what this rule enforces
    pub description: String,
    /// Raw Cedar policy text
    pub cedar_policy: String,
    /// Whether this rule is currently active
    pub enabled: bool,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Filter criteria for listing rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleFilter {
    /// Filter by enabled status
    pub enabled: Option<bool>,
    /// Filter by tags (any match)
    pub tags: Option<Vec<String>>,
    /// Filter by name substring
    pub name_contains: Option<String>,
}

/// In-memory rule store with thread-safe access
#[derive(Debug)]
pub struct RuleStore {
    rules: RwLock<HashMap<RuleId, Rule>>,
}

impl RuleStore {
    /// Create a new empty rule store
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(HashMap::new()),
        }
    }

    /// Add a new rule
    pub fn add(&self, rule: Rule) -> AxiomResult<RuleId> {
        let mut rules = self.rules.write().map_err(|_| {
            AxiomError::Other("RuleStore lock poisoned".to_string())
        })?;
        if rules.contains_key(&rule.id) {
            return Err(AxiomError::RuleAlreadyExists {
                rule_id: rule.id.clone(),
            });
        }
        let id = rule.id.clone();
        rules.insert(id.clone(), rule);
        Ok(id)
    }

    /// Remove a rule by ID
    pub fn remove(&self, id: &RuleId) -> AxiomResult<()> {
        let mut rules = self.rules.write().map_err(|_| {
            AxiomError::Other("RuleStore lock poisoned".to_string())
        })?;
        rules
            .remove(id)
            .ok_or_else(|| AxiomError::RuleNotFound {
                rule_id: id.clone(),
            })?;
        Ok(())
    }

    /// Update an existing rule
    pub fn update(&self, id: &RuleId, update_fn: impl FnOnce(&mut Rule)) -> AxiomResult<()> {
        let mut rules = self.rules.write().map_err(|_| {
            AxiomError::Other("RuleStore lock poisoned".to_string())
        })?;
        let rule = rules.get_mut(id).ok_or_else(|| AxiomError::RuleNotFound {
            rule_id: id.clone(),
        })?;
        update_fn(rule);
        rule.updated_at = Utc::now();
        Ok(())
    }

    /// Get a rule by ID
    pub fn get(&self, id: &RuleId) -> Option<Rule> {
        self.rules.read().ok().and_then(|r| r.get(id).cloned())
    }

    /// List rules matching an optional filter
    pub fn list(&self, filter: Option<&RuleFilter>) -> Vec<Rule> {
        let rules = match self.rules.read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let values: Vec<Rule> = rules.values().cloned().collect();

        match filter {
            Some(f) => values
                .into_iter()
                .filter(|r| {
                    if let Some(enabled) = f.enabled {
                        if r.enabled != enabled {
                            return false;
                        }
                    }
                    if let Some(ref tags) = f.tags {
                        if !tags.iter().any(|t| r.tags.contains(t)) {
                            return false;
                        }
                    }
                    if let Some(ref name) = f.name_contains {
                        if !r.name.to_lowercase().contains(&name.to_lowercase()) {
                            return false;
                        }
                    }
                    true
                })
                .collect(),
            None => values,
        }
    }

    /// Get count of rules
    pub fn count(&self) -> usize {
        self.rules.read().map(|r| r.len()).unwrap_or(0)
    }

    /// Create a new rule with auto-generated ID and timestamps
    pub fn create_rule(
        name: String,
        description: String,
        cedar_policy: String,
        tags: Vec<String>,
    ) -> Rule {
        let now = Utc::now();
        Rule {
            id: generate_rule_id(),
            name,
            description,
            cedar_policy,
            enabled: true,
            tags,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Generate a unique rule ID using timestamp + random suffix
fn generate_rule_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let random_suffix: u16 = rand_simple();
    format!("rule_{}_{:x}", timestamp, random_suffix)
}

/// Simple deterministic "random" for ID generation (not cryptographically secure)
fn rand_simple() -> u16 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::Instant;
    
    static mut LAST_VAL: u64 = 0;
    let instant = Instant::now();
    let mut hasher = DefaultHasher::new();
    instant.hash(&mut hasher);
    unsafe { LAST_VAL = LAST_VAL.wrapping_add(1) };
    hasher.write_u64(unsafe { LAST_VAL });
    let hash = hasher.finish();
    (hash % 0xFFFF) as u16
}

impl Default for RuleStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rule(name: &str, policy: &str) -> Rule {
        Rule {
            id: generate_rule_id(),
            name: name.to_string(),
            description: format!("Test rule: {}", name),
            cedar_policy: policy.to_string(),
            enabled: true,
            tags: vec!["test".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_add_and_get() {
        let store = RuleStore::new();
        let rule = test_rule("test", "permit(principal, action, resource);");
        let id = store.add(rule.clone()).unwrap();
        assert_eq!(store.get(&id).unwrap().name, "test");
    }

    #[test]
    fn test_add_duplicate_fails() {
        let store = RuleStore::new();
        let rule = test_rule("test", "permit(principal, action, resource);");
        let id = rule.id.clone();
        store.add(rule).unwrap();
        let duplicate = test_rule("test2", "permit(principal, action, resource);");
        // Use same id
        let mut dup = duplicate;
        dup.id = id.clone();
        assert!(store.add(dup).is_err());
    }

    #[test]
    fn test_remove() {
        let store = RuleStore::new();
        let rule = test_rule("test", "permit(principal, action, resource);");
        let id = rule.id.clone();
        store.add(rule).unwrap();
        store.remove(&id).unwrap();
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn test_update() {
        let store = RuleStore::new();
        let rule = test_rule("test", "permit(principal, action, resource);");
        let id = rule.id.clone();
        store.add(rule).unwrap();

        store.update(&id, |r| {
            r.name = "updated".to_string();
        }).unwrap();

        assert_eq!(store.get(&id).unwrap().name, "updated");
    }

    #[test]
    fn test_list_with_filter() {
        let store = RuleStore::new();

        let mut rule1 = test_rule("active", "permit(principal, action, resource);");
        rule1.tags = vec!["security".to_string()];
        store.add(rule1).unwrap();

        let mut rule2 = test_rule("disabled", "permit(principal, action, resource);");
        rule2.enabled = false;
        rule2.tags = vec!["testing".to_string()];
        store.add(rule2).unwrap();

        // Filter: enabled only
        let filter = RuleFilter {
            enabled: Some(true),
            ..Default::default()
        };
        assert_eq!(store.list(Some(&filter)).len(), 1);

        // Filter: by tag
        let filter = RuleFilter {
            tags: Some(vec!["security".to_string()]),
            ..Default::default()
        };
        assert_eq!(store.list(Some(&filter)).len(), 1);

        // No filter
        assert_eq!(store.list(None).len(), 2);
    }

    #[test]
    fn test_create_rule_helper() {
        let rule = RuleStore::create_rule(
            "Test".to_string(),
            "Description".to_string(),
            "permit(principal, action, resource);".to_string(),
            vec!["tag1".to_string()],
        );
        assert!(rule.enabled);
        assert_eq!(rule.name, "Test");
        assert_eq!(rule.tags, vec!["tag1"]);
    }
}

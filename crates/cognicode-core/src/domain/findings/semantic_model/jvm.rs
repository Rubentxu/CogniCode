//! JVM semantic context for Java and Kotlin callees.
//!
//! A JVM call site is rarely spelled with the fully qualified name the API
//! catalog uses: source says `stmt.executeQuery(..)` where `stmt` was declared
//! as `Statement`, imported from `java.sql`, or as a pooled subclass of it. The
//! information that closes the gap (imports, aliases, the supertypes the
//! classpath gives a type) is **context the source frontend may or may not
//! have**.
//!
//! This module keeps that context as plain data and resolves with it. It does
//! not scan, compile or run anything: no JVM, no Detekt, no build tool is
//! needed. When the context is absent or does not cover a callee, the callee
//! stays unresolved and the analysis says so (`Unknown`) instead of guessing.
//!
//! Pure domain: no I/O.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

/// Imports, aliases and classpath supertypes visible at a call site.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JvmContext {
    /// Simple name or alias in scope -> fully qualified type.
    #[serde(default)]
    imports: BTreeMap<String, String>,
    /// Fully qualified type -> its direct supertypes, as the classpath
    /// declares them.
    #[serde(default)]
    supertypes: BTreeMap<String, Vec<String>>,
}

impl JvmContext {
    /// An empty context: it resolves nothing beyond exact names.
    pub fn new() -> Self {
        Self::default()
    }

    /// `name` (a simple name or an alias) stands for `qualified`.
    pub fn with_import(mut self, name: &str, qualified: &str) -> Self {
        self.imports.insert(name.to_string(), qualified.to_string());
        self
    }

    /// `qualified` directly extends or implements `supertype`.
    pub fn with_supertype(mut self, qualified: &str, supertype: &str) -> Self {
        self.supertypes
            .entry(qualified.to_string())
            .or_default()
            .push(supertype.to_string());
        self
    }

    /// Whether the context carries no information at all.
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty() && self.supertypes.is_empty()
    }

    /// Resolve a call-site `callee` (`Type.member`) to a catalog spelling.
    ///
    /// The type is expanded through the imports, then the type and its
    /// transitive supertypes are tried in breadth-first order until `known`
    /// accepts `qualified.member`. Cycles in the declared hierarchy are
    /// tolerated. Returns `None` when nothing the context knows reaches a
    /// known API.
    pub fn resolve(&self, callee: &str, known: impl Fn(&str) -> bool) -> Option<String> {
        let (type_name, member) = callee.rsplit_once('.')?;
        let start = self
            .imports
            .get(type_name)
            .map_or(type_name, String::as_str)
            .to_string();

        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let candidate = format!("{current}.{member}");
            if known(&candidate) {
                return Some(candidate);
            }
            if let Some(parents) = self.supertypes.get(&current) {
                queue.extend(parents.iter().cloned());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(callee: &str) -> bool {
        callee == "java.sql.Statement.executeQuery"
    }

    #[test]
    fn an_import_expands_a_simple_name() {
        let ctx = JvmContext::new().with_import("Statement", "java.sql.Statement");
        assert_eq!(
            ctx.resolve("Statement.executeQuery", known).as_deref(),
            Some("java.sql.Statement.executeQuery")
        );
    }

    #[test]
    fn an_alias_resolves_like_the_type_it_names() {
        let ctx = JvmContext::new().with_import("Stmt", "java.sql.Statement");
        assert!(ctx.resolve("Stmt.executeQuery", known).is_some());
    }

    #[test]
    fn a_subtype_resolves_through_its_classpath_supertypes() {
        let ctx = JvmContext::new()
            .with_supertype("com.pool.PooledStatement", "com.pool.Base")
            .with_supertype("com.pool.Base", "java.sql.Statement");
        assert_eq!(
            ctx.resolve("com.pool.PooledStatement.executeQuery", known)
                .as_deref(),
            Some("java.sql.Statement.executeQuery")
        );
    }

    #[test]
    fn without_context_a_simple_name_stays_unresolved() {
        assert_eq!(
            JvmContext::new().resolve("Statement.executeQuery", known),
            None
        );
    }

    #[test]
    fn a_hierarchy_cycle_terminates_without_a_match() {
        let ctx = JvmContext::new()
            .with_supertype("A", "B")
            .with_supertype("B", "A");
        assert_eq!(ctx.resolve("A.executeQuery", known), None);
    }

    #[test]
    fn an_unrelated_type_never_borrows_a_known_api() {
        let ctx = JvmContext::new().with_import("Dao", "com.example.Dao");
        assert_eq!(ctx.resolve("Dao.executeQuery", known), None);
    }

    #[test]
    fn a_callee_without_a_type_part_does_not_resolve() {
        assert_eq!(JvmContext::new().resolve("executeQuery", known), None);
    }
}

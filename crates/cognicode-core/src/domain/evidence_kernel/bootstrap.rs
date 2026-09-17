//! Canonical schema bootstrap (E37 design D2).
//!
//! Pins the canonical `core:*` predicate vocabulary that fact producers use.
//! The registry port is append-only (design D6 of e36), so every process
//! entry point (runtime wiring, tests, the equivalence harness) may call
//! [`bootstrap_registry`] on a shared [`SchemaRegistry`]: re-registering an
//! IDENTICAL spec is a successful no-op, while a CONFLICTING spec (same
//! predicate, different description) is an error.
//!
//! `core:uses_generic` and `core:annotated_by` stay unregistered on purpose:
//! they have no M2 producer and join the vocabulary later, when one exists.

use super::ports::{SchemaError, SchemaRegistry};
use super::relation::{RelationKind, RelationSpec};

/// The canonical `core:*` predicate set (design D2) — exactly six, no more.
pub const CORE_RELATIONS: [&str; 6] = [
    "core:calls",
    "core:imports",
    "core:contains",
    "core:defines",
    "core:inherits",
    "core:references",
];

/// Returns the canonical spec for a canonical predicate name.
///
/// The descriptions are part of the bootstrap identity: a spec that differs
/// from these makes [`bootstrap_registry`] fail on an append-only registry,
/// forcing an explicit vocabulary decision instead of a silent overwrite.
fn canonical_spec(name: &str) -> Option<RelationSpec> {
    let description = match name {
        "core:calls" => "direct call edge from a caller symbol to the called entity name",
        "core:imports" => "import relationship from a file entity to the imported module name",
        "core:contains" => "containment between a file entity and a symbol defined in it",
        "core:defines" => {
            "definition record pinning a symbol entity to its fully-qualified identity"
        }
        "core:inherits" => "type-hierarchy parent edge from a type entity to its parent name",
        "core:references" => "name/type reference from a symbol entity to the referenced name",
        _ => return None,
    };
    Some(RelationSpec::new(description))
}

/// Registers the canonical [`CORE_RELATIONS`] set into `registry`
/// (design D2).
///
/// Idempotent: a predicate already registered with the IDENTICAL canonical
/// spec is accepted (success), so repeated bootstrap calls are safe on the
/// append-only vocabulary. A predicate registered with a CONFLICTING spec
/// fails with [`SchemaError::AlreadyRegistered`] and stops the bootstrap.
pub fn bootstrap_registry(registry: &dyn SchemaRegistry) -> Result<(), SchemaError> {
    for name in CORE_RELATIONS {
        let kind = RelationKind::try_new(name).expect("canonical predicate is a valid 'ns:name'");
        let spec = canonical_spec(name).expect("canonical predicate has a canonical spec");
        match registry.lookup(&kind) {
            Some(existing) if existing == spec => {} // identical: idempotent success
            Some(_existing) => return Err(SchemaError::AlreadyRegistered(kind)),
            None => registry.register(kind, spec)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CORE_RELATIONS, bootstrap_registry, canonical_spec};
    use crate::domain::evidence_kernel::ports::{SchemaError, SchemaRegistry};
    use crate::domain::evidence_kernel::relation::{RelationKind, RelationSpec};
    use std::sync::Mutex;

    /// Test-only in-process implementation of the domain [`SchemaRegistry`]
    /// port. Lives inside `mod tests` so the domain layer does not depend on
    /// `infrastructure::InMemorySchemaRegistry` from a test target (P1.1).
    /// The semantics intentionally mirror the production in-memory adapter:
    /// the vocabulary is append-only, `register` rejects re-registration,
    /// and `list` returns entries in deterministic (insertion) order.
    struct TestSchemaRegistry {
        entries: Mutex<Vec<(RelationKind, RelationSpec)>>,
    }

    impl TestSchemaRegistry {
        fn new() -> Self {
            Self {
                entries: Mutex::new(Vec::new()),
            }
        }
    }

    impl SchemaRegistry for TestSchemaRegistry {
        fn register(&self, k: RelationKind, s: RelationSpec) -> Result<(), SchemaError> {
            let mut entries = self.entries.lock().expect("test mutex poisoned");
            if entries.iter().any(|(kind, _)| kind == &k) {
                return Err(SchemaError::AlreadyRegistered(k));
            }
            entries.push((k, s));
            Ok(())
        }

        fn lookup(&self, k: &RelationKind) -> Option<RelationSpec> {
            self.entries
                .lock()
                .expect("test mutex poisoned")
                .iter()
                .find(|(kind, _)| kind == k)
                .map(|(_, spec)| spec.clone())
        }

        fn list(&self) -> Vec<(RelationKind, RelationSpec)> {
            self.entries.lock().expect("test mutex poisoned").clone()
        }
    }

    /// The bootstrap must be idempotent: two consecutive calls succeed, and
    /// the vocabulary is exactly the six canonical predicates afterwards.
    #[test]
    fn bootstrap_registry_is_idempotent_and_registers_exactly_six() {
        let registry = TestSchemaRegistry::new();
        bootstrap_registry(&registry).expect("first bootstrap");
        bootstrap_registry(&registry).expect("second bootstrap is an idempotent no-op");

        let listed = registry.list();
        assert_eq!(listed.len(), 6, "exactly the six canonical predicates");
        for (kind, spec) in &listed {
            assert_eq!(kind.ns(), "core");
            assert_eq!(
                Some(spec),
                canonical_spec(kind.as_str()).as_ref(),
                "registered spec must be the canonical one"
            );
        }
        let names: Vec<&str> = listed.iter().map(|(k, _)| k.as_str()).collect();
        for name in CORE_RELATIONS {
            assert!(names.contains(&name), "{name} missing from vocabulary");
        }
    }

    /// Re-registering an identical spec before bootstrap is a success (the
    /// idempotency contract is symmetric).
    #[test]
    fn bootstrap_accepts_pre_registered_identical_specs() {
        let registry = TestSchemaRegistry::new();
        registry
            .register(
                RelationKind::try_new("core:calls").expect("valid kind"),
                canonical_spec("core:calls").expect("canonical"),
            )
            .expect("pre-registration");
        bootstrap_registry(&registry).expect("identical spec is accepted");
        assert_eq!(registry.list().len(), 6);
    }

    /// A conflicting spec for a canonical predicate must fail the bootstrap
    /// with `AlreadyRegistered` (append-only vocabulary, design D2).
    #[test]
    fn bootstrap_rejects_conflicting_specs() {
        let registry = TestSchemaRegistry::new();
        registry
            .register(
                RelationKind::try_new("core:calls").expect("valid kind"),
                RelationSpec::new("a different description"),
            )
            .expect("conflicting pre-registration");
        let err =
            bootstrap_registry(&registry).expect_err("conflicting spec must fail the bootstrap");
        assert_eq!(
            err,
            SchemaError::AlreadyRegistered(RelationKind::try_new("core:calls").expect("valid"))
        );
    }

    /// Non-canonical predicates stay unregistered: `core:uses_generic` and
    /// `core:annotated_by` have no M2 producer (design D2).
    #[test]
    fn bootstrap_leaves_non_canonical_predicates_unregistered() {
        let registry = TestSchemaRegistry::new();
        bootstrap_registry(&registry).expect("bootstrap");
        for name in ["core:uses_generic", "core:annotated_by"] {
            let kind = RelationKind::try_new(name).expect("valid kind");
            assert!(
                registry.lookup(&kind).is_none(),
                "{name} must stay unregistered"
            );
        }
    }

    /// `CORE_RELATIONS` is exactly six distinct `core:*` names (design D2).
    #[test]
    fn core_relations_constant_is_the_six_predicate_set() {
        assert_eq!(CORE_RELATIONS.len(), 6);
        let mut sorted = CORE_RELATIONS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 6, "no duplicates");
        for name in CORE_RELATIONS {
            assert!(name.starts_with("core:"));
            assert!(canonical_spec(name).is_some(), "{name} needs a spec");
        }
    }
}

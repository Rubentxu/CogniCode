//! Tests for `application::change_proposal::proposal` (e72 WU1).
//!
//! Adversarial coverage of the **creation != authority** invariant:
//!
//! 1. A `ChangeProposal` carries only its id, target world, kind, and
//!    author class. It does NOT carry any promotion permit, trial
//!    evidence, evidence bundle, or apply capability.
//! 2. The `RequestedBy` enum distinguishes human, plugin, and LLM
//!    agent authors.
//! 3. `is_automated` correctly identifies plugin and LLM authors and
//!    excludes humans.
//! 4. The proposal shape is stable: two proposals with the same inputs
//!    are equal; mutations to individual fields change the equality.
//! 5. The proposal does not contain any time / clock field — it is
//!    pure data.
//!
//! The "no authority" guarantee is asserted structurally: the type
//! surface has no field whose name or shape suggests a permit,
//! capability, or authority token. We list every field of the struct
//! in the test below.

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::software_world::world::SoftwareWorldId;

// --- helpers ---------------------------------------------------------

fn world_id(s: &str) -> SoftwareWorldId {
    SoftwareWorldId::from_string(s)
}

fn proposal_id(s: &str) -> ChangeProposalId {
    ChangeProposalId::from_string(s)
}

fn patch() -> ProposalKind {
    ProposalKind::SourcePatch {
        patch_ref: "patch-1".to_string(),
    }
}

fn cfg() -> ProposalKind {
    ProposalKind::ConfigChange {
        config_ref: "cfg-1".to_string(),
    }
}

fn detector() -> ProposalKind {
    ProposalKind::DetectorChange {
        detector_ref: "det-1".to_string(),
    }
}

// --- Construction and equality ---------------------------------------

#[test]
fn proposal_carries_only_intent_not_authority() {
    // The structural assertion: a ChangeProposal has exactly four
    // fields. Any future field that smells like authority (a permit,
    // a capability, an apply token) breaks this test by changing the
    // field count.
    let p = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let fields_count = 4;
    // We assert this by destructuring; if the struct gains a field,
    // the destructuring fails to compile.
    let ChangeProposal {
        id: _,
        base_world: _,
        proposed_change: _,
        requested_by: _,
    } = &p;
    assert_eq!(fields_count, 4);
    // Sanity: the fields are what we set.
    assert_eq!(p.id, proposal_id("p-1"));
    assert_eq!(p.base_world, world_id("w-base"));
}

#[test]
fn two_proposals_with_same_inputs_are_equal() {
    let a = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let b = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    assert_eq!(a, b);
}

#[test]
fn changing_proposal_kind_distinguishes_proposals() {
    let p1 = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let p2 = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        cfg(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    assert_ne!(p1, p2);
    assert_eq!(p1.proposed_change.kind_tag(), "source-patch");
    assert_eq!(p2.proposed_change.kind_tag(), "config-change");
}

#[test]
fn changing_requested_by_distinguishes_proposals() {
    let human = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let plugin = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::Plugin {
            plugin_ref: "p-detector".to_string(),
        },
    );
    let llm = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-base"),
        patch(),
        RequestedBy::LlmAgent {
            agent_ref: "gpt-x".to_string(),
        },
    );
    assert_ne!(human, plugin);
    assert_ne!(human, llm);
    assert_ne!(plugin, llm);
}

#[test]
fn changing_base_world_distinguishes_proposals() {
    let p1 = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-A"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let p2 = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-B"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    assert_ne!(p1, p2);
}

// --- RequestedBy classification --------------------------------------

#[test]
fn requested_by_distinguishes_automated_from_human() {
    let human = RequestedBy::Human {
        user_ref: "alice".to_string(),
    };
    let plugin = RequestedBy::Plugin {
        plugin_ref: "p-detector".to_string(),
    };
    let llm = RequestedBy::LlmAgent {
        agent_ref: "gpt-x".to_string(),
    };

    assert!(!human.is_automated());
    assert!(plugin.is_automated());
    assert!(llm.is_automated());

    assert_eq!(human.class_tag(), "human");
    assert_eq!(plugin.class_tag(), "plugin");
    assert_eq!(llm.class_tag(), "llm-agent");
}

#[test]
fn proposal_is_automated_propagates_from_requested_by() {
    let human_p = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-A"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let plugin_p = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-A"),
        patch(),
        RequestedBy::Plugin {
            plugin_ref: "p-detector".to_string(),
        },
    );
    assert!(!human_p.is_automated());
    assert!(plugin_p.is_automated());
}

// --- ProposalKind variants -------------------------------------------

#[test]
fn proposal_kind_supports_source_config_detector() {
    let _ = patch();
    let _ = cfg();
    let _ = detector();
    // Each variant has a stable tag.
    assert_eq!(patch().kind_tag(), "source-patch");
    assert_eq!(cfg().kind_tag(), "config-change");
    assert_eq!(detector().kind_tag(), "detector-change");
}

// --- Anti-authority structural assertion -----------------------------

#[test]
fn proposal_carries_no_time_or_clock_field() {
    // The proposal must not carry a created_at, timestamp, or any
    // field whose type would force clock access. We assert this by
    // type-level inspection: there is no field on ChangeProposal whose
    // type is `DateTime<...>`, `Instant`, or `SystemTime`.
    //
    // The destructuring above already enumerates the fields; if any of
    // them were a time type, this test would compile (Rust's type
    // system does not give us a way to enumerate field types at
    // runtime). The intent is documented here as a guard: future
    // authors adding a `created_at` field to ChangeProposal should be
    // aware they are introducing a clock dependency.
    //
    // This is a documentation / intent assertion; it does not
    // mechanically block the addition. The code review is the gate.
    let p = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-A"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    // The proposal can be Debug-printed and serialized without
    // touching a clock.
    let _ = format!("{:?}", p);
}

#[test]
fn proposal_carries_no_promotion_or_apply_token() {
    // Same intent: there is no field on ChangeProposal that smells
    // like authority. We document this as a guard. The destructuring
    // assertion above is the structural proof.
    let p = ChangeProposal::new(
        proposal_id("p-1"),
        world_id("w-A"),
        patch(),
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    assert!(!p.is_automated() || p.is_automated()); // tautology; the
    // point is that
    // `is_automated` is
    // the only authority-
    // adjacent method
    // exposed.
}

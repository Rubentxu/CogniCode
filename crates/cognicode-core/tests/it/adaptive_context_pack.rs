//! Adaptive context selection over the storage fixture: what a pack shows of
//! each symbol, which symbols it picks, how it reports what it could not use
//! and how it stops.

use std::collections::BTreeSet;
use std::sync::Arc;

use cognicode_core::application::context_pack::{
    Adaptive, ContextPack, Detail, Expansion, ExpansionStatus, Interrupt, Purpose, Role,
    SelectionPolicy, Stop, Weights, WorkBudget,
};
use cognicode_core::domain::findings::digest::sha256_hex;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::DependencyType;

use super::adaptive_pack_storage::{HELPERS, storage_symbols, storage_world};
use super::adaptive_pack_world::World;

/// A budget that fits the seed and a handful of neighbours, not the world.
const TIGHT: u64 = 1500;

const INTENT: &str = "storage save";

fn adaptive(mode: Purpose) -> Adaptive {
    Adaptive {
        policy: SelectionPolicy::for_mode(mode),
        budget: WorkBudget::default(),
    }
}

fn anchored(world: &World, adaptive: Option<Adaptive>, budget: u64) -> ContextPack {
    world.pack(&["file_save"], Some(INTENT), adaptive, budget)
}

fn roles(pack: &ContextPack) -> BTreeSet<Role> {
    pack.items
        .iter()
        .filter_map(|item| item.selection.as_ref().map(|s| s.role))
        .collect()
}

fn helpers_in(world: &World, pack: &ContextPack) -> usize {
    world
        .keys(pack)
        .iter()
        .filter(|key| HELPERS.contains(&key.as_str()))
        .count()
}

// -- the selection: which symbols, and not the same one many times ----------

#[test]
fn a_tight_pack_covers_the_contract_an_implementation_and_a_user_of_the_seed() {
    let world = storage_world();

    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), TIGHT);

    let keys = world.keys(&pack);
    assert_eq!(keys[0], "file_save", "the named symbol goes first");
    let roles = roles(&pack);
    assert!(roles.contains(&Role::Interface), "{keys:?}");
    assert!(roles.contains(&Role::Implementation), "{keys:?}");
    assert!(
        roles.contains(&Role::Consumer) || roles.contains(&Role::Test),
        "{keys:?}"
    );
    assert!(pack.budget.verified, "the budget holds");
}

#[test]
fn the_helpers_of_one_class_do_not_crowd_out_the_other_roles() {
    let world = storage_world();

    let adaptive_pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), TIGHT);
    let mut no_diversity = adaptive(Purpose::Diagnose);
    no_diversity.policy.diversity = 0.0;
    no_diversity.policy.role_bonus = 0.0;
    let greedy_pack = anchored(&world, Some(no_diversity), TIGHT);
    let legacy = anchored(&world, None, TIGHT);

    let adaptive_helpers = helpers_in(&world, &adaptive_pack);
    assert!(
        adaptive_helpers < helpers_in(&world, &greedy_pack),
        "ranking by value alone repeats the helpers: {:?} vs {:?}",
        world.keys(&greedy_pack),
        world.keys(&adaptive_pack)
    );
    assert!(adaptive_helpers < HELPERS.len());
    assert!(
        roles(&adaptive_pack).len() > roles(&greedy_pack).len() || roles(&adaptive_pack).len() >= 4,
        "the adaptive pack covers more roles"
    );
    assert!(legacy.adaptive.is_none());
}

#[test]
fn an_item_says_why_it_is_where_it_is_without_calling_it_confidence() {
    let world = storage_world();

    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), TIGHT);

    let report = pack.adaptive.as_ref().expect("an adaptive pack reports");
    assert!(report.score_semantics.contains("not a confidence"));
    for item in &pack.items {
        let selection = item.selection.as_ref().expect("every item says its role");
        assert!((0.0..=1.0).contains(&selection.value));
        assert!((0.0..=1.0).contains(&selection.redundancy));
    }
    let json = serde_json::to_string(&pack).unwrap();
    assert!(!json.contains("\"confidence\""));
}

// -- the representations: one parser, the original ranges -------------------

fn lines_of(source: &str, start: u32, end: u32) -> String {
    source
        .lines()
        .skip(start as usize - 1)
        .take((end - start + 1) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_representation_quotes_the_source_at_the_original_lines() {
    let world = storage_world();
    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), 4000);

    let mut seen = BTreeSet::new();
    for item in &pack.items {
        let source = world.text(&item.file);
        let shown = item.representation.as_ref().expect("an adaptive item");
        seen.insert(shown.level);
        for segment in &shown.segments {
            assert_eq!(
                segment.text,
                lines_of(source, segment.start_line, segment.end_line),
                "{} quotes lines {}-{} verbatim",
                item.name,
                segment.start_line,
                segment.end_line
            );
            assert!(segment.start_line >= item.range.start_line);
            assert!(segment.end_line <= item.range.end_line);
        }
        assert_eq!(
            item.source_digest,
            format!("sha256:{}", sha256_hex(source.as_bytes()))
        );
    }
    assert!(
        seen.contains(&Detail::Full) && seen.contains(&Detail::Slice),
        "{seen:?}: the fixture exercises full, slice and signature"
    );
    assert!(seen.contains(&Detail::Signature));
}

#[test]
fn a_signature_is_a_header_the_file_has_and_a_slice_keeps_it() {
    let world = storage_world();
    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), 4000);

    for item in &pack.items {
        let source = world.text(&item.file);
        let header = item.signature.trim();
        assert!(!header.is_empty());
        assert!(
            source.contains(header),
            "`{header}` is not written in {}: a signature must not be invented",
            item.file
        );
        let shown = item.representation.as_ref().unwrap();
        if shown.level != Detail::Signature {
            let first = &shown.segments[0];
            assert_eq!(first.start_line, item.range.start_line);
            assert!(
                first
                    .text
                    .trim_start()
                    .starts_with(header.lines().next().unwrap()),
                "{}: the slice starts with its declaration",
                item.name
            );
        } else {
            assert!(shown.segments.is_empty());
        }
    }
}

#[test]
fn a_slice_and_its_elisions_account_for_every_line_of_the_declaration() {
    let world = storage_world();
    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), 4000);

    let slices: Vec<_> = pack
        .items
        .iter()
        .filter(|item| item.representation.as_ref().unwrap().level == Detail::Slice)
        .collect();
    assert!(!slices.is_empty());
    for item in slices {
        let shown = item.representation.as_ref().unwrap();
        let mut covered: Vec<u32> = Vec::new();
        for segment in &shown.segments {
            covered.extend(segment.start_line..=segment.end_line);
        }
        for gap in &shown.elided {
            covered.extend(gap.start_line..=gap.end_line);
        }
        covered.sort_unstable();
        let expected: Vec<u32> = (item.range.start_line..=item.range.end_line).collect();
        assert_eq!(
            covered, expected,
            "{}: no line is lost or repeated",
            item.name
        );
    }
}

#[test]
fn a_named_contract_is_shown_whole() {
    let world = storage_world();

    let pack = world.pack(
        &["storage"],
        Some(INTENT),
        Some(adaptive(Purpose::Diagnose)),
        4000,
    );

    let contract = &pack.items[0];
    assert_eq!(world.key_of(&contract.id), "storage");
    assert!(contract.range.end_line > contract.range.start_line);
    let shown = contract.representation.as_ref().unwrap();
    assert_eq!(shown.level, Detail::Full, "the named symbol is shown whole");
    assert!(shown.segments[0].text.contains("fn save"));
}

#[test]
fn the_requested_detail_replaces_the_one_the_role_would_get() {
    let world = storage_world();
    let mut signatures = adaptive(Purpose::Diagnose);
    signatures.policy.detail_override = Some(Detail::Signature);

    let pack = anchored(&world, Some(signatures), TIGHT);

    assert!(
        pack.items
            .iter()
            .all(|item| item.representation.as_ref().unwrap().level == Detail::Signature)
    );
}

// -- the modes: declared, traceable, reproducible ---------------------------

#[test]
fn each_mode_publishes_the_weights_it_ranked_with() {
    let world = storage_world();

    let weights: Vec<Weights> = [Purpose::Diagnose, Purpose::Refactor, Purpose::Architecture]
        .into_iter()
        .map(|mode| {
            let pack = anchored(&world, Some(adaptive(mode)), TIGHT);
            let report = pack.adaptive.unwrap();
            assert_eq!(report.policy.mode, Some(mode));
            assert_eq!(report.weights_applied, report.policy.weights);
            assert!(pack.rendered.contains(&format!("mode={}", mode.code())));
            report.weights_applied
        })
        .collect();

    assert_ne!(weights[0], weights[1]);
    assert_ne!(weights[1], weights[2]);
    assert_ne!(weights[0], weights[2]);
}

#[test]
fn the_same_request_over_the_same_snapshot_gives_the_same_pack() {
    for mode in [Purpose::Diagnose, Purpose::Refactor, Purpose::Architecture] {
        let first = anchored(&storage_world(), Some(adaptive(mode)), TIGHT);
        let second = anchored(&storage_world(), Some(adaptive(mode)), TIGHT);

        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap(),
            "{mode:?}"
        );
    }
}

#[test]
fn a_world_without_relations_degrades_to_the_name_and_says_so() {
    let world = storage_symbols().build();
    let mut walk = adaptive(Purpose::Diagnose);
    walk.policy.expansion = Expansion::Always;

    let pack = world.pack(&[], Some(INTENT), Some(walk), TIGHT);

    let report = pack.adaptive.as_ref().unwrap();
    assert_eq!(report.expansion.status, ExpansionStatus::Unknown);
    assert_eq!(report.degraded_to.as_deref(), Some("lexical"));
    assert_eq!(report.weights_applied, Weights::LEXICAL_ONLY);
    assert!(!report.gaps.is_empty(), "the gap is reported");
    assert!(pack.rendered.contains("degraded to lexical"));
    assert!(!pack.items.is_empty(), "the name still finds symbols");
    for item in &pack.items {
        let role = item.selection.as_ref().unwrap().role;
        assert!(
            !matches!(
                role,
                Role::Interface | Role::Implementation | Role::Consumer | Role::Test
            ),
            "{}: a relation the graph does not hold was invented ({role:?})",
            item.name
        );
    }
}

#[test]
fn a_relation_the_extractor_could_not_resolve_is_not_believed() {
    let world = storage_symbols()
        .edge_with(
            "backup_job",
            DependencyType::Calls,
            "file_save",
            ExtractionContext::Unresolved,
        )
        .edge_with(
            "memory",
            DependencyType::Inherits,
            "storage",
            ExtractionContext::Unresolved,
        )
        .build();

    let pack = anchored(&world, Some(adaptive(Purpose::Diagnose)), TIGHT);

    let report = pack.adaptive.as_ref().unwrap();
    assert_eq!(report.expansion.status, ExpansionStatus::Unknown);
    assert!(report.expansion.edges_ignored >= 1);
    assert_eq!(report.degraded_to.as_deref(), Some("lexical"));
    let keys = world.keys(&pack);
    assert!(!keys.contains(&"backup_job".to_string()), "{keys:?}");
}

// -- the work: abstain, bound, cancel ---------------------------------------

#[derive(Debug)]
struct Stopped(Stop);

impl Interrupt for Stopped {
    fn poll(&self) -> Option<Stop> {
        Some(self.0)
    }
}

fn with_budget(mode: Purpose, max_visits: usize, interrupt: Option<Stop>) -> Adaptive {
    let mut request = adaptive(mode);
    request.budget.max_visits = max_visits;
    if let Some(stop) = interrupt {
        request.budget.interrupt = Arc::new(Stopped(stop));
    }
    request
}

#[test]
fn a_simple_lookup_does_not_expand_the_graph() {
    let world = storage_world();

    let pack = world.pack(&[], Some("save"), Some(adaptive(Purpose::Diagnose)), TIGHT);

    let report = pack.adaptive.as_ref().unwrap();
    assert_eq!(report.expansion.status, ExpansionStatus::Abstained);
    assert_eq!(report.expansion.visited, 0);
    assert!(pack.rendered.contains("expansion=abstained"));
    assert!(
        roles(&pack)
            .iter()
            .all(|role| matches!(role, Role::Seed | Role::Related)),
        "no relation was walked"
    );
}

#[test]
fn a_richer_request_expands_and_a_policy_can_forbid_it() {
    let world = storage_world();
    let mut never = adaptive(Purpose::Diagnose);
    never.policy.expansion = Expansion::Never;

    let expanded = anchored(&world, Some(adaptive(Purpose::Diagnose)), TIGHT);
    let forbidden = anchored(&world, Some(never), TIGHT);

    assert_eq!(
        expanded.adaptive.unwrap().expansion.status,
        ExpansionStatus::Expanded
    );
    assert_eq!(
        forbidden.adaptive.unwrap().expansion.status,
        ExpansionStatus::Abstained
    );
}

#[test]
fn a_visit_budget_stops_the_walk_and_the_pack_says_it_is_partial() {
    let world = storage_world();

    let pack = anchored(&world, Some(with_budget(Purpose::Diagnose, 3, None)), TIGHT);

    let report = pack.adaptive.as_ref().unwrap();
    assert_eq!(report.expansion.status, ExpansionStatus::Partial);
    assert!(report.expansion.visited <= 3);
    assert_eq!(report.expansion.max_visits, 3);
    assert!(pack.rendered.contains("expansion=partial"));
}

#[test]
fn a_deadline_is_partial_a_cancellation_is_cancelled_and_neither_is_complete() {
    let world = storage_world();

    let late = anchored(
        &world,
        Some(with_budget(
            Purpose::Diagnose,
            100,
            Some(Stop::DeadlineExceeded),
        )),
        TIGHT,
    );
    let cancelled = anchored(
        &world,
        Some(with_budget(Purpose::Diagnose, 100, Some(Stop::Cancelled))),
        TIGHT,
    );
    let unknown = storage_symbols().build().pack(
        &[],
        Some(INTENT),
        Some({
            let mut walk = adaptive(Purpose::Diagnose);
            walk.policy.expansion = Expansion::Always;
            walk
        }),
        TIGHT,
    );

    let statuses = [
        late.adaptive.as_ref().unwrap().expansion.status,
        cancelled.adaptive.as_ref().unwrap().expansion.status,
        unknown.adaptive.as_ref().unwrap().expansion.status,
    ];
    assert_eq!(
        statuses,
        [
            ExpansionStatus::Partial,
            ExpansionStatus::Cancelled,
            ExpansionStatus::Unknown
        ]
    );
    assert!(late.rendered.contains("time budget"));
    assert!(cancelled.rendered.contains("expansion=cancelled"));
    assert_eq!(
        world.keys(&late)[0],
        "file_save",
        "the seed still goes first"
    );
}

#[test]
fn a_bounded_walk_keeps_the_identity_of_every_piece_of_evidence() {
    let world = storage_world();
    let full = anchored(&world, Some(adaptive(Purpose::Diagnose)), 4000);
    let bounded = anchored(&world, Some(with_budget(Purpose::Diagnose, 3, None)), 4000);

    for item in &bounded.items {
        let source = world.text(&item.file);
        assert_eq!(
            item.source_digest,
            format!("sha256:{}", sha256_hex(source.as_bytes()))
        );
        if let Some(twin) = full.items.iter().find(|other| other.id == item.id) {
            assert_eq!(item.range, twin.range);
            assert_eq!(item.signature, twin.signature);
            assert_eq!(item.source_digest, twin.source_digest);
        }
    }
}

// -- the old contract -------------------------------------------------------

#[test]
fn a_plain_request_keeps_the_plain_answer() {
    let world = storage_world();

    let pack = anchored(&world, None, TIGHT);

    assert!(pack.adaptive.is_none());
    assert!(
        pack.items
            .iter()
            .all(|item| item.representation.is_none() && item.selection.is_none())
    );
    let json = serde_json::to_value(&pack).unwrap();
    assert!(json.get("adaptive").is_none());
    assert!(json["items"][0].get("representation").is_none());
    assert!(!pack.rendered.contains("selection:"));
}

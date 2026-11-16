//! CGH-027 (CAA-09): cost-aware planning with single-flight and safe
//! cancellation.
//!
//! Each test names the AC it pins:
//! #1 three concurrent requests for the same snapshot/capabilities
//!    produce ONE physical computation;
//! #2 cancelling an agent revision does not invalidate CI work on
//!    another revision, and does not release shared referenced
//!    objects;
//! #3 a required check over budget returns Inconclusive with
//!    explicit metrics (and the estimate is measure-based);
//! #4 daemon-vs-direct cost comparison; an absent daemon never
//!    produces a PASS and never blocks unboundedly.
//! Plus: bounded, CI-fair admission.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cognicode_core::application::change_tracking::planner::WorkId;
use cognicode_core::application::change_tracking::scheduling::Coordinator;
use cognicode_core::application::change_tracking::scheduling::{
    CheckOutput, Computed, ConsumerClass, CostEstimator, CostMetrics, DaemonProbe, ObjectId,
    QueuePolicy, RevisionTag, Route, RunOutcome, RunStatus, ScheduleRequest, WorkKey,
};
use cognicode_core::domain::budgets::BudgetDeclaration;
use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
use cognicode_core::domain::findings::detector_ir::AnalysisCapability;
use cognicode_core::domain::naming::NamespacedName;

fn work(name: &str) -> WorkId {
    WorkId::new(NamespacedName::new(name).expect("valid namespace.name"))
}

fn key(name: &str, snapshot: u64) -> WorkKey {
    WorkKey::new(
        work(name),
        SnapshotId::new(snapshot),
        [AnalysisCapability::AstPattern],
    )
}

fn request(
    name: &str,
    consumer: ConsumerClass,
    revision: &str,
    required: bool,
    direct: u64,
) -> ScheduleRequest {
    ScheduleRequest {
        consumer,
        key: key(name, 7),
        revision: RevisionTag::new(revision),
        required,
        direct_units: direct,
    }
}

fn coordinator(capacity: usize, ci_reserved: usize, effects: Option<u64>) -> Coordinator {
    let declaration = match effects {
        Some(n) => BudgetDeclaration::effects(NonZeroU64::new(n).expect("non-zero")),
        None => BudgetDeclaration::none(),
    };
    Coordinator::new(
        QueuePolicy {
            capacity,
            ci_reserved,
        },
        declaration,
        Arc::new(CostEstimator::new()),
    )
}

#[test]
fn ac1_same_identity_shares_one_physical_computation() {
    let subject = Arc::new(coordinator(4, 1, None));
    let computations = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
    let hold = Arc::new(Mutex::new(hold_rx));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let subject = Arc::clone(&subject);
        let computations = Arc::clone(&computations);
        let start_tx = start_tx.clone();
        let hold = Arc::clone(&hold);
        handles.push(std::thread::spawn(move || {
            subject.run(
                &request("detect.xss", ConsumerClass::Agent, "rev-a", true, 1),
                DaemonProbe::ABSENT,
                |_| {
                    computations.fetch_add(1, Ordering::SeqCst);
                    start_tx.send(()).expect("leader started");
                    hold.lock()
                        .expect("hold lock")
                        .recv()
                        .expect("leader released");
                    Ok(Computed {
                        output: CheckOutput::Pass(vec!["finding-1"]),
                        measured_units: 1,
                    })
                },
            )
        }));
    }
    drop(start_tx);
    start_rx.recv().expect("one leader ran");
    std::thread::sleep(std::time::Duration::from_millis(50));
    hold_tx.send(()).expect("release leader");
    let outcomes: Vec<RunOutcome<Vec<&str>>> = handles
        .into_iter()
        .map(|h| h.join().expect("worker"))
        .collect();
    assert_eq!(
        computations.load(Ordering::SeqCst),
        1,
        "single-flight: one physical computation"
    );
    for outcome in &outcomes {
        assert_eq!(outcome.status, RunStatus::Pass(vec!["finding-1"]));
    }
    assert_eq!(outcomes.iter().filter(|o| !o.joined).count(), 1);
    let metrics = subject.metrics();
    assert_eq!(metrics.physical_computations, 1);
    assert_eq!(metrics.requests_joined, 2);
}

#[test]
fn ac2_cancelling_agent_revision_spares_ci_and_shared_objects() {
    let subject = Arc::new(coordinator(4, 1, None));
    let agent_rev = RevisionTag::new("agent-rev");
    let ci_rev = RevisionTag::new("ci-rev");
    let shared = ObjectId::new("index:segment-shared");
    let agent_only = ObjectId::new("index:segment-agent");
    subject.register_objects(&agent_rev, &[shared.clone(), agent_only.clone()]);
    subject.register_objects(&ci_rev, std::slice::from_ref(&shared));

    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let running = Arc::new(AtomicBool::new(false));
    let agent_subject = Arc::clone(&subject);
    let agent_running = Arc::clone(&running);
    let agent = std::thread::spawn(move || {
        agent_subject.run(
            &request("detect.xss", ConsumerClass::Agent, "agent-rev", true, 1),
            DaemonProbe::ABSENT,
            |flag| {
                agent_running.store(true, Ordering::SeqCst);
                start_tx.send(()).expect("agent leader running");
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Ok(Computed {
                    output: CheckOutput::Pass("agent stale"),
                    measured_units: 1,
                })
            },
        )
    });
    start_rx.recv().expect("agent leader started");

    // CI work on a different revision runs while the agent leader is cancelled.
    let ci_outcome = subject.run(
        &request("detect.sqli", ConsumerClass::Ci, "ci-rev", true, 1),
        DaemonProbe::ABSENT,
        |_| {
            Ok(Computed {
                output: CheckOutput::Pass("ci result"),
                measured_units: 1,
            })
        },
    );

    let report = subject.cancel_revision(&agent_rev);
    assert_eq!(
        report.cancelled, 1,
        "only the agent revision's flag is cancelled"
    );
    assert_eq!(
        ci_outcome.status,
        RunStatus::Pass("ci result"),
        "CI work is not invalidated"
    );
    assert_eq!(
        report.released_objects,
        vec![agent_only],
        "shared objects survive"
    );
    assert_eq!(subject.object_revisions(&shared), vec![ci_rev.clone()]);

    let agent_outcome: RunOutcome<&str> = agent.join().expect("agent thread");
    assert_eq!(
        agent_outcome.status,
        RunStatus::Cancelled,
        "stale candidate result is discarded"
    );
    assert!(running.load(Ordering::SeqCst));
}

#[test]
fn ac3_required_over_budget_returns_inconclusive_with_explicit_measured_metrics() {
    let declaration = BudgetDeclaration::effects(NonZeroU64::new(10).expect("non-zero"));
    let estimator = Arc::new(CostEstimator::new());
    let subject = Coordinator::new(
        QueuePolicy {
            capacity: 4,
            ci_reserved: 1,
        },
        declaration,
        Arc::clone(&estimator),
    );

    // The estimate is measure-based: two observations of 15 units each.
    let item = work("detect.xss");
    estimator.observe(&item, 15);
    estimator.observe(&item, 15);
    let measured = estimator.estimate_or(&item, u64::MAX);
    assert_eq!(measured, 15);

    let ran = AtomicBool::new(false);
    let outcome = subject.run(
        &ScheduleRequest {
            consumer: ConsumerClass::Ci,
            key: key("detect.xss", 7),
            revision: RevisionTag::new("rev-a"),
            required: true,
            direct_units: 2,
        },
        DaemonProbe::ABSENT,
        |_| {
            ran.store(true, Ordering::SeqCst);
            Ok(Computed {
                output: CheckOutput::Pass("computed"),
                measured_units: 1,
            })
        },
    );
    match outcome.status {
        RunStatus::Inconclusive(
            cognicode_core::application::change_tracking::scheduling::InconclusiveReason::BudgetExhausted {
                attempted,
                remaining,
            },
        ) => {
            assert_eq!(attempted, 15);
            assert_eq!(remaining, 10);
        }
        other => panic!("expected budget Inconclusive, got {other:?}"),
    }
    assert!(
        !ran.load(Ordering::SeqCst),
        "over-budget required work must never compute"
    );
    let CostMetrics {
        estimated_units,
        spent_units,
        remaining_units,
        physical_computations,
        ..
    } = outcome.metrics;
    assert_eq!(
        (
            estimated_units,
            spent_units,
            remaining_units,
            physical_computations
        ),
        (15, 0, 10, 0)
    );
}

#[test]
fn ac4_warm_daemon_is_preferred_and_absent_daemon_never_passes_nor_blocks() {
    // Warm daemon reuse (3 units) beats direct (9) under a 5-unit budget.
    let warm_subject = coordinator(4, 1, Some(5));
    let warm = warm_subject.run(
        &request("detect.xss", ConsumerClass::Ci, "rev-a", true, 9),
        DaemonProbe::warm(3),
        |_| {
            Ok(Computed {
                output: CheckOutput::Pass("from warm index"),
                measured_units: 1,
            })
        },
    );
    assert_eq!(warm.route, Route::Daemon);
    assert_eq!(warm.status, RunStatus::Pass("from warm index"));
    assert_eq!(
        warm.metrics.spent_units, 3,
        "charged the daemon reuse cost, not the direct cost"
    );

    // Same budget, daemon absent: direct is 9 > 5, so the required
    // check is Inconclusive — the absence never fabricates a PASS —
    // and the call returns synchronously (bounded, no waiting).
    let cold_subject = coordinator(4, 1, Some(5));
    let started = std::time::Instant::now();
    let cold: RunOutcome<&str> = cold_subject.run(
        &request("detect.xss", ConsumerClass::Ci, "rev-a", true, 9),
        DaemonProbe::ABSENT,
        |_| {
            Ok(Computed {
                output: CheckOutput::Pass("must not run"),
                measured_units: 1,
            })
        },
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "absent daemon must not block"
    );
    assert!(
        !matches!(cold.status, RunStatus::Pass(_)),
        "daemon absence never yields a PASS"
    );
    assert_eq!(cold.route, Route::Direct);

    // A pricier warm daemon loses to direct execution.
    let expensive = coordinator(4, 1, None);
    let chosen = expensive.run(
        &request("detect.xss", ConsumerClass::Ci, "rev-a", true, 9),
        DaemonProbe::warm(12),
        |_| {
            Ok(Computed {
                output: CheckOutput::Pass("direct"),
                measured_units: 1,
            })
        },
    );
    assert_eq!(chosen.route, Route::Direct);
}

#[test]
fn admission_is_bounded_and_guarantees_ci_capacity_under_agent_flood() {
    // capacity 3, two slots reserved for CI: agents top out at one.
    let subject = Arc::new(coordinator(3, 2, None));
    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
    let hold = Arc::new(Mutex::new(hold_rx));
    let flooding = Arc::clone(&subject);
    let hold_for_agent = Arc::clone(&hold);
    let flood = std::thread::spawn(move || {
        flooding.run(
            &request("detect.flood", ConsumerClass::Agent, "rev-a", true, 1),
            DaemonProbe::ABSENT,
            |_| {
                start_tx.send(()).expect("flood agent running");
                hold_for_agent
                    .lock()
                    .expect("hold lock")
                    .recv()
                    .expect("flood released");
                Ok(Computed {
                    output: CheckOutput::Pass("flood"),
                    measured_units: 1,
                })
            },
        )
    });
    start_rx.recv().expect("flood agent holds its slot");

    // A second agent exceeds the sheddable bound: bounded refusal, never an unbounded queue.
    let blocked: RunOutcome<&str> = subject.run(
        &request("detect.second", ConsumerClass::Agent, "rev-b", false, 1),
        DaemonProbe::ABSENT,
        |_| unreachable!("shed agent must not compute"),
    );
    assert_eq!(blocked.status, RunStatus::Deferred);

    // CI still gets a reserved slot while the agent flood holds one.
    let ci = subject.run(
        &request("detect.ci", ConsumerClass::Ci, "rev-ci", true, 1),
        DaemonProbe::ABSENT,
        |_| {
            Ok(Computed {
                output: CheckOutput::Pass("ci admitted"),
                measured_units: 1,
            })
        },
    );
    assert_eq!(ci.status, RunStatus::Pass("ci admitted"));

    hold_tx.send(()).expect("release flood agent");
    flood.join().expect("flood thread");
}

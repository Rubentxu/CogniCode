//! Cost-aware scheduling of affected work (CAA-09).
//!
//! Turns an [`AffectedWorkPlan`](super::planner::AffectedWorkPlan)
//! identity into executed work with four guarantees:
//!
//! - **Single-flight**: concurrent requests for the same
//!   [`WorkKey`] (real identity: `WorkId` + `SnapshotId` +
//!   capability set) trigger one physical computation; joiners
//!   clone the leader's outcome.
//! - **Bounded, fair queue**: [`QueuePolicy`] caps concurrent
//!   runs per consumer class and reserves CI capacity so a flood
//!   of agent requests can never starve CI.
//! - **Measure-based estimates**: [`CostEstimator`] averages the
//!   most recent [`Computed::measured_units`] reports; the budget
//!   check rides the domain
//!   [`BudgetAuthorizer`] and refuses over-budget work honestly
//!   (`Deferred` / `Inconclusive`, never a fabricated `Pass`).
//! - **Safe cancellation**: [`Coordinator::cancel_revision`]
//!   cancels only the cancelled revision's cancellation flags and
//!   releases only objects no other live revision still
//!   references (shared object refcounts).
//!
//! Daemon vs direct: [`compare_routes`] picks the cheaper route
//! when a warm daemon index exists; an absent daemon falls back to
//! direct execution — never a PASS and never an unbounded wait
//! (routing is a synchronous cost choice, not a blocking probe).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::application::cancellation::CancellationFlag;
use crate::application::change_tracking::planner::WorkId;
use crate::domain::budgets::{BudgetAuthorizer, BudgetDeclaration, BudgetKind, BudgetState};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::findings::detector_ir::AnalysisCapability;

/// Which consumer asked for the work. Urgency and fairness differ:
/// CI has guaranteed capacity; agent traffic is shed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsumerClass {
    /// Interactive agent request (preemptible, sheddable).
    Agent,
    /// Continuous-integration request (guaranteed capacity).
    Ci,
}

/// Real identity of one unit of work: the planned `WorkId`, the
/// snapshot it runs against, and the sorted capability set. Two
/// requests share a flight only when all three match.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkKey {
    work: WorkId,
    snapshot: SnapshotId,
    capabilities: BTreeSet<AnalysisCapability>,
}

impl WorkKey {
    #[must_use]
    pub fn new(
        work: WorkId,
        snapshot: SnapshotId,
        capabilities: impl IntoIterator<Item = AnalysisCapability>,
    ) -> Self {
        Self {
            work,
            snapshot,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    /// Stable canonical identity used for single-flight dedup.
    #[must_use]
    pub fn canonical(&self) -> String {
        let caps: Vec<&str> = self.capabilities.iter().map(|cap| cap.name()).collect();
        format!(
            "{}|{}|[{}]",
            self.work.as_namespaced_name().as_str(),
            self.snapshot.0,
            caps.join(",")
        )
    }
}

/// Candidate revision a request belongs to. Cancelling a revision
/// cancels its in-flight flags and releases its object references.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionTag(String);

impl RevisionTag {
    /// Construct from a non-empty revision identifier.
    #[must_use]
    pub fn new(revision: impl Into<String>) -> Self {
        Self(revision.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An artifact a scheduled computation keeps alive (e.g. a loaded
/// index segment). Tracked by refcount over live revisions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Measure-based cost estimator: integer mean of the most recent
/// `ESTIMATOR_WINDOW` observed unit costs per `WorkId`. Cold work
/// has no estimate; callers supply a conservative default.
#[derive(Debug, Default)]
pub struct CostEstimator {
    samples: Mutex<BTreeMap<String, Vec<u64>>>,
}

/// Sliding window cap so long-lived daemons do not grow samples
/// without bound.
const ESTIMATOR_WINDOW: usize = 16;

impl CostEstimator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one measured cost for a work item.
    pub fn observe(&self, work: &WorkId, units: u64) {
        let mut samples = self.samples.lock().expect("estimator lock");
        let values = samples
            .entry(work.as_namespaced_name().as_str().to_string())
            .or_default();
        values.push(units);
        let excess = values.len().saturating_sub(ESTIMATOR_WINDOW);
        values.drain(..excess);
    }

    /// Mean of the observed samples, or `None` when cold.
    #[must_use]
    pub fn estimate(&self, work: &WorkId) -> Option<u64> {
        let samples = self.samples.lock().expect("estimator lock");
        let values = samples.get(work.as_namespaced_name().as_str())?;
        let sum: u64 = values.iter().sum();
        Some(sum / values.len() as u64)
    }

    /// Estimate when measured, `default` when cold.
    #[must_use]
    pub fn estimate_or(&self, work: &WorkId, default: u64) -> u64 {
        self.estimate(work).unwrap_or(default)
    }
}

/// Bounded concurrency with CI fairness. `ci_reserved` slots of
/// `capacity` are exclusively available to CI, so agent floods can
/// occupy at most `capacity - ci_reserved`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuePolicy {
    pub capacity: usize,
    pub ci_reserved: usize,
}

impl QueuePolicy {
    /// Largest class bound; over-admission is refused, never queued
    /// without limit.
    #[must_use]
    pub fn admits(&self, class: ConsumerClass, agent_active: usize, ci_active: usize) -> bool {
        let total = agent_active.saturating_add(ci_active);
        if total >= self.capacity {
            return false;
        }
        match class {
            ConsumerClass::Agent => agent_active < self.capacity.saturating_sub(self.ci_reserved),
            ConsumerClass::Ci => true,
        }
    }
}

/// One request to schedule.
#[derive(Debug, Clone)]
pub struct ScheduleRequest {
    pub consumer: ConsumerClass,
    pub key: WorkKey,
    pub revision: RevisionTag,
    /// When true, refusal surfaces as `Inconclusive` (a required
    /// check that did not run); when false, as `Deferred`.
    pub required: bool,
    /// Direct-execution cost estimate in budget units.
    pub direct_units: u64,
}

/// Snapshot of the daemon route option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonProbe {
    /// Warm index available with its reuse cost. `None` when the
    /// daemon is absent or cold.
    pub warm_units: Option<u64>,
}

impl DaemonProbe {
    /// No daemon (or no warm index): direct execution only.
    pub const ABSENT: DaemonProbe = DaemonProbe { warm_units: None };

    /// Daemon holds a warm index costing `units` to reuse.
    #[must_use]
    pub fn warm(units: u64) -> Self {
        Self {
            warm_units: Some(units),
        }
    }
}

/// Where the computation will run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Reuse the daemon's warm index.
    Daemon,
    /// Compute in-process from scratch.
    Direct,
}

/// Outcome of the daemon-vs-direct cost comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteComparison {
    pub daemon_units: Option<u64>,
    pub direct_units: u64,
    pub choice: Route,
}

/// Pick the cheaper route. An absent daemon never blocks and never
/// fabricates a result: the choice is `Direct` and the caller runs
/// the computation for real.
#[must_use]
pub fn compare_routes(daemon: DaemonProbe, direct_units: u64) -> RouteComparison {
    let choice = match daemon.warm_units {
        Some(units) if units < direct_units => Route::Daemon,
        _ => Route::Direct,
    };
    RouteComparison {
        daemon_units: daemon.warm_units,
        direct_units,
        choice,
    }
}

/// Honest per-run metrics. Counters are process-cumulative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostMetrics {
    /// Units this run was estimated to need.
    pub estimated_units: u64,
    /// Units the coordinator has committed (cumulative, this run included).
    pub spent_units: u64,
    /// Budget units left after this run's spend.
    pub remaining_units: u64,
    /// Physical computations executed (cumulative).
    pub physical_computations: u64,
    /// Requests that joined an existing flight (cumulative).
    pub requests_joined: u64,
    /// Runs whose cancellation flag was observed cancelled (cumulative).
    pub cancelled: u64,
}

/// Why a run produced no verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InconclusiveReason {
    /// A required check exceeded the declared budget.
    BudgetExhausted { attempted: u64, remaining: u64 },
    /// Bounded queue full for this class.
    QueueFull,
    /// The computation itself failed; no verdict is invented.
    ComputeFailed(String),
}

/// What the computation decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutput<R> {
    /// Check ran and passed, carrying its evidence.
    Pass(R),
    /// Check ran and did not pass.
    NotPass,
}

/// What a computation reports: its verdict plus the units it
/// ACTUALLY consumed. The measured units feed [`CostEstimator`], so
/// future estimates come from observations, not from the request's
/// own claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Computed<R> {
    pub output: CheckOutput<R>,
    pub measured_units: u64,
}

/// Why admission refused a leader before any compute ran.
enum Refusal {
    QueueFull,
    Budget { attempted: u64, remaining: u64 },
}

/// Terminal status of one scheduling attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus<R> {
    /// Computation ran and passed.
    Pass(R),
    /// Computation ran and did not pass.
    NotPass,
    /// This revision's flag was cancelled; result discarded.
    Cancelled,
    /// Optional work refused by budget or queue; not run.
    Deferred,
    /// Required work could not run, or compute failed: never a Pass.
    Inconclusive(InconclusiveReason),
}

/// Full outcome of one [`Coordinator::run`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome<R> {
    pub status: RunStatus<R>,
    pub route: Route,
    /// True when this request joined another thread's computation.
    pub joined: bool,
    pub metrics: CostMetrics,
}

/// What [`Coordinator::cancel_revision`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelReport {
    /// Cancellation flags flipped for the revision.
    pub cancelled: usize,
    /// Objects no other live revision references, in stable order.
    pub released_objects: Vec<ObjectId>,
}

struct Flight<R> {
    done: Mutex<Option<Result<Computed<R>, String>>>,
    cv: Condvar,
}

enum Decision<R> {
    Join(Arc<Flight<R>>, CancellationFlag, u64),
    Lead(CancellationFlag, Arc<Flight<R>>),
    Refused(RunStatus<R>, u64),
}

struct Inner {
    inflight: HashMap<String, Arc<dyn Any + Send + Sync>>,
    handles: BTreeMap<RevisionTag, Vec<CancellationFlag>>,
    object_refs: BTreeMap<ObjectId, BTreeSet<RevisionTag>>,
    agent_active: usize,
    ci_active: usize,
}

struct BudgetCell {
    authorizer: BudgetAuthorizer,
    state: BudgetState,
    spent: u64,
}

impl BudgetCell {
    fn remaining(&self, amount: u64) -> u64 {
        self.authorizer
            .check(&self.state, BudgetKind::EffectCount, amount)
            .unwrap_or_else(|exhausted| exhausted.remaining)
    }
}

/// In-process cost-aware coordinator for affected work.
///
/// Thread-safe: leader/follower single-flight uses condvars, and
/// the lock order is always `inner -> budget`, never the reverse.
pub struct Coordinator {
    policy: QueuePolicy,
    estimator: Arc<CostEstimator>,
    inner: Mutex<Inner>,
    budget: Mutex<BudgetCell>,
    computations: AtomicU64,
    joined: AtomicU64,
    cancelled: AtomicU64,
}

impl std::fmt::Debug for Coordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coordinator")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl Coordinator {
    #[must_use]
    pub fn new(
        policy: QueuePolicy,
        declaration: BudgetDeclaration,
        estimator: Arc<CostEstimator>,
    ) -> Self {
        let state = BudgetState::new(&declaration, 0);
        let authorizer = BudgetAuthorizer::new(declaration);
        Self {
            policy,
            estimator,
            inner: Mutex::new(Inner {
                inflight: HashMap::new(),
                handles: BTreeMap::new(),
                object_refs: BTreeMap::new(),
                agent_active: 0,
                ci_active: 0,
            }),
            budget: Mutex::new(BudgetCell {
                authorizer,
                state,
                spent: 0,
            }),
            computations: AtomicU64::new(0),
            joined: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn estimator(&self) -> &Arc<CostEstimator> {
        &self.estimator
    }

    /// Record that `revision` currently pins `objects`.
    pub fn register_objects(&self, revision: &RevisionTag, objects: &[ObjectId]) {
        let mut inner = self.inner.lock().expect("coordinator lock");
        for object in objects {
            inner
                .object_refs
                .entry(object.clone())
                .or_default()
                .insert(revision.clone());
        }
    }

    /// Live referencing revisions of one object, for callers that
    /// must not release pinned artifacts (e.g. a second revision).
    #[must_use]
    pub fn object_revisions(&self, object: &ObjectId) -> Vec<RevisionTag> {
        let inner = self.inner.lock().expect("coordinator lock");
        inner
            .object_refs
            .get(object)
            .map(|revs| revs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Cancel a candidate revision: flips its in-flight flags and
    /// releases only objects no other live revision still pins.
    ///
    /// A released object means "no registered pin remains" — a
    /// cancelled computation may still be unwinding, so callers
    /// that physically evict artifacts should wait for that run's
    /// [`RunStatus::Cancelled`] outcome first.
    pub fn cancel_revision(&self, revision: &RevisionTag) -> CancelReport {
        let mut inner = self.inner.lock().expect("coordinator lock");
        let flags = inner.handles.remove(revision).unwrap_or_default();
        for flag in &flags {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let mut released: Vec<ObjectId> = Vec::new();
        for (object, revs) in inner.object_refs.iter_mut() {
            if revs.remove(revision) && revs.is_empty() {
                released.push(object.clone());
            }
        }
        for object in &released {
            inner.object_refs.remove(object);
        }
        released.sort();
        CancelReport {
            cancelled: flags.len(),
            released_objects: released,
        }
    }

    /// Current cumulative metrics snapshot.
    #[must_use]
    pub fn metrics(&self) -> CostMetrics {
        let budget = self.budget.lock().expect("budget lock");
        CostMetrics {
            estimated_units: 0,
            spent_units: budget.spent,
            remaining_units: budget.remaining(0),
            physical_computations: self.computations.load(std::sync::atomic::Ordering::SeqCst),
            requests_joined: self.joined.load(std::sync::atomic::Ordering::SeqCst),
            cancelled: self.cancelled.load(std::sync::atomic::Ordering::SeqCst),
        }
    }

    fn snapshot_metrics(&self, estimated_units: u64, remaining_units: u64) -> CostMetrics {
        let mut m = self.metrics();
        m.estimated_units = estimated_units;
        m.remaining_units = remaining_units;
        m
    }

    /// Schedule `req`: join an in-flight computation for the same
    /// [`WorkKey`], or — when admitted and within budget — run
    /// `compute` as the physical leader.
    ///
    /// Budget units are RESERVED atomically with admission (the
    /// check and the commit share one budget lock), so concurrent
    /// leaders can never overspend the declared ceiling together.
    /// A panicking computation is isolated: joiners receive
    /// `ComputeFailed`, never a stranded wait.
    pub fn run<R, F>(&self, req: &ScheduleRequest, daemon: DaemonProbe, compute: F) -> RunOutcome<R>
    where
        R: Clone + Send + Sync + 'static,
        F: FnOnce(&CancellationFlag) -> Result<Computed<R>, String>,
    {
        // Estimates are measure-based: observed costs outrank the
        // caller's cold hint for the direct route.
        let direct = self
            .estimator
            .estimate(&req.key.work)
            .unwrap_or(req.direct_units);
        let cmp = compare_routes(daemon, direct);
        let cost = match cmp.choice {
            Route::Daemon => cmp.daemon_units.unwrap_or(direct),
            Route::Direct => direct,
        };
        let key = req.key.canonical();

        // Lock order is always inner -> budget, never the reverse.
        let decision: Decision<R> = {
            let mut inner = self.inner.lock().expect("coordinator lock");
            if let Some(any) = inner.inflight.get(&key).cloned() {
                if let Ok(flight) = any.downcast::<Flight<R>>() {
                    let flag = new_flag(&mut inner, &req.revision);
                    let remaining = self.budget_remaining(cost);
                    Decision::Join(flight, flag, remaining)
                } else {
                    // Same key but a different payload type is a
                    // caller contract violation; lead without
                    // stealing the live flight's registration.
                    debug_assert!(false, "WorkKey reused across payload types");
                    self.lead_or_refused(&mut inner, req, &key, cost, false)
                }
            } else {
                self.lead_or_refused(&mut inner, req, &key, cost, true)
            }
        };

        match decision {
            Decision::Join(flight, flag, remaining) => {
                self.joined.fetch_add(1, Ordering::SeqCst);
                let stored = self.await_flight(&flight, &flag);
                self.retire_flag(&req.revision, &flag);
                let status = match stored {
                    Some(output) if !flag.load(Ordering::SeqCst) => mirror_status(output),
                    _ => RunStatus::Cancelled,
                };
                self.count_cancelled_run(&status);
                RunOutcome {
                    status,
                    route: cmp.choice,
                    joined: true,
                    metrics: self.snapshot_metrics(cost, remaining),
                }
            }
            Decision::Lead(flag, flight) => {
                let published =
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compute(&flag)))
                    {
                        Ok(result) => result,
                        Err(payload) => Err(panic_note(&payload)),
                    };
                self.computations.fetch_add(1, Ordering::SeqCst);
                if let Ok(computed) = &published {
                    self.estimator
                        .observe(&req.key.work, computed.measured_units);
                }
                {
                    let mut guard = flight.done.lock().expect("flight lock");
                    *guard = Some(published);
                    flight.cv.notify_all();
                }
                let stored = flight.done.lock().expect("flight lock").clone();
                {
                    let mut inner = self.inner.lock().expect("coordinator lock");
                    if let Some(existing) = inner.inflight.get(&key) {
                        let mine: Arc<dyn Any + Send + Sync> = flight.clone();
                        if Arc::ptr_eq(existing, &mine) {
                            inner.inflight.remove(&key);
                        }
                    }
                    self.release_slot(&mut inner, req.consumer);
                    remove_flag(&mut inner, &req.revision, &flag);
                }
                // The reservation was committed at admission; the
                // spend is real even for Cancelled runs (work
                // physically happened), and failed computes keep
                // their reservation as the honest physical cost.
                let status = if flag.load(Ordering::SeqCst) {
                    RunStatus::Cancelled
                } else {
                    mirror_status(stored.expect("flight published above"))
                };
                self.count_cancelled_run(&status);
                RunOutcome {
                    status,
                    route: cmp.choice,
                    joined: false,
                    metrics: self.snapshot_metrics(cost, self.budget_remaining(0)),
                }
            }
            Decision::Refused(status, cost) => RunOutcome {
                status,
                route: cmp.choice,
                joined: false,
                metrics: self.snapshot_metrics(cost, self.budget_remaining(0)),
            },
        }
    }

    /// Wait for the leader's verdict, shedding within one poll
    /// window when this request's revision gets cancelled.
    fn await_flight<R: Clone>(
        &self,
        flight: &Arc<Flight<R>>,
        flag: &CancellationFlag,
    ) -> Option<Result<Computed<R>, String>> {
        let mut guard = flight.done.lock().expect("flight lock");
        loop {
            if guard.is_some() || flag.load(Ordering::SeqCst) {
                return guard.clone();
            }
            let (next, _timeout) = flight
                .cv
                .wait_timeout(guard, std::time::Duration::from_millis(100))
                .expect("flight condvar");
            guard = next;
        }
    }

    fn count_cancelled_run<R>(&self, status: &RunStatus<R>) {
        if matches!(status, RunStatus::Cancelled) {
            self.cancelled.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn budget_remaining(&self, amount: u64) -> u64 {
        self.budget.lock().expect("budget lock").remaining(amount)
    }

    /// Admission + budget reservation for a leader, under the
    /// caller's inner lock.
    fn lead_or_refused<R: Send + Sync + 'static>(
        &self,
        inner: &mut MutexGuard<'_, Inner>,
        req: &ScheduleRequest,
        key: &str,
        cost: u64,
        register: bool,
    ) -> Decision<R> {
        match self.start_lead(inner, req, key, cost, register) {
            Ok((flag, flight)) => Decision::Lead(flag, flight),
            Err(refusal) => Decision::Refused(status_for_refusal(req, refusal), cost),
        }
    }

    fn start_lead<R: Send + Sync + 'static>(
        &self,
        inner: &mut MutexGuard<'_, Inner>,
        req: &ScheduleRequest,
        key: &str,
        cost: u64,
        register: bool,
    ) -> Result<(CancellationFlag, Arc<Flight<R>>), Refusal> {
        if !self
            .policy
            .admits(req.consumer, inner.agent_active, inner.ci_active)
        {
            return Err(Refusal::QueueFull);
        }
        // Check and reserve atomically: one budget lock hold covers
        // both, so concurrent leaders cannot overspend together.
        {
            let mut budget = self.budget.lock().expect("budget lock");
            match budget
                .authorizer
                .check(&budget.state, BudgetKind::EffectCount, cost)
            {
                Ok(_) => {
                    let authorizer = budget.authorizer.clone();
                    authorizer.commit(&mut budget.state, BudgetKind::EffectCount, cost);
                    budget.spent += cost;
                }
                Err(exhausted) => {
                    return Err(Refusal::Budget {
                        attempted: cost,
                        remaining: exhausted.remaining,
                    });
                }
            }
        }
        match req.consumer {
            ConsumerClass::Agent => inner.agent_active += 1,
            ConsumerClass::Ci => inner.ci_active += 1,
        }
        let flag = new_flag(inner, &req.revision);
        let flight = Arc::new(Flight {
            done: Mutex::new(None),
            cv: Condvar::new(),
        });
        if register {
            inner.inflight.insert(
                key.to_string(),
                Arc::clone(&flight) as Arc<dyn Any + Send + Sync>,
            );
        }
        Ok((flag, flight))
    }

    fn release_slot(&self, inner: &mut Inner, class: ConsumerClass) {
        match class {
            ConsumerClass::Agent => inner.agent_active = inner.agent_active.saturating_sub(1),
            ConsumerClass::Ci => inner.ci_active = inner.ci_active.saturating_sub(1),
        }
    }

    fn retire_flag(&self, revision: &RevisionTag, flag: &CancellationFlag) {
        let mut inner = self.inner.lock().expect("coordinator lock");
        remove_flag(&mut inner, revision, flag);
    }
}

fn new_flag(inner: &mut Inner, revision: &RevisionTag) -> CancellationFlag {
    let flag: CancellationFlag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    inner
        .handles
        .entry(revision.clone())
        .or_default()
        .push(Arc::clone(&flag));
    flag
}

fn remove_flag(inner: &mut Inner, revision: &RevisionTag, flag: &CancellationFlag) {
    if let Some(flags) = inner.handles.get_mut(revision) {
        flags.retain(|existing| !Arc::ptr_eq(existing, flag));
        if flags.is_empty() {
            inner.handles.remove(revision);
        }
    }
}

fn mirror_status<R>(output: Result<Computed<R>, String>) -> RunStatus<R> {
    match output {
        Ok(Computed {
            output: CheckOutput::Pass(value),
            ..
        }) => RunStatus::Pass(value),
        Ok(Computed {
            output: CheckOutput::NotPass,
            ..
        }) => RunStatus::NotPass,
        Err(reason) => RunStatus::Inconclusive(InconclusiveReason::ComputeFailed(reason)),
    }
}

fn status_for_refusal<R>(req: &ScheduleRequest, refusal: Refusal) -> RunStatus<R> {
    match (req.required, refusal) {
        (
            true,
            Refusal::Budget {
                attempted,
                remaining,
            },
        ) => RunStatus::Inconclusive(InconclusiveReason::BudgetExhausted {
            attempted,
            remaining,
        }),
        (true, Refusal::QueueFull) => RunStatus::Inconclusive(InconclusiveReason::QueueFull),
        (false, _) => RunStatus::Deferred,
    }
}

fn panic_note(payload: &Box<dyn Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "computation panicked".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::naming::NamespacedName;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    fn work(name: &str) -> WorkId {
        WorkId::new(NamespacedName::new(name).expect("valid name"))
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
        required: bool,
        direct: u64,
    ) -> ScheduleRequest {
        ScheduleRequest {
            consumer,
            key: key(name, 7),
            revision: RevisionTag::new("rev-1"),
            required,
            direct_units: direct,
        }
    }

    fn coordinator(capacity: usize, ci_reserved: usize, effects: Option<u64>) -> Coordinator {
        let declaration = match effects {
            Some(n) => BudgetDeclaration::effects(std::num::NonZeroU64::new(n).expect("nonzero")),
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
    fn canonical_identity_is_order_insensitive() {
        let a = WorkKey::new(
            work("detect.xss"),
            SnapshotId::new(3),
            [AnalysisCapability::Dataflow, AnalysisCapability::AstPattern],
        );
        let b = WorkKey::new(
            work("detect.xss"),
            SnapshotId::new(3),
            [AnalysisCapability::AstPattern, AnalysisCapability::Dataflow],
        );
        assert_eq!(a.canonical(), b.canonical());
        let other_snapshot = WorkKey::new(
            work("detect.xss"),
            SnapshotId::new(4),
            [AnalysisCapability::AstPattern],
        );
        assert_ne!(a.canonical(), other_snapshot.canonical());
    }

    #[test]
    fn estimator_averages_measurements() {
        let estimator = CostEstimator::new();
        let item = work("detect.xss");
        assert_eq!(estimator.estimate(&item), None);
        assert_eq!(estimator.estimate_or(&item, 9), 9);
        estimator.observe(&item, 4);
        estimator.observe(&item, 6);
        assert_eq!(estimator.estimate(&item), Some(5));
    }

    #[test]
    fn queue_policy_sheds_agents_before_ci() {
        let policy = QueuePolicy {
            capacity: 4,
            ci_reserved: 2,
        };
        assert!(policy.admits(ConsumerClass::Agent, 0, 0));
        assert!(policy.admits(ConsumerClass::Agent, 1, 0));
        // Agent bound is capacity - ci_reserved = 2.
        assert!(!policy.admits(ConsumerClass::Agent, 2, 0));
        // CI is guaranteed up to total capacity.
        assert!(policy.admits(ConsumerClass::Ci, 2, 0));
        assert!(policy.admits(ConsumerClass::Ci, 2, 1));
        assert!(!policy.admits(ConsumerClass::Ci, 2, 2));
    }

    #[test]
    fn routes_prefer_cheaper_warm_daemon() {
        let warm = compare_routes(DaemonProbe::warm(3), 9);
        assert_eq!(warm.choice, Route::Daemon);
        assert_eq!(warm.daemon_units, Some(3));
        let cold = compare_routes(DaemonProbe::ABSENT, 9);
        assert_eq!(cold.choice, Route::Direct);
        assert_eq!(cold.daemon_units, None);
        let pricier = compare_routes(DaemonProbe::warm(12), 9);
        assert_eq!(pricier.choice, Route::Direct);
    }

    #[test]
    fn required_over_budget_is_inconclusive_and_never_computes() {
        let subject = coordinator(4, 1, Some(10));
        let ran = AtomicBool::new(false);
        let outcome = subject.run(
            &request("detect.xss", ConsumerClass::Ci, true, 15),
            DaemonProbe::ABSENT,
            |_| {
                ran.store(true, Ordering::SeqCst);
                Ok(Computed {
                    output: CheckOutput::Pass(()),
                    measured_units: 1,
                })
            },
        );
        assert_eq!(
            outcome.status,
            RunStatus::Inconclusive(InconclusiveReason::BudgetExhausted {
                attempted: 15,
                remaining: 10
            })
        );
        assert!(!ran.load(Ordering::SeqCst), "refused work must not compute");
        assert_eq!(outcome.metrics.spent_units, 0);
        assert_eq!(outcome.metrics.remaining_units, 10);
    }

    #[test]
    fn optional_over_budget_defers_without_failure() {
        let subject = coordinator(4, 1, Some(10));
        let outcome = subject.run(
            &request("detect.xss", ConsumerClass::Agent, false, 15),
            DaemonProbe::ABSENT,
            |_| -> Result<Computed<()>, String> { panic!("deferred work must not compute") },
        );
        assert_eq!(outcome.status, RunStatus::Deferred);
    }

    #[test]
    fn warm_daemon_route_charges_reuse_cost_not_direct_cost() {
        let subject = coordinator(4, 1, Some(5));
        let outcome = subject.run(
            &request("detect.xss", ConsumerClass::Ci, true, 9),
            DaemonProbe::warm(3),
            |_| {
                Ok(Computed {
                    output: CheckOutput::Pass(42),
                    measured_units: 3,
                })
            },
        );
        assert_eq!(outcome.route, Route::Daemon);
        assert_eq!(outcome.status, RunStatus::Pass(42));
        assert_eq!(outcome.metrics.estimated_units, 3);
        assert_eq!(outcome.metrics.spent_units, 3);
        assert_eq!(outcome.metrics.remaining_units, 2);
    }

    #[test]
    fn compute_failure_surfaces_as_inconclusive_not_pass() {
        let subject = coordinator(4, 1, None);
        let outcome = subject.run(
            &request("detect.xss", ConsumerClass::Ci, true, 1),
            DaemonProbe::ABSENT,
            |_| -> Result<Computed<()>, String> { Err("parser exploded".to_string()) },
        );
        assert!(matches!(
            outcome.status,
            RunStatus::Inconclusive(InconclusiveReason::ComputeFailed(_))
        ));
    }

    #[test]
    fn three_concurrent_requests_one_physical_computation() {
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
                    &request("detect.xss", ConsumerClass::Agent, true, 1),
                    DaemonProbe::ABSENT,
                    |_| {
                        computations.fetch_add(1, Ordering::SeqCst);
                        start_tx.send(()).expect("signal start");
                        hold.lock()
                            .expect("hold lock")
                            .recv()
                            .expect("release leader");
                        Ok(Computed {
                            output: CheckOutput::Pass("findings"),
                            measured_units: 1,
                        })
                    },
                )
            }));
        }
        drop(start_tx);
        start_rx.recv().expect("one leader started");
        std::thread::sleep(std::time::Duration::from_millis(50));
        hold_tx.send(()).expect("release leader");
        let outcomes: Vec<RunOutcome<&str>> = handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect();
        assert_eq!(computations.load(Ordering::SeqCst), 1);
        assert!(
            outcomes
                .iter()
                .all(|o| o.status == RunStatus::Pass("findings"))
        );
        assert_eq!(outcomes.iter().filter(|o| o.joined).count(), 2);
        assert_eq!(subject.metrics().physical_computations, 1);
        assert_eq!(subject.metrics().requests_joined, 2);
    }

    #[test]
    fn cancel_revision_releases_only_unpinned_objects() {
        let subject = coordinator(4, 1, None);
        let shared = ObjectId::new("index:seg-1");
        let agent_only = ObjectId::new("index:seg-agent");
        let agent_rev = RevisionTag::new("agent-rev");
        let ci_rev = RevisionTag::new("ci-rev");
        subject.register_objects(&agent_rev, &[shared.clone(), agent_only.clone()]);
        subject.register_objects(&ci_rev, std::slice::from_ref(&shared));
        let report = subject.cancel_revision(&agent_rev);
        assert_eq!(report.released_objects, vec![agent_only]);
        assert_eq!(subject.object_revisions(&shared), vec![ci_rev]);
    }

    fn wait_until(deadline_ms: u64, mut cond: impl FnMut() -> bool) -> bool {
        let started = std::time::Instant::now();
        while !cond() {
            if started.elapsed() > std::time::Duration::from_millis(deadline_ms) {
                return false;
            }
            std::thread::yield_now();
        }
        true
    }

    #[test]
    fn leader_panic_fails_joiners_instead_of_stranding_them() {
        let subject = Arc::new(coordinator(4, 1, None));
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let (panic_go_tx, panic_go_rx) = std::sync::mpsc::channel::<()>();
        let leader_subject = Arc::clone(&subject);
        let leader = std::thread::spawn(move || {
            leader_subject.run(
                &request("detect.xss", ConsumerClass::Ci, true, 1),
                DaemonProbe::ABSENT,
                |_| -> Result<Computed<()>, String> {
                    started_tx.send(()).expect("leader running");
                    panic_go_rx.recv().expect("joiner arrived");
                    panic!("leader exploded")
                },
            )
        });
        started_rx.recv().expect("leader started");
        let joiner_subject = Arc::clone(&subject);
        let joiner = std::thread::spawn(move || {
            joiner_subject.run(
                &request("detect.xss", ConsumerClass::Ci, true, 1),
                DaemonProbe::ABSENT,
                |_| {
                    Ok(Computed {
                        output: CheckOutput::Pass(()),
                        measured_units: 1,
                    })
                },
            )
        });
        assert!(
            wait_until(2_000, || subject.metrics().requests_joined == 1),
            "joiner must register before the panic is released"
        );
        panic_go_tx.send(()).expect("release panic");
        let leader_outcome = leader.join().expect("leader thread");
        let joiner = joiner.join().expect("joiner thread");
        assert!(joiner.joined);
        assert!(matches!(
            joiner.status,
            RunStatus::Inconclusive(InconclusiveReason::ComputeFailed(_))
        ));
        assert!(matches!(
            leader_outcome.status,
            RunStatus::Inconclusive(InconclusiveReason::ComputeFailed(_))
        ));
        // Inflight and slot are cleaned: a later request leads again.
        let third = subject.run(
            &request("detect.xss", ConsumerClass::Ci, true, 1),
            DaemonProbe::ABSENT,
            |_| {
                Ok(Computed {
                    output: CheckOutput::Pass(()),
                    measured_units: 1,
                })
            },
        );
        assert_eq!(third.status, RunStatus::Pass(()));
    }

    #[test]
    fn budget_reservation_is_atomic_across_concurrent_leaders() {
        // Ceiling 10: two concurrent leaders of 6 cannot both spend.
        let subject = Arc::new(coordinator(4, 1, Some(10)));
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let held_subject = Arc::clone(&subject);
        let held = std::thread::spawn(move || {
            held_subject.run(
                &request("detect.held", ConsumerClass::Ci, true, 6),
                DaemonProbe::ABSENT,
                |_| {
                    started_tx.send(()).expect("held leader running");
                    release_rx.recv().expect("held released");
                    Ok(Computed {
                        output: CheckOutput::Pass(()),
                        measured_units: 6,
                    })
                },
            )
        });
        started_rx.recv().expect("held leader reserved 6");
        let second: RunOutcome<()> = subject.run(
            &request("detect.second", ConsumerClass::Ci, true, 6),
            DaemonProbe::ABSENT,
            |_| {
                Ok(Computed {
                    output: CheckOutput::Pass(()),
                    measured_units: 6,
                })
            },
        );
        assert!(
            matches!(
                second.status,
                RunStatus::Inconclusive(InconclusiveReason::BudgetExhausted {
                    attempted: 6,
                    remaining: 4,
                })
            ),
            "concurrent leaders must not overspend the ceiling together"
        );
        release_tx.send(()).expect("release held leader");
        let held_outcome = held.join().expect("held thread");
        assert_eq!(held_outcome.status, RunStatus::Pass(()));
        let metrics = subject.metrics();
        assert_eq!(metrics.spent_units, 6, "only the reserved leader spent");
        assert_eq!(metrics.physical_computations, 1);
    }
}

//! One run of a piece of work, shared by everybody who asks for it while it
//! runs.
//!
//! ## Why this exists
//!
//! Building the graph of a tree costs seconds of CPU and disk, and the
//! product answers several callers at once: four agents asking four
//! questions of the same cold workspace used to start four builds of the same
//! tree, each one publishing its own graph over the others. The work is the
//! same and so is its result, so it should run once.
//!
//! ## The contract
//!
//! [`SingleFlight::run`] takes a key and the work. The first caller for a key
//! **leads**: it runs the work. Every caller that arrives for the same key
//! while the work runs **follows**: it parks and receives a copy of what the
//! leader produced, whether that is a value or an error.
//!
//! - **A failure is shared, never kept.** The flight ends when the leader
//!   returns; the next caller leads a new one. An error that was remembered
//!   would be an answer served after the cause had gone.
//! - **A caller that arrives after the flight ended leads its own.** Only
//!   callers that were already parked receive the result, so nobody is served
//!   a result that was already finished when they asked.
//! - **A caller that arrives while the work runs receives that run's
//!   answer**, as the work saw the world when it began. The window is one run:
//!   a caller that needs a later view asks again once the run is over.
//! - **A leader that panics strands nobody.** Its followers wake up, and one
//!   of them leads with its own work.
//!
//! The primitive holds no result after the flight: it is a rendezvous, not a
//! cache. What is worth keeping is the owner's business (the graph cache and
//! the durable snapshot already keep it).

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

/// The flights in progress, by key.
pub struct SingleFlight<K, T, E> {
    flights: Mutex<HashMap<K, Arc<Flight<T, E>>>>,
}

/// One run of the work, as the callers parked behind it see it.
struct Flight<T, E> {
    state: Mutex<State<T, E>>,
    settled: Condvar,
    /// The followers that attached to this flight.
    ///
    /// A follower is counted when it attaches, under the lock of the map,
    /// so the leader that retires the flight sees every follower that can
    /// ever read it: one that arrives later finds no flight and leads its
    /// own. The count is never lowered: a flight that has answered has left
    /// the map, and the map is the only place it can be asked.
    followers: AtomicUsize,
}

enum State<T, E> {
    Running,
    Done(Result<T, E>),
    /// The leader panicked: there is no result, and the followers must lead.
    Abandoned,
}

impl<K: Hash + Eq + Clone, T: Clone, E: Clone> SingleFlight<K, T, E> {
    pub fn new() -> Self {
        Self {
            flights: Mutex::new(HashMap::new()),
        }
    }

    /// Runs `work` for `key`, or waits for the run that is already going.
    ///
    /// The leader's own result is returned as it is; followers receive a
    /// copy. `work` runs on the calling thread, so a caller that must not
    /// block gives this function to a thread that may.
    pub fn run(&self, key: K, work: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        let mut work = Some(work);
        loop {
            match self.attach(&key) {
                Role::Leader(flight) => {
                    let work = work.take().expect("a caller leads at most once");
                    return self.lead(key, flight, work);
                }
                Role::Follower(flight) => {
                    if let Some(result) = flight.wait() {
                        return result;
                    }
                    // The leader panicked before it answered: this caller
                    // asks again, and leads if nobody else got there first.
                }
            }
        }
    }

    /// How many callers attached to the run in progress for `key`; zero when
    /// no run of that key is going.
    pub fn waiting(&self, key: &K) -> usize {
        lock(&self.flights)
            .get(key)
            .map_or(0, |flight| flight.followers.load(Ordering::SeqCst))
    }

    /// Joins the flight of `key`, or opens it.
    fn attach(&self, key: &K) -> Role<T, E> {
        let mut flights = lock(&self.flights);
        if let Some(flight) = flights.get(key) {
            flight.followers.fetch_add(1, Ordering::SeqCst);
            return Role::Follower(flight.clone());
        }
        let flight = Arc::new(Flight {
            state: Mutex::new(State::Running),
            settled: Condvar::new(),
            followers: AtomicUsize::new(0),
        });
        flights.insert(key.clone(), flight.clone());
        Role::Leader(flight)
    }

    fn lead(
        &self,
        key: K,
        flight: Arc<Flight<T, E>>,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let mut leadership = Leadership {
            flights: &self.flights,
            key: &key,
            flight: &flight,
            settled: false,
        };
        let result = work();
        leadership.settle(&result);
        result
    }
}

impl<K: Hash + Eq + Clone, T: Clone, E: Clone> Default for SingleFlight<K, T, E> {
    fn default() -> Self {
        Self::new()
    }
}

enum Role<T, E> {
    Leader(Arc<Flight<T, E>>),
    Follower(Arc<Flight<T, E>>),
}

impl<T: Clone, E: Clone> Flight<T, E> {
    /// Parks until the leader answers; `None` when the leader never will.
    fn wait(&self) -> Option<Result<T, E>> {
        let mut state = lock(&self.state);
        while matches!(*state, State::Running) {
            state = self
                .settled
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        match &*state {
            State::Done(result) => Some(result.clone()),
            State::Abandoned | State::Running => None,
        }
    }

    fn publish(&self, state: State<T, E>) {
        *lock(&self.state) = state;
        self.settled.notify_all();
    }
}

/// The leader's obligation to end the flight, whatever the work does.
///
/// Dropping it without having settled means the work panicked: the flight is
/// retired and abandoned so the followers do not wait for a result that will
/// not come.
struct Leadership<'a, K: Hash + Eq, T, E> {
    flights: &'a Mutex<HashMap<K, Arc<Flight<T, E>>>>,
    key: &'a K,
    flight: &'a Arc<Flight<T, E>>,
    settled: bool,
}

impl<K: Hash + Eq, T: Clone, E: Clone> Leadership<'_, K, T, E> {
    /// Ends the flight with the leader's result.
    fn settle(&mut self, result: &Result<T, E>) {
        // Out of the map before anyone is answered: a caller that arrives
        // from here on leads its own flight and sees the world as it is by
        // now, and only the callers already parked receive this result.
        let followers = self.retire();
        // The copy is made before the flight counts as settled: if copying
        // panics, `Drop` still abandons the flight and the followers wake.
        let answer = (followers > 0).then(|| State::Done(result.clone()));
        self.settled = true;
        if let Some(answer) = answer {
            self.flight.publish(answer);
        }
    }

    /// Removes the flight from the map; says how many followers attached.
    fn retire(&self) -> usize {
        let mut flights = lock(self.flights);
        if flights
            .get(self.key)
            .is_some_and(|current| Arc::ptr_eq(current, self.flight))
        {
            flights.remove(self.key);
        }
        self.flight.followers.load(Ordering::SeqCst)
    }
}

impl<K: Hash + Eq, T, E> Drop for Leadership<'_, K, T, E> {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        let mut flights = lock(self.flights);
        if flights
            .get(self.key)
            .is_some_and(|current| Arc::ptr_eq(current, self.flight))
        {
            flights.remove(self.key);
        }
        drop(flights);
        *lock(&self.flight.state) = State::Abandoned;
        self.flight.settled.notify_all();
    }
}

/// A mutex whose owner panicked is still a mutex: the state it guards is
/// replaced whole under it, never half-written.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// How long a step that must happen is given to happen.
    const DEADLINE: Duration = Duration::from_secs(10);

    type Flights = SingleFlight<&'static str, u32, String>;

    /// A latch the leader's work waits on, with a deadline of its own.
    struct Latch {
        open: Mutex<bool>,
        signal: Condvar,
    }

    impl Latch {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                open: Mutex::new(false),
                signal: Condvar::new(),
            })
        }

        fn open(&self) {
            *lock(&self.open) = true;
            self.signal.notify_all();
        }

        fn wait(&self) {
            let open = lock(&self.open);
            let (_open, timeout) = self
                .signal
                .wait_timeout_while(open, DEADLINE * 3, |open| !*open)
                .unwrap_or_else(PoisonError::into_inner);
            assert!(!timeout.timed_out(), "the test never opened the latch");
        }
    }

    fn becomes_true_within_the_deadline(condition: impl Fn() -> bool) -> bool {
        let started = Instant::now();
        while !condition() {
            if started.elapsed() > DEADLINE {
                return false;
            }
            std::thread::yield_now();
        }
        true
    }

    /// Starts `callers` threads that ask for the same key; the first work to
    /// run is held until the latch opens. Returns the channel of answers.
    fn callers_for_one_key(
        flights: &Arc<Flights>,
        callers: usize,
        runs: &Arc<AtomicUsize>,
        latch: &Arc<Latch>,
        outcome: impl Fn() -> Result<u32, String> + Send + Sync + Clone + 'static,
    ) -> mpsc::Receiver<Result<u32, String>> {
        let (tx, rx) = mpsc::channel();
        for _ in 0..callers {
            let (flights, runs, latch, outcome, tx) = (
                flights.clone(),
                runs.clone(),
                latch.clone(),
                outcome.clone(),
                tx.clone(),
            );
            std::thread::spawn(move || {
                let answer = flights.run("tree", || {
                    runs.fetch_add(1, Ordering::SeqCst);
                    latch.wait();
                    outcome()
                });
                let _ = tx.send(answer);
            });
        }
        rx
    }

    fn collect(rx: &mpsc::Receiver<Result<u32, String>>, count: usize) -> Vec<Result<u32, String>> {
        (0..count)
            .map(|_| {
                rx.recv_timeout(DEADLINE)
                    .expect("every caller is answered within the deadline")
            })
            .collect()
    }

    #[test]
    fn callers_that_arrive_while_the_work_runs_share_its_one_run() {
        let flights = Arc::new(Flights::new());
        let runs = Arc::new(AtomicUsize::new(0));
        let latch = Latch::new();
        let answers = callers_for_one_key(&flights, 5, &runs, &latch, || Ok(7));

        assert!(
            becomes_true_within_the_deadline(|| flights.waiting(&"tree") == 4),
            "four callers park behind the one that runs"
        );
        latch.open();

        assert_eq!(collect(&answers, 5), vec![Ok(7); 5]);
        assert_eq!(runs.load(Ordering::SeqCst), 1, "the work ran once");
        assert_eq!(
            flights.waiting(&"tree"),
            0,
            "the flight left the map once it had answered"
        );
    }

    #[test]
    fn a_failure_reaches_every_waiting_caller() {
        let flights = Arc::new(Flights::new());
        let runs = Arc::new(AtomicUsize::new(0));
        let latch = Latch::new();
        let answers = callers_for_one_key(&flights, 4, &runs, &latch, || Err("broken".to_string()));

        assert!(becomes_true_within_the_deadline(|| flights
            .waiting(&"tree")
            == 3));
        latch.open();

        assert_eq!(collect(&answers, 4), vec![Err("broken".to_string()); 4]);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_failure_is_not_kept_for_the_next_caller() {
        let flights = Flights::new();

        let first = flights.run("tree", || Err("broken".to_string()));
        let second = flights.run("tree", || Ok(3));

        assert_eq!(first, Err("broken".to_string()));
        assert_eq!(second, Ok(3), "the next caller runs the work again");
    }

    #[test]
    fn a_success_is_not_kept_either() {
        let flights = Flights::new();
        let runs = AtomicUsize::new(0);

        for _ in 0..3 {
            let answer = flights.run("tree", || Ok(runs.fetch_add(1, Ordering::SeqCst) as u32));
            assert!(answer.is_ok());
        }

        assert_eq!(
            runs.load(Ordering::SeqCst),
            3,
            "sequential callers each run the work: the primitive is a rendezvous, not a cache"
        );
    }

    #[test]
    fn different_keys_do_not_wait_for_each_other() {
        let flights = Arc::new(Flights::new());
        let held = Latch::new();
        let (started_tx, started_rx) = mpsc::channel();

        let slow = {
            let (flights, held) = (flights.clone(), held.clone());
            std::thread::spawn(move || {
                flights.run("slow", || {
                    started_tx.send(()).unwrap();
                    held.wait();
                    Ok(1)
                })
            })
        };
        started_rx
            .recv_timeout(DEADLINE)
            .expect("the slow work started");

        let quick = flights.run("quick", || Ok(2));
        held.open();

        assert_eq!(quick, Ok(2), "another key is answered while one is held");
        assert_eq!(slow.join().unwrap(), Ok(1));
    }

    #[test]
    fn a_leader_that_panics_does_not_strand_its_followers() {
        let flights = Arc::new(Flights::new());
        let latch = Latch::new();
        let (started_tx, started_rx) = mpsc::channel();

        let leader = {
            let (flights, latch) = (flights.clone(), latch.clone());
            std::thread::spawn(move || {
                flights.run("tree", || {
                    started_tx.send(()).unwrap();
                    latch.wait();
                    panic!("the work blew up");
                })
            })
        };
        started_rx
            .recv_timeout(DEADLINE)
            .expect("the leader started");

        let (tx, rx) = mpsc::channel();
        let follower = {
            let (flights, tx) = (flights.clone(), tx);
            std::thread::spawn(move || {
                let _ = tx.send(flights.run("tree", || Ok(9)));
            })
        };
        assert!(
            becomes_true_within_the_deadline(|| flights.waiting(&"tree") == 1),
            "the follower parks behind the leader"
        );
        latch.open();

        assert!(leader.join().is_err(), "the panic belongs to the leader");
        assert_eq!(
            rx.recv_timeout(DEADLINE)
                .expect("the follower is answered, not stranded"),
            Ok(9),
            "the follower led with its own work"
        );
        follower.join().unwrap();
        assert_eq!(flights.waiting(&"tree"), 0);
    }

    /// A value whose copy for the followers fails.
    struct Fragile;

    impl Clone for Fragile {
        fn clone(&self) -> Self {
            panic!("the copy for the followers blew up");
        }
    }

    #[test]
    fn a_leader_whose_copy_panics_does_not_strand_its_followers() {
        let flights = Arc::new(SingleFlight::<&'static str, Fragile, String>::new());
        let latch = Latch::new();
        let (started_tx, started_rx) = mpsc::channel();

        let leader = {
            let (flights, latch) = (flights.clone(), latch.clone());
            std::thread::spawn(move || {
                flights
                    .run("tree", || {
                        started_tx.send(()).unwrap();
                        latch.wait();
                        Ok(Fragile)
                    })
                    .is_ok()
            })
        };
        started_rx
            .recv_timeout(DEADLINE)
            .expect("the leader started");

        let (tx, rx) = mpsc::channel();
        let follower = {
            let (flights, tx) = (flights.clone(), tx);
            std::thread::spawn(move || {
                let _ = tx.send(flights.run("tree", || Ok(Fragile)).is_ok());
            })
        };
        assert!(
            becomes_true_within_the_deadline(|| flights.waiting(&"tree") == 1),
            "the follower parks behind the leader"
        );
        latch.open();

        assert!(
            leader.join().is_err(),
            "the panic of the copy belongs to the leader"
        );
        assert!(
            rx.recv_timeout(DEADLINE)
                .expect("the follower is answered, not stranded"),
            "the follower led with its own work"
        );
        follower.join().unwrap();
    }

    #[test]
    fn a_caller_that_arrives_after_the_run_leads_its_own() {
        let flights = Arc::new(Flights::new());
        let runs = Arc::new(AtomicUsize::new(0));
        let latch = Latch::new();
        let first = callers_for_one_key(&flights, 1, &runs, &latch, || Ok(1));
        latch.open();
        assert_eq!(collect(&first, 1), vec![Ok(1)]);

        let second = flights.run("tree", || Ok(2));

        assert_eq!(
            second,
            Ok(2),
            "it is not served the result of a finished run"
        );
    }
}

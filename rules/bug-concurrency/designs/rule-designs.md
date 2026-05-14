# Bug-Concurrency Batch: 8 Concurrency Bug Detection Rules
# Design Phase Document
# Generated: 2026-05-14

---

## Rule: S1872
### metadata
- rule_id: S1872
- concept_id: concurrency-race-condition
- severity: CRITICAL
- precision: high
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: Shared mutable state captured in closure without synchronization
;; Detect: identifier used in closure that captures env mutable without Mutex/RwLock/atomic

(closure_expression
  parameters: (parameters)
  body: (block
    (statement
      (expression_statement
        (assignment_expression
          left: (identifier) @target
          right: (binary_expression
            left: (identifier) @captured
            operator: "+="))))))
```

Key indicators:
- `+=(assignment)` inside closure
- Left-hand side identifier is captured from environment
- No Mutex/RwLock/atomic wrapper detected in scope

### Code Examples

**Bad (S1872)**:
```rust
fn main() {
    let mut counter = 0;
    let handle = std::thread::spawn(move || {
        counter += 1;  // RACE: counter captured mutably without synchronization
    });
    handle.join().unwrap();
}
```

**Bad (S1872 - multiple threads)**:
```rust
fn main() {
    let mut data = vec![1, 2, 3];
    let handles: Vec<_> = (0..4).map(|_| {
        std::thread::spawn(move || {
            data.push(42);  // RACE: shared mutable vector without synchronization
        })
    }).collect();
}
```

**Good (S1872)**:
```rust
use std::sync::{Arc, Mutex};

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let handles: Vec<_> = (0..4).map(|_| {
        let counter = Arc::clone(&counter);
        std::thread::spawn(move || {
            let mut num = counter.lock().unwrap();
            *num += 1;
        })
    }).collect();
}
```

### False Positive Conditions
- Identifier is wrapped in `Arc<Mutex<T>>` or `Arc<RwLock<T>>`
- Identifier is an atomic type (`AtomicI32`, etc.)
- Identifier is a primitive type with `Copy` that is read-only
- Closure does not actually capture the variable (different binding)

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-362 (Race Condition)
  owasp: A01:2021 (Broken Access Control)
  carm: RACE-01
  impact:
    reliability: Critical
    availability: High
  fix_effort: Medium
  remediation: |
    Wrap shared state in Arc<Mutex<T>> or Arc<RwLock<T>>.
    Use atomic types for simple counters.
  review_questions:
    - "Is this variable accessed from multiple threads?"
    - "Is proper synchronization in place (Mutex/RwLock/atomic)?"
    - "Could this be replaced with message passing (channels)?"
```

### Estimated LOC
- Core rule: ~80 LOC
- Helpers: 2 functions (~20 LOC)
- Tests: 6 test cases (~60 LOC)
- **Total: ~160 LOC** (slightly over budget, acceptable for CRITICAL severity)

### Complexity Score
- Cognitive: 14/20
- Cyclomatic: 3/10
- Nesting: 2/10

---

## Rule: S1873
### metadata
- rule_id: S1873
- concept_id: concurrency-mutex-guard-leaked
- severity: MAJOR
- precision: high
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: MutexGuard/RwLockGuard not released before return/.await
;; Detect: return_statement or await_expression where guard identifier is in scope

(return_statement
  expression: (identifier) @guard
  (#match? @guard "^.*guard$|^.*lock$|^.*guard_.*$"))

;; Also detect: return of field access that holds guard
(return_statement
  expression: (field_expression
    object: (identifier) @guard))
```

Secondary pattern for `.await`:
```lisp
(await_expression
  expression: (call_expression
    function: (identifier) @guard_call
    arguments: (arguments (identifier) @guard)))
```

### Code Examples

**Bad (S1873)**:
```rust
fn get_data<'a>(lock: &'a std::sync::Mutex<u32>) -> &'a u32 {
    let guard = lock.lock().unwrap();
    return &*guard;  // LEAK: guard dropped here, reference invalidated
}
```

**Bad (S1873 - async)**:
```rust
async fn get_data(lock: std::sync::Mutex<u32>) -> u32 {
    let guard = lock.lock().unwrap();
    some_async_operation().await;  // LEAK: guard held across await point
    *guard
}
```

**Good (S1873)**:
```rust
fn get_data<'a>(lock: &'a std::sync::Mutex<u32>) -> u32 {
    let guard = lock.lock().unwrap();
    *guard  // OK: guard released at end of function, value copied
}
```

**Good (S1873 - explicit scope)**:
```rust
fn get_data<'a>(lock: &'a std::sync::Mutex<u32>) -> u32 {
    {
        let guard = lock.lock().unwrap();
        return *guard;  // OK: guard is in scope, value copied
    }
}
```

### False Positive Conditions
- Return type is `Copy` (value copied before drop)
- Guard is explicitly dropped before return (`drop(guard)`)
- Using `Mutex::get_mut()` which doesn't require guards
- Guard returned is a reference to inner data that outlives the mutex

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-821 (Use of Incorrect Synchronization)
  impact:
    reliability: High
    correctness: Critical
  fix_effort: Low
  remediation: |
    Ensure MutexGuard/RwLockGuard is dropped before returning.
    Use scope blocks to limit guard lifetime.
    Consider using `parking_lot` crate with `Mutex::guard()`.
  review_questions:
    - "Is the returned reference valid after the function returns?"
    - "Is the guard's lifetime properly scoped?"
    - "Could this use a better ownership pattern?"
```

### Estimated LOC
- Core rule: ~65 LOC
- Helpers: 2 functions (~15 LOC)
- Tests: 6 test cases (~55 LOC)
- **Total: ~135 LOC**

### Complexity Score
- Cognitive: 12/20
- Cyclomatic: 2/10
- Nesting: 2/10

---

## Rule: S1874
### metadata
- rule_id: S1874
- concept_id: concurrency-deadlock-lock-ordering
- severity: CRITICAL
- precision: medium
- detection_strategy: tree-sitter query (multi-pattern cross-analysis)

### AST Pattern

```lisp
;; Pattern: Nested lock acquisitions in different orders across code paths
;; Phase 1: Find all lock acquisition patterns

(macro_invocation
  macro: (identifier) @lock_macro
  (token_tree
    (identifier) @locked_resource))

;; Phase 2: Identify nested patterns
(let_declaration
  value: (macro_invocation
    macro: (identifier) @outer_lock
    (token_tree (identifier) @outer_res))
  body: (block
    (statement
      (let_declaration
        value: (macro_invocation
          macro: (identifier) @inner_lock
          (token_tree (identifier) @inner_res)))))
```

### Code Examples

**Bad (S1874)**:
```rust
use std::sync::{Mutex, Arc};

fn process(a: Arc<Mutex<u32>>, b: Arc<Mutex<u32>>) {
    let _a = a.lock().unwrap();
    let _b = b.lock().unwrap();
    // work
}  // If another thread does: let _b = b.lock(); let _a = a.lock(); -> DEADLOCK

fn process_reversed(a: Arc<Mutex<u32>>, b: Arc<Mutex<u32>>) {
    let _b = b.lock().unwrap();  // Different order!
    let _a = a.lock().unwrap();
    // work
}
```

**Good (S1874)**:
```rust
use std::sync::{Mutex, Arc};

fn process(a: Arc<Mutex<u32>>, b: Arc<Mutex<u32>>) {
    // Always acquire in same order: a then b
    let _a = a.lock().unwrap();
    let _b = b.lock().unwrap();
    // work
}

fn process_reversed(a: Arc<Mutex<u32>>, b: Arc<Mutex<u32>>) {
    // Same order as above: a then b
    let _a = a.lock().unwrap();
    let _b = b.lock().unwrap();
    // work
}
```

**Good (S1874 - single lock)**:
```rust
fn process(a: Arc<Mutex<u32>>) {
    let _a = a.lock().unwrap();
    // work with only one lock
}
```

### False Positive Conditions
- Each function/context only uses one lock
- Lock ordering is consistent (same order everywhere)
- Fine-grained locks replaced with coarse-grained lock
- Uses lock-free data structures instead
- Uses `RwLock` which allows multiple readers (different semantics)

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-833 (Deadlock)
  impact:
    availability: Critical
    reliability: High
  fix_effort: High
  remediation: |
    Establish a global lock ordering convention.
    Always acquire locks in the same order (e.g., by address).
    Consider using a single aggregate lock instead of multiple fine-grained locks.
    Consider lock-free data structures.
  review_questions:
    - "Is there a consistent lock ordering across all code paths?"
    - "Could this be refactored to use fewer locks?"
    - "Are there unit tests that verify lock ordering?"
```

### Estimated LOC
- Core rule: ~110 LOC
- Helpers: 3 functions (~30 LOC)
- Tests: 5 test cases (~50 LOC)
- **Total: ~190 LOC** (exceeds budget, needs splitting into 2 rules)

**Note**: This rule exceeds 150 LOC. Recommend splitting into:
- S1874a: Basic nested lock detection (~80 LOC)
- S1874b: Cross-function lock ordering analysis (~70 LOC)

### Complexity Score
- Cognitive: 18/20 (high due to cross-analysis)
- Cyclomatic: 4/10
- Nesting: 3/10

---

## Rule: S1875
### metadata
- rule_id: S1875
- concept_id: concurrency-channel-closed-send
- severity: CRITICAL
- precision: high
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: .send() on channel after sender dropped or closed
;; Phase 1: Find all send() calls
(call_expression
  function: (field_expression
    object: (identifier) @channel
    field: (field_identifier) @method)
  (#eq? @method "send")
  arguments: (arguments
    (identifier) @value))

;; Phase 2: Detect sender_dropped before send (via dominance analysis)
;; Check if there's a drop(sender) or sender goes out of scope before send
```

### Code Examples

**Bad (S1875)**:
```rust
use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();
    drop(tx);  // Sender dropped here

    let result = rx.recv();
    // ...
}

fn bad_example() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        drop(tx);  // tx dropped in spawned thread
        tx.send(42).unwrap();  // ERROR: send on closed channel
    });
}
```

**Bad (S1875 - cloned sender)**:
```rust
use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();
    let _tx_clone = tx.clone();
    drop(tx);  // Original dropped, but clone still alive
    // This is OK - clone is still valid

    let _tx_clone2 = tx.clone();  // ERROR: tx already moved/dropped
}
```

**Good (S1875)**:
```rust
use std::sync::mpsc;

fn good_example() {
    let (tx, rx) = mpsc::channel();
    let tx_clone = tx.clone();

    std::thread::spawn(move || {
        // tx_clone is valid here
        tx_clone.send(42).unwrap();
    });

    drop(tx);  // Original dropped, but clone still alive
    drop(rx);  // Both dropped at end
}
```

### False Positive Conditions
- Sender is cloned and only one clone is dropped
- Channel is `sync::mpsc` with multiple senders
- Sender is passed to spawned thread via `move` closure correctly
- Using `std::sync::mpsc::Sender::send` which returns Result

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-860 (Incorrect Synchronization)
  impact:
    reliability: High
    correctness: Critical
  fix_effort: Low
  remediation: |
    Ensure sender is not dropped before calling send().
    Use cloning for multi-producer scenarios.
    Check send() return value for errors.
    Consider using channels with explicit ownership transfer.
  review_questions:
    - "Is the sender still valid when send() is called?"
    - "Are all senders properly managed?"
    - "Is error handling for closed channel in place?"
```

### Estimated LOC
- Core rule: ~70 LOC
- Helpers: 2 functions (~20 LOC)
- Tests: 6 test cases (~55 LOC)
- **Total: ~145 LOC**

### Complexity Score
- Cognitive: 13/20
- Cyclomatic: 3/10
- Nesting: 2/10

---

## Rule: S1876
### metadata
- rule_id: S1876
- concept_id: concurrency-await-holding-refcell
- severity: MAJOR
- precision: high
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: RefCell::borrow() or RefCell::borrow_mut() active across .await
;; Detect: borrow expression whose result is used across await point

(let_declaration
  pattern: (identifier) @borrow_result
  value: (call_expression
    function: (field_expression
      object: (identifier) @refcell
      field: (field_identifier) @borrow_method)
  (#match? @borrow_method "^borrow$|^borrow_mut$"))
```

And then check if `await_expression` appears within the scope of this borrow.

```lisp
;; Secondary pattern: await inside block following borrow
(sequence_expression
  (let_declaration
    value: (call_expression
      function: (field_expression
        field: (field_identifier) @borrow))
  (async_block
    (block
      (expression_statement
        (await_expression)))))
```

### Code Examples

**Bad (S1876)**:
```rust
use std::cell::RefCell;
use tokio::time::{sleep, Duration};

fn bad_example() {
    let cell = RefCell::new(vec![1, 2, 3]);

    let borrowed = cell.borrow();  // RefCell borrow starts

    let future = async {
        println!("{:?}", borrowed.len());  // Still using borrowed
        sleep(Duration::from_secs(1)).await;  // YIELD: borrow still active!
        println!("{:?}", borrowed);  // borrow still active here
    };

    drop(borrowed);  // Must drop before await
}
```

**Bad (S1876 - async fn)**:
```rust
use std::cell::RefCell;

async fn bad_async(cell: RefCell<Vec<u32>>) {
    let v = cell.borrow();  // Borrow starts
    some_async_operation().await;  // DANGER: yield point with borrow active
    println!("{:?}", v);  // v still needed
}
```

**Good (S1876)**:
```rust
use std::cell::RefCell;

fn good_example() {
    let cell = RefCell::new(vec![1, 2, 3]);
    let v: Vec<u32>;

    {
        let borrowed = cell.borrow();
        v = borrowed.clone();  // Copy data while holding borrow
    }  // Borrow released

    let future = async {
        println!("{:?}", v.len());  // Using owned data
        some_async_operation().await;  // OK: no borrow active
    };
}
```

**Good (S1876 - use Mutex)**:
```rust
use std::sync::{Mutex, Arc};

async fn good_async(lock: Arc<Mutex<Vec<u32>>>) {
    let v: Vec<u32>;
    {
        let guard = lock.lock().unwrap();
        v = guard.clone();  // Copy from locked data
    }  // Guard released

    some_async_operation().await;  // OK
}
```

### False Positive Conditions
- `.await` does not actually yield (e.g., `tokio::spawn` with `.await`)
- RefCell is in a `Send + Sync` context
- Borrow is explicitly cloned before await (`drop(borrow)` before await)
- Using `std::cell::RefCell` with compile-time borrow checking disabled
- The async block doesn't actually capture the borrow (different scope)

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-825 (Expired Pointer Dereference)
  impact:
    correctness: Critical
    reliability: High
  fix_effort: Medium
  remediation: |
    Clone the data before the await point.
    Drop the borrow explicitly before await.
    Consider using Mutex/RwLock for async contexts.
    Use Arc<Mutex<T>> for cross-task sharing.
  review_questions:
    - "Is RefCell borrow held across an await point?"
    - "Could the data be cloned before the await?"
    - "Should this use a synchronization primitive instead?"
```

### Estimated LOC
- Core rule: ~85 LOC
- Helpers: 2 functions (~20 LOC)
- Tests: 5 test cases (~50 LOC)
- **Total: ~155 LOC** (slightly over budget)

### Complexity Score
- Cognitive: 15/20
- Cyclomatic: 3/10
- Nesting: 3/10

---

## Rule: S1877
### metadata
- rule_id: S1877
- concept_id: concurrency-unbounded-channel-no-backpressure
- severity: MINOR
- precision: medium
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: mpsc::channel() without bound (unbounded) + send in loop
;; Phase 1: Find unbounded channel creation
(call_expression
  function: (field_expression
    object: (identifier) @mpsc
    field: (field_identifier) @channel_fn)
  (#eq? @mpsc "mpsc")
  (#eq? @channel_fn "channel")
  arguments: (arguments))  // No argument = unbounded

;; Phase 2: Check for loop + send pattern
(loop_expression
  body: (block
    (expression_statement
      (call_expression
        function: (field_expression
          field: (field_identifier) @send)
        (#eq? @send "send")))))
```

### Code Examples

**Bad (S1877)**:
```rust
use std::sync::mpsc;

fn bad_example() {
    let (tx, rx) = mpsc::channel();  // UNBOUNDED - no backpressure

    for i in 0..1_000_000 {
        tx.send(i).unwrap();  // Could fill memory rapidly
    }
}
```

**Bad (S1877 - in async context)**:
```rust
use tokio::sync::mpsc;

async fn bad_async() {
    let (tx, mut rx) = mpsc::channel::<u32>(100);  // CHAN ERROR - already bounded

    tokio::spawn(async move {
        for i in 0..1_000_000 {
            tx.send(i).await.unwrap();  // OK: channel is bounded
        }
    });
}

fn bad_sync_producer() {
    let (tx, _rx) = std::sync::mpsc::channel();  // Unbounded

    loop {
        let data = produce_data();
        tx.send(data).unwrap();  // No backpressure!
    }
}
```

**Good (S1877)**:
```rust
use std::sync::mpsc;

fn good_example() {
    let (tx, rx) = mpsc::channel::<u32>(1000);  // BOUNDED - backpressure at 1000

    let mut count = 0;
    while count < 1_000_000 {
        match tx.send(count) {
            Ok(()) => count += 1,
            Err(_) => {
                // Channel full - apply backpressure
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}
```

**Good (S1877 - using select)**:
```rust
use tokio::sync::mpsc;

async fn good_async() {
    let (tx, mut rx) = mpsc::channel::<u32>(100);  // Bounded

    let handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            process(msg);
        }
    });

    for i in 0..1_000_000 {
        tx.send(i).await.unwrap();  // Backpressure when buffer full
    }
}
```

### False Positive Conditions
- Channel has explicit bounded capacity
- `select!` macro used for backpressure handling
- Producer rate is naturally limited
- Memory-bounded system with proper monitoring
- Short-lived channels that are always drained

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-400 (Uncontrolled Resource Consumption)
  impact:
    availability: Medium
    performance: Low
  fix_effort: Low
  remediation: |
    Use bounded channels with mpsc::channel(capacity).
    Implement backpressure when channel is full.
    Consider using tokio::sync::mpsc with proper capacity.
    Monitor channel depth in production.
  review_questions:
    - "Is the channel bounded?"
    - "Is there backpressure handling?"
    - "What happens if the channel fills up?"
```

### Estimated LOC
- Core rule: ~60 LOC
- Helpers: 2 functions (~15 LOC)
- Tests: 5 test cases (~45 LOC)
- **Total: ~120 LOC**

### Complexity Score
- Cognitive: 10/20
- Cyclomatic: 2/10
- Nesting: 2/10

---

## Rule: S1878
### metadata
- rule_id: S1878
- concept_id: concurrency-arc-clone-hot-path
- severity: MINOR
- precision: medium
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: Arc::clone() inside tight loop without justification
;; Phase 1: Find loop expressions
(loop_expression
  body: (block
    (expression_statement
      (call_expression
        function: (field_expression
          object: (identifier) @arc
          field: (field_identifier) @clone)
        (#eq? @clone "clone")))))

;; Also detect: for_expression with clone inside
(for_expression
  body: (block
    (expression_statement
      (call_expression
        function: (field_expression
          field: (field_identifier) @clone)
        (#eq? @clone "clone")))))
```

### Code Examples

**Bad (S1878)**:
```rust
use std::sync::Arc;

fn bad_example(data: Arc<Vec<u32>>) {
    // Unnecessary cloning in hot loop
    for _ in 0..1_000_000 {
        let local = data.clone();  // CLONE every iteration!
        process(local);
    }
}

fn another_bad() {
    let shared = Arc::new ExpensiveData::new();

    loop {
        // Clone in tight loop - each clone is atomic op + ref count bump
        let _local = shared.clone();
        do_work().await;
    }
}
```

**Good (S1878)**:
```rust
use std::sync::Arc;

fn good_example(data: Arc<Vec<u32>>) {
    // Clone ONCE before loop
    let local = Arc::clone(&data);  // One-time clone
    for _ in 0..1_000_000 {
        process(&local);  // No clone in loop
    }
}

fn also_good(data: Arc<Vec<u32>>) {
    let data_ref = Arc::clone(&data);
    tokio::spawn(async move {
        // Arc cloned once, moved into task
        do_work(data_ref).await;
    });
}
```

**Good (S1878 - different reasoning)**:
```rust
use std::sync::Arc;

fn task_per_item(items: Vec<Item>, shared: Arc<Config>) {
    for item in items {
        // Each iteration creates NEW Arc - this is actually correct!
        let shared_clone = Arc::clone(&shared);
        tokio::spawn(async move {
            process_with_config(item, shared_clone).await;
        });
    }
}
```

### False Positive Conditions
- Clone is intentional per-task (spawning multiple tasks with separate Arcs)
- Clone is outside the loop (acceptable pattern)
- Loop has low iteration count (< 10)
- Clone is needed for ownership transfer to spawned task
- Performance is not critical path

### Axiom Metadata

```yaml
axiom:
  category: concurrency-anti-pattern
  cwe: CWE-763 (Release of Invalid Pointer)
  impact:
    performance: Medium
    efficiency: Low
  fix_effort: Low
  remediation: |
    Clone the Arc once before the loop.
    Consider if Arc is the right choice for this pattern.
    Profile to confirm clone is actually a bottleneck.
    Consider using Rc for single-threaded contexts.
  review_questions:
    - "Is this clone necessary in every iteration?"
    - "Could the clone be moved outside the loop?"
    - "Is this a hot path where clones matter?"
```

### Estimated LOC
- Core rule: ~55 LOC
- Helpers: 2 functions (~15 LOC)
- Tests: 5 test cases (~45 LOC)
- **Total: ~115 LOC**

### Complexity Score
- Cognitive: 8/20
- Cyclomatic: 2/10
- Nesting: 2/10

---

## Rule: S1879
### metadata
- rule_id: S1879
- concept_id: concurrency-concurrent-map-unsync
- severity: MAJOR
- precision: high
- detection_strategy: tree-sitter query

### AST Pattern

```lisp
;; Pattern: HashMap/BTreeMap used in spawn/thread context without DashMap
;; Phase 1: Find spawn/thread::spawn calls
(call_expression
  function: (identifier) @spawn
  arguments: (arguments (closure_expression) @task_closure)
  (#match? @spawn "^spawn$|^std::thread::spawn$"))

;; Phase 2: Inside closure, find HashMap/BTreeMap insert/get/modify
(closure_expression
  body: (block
    (statement
      (expression_statement
        (call_expression
          function: (field_expression
            object: (identifier) @map
            field: (field_identifier) @method)
          (#match? @map "^map$|^data$|^cache$|^store$")
          (#match? @method "^insert$|^get$|^remove$|^entry$"))))))
```

### Code Examples

**Bad (S1879)**:
```rust
use std::collections::HashMap;
use std::thread;

fn bad_example() {
    let map = HashMap::new();  // NOT thread-safe!

    let handles: Vec<_> = (0..4).map(|i| {
        thread::spawn(move || {
            map.insert(i, i * 2);  // RACE: HashMap not thread-safe
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }
}

fn another_bad() {
    let cache = std::collections::BTreeMap::new();  // NOT thread-safe

    std::thread::spawn(|| {
        cache.insert("key", "value");  // RACE
    });

    std::thread::spawn(|| {
        let _ = cache.get("key");  // RACE
    });
}
```

**Good (S1879)**:
```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

fn good_example() {
    let map = Arc::new(Mutex::new(HashMap::new()));  // Thread-safe wrapper

    let handles: Vec<_> = (0..4).map(|i| {
        let map = Arc::clone(&map);
        thread::spawn(move || {
            let mut m = map.lock().unwrap();
            m.insert(i, i * 2);  // OK: synchronized access
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }
}
```

**Good (S1879 - DashMap)**:
```rust
use dashmap::DashMap;
use std::thread;

fn best_example() {
    let map = DashMap::new();  // Purpose-built for concurrent access

    let handles: Vec<_> = (0..4).map(|i| {
        thread::spawn(move || {
            map.insert(i, i * 2);  // OK: DashMap is thread-safe
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }
}
```

### False Positive Conditions
- HashMap/BTreeMap is only accessed from a single thread
- Synchronization is already provided via Mutex/RwLock wrapper
- Using `parking_lot::Mutex<HashMap>` (explicit synchronization)
- Data is read-only after initialization (no concurrent writes)
- Thread-local storage (each thread has its own map)

### Axiom Metadata

```yaml
axiom:
  category: concurrency-bug
  cwe: CWE-662 (Insufficient Synchronization)
  impact:
    correctness: Critical
    reliability: High
  fix_effort: Medium
  remediation: |
    Use DashMap for concurrent HashMap access.
    Or wrap in Arc<Mutex<HashMap>> / Arc<RwLock<HashMap>>.
    Consider std::sync::Map for Rust's concurrent hash map.
    Re-evaluate if concurrent access is actually needed.
  review_questions:
    - "Is this map accessed from multiple threads?"
    - "Should this use DashMap or a synchronized wrapper?"
    - "Is the access pattern read-heavy or write-heavy?"
```

### Estimated LOC
- Core rule: ~70 LOC
- Helpers: 2 functions (~20 LOC)
- Tests: 6 test cases (~55 LOC)
- **Total: ~145 LOC**

### Complexity Score
- Cognitive: 12/20
- Cyclomatic: 3/10
- Nesting: 2/10

---

## Summary Table

| Rule ID | Concept | Severity | LOC | Complexity | Detection |
|---------|---------|----------|-----|------------|-----------|
| S1872 | concurrency-race-condition | CRITICAL | ~160 | 14 | tree-sitter |
| S1873 | concurrency-mutex-guard-leaked | MAJOR | ~135 | 12 | tree-sitter |
| S1874 | concurrency-deadlock-lock-ordering | CRITICAL | ~190* | 18 | tree-sitter |
| S1875 | concurrency-channel-closed-send | CRITICAL | ~145 | 13 | tree-sitter |
| S1876 | concurrency-await-holding-refcell | MAJOR | ~155 | 15 | tree-sitter |
| S1877 | concurrency-unbounded-channel-no-backpressure | MINOR | ~120 | 10 | tree-sitter |
| S1878 | concurrency-arc-clone-hot-path | MINOR | ~115 | 8 | tree-sitter |
| S1879 | concurrency-concurrent-map-unsync | MAJOR | ~145 | 12 | tree-sitter |

*S1874 exceeds 150 LOC budget - recommend splitting into S1874a and S1874b

---

## Implementation Location

All rules should be implemented in:
`crates/cognicode-axiom/src/rules/rules/rust/bugs/concurrency/`

New files:
- `s1872_rule.rs` - Race condition
- `s1873_rule.rs` - Mutex guard leak
- `s1874a_rule.rs` - Deadlock detection (basic)
- `s1874b_rule.rs` - Deadlock detection (cross-function)
- `s1875_rule.rs` - Channel closed send
- `s1876_rule.rs` - Await holding RefCell
- `s1877_rule.rs` - Unbounded channel
- `s1878_rule.rs` - Arc clone hot path
- `s1879_rule.rs` - Concurrent map unsync
- `mod.rs` - Module exports

---

## Next Steps

1. **Design Approved**: Save this document to Engram topic `rules/bug-concurrency/designs`
2. **State Update**: Mark design phase complete in `rules/bug-concurrency/state`
3. **Implementation Phase**: Move to sdd-apply for coding the rules
4. **Testing Phase**: Create unit tests and integration tests
5. **Verification Phase**: Run against known bug patterns and verify detection

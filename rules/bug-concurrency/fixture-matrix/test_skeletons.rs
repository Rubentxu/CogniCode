// Rust test skeletons for bug-concurrency rules
// Generated: 2026-05-14
// Total rules: 8

#[cfg(test)]
mod cc_conc001_tests {
    // Rule: Race condition: shared mutable state without synchronization
    // Detection: AST-based (shared mutability detection + sync primitive analysis)

    #[test]
    fn test_cc_conc001_positive_case_1() {
        // Shared mutable counter without synchronization
        let code = r#"
        static mut COUNTER: u32 = 0;

        fn increment() {
            unsafe { COUNTER += 1; }
        }
        "#;
        // Expected: trigger - static mutable without synchronization
        // assert rule detects issue at line 1
    }

    #[test]
    fn test_cc_conc001_positive_case_2() {
        // Global mutable state accessed from multiple threads
        let code = r#"
        use std::thread;

        static mut DATA: Vec<u32> = Vec::new();

        fn worker() {
            unsafe { DATA.push(42); }
        }

        fn main() {
            let handles: Vec<_> = (0..4).map(|_| thread::spawn(worker)).collect();
            for h in handles { h.join().unwrap(); }
        }
        "#;
        // Expected: trigger - global mutable shared across threads
        // assert rule detects issue at line 3
    }

    #[test]
    fn test_cc_conc001_positive_case_3() {
        // Struct with interior mutability accessed concurrently
        let code = r#"
        use std::rc::Rc;
        use std::cell::RefCell;
        use std::thread;

        struct Counter { count: u32 }

        fn main() {
            let shared = Rc::new(RefCell::new(Counter { count: 0 }));
            let shared2 = shared.clone();

            let _ = thread::spawn(move || {
                shared2.borrow_mut().count += 1;
            }).join();

            println!("{}", shared.borrow().count);
        }
        "#;
        // Expected: trigger - Rc<RefCell<T>> is not thread-safe
        // assert rule detects issue at line 9
    }

    #[test]
    fn test_cc_conc001_negative_case_1() {
        // Arc<Mutex<T>> properly synchronized
        let code = r#"
        use std::sync::{Arc, Mutex};
        use std::thread;

        fn main() {
            let counter = Arc::new(Mutex::new(0));
            let mut handles = vec![];

            for _ in 0..4 {
                let c = counter.clone();
                handles.push(thread::spawn(move || {
                    let mut num = c.lock().unwrap();
                    *num += 1;
                }));
            }

            for h in handles { h.join().unwrap(); }
            println!("{}", *counter.lock().unwrap());
        }
        "#;
        // Expected: no_trigger - properly synchronized with Arc<Mutex>
    }

    #[test]
    fn test_cc_conc001_negative_case_2() {
        // Atomic counter using AtomicU32
        let code = r#"
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::thread;

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        fn main() {
            let handles: Vec<_> = (0..4).map(|_| {
                thread::spawn(|| { COUNTER.fetch_add(1, Ordering::SeqCst); })
            }).collect();

            for h in handles { h.join().unwrap(); }
            println!("{}", COUNTER.load(Ordering::SeqCst));
        }
        "#;
        // Expected: no_trigger - atomic operations are thread-safe
    }

    #[test]
    fn test_cc_conc001_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc001_edge_case_2() {
        // Single thread, no sharing
        let code = r#"
        fn main() {
            let mut counter = 0;
            counter += 1;
            println!("{}", counter);
        }
        "#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc001_false_positive_guard_1() {
        // Comment mentions shared mutability
        let code = r#"
        // WARNING: shared mutable state here
        static FOO: u32 = 0;
        "#;
        // Expected: no_trigger - comments should not trigger
    }

    #[test]
    fn test_cc_conc001_false_positive_guard_2() {
        // Constant data is immutable and safe
        let code = r#"
        const MAX_CONNECTIONS: u32 = 100;
        "#;
        // Expected: no_trigger - constants are immutable
    }
}

#[cfg(test)]
mod cc_conc002_tests {
    // Rule: Mutex guard leaked: MutexGuard not released before return/.await
    // Detection: AST-based (borrow analysis + control flow + await detection)

    #[test]
    fn test_cc_conc002_positive_case_1() {
        // MutexGuard returned without drop
        let code = r#"
        use std::sync::Mutex;

        fn get_lock<'a>(lock: &'a Mutex<u32>) -> &'a u32 {
            let guard = lock.lock().unwrap();
            return &*guard;
        }
        "#;
        // Expected: trigger - guard dropped but borrow escapes
    }

    #[test]
    fn test_cc_conc002_positive_case_2() {
        // Early return with guard in scope
        let code = r#"
        use std::sync::Mutex;

        fn process(lock: &Mutex<Vec<u32>>) -> usize {
            let mut guard = lock.lock().unwrap();
            if guard.len() > 100 {
                return guard.len();
            }
            guard.push(42);
            guard.len()
        }
        "#;
        // Expected: trigger - early return with guard
    }

    #[test]
    fn test_cc_conc002_positive_case_3() {
        // RwLockReadGuard returned
        let code = r#"
        use std::sync::RwLock;

        fn read_value<'a>(lock: &'a RwLock<u32>) -> &'a u32 {
            let guard = lock.read().unwrap();
            return &*guard;
        }
        "#;
        // Expected: trigger - RwLockGuard also subject to leak
    }

    #[test]
    fn test_cc_conc002_negative_case_1() {
        // Guard explicitly dropped before return
        let code = r#"
        use std::sync::Mutex;

        fn get_value(lock: &Mutex<u32>) -> u32 {
            let guard = lock.lock().unwrap();
            let value = *guard;
            drop(guard);
            value
        }
        "#;
        // Expected: no_trigger - guard explicitly dropped
    }

    #[test]
    fn test_cc_conc002_negative_case_2() {
        // Guard used in scope and naturally dropped
        let code = r#"
        use std::sync::Mutex;

        fn get_len(lock: &Mutex<Vec<u32>>) -> usize {
            let guard = lock.lock().unwrap();
            guard.len()
        }
        "#;
        // Expected: no_trigger - guard dropped at end of scope
    }

    #[test]
    fn test_cc_conc002_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc002_false_positive_guard_1() {
        // Variable named mutex_guard
        let code = r#"
        let mutex_guard = 42;
        "#;
        // Expected: no_trigger - not an actual MutexGuard
    }
}

#[cfg(test)]
mod cc_conc003_tests {
    // Rule: Deadlock: nested locks acquired in inconsistent order
    // Detection: AST-based (lock ordering graph analysis)

    #[test]
    fn test_cc_conc003_positive_case_1() {
        // Lock A then B in one function, B then A in another
        let code = r#"
        use std::sync::Mutex;

        static LOCK_A: Mutex<()> = Mutex::new(());
        static LOCK_B: Mutex<()> = Mutex::new(());

        fn func1() {
            let a = LOCK_A.lock().unwrap();
            let b = LOCK_B.lock().unwrap();
        }

        fn func2() {
            let b = LOCK_B.lock().unwrap();
            let a = LOCK_A.lock().unwrap();
        }
        "#;
        // Expected: trigger - inconsistent lock ordering
    }

    #[test]
    fn test_cc_conc003_positive_case_2() {
        // Three locks with inconsistent ordering
        let code = r#"
        use std::sync::Mutex;

        static L1: Mutex<()> = Mutex::new(());
        static L2: Mutex<()> = Mutex::new(());
        static L3: Mutex<()> = Mutex::new(());

        fn order_xyz() {
            let x = L1.lock().unwrap();
            let y = L2.lock().unwrap();
            let z = L3.lock().unwrap();
        }

        fn order_zyx() {
            let z = L3.lock().unwrap();
            let y = L2.lock().unwrap();
            let x = L1.lock().unwrap();
        }
        "#;
        // Expected: trigger - three locks with inverted order
    }

    #[test]
    fn test_cc_conc003_negative_case_1() {
        // Consistent lock ordering across all functions
        let code = r#"
        use std::sync::Mutex;

        static L1: Mutex<()> = Mutex::new(());
        static L2: Mutex<()> = Mutex::new(());

        fn func_a() {
            let l1 = L1.lock().unwrap();
            let l2 = L2.lock().unwrap();
        }

        fn func_b() {
            let l1 = L1.lock().unwrap();
            let l2 = L2.lock().unwrap();
        }
        "#;
        // Expected: no_trigger - consistent ordering
    }

    #[test]
    fn test_cc_conc003_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc003_false_positive_guard_1() {
        // Dynamic locking cannot be statically analyzed
        let code = r#"
        use std::sync::Mutex;

        fn dynamic_lock(locks: &[&Mutex<()>]) {
            for lock in locks {
                let _g = lock.lock().unwrap();
            }
        }
        "#;
        // Expected: no_trigger - dynamic lock acquisition
    }
}

#[cfg(test)]
mod cc_conc004_tests {
    // Rule: Channel closed/broken: send on closed channel
    // Detection: AST-based (channel send detection + sender lifecycle)

    #[test]
    fn test_cc_conc004_positive_case_1() {
        // Send after Sender is dropped
        let code = r#"
        use std::sync::mpsc;

        fn main() {
            let (tx, rx) = mpsc::channel::<u32>();
            drop(tx);
            let (tx2, _rx2) = mpsc::channel();
            tx2.send(42).unwrap();
            rx.recv().unwrap();
        }
        "#;
        // Expected: trigger - tx dropped, channel closed
    }

    #[test]
    fn test_cc_conc004_positive_case_2() {
        // Send on channel after receiver dropped
        let code = r#"
        use std::sync::mpsc;

        fn main() {
            let (tx, rx) = mpsc::channel();
            drop(rx);

            let tx2 = tx.clone();
            std::thread::spawn(move || {
                tx2.send(42).unwrap();
            });
        }
        "#;
        // Expected: trigger - receiver dropped, channel closed
    }

    #[test]
    fn test_cc_conc004_negative_case_1() {
        // Sender kept alive while sending
        let code = r#"
        use std::sync::mpsc;

        fn main() {
            let (tx, rx) = mpsc::channel();
            tx.send(42).unwrap();
            drop(tx);
            drop(rx);
        }
        "#;
        // Expected: no_trigger - sender still alive during send
    }

    #[test]
    fn test_cc_conc004_negative_case_2() {
        // try_send is explicitly fallible
        let code = r#"
        use std::sync::mpsc;

        fn main() {
            let (tx, _rx) = mpsc::channel::<u32>();

            if let Err(_) = tx.try_send(42) {
                println!("Channel closed");
            }
        }
        "#;
        // Expected: no_trigger - try_send is fallible
    }

    #[test]
    fn test_cc_conc004_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc004_false_positive_guard_1() {
        // Variable named sender
        let code = r#"
        let sender = 42;
        "#;
        // Expected: no_trigger - not an actual Sender
    }
}

#[cfg(test)]
mod cc_conc005_tests {
    // Rule: RefCell borrowed across await: RefCell::borrow() held across .await
    // Detection: AST-based (borrow scope + await point detection)

    #[test]
    fn test_cc_conc005_positive_case_1() {
        // RefCell borrow held across async await
        let code = r#"
        use std::cell::RefCell;

        async fn bad_read(refcell: &RefCell<u32>) -> u32 {
            let value = refcell.borrow();
            async_op().await;
            *value
        }

        async fn async_op() {}
        "#;
        // Expected: trigger - borrow held across await
    }

    #[test]
    fn test_cc_conc005_positive_case_2() {
        // Mutable borrow across await
        let code = r#"
        use std::cell::RefCell;

        async fn bad_write(refcell: &RefCell<u32>) {
            let mut value = refcell.borrow_mut();
            some_async_call().await;
            *value = 42;
        }

        async fn some_async_call() {}
        "#;
        // Expected: trigger - mutable borrow held across await
    }

    #[test]
    fn test_cc_conc005_negative_case_1() {
        // Borrow dropped before await
        let code = r#"
        use std::cell::RefCell;

        async fn good_read(refcell: &RefCell<u32>) -> u32 {
            {
                let value = refcell.borrow();
                *value
            }
            async_op().await;
            42
        }

        async fn async_op() {}
        "#;
        // Expected: no_trigger - borrow dropped before await
    }

    #[test]
    fn test_cc_conc005_negative_case_2() {
        // Use after await pattern
        let code = r#"
        use std::cell::RefCell;

        async fn use_after_await(refcell: &RefCell<u32>) -> u32 {
            some_async_call().await;
            *refcell.borrow()
        }

        async fn some_async_call() {}
        "#;
        // Expected: no_trigger - borrow only after await completes
    }

    #[test]
    fn test_cc_conc005_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc005_edge_case_2() {
        // No await in function
        let code = r#"
        use std::cell::RefCell;

        fn sync_read(refcell: &RefCell<u32>) -> u32 {
            *refcell.borrow()
        }
        "#;
        // Expected: no_trigger - no await point
    }

    #[test]
    fn test_cc_conc005_false_positive_guard_1() {
        // Variable named refcell
        let code = r#"
        let refcell = 42;
        "#;
        // Expected: no_trigger - not an actual RefCell
    }
}

#[cfg(test)]
mod cc_conc006_tests {
    // Rule: Unbounded channel without backpressure mechanism
    // Detection: AST-based (mpsc::channel detection + backpressure pattern analysis)

    #[test]
    fn test_cc_conc006_positive_case_1() {
        // Unbounded channel without backpressure
        let code = r#"
        use std::sync::mpsc;
        use std::thread;

        fn main() {
            let (tx, rx) = mpsc::channel();

            for i in 0..1_000_000 {
                tx.send(i).unwrap();
            }

            while let Ok(v) = rx.recv() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        "#;
        // Expected: trigger - unbounded channel, no backpressure
    }

    #[test]
    fn test_cc_conc006_positive_case_2() {
        // Multiple producers unbounded channel
        let code = r#"
        use std::sync::mpsc;
        use std::thread;

        fn main() {
            let (tx, rx) = mpsc::channel();

            for i in 0..100 {
                let tx_clone = tx.clone();
                thread::spawn(move || {
                    for j in 0..10_000 {
                        tx_clone.send(j).unwrap();
                    }
                });
            }

            drop(tx);
            let _ = rx.iter().collect::<Vec<_>>();
        }
        "#;
        // Expected: trigger - unbounded with multiple producers
    }

    #[test]
    fn test_cc_conc006_negative_case_1() {
        // Bounded channel with capacity limit
        let code = r#"
        use std::sync::mpsc;

        fn bounded_channel() {
            let (tx, rx) = mpsc::channel::<u32>(10);

            tx.send(1).unwrap();
            tx.send(2).unwrap();

            let _ = rx.recv();
        }
        "#;
        // Expected: no_trigger - bounded channel
    }

    #[test]
    fn test_cc_conc006_negative_case_2() {
        // sync_channel provides backpressure
        let code = r#"
        use std::sync::mpsc;

        fn synchronous_channel() {
            let (tx, rx) = mpsc::sync_channel(1);

            tx.send(1).unwrap();
            rx.recv().unwrap();
        }
        "#;
        // Expected: no_trigger - sync_channel provides backpressure
    }

    #[test]
    fn test_cc_conc006_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc006_false_positive_guard_1() {
        // Variable named channel
        let code = r#"
        let channel = 42;
        "#;
        // Expected: no_trigger - not an actual channel
    }
}

#[cfg(test)]
mod cc_conc007_tests {
    // Rule: Arc::clone in hot path without justification
    // Detection: AST-based (Arc::clone detection + hot path heuristics)

    #[test]
    fn test_cc_conc007_positive_case_1() {
        // Arc::clone inside tight loop
        let code = r#"
        use std::sync::Arc;
        use std::thread;

        fn process_items() {
            let data = Arc::new(vec![1, 2, 3, 4, 5]);

            for i in 0..1000 {
                let data_clone = data.clone();
                thread::spawn(move || {
                    let _ = data_clone;
                });
            }
        }
        "#;
        // Expected: trigger - clone in every iteration
    }

    #[test]
    fn test_cc_conc007_positive_case_2() {
        // Arc::clone in recursive function
        let code = r#"
        use std::sync::Arc;

        fn recursive_compute(data: Arc<Vec<u32>>, depth: u32) -> u32 {
            if depth == 0 {
                return data.iter().sum();
            }

            let data_clone = data.clone();
            recursive_compute(data_clone, depth - 1)
        }
        "#;
        // Expected: trigger - unnecessary clone at each recursion
    }

    #[test]
    fn test_cc_conc007_negative_case_1() {
        // Arc::clone once before loop (efficient)
        let code = r#"
        use std::sync::Arc;
        use std::thread;

        fn efficient_clone() {
            let data = Arc::new(vec![1, 2, 3]);
            let data_clone = data.clone();

            let handles: Vec<_> = (0..4).map(|_| {
                let d = data_clone.clone();
                thread::spawn(move || { *d })
            }).collect();

            for h in handles { h.join().unwrap(); }
        }
        "#;
        // Expected: no_trigger - clone once, use multiple times
    }

    #[test]
    fn test_cc_conc007_negative_case_2() {
        // Clone moved to thread (necessary)
        let code = r#"
        use std::sync::Arc;
        use std::thread;

        fn necessary_clone() {
            let data = Arc::new(42);

            let handle = thread::spawn({
                let data = data.clone();
                move || { *data }
            });

            handle.join().unwrap();
        }
        "#;
        // Expected: no_trigger - necessary for thread ownership
    }

    #[test]
    fn test_cc_conc007_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc007_edge_case_2() {
        // Single clone, single use
        let code = r#"
        use std::sync::Arc;

        fn single_use() {
            let data = Arc::new(42);
            let clone = data.clone();
            println!("{}", *clone);
        }
        "#;
        // Expected: no_trigger - one clone is often necessary
    }

    #[test]
    fn test_cc_conc007_false_positive_guard_1() {
        // Variable named arc_clone
        let code = r#"
        let arc_clone = 42;
        "#;
        // Expected: no_trigger - not an actual Arc::clone
    }
}

#[cfg(test)]
mod cc_conc008_tests {
    // Rule: Concurrent map access without synchronization: HashMap/BTreeMap in multi-threaded context
    // Detection: AST-based (HashMap/BTreeMap detection + thread context analysis)

    #[test]
    fn test_cc_conc008_positive_case_1() {
        // HashMap shared between threads without sync
        let code = r#"
        use std::collections::HashMap;
        use std::thread;
        use std::sync::Arc;

        fn main() {
            let shared_map = Arc::new(HashMap::new());

            let handle = thread::spawn({
                let map = shared_map.clone();
                move || {
                    map.insert("key", 42);
                }
            });

            handle.join().unwrap();
            shared_map.get("key");
        }
        "#;
        // Expected: trigger - HashMap is not thread-safe
    }

    #[test]
    fn test_cc_conc008_positive_case_2() {
        // BTreeMap accessed concurrently
        let code = r#"
        use std::collections::BTreeMap;
        use std::thread;
        use std::sync::Arc;

        fn btree_concurrent() {
            let map = Arc::new(BTreeMap::new());

            let map2 = map.clone();
            thread::spawn(move || {
                map2.insert(1, "one");
            }).join();

            map.get(&1);
        }
        "#;
        // Expected: trigger - BTreeMap is not thread-safe
    }

    #[test]
    fn test_cc_conc008_negative_case_1() {
        // DashMap instead of HashMap for concurrent access
        let code = r#"
        use std::collections::HashMap;
        use dashmap::DashMap;
        use std::thread;

        fn correct_concurrent_map() {
            let map = DashMap::new();

            let handles: Vec<_> = (0..4).map(|_| {
                let map_clone = map.clone();
                thread::spawn(move || {
                    map_clone.insert("key", 42);
                })
            }).collect();

            for h in handles { h.join().unwrap(); }

            assert_eq!(*map.get("key").unwrap(), 42);
        }
        "#;
        // Expected: no_trigger - DashMap is thread-safe
    }

    #[test]
    fn test_cc_conc008_negative_case_2() {
        // RwLock<HashMap> provides thread safety
        let code = r#"
        use std::collections::HashMap;
        use std::sync::{Arc, RwLock};
        use std::thread;

        fn locked_hashmap() {
            let map = Arc::new(RwLock::new(HashMap::new()));

            let handles: Vec<_> = (0..4).map(|_| {
                let m = map.clone();
                thread::spawn(move || {
                    let mut map = m.write().unwrap();
                    map.insert("key", 42);
                })
            }).collect();

            for h in handles { h.join().unwrap(); }
        }
        "#;
        // Expected: no_trigger - protected by RwLock
    }

    #[test]
    fn test_cc_conc008_edge_case_1() {
        // Empty file
        let code = r#""#;
        // Expected: no_trigger
    }

    #[test]
    fn test_cc_conc008_edge_case_2() {
        // HashMap in single-threaded context
        let code = r#"
        use std::collections::HashMap;

        fn single_thread() {
            let mut map = HashMap::new();
            map.insert("key", 42);
            println!("{:?}", map.get("key"));
        }
        "#;
        // Expected: no_trigger - single-threaded context
    }

    #[test]
    fn test_cc_conc008_false_positive_guard_1() {
        // Variable named hashmap
        let code = r#"
        let hashmap = 42;
        "#;
        // Expected: no_trigger - not an actual HashMap
    }
}

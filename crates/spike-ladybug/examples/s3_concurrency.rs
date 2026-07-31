//! E29 S3 in-process concurrency probe — validates lbug 0.19.0 single-writer constraint.
//!
//! Three sections:
//!   1. Write contention: c1 and c2 concurrent writes → one errors
//!   2. Four concurrent readers: all see the same snapshot of N rows
//!   3. MVCC snapshot isolation: reader sees old data until new txn begins

use lbug::{Connection, Database, SystemConfig, Value};
use tempfile::TempDir;

fn main() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("s3_concurrency.lbdb");
    let path_str = db_path.to_str().unwrap();

    // -------------------------------------------------------------------------
    // Section 1: Write contention
    // -------------------------------------------------------------------------
    println!("=== Section 1: Write contention ===");

    let db = Database::new(path_str, SystemConfig::default())?;

    // Create table
    {
        let conn = Connection::new(&db)?;
        conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")?;
        conn.query("CREATE (:Test {id: 1, name: 'initial'});")?;
    }

    // Use Barrier to synchronize both threads starting concurrent writes
    let barrier = std::sync::Barrier::new(2);
    let db_ref = &db;

    // Both threads in same scope
    let contention_error = std::thread::scope(|s| {
        let handle1 = s.spawn(|| {
            let c = Connection::new(db_ref).unwrap();
            let _ = barrier.wait();
            let _ = barrier.wait();
            c.query("CREATE (:Test {id: 2, name: 'from_c1'});")
        });

        let handle2 = s.spawn(|| {
            let c = Connection::new(db_ref).unwrap();
            let _ = barrier.wait();
            let _ = barrier.wait();
            c.query("CREATE (:Test {id: 3, name: 'from_c2'});")
        });

        let r1 = handle1.join().unwrap();
        let r2 = handle2.join().unwrap();

        // One should succeed, one should fail
        let err = if r1.is_ok() {
            println!("c1 succeeded, c2 got error");
            r2.unwrap_err()
        } else {
            println!("c2 succeeded, c1 got error");
            r1.unwrap_err()
        };
        err
    });

    println!("Contention error: {:?}", contention_error);

    // Verify retry works
    println!("\nVerifying retry succeeds after first writer commits...");
    {
        let conn = Connection::new(&db)?;
        let retry = conn.query("CREATE (:Test {id: 4, name: 'retry'});");
        println!("Retry after contention: {:?}", retry.is_ok());
    }

    // -------------------------------------------------------------------------
    // Section 2: Four concurrent readers
    // -------------------------------------------------------------------------
    println!("\n=== Section 2: Four concurrent readers ===");

    let db2 = Database::new(path_str, SystemConfig::default())?;

    let reader_counts: Vec<i64> = std::thread::scope(|s| {
        let mut handles = vec![];
        for i in 0..4 {
            let db_ref = &db2;
            handles.push(s.spawn(move || {
                let c = Connection::new(db_ref).unwrap();
                let count: i64 = {
                    let mut rows = c.query("MATCH (t:Test) RETURN count(t);").unwrap();
                    let row = rows.next().unwrap();
                    if let Value::Int64(n) = &row[0] { *n } else { 0 }
                };
                println!("Reader {} sees {} rows", i, count);
                count
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let all_same = reader_counts.iter().all(|&c| c == reader_counts[0]);
    println!("All 4 readers saw same count: {}", all_same);

    // -------------------------------------------------------------------------
    // Section 3: MVCC snapshot isolation
    // -------------------------------------------------------------------------
    println!("\n=== Section 3: MVCC snapshot isolation ===");

    let db3 = Database::new(path_str, SystemConfig::default())?;
    let conn_read = Connection::new(&db3)?;

    let count_before: i64 = {
        let mut rows = conn_read.query("MATCH (t:Test) RETURN count(t);")?;
        let row = rows.next().unwrap();
        if let Value::Int64(n) = &row[0] { *n } else { 0 }
    };
    println!("Reader sees {} rows before new write", count_before);

    let conn_write = Connection::new(&db3)?;
    conn_write.query("CREATE (:Test {id: 100, name: 'new_row'});")?;

    let count_after: i64 = {
        let mut rows = conn_read.query("MATCH (t:Test) RETURN count(t);")?;
        let row = rows.next().unwrap();
        if let Value::Int64(n) = &row[0] { *n } else { 0 }
    };
    println!("Reader sees {} rows after new write", count_after);

    println!("\n=== All sections complete ===");

    Ok(())
}
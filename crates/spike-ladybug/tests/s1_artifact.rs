//! E29 S1 artifact test — validates lbug 0.19.0 round-trips and .lbdb artifact.
use lbug::{Connection, Database, SystemConfig, Value};
use tempfile::TempDir;

#[test]
fn s1_creates_lbdb_directory_and_round_trips_one_row() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("spike.lbdb");
    let path_str = db_path.to_str().unwrap();

    let db = Database::new(path_str, SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")
        .expect("CREATE TABLE");
    conn.query("CREATE (:Test {id: 1, name: 'hello'});")
        .expect("INSERT");

    let mut rows: Vec<(i64, String)> = Vec::new();
    for row in conn
        .query("MATCH (t:Test) RETURN t.id, t.name;")
        .expect("MATCH")
    {
        if let (Value::Int64(id), Value::String(name)) = (&row[0], &row[1]) {
            rows.push((*id, name.clone()));
        }
    }
    assert_eq!(rows, vec![(1, "hello".to_string())]);

    drop(conn);
    drop(db);

    assert!(
        db_path.is_dir(),
        "expected .lbdb directory at {path_str}"
    );
}

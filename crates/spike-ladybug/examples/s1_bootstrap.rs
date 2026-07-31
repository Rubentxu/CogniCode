//! E29 S1 bootstrap example — validates lbug 0.19.0 round-trips one row.
use lbug::{Connection, Database, SystemConfig, Value};

fn main() -> anyhow::Result<()> {
    let db = Database::new("spike.lbdb", SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")?;
    conn.query("CREATE (:Test {id: 1, name: 'hello'});")?;

    for row in conn.query("MATCH (t:Test) RETURN t.id, t.name;")? {
        if let (Value::Int64(id), Value::String(name)) = (&row[0], &row[1]) {
            println!("id={id} name={name}");
        }
    }

    Ok(())
}

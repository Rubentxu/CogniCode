use sddk_domain::event_envelope::{ActorRef, EntityRef, EventEnvelopeV1};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::process::ExitCode;

#[derive(Debug)]
struct Row {
    sequence: i64,
    event_id: String,
    event_type: String,
    stream_id: String,
    #[allow(dead_code)]
    schema_version: i64,
    project_id: String,
    occurred_at: String,
    #[allow(dead_code)]
    recorded_at: String,
    actor_json: String,
    subjects_json: String,
    payload_json: String,
    evidence_refs_json: String,
    metadata_json: Option<String>,
    old_content_hash: String,
    causation_id: Option<String>,
    correlation_id: Option<String>,
    cycle_id: Option<String>,
    frame_id: Option<String>,
    fork_id: Option<String>,
}

fn read_all_events(conn: &rusqlite::Connection) -> Vec<Row> {
    let mut stmt = conn.prepare(
        "SELECT sequence, event_id, event_type, stream_id, schema_version, project_id, occurred_at, recorded_at,
                actor_json, subjects_json, payload_json, evidence_refs_json, metadata_json, content_hash,
                causation_id, correlation_id, cycle_id, frame_id, fork_id
         FROM events_v1 ORDER BY stream_id, sequence"
    ).expect("prep");
    stmt.query_map([], |r| {
        Ok(Row {
            sequence: r.get(0)?,
            event_id: r.get(1)?,
            event_type: r.get(2)?,
            stream_id: r.get(3)?,
            schema_version: r.get(4)?,
            project_id: r.get(5)?,
            occurred_at: r.get(6)?,
            recorded_at: r.get(7)?,
            actor_json: r.get(8)?,
            subjects_json: r.get(9)?,
            payload_json: r.get(10)?,
            evidence_refs_json: r.get(11)?,
            metadata_json: r.get(12)?,
            old_content_hash: r.get(13)?,
            causation_id: r.get(14)?,
            correlation_id: r.get(15)?,
            cycle_id: r.get(16)?,
            frame_id: r.get(17)?,
            fork_id: r.get(18)?,
        })
    }).expect("map").map(|r| r.expect("row")).collect()
}

// Use the canonical EventEnvelopeV1 + compute_content_hash to ensure
// bit-perfect parity with the framework's verifier.
fn compute_content_hash_from_value(row: &Row, subjects_value: &Value, evidence_refs_value: &[String]) -> String {
    let actor_v: Value = serde_json::from_str(&row.actor_json).expect("parse actor");
    let actor_kind_str = actor_v.get("kind").and_then(|v| v.as_str()).expect("actor.kind");
    let actor = ActorRef {
        kind: match actor_kind_str {
            "human" => sddk_domain::event_envelope::ActorKind::Human,
            "agent" => sddk_domain::event_envelope::ActorKind::Agent,
            "system" => sddk_domain::event_envelope::ActorKind::System,
            _ => panic!("unknown actor.kind: {}", actor_kind_str),
        },
        id: actor_v.get("id").and_then(|v| v.as_str()).expect("actor.id").to_string(),
        definition_hash: actor_v.get("definition_hash").and_then(|v| v.as_str()).map(String::from),
        policy_hash: actor_v.get("policy_hash").and_then(|v| v.as_str()).map(String::from),
        model: actor_v.get("model").and_then(|v| v.as_str()).map(String::from),
        role: actor_v.get("role").and_then(|v| v.as_str()).map(String::from),
    };
    let subjects: Vec<EntityRef> = if let Value::Array(arr) = subjects_value {
        arr.iter()
            .filter_map(|v| serde_json::from_value::<EntityRef>(v.clone()).ok())
            .collect()
    } else {
        vec![]
    };
    let payload_v: Value = if row.payload_json.is_empty() { Value::Null } else {
        serde_json::from_str(&row.payload_json).expect("parse payload")
    };
    let metadata_v: Option<Value> = row.metadata_json.as_ref().filter(|s| !s.is_empty())
        .map(|s| serde_json::from_str(s).expect("parse metadata"));
    let env = EventEnvelopeV1 {
        event_id: row.event_id.clone(),
        event_type: row.event_type.clone(),
        schema_version: 1,
        stream_id: row.stream_id.clone(),
        sequence: 0, // excluded from hash by compute_content_hash
        project_id: row.project_id.clone(),
        occurred_at: row.occurred_at.clone(),
        recorded_at: String::new(), // excluded from hash
        actor,
        subjects,
        payload: payload_v,
        evidence_refs: evidence_refs_value.to_vec(),
        content_hash: String::new(), // excluded from hash
        metadata: metadata_v,
        causation_id: row.causation_id.clone(),
        correlation_id: row.correlation_id.clone(),
        cycle_id: row.cycle_id.clone(),
        frame_id: row.frame_id.clone(),
        fork_id: row.fork_id.clone(),
    };
    env.compute_content_hash()
}

// Compute the chain_hash inline. The framework computes it the same way
// at runtime (see event_store.rs); we don't expose a helper in the domain.
fn chain_hash(content_hash: &str, prev_chain_hash: &str) -> String {
    let input: Vec<u8> = if prev_chain_hash.is_empty() {
        content_hash.as_bytes().iter().chain("genesis".as_bytes()).cloned().collect()
    } else {
        content_hash.as_bytes().iter().chain(prev_chain_hash.as_bytes()).cloned().collect()
    };
    let digest = Sha256::digest(&input);
    format!("sha256:{:x}", digest)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: {} <input_db> <output_db>", args[0]);
        return ExitCode::from(2);
    }
    let input_db = &args[1];
    let output_db = &args[2];

    println!("=== SDDK ledger compatibility migration ===");
    println!("SDDK framework version: 1.169.62 (commit b9e928c)");
    println!("Source: {}", input_db);
    println!("Target: {}", output_db);

    // 1. fingerprint source
    let src_bytes = std::fs::read(input_db).expect("read source");
    let src_sha = Sha256::digest(&src_bytes);
    println!("Source SHA-256: sha256:{:x}", src_sha);

    // 2. consistent backup
    {
        use rusqlite::backup::Backup;
        let src = rusqlite::Connection::open(input_db).expect("open source");
        let mut dst = rusqlite::Connection::open(output_db).expect("open target");
        let backup = Backup::new(&src, &mut dst).expect("backup init");
        backup.run_to_completion(100, std::time::Duration::from_millis(100), None).expect("backup run");
    }
    println!("Backup created (consistent via SQLite backup API).");

    // 3. read target
    let conn = rusqlite::Connection::open(output_db).expect("open target");

    // 4. find malformed events (exact match on the field name `kind`, not
    //    substring — `"cycle"` contains `kind` but is not malformed).
    let malformed_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events_v1
         WHERE subjects_json LIKE '%\"kind\":%'
           AND subjects_json NOT LIKE '%\"type\":%'",
        [],
        |r| r.get(0),
    ).expect("count malformed");
    println!("Malformed events with `kind` field: {}", malformed_count);
    if malformed_count != 2 {
        eprintln!("FAIL: expected exactly 2 malformed events; abort.");
        return ExitCode::from(1);
    }
    let mut stmt = conn.prepare(
        "SELECT sequence, event_type, subjects_json FROM events_v1
         WHERE subjects_json LIKE '%\"kind\":%' AND subjects_json NOT LIKE '%\"type\":%'
         ORDER BY sequence"
    ).expect("prep");
    let malformed: Vec<(i64, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("map").map(|r| r.expect("row")).collect();

    let expected: HashMap<i64, &'static str> = [
        (4_i64, "cycle.supersede.requested"),
        (5_i64, "cycle.supersede.applied"),
    ].iter().map(|(k,v)| (*k, *v)).collect();
    for m in &malformed {
        match expected.get(&m.0) {
            Some(et) if *et == m.1.as_str() => {},
            _ => {
                eprintln!("FAIL: precondition violated: sequence={} type={}", m.0, m.1);
                return ExitCode::from(1);
            }
        }
        if m.2.contains("\"type\"") {
            eprintln!("FAIL: subjects_json already has `type` for sequence {}; cannot migrate", m.0);
            return ExitCode::from(1);
        }
    }
    println!("All preconditions satisfied (sequences 4,5 from cycle.supersede.* of e66).");

    // 5. drop append-only triggers temporarily
    conn.execute_batch("
        DROP TRIGGER IF EXISTS events_v1_no_update;
        DROP TRIGGER IF EXISTS events_v1_no_delete;
    ").expect("drop triggers");
    println!("Append-only triggers temporarily disabled.");

    // 6. read all events in (stream, sequence) order
    let all = read_all_events(&conn);
    let malformed_seqs: HashSet<i64> = malformed.iter().map(|m| m.0).collect();

    // 7. process events, computing new subjects/content_hash for malformed,
    //    then chain_hash per stream. KEYED BY event_id (PK), not sequence —
    //    sequence is unique per stream but not globally.
    let mut new_subjects: HashMap<String, String> = HashMap::new();
    let mut updates: HashMap<String, (String, String)> = HashMap::new(); // event_id -> (new_content_hash, new_chain_hash)
    let mut prev_chain_by_stream: HashMap<String, String> = HashMap::new();

    for row in &all {
        let is_malformed = malformed_seqs.contains(&row.sequence)
            && row.stream_id == "cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets";
        // Normalize evidence_refs (legacy rows use Vec<Map>, current uses Vec<String>).
        let evidence_refs_for_hash: Vec<String> = if row.evidence_refs_json.is_empty() {
            vec![]
        } else {
            match serde_json::from_str::<Vec<String>>(&row.evidence_refs_json) {
                Ok(v) => v,
                Err(_) => {
                    match serde_json::from_str::<Vec<serde_json::Value>>(&row.evidence_refs_json) {
                        Ok(arr) => arr.into_iter().map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else if let Some(obj) = v.as_object() {
                                obj.get("path").and_then(|x| x.as_str())
                                    .or_else(|| obj.get("hash").and_then(|x| x.as_str()))
                                    .or_else(|| obj.get("commit").and_then(|x| x.as_str()))
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| v.to_string())
                            } else {
                                v.to_string()
                            }
                        }).collect(),
                        Err(_) => vec![],
                    }
                }
            }
        };

        let subjects_value: Value = if is_malformed {
            let mut subs: Value = serde_json::from_str(&row.subjects_json).expect("parse subjects");
            if let Value::Array(arr) = &mut subs {
                for s in arr.iter_mut() {
                    if let Some(obj) = s.as_object_mut() {
                        if let Some(kind_val) = obj.remove("kind") {
                            obj.insert("type".to_string(), kind_val);
                        }
                        // Also drop the legacy `role` field.
                        obj.remove("role");
                    }
                }
            }
            let canon = serde_json::to_string(&subs).expect("re-serialize subjects");
            new_subjects.insert(row.event_id.clone(), canon.clone());
            subs
        } else if row.subjects_json.is_empty() {
            Value::Array(vec![])
        } else {
            serde_json::from_str(&row.subjects_json).expect("parse subjects")
        };

        let new_content_hash = if is_malformed {
            compute_content_hash_from_value(row, &subjects_value, &evidence_refs_for_hash)
        } else {
            row.old_content_hash.clone()
        };

        let prev_chain = prev_chain_by_stream.get(&row.stream_id).cloned().unwrap_or_default();
        let new_chain = chain_hash(&new_content_hash, &prev_chain);

        updates.insert(row.event_id.clone(), (new_content_hash.clone(), new_chain.clone()));
        prev_chain_by_stream.insert(row.stream_id.clone(), new_chain);
    }

    // 8. apply updates
    let tx = conn.unchecked_transaction().expect("begin tx");
    for row in &all {
        let (new_ch, new_chh) = updates.get(&row.event_id).expect("update prepared").clone();
        if let Some(new_subj) = new_subjects.get(&row.event_id) {
            tx.execute(
                "UPDATE events_v1 SET subjects_json=?, content_hash=?, chain_hash=? WHERE event_id=?",
                rusqlite::params![new_subj, new_ch, new_chh, row.event_id],
            ).unwrap_or_else(|e| {
                eprintln!("FAIL at sequence {} (event_id={}): {}", row.sequence, row.event_id, e);
                std::process::exit(1);
            });
        } else {
            tx.execute(
                "UPDATE events_v1 SET chain_hash=? WHERE event_id=?",
                rusqlite::params![new_chh, row.event_id],
            ).unwrap_or_else(|e| {
                eprintln!("FAIL at sequence {} (chain_hash only, event_id={}): {}", row.sequence, row.event_id, e);
                std::process::exit(1);
            });
        }
    }
    tx.commit().expect("commit");
    println!("Applied {} updates ({} content+chain, others chain-only).", all.len(), new_subjects.len());

    // 9. re-create triggers with the SAME names so any external reader
    //    relying on trigger existence (e.g. for verification) sees them again.
    conn.execute_batch("
        CREATE TRIGGER IF NOT EXISTS events_v1_no_update
        BEFORE UPDATE ON events_v1
        BEGIN
            SELECT RAISE(ABORT, 'events_v1 are append-only');
        END;
        CREATE TRIGGER IF NOT EXISTS events_v1_no_delete
        BEFORE DELETE ON events_v1
        BEGIN
            SELECT RAISE(ABORT, 'events_v1 are append-only');
        END;
    ").expect("re-create triggers");
    println!("Append-only triggers restored.");

    // 10. fingerprint output (read after explicit fsync-flush via checkpoint)
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
    let out_bytes = std::fs::read(output_db).expect("read output");
    let out_sha = Sha256::digest(&out_bytes);
    println!("Output SHA-256: sha256:{:x}", out_sha);

    // 11. report
    println!("\n=== Migration summary ===");
    println!("Source SHA-256: sha256:{:x}", src_sha);
    println!("Output SHA-256: sha256:{:x}", out_sha);
    println!("Events mutated (subjects_json kind -> type): {}", new_subjects.len());
    println!("Events with content_hash recomputed: {}", updates.len());
    println!("Affected stream: cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets");
    println!("Affected sequences: 4, 5 (and chain_hash for downstream events in same stream)");

    ExitCode::SUCCESS
}

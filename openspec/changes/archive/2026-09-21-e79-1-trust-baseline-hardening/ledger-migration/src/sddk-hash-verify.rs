use sddk_domain::event_envelope::{EventEnvelopeV1, EntityRef, ActorRef};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} <db_path>", args[0]);
        std::process::exit(2);
    }
    let db_path = &args[1];
    let conn = rusqlite::Connection::open(db_path).expect("open db");
    
    // query all events
    let mut stmt = conn.prepare(
        "SELECT event_id, event_type, schema_version, stream_id, sequence, project_id,
                occurred_at, recorded_at, actor_json, subjects_json, payload_json,
                evidence_refs_json, content_hash, metadata_json, causation_id,
                correlation_id, cycle_id, frame_id, fork_id
         FROM events_v1
         ORDER BY stream_id, sequence"
    ).expect("prepare");
    
    let mut rows = stmt.query([]).expect("query");
    let mut total = 0;
    let mut ok = 0;
    let mut first_mismatch: Option<(i64, String, String, String)> = None;
    while let Some(row) = rows.next().expect("row") {
        total += 1;
        let event_id: String = row.get(0).unwrap();
        let event_type: String = row.get(1).unwrap();
        let schema_version: i64 = row.get(2).unwrap();
        let stream_id: String = row.get(3).unwrap();
        let sequence: i64 = row.get(4).unwrap();
        let project_id: String = row.get(5).unwrap();
        let occurred_at: String = row.get(6).unwrap();
        let recorded_at: String = row.get(7).unwrap();
        let actor_json: String = row.get(8).unwrap_or_default();
        let subjects_json: String = row.get(9).unwrap_or_default();
        let payload_json: String = row.get(10).unwrap_or_default();
        let evidence_refs_json: String = row.get(11).unwrap_or_default();
        let stored_content_hash: String = row.get(12).unwrap();
        let metadata_json: Option<String> = row.get(13).unwrap();
        let causation_id: Option<String> = row.get(14).unwrap();
        let correlation_id: Option<String> = row.get(15).unwrap();
        let cycle_id: Option<String> = row.get(16).unwrap();
        let frame_id: Option<String> = row.get(17).unwrap();
        let fork_id: Option<String> = row.get(18).unwrap();
        
        // build envelope. We use Value for fields where the current schema
        // may not match legacy data; only subjects/actor have strict types.
        let actor: ActorRef = serde_json::from_str(&actor_json)
            .unwrap_or_else(|e| { eprintln!("SKIP seq={} actor parse error: {}", sequence, e); std::process::exit(2) });
        let subjects: Vec<EntityRef> = if subjects_json.is_empty() { vec![] } else {
            match serde_json::from_str(&subjects_json) {
                Ok(v) => v,
                Err(e) => { eprintln!("SKIP seq={} subjects parse: {}", sequence, e); continue; }
            }
        };
        let payload: Value = if payload_json.is_empty() { Value::Null } else {
            serde_json::from_str(&payload_json).unwrap_or(Value::Null)
        };
        // evidence_refs may be either Vec<String> (current) or Vec<Map> (legacy).
        // We accept both by deserializing as Value first, then re-serializing as
        // a normalized Vec<String> representation matching what the canonical
        // envelope would have computed at write time.
        let evidence_refs: Vec<String> = if evidence_refs_json.is_empty() {
            vec![]
        } else {
            match serde_json::from_str::<Vec<String>>(&evidence_refs_json) {
                Ok(v) => v,
                Err(_) => {
                    // Legacy shape: array of maps with `path`/`commit`/`hash` keys.
                    // Normalize to a single string per element using the most
                    // identifying key. This is a VERIFY-TIME approximation; the
                    // stored content_hash may legitimately differ for these rows.
                    match serde_json::from_str::<Vec<serde_json::Value>>(&evidence_refs_json) {
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
        let metadata: Option<Value> = metadata_json.as_ref().filter(|s| !s.is_empty())
            .map(|s| serde_json::from_str(s).unwrap_or(Value::Null));
        
        let envelope = EventEnvelopeV1 {
            event_id: event_id.clone(),
            event_type: event_type.clone(),
            schema_version: schema_version as u32,
            stream_id: stream_id.clone(),
            sequence: sequence as u64,
            project_id: project_id.clone(),
            occurred_at: occurred_at.clone(),
            recorded_at: recorded_at.clone(),
            actor,
            subjects,
            payload,
            evidence_refs,
            content_hash: String::new(),
            metadata,
            causation_id,
            correlation_id,
            cycle_id,
            frame_id,
            fork_id,
        };
        
        let computed = envelope.compute_content_hash();
        if computed == stored_content_hash {
            ok += 1;
        } else {
            if first_mismatch.is_none() {
                first_mismatch = Some((sequence, event_id, stored_content_hash, computed));
            }
        }
    }
    println!("total events: {}", total);
    println!("content_hash matches: {}", ok);
    if let Some((seq, eid, stored, computed)) = first_mismatch {
        println!("first mismatch: seq={} event={}", seq, eid);
        println!("  stored:   {}", stored);
        println!("  computed: {}", computed);
    }
    // also verify chain_hash
    println!("\nchain_hash verification:");
    let mut stmt2 = conn.prepare(
        "SELECT stream_id, sequence, content_hash, chain_hash FROM events_v1
         ORDER BY stream_id, sequence"
    ).expect("prepare");
    let mut rows2 = stmt2.query([]).expect("query");
    let mut last_chain_by_stream: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut chain_ok = 0;
    let mut chain_bad = 0;
    while let Some(row) = rows2.next().expect("row") {
        let stream_id: String = row.get(0).unwrap();
        let content_hash: String = row.get(2).unwrap();
        let stored_chain: String = row.get(3).unwrap();
        let prev = last_chain_by_stream.get(&stream_id).cloned().unwrap_or_default();
        let input: Vec<u8> = if prev.is_empty() {
            content_hash.as_bytes().iter().chain("genesis".as_bytes()).cloned().collect()
        } else {
            content_hash.as_bytes().iter().chain(prev.as_bytes()).cloned().collect()
        };
        let digest = Sha256::digest(&input);
        let computed = format!("sha256:{:x}", digest);
        if computed == stored_chain {
            chain_ok += 1;
        } else {
            chain_bad += 1;
            if chain_bad <= 3 {
                println!("chain mismatch: stream={} seq={} stored={} computed={}",
                    stream_id, row.get::<_, i64>(1).unwrap(), stored_chain, computed);
            }
        }
        last_chain_by_stream.insert(stream_id.clone(), computed);
    }
    println!("chain_hash matches: {} / {} mismatches", chain_ok, chain_bad);
}

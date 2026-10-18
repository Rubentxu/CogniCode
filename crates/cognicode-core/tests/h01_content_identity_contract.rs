//! H0.1 (delivery-first radical, AC-01) — content identity is
//! part of the cache fingerprint, not just mtime+size.
//!
//! The H0.1 spec says: "Reparar `PerFileGraphCache::merge_with_report`:
//! no usar `get_cached()` sin validar `entry.valid`, identidad de
//! bytes y ruta." The H0.5 acceptance law
//! `editar bytes con mtime/tamaño iguales ⇒ cambia el resultado`
//! is the contract this test pins.
//!
//! A file whose bytes were rewritten in place — `git checkout` of
//! an unchanged-size blob, an editor that pads, an `rsync` of an
//! identical-length replacement — can leave the filesystem
//! metadata (mtime+size) unchanged while the content has changed.
//! A mtime+size-only fingerprint would return the previous graph
//! and the cache would serve a stale answer. The corrected
//! fingerprint includes a 64-bit hash of the file body; the cache
//! is honest about content change regardless of metadata.
//!
//! The legitimate fast-path ("no re-parse on an unchanged file")
//! is already pinned by `b3_w4_incremental_analysis::content_hash_detects_edit_with_same_size_and_mtime`
//! and other AAT-B3-02 invariants. This file pins the inverse:
//! same mtime+size, different content ⇒ cache must invalidate.

use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;

/// V1 and V2 share the same byte length but differ in content.
/// Identical mtime+size fingerprint ⇒ previous code returned
/// the V1 graph for V2; corrected code returns a fresh V2 graph.
const FIXTURE_V1: &[u8] = b"pub fn helper() -> u32 { 1 }\n";
const FIXTURE_V2: &[u8] = b"pub fn helper() -> u32 { 2 }\n";

#[test]
fn cache_misses_when_content_changes_with_identical_metadata() {
    // Invariant: V1 and V2 are byte-equal in length. A
    // mtime+size-only fingerprint would say "no change" and
    // return the V1 graph; the corrected cache must return
    // the V2 graph.
    assert_eq!(
        FIXTURE_V1.len(),
        FIXTURE_V2.len(),
        "fixture invariant: V1 and V2 must have identical length"
    );

    let tmp = tempfile::tempdir().expect("create tempdir");
    let file_path = tmp.path().join("lib.rs");
    std::fs::write(&file_path, FIXTURE_V1).expect("write V1");

    // Snapshot V1's mtime. We will force V2 to this exact mtime
    // so the only thing that distinguishes V1 from V2 on disk
    // is the bytes themselves.
    let v1_mtime_secs = std::fs::metadata(&file_path)
        .expect("stat V1")
        .modified()
        .expect("V1 mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("V1 mtime is post-epoch")
        .as_secs();

    let project_dir = tmp.path().to_string_lossy().into_owned();
    let cache = PerFileGraphCache::with_project_dir(project_dir);
    let v1_graph = cache
        .get_or_build(&file_path)
        .expect("build V1 graph");

    // Rewrite the file in place with V2. Force V2's mtime to
    // V1's mtime so the mtime+size fingerprint is identical.
    std::fs::write(&file_path, FIXTURE_V2).expect("write V2");
    let v1_mtime_filetime =
        filetime::FileTime::from_unix_time(v1_mtime_secs as i64, 0);
    filetime::set_file_mtime(&file_path, v1_mtime_filetime)
        .expect("force V2 mtime to V1's mtime");

    let v2_graph = cache
        .get_or_build(&file_path)
        .expect("build V2 graph");

    // Belt-and-braces: confirm the fingerprint is what we
    // intended. mtime+size identical; only the bytes differ.
    let v2_meta = std::fs::metadata(&file_path).expect("stat V2");
    assert_eq!(
        v1_mtime_secs,
        v2_meta
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "test setup: V1 and V2 mtimes must be identical"
    );
    assert_eq!(
        v2_meta.len(),
        FIXTURE_V2.len() as u64,
        "test setup: V2 size must match V2 byte length"
    );

    // The corrected cache rebuilds on content change even when
    // mtime+size match. The graph returned for V2 must NOT be
    // the same `Arc` as V1's. A pointer-equal result means the
    // cache served a stale graph — the failure mode the
    // content-identity check exists to prevent.
    assert!(
        !std::sync::Arc::ptr_eq(&v1_graph, &v2_graph),
        "cache must invalidate on in-place content change with \
         identical mtime+size; the cache returned the same Arc \
         for V1 and V2, which is the failure mode the \
         content-identity check exists to prevent"
    );
}

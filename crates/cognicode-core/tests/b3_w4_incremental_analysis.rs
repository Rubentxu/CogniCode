//! B3.W4 — Análisis incremental fiable: invalidación content-based.
//!
//! Estado pre-B3.W4:
//!   - El codebase tiene cache `(mtime, size, content_hash)` con SHA-256
//!     en `AnalysisService` (post H-01 verde) pero el primitivo
//!     `content_hash` NO está expuesto como API pública reusable.
//!   - No existe `ContentHash` ni `CacheStore` a nivel de aplicación
//!     para que `WorkspaceSession` consuma.
//!
//! Lo que cierra este test (AAT-B3-02 / AAT-B3-03 / AAT-B3-04 parcial):
//!   - `ContentHash` se computa con BLAKE3 sobre los BYTES del archivo
//!     y detecta una edición aunque mtime y size no cambien.
//!   - `CacheStore` (BTreeMap<PathBuf, ContentHash>) marca un archivo
//!     como stale al re-leer del disco y comparar el hash cacheado.
//!   - Dos instancias en el mismo proceso NO comparten estado.
//!   - Unicode, espacios y contenido vacío se manejan sin panics.
//!
//! Política B3.W4:
//!   - El módulo es público: `cognicode_core::incremental::content_hash`.
//!   - BLAKE3, no SHA-256. blake3 ya es workspace dep.
//!   - `Display` produce hex en minúsculas.
//!   - No se introduce estado global: cada `CacheStore` es propio.

use cognicode_core::incremental::content_hash::{content_hash, CacheStore, ContentHash};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn epoch_secs(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn write_with_mtime(path: &Path, body: &[u8], mtime: SystemTime) {
    fs::write(path, body).expect("write fixture");
    let f = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for mtime");
    f.set_modified(mtime).expect("set_modified");
}

fn git_in(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn git {args:?}: {e}"));
    assert!(
        status.success(),
        "git {args:?} failed in {} with status {status:?}",
        cwd.display()
    );
}

// ---------------------------------------------------------------------------
// AAT-B3-02 — bytes different with SAME size AND SAME mtime.
// Killer case for mtime-only invalidation. Two 39-byte blobs, mtime
// preserved, must hash differently.
// ---------------------------------------------------------------------------
#[test]
fn content_hash_detects_edit_with_same_size_and_mtime() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("subject.txt");
    let fixed_mtime = epoch_secs(1_700_000_000);

    let original = b"the quick brown fox jumps over a dog";
    let edited = b"the quick brown cat jumps over a dog";
    assert_eq!(original.len(), edited.len(), "fixture invariant: same size");

    write_with_mtime(&path, original, fixed_mtime);
    let h_before = content_hash(&path).expect("hash original");

    write_with_mtime(&path, edited, fixed_mtime);
    let h_after = content_hash(&path).expect("hash edited");

    let meta = fs::metadata(&path).expect("stat");
    assert_eq!(meta.len(), original.len() as u64, "size preserved");
    assert_eq!(
        meta.modified().expect("mtime"),
        fixed_mtime,
        "mtime preserved"
    );

    assert_ne!(
        h_before, h_after,
        "different bytes with same size+mtime must hash differently (AAT-B3-02)"
    );
}

// ---------------------------------------------------------------------------
// CacheStore invalidates when content changes with mtime preserved.
// ---------------------------------------------------------------------------
#[test]
fn cache_invalidates_on_content_change_with_unchanged_mtime() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("src").join("lib.rs");
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    let fixed_mtime = epoch_secs(1_700_000_100);

    write_with_mtime(&path, b"pub fn alpha() -> i32 { 1 }\n", fixed_mtime);
    let mut store = CacheStore::new();
    store.observe(&path).expect("observe original");
    assert!(
        !store.is_stale(&path).expect("first observe is fresh"),
        "freshly observed file must not be stale"
    );

    write_with_mtime(&path, b"pub fn alpha() -> i32 { 7 }\n", fixed_mtime);
    assert!(
        store.is_stale(&path).expect("stale after content change"),
        "content change with preserved size+mtime MUST invalidate cache"
    );
}

// ---------------------------------------------------------------------------
// Identical content must NOT cause a spurious invalidation.
// ---------------------------------------------------------------------------
#[test]
fn cache_does_not_invalidate_on_identical_content() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("data.bin");
    let fixed_mtime = epoch_secs(1_700_000_200);

    write_with_mtime(&path, b"\x00\x01\x02stable content", fixed_mtime);
    let mut store = CacheStore::new();
    store.observe(&path).expect("observe");

    write_with_mtime(
        &path,
        b"\x00\x01\x02stable content",
        epoch_secs(1_700_000_300),
    );
    assert!(
        !store.is_stale(&path).expect("stale check"),
        "identical bytes must NOT be reported as stale even when mtime moves"
    );
}

// ---------------------------------------------------------------------------
// Branch switch — switching branches rewrites the file content under the
// same path, so the cache (still pinned to the previous branch's bytes)
// MUST report the file as stale.
// ---------------------------------------------------------------------------
#[test]
fn branch_switch_invalidates_cache() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path();

    git_in(&repo, &["init", "-q", "--initial-branch=main"]);
    git_in(&repo, &["checkout", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "b3@test.invalid"]);
    git_in(&repo, &["config", "user.name", "B3 Tester"]);

    let subject = repo.join("subject.txt");
    fs::write(&subject, "branch A content\n").expect("write A");
    git_in(&repo, &["add", "subject.txt"]);
    git_in(&repo, &["commit", "-q", "-m", "A"]);

    let mut store = CacheStore::new();
    store.observe(&subject).expect("observe A");
    assert!(!store.is_stale(&subject).expect("stale A"));

    git_in(&repo, &["checkout", "-q", "-b", "feature"]);
    fs::write(&subject, "branch B content that is different\n").expect("write B");

    assert!(
        store.is_stale(&subject).expect("stale after branch switch"),
        "branch switch to a different-content branch must invalidate cache"
    );

    store.observe(&subject).expect("observe B");
    assert!(!store.is_stale(&subject).expect("stale B"));
}

// ---------------------------------------------------------------------------
// Cold query — a brand-new store observes once and sees the entry
// populated; before observe() the cache is empty for that path.
// ---------------------------------------------------------------------------
#[test]
fn cold_query_does_not_use_warm_cache() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("cold.txt");
    fs::write(&path, b"cold content").expect("write");

    let mut store = CacheStore::new();
    assert!(
        store.get(&path).is_none(),
        "cold store must not have a cached entry"
    );
    store.observe(&path).expect("observe cold");
    assert!(
        store.get(&path).is_some(),
        "after observe() the entry exists"
    );
}

// ---------------------------------------------------------------------------
// Hot query — second observe() with identical bytes is a hot pass.
// ---------------------------------------------------------------------------
#[test]
fn hot_query_returns_cached_result() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("hot.txt");
    fs::write(&path, b"hot content").expect("write");

    let mut store = CacheStore::new();
    store.observe(&path).expect("first observe");
    let first_hash = store.get(&path).expect("cached hash");

    store.observe(&path).expect("second observe");
    let second_hash = store.get(&path).expect("cached hash 2");
    assert_eq!(first_hash, second_hash, "hot pass keeps the same hash");
    assert!(
        !store.is_stale(&path).expect("stale hot"),
        "second observe of identical bytes must be a cache hit"
    );
}

// ---------------------------------------------------------------------------
// AAT-B3-03 — two workspaces in the same process do NOT share cache state.
// ---------------------------------------------------------------------------
#[test]
fn two_workspaces_in_same_process_do_not_share_cache() {
    let ws_a = tempfile::tempdir().expect("a");
    let ws_b = tempfile::tempdir().expect("b");
    let file_a = ws_a.path().join("src").join("lib_a.rs");
    let file_b = ws_b.path().join("src").join("lib_b.rs");
    fs::create_dir_all(file_a.parent().unwrap()).expect("mkdir a");
    fs::create_dir_all(file_b.parent().unwrap()).expect("mkdir b");
    fs::write(&file_a, b"pub fn alpha() {}").expect("write a");
    fs::write(&file_b, b"pub fn alpha() {}").expect("write b");

    let mut store_x = CacheStore::new();
    let mut store_y = CacheStore::new();

    store_x.observe(&file_a).expect("x obs a");
    store_y.observe(&file_b).expect("y obs b");

    assert!(store_x.get(&file_a).is_some());
    assert!(store_y.get(&file_b).is_some());
    assert!(
        store_x.get(&file_b).is_none(),
        "store_x must NOT see file_b (different workspace)"
    );
    assert!(
        store_y.get(&file_a).is_none(),
        "store_y must NOT see file_a (different workspace)"
    );

    assert!(
        !store_x.is_stale(&file_b).expect("stale cold"),
        "first-time reference to a different workspace must be cold"
    );
}

// ---------------------------------------------------------------------------
// AAT-B3-04 (partial) — Unicode content + paths with spaces.
// ---------------------------------------------------------------------------
#[test]
fn unicode_path_with_spaces_does_not_break_content_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let unicode_dir = tmp.path().join("espacio ñ漢字 🚀");
    fs::create_dir_all(&unicode_dir).expect("mkdir unicode");
    let path = unicode_dir.join("código fuente.txt");
    let body = "línea 1 con ñ\nlínea 2 你好 🚀\n";
    fs::write(&path, body).expect("write unicode");

    let h1 = content_hash(&path).expect("unicode hash");
    let h2 = content_hash(&path).expect("unicode hash repeat");
    assert_eq!(h1, h2, "blake3 must be deterministic across calls");
    assert_eq!(h1.to_string().len(), 64, "blake3 hex is 64 lowercase chars");
    assert!(
        h1.to_string().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "Display must be lowercase hex: {:?}",
        h1.to_string()
    );
}

// ---------------------------------------------------------------------------
// Empty file — blake3 has a well-defined empty digest (af1349b9...).
// ---------------------------------------------------------------------------
#[test]
fn empty_file_produces_blake3_empty_digest() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("empty.txt");
    fs::write(&path, b"").expect("write empty");

    let h = content_hash(&path).expect("hash empty");
    let hex = h.to_string();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.starts_with("af1349b9"),
        "empty file must hash to the standard blake3 empty digest, got {hex}"
    );
}

// ---------------------------------------------------------------------------
// Non-existent file — content_hash must surface `NotFound` rather than
// silently returning a zero digest.
// ---------------------------------------------------------------------------
#[test]
fn non_existent_file_returns_io_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("never_created.rs");
    let err = content_hash(&missing).expect_err("missing file must error");
    assert_eq!(err.kind(), ErrorKind::NotFound, "got {err:?}");
}

// ---------------------------------------------------------------------------
// `ContentHash` is `Copy + Eq + Hash` so it can live in a `HashMap`.
// ---------------------------------------------------------------------------
#[test]
fn content_hash_supports_hash_map_key_roundtrip() {
    use std::collections::HashMap;
    let mut map: HashMap<ContentHash, &'static str> = HashMap::new();
    let h1 = ContentHash::from_bytes([1u8; 32]);
    let h2 = ContentHash::from_bytes([2u8; 32]);
    map.insert(h1, "alpha");
    map.insert(h2, "beta");
    assert_eq!(map.get(&h1).copied(), Some("alpha"));
    assert_eq!(map.get(&h2).copied(), Some("beta"));
    assert_ne!(h1, h2);
}

// Silence the unused PathBuf warnings if helpers go unused.
#[allow(dead_code)]
fn _path_buf_marker() -> PathBuf {
    PathBuf::new()
}

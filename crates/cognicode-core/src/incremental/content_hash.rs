//! BLAKE3-based content hashing for cache invalidation (B3.W4).
//!
//! BLAKE3 se computa sobre los bytes crudos del archivo (no sobre
//! `mtime` ni `size`). Esto cierra AAT-B3-02: una edición con bytes
//! distintos pero `mtime` y `size` preservados produce un hash
//! diferente y por tanto invalida la entrada cacheada.
//!
//! BLAKE3 is computed over the raw file bytes (not mtime or size). This
//! closes AAT-B3-02 — an edit with identical mtime/size produces a
//! different digest, so the cached entry is invalidated.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Hash de contenido de 256 bits (BLAKE3 sobre los bytes del archivo).
///
/// Es `Copy + Eq + Hash` para poder vivir en `BTreeMap`/`HashMap` y
/// además serializarse como hex ASCII estable. La serialización es la
/// forma contractualmente intercambiable entre procesos (CLI ↔ MCP).
///
/// 256-bit content hash (BLAKE3 over the file bytes).
///
/// `Copy + Eq + Hash` so it can live in `BTreeMap`/`HashMap`. Serialized
/// as stable lowercase hex so the CLI/MCP boundary can exchange it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// Build a `ContentHash` from a raw 32-byte digest.
    ///
    /// Construye un `ContentHash` a partir de un digest crudo de 32B.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw 32-byte digest (little-endian bytes of BLAKE3).
    ///
    /// Devuelve el digest crudo de 32B (bytes little-endian de BLAKE3).
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Display produce hex en minúsculas — invariante de todos los
/// consumidores (CLI output, MCP `ContentHash` field, logs).
/// Display produces lowercase hex — invariant for all consumers.
impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self)
    }
}

/// Compute the BLAKE3 hash of the bytes at `path`.
///
/// I/O errors propagate as `io::Error` — el caller decide si el
/// archivo faltante es bloqueante o solo un skip silencioso del
/// invalidator. La cache no devuelve nunca un digest cero inventado.
///
/// BLAKE3 of the bytes at `path`. I/O errors propagate; the caller
/// decides whether a missing file blocks or is silently skipped. The
/// cache NEVER returns a fabricated zero digest.
pub fn content_hash(path: &Path) -> io::Result<ContentHash> {
    let bytes = fs::read(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let raw: [u8; 32] = digest.into();
    Ok(ContentHash(raw))
}

/// Per-workspace storage of `PathBuf -> ContentHash`.
///
/// Cada `WorkspaceSession` carga su propio `CacheStore` (no hay estado
/// global). El `BTreeMap` interno da un orden estable de keys para
/// serialización determinista y para depuración.
///
/// Each `WorkspaceSession` owns its own `CacheStore` (no global state).
/// The internal `BTreeMap` gives stable key order for deterministic
/// serialization and debugging.
#[derive(Default, Debug, Clone)]
pub struct CacheStore {
    entries: BTreeMap<PathBuf, ContentHash>,
}

impl CacheStore {
    /// Build an empty store.
    ///
    /// Construye un almacén vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read `path` from disk, hash it, store the result, return it.
    ///
    /// Lee `path`, calcula el hash, lo guarda y lo devuelve.
    pub fn observe(&mut self, path: &Path) -> io::Result<ContentHash> {
        let h = content_hash(path)?;
        self.entries.insert(path.to_path_buf(), h);
        Ok(h)
    }

    /// Look up the cached hash for `path` (if any). No disk I/O.
    ///
    /// Busca el hash cacheado para `path`. No toca disco.
    pub fn get(&self, path: &Path) -> Option<ContentHash> {
        self.entries.get(path).copied()
    }

    /// Returns `true` if the file at `path` has different bytes than the
    /// last observed snapshot. Returns `false` for never-observed paths
    /// (cold; treat as not stale — the consumer can branch on `get`).
    ///
    /// Devuelve `true` si los bytes del archivo en `path` difieren del
    /// último snapshot observado. Para paths nunca observados devuelve
    /// `false` (cold: no hay baseline previa con la que comparar).
    pub fn is_stale(&self, path: &Path) -> io::Result<bool> {
        match self.entries.get(path) {
            None => Ok(false),
            Some(cached) => {
                let current = content_hash(path)?;
                Ok(cached != &current)
            }
        }
    }

    /// Number of tracked paths.
    ///
    /// Número de paths rastreados.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if no paths are tracked.
    ///
    /// `true` si no hay paths rastreados.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate tracked `(path, hash)` pairs in BTreeMap order.
    ///
    /// Itera sobre pares `(path, hash)` en orden estable del BTreeMap.
    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &ContentHash)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the `content_hash` and `CacheStore` primitives.
    //!
    //! Tests unitarios de los primitivos `content_hash` y `CacheStore`.
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn epoch_secs(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn write_with_mtime(path: &Path, body: &[u8], mtime: SystemTime) {
        std::fs::write(path, body).expect("write fixture");
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open for mtime");
        f.set_modified(mtime).expect("set_modified");
    }

    #[test]
    fn same_bytes_produce_same_hash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        write_with_mtime(&a, b"identical body", epoch_secs(1_700_000_010));
        write_with_mtime(&b, b"identical body", epoch_secs(1_700_000_020));
        // mtimes differ, content identical.
        assert_ne!(
            std::fs::metadata(&a).unwrap().modified().unwrap(),
            std::fs::metadata(&b).unwrap().modified().unwrap(),
        );
        assert_eq!(
            content_hash(&a).unwrap(),
            content_hash(&b).unwrap(),
            "same bytes must produce the same BLAKE3 digest regardless of mtime"
        );
    }

    #[test]
    fn different_bytes_produce_different_hashes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        write_with_mtime(&a, b"alpha", epoch_secs(1_700_000_030));
        write_with_mtime(&b, b"beta", epoch_secs(1_700_000_040));
        assert_ne!(
            content_hash(&a).unwrap(),
            content_hash(&b).unwrap(),
            "distinct bytes must yield distinct digests"
        );
    }

    #[test]
    fn nonexistent_file_returns_io_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = content_hash(&tmp.path().join("never.rs"))
            .expect_err("missing file must error");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn empty_file_produces_blake3_empty_digest() {
        // blake3("") = af1349b9f5f9a1a2a3a4a5a6a7a8a9aa…
        let tmp = tempfile::tempdir().expect("tempdir");
        let p = tmp.path().join("zero.bin");
        std::fs::write(&p, b"").expect("write empty");
        let h = content_hash(&p).expect("hash empty");
        let hex = h.to_string();
        assert_eq!(hex.len(), 64, "BLAKE3 hex is 64 lowercase chars");
        assert!(
            hex.starts_with("af1349b9"),
            "empty file must hash to the standard blake3 empty digest, \
             got {hex}"
        );
    }
}

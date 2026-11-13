//! What the service remembers of each file it has read.
//!
//! A graph build reads every file of the tree, and most of them have not
//! changed since the last build. The cache keeps what a file declared,
//! under the identity of the bytes it was read from, so that a build
//! interprets again only the files whose bytes are not the ones it
//! remembers.
//!
//! ## The identity
//!
//! An entry answers for a file only when the size, the clock and the
//! SHA-256 of the content all agree. The clock alone is not enough: a
//! file rewritten under an unchanged clock has other bytes, and the hash is
//! what notices. The clock and the size are checked first because they are
//! free, never instead of the hash.
//!
//! ## The facts are shared, not copied
//!
//! An entry holds its facts behind an `Arc`. A hit hands out a pointer, so
//! the lock is held for a lookup and not for a deep copy of every symbol and
//! call of the file, and the tree-wide pass that follows reads the same
//! allocation the cache owns.
//!
//! ## What "unfolded" means
//!
//! A file can be read before the build that will use it: the incremental
//! reindex reads the file that changed, and hands the facts to the cache
//! instead of letting the build read the same bytes a second time. Such an
//! entry is *unfolded* until a build has taken it into a graph. The build
//! that takes it counts the file as interpreted, because it was: the
//! coverage of a build says how many files were read since the graph it
//! replaces, and a file read a moment before is one of them.

use crate::application::ingest::types::FileFacts;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// One remembered file.
struct CachedFile {
    /// The modification time in whole seconds, when the filesystem
    /// reported one. An absent time is not the epoch, and it does not
    /// invalidate the entry by itself: the hash decides.
    mtime: Option<i64>,
    size: u64,
    /// SHA-256 of the bytes the facts were read from.
    content_hash: [u8; 32],
    facts: Arc<FileFacts>,
    /// Read, but not yet taken into a graph by any build.
    unfolded: bool,
}

/// What the cache answers for a file that is the one it remembers.
pub(crate) struct Remembered {
    pub facts: Arc<FileFacts>,
    /// The file was read since the last build that took it into a graph,
    /// so the build that finds it counts it as interpreted.
    pub unfolded: bool,
}

/// The facts of the files that have been read, by path.
///
/// Cloning shares the cache: the async routes move a clone into the
/// blocking task that builds.
#[derive(Clone, Default)]
pub(crate) struct FileFactsCache {
    entries: Arc<Mutex<HashMap<String, CachedFile>>>,
}

impl FileFactsCache {
    /// What the cache remembers of `path`, if the file is the one it read.
    pub(crate) fn lookup(
        &self,
        path: &str,
        mtime: Option<i64>,
        size: u64,
        content_hash: [u8; 32],
    ) -> Option<Remembered> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(path)?;
        (entry.mtime == mtime && entry.size == size && entry.content_hash == content_hash).then(
            || Remembered {
                facts: entry.facts.clone(),
                unfolded: entry.unfolded,
            },
        )
    }

    /// Remembers what a build read, once the build has taken it in.
    pub(crate) fn remember(
        &self,
        path: String,
        mtime: Option<i64>,
        size: u64,
        content_hash: [u8; 32],
        facts: Arc<FileFacts>,
    ) {
        self.insert(path, mtime, size, content_hash, facts, false);
    }

    /// Remembers what was read ahead of the build that will use it.
    pub(crate) fn seed(
        &self,
        path: String,
        mtime: Option<i64>,
        size: u64,
        content_hash: [u8; 32],
        facts: FileFacts,
    ) -> Arc<FileFacts> {
        let facts = Arc::new(facts);
        self.insert(path, mtime, size, content_hash, facts.clone(), true);
        facts
    }

    fn insert(
        &self,
        path: String,
        mtime: Option<i64>,
        size: u64,
        content_hash: [u8; 32],
        facts: Arc<FileFacts>,
        unfolded: bool,
    ) {
        self.entries.lock().unwrap().insert(
            path,
            CachedFile {
                mtime,
                size,
                content_hash,
                facts,
                unfolded,
            },
        );
    }

    /// Forgets the files at `paths`: the next build reads them again.
    pub(crate) fn invalidate(&self, paths: &[PathBuf]) {
        let mut entries = self.entries.lock().unwrap();
        for path in paths {
            entries.remove(&path.to_string_lossy().into_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: [u8; 32] = [7; 32];

    fn cache_with_one_file() -> FileFactsCache {
        let cache = FileFactsCache::default();
        cache.remember(
            "a.rs".to_string(),
            Some(10),
            5,
            HASH,
            Arc::new(FileFacts::default()),
        );
        cache
    }

    #[test]
    fn an_entry_answers_only_for_the_identity_it_was_read_under() {
        let cache = cache_with_one_file();
        assert!(cache.lookup("a.rs", Some(10), 5, HASH).is_some());
        assert!(
            cache.lookup("a.rs", Some(11), 5, HASH).is_none(),
            "another clock is another file"
        );
        assert!(
            cache.lookup("a.rs", Some(10), 6, HASH).is_none(),
            "another size is another file"
        );
        assert!(
            cache.lookup("a.rs", Some(10), 5, [8; 32]).is_none(),
            "other bytes under the same clock and size are another file"
        );
        assert!(cache.lookup("b.rs", Some(10), 5, HASH).is_none());
    }

    #[test]
    fn a_hit_shares_the_facts_instead_of_copying_them() {
        let cache = FileFactsCache::default();
        let seeded = cache.seed("a.rs".to_string(), Some(10), 5, HASH, FileFacts::default());
        let hit = cache.lookup("a.rs", Some(10), 5, HASH).expect("a hit");
        assert!(Arc::ptr_eq(&seeded, &hit.facts));
    }

    #[test]
    fn a_seeded_file_is_unfolded_until_a_build_remembers_it() {
        let cache = FileFactsCache::default();
        let seeded = cache.seed("a.rs".to_string(), Some(10), 5, HASH, FileFacts::default());
        assert!(cache.lookup("a.rs", Some(10), 5, HASH).unwrap().unfolded);

        cache.remember("a.rs".to_string(), Some(10), 5, HASH, seeded);
        assert!(
            !cache.lookup("a.rs", Some(10), 5, HASH).unwrap().unfolded,
            "a build took the file in: the next build finds it already counted"
        );
    }

    #[test]
    fn an_invalidated_file_is_read_again() {
        let cache = cache_with_one_file();
        cache.invalidate(&[PathBuf::from("a.rs")]);
        assert!(cache.lookup("a.rs", Some(10), 5, HASH).is_none());
    }
}

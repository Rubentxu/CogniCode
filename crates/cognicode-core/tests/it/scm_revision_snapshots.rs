//! Immutable revision snapshots: the derived work of a commit outlives
//! HEAD, and reuse across branches is decided by content, not by luck.
//!
//! AC #1 — reopening two divergent commits reproduces their facts and
//! keeps both snapshots valid.
//! AC #2 — identical extraction objects are reused across branches,
//! but a resolution earned under one classpath is never reused under
//! another.
//! AC #3 — atomic writes and interrupted writes leave the store
//! consistent, and quota/GC never deletes a referenced snapshot.
//! AC #4 — producer version, coverage, source content and revision
//! lineage are separable claims, and a cache written by another
//! producer is invalidated explicitly, not trusted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cognicode_core::application::revision_snapshots::{
    ExtractionObject, LegacyCache, ProducerVersions, ResolutionMemo, RevisionSnapshot,
    RevisionSnapshotStore, inspect_legacy, invalidate_legacy, resolution_reusable,
    reuse_extraction,
};
use cognicode_core::domain::scm::{ContentDigest, PathKey, RevisionHandle};

fn handle(value: &str) -> RevisionHandle {
    RevisionHandle::new(value).expect("opaque lineage")
}

fn key(path: &str) -> PathKey {
    PathKey::new(path).expect("relative path")
}

fn digest(text: &str) -> ContentDigest {
    ContentDigest::from_bytes(text.as_bytes())
}

fn producer(extractor: &str) -> ProducerVersions {
    ProducerVersions::new(
        extractor,
        "graph-v1",
        "facts-v1",
        "detector-v1",
        "policy-v1",
    )
}

fn file_object(text: &str, artifact: &str) -> ExtractionObject {
    ExtractionObject {
        content: digest(text),
        artifact: artifact.to_string(),
    }
}

fn snapshot(
    lineage: &str,
    files: &[(&str, &str, &str)],
    resolution: Option<&str>,
) -> RevisionSnapshot {
    let mut map = BTreeMap::new();
    for (path, content, artifact) in files {
        map.insert(key(path), file_object(content, artifact));
    }
    RevisionSnapshot {
        lineage: handle(lineage),
        source_content: digest(&files.iter().map(|(_, c, _)| *c).collect::<String>()),
        coverage: "full".to_string(),
        producer: producer("extractor-v1"),
        files: map,
        resolution: resolution.map(|context| ResolutionMemo {
            context_fingerprint: context.to_string(),
            entries: vec![("mod:a".to_string(), "file.rs:1".to_string())],
        }),
    }
}

fn workspace(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("the workspace exists");
    dir
}

/// AC #1: two divergent commits, one store, both answers.
#[test]
fn two_divergent_commits_stay_valid_at_the_same_time() {
    let root = tempfile::tempdir().expect("a temporary root");
    let cache = root.path().join("cache");
    let work = workspace(root.path(), "repo");
    let store = RevisionSnapshotStore::for_workspace(&cache, &work);

    let left = snapshot("commit-left", &[("a.rs", "shared", "art-shared")], None);
    let right = snapshot(
        "commit-right",
        &[
            ("a.rs", "shared", "art-shared"),
            ("b.rs", "only-here", "art-b"),
        ],
        None,
    );
    store.save(&left).expect("left is published");
    store.save(&right).expect("right is published");

    // Reopening either commit reproduces its facts — the other one
    // having been opened in between changes nothing.
    let reopened_left = store
        .load(&handle("commit-left"), &producer("extractor-v1"))
        .expect("the left snapshot is still valid");
    let reopened_right = store
        .load(&handle("commit-right"), &producer("extractor-v1"))
        .expect("the right snapshot is still valid");

    assert_eq!(
        left, reopened_left,
        "the reopened facts of the left commit are its own"
    );
    assert_eq!(
        right, reopened_right,
        "the reopened facts of the right commit are its own"
    );
    assert_eq!(
        reopened_right
            .files
            .get(&key("b.rs"))
            .map(|o| o.artifact.as_str()),
        Some("art-b"),
        "a divergent commit keeps what only it has"
    );

    let lineages = store.lineages().expect("the store can be listed");
    assert_eq!(
        lineages.len(),
        2,
        "both snapshots live in the store at once"
    );
}

/// AC #2: extraction is reused by content across branches; resolution
/// is reused only under the classpath that earned it.
#[test]
fn extraction_crosses_branches_but_resolution_does_not() {
    let root = tempfile::tempdir().expect("a temporary root");
    let cache = root.path().join("cache");
    let work = workspace(root.path(), "repo");
    let store = RevisionSnapshotStore::for_workspace(&cache, &work);

    let base = snapshot(
        "commit-base",
        &[("a.rs", "shared", "art-shared")],
        Some("classpath-X"),
    );
    let head = snapshot(
        "commit-head",
        &[
            ("a.rs", "shared", "art-shared"),
            ("c.rs", "new-here", "art-c"),
        ],
        Some("classpath-Y"),
    );
    store.save(&base).expect("base is published");
    store.save(&head).expect("head is published");

    let reused = reuse_extraction(&base, &head, &key("a.rs"));
    assert_eq!(
        reused.map(|o| o.artifact.as_str()),
        Some("art-shared"),
        "identical extraction objects are reused between branches"
    );
    assert_eq!(
        reuse_extraction(&base, &head, &key("c.rs")).map(|o| o.artifact.as_str()),
        None,
        "content the base never saw is never reused from it"
    );

    assert!(
        !resolution_reusable(&base, &head),
        "a resolution earned against classpath-X is not a resolution against classpath-Y"
    );

    let same_context = snapshot(
        "commit-sibling",
        &[("a.rs", "shared", "art-shared")],
        Some("classpath-X"),
    );
    assert!(
        resolution_reusable(&base, &same_context),
        "the same classpath does make the memo reusable"
    );
}

/// AC #3: an interrupted write leaves nothing readable, and GC never
/// removes a referenced snapshot; quota only sweeps leftovers.
#[test]
fn interrupted_writes_and_gc_keep_the_referenced_truth() {
    let root = tempfile::tempdir().expect("a temporary root");
    let cache = root.path().join("cache");
    let work = workspace(root.path(), "repo");
    let store = RevisionSnapshotStore::for_workspace(&cache, &work);

    let kept = snapshot("commit-kept", &[("a.rs", "shared", "art-shared")], None);
    let doomed = snapshot("commit-doomed", &[("z.rs", "gone", "art-z")], None);
    store.save(&kept).expect("kept is published");
    store.save(&doomed).expect("doomed is published");

    // Simulate a crash mid-write: a temp file whose contents are the
    // doomed snapshot bytes under a `.tmp.` name.
    let tmp = store.root().join("deadbeef.snap.tmp.1.0");
    std::fs::write(&tmp, b"half written").expect("the interrupted write exists");

    let mut referenced = BTreeSet::new();
    referenced.insert("commit-kept".to_string());
    let report = store.gc(&referenced, 0).expect("the sweep runs");

    assert_eq!(
        report.removed,
        vec!["commit-doomed".to_string()],
        "the sweep removes exactly the one unreferenced snapshot"
    );
    assert!(!tmp.exists(), "a temp file is swept at once");
    assert!(
        store
            .load(&handle("commit-kept"), &producer("extractor-v1"))
            .is_some(),
        "the referenced snapshot survives the sweep"
    );
    assert!(
        store
            .load(&handle("commit-doomed"), &producer("extractor-v1"))
            .is_none(),
        "the unreferenced snapshot is gone"
    );

    // A truncation in place is not evidence either: it reads absent.
    let doomed_path_after = store
        .lineages()
        .expect("listing after the sweep")
        .contains(&"commit-doomed".to_string());
    assert!(!doomed_path_after, "the store lists what it actually holds");
}

/// AC #4: the four claims are separable on the snapshot, and an entry
/// written by another producer is not served to this one.
#[test]
fn producer_coverage_content_and_lineage_are_separate_claims() {
    let root = tempfile::tempdir().expect("a temporary root");
    let cache = root.path().join("cache");
    let work = workspace(root.path(), "repo");
    let store = RevisionSnapshotStore::for_workspace(&cache, &work);

    let one = snapshot("commit-one", &[("a.rs", "shared", "art-shared")], None);
    store.save(&one).expect("published by extractor-v1");

    assert_eq!(one.coverage, "full", "coverage is its own claim");
    assert_eq!(
        one.lineage.as_str(),
        "commit-one",
        "lineage is its own claim"
    );
    assert_eq!(
        one.source_content,
        digest("shared"),
        "content is its own claim"
    );

    // Same bytes, next release of the extractor: this producer is not
    // the one that wrote the entry, and the entry is not served.
    let reopened = store.load(&handle("commit-one"), &producer("extractor-v2"));
    assert!(
        reopened.is_none(),
        "a snapshot from another producer version is invalidated explicitly"
    );
    // And the original producer still finds it: invalidation is per
    // claim, not a wipe.
    assert!(
        store
            .load(&handle("commit-one"), &producer("extractor-v1"))
            .is_some()
    );
}

/// AC #4 (old cache half): a legacy latest-tree snapshot from another
/// producer is reported stale and can be invalidated on purpose.
#[test]
fn the_legacy_cache_is_inspected_and_invalidated_explicitly() {
    let root = tempfile::tempdir().expect("a temporary root");
    let cache = root.path().join("cache");
    let work = workspace(root.path(), "repo");

    // Nothing written yet: absent, and invalidating it says so.
    assert_eq!(inspect_legacy(&cache, &work), LegacyCache::Absent);
    assert!(!invalidate_legacy(&cache, &work).expect("nothing to remove"));

    // A legacy snapshot produced by the current analyzer stays.
    let path = cognicode_core::application::graph_cache::snapshot_path(&cache, &work);
    let graph = cognicode_core::domain::aggregates::call_graph::CallGraph::new();
    let manifest =
        cognicode_core::domain::value_objects::file_manifest::FileManifest::new(work.clone());
    let current =
        cognicode_core::domain::value_objects::snapshot_key::SnapshotKey::for_current_analyzer(
            cognicode_core::application::graph_snapshot::canonical_workspace(&work),
        );
    cognicode_core::application::graph_snapshot::save_durable_snapshot(
        &path, &current, &graph, &manifest,
    )
    .expect("the current legacy snapshot is written");
    assert_eq!(inspect_legacy(&cache, &work), LegacyCache::Current);

    // A foreign producer version makes it stale, and invalidation
    // removes it whole — never half.
    let mut foreign = current.clone();
    foreign.analyzer.version = "0.0.0-foreign".to_string();
    cognicode_core::application::graph_snapshot::save_durable_snapshot(
        &path, &foreign, &graph, &manifest,
    )
    .expect("the foreign legacy snapshot is written");
    match inspect_legacy(&cache, &work) {
        LegacyCache::Stale(at) => assert_eq!(at, path),
        other => panic!("the foreign cache is stale, got {other:?}"),
    }
    assert!(invalidate_legacy(&cache, &work).expect("the stale cache is removed"));
    assert!(!path.exists(), "invalidation leaves nothing behind");
}

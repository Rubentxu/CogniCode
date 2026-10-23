//! A file the reader could not read whole is not published, and the report
//! says why.
//!
//! The generic reader refuses a file it could not parse, and says so. The
//! reader of a language used to publish the declarations it could find in
//! that same file anyway, so the two paths of the product disagreed about
//! what a broken file is: one refused it, the other published half of it
//! and said the coverage was complete. Now the reader of the language
//! refuses it too, and the file stays visible as a hole.

use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, PerFileGraphCache};

/// The corpus: the files the product's own readers are asked about.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/identity_corpus")
}

/// A file that reads whole is published, and a file that does not is named.
#[test]
fn a_file_the_reader_cannot_read_whole_is_not_published_and_is_reported() {
    let corpus = corpus();
    let good = corpus.join("main.tf");
    let broken = corpus.join("broken.tf");
    let cache = PerFileGraphCache::new();
    let report = cache.merge_with_report(&[good.as_path(), broken.as_path()]);

    let names: Vec<String> = report
        .graph
        .symbols()
        .map(|symbol| symbol.name().to_string())
        .collect();

    assert!(
        names.iter().any(|name| name == "aws_s3_bucket.app_bucket"),
        "the file that reads whole publishes its declarations: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|name| name == "aws_s3_bucket.broken_bucket"),
        "a file the reader could not read whole published the declarations it \
         could not read: {names:?}"
    );

    match &report.status {
        BuildStatus::Partial { skipped, .. } => {
            let entry = skipped
                .iter()
                .find(|file| file.path.ends_with("broken.tf"))
                .unwrap_or_else(|| {
                    panic!("the file that could not be read is named in the report: {skipped:?}")
                });
            assert!(
                matches!(
                    entry.reason,
                    cognicode_core::infrastructure::graph::per_file_graph::SkipReason::Parse(_)
                ),
                "the report says why the file was not read: {:?}",
                entry.reason
            );
        }
        BuildStatus::Complete => {
            panic!("a file that could not be read was published as complete coverage")
        }
    }
}

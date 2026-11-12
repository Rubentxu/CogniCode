//! The harness finds the binary it tests.
//!
//! The tests that check the harness used to live inside the shared module,
//! which thirteen test binaries include. From there they ran thirteen times
//! and said the same thing thirteen times. Here they run once.
//!
//! What they protect is not a detail of the harness but the meaning of every
//! black-box check: a test that runs a binary that was built before the
//! change it is supposed to measure passes while measuring nothing.

mod harness {
    use crate::common::*;
    use std::path::{Path, PathBuf};

    /// `binary_path` must return an absolute, well-formed path that ends with
    /// the binary's filename. The actual existence check is environment
    /// dependent (CARGO_BIN_EXE_* may or may not be set, the workspace target
    /// may or may not be built); this test only pins the path shape so a
    /// refactor that, say, dropped the file name is caught immediately.
    #[test]
    fn binary_path_resolves_to_cognicode_mcp_filename() {
        let path = binary_path();
        assert!(
            path.is_absolute() || path.starts_with("/"),
            "binary path must be absolute, got: {}",
            path.display()
        );
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("cognicode-mcp"),
            "binary path must end with the binary name, got: {}",
            path.display()
        );
    }

    /// When `CARGO_BIN_EXE_cognicode-mcp` is set at compile time, the path
    /// returned must point at that env var. The env var is set by Cargo for
    /// integration tests in the same crate as the binary; without it the
    /// resolution falls through to the other branches.
    #[test]
    fn binary_path_prefers_cargo_bin_exe_env_var_when_set() {
        if let Some(expected) = option_env!("CARGO_BIN_EXE_cognicode-mcp") {
            let path = binary_path();
            assert_eq!(
                path.to_str(),
                Some(expected),
                "binary_path must return CARGO_BIN_EXE_cognicode-mcp when set at compile time"
            );
        }
        // If the env var is unset at compile time, the test is a no-op:
        // `binary_path` falls back to the other resolution branches, which
        // are environment dependent and not worth pinning here.
    }

    /// The binary the UATs will actually run must not predate the code.
    ///
    /// The failure this catches is silent and expensive: a workspace that
    /// builds into a shared `CARGO_TARGET_DIR` keeps an older
    /// `target/debug/<binary>` around, every black-box UAT runs against that
    /// stale build, the suite stays green, and the results describe code that
    /// no longer exists.
    ///
    /// The oracle is the newest source under the binary's own crate
    /// (`crates/<this-crate>/`), not the workspace as a whole. The
    /// workspace-wide scan was wrong: cargo's per-target incremental build
    /// watches the bin's own crate plus the fingerprints of crates it
    /// depends on, so source edits in *unrelated* crates are not a reason
    /// to rebuild this bin. Walking the whole tree made the guard a
    /// duplicate (and coarser) freshness check that fired on edits the
    /// build system would not, and made every test stage that ran in an
    /// isolated target dir panic when an unrelated file (e.g. a CHANGELOG
    /// edit or a sibling crate's test) was touched after the build.
    ///
    /// Cargo handles cross-crate staleness itself through fingerprint
    /// propagation, so this test only needs to mirror its per-crate view.
    /// Comparing against the test binary is also wrong: cargo links the
    /// two in the same build and their mtimes differ by however long the
    /// link step took, so a correct setup fails by milliseconds.
    #[test]
    fn the_binary_under_test_is_not_older_than_the_sources_it_was_built_from() {
        let path = binary_path();
        assert!(path.exists(), "resolved binary does not exist: {path:?}");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let binary_time =
            modified(&path).unwrap_or_else(|| panic!("cannot stat the resolved binary: {path:?}"));
        let Some((newest, newest_time)) = newest_source_under(&manifest_dir, "cognicode-mcp")
        else {
            // No sources to compare against: nothing to assert.
            return;
        };
        assert!(
            binary_time >= newest_time,
            "the binary the UATs will run is stale: {path:?} (mtime {binary_time:?}) predates \
         {newest:?} ({newest_time:?}); every black-box UAT would run against a build that \
         does not contain the current code"
        );
    }

    /// The most recently modified source under `root` that `binary` is built
    /// from, with its mtime.
    ///
    /// `tests/` is excluded on purpose: an integration test never links into
    /// the server binary, so a test edited seconds ago must not make a
    /// perfectly current binary look stale. So are the entry points of the
    /// crate's other binaries: `src/server.rs` is the HTTP server, not part
    /// of `cognicode-mcp`, and cargo does not relink `cognicode-mcp` when
    /// only that file changes. Only files the binary is actually built from
    /// are a valid oracle.
    fn newest_source_under(root: &Path, binary: &str) -> Option<(PathBuf, std::time::SystemTime)> {
        let other_entry_points = other_binary_entry_points(root, binary);
        let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
        // `target` holds build output, not sources; skipping it keeps the walk
        // bounded and stops a stale artifact from counting as input.
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name();
                    if name.is_some_and(|n| n == "target" || n == "tests" || n == "benches") {
                        continue;
                    }
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !other_entry_points.contains(&path)
                    && let Some(time) = modified(&path)
                    && newest.as_ref().is_none_or(|(_, best)| time > *best)
                {
                    newest = Some((path, time));
                }
            }
        }
        newest
    }

    /// The `path` of every `[[bin]]` in the crate's manifest other than
    /// `binary`, resolved against `root`.
    ///
    /// Read from the manifest as text: the entries are `name = "…"` and
    /// `path = "…"` lines, and a manifest without `[[bin]]` tables has no
    /// other binary to exclude.
    fn other_binary_entry_points(root: &Path, binary: &str) -> Vec<PathBuf> {
        let Ok(manifest) = std::fs::read_to_string(root.join("Cargo.toml")) else {
            return Vec::new();
        };
        let value = |line: &str, key: &str| -> Option<String> {
            let (found, rest) = line.split_once('=')?;
            (found.trim() == key).then(|| rest.trim().trim_matches('"').to_string())
        };
        let mut entry_points = Vec::new();
        let mut in_bin = false;
        let (mut name, mut path) = (None, None);
        let mut close = |name: &mut Option<String>, path: &mut Option<String>| {
            if let (Some(found), Some(entry)) = (name.take(), path.take())
                && found != binary
            {
                entry_points.push(root.join(entry));
            }
        };
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                if in_bin {
                    close(&mut name, &mut path);
                }
                in_bin = line == "[[bin]]";
            } else if in_bin {
                name = value(line, "name").or(name);
                path = value(line, "path").or(path);
            }
        }
        if in_bin {
            close(&mut name, &mut path);
        }
        entry_points
    }

    fn modified(path: &Path) -> Option<std::time::SystemTime> {
        std::fs::metadata(path).ok()?.modified().ok()
    }

    /// Pin the scope of `newest_source_under` (and the caller
    /// `the_binary_under_test_is_not_older_than_the_sources_it_was_built_from`):
    /// the walk must NOT cross into sibling crates' `src/` trees. An edit
    /// to a file outside this crate is not a reason to rebuild this bin
    /// (cargo's per-crate incremental build agrees), and the guard
    /// pretending it is makes the merge-gate red for unrelated changes.
    ///
    /// The temporary file lives only for the duration of the test; it is
    /// placed inside a subdirectory of the current crate so the walk
    /// actually visits it. The other file is placed OUTSIDE the crate
    /// (sibling directory) and the walk must skip it.
    #[test]
    fn newest_source_under_does_not_see_sibling_crate_sources() {
        let tmp = std::env::temp_dir().join(format!("newest-source-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Fake crate layout: tmp/crate_a/src/ and tmp/crate_b/src/. The
        // walk is rooted at tmp/crate_a and must only see crate_a/src/.
        let in_scope = tmp.join("crate_a").join("src");
        let out_of_scope = tmp.join("crate_b").join("src");
        std::fs::create_dir_all(&in_scope).unwrap();
        std::fs::create_dir_all(&out_of_scope).unwrap();

        // Both files exist before we muck with mtimes. The in-scope
        // file is older than the out-of-scope one, so a buggy walk
        // would report the out-of-scope file as the "newest".
        let in_path = in_scope.join("main.rs");
        let out_path = out_of_scope.join("main.rs");
        std::fs::write(&in_path, b"// inside\n").unwrap();
        std::fs::write(&out_path, b"// outside, newer\n").unwrap();

        let t_in = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        let t_out = std::time::SystemTime::now();
        filetime_set(&in_path, t_in);
        filetime_set(&out_path, t_out);

        fn filetime_set(p: &Path, t: std::time::SystemTime) {
            let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
            f.set_modified(t).unwrap();
        }

        let root = tmp.join("crate_a");
        let (found_path, found_time) =
            newest_source_under(&root, "crate_a").expect("walk must find at least one source");

        assert!(
            found_path.starts_with(&in_scope),
            "walk crossed into a sibling crate: {found_path:?}"
        );
        assert_eq!(
            found_time, t_in,
            "the newest source under the crate must come from inside it"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// The entry point of another binary of the same crate is not an input
    /// of the binary under test: cargo does not relink this binary when only
    /// that file changes, so the guard must not call it stale for it.
    #[test]
    fn newest_source_under_does_not_see_the_entry_points_of_other_binaries() {
        let crate_root = tempfile::tempdir().expect("a crate directory");
        let src = crate_root.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            crate_root.path().join("Cargo.toml"),
            "[package]\nname = \"fake\"\n\n[[bin]]\nname = \"tested\"\npath = \"src/main.rs\"\n\n\
             [[bin]]\nname = \"other\"\npath = \"src/other.rs\"\n\n[dependencies]\n",
        )
        .unwrap();
        let tested_entry = src.join("main.rs");
        let shared_module = src.join("lib.rs");
        let other_entry = src.join("other.rs");
        for (path, age) in [
            (&tested_entry, 120),
            (&shared_module, 60),
            (&other_entry, 0),
        ] {
            std::fs::write(path, b"// source\n").unwrap();
            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(age))
                .unwrap();
        }

        let (found, _) = newest_source_under(crate_root.path(), "tested")
            .expect("the crate has sources of the tested binary");

        assert_eq!(
            found, shared_module,
            "the newest input of `tested` is its own module, not another binary's entry point"
        );
        let (found, _) = newest_source_under(crate_root.path(), "other")
            .expect("the crate has sources of the other binary");
        assert_eq!(
            found, other_entry,
            "the other binary does see its own entry point"
        );
    }
}

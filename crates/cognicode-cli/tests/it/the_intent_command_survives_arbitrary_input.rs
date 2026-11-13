//! `cognicode intent` is a scaffold, and a scaffold must survive any input.
//!
//! The command parses its input best-effort, announces on stderr that its
//! contract is still `PROPOSED`, and exits 0. This file pins that the binary
//! survives arbitrary free text, not the routing it will eventually do.
//!
//! The command was `inspect` until the goal `inspect` took that published
//! name. The consumer's guarantee —"the scaffold survives arbitrary free-text
//! input"— travels with the command, and so does the deprecation notice: a
//! deprecation nobody reads is not a deprecation.
//!
//! ## Why this lives in `cognicode-cli`
//!
//! It used to be a test of `cognicode-core`, whose package does not declare
//! the `cognicode` binary. Cargo builds a package's binaries before its
//! integration tests, so core could not be handed the path and went looking:
//! first in the environment, then in the target directory that `cargo
//! metadata` reported, then in `<repo>/target`. That search paid a `cargo`
//! subprocess on every run, and it could land on a binary built from another
//! checkout. The package that builds the binary is the one that is handed
//! its path (`common::binary_path`), so the test belongs to it.

use std::process::Command;

use crate::common;

#[test]
fn intent_subcommand_does_not_panic_on_arbitrary_input() {
    let bin = common::binary_path("cognicode");

    let inputs: &[&[&str]] = &[
        &["intent", "fn", "foo"],
        &["intent", "path/to/file.rs:10-20"],
        &["intent", ""],
        &["intent", "completely", "arbitrary", "text"],
        &["intent", "sym::Method"],
        &["intent", "verify that 2+3=5"],
        &["intent", "summary of crates/cognicode-core"],
    ];

    for args in inputs {
        let out = Command::new(&bin)
            .args(*args)
            .output()
            .expect("spawn cognicode binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let code = out.status.code().unwrap_or(-1);
        assert_eq!(
            code, 0,
            "intent must exit 0 (no panic) for args={args:?}; \
             stderr={stderr}"
        );
        // The scaffold must announce its provisional status so the
        // agent does not treat the result as the production contract.
        assert!(
            stderr.contains("PROPOSED"),
            "stderr must contain the PROPOSED status marker for \
             args={args:?}; got stderr={stderr}"
        );
        // And it must say where it came from.
        assert!(
            stderr.contains("Deprecated: this command was `cognicode inspect`"),
            "stderr must carry the deprecation notice for args={args:?}; \
             got stderr={stderr}"
        );
    }
}

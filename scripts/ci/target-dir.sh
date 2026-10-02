#!/usr/bin/env bash
# Print the directory cargo actually writes build output to.
#
# ## Why this exists
#
# `target/release/<bin>` is a guess. Cargo puts artifacts there only when
# nothing else says otherwise, and three things routinely say otherwise:
# `CARGO_TARGET_DIR`, a repository `.cargo/config.toml`, and a `~/.cargo/config.toml`.
# On a hosted CI runner the first two are almost always unset, which is why a
# workflow full of `target/release/...` can be correct for years and still be
# wrong the moment the pipeline runs on a developer machine.
#
# That is not hypothetical here. The first end-to-end run of
# `merge-gate.pipeline.kts` on 2026-10-02 built all three release binaries and
# then failed `verify-release-binaries` with:
#
#     FAIL: target/release/cognicode was not built
#
# The build had succeeded. This machine's `~/.cargo/config.toml` sets
# `build.target-dir = /var/home/rubentxu/cargo-targets`, because it is shared
# by several checkouts and concurrent agents. The stage that built the
# binaries asked cargo where to put them; the stage that verified them had the
# answer hardcoded. A build stage and its verifier must ask the same authority,
# or the verifier is checking a directory nothing was ever going to write.
#
# `cargo metadata` is the authority — not a re-implementation of its config
# resolution. This script deliberately does not parse `config.toml` itself: the
# layering of environment over repository config over user config is cargo's
# business, and a second implementation of it here would drift exactly the way
# the hardcoded path did.
#
# Usage:  TARGET_DIR=$(scripts/ci/target-dir.sh)
# Exit:   0 with an absolute path on stdout; non-zero if cargo cannot be run
#         and no fallback applies.

set -euo pipefail

cd "$(dirname "$0")/../.."

if metadata=$(cargo metadata --format-version 1 --no-deps 2>/dev/null); then
    resolved=$(printf '%s' "$metadata" \
        | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])' 2>/dev/null || true)
    if [[ -n "$resolved" ]]; then
        printf '%s\n' "$resolved"
        exit 0
    fi
fi

# cargo could not be run or answered with something unreadable. Fall back the
# way cargo itself would, and say so on stderr so a stage that runs on a broken
# checkout does not fail with a confusing "no such file" much later.
if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    printf '%s\n' "$CARGO_TARGET_DIR"
else
    echo "warning: cargo metadata did not resolve a target directory; assuming $PWD/target" >&2
    printf '%s\n' "$PWD/target"
fi

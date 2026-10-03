// release-candidate — build a candidate and prove it, without publishing it.
//
// This is the first half of the release factory, extracted from
// `.github/workflows/release.yml` (`preflight`, `build`, and the validation
// half of `release`) and from the `build` / `validate` jobs of
// `release-validate.yml`. The publication half lives in `release.pipeline.kts`.
//
// The chain, which is the whole point:
//
//     source SHA
//         ↓
//     build candidate          release-tool, per-platform binaries, SBOMs
//         ↓
//     immutable candidate      dist/*.tar.gz + SHA256SUMS + BundleManifest v2
//         ↓
//     validate those same      cognicode-release verify, install smoke,
//     artifacts                standalone extraction
//
// and then `release.pipeline.kts` publishes exactly the files this lane wrote.
//
// WHY THAT IS NOT A RESTATEMENT
// -----------------------------
// The workflow expressed "publish what you validated" through
// `actions/upload-artifact` in one job and `actions/download-artifact` in
// another, and the property depended on that transfer being faithful. Measured
// on 2026-10-02 under pipelinek 0.46.0, stages share one filesystem and one
// working directory — a file a stage writes is visible to the next. So the
// candidate is a directory. `release.pipeline.kts` reads `release/` and
// `dist/`; it has no way to rebuild, and nothing to accidentally rebuild, which
// is the property the whole chain exists to guarantee. See
// `scripts/ci/probe-pipelinek-semantics.sh`.
//
// The two negative tests from `release-validate.yml` — that `verify` rejects a
// missing archive and rejects an altered one — are carried into
// `release.pipeline.kts` rather than dropped. They are the only thing that
// makes "the checker would have noticed" a fact, and they were blocking there.
//
// WHAT IS NOT HERE, AND WHY
// -------------------------
// `actions/upload-artifact` and `actions/download-artifact` are absent on
// purpose. They existed to move a payload between isolated runners, and the
// isolation is the thing that does not exist here. Copying them would import
// the mechanism and leave the property unenforced.
//
// `gh release …` is absent because this lane must not publish. Those calls are
// in `release.pipeline.kts`, and they are calls to the distribution channel
// rather than to an orchestrator: GitHub Releases is where the product goes,
// and `gh` is how you talk to it. PipelineK is what decides to call it.
//
// Matrix → explicit stages. The workflow fans `build` over two runners,
// x86_64 and aarch64, because each leg has to run natively. A PipelineK script
// has no runner matrix, so the two platforms are two stage groups spelled out.
// Same precedent as the feature matrix in `integration.pipeline.kts`: Actions
// collapses a matrix into one parameterised gate, which loses per-platform
// identity in a log, and here that identity is the difference between "the
// aarch64 leg failed" and "build failed".
//
// The one thing that cannot be faked away: building for a foreign target
// needs that target's toolchain and a linker for it. `toolchain-for-<target>`
// checks and says so plainly rather than producing a binary that cannot run.

import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

// The two platforms release.yml builds for. Declared once, used by every stage
// below, so that adding a platform is one edit rather than a hunt for the four
// places a target name appears.
val targets = listOf("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")

// The version the workspace declares, read the way the workflows read it: the
// first `version = "..."` line of the root manifest.
//
// An empty tag means a dry run, and a dry run that stamps the candidate
// `0.0.0-dev` fails the coherence check by construction — `verify --version
// 0.0.0-dev` against a workspace at 0.100.0 is guaranteed to be red, so the
// "run it locally to see if it works" path could never work. The placeholder
// was a nicer-looking way of saying the same thing.
fun workspaceVersion(root: String): String {
    val manifest = File("$root/Cargo.toml")
    if (!manifest.isFile) {
        error("no Cargo.toml at $root; this is not the repository root")
    }
    val declared = manifest.readLines()
        .firstOrNull { it.trimStart().startsWith("version") }
        ?.let { Regex("version\\s*=\\s*\"([^\"]+)\"").find(it) }
        ?.groupValues
        ?.get(1)
    return declared ?: error(
        "Cargo.toml declares no `version = \"...\"`, so there is nothing to " +
            "stamp a candidate with. A placeholder would make every dry run red."
    )
}

// `x86_64-unknown-linux-gnu` -> `linux-x86-64`. The release tool's `--platform`
// spelling is its own, not cargo's, and the mapping between the two is the
// product's rather than this script's.
//
// The hyphens matter and were wrong here until 2026-10-02: this arm said
// `linux-x86_64`, and `cognicode-release plan --platform linux-x86_64` answers
// `Error: unknown platform`. Nothing compared this mapping against the contract,
// because the only check that did read the lane — `scripts/check-release-matrix.sh`
// — was reading `.github/workflows/release.yml` instead, where the id was
// spelled correctly. The packaging stage would have failed on the first real
// run. The two spellings also disagreed with each other inside this file:
// `linux-aarch64` has always been hyphenated here.
//
// `scripts/product/release_lane.py` reads both this mapping and `val targets`
// and refuses to answer when they disagree, and
// `scripts/check-release-matrix.sh` now compares this name against the
// contract's own `platform-token`. That is what catches the next one.
fun platformOf(target: String): String = when (target) {
    "x86_64-unknown-linux-gnu" -> "linux-x86-64"
    "aarch64-unknown-linux-gnu" -> "linux-aarch64"
    else -> error("no release platform is declared for cargo target `$target`. Add it to `targets` and to `platformOf` together, so a target can never be built without a platform name to package it under.")
}

// The tag being released. A pipeline run has no `push` event to read a ref name
// from, so it is an input: `RELEASE_TAG=v1.2.3 pipelinek run …`. Empty means
// "derive from the tree", which is what a pre-release dry run wants.
val tag: String = System.getenv("RELEASE_TAG") ?: ""
val version: String = if (tag.isEmpty()) workspaceVersion(repoRoot) else tag.removePrefix("v")

// The release tool is built once and reused by packaging, generation and
// verification, rather than rebuilt per stage as the workflow's three jobs each
// did. One build, one binary, one version.
//
// Only the *directory* is resolved here, and each command still spells the tool
// by name. Binding the whole path to a shell variable — `TOOL=…` then `$TOOL
// verify …` — reads well and hides the program from the wiring contracts, which
// have to recognise this gate as the one the lane runs. Resolving the directory
// is what the machine decides; naming the binary is what the pipeline decides,
// and the second one is the gate.
//
// `target/release/` is where cargo writes only when nothing else says otherwise,
// and on this repository it does not: `~/.cargo/config.toml` sets
// `build.target-dir`, because the checkout is one of several sharing a machine.
// A path computed in Kotlin at script-compile time would be wrong on exactly
// those machines, and wrong in the direction that looks most like success. Cargo
// is the authority for where its own output goes; every stage that needs a built
// artifact asks it through `scripts/ci/target-dir.sh`.
val releasePaths = """
    $cd || exit 1
    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
""".trimIndent()

pipeline {
    stages {
        // ------------------------------------------------------------- preflight
        stage("preflight") {
            stage("qw04-preflight-contract") {
                sh("$cd && cargo test -p cognicode-cli --test qw04_preflight_contract --quiet")
            }

            // The clean clone is the property that a build of a dirty tree does
            // not have. It stays the first thing that runs.
            stage("clean-clone") {
                sh("$cd && bash scripts/ci/preflight-clean-clone.sh \"${'$'}{RELEASE_SHA:-HEAD}\"")
            }
        }

        // ----------------------------------------------------------- coherence
        stage("coherence") {
            stage("tag-coherence") {
                sh("""
                    $cd || exit 1
                    if [ -z "${'$'}{RELEASE_TAG:-}" ]; then
                        echo "RELEASE_TAG is unset, so this is a dry run: there is no tag to be coherent with."
                        exit 0
                    fi
                    bash scripts/ci/release-tag-coherence.sh "${'$'}RELEASE_TAG" "${'$'}{RELEASE_SHA:-HEAD}"
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------- supply
        stage("supply-chain") {
            // The workflow installs cargo-deny per lane and tolerates a failure
            // there, because the install failing surfaces as the gate that needs
            // it failing. Same shape here: the version is pinned to the one
            // deny.toml is written against, and the install is not the verdict.
            stage("install-cargo-deny") {
                sh("$cd && cargo install cargo-deny --version 0.20.2 --locked || echo 'the install is not the verdict; the gate below reports it'")
            }

            stage("advisories") {
                sh("$cd && cargo deny check advisories")
            }

            stage("licenses") {
                sh("$cd && cargo deny check licenses")
            }
        }

        // --------------------------------------------------------------- tools
        stage("toolchain") {
            stage("release-tool") {
                sh("$cd && cargo build --release --bin cognicode-release")
            }
        }

        // ---------------------------------------------------------------- build
        // One stage group per platform, in the same order the matrix produced.
        stage("build") {
            targets.forEach { target ->
                stage("toolchain-for-$target") {
                    sh("""
                        $cd || exit 1
                        rustup target list --installed | grep -qx '$target' || {
                            echo "cargo target '$target' is not installed."
                            echo "  rustup target add $target"
                            echo "A cross target also needs a linker for it. Building for a target whose"
                            echo "binaries cannot link produces a candidate that looks built and is not."
                            exit 1
                        }
                        # Having the target installed is half the toolchain. This tree
                        # carries crates with C and C++ build scripts --`ring`,
                        # `tree-sitter`, `link-cplusplus`-- so a target whose cross
                        # compiler is missing, or present but unable to parse the
                        # triple `cc-rs` appends, fails here in milliseconds instead
                        # of twenty minutes later inside somebody else's build
                        # script. MEDIDO 2026-10-03 on v0.101.4: the rustup check
                        # passed, and `binaries-aarch64` then failed three ways.
                        scripts/ci/check-cross-toolchain.sh '$target'
                    """.trimIndent())
                }

                stage("binaries-$target") {
                    sh("""
                        $cd || exit 1
                        # cogh consumes evidence-gated types, so the feature is on for
                        # the whole cognicode-cli package, not per binary.
                        cargo build --release --target $target \
                          -p cognicode-cli --bin cogh --bin cognicode \
                          --features cognicode-core/evidence-kernel
                        cargo build --release --target $target \
                          -p cognicode-mcp --bin cognicode-mcp
                    """.trimIndent())
                }

                stage("sbom-$target") {
                    sh("$cd && bash scripts/ci/build-sboms-for-lane.sh $target")
                }

                // The lane's output has to land where the flatten script looks
                // for it: `staging/payloads-<platform>/dist/` and
                // `staging/payloads-<platform>/crates/`. This stage wrote
                // `dist/*.tar.gz` at the repository root, because that is where
                // the workflow's `upload-artifact` step listed its `path:` and
                // the transfer itself — the thing that used to build the lane
                // directories — has no equivalent here. So `payloads` failed
                // with "no payloads-* lane directories found", and `generate`
                // had an empty staging tree to work from.
                //
                // The directory layout is not an accident of the YAML even
                // though the transfer was: `stage-platform-payloads.sh` reads
                // exactly this shape, rejects anything else at the staging root,
                // and is exercised by five negative cases. Migrating the
                // property means producing the layout it consumes.
                stage("package-$target") {
                    sh(releasePaths + "\n" + """
                        platform=${platformOf(target)}
                        lane="staging/payloads-${'$'}platform"
                        mkdir -p "${'$'}lane/dist" "${'$'}lane/crates"
                        # The published product surface is the contract's, not this
                        # script's: `plan` prints the canonical filenames, and the
                        # component name is recovered by stripping the derived
                        # `-{version}-{token}.tar.gz` suffix. A third list of
                        # components written here would be a third place to forget.
                        planned=0
                        for filename in $("${'$'}TARGET_DIR/release/cognicode-release" plan --platform "${'$'}platform" --version "$version"); do
                            planned=$((planned + 1))
                            # `$version` is the Kotlin value, not a shell one: it is
                            # what `--version "$version"` above already used. Spelled
                            # `${'$'}version` this is an *unset* shell variable, the
                            # suffix pattern collapses to `---*`, matches nothing, and
                            # `component` silently keeps the whole archive filename —
                            # so `cp` looked for a file that does not exist and no
                            # payload was produced. MEDIDO 2026-10-03, first candidate
                            # to reach this stage since the lane existed.
                            component="${'$'}{filename%-$version-*}"
                            filename=$("${'$'}TARGET_DIR/release/cognicode-release" name --component "${'$'}component" \
                                        --platform "${'$'}platform" --version "$version")
                            if [ -z "${'$'}filename" ]; then
                                echo "FAIL: name --component ${'$'}component produced no filename."
                                echo "  A component with no name cannot be packaged, and an empty"
                                echo "  name would make tar write to the dist/ directory itself."
                                exit 1
                            fi
                            stage=$(mktemp -d)
                            mkdir -p "${'$'}stage/bin"
                            cp "${'$'}TARGET_DIR/$target/release/${'$'}component" "${'$'}stage/bin/${'$'}component" || {
                                echo "FAIL: ${'$'}TARGET_DIR/$target/release/${'$'}component is missing."
                                echo "  binaries-${'$'}target runs before this one and did not produce it."
                                exit 1
                            }
                            tar -czf "${'$'}lane/dist/${'$'}filename" -C "${'$'}stage" bin || {
                                echo "FAIL: could not package ${'$'}filename"
                                exit 1
                            }
                            echo "packaged ${'$'}filename for ${'$'}platform"
                            rm -rf "${'$'}stage"
                        done
                        # `plan` answering with nothing is not an empty release, it
                        # is a stage that cannot see the product surface. Without
                        # this the loop above simply does not run and the stage
                        # reports success having produced no payload at all — which
                        # is how the defect above survived a whole candidate run and
                        # only surfaced two stages later, in a message about the
                        # archive rather than about packaging.
                        if [ "${'$'}planned" -lt 1 ]; then
                            echo "FAIL: cognicode-release plan produced no artifacts for ${'$'}platform."
                            echo "  An empty candidate is not a candidate."
                            exit 1
                        fi
                        produced=$(ls -1 "${'$'}lane/dist" | wc -l)
                        if [ "${'$'}produced" -ne "${'$'}planned" ]; then
                            echo "FAIL: planned ${'$'}planned artifacts for ${'$'}platform, produced ${'$'}produced."
                            exit 1
                        fi
                        # The SBOM belongs to the lane, not to the repository: the
                        # flatten script pairs `<component>-<triple>.cdx.json`
                        # with the payloads of the same lane, and a payload whose
                        # SBOM sits elsewhere is a payload with no evidence.
                        for component in cogh cognicode cognicode-mcp; do
                            sbom="crates/${'$'}component-$target.cdx.json"
                            if [ ! -f "${'$'}sbom" ]; then
                                echo "FAIL: ${'$'}sbom is missing."
                                echo "  build-sboms-for-lane.sh writes crates/<component>-<target>.cdx.json;"
                                echo "  the sbom-$target stage runs before this one and did not produce it."
                                exit 1
                            fi
                            cp "${'$'}sbom" "${'$'}lane/crates/"
                        done
                        ls -la "${'$'}lane/dist" "${'$'}lane/crates"
                    """.trimIndent())
                }

                stage("binary-smoke-$target") {
                    sh(releasePaths + "\n" + """
                        export HOME=$(mktemp -d)
                        export XDG_CONFIG_HOME="${'$'}HOME/.config"
                        export XDG_DATA_HOME="${'$'}HOME/.local/share"
                        export COGNICODE_HOME="${'$'}HOME/.cognicode"
                        # `--target $target` puts cross output under the target
                        # subdirectory of whatever cargo's target dir is.
                        dir="${'$'}TARGET_DIR/$target/release"
                        "${'$'}dir/cogh" --version
                        "${'$'}dir/cogh" --help | head -5
                        "${'$'}dir/cognicode" --version
                        "${'$'}dir/cognicode-mcp" --version
                    """.trimIndent())
                }

                stage("archive-standalone-$target") {
                    sh(releasePaths + "\n" + """
                        export HOME=$(mktemp -d)
                        work=$(mktemp -d)
                        for archive in "staging/payloads-${platformOf(target)}"/dist/*.tar.gz; do
                            tar -xzf "${'$'}archive" -C "${'$'}work"
                        done
                        # No Rust toolchain and no repository files after extraction.
                        test -x "${'$'}work/bin/cogh" || { echo "cogh missing or not executable from the archive"; exit 1; }
                        "${'$'}work/bin/cogh" --version
                        test -x "${'$'}work/bin/cognicode" || { echo "cognicode missing or not executable from the archive"; exit 1; }
                        "${'$'}work/bin/cognicode" --version
                    """.trimIndent())
                }
            }
        }

        // ------------------------------------------------------------- candidate
        stage("candidate") {
            // The flatten script rejects anything at the staging root that is
            // not a `payloads-*` lane directory or a pre-staged skill bundle, and
            // `generate` expects the bundles there. The workflow had a step for
            // this between the artifact transfer and the flatten; the port did
            // not, so `payloads` and `generate` both ran against a tree that was
            // missing part of what it publishes.
            stage("skill-bundles") {
                sh(releasePaths + "\n" + """
                    mkdir -p staging
                    # Which bundles are published is the contract's answer, not a
                    # list written here. The workflow carried a python fallback
                    # for when the binary was absent; the candidate lane builds
                    # the tool two stages earlier and has no reason to.
                    #
                    # MEDIDO 2026-10-03. This read the answer through a pipe
                    # (`done < <(... skills --published)`) and trusted that
                    # reaching the end of the loop meant the tool had answered.
                    # It had not: the subcommand did not exist, the binary exited
                    # 2, the loop consumed nothing, and the stage passed with zero
                    # bundles. `verify` could not catch it either — it recognised
                    # skill bundles if present but never required them. So the
                    # answer is fetched first and checked, and the count of what
                    # was actually staged is asserted. Asking a question is not
                    # the same as receiving an answer.
                    bundles=$("${'$'}TARGET_DIR/release/cognicode-release" skills --published) || {
                        echo "FAIL: the release tool could not report the published skill bundles."
                        echo "  Without its answer this stage cannot know what to stage, and"
                        echo "  staging nothing is not the same as having nothing to stage."
                        exit 1
                    }
                    if [ -z "${'$'}bundles" ]; then
                        echo "FAIL: the release tool reports no published skill bundles."
                        echo "  The contract publishes them; an empty list means the tool and"
                        echo "  the contract disagree."
                        exit 1
                    fi
                    staged=0
                    while IFS= read -r id; do
                        [ -n "${'$'}id" ] || continue
                        if [ ! -f "skills/${'$'}id/manifest.yaml" ]; then
                            echo "FAIL: published skill bundle '${'$'}id' has no skills/${'$'}id/manifest.yaml"
                            exit 1
                        fi
                        tar -czf "staging/${'$'}id-$version.tar.gz" -C "skills/${'$'}id" . || {
                            echo "FAIL: could not stage skill bundle '${'$'}id'"
                            exit 1
                        }
                        staged=$((staged + 1))
                        echo "staged ${'$'}id-$version.tar.gz"
                    done <<< "${'$'}bundles"
                    if [ "${'$'}staged" -lt 1 ]; then
                        echo "FAIL: nothing was staged from a non-empty bundle list."
                        exit 1
                    fi
                    ls -la staging
                """.trimIndent())
            }

            stage("payloads") {
                sh("$cd && bash scripts/ci/stage-platform-payloads.sh staging")
            }

            stage("generate") {
                sh(releasePaths + "\n" + """
                    # `--tag` and `--source-commit` are not decoration:
                    # `source_commit` is a required field of the release
                    # inventory, and `prf_dist_01_06_release_candidate_uat`
                    # asserts the inventory records HEAD. A manifest that does
                    # not name the commit it was built from cannot be tied to
                    # one, which is the first link of the chain this lane exists
                    # to keep.
                    source=$(git rev-parse HEAD)
                    "${'$'}TARGET_DIR/release/cognicode-release" generate \
                        --staging staging \
                        --out release \
                        --version "$version" \
                        --tag "$tag" \
                        --source-commit "${'$'}source"
                """.trimIndent())
            }

            // The candidate is now a set of files on disk, hashed by
            // SHA256SUMS. Everything after this point is about those files, not
            // about rebuilding anything.
            stage("verify") {
                sh(releasePaths + "\n" + """
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging release --version "$version"
                """.trimIndent())
            }

            stage("install-smoke") {
                sh("""
                    $cd || exit 1
                    bash scripts/ci/release-install-smoke.sh "$version" release
                """.trimIndent())
            }

            stage("candidate-manifest") {
                sh("""
                    $cd || exit 1
                    test -f release/SHA256SUMS || {
                        echo "the candidate has no SHA256SUMS. Without it, the release lane has"
                        echo "no way to prove it publishes the files this lane produced."
                        exit 1
                    }
                    echo "candidate: $(ls -1 release/*.tar.gz 2>/dev/null | wc -l) archive(s)"
                    echo "candidate hashes: $(wc -l < release/SHA256SUMS) line(s)"
                """.trimIndent())
            }
        }
    }
}

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
val logDir: String = File(System.getProperty("java.io.tmpdir"), "cognicode-logs").absolutePath
val cdLog = "mkdir -p \"$logDir\""

// The platforms release.yml builds for. Declared once, used by every stage
// below, so that adding a platform is one edit rather than a hunt for the four
// places a target name appears.
//
// 2026-10-10: scope = x86_64-only for THIS release (v0.101.10 was published
// x86_64-only per commit 451d0b7, TF-5b). The operator decision to use the
// CI with pipelinek 0.48.0-rc + cross toolchain (commit 8c6d0f7, TF-6.0 /
// TF-7.0) is for a FUTURE release, not this one: aarch64 stays out until
// the CI runner is fully provisioned AND end-to-end tested with a
// no-op v0.101.11 first. Tracking: `odd/tasks/tf-6-aarch64-followup.md`.
//
// To re-enable aarch64 for a future release:
//   1. Confirm the CI runner is provisioned (TF-7.2) and the cross
//      toolchain is pre-installed.
//   2. Restore the aarch64 entry to the list below AND the
//      `aarch64-unknown-linux-gnu` arm in `platformOf`. Both, together:
//      `scripts/product/release_lane.py` reads both declarations and
//      refuses to answer when they disagree, so an arm without a target is
//      as broken as a target without an arm — the first one would publish
//      a platform nothing builds. The platform name is `linux-aarch64`,
//      hyphenated, and it must stay that way (see `platformOf` below).
//   3. Verify the candidate builds in the CI with both targets.
//   4. Then run the release-candidate + release pipelines against the CI.
val targets = listOf("x86_64-unknown-linux-gnu")

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
//
// 2026-10-15: the `aarch64-unknown-linux-gnu` arm that used to sit here is
// gone. It was a note-to-self parked in executable position, and the reader
// cannot tell an inert arm from a live one — it read the file and reported a
// certified `linux-aarch64` that nothing in this lane builds. The re-enable
// steps above carry the spelling instead, where it cannot be mistaken for a
// declaration.
fun platformOf(target: String): String = when (target) {
    "x86_64-unknown-linux-gnu" -> "linux-x86-64"
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

// The environment a release-contract suite runs in.
//
// 2026-10-15 / B1 (ADR-TH-005). These suites execute real release-profile
// bytes: they tar `cogh`, `cognicode` and `cognicode-mcp` out of the directory
// they are pointed at, run `cognicode-release generate` and `verify` on them,
// and spawn two processes for the CLI/MCP equivalence UAT. Two variables carry
// that intent into the suites, and both suites refuse a path that is not a
// release-profile one, so a debug binary that `cargo test` happened to build
// cannot be mistaken for the artifact under test:
//
//   COGNICODE_RELEASE_BIN      the release tool this lane built
//   COGNICODE_RELEASE_BIN_DIR  the directory with the host target's payloads
//
// The host guard is not decoration. The payloads below are host binaries, so
// running the suites for a foreign target would either fail to spawn or, worse,
// execute a stale host artifact and report it as evidence for a platform
// nothing built. The target this group is written for is the first declared
// platform, and a mismatch says so instead of hiding it.
fun releaseContractEnv(target: String) = """
    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
    HOST=$(rustc -vV | sed -n 's/^host: //p')
    if [ -z "${'$'}HOST" ]; then
        echo "FAIL: rustc did not report its host triple."
        exit 1
    fi
    if [ "${'$'}HOST" != "$target" ]; then
        echo "SKIP_NOT_APPLICABLE: $target payloads are not host binaries (host=${'$'}HOST)."
        echo "  The release-contract suites execute real binaries, so they are evidence"
        echo "  for the host platform. This target's own evidence is the per-payload"
        echo "  stages above, which run for every target."
        exit 0
    fi
    export CARGO_TARGET_DIR="${'$'}TARGET_DIR"
    export COGNICODE_RELEASE_BIN="${'$'}TARGET_DIR/release/cognicode-release"
    export COGNICODE_RELEASE_BIN_DIR="${'$'}TARGET_DIR/${'$'}HOST/release"
    for bin in cogh cognicode cognicode-mcp; do
        if [ ! -x "${'$'}COGNICODE_RELEASE_BIN_DIR/${'$'}bin" ]; then
            echo "FAIL: ${'$'}COGNICODE_RELEASE_BIN_DIR/${'$'}bin is missing or not executable."
            echo "  binaries-$target builds it, and a suite that tars real release"
            echo "  binaries cannot run without it: that is not a reason to skip it."
            exit 1
        fi
    done
    if [ ! -x "${'$'}COGNICODE_RELEASE_BIN" ]; then
        echo "FAIL: ${'$'}COGNICODE_RELEASE_BIN is missing; the toolchain stage builds it."
        exit 1
    fi
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
                // MEDIDO 2026-10-17. El transcript de cargo build --release
                // --workspace --bins se desborda del persister de PipelineK
                // (10s drain, varios MB de stdout). Misma forma que
                // `core-unrestricted-evidence-kernel` en merge-gate.pipeline.kts:
                // redirigir a archivo, hacer tail en pantalla, decir donde
                // esta el log completo. El stage sigue siendo PASS/FAIL por
                // exit code, no por bytes emitidos.
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    mkdir -p "$logDir" || exit 1
                    if ! bash scripts/ci/preflight-clean-clone.sh "${'$'}{RELEASE_SHA:-HEAD}" \
                        > "$logDir/clean-clone.log" 2>&1; then
                        tail -40 "$logDir/clean-clone.log"
                        echo "the full log is $logDir/clean-clone.log"
                        exit 1
                    fi
                """.trimIndent())
            }
        }

        // ----------------------------------------------------------- coherence
        stage("coherence") {
            stage("tag-coherence") {
                sh("""
                    set -eo pipefail
                    set -eo pipefail
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
                sh("""
                    set -eo pipefail
                    set -eo pipefail
                    $cd || exit 1
                    cargo build --release --bin cognicode-release
                    # MEDIDO 2026-10-03. Esta stage se quedaba con "cargo
                    # build salio 0", y en una maquina con `build.target-dir`
                    # compartido eso no dice de que checkout salio el binario.
                    # La lane de v0.101.5 ejecuto una `cognicode-release` de
                    # otro arbol y lo supo veinte minutos despues, en
                    # `skill-bundles`, con "unrecognized subcommand 'skills'":
                    # un subcomando que el arbol si tiene. Cargo compara
                    # mtimes, y las fuentes de este checkout son mas antiguas
                    # que un binario que otro checkout construyo despues.
                    #
                    # MEDIDO: la guarda que se le pide ahi no era
                    # suficiente. Barria TODOS los `.rs` de `crates/`, y eso
                    # da un falso positivo en la direccion contraria: un
                    # fichero que no entra en `cognicode-release` —`ide.rs`—
                    # hacia fallar la stage con un binario perfectamente
                    # fresco. Y una lista de fuentes escrita a mano tampoco
                    # vale: MEDIDO, este binario lo construyen 415 ficheros
                    # repartidos en cuatro crates, 368 de ellos en
                    # `cognicode-core`. Una lista de los cuatro ficheros del
                    # CLI habria sido verde en falso.
                    #
                    # Por eso la guarda lee el dep-info que escribe rustc junto
                    # al binario, que es la respuesta de cargo y nombra tambien
                    # el arbol desde el que se construyo. La pregunta "¿este
                    # binario es de este arbol?" tiene respuesta directa.
                    bash scripts/ci/check-built-binary.sh \
                        "$(scripts/ci/target-dir.sh)/release/cognicode-release" .
                """.trimIndent())
            }
        }

        // ------------------------------------------------------- host build + docs
        // Moved here from `merge-gate` on 2026-10-15, with the operator's
        // approval. Named `host-build-and-verify` because the `build` stage
        // below is a container for the per-target cross builds, and two stages
        // with one name in one file is the sort of thing nobody notices until
        // a failure is attributed to the wrong one.
        //
        // Measured: in the merge gate these two cost ~700 s and ~472 s of a
        // ~1900 s run — 62% of the gate producing artefacts a merge does not
        // publish. Everything else it checks fits in 730 s.
        //
        // What that costs, stated plainly: a merge no longer proves the release
        // binaries build. It is proved here, on the lane that actually cuts a
        // release, so a broken release build surfaces when preparing to
        // publish rather than when merging. One class of failure is detected
        // later; nothing else changes.
        //
        // `cognicode-control-plane` is a HOST binary. The per-target stage below
        // cross-compiles cogh, cognicode and cognicode-mcp; this is the only
        // place in any lane where the control plane is built.

            stage("public-docs") {
            // the workspace builds its documentation with zero warnings
            // Formerly 1 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            set -eo pipefail
            set -eo pipefail
            # ── rustdoc-gate
            # ─────────────────────────────────────────────
            $cd && cargo test -p cognicode-runtime --test m011_rustdoc_gate --quiet
            """.trimIndent())
            }

            stage("host-build-and-verify") {
            // every binary a release needs, built in release profile
            // Formerly 6 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
                    set -eo pipefail
                    set -eo pipefail
            # ── build-binary
            # ─────────────────────────────────────────────
            #
            # MEDIDO 2026-10-21. Este bloque verificaba los tres binarios release
            # ANTES de construirlos: la comprobacion estaba al principio del
            # cuerpo del shell y los `cargo build --release` estaban despues. La
            # lane solo pasaba en una maquina cuyo target dir ya tuviera los
            # binarios de antes; en una maquina limpia moria con
            #
            #     FAIL: .../release/cognicode was not built
            #
            # sin haber construido nada. El gate de candidato de v0.101.25 murio
            # asi, con el arbol correcto y el corte anterior con el mismo gate
            # en verde.
            #
            # La verificacion que importa es la del final del bloque, y es
            # identica: misma comprobacion, mismos binarios, mismo mensaje. La
            # de aqui no anade garantia: solo anade la condicion de que la
            # maquina ya tenga el trabajo hecho.
            #
            # La regla que queda escrita en el codigo: una lane construye lo que
            # necesita y despues lo verifica. `scripts/ci/
            # test_gate_works_on_a_clean_machine.py` la vigila.

            # ── build-cognicode-mcp-release
            # ─────────────────────────────────────────────
            $cd && cargo build --release -p cognicode-mcp

            # ── build-control-plane-release
            # ─────────────────────────────────────────────
            $cd && cargo build --release --bin cognicode-control-plane

            # ── build-cognicode-cli-release
            # ─────────────────────────────────────────────
            $cd && cargo build --release -p cognicode-cli

            # ── build-cognicode-release-bin
            # ─────────────────────────────────────────────
            $cd && cargo build --release --bin cognicode

            # ── verify-release-binaries
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    set -euo pipefail
                    # The binaries were written wherever cargo says they go, which is
                    # not necessarily the repository's `target/`. This stage asked a
                    # hardcoded path for them and failed on 2026-10-02 with
                    # "target/release/cognicode was not built" immediately after the
                    # stage that builds it had succeeded. A verifier that hardcodes
                    # where the artifact lands is not verifying the artifact.
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    for bin in cognicode cognicode-mcp cognicode-control-plane; do
                        if [ ! -f "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin was not built"
                            echo "  cargo target dir: ${'$'}TARGET_DIR"
                            exit 1
                        fi
                        if [ ! -x "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin is not executable"
                            exit 1
                        fi
                    done
                    "${'$'}TARGET_DIR/release/cognicode" --version
                    "${'$'}TARGET_DIR/release/cognicode-control-plane" --help | head -5
                    "${'$'}TARGET_DIR/release/cognicode-mcp" --version
            """.trimIndent())
            }

        // ---------------------------------------------------------------- build
        // One stage group per platform, in the same order the matrix produced.
        stage("build") {
            targets.forEach { target ->
                stage("toolchain-for-$target") {
                    sh("""
                        set -eo pipefail
                        set -eo pipefail
                        $cd || exit 1
                        rustup target list --installed | grep -qx '$target' || {
                            echo "cargo target '$target' is not installed."
                            echo "  rustup target add $target"
                            echo "A cross target also needs a linker for it. Building for a target whose"
                            echo "binaries cannot link produces a candidate that looks built and is not."
                            exit 1
                        }
                        # The block above names two requirements and checks one. The
                        # requirement it does not check is the one that is machine
                        # state, so it is the one that silently disappears.
                        #
                        # MEDIDO 2026-10-03, lane v0.101.4: the target WAS installed,
                        # so this stage reported success, and `binaries-$target` died
                        # two stages later with exit 101 and a parser error about the
                        # target triple — cc-rs handing zig a `--target=` flag zig
                        # cannot read. The diagnostic named the compiler. The thing
                        # that was actually missing is the sentence this stage prints
                        # when the target is absent.
                        #
                        # A linker is configured in `CC_<triple>`, in
                        # `CARGO_TARGET_<TRIPLE>_LINKER`, or as `linker` in a
                        # `[target.<triple>]` table of a cargo config. All three are
                        # outside the repository, which is why this check exists and
                        # is also why it cannot make the cross toolchain reproducible:
                        # it can say whether one is configured and whether it runs,
                        # and nothing more. Whether it understands this target is
                        # `binaries-$target`'s to find out.
                        # NOTE ON DOLLAR SIGNS IN THIS BLOCK. Everything from here
                        # to the end of the linker check is the body of a shell
                        # script written inside a Kotlin raw string, and in a raw
                        # string a dollar sign opens a template. That is true even
                        # inside what looks like a comment: a hash opens a shell
                        # comment, but the line is still content of the raw string,
                        # so a dollar in it is still a template. Writing a dollar
                        # in prose to illustrate a rule is what produces
                        # `Unresolved reference` at compilation, before a single
                        # stage runs. MEDIDO 2026-10-04: two paragraphs of this
                        # comment, explaining how to escape a dollar, contained
                        # unescaped ones and took the whole lane down in the first
                        # two minutes.
                        #
                        # So this block states the rules without writing a dollar.
                        # To emit one to the shell, the form is the Kotlin template
                        # of a string literal holding a dollar, followed by braces
                        # around the name when the name is glued to a suffix —
                        # otherwise Kotlin reads name_PLUS_SUFFIX as one long
                        # identifier. A DEFINITION takes no escape and must not
                        # have one, because a dollar glued to the left of the
                        # equals sign makes the name contain a dollar, and bash
                        # reads the word as a command. MEDIDO 2026-10-03: written
                        # the escaped way, that assignment failed and every
                        # variable below it read as empty.
                        #
                        # The escaping examples are not written here on purpose.
                        # The stage below is the specification, and
                        # `test_pipeline_stage_bodies.py` renders and runs it for
                        # real: three targets crossed, a linker from the
                        # environment, a linker from a committed cargo config, a
                        # linker that does not resolve, and the host case. What
                        # this comment is for is the reader, and a reader who
                        # needs the literal form can read it out of a rendering.
                        host=$(rustc -vV | sed -n 's/^host: //p')
                        if [ "${'$'}host" != "$target" ]; then
                            key=$(printf '%s' "$target" | tr '[:lower:]-' '[:upper:]_')
                            cc_key=$(printf '%s' "$target" | tr '-' '_')
                            linker_var="CARGO_TARGET_${'$'}{key}_LINKER"
                            cc_var="CC_${'$'}{cc_key}"
                            # bash's indirect form takes the variable name directly,
                            # so a `$` in front of it is a bad substitution, and a
                            # `$` inside a parameter name is a second bad
                            # substitution. Hence the two name variables above
                            # instead of nesting the expansion inline.
                            linker="${'$'}{!linker_var:-}"
                            [ -n "${'$'}linker" ] || linker="${'$'}{!cc_var:-}"
                            # `[target.<triple>]` followed by a `linker` line, in
                            # either the repository's config or the user's. A missing
                            # file is an absent answer, not an error here. The
                            # section header is COMPARED, never matched as a regex:
                            # a target triple is full of `-`, and `a-b` inside a
                            # bracket expression is an invalid range, which is how
                            # this first read the header as a fatal regexp error
                            # and reported "no linker is configured" for a file that
                            # had one.
                            cfg_has_linker=no
                            for cfg in "$repoRoot/.cargo/config.toml" "${'$'}{CARGO_HOME:-${'$'}HOME/.cargo}/config.toml"; do
                                [ -f "${'$'}cfg" ] || continue
                                if awk -v section="[target.$target]" '
                                    {
                                        line = $0
                                        gsub(/^[[:space:]]+/, "", line)
                                        gsub(/[[:space:]]+$/, "", line)
                                        if (line == section) { inside = 1; next }
                                        if (substr(line, 1, 1) == "[") { inside = 0 }
                                        if (inside && line ~ /^linker[[:space:]]*=/) { found = 1 }
                                    }
                                    END { exit(found ? 0 : 1) }
                                ' "${'$'}cfg"; then
                                    cfg_has_linker=yes
                                    break
                                fi
                            done
                            if [ -z "${'$'}linker" ] && [ "${'$'}cfg_has_linker" != "yes" ]; then
                                echo "cargo target '$target' is installed but no linker is configured for it."
                                echo "  Host is '${'$'}host', so this is a cross build, and a cross build"
                                echo "  that cannot link produces a candidate that looks built and is not."
                                echo "  Configure one, either as:"
                                echo "    .cargo/config.toml:   [target.$target]"
                                echo "                          linker = \"<linker>\""
                                echo "    or in the environment: CC_${'$'}{cc_key}=\"<linker>\""
                                echo "                          CARGO_TARGET_${'$'}{key}_LINKER=\"<linker>\""
                                exit 1
                            fi
                            if [ -n "${'$'}linker" ] && ! command -v "${'$'}linker" >/dev/null 2>&1 && [ ! -x "${'$'}linker" ]; then
                                echo "the linker configured for '$target' does not resolve: ${'$'}linker"
                                echo "  A linker that is not on PATH and not executable fails later inside"
                                echo "  the build, where the error names the compiler instead of the tool"
                                echo "  that is missing. Check CC_${'$'}{cc_key} and CARGO_TARGET_${'$'}{key}_LINKER."
                                exit 1
                            fi
                            echo "cross target '$target' (host '${'$'}host'): linker configured and resolves"
                        fi
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
                        set -eo pipefail
                        set -eo pipefail
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
                        set -eo pipefail
                        platform=${platformOf(target)}
                        lane="staging/payloads-${'$'}platform"
                        # This directory is this stage's output, and this stage is
                        # its owner. `mkdir -p` on a directory that already exists
                        # is not an innocent no-op: it is the mechanism by which a
                        # build directory inherits state from a previous run. The
                        # worktree that builds the candidate is long lived, so
                        # `mkdir -p` alone meant the previous candidate's payloads
                        # were still in here. MEDIDO 2026-10-03, lane v0.101.5:
                        #
                        #     FAIL: planned 3 artifacts for linux-x86-64, produced 6.
                        #
                        # Three of the six were 0.101.4. The lane is written to be
                        # re-runnable in a worktree that already holds a candidate,
                        # so it starts from a directory that is known to be its own.
                        #
                        # QW-04's "the cleanup cannot be the verdict" does not
                        # apply here, and the difference is the whole point: that
                        # rule is about what happens AFTER the stages have decided,
                        # and this is before anything is decided. If the input
                        # state cannot be established, there is no known state to
                        # package into, and continuing would mean packaging into
                        # whatever the previous lane happened to leave.
                        if [ -e "${'$'}lane" ]; then
                            if ! rm -rf -- "${'$'}lane"; then
                                echo "FAIL: could not clear the lane directory ${'$'}lane"
                                echo "  It exists from a previous candidate, and this lane cannot"
                                echo "  promise that the payloads it produces are its own. Clear it"
                                echo "  by hand, or run in a worktree without a previous candidate."
                                exit 1
                            fi
                            echo "cleared ${'$'}lane from a previous candidate"
                        fi
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
                            # The count is still worth keeping as a last line of
                            # defense, but it no longer tries to name the offender:
                            # the directory is cleared above, so nothing can be in
                            # here that this loop did not just write, and a message
                            # that guesses would be a message that lies. The stage
                            # that can name the files is the flatten script, which
                            # refuses an ambiguous lane by name.
                            echo "FAIL: planned ${'$'}planned artifacts for ${'$'}platform, produced ${'$'}produced."
                            echo "  ${'$'}lane is created fresh by this stage; if this fires, something"
                            echo "  other than this loop wrote into it."
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
                        set -eo pipefail
                        export HOME=$(mktemp -d)
                        export XDG_CONFIG_HOME="${'$'}HOME/.config"
                        export XDG_DATA_HOME="${'$'}HOME/.local/share"
                        export COGNICODE_HOME="${'$'}HOME/.cognicode"
                        # `--target $target` puts cross output under the target
                        # subdirectory of whatever cargo's target dir is.
                        dir="${'$'}TARGET_DIR/$target/release"
                        # MEDIDO 2026-10-03. Esto ejecutaba el binario tal cual.
                        # En el host es ejecutarlo; en un target extranjero entra
                        # binfmt_misc y sale
                        #
                        #     qemu-aarch64-static: Could not open
                        #     '/lib/ld-linux-aarch64.so.1': No such file or directory
                        #
                        # que no dice que la maquina no tiene la libc invitada de
                        # ese target. run-target-binary.sh dice COMO ejecuta, y
                        # cuando falta el emulador o el sysroot lo dice antes de
                        # intentarlo y nombra lo que falta.
                        runner=scripts/ci/run-target-binary.sh
                        for component in cogh cognicode cognicode-mcp; do
                            test -x "${'$'}dir/${'$'}component" || {
                                echo "FAIL: ${'$'}dir/${'$'}component no existe o no es ejecutable."
                                echo "  binaries-${'$'}target corre antes de esta stage y no lo produjo."
                                exit 1
                            }
                        done
                        bash "${'$'}runner" $target "${'$'}dir/cogh" --version
                        bash "${'$'}runner" $target "${'$'}dir/cogh" --help | head -5
                        bash "${'$'}runner" $target "${'$'}dir/cognicode" --version
                        bash "${'$'}runner" $target "${'$'}dir/cognicode-mcp" --version
                    """.trimIndent())
                }

                stage("archive-standalone-$target") {
                    sh(releasePaths + "\n" + """
                        set -eo pipefail
                        export HOME=$(mktemp -d)
                        work=$(mktemp -d)
                        for archive in "staging/payloads-${platformOf(target)}"/dist/*.tar.gz; do
                            tar -xzf "${'$'}archive" -C "${'$'}work"
                        done
                        # No Rust toolchain and no repository files after extraction.
                        test -x "${'$'}work/bin/cogh" || { echo "cogh missing or not executable from the archive"; exit 1; }
                        test -x "${'$'}work/bin/cognicode" || { echo "cognicode missing or not executable from the archive"; exit 1; }
                        # MEDIDO 2026-10-03. El binario extraido se ejecutaba
                        # directamente, con el mismo problema que binary-smoke:
                        # en un target extranjero eso no es ejecutarlo, y el
                        # fallo que sale no dice que falte la libc de ese target.
                        runner=scripts/ci/run-target-binary.sh
                        bash "${'$'}runner" $target "${'$'}work/bin/cogh" --version
                        bash "${'$'}runner" $target "${'$'}work/bin/cognicode" --version
                    """.trimIndent())
                }
            }
        }

        // ------------------------------------------------------ release-contracts
        //
        // 2026-10-15 / B1 (ADR-TH-005). These suites are the reason a merge
        // used to build four release artifacts: they tar, generate and verify
        // release-profile bytes, or spawn them as two processes. They run here
        // now, against the artifacts this lane builds and packages, and the
        // merge lane no longer pays for binaries it does not read.
        //
        // MEDIDO 2026-10-18. "binaries it does not read" era la premisa, y era
        // falsa por uno: `b6_w1_mcp_robustness` leía release y se quedó aquí
        // porque el merge gate corre el fichero entero. No se notó porque el test
        // se saltaba en silencio. Ese caso se corrige dos entradas más abajo, con
        // su propia stage y su nombre.
        //
        // The group sits outside `targets.forEach` on purpose: the suites execute
        // host binaries, so they are evidence for the host target once, not for
        // every target. `releaseContractEnv` says so out loud for a foreign one
        // instead of executing a cross binary that cannot run.
        stage("release-contracts") {
            stage("prf-f6-w1-release-coherence") {
                sh(releasePaths + "\n" + releaseContractEnv(targets.first()) + "\n" + """
                    cargo test -p cognicode-cli --test prf_f6_w1_release_coherence --quiet
                """.trimIndent())
            }

            stage("prf-f6-w2-staging-contract") {
                sh(releasePaths + "\n" + releaseContractEnv(targets.first()) + "\n" + """
                    cargo test -p cognicode-cli --test prf_f6_w2_staging_contract --quiet
                """.trimIndent())
            }

            stage("published-skill-bundles-are-required") {
                sh(releasePaths + "\n" + releaseContractEnv(targets.first()) + "\n" + """
                    cargo test -p cognicode-cli --test published_skill_bundles_are_required --quiet
                """.trimIndent())
            }

            // MEDIDO 2026-10-18. `b6_w1_mcp_robustness` trae un test que
            // consume bytes EN RELEASE y que el merge gate corría, con la lane
            // sin construir release: el test pedía release y el gate no
            // construía release. Antes de endurecer su aserción imprimía `SKIP:`
            // y hacía `return`, así que el rojo no existía y la cobertura era
            // nominal — un gate que declara medir release sin medirlo.
            //
            // Su sitio es aquí, con los otros, por la razón de siempre: esta lane
            // ya construyó este binario en release (`build-cognicode-mcp-release`)
            // y `releaseContractEnv` ya lo verificó ejecutándolo. Correrlo aquí
            // no cuesta un build; correrlo en el merge gate costaba 269 s.
            //
            // La ruta se pasa explícita. `cargo metadata` resolvería el mismo
            // binario, pero el test entonces mide "lo que cargo dice" en vez de
            // "el artefacto que esta lane verificó", que es la diferencia entre
            // una prueba y una suposición.
            stage("core-b6-w1-release-stdio-smoke") {
                sh(releasePaths + "\n" + releaseContractEnv(targets.first()) + "\n" + """
                    set -eo pipefail
                    export COGNICODE_MCP_BIN_RELEASE="${'$'}TARGET_DIR/release/cognicode-mcp"
                    if [ ! -x "${'$'}COGNICODE_MCP_BIN_RELEASE" ]; then
                        echo "FAIL: ${'$'}COGNICODE_MCP_BIN_RELEASE is missing or not executable."
                        exit 1
                    fi
                    cargo test -p cognicode-core --test b6_w1_mcp_robustness --quiet \
                        -- release_binary_stdio_round_trip_smoke
                """.trimIndent())
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
                    set -eo pipefail
                    # MEDIDO 2026-10-03. Esto hacia `mkdir -p staging`, y la
                    # raiz de `staging/` no se limpiaba. La lane de v0.101.5
                    # llego aqui y se rompio en la stage siguiente:
                    #
                    #     ::error::unexpected file at staging root:
                    #     cogh-aarch64-unknown-linux-gnu.cdx.json
                    #
                    # Esos ficheros los habia dejado la corrida ANTERIOR, que si
                    # llego a `generate`: el aplanado copia de
                    # `staging/payloads-<plataforma>/` a la raiz, y la copia se
                    # queda. En la siguiente corrida el propio aplanado rechaza
                    # su salida de antes —que es exactamente lo que debe hacer
                    # con un fichero que no reconoce— y la lane cae.
                    #
                    # Es el mismo defecto que 32fb1ff6 arreglo un nivel mas
                    # abajo, y con el mismo motivo: un release re-ejecutado
                    # sobre un arbol sucio tiene que partir del estado que el
                    # produce, no del que le dejaron. Aqui se limpian los
                    # FICHEROS sueltos de la raiz, antes de preparar los
                    # bundles, que es lo primero que esta lane escribe en ella.
                    #
                    # Solo ficheros, y a profundidad 1, por una razon que no es
                    # cosmetica: los directorios `payloads-*` los acaba de
                    # producir `package-<target>` en ESTA corrida, unas stages
                    # antes. Borrarlos seria tirar el trabajo del build para
                    # arreglar un arbol sucio.
                    #
                    # MEDIDO: escribir el nombre de esa stage con un dollar
                    # delante rompe la compilacion. Un `#` abre un comentario
                    # de shell, pero el texto sigue siendo el contenido de un
                    # raw string de Kotlin, y para Kotlin un dollar abre una
                    # plantilla. En `stage("skill-bundles")` no hay `target` en
                    # ambito —esa stage esta fuera del `forEach` que lo
                    # define— asi que el comentario no es un comentario: es una
                    # referencia sin resolver. Hasta este parrafo lo decia.
                    find staging -maxdepth 1 -type f -delete 2>/dev/null || true
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
                    set -eo pipefail
                    # MEDIDO 2026-10-04. `release/` es la salida de esta stage y
                    # esta stage es su duena, y no lo era: se creaba con lo que
                    # hubiera. La lane v0.101.9, que corre en el worktree donde
                    # la v0.101.8 habia dejado su candidato, llego hasta aqui y
                    # murio en la stage siguiente:
                    #
                    #     Error: artifact `cogh-0.101.8-x86_64-unknown-linux-gnu.tar.gz`
                    #     declares version `0.101.8` but the release version is `0.101.9`
                    #
                    # El gate de `verify` hizo bien su trabajo —eso es un artefacto
                    # de la release anterior dentro del candidato, que es
                    # exactamente lo que R1 prohibe— pero la lane no deberia llegar
                    # a depender de que un gate posterior lo detecte.
                    #
                    # Es el mismo caso que la stage `package`, y que la limpieza
                    # de la raiz de `staging/`, un directorio mas alla: un
                    # `mkdir -p` sobre un directorio que ya existe es el mecanismo
                    # por el que un directorio de build hereda estado de una
                    # corrida anterior. El worktree que construye el candidato es
                    # de larga vida, y la lane esta escrita para poder repetirse
                    # en el.
                    #
                    # Y el mismo matiz de QW-04, por el mismo motivo: esa regla es
                    # sobre lo que pasa DESPUES de que las stages han decidido;
                    # esto es antes de decidir nada. Si no se puede establecer el
                    # estado de entrada, no hay estado conocido donde generar.
                    if [ -e release ]; then
                        if ! rm -rf -- release; then
                            echo "FAIL: could not clear the release directory"
                            echo "  It exists from a previous candidate, and this lane cannot"
                            echo "  promise that the candidate it produces is its own. Clear it"
                            echo "  by hand, or run in a worktree without a previous candidate."
                            exit 1
                        fi
                        echo "cleared release/ from a previous candidate"
                    fi

                    # `--tag` and `--source-commit` are not decoration:
                    # `source_commit` is a required field of the release
                    # inventory, and the inventory is what ties a candidate to
                    # the commit it was built from. A manifest that does not
                    # name that commit cannot be tied to one, which is the first
                    # link of the chain this lane exists to keep.
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
                    set -eo pipefail
                    # MEDIDO 2026-10-03. Esta stage llevaba `--version` y nada
                    # mas, y `verify` exige tambien `--tag`:
                    #
                    #     error: the following required arguments were not provided:
                    #       --tag <TAG>
                    #
                    # No se habia visto porque la stage no se habia ejecutado
                    # nunca: es la primera vez que la lane llegaba aqui. La misma
                    # invocacion en release.pipeline.kts si lo pasa, asi que el
                    # precedente estaba dentro del repo.
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging release --version "$version" --tag "$tag"
                """.trimIndent())
            }

            stage("install-smoke") {
                sh("""
                    set -eo pipefail
                    set -eo pipefail
                    $cd || exit 1
                    bash scripts/ci/release-install-smoke.sh "$version" release
                """.trimIndent())
            }

            stage("candidate-manifest") {
                sh("""
                    set -eo pipefail
                    set -eo pipefail
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

            // La mitad de GENERACION de provenance, que faltaba desde que
            // `actions/attest-build-provenance` dejo de existir con el runtime de
            // Actions. Va aqui y no en `release.pipeline.kts` porque la
            // atestation tiene que cubrir **el candidato que fue certificado**:
            // generarla en la lane de publicacion describiría un artefacto que
            // nadie certifico, que es la misma propiedad rota que el candidato
            // inmutable previene para los binarios.
            //
            // La condicion es "hay una clave con la que firmar", que es una
            // CAPACIDAD, no una politica. La politica —que una atestation
            // faltante sea fatal— sigue siendo de `verify-provenance.sh`, que
            // es el unico que lee `RELEASE_REQUIRE_PROVENANCE`. Poner el switch
            // aqui crearia la segunda fuente de verdad que
            // `test_provenance_gate.py` prohibe, y que ya costo una release
            // inalcanzable (v0.101.3).
            //
            // MEDIDO 2026-10-04, cosign 3.1.3: sin clave, `attest-provenance.sh`
            // falla y NO genera ninguna. Por eso la etapa se salta en silencio
            // en vez de llamar al script: llamar sin clave seria un fallo
            // deliberado, y el fallo deliberado de una etapa opcional es una
            // release rota. Lo que no es opcional es decirlo, asi que la etapa
            // anuncia por que no firmo.
            stage("provenance") {
                sh("""
                    set -eo pipefail
                    set -eo pipefail
                    $cd || exit 1
                    if [ -z "${'$'}{RELEASE_PROVENANCE_KEY:-}" ]; then
                        echo "provenance: NOT generated — no RELEASE_PROVENANCE_KEY on this host."
                        echo "  This is the open custody decision, not a defect: see"
                        echo "  docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md, 'Open decision'."
                        echo "  The release ships without attestation and says so here, which is"
                        echo "  what verify-provenance.sh reports downstream."
                        exit 0
                    fi
                    echo "provenance: signing with the key named by RELEASE_PROVENANCE_KEY"
                    scripts/ci/attest-provenance.sh \
                        --key "${'$'}{RELEASE_PROVENANCE_KEY}" \
                        --out-dir release \
                        release/*.tar.gz release/SHA256SUMS
                """.trimIndent())
            }
        }
    }
}

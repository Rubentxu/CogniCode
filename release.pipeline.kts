import java.io.File

// release — publish a candidate that has already been built and proved.
//
// The second half of the release factory. `release-candidate.pipeline.kts`
// writes `dist/` and `release/`; this lane publishes those files and nothing
// else. It cannot rebuild them, and that is the property the whole chain
// exists for:
//
//     build candidate -> immutable candidate -> validate -> publish
//
// with no rebuild in the middle. `release.yml` had one anyway, in the form of
// `cargo build --release --bin cognicode-release` appearing in all three of its
// jobs; here the tool is built once, by the candidate lane, and read.
//
// Order is not incidental. The draft is created, populated and confirmed
// before it is published, and the candidate is re-verified after the upload and
// again by re-downloading the published bytes and checking the sums from those
// bytes. A consumer gets the files that were validated, or the lane fails.
//
// THE NEGATIVE TESTS, AND WHY THEY RUN ON A COPY
// ----------------------------------------------
// `release-validate.yml` proved that `cognicode-release verify` rejects a
// missing archive and rejects an altered one. Those two jobs worked on a
// downloaded copy, which was the only way to damage a payload without
// damaging the release — each job got its own fresh runner.
//
// Here the stages share one filesystem, which is what makes the candidate a
// directory and what makes publishing it safe. It is also what would make a
// naive port of those two jobs destructive: `rm` on the real archive would
// delete the artefact this lane is about to publish. So both negative tests
// copy the candidate first, corrupt the copy, and assert the checker fails on
// the copy. If the copy step ever stops happening, the release stops being
// possible, which is the safe direction for that mistake to point.
//
// ONE CAPABILITY HAS NO EQUIVALENT HERE
// -------------------------------------
// `actions/attest-build-provenance` generates SLSA build provenance. It is an
// action, and PipelineK has no action runtime, so there is nothing to port it
// to. The consumer-facing half — the part that proves the published bytes are
// the attested ones — is kept, because `gh` is a tool this lane can call.
//
// The generation half is an open item, not a silent drop. `scripts/ci/
// verify-provenance.sh` holds the whole policy, and it is the only thing that
// reads `RELEASE_REQUIRE_PROVENANCE`: the artifacts are still checked on every
// run and the result is printed, but only an enforced run treats a missing
// attestation as fatal. Until a replacement generator is chosen, publishing
// runs without generated provenance and the lane says so out loud rather than
// implying otherwise.
//
// MEDIDO 2026-10-03. This used to be two stages. `attestations` checked the
// artifacts unconditionally and `provenance-required` implemented the switch
// above, so the header described the second and the code did the first. With
// nothing in this repository able to generate an attestation, the check
// returned non-zero for every candidate and the lane died before `publish`:
// not a release without provenance, an unreachable release. `scripts/ci/
// test_provenance_gate.py` now holds that state red.
//
// `gh` is not an orchestrator mechanism. GitHub Releases is where the product
// goes; PipelineK is what decides to call it.

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

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

val tag: String = System.getenv("RELEASE_TAG") ?: ""
val version: String = if (tag.isEmpty()) workspaceVersion(repoRoot) else tag.removePrefix("v")
val repo: String = System.getenv("GITHUB_REPOSITORY") ?: "Rubentxu/CogniCode"

// The release tool is built by the candidate lane and read here. Only the
// *directory* is resolved: `target/release/` is where cargo writes only when
// nothing else says otherwise, and on this repository it does not —
// `~/.cargo/config.toml` sets `build.target-dir` for the shared machine. Each
// command still names the tool, because the name is the gate and the directory
// is the machine's business. See `scripts/ci/target-dir.sh` and the longer note
// in `release-candidate.pipeline.kts`.
val releasePaths = """
    $cd || exit 1
    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
""".trimIndent()

pipeline {
    stages {
        // ------------------------------------------------------------- consume
        stage("consume-candidate") {
            stage("candidate-present") {
                sh("""
                    $cd || exit 1
                    for d in dist release; do
                        test -d "${'$'}d" || {
                            echo "this lane publishes a candidate; it does not build one."
                            echo "No ${'$'}d/ directory. Run release-candidate.pipeline.kts first,"
                            echo "or point this lane at the tree it wrote."
                            exit 1
                        }
                    done
                    archives=$(ls -1 release/*.tar.gz 2>/dev/null | wc -l)
                    test "${'$'}archives" -ge 1 || {
                        echo "release/ holds no archives. An empty candidate is not a candidate."
                        exit 1
                    }
                    echo "candidate: ${'$'}archives archive(s)"
                """.trimIndent())
            }

            stage("re-verify-candidate") {
                sh(releasePaths + "\n" + """
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging release --version "$version" --tag "$tag"
                """.trimIndent())
            }
        }

        // ------------------------------------------------------- negative tests
        // Both corrupt a copy. See the header: the stages share a filesystem,
        // and a corrupt-the-real-thing port would delete the release.
        stage("negative") {
            stage("verify-rejects-missing-artifact") {
                sh(releasePaths + "\n" + """
                    work=$(mktemp -d)
                    cp -r release "${'$'}work/release"
                    shopt -s nullglob
                    archives=("${'$'}work"/release/*.tar.gz)
                    if [ "${'$'}{#archives[@]}" -lt 2 ]; then
                        echo "need at least 2 archives to delete one and still exercise the verify path; got ${'$'}{#archives[@]}"
                        exit 1
                    fi
                    victim="${'$'}{archives[0]}"
                    echo "removing ${'$'}victim from the COPY to simulate a corrupted payload"
                    rm "${'$'}victim"
                    set +e
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging "${'$'}work/release" --version "$version" --tag "$tag"
                    rc=${'$'}?
                    set -e
                    if [ "${'$'}rc" -eq 0 ]; then
                        echo "release-verify returned 0 against a staging set missing an archive."
                        echo "The checker is a no-op and every other verdict in this lane is worthless."
                        exit 1
                    fi
                    echo "release-verify correctly rejected the missing artifact (rc=${'$'}rc)"
                """.trimIndent())
            }

            stage("verify-rejects-altered-artifact") {
                sh(releasePaths + "\n" + """
                    work=$(mktemp -d)
                    cp -r release "${'$'}work/release"
                    shopt -s nullglob
                    archives=("${'$'}work"/release/*.tar.gz)
                    victim="${'$'}{archives[0]}"
                    printf 'x' >> "${'$'}victim"
                    set +e
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging "${'$'}work/release" --version "$version" --tag "$tag"
                    rc=${'$'}?
                    set -e
                    if [ "${'$'}rc" -eq 0 ]; then
                        echo "release-verify returned 0 against an altered archive."
                        echo "SHA256SUMS is not being checked, so publication would ship"
                        echo "whatever bytes happened to be in the file."
                        exit 1
                    fi
                    echo "release-verify correctly rejected the altered artifact (rc=${'$'}rc)"
                """.trimIndent())
            }
        }

        // ---------------------------------------------------------------- draft
        stage("draft") {
            stage("create-draft") {
                sh("""
                    $cd || exit 1
                    if [ -z "${'$'}RELEASE_TAG" ]; then
                        echo "RELEASE_TAG is unset, so there is nothing to publish to."
                        echo "This lane publishes. It does not dry-run."
                        exit 1
                    fi
                    NOTES=$(bash scripts/generate-release-notes.sh "" "${'$'}RELEASE_TAG" || echo "CogniCode ${'$'}RELEASE_TAG")
                    # The tag already exists, so --target is not passed: that flag
                    # is for creating a tag that is missing, and passing it would
                    # move the release onto whatever the name resolves to now.
                    if gh release view "${'$'}RELEASE_TAG" >/dev/null 2>&1; then
                        echo "release ${'$'}RELEASE_TAG already exists; reusing it"
                    else
                        gh release create "${'$'}RELEASE_TAG" --draft --title "CogniCode ${'$'}RELEASE_TAG" --notes "${'$'}NOTES"
                    fi
                """.trimIndent())
            }

            stage("upload-payloads") {
                sh("""
                    $cd || exit 1
                    gh release upload "${'$'}RELEASE_TAG" release/*.tar.gz --clobber
                """.trimIndent())
            }

            stage("confirm-uploaded-set") {
                sh("""
                    $cd || exit 1
                    expected=$(cd release && ls *.tar.gz | sort)
                    actual=$(gh release view "${'$'}RELEASE_TAG" --json assets -q '.assets[].name' | sort)
                    if [ "${'$'}expected" != "${'$'}actual" ]; then
                        echo "the uploaded asset set does not match the produced payload set"
                        diff <(echo "${'$'}expected") <(echo "${'$'}actual") || true
                        exit 1
                    fi
                    echo "uploaded payload set confirmed ($(echo "${'$'}expected" | wc -l) artifacts)"
                """.trimIndent())
            }

            stage("upload-manifests") {
                sh("""
                    $cd || exit 1
                    gh release upload "${'$'}RELEASE_TAG" \
                        release/bundle-*.yaml release/release-inventory-*.json release/SHA256SUMS --clobber
                """.trimIndent())
            }
        }

        // ----------------------------------------------------------- provenance
        // One stage, one implementation. See `scripts/ci/verify-provenance.sh`
        // and the header: the policy — enforced only when
        // `RELEASE_REQUIRE_PROVENANCE=1` — lives in that script, and this
        // stage only names the artifacts it must judge.
        stage("provenance") {
            stage("attestations") {
                sh("""
                    $cd || exit 1
                    scripts/ci/verify-provenance.sh "${'$'}{GITHUB_REPOSITORY:-Rubentxu/CogniCode}" \
                        release/*.tar.gz release/SHA256SUMS
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------- publish
        stage("publish") {
            // The draft-first safety net: the candidate is re-verified after the
            // upload, so a payload that changed in transit fails here rather
            // than at a consumer's install.
            stage("re-verify-after-upload") {
                sh("""
                    $cd || exit 1
                    "${'$'}TARGET_DIR/release/cognicode-release" verify --staging release --version "$version" --tag "$tag"
                """.trimIndent())
            }

            stage("publish-draft") {
                sh("""
                    $cd || exit 1
                    gh release edit "${'$'}RELEASE_TAG" --draft=false
                """.trimIndent())
            }

            // MEDIDO 2026-10-03. Ningun stage de esta lane miraba `latest`, y
            // ese puntero es lo que resuelve `install.sh`:
            //
            //     curl -fsSL "$api/releases/latest" | sed -n 's/.*"tag_name"...'
            //
            // GitHub excluye drafts y prereleases de `/releases/latest`. Publicar
            // la release no garantiza por si solo que ese puntero se mueva: si
            // no se mueve, quedan los tres artefactos en su sitio —tag, release y
            // manifest dicen 0.101.2— y todo usuario sin pin sigue recibiendo la
            // release anterior. Es exactamente la incoherencia que R1 viene a
            // cerrar, y medida en la auditoria inicial: el tag era v0.100.0 y
            // `latest` servia v0.98.1.
            //
            // Se consulta el MISMO endpoint que install.sh, no `gh release view`,
            // porque `gh` puede razonar sobre un notion distinta de "latest" y
            // un gate que mide lo que el gate cree medir es peor que no medir.
            stage("verify-latest-resolution") {
                sh("""
                    $cd || exit 1
                    api="https://api.github.com/repos/${'$'}{GITHUB_REPOSITORY:-Rubentxu/CogniCode}"
                    resolved=$(curl -fsSL "${'$'}api/releases/latest" \
                        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
                        | head -n1)
                    if [ -z "${'$'}resolved" ]; then
                        echo "FAIL: /releases/latest no devolvio ningun tag; install.sh no podria instalar"
                        exit 1
                    fi
                    if [ "${'$'}resolved" != "${'$'}RELEASE_TAG" ]; then
                        echo "FAIL: la release publicada es ${'$'}RELEASE_TAG pero /releases/latest resuelve ${'$'}resolved"
                        echo "      install.sh serviria ${'$'}resolved a quien no fije version: la release"
                        echo "      existe y es invisible. Marcar ${'$'}RELEASE_TAG como prerelease, o"
                        echo "      revisar si hay una release mas reciente, lo devuelve a la cola."
                        exit 1
                    fi
                    echo "latest resuelto a ${'$'}resolved, el mismo tag recien publicado"
                """.trimIndent())
            }

            stage("verify-as-consumer") {
                sh("""
                    $cd || exit 1
                    draft=$(gh release view "${'$'}RELEASE_TAG" --json isDraft -q .isDraft)
                    if [ "${'$'}draft" != "false" ]; then
                        echo "release ${'$'}RELEASE_TAG is still a draft after publishing it"
                        exit 1
                    fi
                    gh release view "${'$'}RELEASE_TAG" --json assets -q '.assets[].name' | sort
                    # Re-download every asset and check the sums from the published
                    # bytes. Everything before this proves the local candidate;
                    # this proves the candidate a consumer would actually get.
                    work=$(mktemp -d)
                    cd "${'$'}work"
                    gh release download "${'$'}RELEASE_TAG"
                    sha256sum -c SHA256SUMS
                    echo "published release verified from re-downloaded bytes"
                """.trimIndent())
            }
        }
    }
}

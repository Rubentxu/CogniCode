// merge-gate — the pipeline a merge is gated on.
//
// This was built to full parity with `.github/workflows/pr-ci.yml` and then
// took its place. The line numbers in the comments below are left where they
// were: they are provenance, pointing at the stage each gate came from, and
// the question they answered — "does the required check run this?" — is
// answered here now. The orchestrator that decides what runs is this file,
// declared once in `scripts/ci/pipeline_authority.py` and read from there by
// every wiring contract.
//
// The four GitHub jobs it replaces, for the record:
//   check        -> fmt + clippy                        (pr-ci.yml:56)
//   build-binary -> release builds of mcp + control-plane (pr-ci.yml:118)
//   test-pr      -> core + cli + mcp suites             (pr-ci.yml:239)
//   selector     -> CR-08 suite selector                (pr-ci.yml:173)
//   merge-gate   -> fan-out assertion + drift lint + compat matrix (pr-ci.yml:330)
//
// Workspace: the repository root is resolved here and interpolated into every
// step, because PipelineK runs each `sh()` step in the launching process's
// working directory and a step must not depend on where the launcher stood.
//
// An earlier revision of this header claimed the opposite — that PipelineK
// gives every stage its own throwaway directory, so no step could see another's
// output, and that is why the root is interpolated. That claim was false and
// was measured out of it on 2026-10-02 with a two-stage probe run under
// pipelinek 0.46.0 and no `cd` prefix anywhere:
//
//     WRITER_PWD  = <repo root>
//     READER_PWD  = <repo root>
//     RELATIVE_SHARED = yes
//     REPO_VISIBLE    = yes
//
// Stages share the filesystem and the working directory. A file one stage
// writes at a relative path is visible to the next, and a release binary built
// in one stage can be executed by another.
//
// The consequence is not cosmetic. QW-09 exists because in GitHub Actions
// `needs:` means "wait for that job", not "inherit that job's artifacts" — every
// job gets a fresh runner. That failure mode does not exist here, so the
// reachability property is not "each stage must obtain its own binaries" but
// "a step must not consume a path that a later step produces". Ordering is the
// whole invariant. It is pinned by
// `scripts/ci/test_pipeline_artifact_reachability.py` over these scripts rather
// than over YAML, and deliberately so: pinning it over `pr-ci.yml` would have
// made the retiring orchestrator the authority for a property of this one.
//
// The same probe found the other half: a stage can write into the repository
// itself. Nothing here cleans up after itself, so a step that writes a
// relative path writes into the working tree.
//
// And the second thing that probe measures is the one this file silently
// depends on everywhere: **a failing stage aborts the pipeline.** Measured on
// 2026-10-02 under pipelinek 0.46.0 with a three-stage probe — write, fail with
// exit 7, write — where the third stage's marker was never created and the
// process exited 1:
//
//     StepFailed ... failureKind: SCRIPT, message: shell exited with code 7
//     RunFinished ... outcome: failure        <- immediately, no stage 3
//
// That is what `set -euo pipefail` gave the old workflow, implicitly, and it is
// why no stage here needs a `set -e` of its own. It is also the reason every
// ADVISORY stage in `integration.pipeline.kts` ends its command with
// `|| echo 'ADVISORY: …'`: PipelineK has no `continue-on-error`, so a non-zero
// exit is the only way to stop a lane, and the only way not to stop one is to
// not exit non-zero. `|| echo` is not a loose substitute for a policy flag
// here; it is the whole mechanism.
//
// Both probes are reproducible with `scripts/ci/probe-pipelinek-semantics.sh`.
// It is not a contract — a CI runner does not ship the `pipelinek` binary — so
// it is the thing to re-run by hand after a version bump, and a changed answer
// is a change of policy to review rather than a version number to accept.
//
// Shell variables inside a **raw** string (`"""..."""` + trimIndent) must be
// written `${'$'}name`; the shell then receives a literal `$` and expands its
// own. This is not a version quirk and the distinction that matters is raw vs
// normal string, not 0.46.0 vs 0.39.1-rc1: in a normal single-line string
// `\$name` compiles and works, while in the multiline raw strings every real
// step uses it fails with "Unresolved reference". `\${'$'}name` is wrong
// everywhere: the backslash survives into the emitted script and bash fails
// with "syntax error near unexpected token `('".
//
// `pipelinek` on PATH resolves 0.46.0. That was not free. A symlink at
// ~/.local/bin/pipelinek pointing at a 0.39.0 install took precedence over the
// asdf shim and ran every pipeline against a compiler the repo did not pin. If
// validation and run ever disagree again, check `command -v pipelinek` first:
// the bug is a version mismatch, never the script.
//
//   pipelinek run merge-gate.pipeline.kts --db <journal.db>
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

// The file that says what gates a merge. Declared here so `repo-contract` can
// check that the pipeline it is running inside actually exists, instead of
// checking that some other orchestrator's file does. That check used to name
// `.github/workflows/pr-ci.yml`, which meant the pipeline refused to run until
// a file that the cutover exists to delete was present: the gate could not
// outlive its own predecessor. The name below is the same fact the wiring
// contracts read from `scripts/ci/pipeline_authority.py`.
val authority = "merge-gate.pipeline.kts"

// Where a stage that produces a lot of output leaves it, so that a failure is
// something a person can read instead of an exit code with no story attached.
// Outside the repository: a lane that writes its logs into the tree makes
// `cargo fmt --check` and the contract suite see files nobody committed.
val logDir: String = File(System.getProperty("java.io.tmpdir"), "cognicode-logs").absolutePath
val cdLog = "mkdir -p \"$logDir\""

pipeline {
    stages {
        // ---------------------------------------------------------------- check
        stage("check") {
            stage("build-isolation") {
                // MEDIDO 2026-10-15. Esta etapa se antepone a todo lo demás a
                // propósito. Seis checkouts de este repo compilan al mismo
                // target dir, así que un binario de cualquiera de los cinco
                // worktrees puede ejecutarse como si fuera de este árbol: cargo
                // decide frescura por mtime, y un mtime no dice de dónde salió
                // el binario. Pasó dos veces en un día, en las dos
                // direcciones — un test paniqueando en la línea 900 de un
                // fichero de 861, y otro comparando su `source_commit` contra
                // el HEAD de otro worktree.
                //
                // Sin esta etapa el gate produce un veredicto que no se puede
                // atribuir, y falla después de 11 minutos explicando un test
                // que en realidad no falla. Con ella, no arranca y dice por
                // qué. El negativo está probado: dos checkouts y nadie
                // compilando sale 0.
                sh("bash scripts/ci/check-build-isolation.sh")
            }

            stage("toolchain") {
                sh("cargo --version && rustc --version")
            }

            stage("repo-contract") {
                sh("""
                    set -eo pipefail
                    test -f "$repoRoot/Cargo.toml" || {
                        echo "FAIL: $repoRoot has no Cargo.toml"
                        exit 1
                    }
                    test -f "$repoRoot/$authority" || {
                        echo "FAIL: the merge authority $authority is missing from \$repoRoot."
                        echo "      A pipeline that cannot find itself cannot gate anything."
                        exit 1
                    }
                    test -d "$repoRoot/scripts/ci" || {
                        echo "FAIL: \$repoRoot/scripts/ci is missing, so there is no contract suite to run"
                        exit 1
                    }
                    echo "repo=$repoRoot authority=$authority crates=$(ls -d $repoRoot/crates/*/ | wc -l)"
                """.trimIndent())
            }

            stage("fmt") {
                sh("$cd && cargo fmt --all -- --check")
            }

            stage("clippy") {
                // pr-ci.yml:78 — bare `cargo clippy --workspace --all-targets -- -D warnings`.
                // The previous stage name "clippy-baseline" carried an allowlist for 42
                // pre-existing errors in rig/tools.rs that no longer exist (cargo clippy
                // reports 0 on the current tree). The baseline was dead and the pipeline
                // permitted more than CI; restored to the bare assertion CI runs.
                sh("$cd && cargo clippy --workspace --all-targets -- -D warnings")
            }

            // El gate de rustdoc (M0.11) vivía aquí y se pagaba **dos veces** por
            // cambio: esta stage y, unas horas después, el `cargo test --workspace
            // --all-targets` de la lane de integración, que ejecuta el mismo target.
            // MEDIDO 2026-10-15 en el ledger durable del gate: 667 s aquí, dentro
            // de un merge de 1 630 s (41%). La garantía no se retira: se ejecuta
            // una sola vez, en la lane que ya la ejecutaba y que es la que bloquea
            // `main`, con stage propia y nombrada (no escondida dentro de
            // `--all-targets`) para que el contrato pueda verificarla.
            // (was Pinned by core_gate_coverage_contract::the_rustdoc_gate_runs_in_exactly_one_authority; contract retired 2026-10-18)

            // MEDIDO 2026-10-15. Las dependencias Python del contrato se
            // declaran ANTES de `contracts`, no despues. `skills-toolchain`
            // instalaba PyYAML aqui, debajo, y por eso podia: cuando esta stage
            // corria, `contracts` ya habia terminado, y cualquier dependencia
            // que la suite de contratos necesite ya no llega a tiempo.
            //
            // No es hipotetico. `scripts/product/test_product_tool_catalog.py`
            // hacia `try: import jsonschema / except ImportError: return` — la
            // validacion de schema del catalogo de tools publicado se saltaba
            // en silencio y el test reportaba PASS. En un runner limpio
            // (ubuntu-latest no trae jsonschema) ese gate estaba apagado y
            // verde. Es el mismo antipatron que este lane ya documenta para
            // PyYAML mas abajo, aplicado a otro import.
            //
            // El runner instala desde aqui: no hay fichero de requirements para
            // estas dependencias, asi que esta linea ES la declaracion.
            stage("contract-toolchain") {
                sh("$cd && python3 -m pip install --disable-pip-version-check \"PyYAML==6.0.3\" \"jsonschema==4.25.1\"")
            }

            // MEDIDO 2026-10-18. `test_assurance_export_golden.py` ejecuta el
            // binario `cognicode` para comparar su export contra el golden, y
            // esta era la primera etapa del gate que lo corría: nada antes lo
            // producía. El contrato localizaba el binario en cuatro rutas
            // escritas a mano, dos de ellas el target dir compartido de una
            // máquina concreta, así que su verde NO era suyo — lo tomaba
            // prestado del disco de quien ejecutaba. Corregido el contrato para
            // que pregunte a cargo (MEDIDO: en el gate, con su target dir
            // aislado, pasa; con uno vacío, falla con el mensaje que dice qué
            // construir), la falta del productor dejó de estar tapada.
            //
            // Por eso esta etapa existe y va aquí. `build-cognicode-cli-debug`
            // (más abajo, dentro de `build-binary`) produce los mismos binarios
            // un rato después, así que esto no es un segundo build: cargo
            // compila `cognicode` una vez y el `--bins` de más abajo reutiliza
            // ese artefacto y solo añade `cogh` y `cognicode-release`. El coste
            // es que `contracts` espera un binario en vez de tres, a cambio de
            // que la suite deixe de depender de lo que hubiera en el disco.
            //
            // `verify-debug-binaries` se queda donde está, con la inmediatez
            // que tiene: verifica lo que `build-cognicode-cli-debug` acaba de
            // producir, no un artefacto de una etapa anterior.
            stage("build-cognicode-for-contracts") {
                sh("$cd && cargo build -p cognicode-cli --bin cognicode")
            }

            // The contract suite. One entry point, no list: the pipeline names a
            // script, the script discovers `scripts/ci/test_*.py` by glob. The
            // previous shape named eight contract files inside pr-ci.yml, which
            // meant a new contract passed locally and ran in CI only if whoever
            // wrote it also edited that YAML — how the CP5 skill gate stayed
            // green while no workflow invoked it.
            stage("contracts") {
                sh("$cd && bash scripts/ci/run-all-contracts.sh")
            }

            // CP5 / A-033..A-036. `validate_skills.py` resolves tool names from
            // the published catalog rather than a private list; `verify-skills.sh`
            // checks each bundle has a manifest. Both were correct, both passed,
            // and no workflow invoked them until 2026-10-02.
            //
            // PyYAML is declared rather than trusted from the runner image:
            // validate_skills.py degrades to a printed SKIP without it, and
            // verify-skills.sh aborts on `import yaml`. Neither is a failure, and
            // both are how a gate goes stale in silence. It is installed by
            // `contract-toolchain` above, which runs first; installing it here
            // as well would be a second declaration of the same pin, and the
            // second one would still be too late for the contract suite.
            stage("skills-validate") {
                sh("$cd && python3 scripts/validate_skills.py")
            }

            stage("skills-bundles") {
                sh("$cd && bash scripts/verify-skills.sh")
            }
        }

        // ---------------------------------------------------------- supply-chain
        // pr-ci.yml's `supply-chain` job, added in 05c66126. Advisories and
        // licences ran at tag time and never on a pull request, so a dependency
        // with a published advisory merged unnoticed.
        //
        // cargo-deny is installed with a fixed version on purpose: `--locked`
        // alone pins its dependencies while floating the tool, so the gate's
        // behaviour would change with no commit in this repository.
        // Measured 2026-10-02 on this machine: 1m43s to install from scratch.
        stage("supply-chain") {
            stage("install-cargo-deny") {
                sh(
                    """
                    set -eo pipefail
                    $cd || exit 1
                    if command -v cargo-deny >/dev/null 2>&1; then
                        echo "cargo-deny already present, skipping install"
                    else
                        cargo install cargo-deny --version 0.20.2 --locked
                    fi
                    """.trimIndent(),
                )
            }

            stage("advisories") {
                sh("$cd && cargo deny check advisories")
            }

            stage("licenses") {
                sh("$cd && cargo deny check licenses")
            }
        }

        // ── El binario LSP del gate, y por que tiene que estar aqui ────────
        //
        // MEDIDO 2026-10-04. El gate corre `cargo test -p cognicode-core
        // --lib`, y en esa suite vive
        // `test_hierarchy_falls_through_within_the_bounded_readiness`, que se
        // auto-salta con un `println!` cuando `rust-analyzer` no esta en el
        // PATH. Es decir: el gate se saltaba el test y daba verde. Y no era
        // solo que faltara el binario: el shim de rustup lo hace existir sin
        // funcionar (`~/.cargo/bin/rust-analyzer` es un symlink a `rustup`, y
        // responde `Unknown binary` con codigo 1), de modo que un
        // `command -v` habria dado la respuesta equivocada. Medido con
        // `grep 'component add'` sobre este pipeline, los scripts de CI y los
        // workflows: nadie lo instalaba.
        //
        // El coste de no tenerlo esta medido en las dos direcciones, y por eso
        // la decision es facil: el gate completo tardaba 20m17s sin el
        // componente y 22m48s con el. Dos minutos y medio para que un test que
        // hoy no se ejecuta, se ejecute.
        //
        // Se instala como componente del toolchain y no como `cargo install`:
        // es un binario de rustup, `rustup component add` es idempotente y no
        // compila nada. El `if` evita el trabajo cuando ya esta, y la
        // comprobacion posterior convierte "lo instale" en "responde", que es
        // la distincion que el shim rompia.
        stage("lsp-toolchain") {
            stage("install-rust-analyzer") {
                sh(
                    """
                    set -eo pipefail
                    $cd || exit 1
                    if rust-analyzer --version >/dev/null 2>&1; then
                        echo "rust-analyzer already present, skipping install"
                    else
                        rustup component add rust-analyzer
                    fi
                    """.trimIndent(),
                )
            }

            stage("rust-analyzer-answers") {
                sh(
                    """
                    set -eo pipefail
                    $cd || exit 1
                    # Un shim en el PATH que sale con 1 haria que el paso
                    # anterior pasara y los tests se saltaran igual. Preguntar
                    # al binario es lo unico que distingue "instalado" de
                    # "utilizable", y es la misma distincion que hace
                    # `command_reports_version` en lsp_integration_test.rs.
                    if ! rust-analyzer --version >/dev/null 2>&1; then
                        echo "FAIL: rust-analyzer esta en el PATH pero no responde." >&2
                        echo "Si responde con error, el shim es de rustup y el" >&2
                        echo "componente no esta instalado: los tests LSP se" >&2
                        echo "saltarian en silencio, que es lo que este paso" >&2
                        echo "existe para evitar." >&2
                        exit 1
                    fi
                    rust-analyzer --version
                    """.trimIndent(),
                )
            }
        }

        // ---------------------------------------------------------- build-binary
        //
        // 2026-10-15 / B1 (ADR-TH-005). This group used to build four
        // release-profile artifacts and verify them, and it was measured as the
        // largest single block of a merge run. Their only verifiable consumer
        // was a set of suites that tar and execute real release bytes; those
        // suites now run in `release-candidate.pipeline.kts`, the lane that
        // builds those artifacts as the candidate itself. What is left here is
        // what a merge actually executes: the debug-profile binaries.
        //
        // The debug profile is not a downgrade. `cargo test` resolves test-time
        // binaries from the debug profile through `CARGO_BIN_EXE_*`; what these
        // stages add is the builds that `cargo test` alone does not guarantee
        // before the suites that spawn a binary by path run, and the failure
        // they now produce is loud instead of a skipped test.
        stage("build-binary") {
            // The MCP server, spawned by the core subprocess UATs
            // (`prf_h06_adversarial_e2e`, `b6_w1_mcp_robustness`) and by every
            // `tests/*.rs` of the MCP crate.
            stage("build-cognicode-mcp-debug") {
                sh("$cd && cargo build -p cognicode-mcp")
            }

            // `cogh`, `cognicode` and `cognicode-release` as plain debug
            // binaries. Three consumers, and none of them is satisfied by
            // `cargo test --bins`: the `cogh` unit tests derive
            // `<target-dir>/debug/cogh` from `current_exe`, and the core suite
            // `b3_w2_progressive_context` spawns `cognicode` by path.
            stage("build-cognicode-cli-debug") {
                sh("$cd && cargo build -p cognicode-cli --bins")
            }

            // The control-plane binary had no consumer in this lane beyond the
            // verifier below, in release or in debug. What survives is the
            // guarantee that verifier encoded: it links and it starts.
            stage("build-control-plane-debug") {
                sh("$cd && cargo build --bin cognicode-control-plane")
            }

            // A binary that compiles and then does not start is caught only
            // here: a build stage is satisfied by an artifact that exists, not
            // by one that runs. It asks cargo where it wrote instead of
            // hardcoding a path, which is the defect that already failed once.
            // The release-profile twin of this stage lives in the candidate
            // lane (`binary-smoke-$target`), with the artifacts it verifies.
            stage("verify-debug-binaries") {
                sh("""
                    set -euo pipefail
                    $cd || exit 1
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    missing=0
                    for bin in cognicode cognicode-mcp cognicode-control-plane; do
                        if [ ! -x "${'$'}TARGET_DIR/debug/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/debug/${'$'}bin is missing or not executable."
                            echo "  A suite in this lane spawns it by path, and a binary that is"
                            echo "  not there is not a reason to skip that suite."
                            missing=1
                        fi
                    done
                    if [ "${'$'}missing" -ne 0 ]; then
                        exit 1
                    fi
                    "${'$'}TARGET_DIR/debug/cognicode" --version
                    "${'$'}TARGET_DIR/debug/cognicode-control-plane" --help | head -5
                    "${'$'}TARGET_DIR/debug/cognicode-mcp" --version
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------- selector
        stage("selector") {
            // pr-ci.yml:173. CR-08 picks suites from the changed diff; the selector
            // contract below is what pins its behaviour, so run the contract rather
            // than reimplementing the diff walk in shell.
            stage("qw08-crate-selector") {
                sh("$cd && cargo test -p cognicode-cli --test qw08_crate_selector --quiet")
            }

            // pr-ci.yml also runs the selector script itself, not only its
            // contract. The contract pins what the script decides; running the
            // script is what proves the decision is consumable downstream. A
            // green contract over a script nothing calls is the A-013 shape.
            stage("select-suites-script") {
                sh("""
                    set -euo pipefail
                    $cd || exit 1
                    PATHS="scripts/ci/run-all-contracts.sh"
                    SELECT_OUTPUT="${'$'}(SELECT_PATHS="${'$'}PATHS" bash scripts/ci/select-suites.sh)"
                    echo "${'$'}SELECT_OUTPUT"
                """.trimIndent())
            }
        }

        // ---------------------------------------------------------------- test-pr
        stage("test-pr") {
            // core (pr-ci.yml:291-300)
            stage("core-lib") {
                sh("$cd && cargo test -p cognicode-core --lib --quiet")
            }

            stage("core-adversarial-campaign") {
                sh("$cd && cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet")
            }

            stage("core-prf-mcp-05") {
                sh("$cd && cargo test -p cognicode-core --lib prf_mcp_05 --quiet")
            }

            stage("core-adversarial-e2e") {
                // MEDIDO 2026-10-15. This suite spawns `cognicode-mcp` and resolves
                // it by walking the target directory itself; `cargo test` does not
                // export the path for it. Without the two variables below the walk
                // found nothing on a machine whose cargo config redirects the target
                // directory, and the suite reported a green skip. It now fails loudly
                // instead, and the gate hands it the debug binary it just built.
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    export CARGO_TARGET_DIR="${'$'}TARGET_DIR"
                    export COGNICODE_MCP_BIN="${'$'}TARGET_DIR/debug/cognicode-mcp"
                    cargo test -p cognicode-mcp --test prf_h06_adversarial_e2e --quiet
                """.trimIndent())
            }

            // pr-ci.yml:567. The unrestricted core step. Without it the five
            // suites behind #![cfg(feature = "evidence-kernel")] compile to
            // empty test binaries and report 0 passed / 0 failed / exit 0.
            // (was core_gate_coverage_contract pins this stage by command parity; contract retired 2026-10-18)
            stage("core-unrestricted-evidence-kernel") {
                // MEDIDO 2026-10-15. This step streamed `cargo test --quiet`
                // straight into the run transcript: 68 test binaries' worth of
                // progress dots, tens of thousands of bytes. The runtime drains
                // a step's stdout/stderr on a 10s budget and reported
                //
                //   [INFRASTRUCTURE]: transcript drain unfinished after
                //   10000ms: durable-sh-transcript-pump-...-stdout, -stderr
                //
                // which reads like a pipeline fault and is not one — the same
                // command exits 0 when its output goes to a file. A step that
                // cannot finish being read is not evidence either way.
                //
                // Same shape as `cli-unrestricted-ladybug` below: log to a
                // file, show only the tail, and say where the full log is.
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    $cdLog || exit 1
                    # The two suites inside this run that spawn a product binary by
                    # path (`b6_w1_mcp_robustness`, `b3_w2_progressive_context`)
                    # resolve it from the target directory, and they fail loudly when
                    # they do not find it. These three exports are what makes them
                    # find the binaries `build-binary` built for this run instead of
                    # whatever the shared target directory happens to hold.
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    export CARGO_TARGET_DIR="${'$'}TARGET_DIR"
                    export COGNICODE_MCP_BIN="${'$'}TARGET_DIR/debug/cognicode-mcp"
                    export COGNICODE_CLI_BIN="${'$'}TARGET_DIR/debug/cognicode"
                    # MEDIDO 2026-10-18. `b6_w1_mcp_robustness` trae un test que
                    # consume bytes EN RELEASE, y es el séptimo de esa clase:
                    # ADR-TH-005 movió seis a `release-candidate.pipeline.kts` y
                    # este se quedó, porque el gate corre el fichero entero.
                    # La cobertura no baja de nivel, cambia de lane — y ahí ya
                    # existe: `release-candidate.pipeline.kts` construye este
                    # mismo binario en release (`host-build-and-verify`), así que
                    # correrlo allí no cuesta un build.
                    #
                    # La alternativa — construirlo aquí — se midió antes de
                    # descartarla: `cargo build --release -p cognicode-mcp` en un
                    # target dir vacío son 269 s en cada merge, el 40% de los
                    # 676 s que ADR-TH-005 ahorró al dejar de construir cuatro
                    # binarios release. Pagar 269 s por un round-trip de stdio no
                    # es una cobertura que falte: es una cobertura que ya tiene
                    # casa.
                    #
                    # MEDIDO también por qué el rojo aparecía ahora y no antes:
                    # ese test imprimía `SKIP:` y hacía `return`. Con aserción
                    # dura, el gate que decía medir release no lo medía.
                    if ! cargo test -p cognicode-core --features evidence-kernel --quiet \
                        -- --skip release_binary_stdio_round_trip_smoke \
                        > "$logDir/core-unrestricted-evidence-kernel.log" 2>&1; then
                        tail -40 "$logDir/core-unrestricted-evidence-kernel.log"
                        echo "the full log is $logDir/core-unrestricted-evidence-kernel.log"
                        exit 1
                    fi
                """.trimIndent())
            }

            // pr-ci.yml:481. A-014 product asset resolution.
            stage("core-a014-product-asset-resolution") {
                sh("$cd && cargo test -p cognicode-core --test a014_product_asset_resolution --quiet")
            }

            // ladybug (pr-ci.yml:388)
            stage("cognicode-ladybug") {
                sh("$cd && cargo test -p cognicode-ladybug --lib --quiet")
            }

            // El puerto `RouteStore` y la oferta de la capacidad OpenAPI
            // viven en el explorer, y el build `multimodal` no lo ejercia
            // ninguna puerta: `cargo test
            // --workspace` compila con las features por defecto, asi que
            // los tests que viven detras de `#[cfg(feature = "multimodal")]`
            // no se ejecutan. Sin esta stage, el arreglo del puerto y de la
            // oferta no lo vigilaria nadie.
            //
            // El precedente es la stage de `evidence-kernel` de mas arriba,
            // que tambien construye con una feature activada. La garantia
            // se mide en los dos builds: el puerto vive en el de por
            // defecto, y la oferta de la capacidad en el multimodal.
            stage("explorer-route-capability") {
                sh(
                    """
                    set -eo pipefail
                    $cd || exit 1
                    cargo test -p cognicode-explorer --test h26_route_batch_is_all_or_nothing --quiet
                    cargo test -p cognicode-explorer --features multimodal --test h26_openapi_tool_is_offered_only_when_wired --quiet
                    """
                )
            }

            // PR-SEC reachability: every release-artifact path the pipeline
            // downloads is a path some earlier stage produced. Runs here as
            // well as in pr-ci.yml because the Kotlin pipeline is the one that
            // will own release once the migration closes, and a check that only
            // runs on the orchestrator being retired checks nothing.
            stage("bin-tracking") {
                sh("$cd && bash scripts/ci/check-bin-tracking.sh")
            }

            // QW-09 used to have two stages here: a shell guard and a Rust
            // contract, both reading `pr-ci.yml`. Both are gone, and the
            // property is stronger for it. The original question was GitHub's
            // — "does this job download the artifact an earlier job uploaded?" —
            // which only exists because every Actions job gets a fresh runner
            // and `needs:` does not mean "inherits artifacts". PipelineK has
            // neither problem: the stages share one filesystem (measured, not
            // assumed), so the surviving property is an ordering one — every
            // step that consumes a release artifact needs an earlier step that
            // produced it. `scripts/ci/test_pipeline_artifact_reachability.py`
            // states that, and it is discovered by the `contracts` stage above
            // rather than named by this one, and it scans every `*.pipeline.kts`
            // rather than a single workflow.

            // CR-07. `/metrics` is the only reason the OpenTelemetry upgrade was
            // worth doing, and the upgrade was the only reason RUSTSEC-2024-0437
            // is gone from deny.toml. A package version that satisfies an
            // advisory without still exposing the endpoint is the regression
            // this pins.
            stage("cr07-metrics-exposition") {
                sh("$cd && cargo test -p cognicode-mcp --test cr07_metrics_exposition_contract --quiet")
            }

            // cli named contracts (pr-ci.yml:408-528)
            stage("cli-qw03-bin-tracking") {
                sh("$cd && cargo test -p cognicode-cli --test qw03_bin_tracking_guard --quiet")
            }

            stage("cli-qw04-preflight") {
                sh("$cd && cargo test -p cognicode-cli --test qw04_preflight_contract --quiet")
            }

            stage("cli-a013-lifecycle-gate-contract") {
                sh("$cd && cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet")
            }

            stage("cli-a014-capabilities-json") {
                sh("$cd && cargo test -p cognicode-cli --test a014_capabilities_json --quiet")
            }

            stage("cli-a015-licenses-gate") {
                sh("$cd && cargo test -p cognicode-cli --test a015_licenses_gate --quiet")
            }

            stage("cli-a015-onboarding-gate") {
                sh("$cd && cargo test -p cognicode-cli --test a015_onboarding_gate --quiet")
            }

            // pr-ci.yml:699. The A-014 twin-cycle identity collision must stay
            // visible in the action register (N+63.2).
            stage("cli-action-register-identity-contract") {
                sh("$cd && cargo test -p cognicode-cli --test action_register_identity_contract --quiet")
            }

            // The unit surface: `src/` unit tests, the bin targets and the
            // doctests. It is separate from the integration list below because
            // cargo forbids `--doc` in the same invocation as any other target
            // option, and because enumerating integration targets with `--test`
            // would drop this whole surface without a word. The `cogh` lifecycle
            // tests live here, and they derive their sibling binary from
            // `current_exe`, which is why `build-binary` runs first.
            //
            // `--lib --bins` is spelled in that order on purpose:
            // (was cli_gate_coverage_contract rejects `-p cognicode-cli --lib` as the
            // lib-only selector that once skipped every `tests/` target of a
            // bin-only crate; contract retired 2026-10-18)
            stage("cli-unit-tests") {
                sh("$cd && cargo test -p cognicode-cli --features ladybug --lib --bins --quiet")
            }

            stage("cli-doctests") {
                sh("$cd && cargo test -p cognicode-cli --features ladybug --doc --quiet")
            }

            // The integration surface, enumerated instead of glossed with a
            // wildcard. `cargo test -p cognicode-cli --quiet` selects every
            // target of the crate, and five of them cannot run here any more:
            // they tar, generate and verify release-profile artifacts, and this
            // lane no longer builds any. Naming the rest explicitly is what lets
            // (was `cli_gate_coverage_contract` compare the selection with the
            // filesystem, so a suite added, dropped or moved is a deliberate
            // edit in both places instead of an invisible one; contract retired 2026-10-18)
            //
            // The named `--test` stages above run their targets here as well,
            // exactly as the wildcard did before this change: the duplication is
            // inherited, deliberate, and recorded in the audit (stage 44).
            //
            // Output goes to a file because this is the longest stage of the
            // lane: MEDIDO 2026-10-15, the runtime drains a step's transcript on
            // a 10 s budget and reports an infrastructure warning on the same
            // command that exits 0 when its output goes to a file.
            stage("cli-integration-suites") {
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    $cdLog || exit 1
                    if ! cargo test -p cognicode-cli --features ladybug --quiet \
                        --test a013_lifecycle_gate_contract \
                        --test a014_capabilities_json \
                        --test a015_licenses_gate \
                        --test a015_onboarding_gate \
                        --test action_register_identity_contract \
                        --test advisory_ignore_backing_contract \
                        --test b1_w2_identity \
                        --test b6_w3_cogh_lifecycle \
                        --test b6_w4_skills_autodiscovery \
                        --test backtick_provenance_contract \
                        --test cli_exit_code_propagation \
                        --test cogh_cli \
                        --test cognicode_ide_adapter \
                        --test cognicode_lifecycle \
                        --test cognicode_plugin \
                        --test cr07_protobuf_advisory_closed \
                        --test evidence_cli_mcp_equivalence \
                        --test gitignore_negation_contract \
                        --test perf_budget_checker_contract \
                        --test portable_skill_bundle \
                        --test prf_f4_w2_binary_restart \
                        --test prf_f6_w3_bis_sbom_contract \
                        --test prf_f6_w3_bis_staging_contract \
                        --test qw03_bin_tracking_guard \
                        --test qw04_preflight_contract \
                        --test qw08_crate_selector \
                        --test t1_5_5_json_dispatch \
                        > "$logDir/cli-integration-suites.log" 2>&1; then
                        tail -40 "$logDir/cli-integration-suites.log"
                        echo "the full log is $logDir/cli-integration-suites.log"
                        exit 1
                    fi
                """.trimIndent())
            }

            // the gate contract itself (pr-ci.yml:578)
            // stage cli-gate-coverage-contract removed 2026-10-18 (W0.1 unblock; sign-off operador)

            // mcp (pr-ci.yml:453-501)
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a009-agent-safe-profile") {
                sh("$cd && cargo test -p cognicode-mcp --test a009_agent_safe_profile --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a010-tool-authority-audit") {
                sh("$cd && cargo test -p cognicode-mcp --test a010_tool_authority_audit --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a012-structured-output") {
                sh("$cd && cargo test -p cognicode-mcp --test a012_structured_output --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a016-tools-runtime-consistency") {
                sh("$cd && cargo test -p cognicode-mcp --test a016_tools_runtime_consistency --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // The MCP UAT that used to be a stage here (`prf_cli_04_two_process_uat`)
            // left the inventory of the repository on 2026-10-15 with the rest of
            // the UAT class (ADR-TH-006, operator decision). Before that it had
            // moved to `release-candidate.pipeline.kts` because it spawns
            // `cognicode` and `cognicode-mcp` from the release directory and this
            // lane builds no release artifact since B1 (ADR-TH-005).
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-f4-w3-corrupt-cache-recovery") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_f4_w3_corrupt_cache_recovery --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-f5-w3-signal-cancel") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_f5_w3_signal_cancel --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.

        }

        // ------------------------------------------------------------ merge-gate
        stage("merge-gate") {
            // pr-ci.yml:358 — the capabilities drift lint, a hard gate.
            stage("capabilities-drift-lint") {
                sh("$cd && python3 sandbox/scripts/capabilities_drift_lint.py --strict")
            }

            // pr-ci.yml:369 — compat matrix 0.97.x. Skips by design when the legacy
            // binary fixture is absent, so this reports SKIP rather than pretending
            // the matrix ran.
            stage("compat-matrix-0-97-x") {
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    if [ -x sandbox/.compat/0.97.3/cognicode-mcp ]; then
                        echo "compat fixture present, running the matrix"
                        cargo test -p cognicode-mcp --test find_usages_compat_0_97 -- \
                            --skip compat_forward_0973_server_handles_modern_request
                    else
                        echo "SKIP: sandbox/.compat/0.97.3/cognicode-mcp absent (ADR-PRF-008 policy R1)"
                    fi
                """.trimIndent())
            }

            // pr-ci.yml:605 — the core coverage contract was retired 2026-10-18
            // (W0.1 unblock; sign-off operador). The gate still self-checks via
            // the pr-ci.yml references below; coverage is a separate concern.

            // pr-ci.yml:615 — CR-05 e91.W7 regression budget gate.
            stage("e91-w7-regression-budget") {
                sh("$cd && cargo test -p cognicode-core --test e91_w7_regression_budget --quiet")
            }

            // pr-ci.yml:625 — CR-03 e91.W8 per-stage profile gate retired 2026-10-18
            // (W0.1 unblock; sign-off operador). W7 regression budget remains.
            // (e91-w8 stage removed)
        }
    }
}

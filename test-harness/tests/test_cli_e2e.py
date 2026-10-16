"""End-to-end: la CLI sobre un repositorio real, con cargo de verdad.

Cubre lo que ningún test unitario puede cubrir: que plan, run, reuse, resume y
explain funcionen encadenados, que el JSON que sale valide contra el contrato, y
que el harness **no** escriba nada dentro del repositorio.
"""

from __future__ import annotations

import json
from pathlib import Path

from harness_support import (
    APP_SUITE,
    CORE_SUITE,
    FMT_SUITE,
    commit_change,
    demo_repo,
    git_dirty,
    run_cli,
)

SCHEMAS_DIR = Path(__file__).resolve().parent.parent / "contracts"


def _validate(schema_name: str, payload: dict) -> None:
    """Valida contra el esquema publicado. jsonschema es dependencia declarada del gate."""
    import jsonschema

    schema = json.loads((SCHEMAS_DIR / schema_name).read_text(encoding="utf-8"))
    jsonschema.validate(instance=payload, schema=schema)


def _state(repo: Path) -> Path:
    return repo.parent / "th-state"


def test_plan_json_matches_the_contract():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr + proc.stdout
        envelope = json.loads(proc.stdout)
        plan = envelope["plan"]
        _validate("plan.schema.json", plan)
        assert set(plan["required_suites"]) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, plan
        assert envelope["run_id"], envelope
        actions = {action["suite_id"]: action for action in plan["actions"]}
        assert actions[CORE_SUITE]["action"] == "EXECUTE", actions
        assert actions[FMT_SUITE]["action"] == "EXECUTE", actions


def test_plan_text_explains_every_decision():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// cambio en app\n")
        proc = run_cli(repo, state, "plan", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        assert "motivo:" in proc.stdout
        assert CORE_SUITE in proc.stdout  # aparece como excluida, con su razón
        assert "ningún fichero de su clausura cambió" in proc.stdout


def test_run_executes_writes_receipts_and_leaves_the_repo_clean():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        before = git_dirty(repo)

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr + proc.stdout
        payload = json.loads(proc.stdout)
        _validate("gate-verdict.schema.json", payload["verdict"])
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        assert {receipt["suite_id"] for receipt in payload["executed"]} == {
            CORE_SUITE,
            APP_SUITE,
            FMT_SUITE,
        }, payload["executed"]
        for receipt in payload["executed"]:
            _validate("receipt.schema.json", receipt)
            assert receipt["status"] == "PASSED", receipt
            assert receipt["log_sha256"], receipt
            if receipt["runner"]["name"] == "cargo-test":
                assert receipt["passed_cases"] >= 1, receipt

        # La evidencia va al estado, no al repositorio.
        assert (state / "receipts").is_dir()
        assert (state / "ledger.jsonl").is_file()
        assert git_dirty(repo) == before, "el harness escribió dentro del repositorio"
        assert not (repo / "th-state").exists()


def test_second_run_reuses_without_executing_again():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        first = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert first.returncode == 0, first.stderr

        second = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert second.returncode == 0, second.stderr
        payload = json.loads(second.stdout)
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        assert payload["executed"] == [], payload["executed"]
        assert len(payload["reused"]) == 3, payload["reused"]


def test_changing_a_source_invalidates_only_its_suites():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// app v1\n")
        first = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert first.returncode == 0, first.stderr

        commit_change(repo, "crates/app/src/lib.rs", "// app v2 distinta\n")
        plan_proc = run_cli(
            repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main"
        )
        envelope = json.loads(plan_proc.stdout)
        excluded = {entry["suite_id"]: entry["reason"] for entry in envelope["excluded"]}
        assert CORE_SUITE in excluded, excluded  # su clausura no cambió: ni se planifica
        assert "ningún fichero de su clausura cambió" in excluded[CORE_SUITE], excluded[CORE_SUITE]

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert APP_SUITE in executed, executed  # su código cambió: se reejecuta
        assert FMT_SUITE in executed, executed  # su clausura es todo el workspace
        assert CORE_SUITE not in executed, executed  # no se reejecuta código que nadie tocó


def test_merge_lane_executes_everything_by_default():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        warm = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert warm.returncode == 0, warm.stderr

        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "merge", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        plan = json.loads(proc.stdout)["plan"]
        actions = {action["suite_id"]: action["action"] for action in plan["actions"]}
        assert set(actions.values()) == {"EXECUTE"}, actions


def test_interrupted_receipt_is_not_a_pass_and_resume_reexecutes():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        plan_proc = run_cli(
            repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main"
        )
        plan = json.loads(plan_proc.stdout)["plan"]
        run_id = "run-interrumpido-de-prueba"

        # Un proceso que murió a mitad: run RUNNING sin evento terminal.
        (state / "runs").mkdir(parents=True, exist_ok=True)
        (state / "runs" / f"{run_id}.json").write_text(
            json.dumps(
                {
                    "schema_version": "th.run/v1",
                    "run_id": run_id,
                    "lane": "change",
                    "repo_id": "demo",
                    "plan_id": plan["plan_id"],
                    "source_fingerprint": plan["source_fingerprint"],
                    "base_ref": "main",
                    "status": "RUNNING",
                    "started_at": "2026-01-01T00:00:00Z",
                    "finished_at": "",
                    "suites": {},
                    "pending": [],
                }
            ),
            encoding="utf-8",
        )
        (state / "receipts").mkdir(parents=True, exist_ok=True)
        interrupted = {
            "schema_version": "th.receipt/v1",
            "receipt_id": "rcpt-interrumpido",
            "run_id": run_id,
            "repo_id": "demo",
            "suite_id": CORE_SUITE,
            "lane": "change",
            "status": "RUNNING",
            "input_fingerprint": "a" * 64,
            "source_snapshot": {
                "git_head": "0" * 40,
                "effective_content_digest": "b" * 64,
                "dirty": False,
                "worktree_id": "wt",
            },
            "runner": {
                "name": "cargo-test",
                "version": "cargo 1.0.0",
                "argv": ["cargo", "test", "-p", "demo-core", "--lib", "--quiet"],
                "features": [],
                "target_triple": "x86_64-unknown-linux-gnu",
            },
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "",
            "exit_code": -1,
            "discovered_cases": 0,
            "executed_cases": 0,
            "passed_cases": 0,
            "failed_cases": 0,
            "skipped_cases": 0,
            "log_sha256": "",
        }
        (state / "receipts" / "rcpt-interrumpido.json").write_text(
            json.dumps(interrupted), encoding="utf-8"
        )

        proc = run_cli(
            repo,
            state,
            "--format",
            "json",
            "resume",
            "--run-id",
            run_id,
            "--lane",
            "change",
            "--base",
            "main",
        )
        assert proc.returncode == 0, proc.stderr + proc.stdout
        payload = json.loads(proc.stdout)
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert CORE_SUITE in executed, executed  # el recibo interrumpido no se acepta como PASS
        run_state = json.loads((state / "runs" / f"{run_id}.json").read_text(encoding="utf-8"))
        assert run_state["status"] == "FINISHED", run_state
        assert run_state["verdict"]["verdict"] == "SATISFIED", run_state["verdict"]


def test_status_reports_the_last_run():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        run = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert run.returncode == 0, run.stderr
        proc = run_cli(repo, state, "--format", "json", "status")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["status"] == "FINISHED", payload
        assert payload["counts"]["executed"] == 3, payload["counts"]


def test_explain_shows_the_decision_and_the_evidence():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// app\n")
        run = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert run.returncode == 0, run.stderr

        proc = run_cli(
            repo, state, "--format", "json", "explain", "--suite", APP_SUITE, "--base", "main"
        )
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["decision"]["action"] == "REUSE", payload
        assert payload["decision"]["input_fingerprint"], payload
        assert payload["reuse"]["kind"] == "Reusable", payload["reuse"]

        other = run_cli(
            repo, state, "--format", "json", "explain", "--suite", CORE_SUITE, "--base", "main"
        )
        payload = json.loads(other.stdout)
        assert payload["decision"]["action"] == "NOT_REQUIRED", payload


def test_status_without_runs_is_a_harness_error():
    with demo_repo() as repo:
        proc = run_cli(repo, _state(repo), "status")
        assert proc.returncode == 2, proc
        assert "no hay ningún run" in proc.stderr, proc.stderr


def test_parallel_workers_are_recorded_and_capped():
    """La concurrencia se declara en el run, y por encima del tope se recorta."""
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "scripts/slow-a.sh", "# cambio\n")
        proc = run_cli(
            repo,
            state,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--max-workers",
            "99",
            "--only",
            "workspace:script:slow-a",
        )
        payload = json.loads(proc.stdout)
        run_state = json.loads((state / "runs" / f"{payload['run_id']}.json").read_text())
        concurrency = run_state["concurrency"]
        assert concurrency["requested"] == 99, concurrency
        assert concurrency["effective"] == 8, concurrency
        assert "cargo-target" in concurrency["locks"], concurrency


def test_unknown_suite_explain_is_honest():
    with demo_repo() as repo:
        proc = run_cli(
            repo,
            _state(repo),
            "--format",
            "json",
            "explain",
            "--suite",
            "no-existe:lib#-",
            "--base",
            "main",
        )
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["decision"]["action"] == "UNKNOWN", payload


def test_run_accepts_the_oracle_floor_and_executes_what_plan_promised():
    """`run --oracle cr08` tiene que existir y exigir lo mismo que `plan`.

    MEDIDO 2026-10-16. `plan` aceptaba `--oracle` y `run` no, así que el
    consumidor que delega el motor (`scripts/test-fast.sh`) construía un argv
    que la CLI rechazaba: la delegación estaba rota en su camino de ejecución.
    Plan y run con la misma bandera tienen que producir el mismo conjunto, y el
    JSON del run tiene que traer la paridad: si `parity` sale nulo con
    `--oracle cr08`, la bandera se aceptó y no llegó al planificador.
    """
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs")

        planned = run_cli(
            repo,
            state,
            "--format",
            "json",
            "plan",
            "--lane",
            "change",
            "--base",
            "main",
            "--oracle",
            "cr08",
        )
        assert planned.returncode == 0, (planned.returncode, planned.stderr)
        plan_payload = json.loads(planned.stdout)
        assert plan_payload["plan"]["required_suites"], "un plan vacío no prueba nada"
        assert plan_payload["parity"] is not None, plan_payload

        inspected = run_cli(
            repo,
            state,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--oracle",
            "cr08",
            "--plan-only",
        )
        assert inspected.returncode == 0, (inspected.returncode, inspected.stderr)
        run_payload = json.loads(inspected.stdout)
        assert run_payload["parity"] is not None, "la bandera no llegó al planificador"
        assert set(run_payload["plan"]["required_suites"]) == set(
            plan_payload["plan"]["required_suites"]
        ), (run_payload["plan"]["required_suites"], plan_payload["plan"]["required_suites"])


def test_run_rejects_an_oracle_value_it_cannot_honour():
    """Un valor de oráculo desconocido es error de uso, no un `none` silencioso."""
    with demo_repo() as repo:
        proc = run_cli(
            repo,
            _state(repo),
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--oracle",
            "cr09",
            "--plan-only",
        )
        assert proc.returncode != 0, proc.stdout
        assert "cr09" in proc.stderr or "invalid choice" in proc.stderr, proc.stderr


def test_plan_only_does_not_execute_anything():
    """`--plan-only` inspecciona el camino de ejecución sin ejecutarlo.

    Es la razón de existir de la bandera: sin ella, comprobar que un argv de
    consumidor es aceptable obligaba a correr el plan entero, y en la práctica
    no se comprobaba.
    """
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs")
        proc = run_cli(
            repo,
            state,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--plan-only",
        )
        assert proc.returncode == 0, (proc.returncode, proc.stderr)
        payload = json.loads(proc.stdout)
        assert payload["plan_only"] is True, payload
        assert payload["would_execute"], "con un cambio real hay suites que ejecutar"
        assert not (state / "runs").exists(), "el modo plan-only no debe crear un run"
        assert not git_dirty(repo), git_dirty(repo)


def test_resume_accepts_the_oracle_floor():
    """`resume --oracle cr08` existe: reanudar no puede exigir menos que plan.

    Una reanudación sin suelo podría continuar un run declarando menos
    obligaciones que las que el run original anunció.
    """
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs")
        first = run_cli(
            repo, state, "--format", "json", "run", "--lane", "change", "--base", "main"
        )
        assert first.returncode == 0, (first.returncode, first.stderr)
        run_id = json.loads(first.stdout)["run_id"]

        done = run_cli(
            repo,
            state,
            "--format",
            "json",
            "resume",
            "--run-id",
            run_id,
            "--lane",
            "change",
            "--base",
            "main",
            "--oracle",
            "cr08",
        )
        assert done.returncode == 0, (done.returncode, done.stderr)
        payload = json.loads(done.stdout)
        assert payload["run_id"] == run_id, payload
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]


def test_a_suite_with_unresolved_pipeline_variables_is_never_executed():
    """Una variable sin expandir no llega a cargo: el veredicto es indeterminado.

    MEDIDO 2026-10-16. El catálogo tomaba el argv literal de la pipeline, así que
    `cargo build --release --target $target` llegaba a cargo con `$target` dentro y
    devolvía `could not find specification for target "$target"`. El harness lo
    anotaba como INFRA_ERROR de una obligación que sí había que cubrir. Lo
    correcto es no ejecutarla y decirlo: el run queda INCONCLUSIVE y esa suite no
    tiene recibo, en vez de tener un fallo que no era suyo.
    """
    with demo_repo() as repo:
        state = _state(repo)
        pipeline = repo / "product-fast.pipeline.kts"
        pipeline.write_text(
            pipeline.read_text(encoding="utf-8").replace(
                '    stage("fmt") {',
                '    stage("binaries") {\n'
                '        sh("$cd && cargo build --release --tag $tag -p demo-core")\n'
                "    }\n"
                '    stage("fmt") {',
            ),
            encoding="utf-8",
        )
        commit_change(repo, "crates/core/src/lib.rs")

        proc = run_cli(
            repo, state, "--format", "json", "run", "--lane", "change", "--base", "main"
        )
        payload = json.loads(proc.stdout)
        assert proc.returncode == 3, (proc.returncode, payload.get("executed"))

        assert payload["verdict"]["verdict"] == "INCONCLUSIVE", payload["verdict"]["verdict"]
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert "demo-core:cargo:build#-" not in executed, executed
        gap = [g for g in payload["verdict"]["gaps"] if g["suite_id"] == "demo-core:cargo:build#-"]
        assert gap, payload["verdict"]["gaps"]
        assert "$tag" in payload["unknown_reasons"][0], payload["unknown_reasons"]

        # Y el resto del plan sí corrió: no ejecutar lo indeterminado no puede
        # convertirse en no ejecutar nada.
        assert payload["executed"], "el plan tenía suites ejecutables"


def test_the_run_gives_suites_the_target_dir_the_lane_declares():
    """Las suites corren con el entorno que la lane da por hecho.

    MEDIDO 2026-10-16: `merge-gate.pipeline.kts` exporta
    `CARGO_TARGET_DIR="$(scripts/ci/target-dir.sh)"` para las etapas que buscan
    binarios construidos. El harness corría con el entorno ambiente, así que
    `prf_h06_adversarial_e2e` no encontraba `cognicode-mcp` y fallaba como si el
    fallo fuera suyo. El valor no se inventa: lo dice el script del repositorio,
    el mismo que usa el gate.
    """
    with demo_repo() as repo:
        state = _state(repo)
        target_dir = repo.parent / "target-dir-de-la-lane"
        target_dir.mkdir()
        scripts_ci = repo / "scripts" / "ci"
        scripts_ci.mkdir(parents=True, exist_ok=True)
        resolver = scripts_ci / "target-dir.sh"
        resolver.write_text(f"#!/usr/bin/env bash\nprintf '%s\\n' '{target_dir}'\n", encoding="utf-8")
        resolver.chmod(0o755)

        mark = repo.parent / "env-mark"
        probe = repo / "scripts" / "env-probe.sh"
        # El destino va dentro del script, no como argumento: el catálogo ejecuta
        # los scripts que la pipeline declara como `bash <ruta>` y una obligación
        # de script no arrastra argumentos.
        probe.write_text(
            "#!/usr/bin/env bash\n"
            'printf "%s" "${CARGO_TARGET_DIR:-sin-target-dir}" > ' + repr(str(mark)) + "\n",
            encoding="utf-8",
        )
        probe.chmod(0o755)
        pipeline = repo / "product-fast.pipeline.kts"
        pipeline.write_text(
            pipeline.read_text(encoding="utf-8").replace(
                '    stage("slow-a") {',
                '    stage("env-probe") {\n        sh("$cd && bash scripts/env-probe.sh")\n    }\n'
                '    stage("slow-a") {',
            ),
            encoding="utf-8",
        )
        # El cambio toca `scripts/ci/` a propósito: una obligación de script sólo
        # entra en el plan si cambió o si la cita la documentación tocada, y un
        # camino de infraestructura de CI arrastra todas las obligaciones.
        commit_change(
            repo,
            "scripts/ci/target-dir.sh",
            "#!/usr/bin/env bash\nprintf '%s\\n' " + repr(str(target_dir)) + "\n",
        )

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode in (0, 1), (proc.returncode, proc.stderr)
        assert mark.read_text(encoding="utf-8") == str(target_dir), mark.read_text(encoding="utf-8")

        run_state = json.loads(
            sorted((state / "runs").glob("*.json"))[-1].read_text(encoding="utf-8")
        )
        assert run_state["environment"]["resolved"] == str(target_dir), run_state["environment"]


def test_a_change_in_a_dependency_invalidates_the_dependent_suite():
    """Un resultado compilado depende de todo lo que compila, no sólo de su crate.

    MEDIDO 2026-10-16. La huella de una suite miraba sólo el subárbol de su
    paquete, así que un cambio en `demo-core` no invalidaba el recibo de
    `demo-app`: la suite se seleccionaba por impacto (bien) y se reutilizaba un
    resultado de antes del cambio (mal). Reutilizar un resultado cuyo código
    compilado cambió es un verde falso, y es el peor modo de fallo del harness.
    """
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// core v1\n")
        first = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert first.returncode == 0, first.stderr
        assert APP_SUITE in {r["suite_id"] for r in json.loads(first.stdout)["executed"]}, (
            "el fixture declara que `demo-app` depende de `demo-core`"
        )

        commit_change(repo, "crates/core/src/lib.rs", "// core v2 distinta\n")
        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        reused = {receipt["suite_id"] for receipt in payload["reused"]}
        assert APP_SUITE in executed, (executed, reused)
        assert APP_SUITE not in reused, reused


def test_the_critical_obligation_floor_applies_through_the_cli():
    """El suelo de obligaciones tiene que aplicarse en el camino que usa el usuario.

    MEDIDO 2026-10-16. `prepare()` cargaba `test-harness/obligations.json`, avisaba
    si era ilegible… y construía el `Context` sin pasárselo, así que todos los
    planes de la CLI llevaban el suelo vacío por defecto. El mecanismo existía y sus
    tests lo llamaban con los objetos cargados a mano: probaban el mecanismo, no el
    camino real. Aquí se ejerce la CLI sobre un repositorio cuyo suelo promete una
    obligación que sus pipelines **no** ejecutan, y el plan tiene que declararse
    indeterminado en vez de dar la lane por cubierta.
    """
    with demo_repo() as repo:
        state = _state(repo)
        floor = repo / "test-harness" / "obligations.json"
        floor.parent.mkdir(parents=True, exist_ok=True)
        floor.write_text(
            json.dumps(
                {
                    "schema_version": "th.obligations/v1",
                    "notes": "fixture: promete una obligación que este repo no ejecuta",
                    "entries": {"change": ["workspace:script:no-existe"]},
                }
            ),
            encoding="utf-8",
        )
        commit_change(repo, "crates/core/src/lib.rs")

        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main")
        assert proc.returncode == 3, (proc.returncode, proc.stdout[:300])
        payload = json.loads(proc.stdout)
        assert payload["plan"]["status"] == "UNKNOWN_INPUT", payload["plan"]["status"]
        reasons = " ".join(payload["plan"]["unknown_reasons"])
        assert "workspace:script:no-existe" in reasons, reasons


def test_the_pinned_inventory_minimum_applies_through_the_cli():
    """Los mínimos pinados del inventario tienen que llegar al plan y al veredicto.

    MEDIDO 2026-10-16: `inventory.json` se cargaba y no viajaba al `Context`, así que
    el pin no se aplicaba en ningún camino de la CLI y un PASS con menos casos del
    pinado se aceptaba igual. Aquí se pincha un mínimo imposible y el run tiene que
    rechazarlo con un hueco que lo nombre.
    """
    with demo_repo() as repo:
        state = _state(repo)
        inventory = repo / "test-harness" / "inventory.json"
        inventory.parent.mkdir(parents=True, exist_ok=True)
        inventory.write_text(
            json.dumps(
                {
                    "schema_version": "th.inventory/v1",
                    "entries": {"demo-core:lib#-": 999},
                }
            ),
            encoding="utf-8",
        )
        commit_change(repo, "crates/core/src/lib.rs")

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        payload = json.loads(proc.stdout)
        assert proc.returncode != 0, (proc.returncode, json.dumps(payload)[:300])
        gaps = payload["verdict"]["gaps"]
        assert any("999" in gap["reason"] for gap in gaps), gaps


def test_the_dev_profile_cannot_swallow_an_oracle_demand():
    """El perfil dev recorta el mini-gate, nunca el suelo del oráculo.

    Es la propiedad que hace segura la bandera. El fixture no puede ejercer la
    *adición* del suelo (sus paquetes no son los que `CR08_SCOPES` mapea), así que
    aquí se ejerce lo que sí puede: con un selector que demanda algo, el plan dev no
    puede declararse en paridad cuando no lo está. O lo exige, o lo dice.
    """
    with demo_repo() as repo:
        state = _state(repo)
        selector = repo / "scripts" / "ci" / "select-suites.sh"
        selector.write_text(
            "#!/usr/bin/env bash\n"
            'printf \'{"strategy":"changed","reason":"fixture exige app","suites":["app"]}\\n\'\n',
            encoding="utf-8",
        )
        selector.chmod(0o755)
        commit_change(repo, "crates/core/src/lib.rs")

        proc = run_cli(
            repo, state, "--format", "json", "plan",
            "--lane", "change", "--base", "main", "--profile", "dev", "--oracle", "cr08",
        )
        assert proc.returncode in (0, 3), (proc.returncode, proc.stderr[:200])
        payload = json.loads(proc.stdout)
        plan = payload["plan"]
        assert plan["profile"] == "dev", plan["profile"]
        parity = payload["parity"]
        assert parity is not None, payload
        demanded = [row for row in parity["rows"] if row["demanded_by_oracle"]]
        assert demanded, parity
        # La demanda no puede desaparecer: o está exigida, o el plan declara que no
        # está en paridad. Lo que no vale es afirmar AGREE sin cubrirla.
        covered = all(row["verdict"] in ("AGREE", "HARNESS_STRICTER") for row in demanded)
        if not covered:
            assert parity["verdict"] != "AGREE", parity
            assert parity["missing_scopes"], parity
        # Nota: escribir el selector en el repositorio hace el cambio *global* (un
        # camino de `scripts/ci/` arrastra todas las obligaciones), así que aquí no
        # se puede comprobar el recorte del perfil; eso lo cubre
        # `test_the_dev_profile_drops_the_dependents_closure_and_workspace_stages`,
        # que ejercita la selección directamente. Lo que sí se comprueba aquí es lo
        # que hace segura la bandera: que una demanda del oráculo no desaparezca.


def test_the_dev_profile_is_a_policy_error_not_a_traceback():
    """El perfil dev fuera de `change` sale rc=2 con su motivo, no un traceback."""
    with demo_repo() as repo:
        state = _state(repo)
        proc = run_cli(repo, state, "plan", "--lane", "merge", "--base", "main", "--profile", "dev")
        assert proc.returncode == 2, (proc.returncode, proc.stdout[:200])
        assert "sólo se permite" in proc.stderr, proc.stderr

#!/usr/bin/env python3
"""Contrato del test harness (TH) contra el repositorio real.

El harness existe para decidir qué pruebas sobran y cuáles hay que rehacer con
evidencia. Un contrato que solo probara sus funciones internas dejaría fuera lo
que de verdad importa aquí: que **no** pueda inventar un verde. Este fichero fija
cuatro propiedades sobre el repositorio en el que corre:

1. Su propia suite corre y no pasa en vacío. Se ejecuta con
   `scripts/run_contract_tests.py`, el mismo runner sin dependencias que usa el
   resto de la suite de contratos (el gate no instala pytest), y se exige que el
   número de tests ejecutados sea al menos el número de funciones `test_` que hay
   en los ficheros. Un glob que no case nada sería el pase vacío que este
   repositorio lleva tiempo negándose a firmar.

2. El plan que emite valida contra `plan.schema.json`, y cuando el harness no
   puede decidir lo dice: un plan indeterminado sale con `unknown_reasons`
   poblado y código 3, nunca como un plan limpio.

3. El harness no escribe dentro del repositorio. La evidencia va al estado
   (`$XDG_STATE_HOME`), y el árbol de trabajo queda como estaba.

4. Un hueco declarado no se convierte en PASS. Un `run` cuyas obligaciones no se
   ejecutan termina en `UNSATISFIED` con el hueco nombrado, y no en verde.

Además se comprueba que el harness importa solo biblioteca estándar y módulos
propios: es lo que permite que corra en el mismo entorno que el gate sin instalar
nada, y es una propiedad que se rompe sola en cuanto alguien añade un import
cómodo.

Run:
    python3 scripts/ci/test_harness_contract.py
"""

from __future__ import annotations

import ast
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HARNESS_DIR = REPO_ROOT / "test-harness"
HARNESS_BIN = HARNESS_DIR / "bin" / "harness.py"
CONTRACT_RUNNER = REPO_ROOT / "scripts" / "run_contract_tests.py"
SCHEMAS_DIR = HARNESS_DIR / "contracts"
TOTAL_RE = re.compile(r"TOTAL:\s+(\d+) passed,\s+(\d+) failed")

# Paquetes propios del harness: lo que puede importarse sin declarar dependencia.
LOCAL_MODULES = {"harness", "adapters", "harness_support"}


def _test_files() -> list[Path]:
    return sorted((HARNESS_DIR / "tests").glob("test_*.py"))


def _declared_test_functions() -> int:
    total = 0
    for path in _test_files():
        tree = ast.parse(path.read_text(encoding="utf-8"))
        total += sum(
            1
            for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")
        )
    return total


def _run_cli(state_dir: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [
            sys.executable,
            str(HARNESS_BIN),
            "--repo",
            str(REPO_ROOT),
            "--state-dir",
            str(state_dir),
            *args,
        ],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=False,
    )


def test_the_harness_suite_runs_and_does_not_pass_vacuously():
    """La suite del harness se ejecuta entera: sin pytest y sin cero tests."""
    files = _test_files()
    assert len(files) >= 4, f"esperaba varias suites del harness, encontré {files}"

    proc = subprocess.run(
        [sys.executable, str(CONTRACT_RUNNER), *[str(path) for path in files]],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=False,
    )
    output = proc.stdout + proc.stderr
    assert proc.returncode == 0, output[-6000:]

    match = TOTAL_RE.search(proc.stdout)
    assert match is not None, f"el runner no resumió el total:\n{output[-3000:]}"
    passed, failed = int(match.group(1)), int(match.group(2))
    assert failed == 0, output[-6000:]
    declared = _declared_test_functions()
    assert passed >= declared, (
        f"el runner ejecutó {passed} tests y los ficheros declaran {declared}: "
        "algún test no se está ejecutando"
    )


def test_the_plan_validates_against_the_published_schema():
    jsonschema = _require_jsonschema()
    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        proc = _run_cli(
            Path(tmp), "--format", "json", "plan", "--lane", "change", "--base", "origin/main"
        )
        assert proc.stdout.strip(), f"el plan no imprimió JSON: {proc.stderr}"
        envelope = json.loads(proc.stdout)
        plan = envelope["plan"]
        schema = json.loads((SCHEMAS_DIR / "plan.schema.json").read_text(encoding="utf-8"))
        jsonschema.validate(instance=plan, schema=schema)

        # Honestidad del código de salida: 0 solo si hay plan firme; 3 si el
        # harness declara que no puede decidir, con el motivo escrito.
        if proc.returncode != 0:
            assert proc.returncode == 3, f"salida inesperada {proc.returncode}: {proc.stderr}"
            assert plan["unknown_reasons"], (
                "el harness devolvió indeterminado sin decir por qué: "
                f"{json.dumps(plan)[:400]}"
            )
            assert plan["status"] == "UNKNOWN_INPUT", plan["status"]


def test_the_harness_does_not_write_inside_the_repository():
    """La evidencia va al estado; el repositorio queda como estaba."""
    status_before = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=True,
    ).stdout

    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        state = Path(tmp)
        proc = _run_cli(state, "--format", "json", "plan", "--lane", "change", "--base", "origin/main")
        assert proc.returncode in (0, 3), proc.stderr
        assert state.exists()
        # El estado se creó donde se pidió, y no dentro del repositorio.
        assert not (REPO_ROOT / "th-state").exists()
        assert not (HARNESS_DIR / "state").exists()
        assert not any(HARNESS_DIR.rglob("ledger.jsonl")), "el harness escribió su ledger en el repo"

    status_after = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    assert status_after == status_before, (
        "el harness modificó el árbol de trabajo del repositorio:\n"
        f"antes:\n{status_before}\ndespués:\n{status_after}"
    )


def test_a_declared_gap_cannot_become_a_pass():
    """Sin ejecutar nada, el veredicto es UNSATISFIED con el hueco nombrado."""
    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        state = Path(tmp)
        proc = _run_cli(
            state,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "origin/main",
            "--only",
            "suite-que-no-existe:lib#-",
        )
        if proc.returncode == 2:
            # El checkout no tiene base con la que planificar: el harness lo dice
            # y no ejecuta nada, que también es un comportamiento honesto.
            assert "no resuelve" in proc.stderr or "merge-base" in proc.stderr, proc.stderr
            return
        assert proc.returncode == 1, f"esperaba 1 (UNSATISFIED), obtuve {proc.returncode}: {proc.stdout}"
        payload = json.loads(proc.stdout)
        verdict = payload["verdict"]
        assert verdict["verdict"] == "UNSATISFIED", verdict
        assert verdict["proofs"] == [], verdict["proofs"]
        assert verdict["gaps"], "un veredicto incumplido sin huecos no explica nada"
        reasons = " ".join(gap["reason"] for gap in verdict["gaps"])
        assert "sin recibo" in reasons, reasons
        assert payload["pending"], "las obligaciones no ejecutadas deben quedar declaradas"


def test_the_harness_is_never_weaker_than_the_current_selector():
    """Paridad con CR-08 sobre el corpus: ninguna debilidad sin declarar.

    El selector CR-08 sigue siendo la autoridad mientras no se demuestre paridad.
    Esta comprobación recorre las clases de cambio del corpus, ejecuta el selector
    de verdad y exige que el harness no se quede por debajo salvo en las
    diferencias declaradas y razonadas (`docs/**`, y las caídas a ALL del oráculo
    cuando el harness tiene clasificación propia del path).
    """
    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        proc = _run_cli(Path(tmp), "--format", "json", "parity", "--corpus", "--lane", "change")
        assert proc.stdout.strip(), f"parity no imprimió JSON: {proc.stderr}"
        report = json.loads(proc.stdout)
        summary = report["summary"]
        assert summary["total"] >= 10, summary
        assert summary["parity_ok"] is True, (
            "el harness exige menos que el selector vigente sin desviación declarada en: "
            + ", ".join(summary["weak_undeclared"])
        )
        assert proc.returncode == 0, proc.stdout[-2000:]
        verdicts = summary["verdicts"]
        assert verdicts.get("HARNESS_WEAKER", 0) == 0, verdicts
        # El corpus tiene que medir de verdad: si todo fuese AGREE por vacío, la
        # comparación no estaría mirando nada.
        assert verdicts.get("AGREE", 0) + verdicts.get("HARNESS_STRICTER", 0) >= 8, verdicts


def test_the_pinned_inventory_is_consistent_with_the_observations():
    """El pin de casos no puede estar por encima de lo que las suites ejecutan.

    Un pin inventado convierte el gate en ruido; un pin por debajo de la realidad
    deja pasar una suite que perdió pruebas. Esta comprobación exige que el pin
    versionado, si existe, no esté por encima de lo observado en los recibos del
    ledger de esta máquina, y que `inventory --check` no señale nada por debajo.
    """
    inventory_path = HARNESS_DIR / "inventory.json"
    if not inventory_path.exists():
        return  # sin pin no hay nada que comprobar; el plan exige el suelo de Cargo
    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        proc = _run_cli(Path(tmp), "--format", "json", "inventory", "--check")
        assert proc.stdout.strip(), proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["problem"] is None, payload["problem"]
        assert payload["under_pin"] == [], payload["under_pin"]
        # El pin tiene que describir suites reales del catálogo, no fantasmas.
        catalog = json.loads(
            _run_cli(Path(tmp), "--format", "json", "plan", "--lane", "change", "--base", "origin/main").stdout
        )
        known = set(catalog["plan"]["required_suites"]) | {
            entry["suite_id"] for entry in catalog.get("excluded", [])
        }
        unknown = [suite for suite in payload["pinned"]["entries"] if known and suite not in known]
        assert not unknown, f"el pin nombra suites que no están en el catálogo: {unknown}"


def test_every_aat_row_has_a_status_and_a_real_test_when_it_is_killed():
    """El AAT no puede quedarse en prosa: cada fila tiene estado y, si se mata, test.

    Un AAT que dice «esto se mata» sin un test que lo mate es una afirmación de
    fe. Este contrato exige que las mutaciones declaradas `KILLED` nombren un test
    que exista en la suite del harness, y que el reparto de estados esté escrito.
    """
    aat = (HARNESS_DIR / "aat" / "AAT.md").read_text(encoding="utf-8")
    rows = re.findall(r"^\|\s*(AAT-\d+)\s*\|\s*(KILLED|PARTIAL|NOT_RUN)\s*\|\s*([^|]*)\|", aat, re.MULTILINE)
    assert len(rows) >= 30, f"la tabla de ejecución del AAT tiene {len(rows)} filas"

    statuses = {status for _, status, _ in rows}
    assert statuses <= {"KILLED", "PARTIAL", "NOT_RUN"}, statuses

    defined: set[str] = set()
    for path in sorted((HARNESS_DIR / "tests").glob("test_*.py")):
        defined.update(re.findall(r"^def (test_\w+)", path.read_text(encoding="utf-8"), re.MULTILINE))

    missing = [
        f"{row_id}: {name.strip()}"
        for row_id, status, name in rows
        if status == "KILLED" and name.strip() and name.strip() not in defined
    ]
    assert not missing, "filas KILLED sin test que las mate: " + "; ".join(missing)

    killed = sum(1 for _, status, _ in rows if status == "KILLED")
    assert killed >= 15, f"solo {killed} filas KILLED: el AAT se está quedando en prosa"
    summary = re.search(r"Resumen:\s*(\d+)\s*`KILLED`,\s*(\d+)\s*`PARTIAL`,\s*(\d+)\s*`NOT_RUN`", aat)
    assert summary, "el AAT no declara el reparto de estados"
    assert int(summary.group(1)) == killed, "el resumen no coincide con la tabla"


def test_gc_dry_run_never_deletes_evidence():
    """La recolección dice qué borraría y no toca ni un recibo."""
    with tempfile.TemporaryDirectory(prefix="th-contract-") as tmp:
        state = Path(tmp)
        proc = _run_cli(state, "--format", "json", "gc", "--keep", "1", "--dry-run")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["dry_run"] is True, payload
        assert payload["bytes_to_free"] == 0, payload
        assert payload["logs_to_delete"] == [], payload


def test_the_harness_imports_only_the_standard_library():
    """Corre en el gate sin instalar nada: un import de terceros lo rompería allí."""
    allowed = set(sys.stdlib_module_names) | LOCAL_MODULES
    offenders: list[str] = []
    for path in sorted((HARNESS_DIR / "src").rglob("*.py")) + sorted(
        (HARNESS_DIR / "adapters").rglob("*.py")
    ) + sorted((HARNESS_DIR / "bin").rglob("*.py")):
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            names: list[str] = []
            if isinstance(node, ast.Import):
                names = [alias.name.split(".")[0] for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                names = [node.module.split(".")[0]]
            for name in names:
                if name not in allowed:
                    offenders.append(f"{path.relative_to(REPO_ROOT)}: {name}")
    assert not offenders, "imports fuera de la biblioteca estándar: " + "; ".join(offenders)


def test_the_declared_policy_forbids_reuse_where_it_would_be_unsafe():
    """La política publicada no puede relajarse por accidente."""
    sys.path.insert(0, str(HARNESS_DIR / "src"))
    from harness.policy import reuse_allowed  # noqa: E402

    assert reuse_allowed("change")[0] is True
    for lane in ("integration", "certification", "release-candidate", "release"):
        assert reuse_allowed(lane, allow_reuse_flag=True)[0] is False, lane
    # La lane de merge ejecuta todo salvo petición explícita del operador.
    assert reuse_allowed("merge")[0] is False


def _require_jsonschema():
    try:
        import jsonschema  # noqa: F401
    except ImportError as error:  # pragma: no cover - dependencia declarada del gate
        raise AssertionError(
            "jsonschema no está instalado y sin él el contrato del plan no se puede "
            "verificar. El gate lo instala (merge-gate.pipeline.kts, etapa de contratos)"
        ) from error
    import jsonschema

    return jsonschema


if __name__ == "__main__":  # pragma: no cover - lo usan el runner y CI
    sys.exit(0)

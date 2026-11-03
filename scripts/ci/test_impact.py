#!/usr/bin/env python3
"""Contrato del selector por impacto (`scripts/ci/impact.py`).

## Lo que esta en juego

`impact-gate.pipeline.kts` publica el estado `merge-gate` ejecutando solo lo
que un cambio puede romper. Si el selector se queda corto, entra en `main` un
rojo que nadie ejecuto. Este contrato fija:

  - que falla cerrado (diff vacio, manifiesto raiz, ruta desconocida => full);
  - que el radio de impacto sigue las dependencias en la direccion correcta (un
    cambio en core alcanza a la CLI; uno en la CLI no alcanza a core);
  - que un cambio en el propio gate ejecuta las etapas que cambiaron;
  - que el grafo de modulos de core ve las dependencias que un oraculo
    independiente (nombres que solo define un modulo) dice que existen;
  - que cada etapa del merge gate tiene una atribucion decidida, y que el
    filtro de core sigue encontrando la etapa que reescribe.

La red de seguridad de fondo es el gate completo en `main` cada noche: si
alli aparece un rojo que un PR dio por no afectado, el caso entra aqui.

Run:
    python3 scripts/ci/test_impact.py
"""
from __future__ import annotations

import collections
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts" / "ci"))
import impact  # noqa: E402

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def test_it_fails_closed() -> None:
    for files, why in (
        ([], "diff vacio"),
        (["Cargo.toml"], "manifiesto del workspace"),
        ([".cargo/config.toml"], "config de cargo"),
        (["rust-toolchain.toml"], "toolchain"),
        (["sandbox/fixtures/x.json"], "fixtures que leen los tests de core"),
        (["algo/nuevo/que/nadie/conoce.rs"], "ruta sin clasificar"),
    ):
        got = impact.plan(files)["tier"]
        check(got == "full", f"{why} ({files}) debe ser 'full' y es '{got}'")


def test_docs_are_docs_and_only_docs() -> None:
    check(impact.plan(["docs/roadmap/JOURNAL.md", "AGENTS.md"])["tier"] == "docs",
          "un cambio solo de documentacion no sale 'docs'")
    check(impact.plan(["docs/x.md", "crates/cognicode-cli/src/main.rs"])["tier"] == "impact",
          "documentacion + codigo no puede salir 'docs'")


def test_the_radius_follows_the_dependents() -> None:
    p = impact.plan(["crates/cognicode-core/src/domain/naming.rs"])
    check({"cognicode-cli", "cognicode-mcp", "cognicode-explorer"} <= set(p.get("affected_crates", [])),
          f"un cambio en core debe alcanzar a sus dependientes: {p.get('affected_crates')}")
    p = impact.plan(["crates/cognicode-cli/src/main.rs"])
    check(p.get("affected_crates") == ["cognicode-cli"],
          f"un cambio en la CLI no alcanza a nadie mas: {p.get('affected_crates')}")
    p = impact.plan(["product/tools.json"])
    check(impact.CORE in p.get("changed_crates", []),
          "product/ lo compila core con include_str!: tiene que contar como cambio de core")


def test_the_gate_judges_its_own_change() -> None:
    p = impact.plan(["merge-gate.pipeline.kts"])
    check(p["tier"] == "impact" and p.get("gate_changed"),
          "un cambio en merge-gate.pipeline.kts debe ejecutar las etapas que cambian")
    p = impact.plan(["scripts/ci/check-build-isolation.sh"])
    rows = impact.schedule(p)
    check(any(stage == "build-isolation" and ring == 1 for ring, stage, _, _ in rows),
          "un script que cambia debe ejecutar la etapa del gate que lo usa")


def test_every_gate_stage_has_a_decided_attribution() -> None:
    undecided = []
    for stage, body in impact.gate_stages():
        who = impact.stage_crates(stage, body)
        explicit = (stage in impact.ALWAYS or stage in impact.FULL_ONLY or stage in impact.BY_STAGE
                    or stage in impact.BUILDS or stage == "clippy" or isinstance(who, set))
        if not explicit:
            undecided.append(stage)
    check(not undecided,
          f"etapas del merge gate sin atribucion decidida en impact.py (corren siempre por defecto, "
          f"pero alguien tiene que decidirlo): {undecided}")


def test_verify_debug_binaries_never_runs_without_the_binaries_it_verifies() -> None:
    """Un cambio en core alcanza a verify-debug-binaries, que exige los tres
    binarios; la etapa que construye cada uno tiene que correr antes, aunque su
    crate caiga en un anillo posterior (el gate lo vio fallar con
    cognicode-control-plane ausente)."""
    rows = impact.schedule(impact.plan(["crates/cognicode-core/src/lib.rs"]))
    order = [stage for _, stage, _, _ in rows]
    check("verify-debug-binaries" in order, "core debe alcanzar verify-debug-binaries")
    verify = order.index("verify-debug-binaries")
    for builder in ("build-cognicode-mcp-debug", "build-cognicode-cli-debug", "build-control-plane-debug"):
        check(builder in order and order.index(builder) < verify,
              f"{builder} debe correr antes de verify-debug-binaries (orden: {order})")


def test_a_stage_that_exports_a_binary_path_gets_that_binary_built_first() -> None:
    """Un PR que solo cambia la etapa core no afecta a ningun crate, pero la
    etapa exporta COGNICODE_MCP_BIN y sus tests exigen que el binario exista:
    los constructores corren antes que ella."""
    stage = "core-unrestricted-evidence-kernel"
    real = impact.gate_stages

    def with_old_core(text=None):
        stages = real()
        return [(s, b + " # anterior" if s == stage else b) for s, b in stages] if text else stages

    impact.gate_stages = with_old_core
    try:
        p = {"tier": "impact", "affected_crates": [], "changed_crates": [], "changed_scripts": [],
             "gate_changed": True}
        order = [s for _, s, _, _ in impact.schedule(p, base="HEAD")]
    finally:
        impact.gate_stages = real
    check(stage in order, f"la etapa cambiada debe correr (orden: {order})")
    for builder in ("build-cognicode-mcp-debug", "build-cognicode-cli-debug"):
        check(builder in order and order.index(builder) < order.index(stage),
              f"{builder} debe correr antes de {stage} (orden: {order})")


def test_the_core_filter_finds_its_stage() -> None:
    hits = [s for s, b in impact.gate_stages() if impact.CORE_EK.search(b)]
    check(hits == ["core-unrestricted-evidence-kernel"],
          f"el filtro por modulo de core no encuentra su etapa ({hits}); sin el, core corre entero")


def test_the_core_ring_runs_the_multimodal_tests() -> None:
    body = next(b for s, b in impact.gate_stages() if s == "core-unrestricted-evidence-kernel")
    feats = "--features evidence-kernel,multimodal"
    check(feats in body, "la etapa entera de core debe correr con multimodal: sus tests no se ejecutaban en el gate")
    cmd = impact.core_command({"mode": "modules", "affected_modules": ["domain"], "integration": ["m10_acceptance"]})
    check(cmd is not None and cmd.count(feats) == 2,
          f"el filtro por modulo debe usar los mismos features que la etapa entera ({cmd})")
    check(impact.CORE_EK.search(body) is not None, "el filtro por modulo debe seguir encontrando la etapa")


def test_the_gate_reports_where_the_time_went() -> None:
    check(impact.fmt_duration(0.4) == "0s" and impact.fmt_duration(83) == "1m23s"
          and impact.fmt_duration(3600) == "60m00s", "fmt_duration debe dar 0s, 1m23s y 60m00s")
    rows = [(0, "fmt", 2.0, False), (1, "core", 300.0, False), (2, "qw08", 0.0, True)]
    lines = impact.timing_report(rows).splitlines()
    check(lines[0].startswith("==> tiempo por etapa"), "la tabla abre con su titulo")
    check("core" in lines[1], "la etapa mas lenta va primero")
    check(any("qw08" in l and "reutilizada" in l for l in lines), "una etapa reutilizada se marca como tal")
    check("5m02s" in lines[-1] and "total" in lines[-1], "la ultima linea suma las etapas ejecutadas")


def test_core_graph_sees_what_an_independent_oracle_sees() -> None:
    """Oraculo: un nombre de tipo que define UN solo modulo y que otro modulo
    menciona (fuera de comentarios y strings) es una dependencia real casi
    siempre. El grafo tiene que cubrir al menos el 98 % de esos pares."""
    g = impact.CoreGraph()
    owners = collections.defaultdict(set)
    for mod, items in g.defines.items():
        for item in items:
            owners[item].add(mod)
    unique = {i: next(iter(ms)) for i, ms in owners.items() if len(ms) == 1 and len(i) >= 8 and i[0].isupper()}
    memo: dict[str, set[str]] = {}

    def reach(m: str) -> set[str]:
        if m not in memo:
            memo[m] = impact.closure({m}, g.importers)
        return memo[m]

    pairs = missed = 0
    for user, code in g.code.items():
        for name in set(re.findall(r"\b[A-Z][A-Za-z0-9_]{7,}\b", code)):
            definer = unique.get(name)
            if definer is None or definer == user:
                continue
            pairs += 1
            missed += user not in reach(definer)
    check(pairs > 1000, f"el oraculo encontro solo {pairs} pares: no esta midiendo")
    recall = 1 - missed / max(pairs, 1)
    check(recall >= 0.98, f"el grafo de core cubre el {recall:.2%} de las dependencias del oraculo (minimo 98 %)")


def test_inline_test_modules_do_not_widen_the_radius() -> None:
    """`mod tests { use super::*; }` es el propio fichero, no su padre. Si se
    leyera como el padre, cada test unitario dependeria de toda la fachada."""
    g = impact.CoreGraph()
    sel = g.select({"domain::aggregates::refactor"})
    total = len(g.files)
    check(len(sel["affected_modules"]) < total // 2,
          f"cambiar domain::aggregates::refactor alcanza {len(sel['affected_modules'])} de {total} modulos: "
          "el resolvedor vuelve a confundir super:: dentro de un mod en linea")


def test_explorer_and_runtime_never_reach_the_clippy_stage() -> None:
    p = impact.plan(["crates/cognicode-core/src/domain/naming.rs"])
    clippy = [body for _, stage, _, body in impact.schedule(p) if stage == "clippy"]
    check(len(clippy) == 1 and "-p cognicode-cli" in clippy[0],
          f"un cambio en core sigue lintando a sus dependientes: {clippy}")
    check(all(c not in clippy[0] for c in ("cognicode-explorer", "cognicode-runtime")),
          f"explorer y runtime estan fuera de los gates: {clippy}")


def test_prior_evidence_is_keyed_by_content_not_by_command() -> None:
    dirs, dependents = impact.workspace()
    ids = {"crates/cognicode-cli": "cli-1", "crates/cognicode-core": "core-1", "crates/cognicode-mcp": "mcp-1"}
    real = impact.git_id
    impact.git_id = lambda sha, path: ids.get(path, "same")
    try:
        body = "cargo test -p cognicode-cli --quiet"
        key = lambda: impact.evidence_key("cli-unit-tests", body, {"cognicode-cli"}, "x", dirs, dependents)
        base = key()
        ids["crates/cognicode-mcp"] = "mcp-2"   # no es dependencia de la CLI
        check(key() == base, "un cambio en un crate del que la etapa no depende no puede invalidar su evidencia")
        ids["crates/cognicode-core"] = "core-2"  # la CLI depende de core
        check(key() != base, "un cambio en una dependencia de la etapa tiene que invalidar su evidencia")
        ids["crates/cognicode-core"] = "core-1"
        check(key() == base, "las mismas entradas dan la misma clave")
        check(impact.evidence_key("cli-unit-tests", body + " --extra", {"cognicode-cli"}, "x", dirs, dependents) != base,
              "cambiar el cuerpo de la etapa invalida su evidencia")
        for producer in ("build-cognicode-for-contracts", "install-cosign", "toolchain"):
            check(impact.evidence_key(producer, "x", "always", "x", dirs, dependents) is None,
                  f"{producer} prepara entorno o produce binarios: nunca se reutiliza")
        check(impact.evidence_key("fmt", "cargo fmt", "always", "x", dirs, dependents) is not None,
              "un veredicto sobre el arbol entero se reutiliza si el arbol es el mismo")
    finally:
        impact.git_id = real


def test_only_a_recorded_pass_is_reused() -> None:
    import tempfile
    real = impact.ledger_dir
    with tempfile.TemporaryDirectory() as d:
        impact.ledger_dir = lambda: Path(d) / "ledger"
        try:
            check(impact.reused("k") is None, "sin PASS registrado no hay nada que reutilizar")
            impact.record("k", "clippy", "abc1234")
            got = impact.reused("k")
            check(bool(got) and got["sha"] == "abc1234", f"el PASS registrado se recupera: {got}")
        finally:
            impact.ledger_dir = real


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — impact: falla cerrado, sigue a los dependientes y su grafo de core ve >= 98 % de lo que ve el oraculo")
    return 0


if __name__ == "__main__":
    sys.exit(main())

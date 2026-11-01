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


def test_the_core_filter_finds_its_stage() -> None:
    hits = [s for s, b in impact.gate_stages() if impact.CORE_EK.search(b)]
    check(hits == ["core-unrestricted-evidence-kernel"],
          f"el filtro por modulo de core no encuentra su etapa ({hits}); sin el, core corre entero")


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

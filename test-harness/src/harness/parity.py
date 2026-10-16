"""Paridad con el oráculo CR-08: comparar, y no quedarse por debajo.

TH-S01 lo pide con estas palabras: «se preserva el selector CR-08 como oráculo de
compatibilidad mientras migra; comparar sus decisiones antes de cambiar la
cobertura». Este módulo hace esa comparación de forma mecánica y con la misma
vocabulario del oráculo:

  * CR-08 habla de *scopes* (`core`, `explorer`, `mcp`, `cli`, `ladybug`) o de
    `fallback` (= todo). Cada scope se traduce a paquetes.
  * El harness habla de obligaciones por suite. Un scope está *cubierto* si el
    plan exige alguna suite de esos paquetes; `check_only` si solo exige un
    `cargo fmt/clippy/build` (compila, pero no ejecuta la suite del scope).

Veredictos posibles, sin ambigüedad:

| Veredicto | Significado |
|---|---|
| `AGREE` | El harness exige al menos lo mismo que el oráculo |
| `HARNESS_STRICTER` | El harness exige más; el oráculo iba corto |
| `DECLARED_DEVIATION` | El harness exige menos en una clase donde la diferencia está declarada y razonada |
| `HARNESS_WEAKER` | El harness exige menos sin justificación: es un fallo, no un matiz |
| `ORACLE_UNKNOWN` | El oráculo no se pudo leer: no se puede afirmar paridad |

La desviación declarada no se decide por lista de paths inventada aquí: se
atribuye con **el motivo que el propio oráculo da** (`docs_changed(...)`), porque
una demanda que el oráculo explica como heurística de documentación es una
demanda que el harness puede refutar con evidencia (la clausura textual de citas)
siempre que lo diga en voz alta.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from adapters.cr08_selector import SelectorDecision
from adapters.pipeline_catalog import CatalogEntry

# Traducción de scope a paquete. Es 1:1 a propósito: CR-08 emite un nombre por
# suite que corre, y cuando su regla acopla dos crates (explorer→core,
# mcp→core) emite **los dos nombres**. Traducir `explorer` a (explorer, core)
# haría que un cambio que solo exige `core` pareciera cubrir `explorer`, que es
# justo la diferencia que esta comparación existe para detectar.
CR08_SCOPES: dict[str, tuple[str, ...]] = {
    "core": ("cognicode-core",),
    "explorer": ("cognicode-explorer",),
    "mcp": ("cognicode-mcp",),
    "cli": ("cognicode-cli",),
    "ladybug": ("cognicode-ladybug",),
}

# Demanda del oráculo que el harness puede refutar con evidencia, y con qué
# motivo. Cada entrada es `(prefijo del reason de CR-08, exige atribución, motivo)`.
# `exige atribución` significa que la desviación sólo vale si el harness puede
# demostrar que sabe más que el oráculo sobre ese path (lo sitúa en el grafo de
# cargo metadata); si no, es una debilidad y se declara como tal.
DECLARED_DEVIATIONS: tuple[tuple[str, bool, str], ...] = (
    (
        "docs_changed",
        False,
        "CR-08 exige `ladybug` para cualquier cambio en `docs/**` como heurística "
        "conservadora (lo documenta como cousin del vector e29/e30). El harness exige "
        "solo los paquetes y scripts que citan textualmente el fichero cambiado, que es "
        "una clausura comprobable; si nadie lo cita, no hay mecanismo por el que el "
        "cambio altere una suite",
    ),
    ("unmatched_slashed_rs", True, "El oráculo cae a ALL porque su lista de crates no menciona ese crate (regla "
        "histórica `*.rs` con barra). El harness sí lo sitúa en el grafo de cargo "
        "metadata y exige su clausura de dependientes, que es información más precisa "
        "sobre el mismo repositorio"),
    (
        "unknown_path",
        True,
        "El oráculo cae a ALL porque el path no está en su lista de reglas. El harness "
        "tiene una clasificación positiva y comprobable para esos ficheros: "
        "documentación de raíz de la lista verificada, con búsqueda de citas completa, o "
        "documentación bajo `docs/**` con esa misma búsqueda. Sin esa clasificación la "
        "caída del oráculo se respeta y la diferencia se reporta como debilidad",
    ),
    (
        "doc_or_sandbox_meta_changed",
        False,
        "CR-08 declara noop la documentación de raíz y `sandbox/**`; el harness trata la "
        "documentación de raíz como inerte solo si nadie la cita, y `sandbox/**` como "
        "global. La diferencia, cuando aparece, va en la dirección conservadora",
    ),
)

AGREE = "AGREE"
HARNESS_STRICTER = "HARNESS_STRICTER"
DECLARED_DEVIATION = "DECLARED_DEVIATION"
HARNESS_WEAKER = "HARNESS_WEAKER"
ORACLE_UNKNOWN = "ORACLE_UNKNOWN"
# El oráculo pide un scope que **ninguna** obligación de esta lane puede cubrir
# (su vocabulario es anterior al reparto de lanes: nombra `explorer`, y ninguna
# pipeline de merge ejecuta suites de explorer). No es una debilidad del harness:
# es una diferencia entre dos inventarios, y se reporta como tal en vez de
# convertirse en un hueco inventado.
NOT_IN_CATALOG = "NOT_IN_CATALOG"

# Corpus de clases de cambio con las que se mide la paridad. Cada entrada es un
# path real del repositorio y por qué está: no se compara "un caso cualquiera",
# se comparan las clases donde las dos decisiones pueden diferir (dependencias,
# infraestructura, documentación, rutas desconocidas y metadata).
CORPUS: tuple[tuple[str, str], ...] = (
    ("crates/cognicode-core/src/lib.rs", "paquete base: casi todos dependen de él"),
    ("crates/cognicode-cli/tests/cli_gate_coverage_contract.rs", "test de contrato dentro de un crate"),
    ("crates/cognicode-mcp/src/lib.rs", "crate que arrastra core en la regla de CR-08"),
    ("crates/cognicode-ladybug/src/lib.rs", "crate hoja con dependientes por camino transitivo"),
    ("crates/cognicode-runtime/src/lib.rs", "crate intermedio entre ladybug y mcp"),
    ("docs/roadmap/ROADMAP.md", "documentación citada por tests: heurística de CR-08 frente a clausura textual"),
    ("AGENTS.md", "documentación de raíz que varios tests citan"),
    ("README.md", "documentación de raíz que nadie afirma leer en un test"),
    ("Cargo.toml", "manifest del workspace: los dos exigen todo"),
    ("Cargo.lock", "lock: los dos exigen todo"),
    ("rust-toolchain.toml", "toolchain: los dos exigen todo"),
    ("scripts/ci/select-suites.sh", "infraestructura de CI: los dos exigen todo"),
    ("merge-gate.pipeline.kts", "definición del gate: los dos exigen todo"),
    ("sandbox/run.sh", "sandbox: CR-08 lo declara noop, el harness lo trata como global"),
    ("test-harness/src/harness/cli.py", "ruta que ningún paquete posee: los dos exigen todo"),
    (".github/workflows/pr-ci.yml", "ruta inexistente y desconocida para los dos"),
)


@dataclass(frozen=True)
class ParityRow:
    scope: str
    packages: tuple[str, ...]
    demanded_by_oracle: bool
    harness_how: str  # suite | check_only | none
    verdict: str

    def to_json(self) -> dict:
        return {
            "scope": self.scope,
            "packages": list(self.packages),
            "demanded_by_oracle": self.demanded_by_oracle,
            "harness_how": self.harness_how,
            "verdict": self.verdict,
        }


@dataclass
class ParityReport:
    decision: SelectorDecision
    rows: tuple[ParityRow, ...] = ()
    verdict: str = ORACLE_UNKNOWN
    deviations: tuple[str, ...] = ()
    missing: tuple[str, ...] = ()
    extra: tuple[str, ...] = ()
    untracked: tuple[str, ...] = ()
    notes: tuple[str, ...] = field(default_factory=tuple)

    def to_json(self) -> dict:
        return {
            "oracle": self.decision.to_json(),
            "verdict": self.verdict,
            "rows": [row.to_json() for row in self.rows],
            "deviations": list(self.deviations),
            "missing_scopes": list(self.missing),
            "extra_scopes": list(self.extra),
            "scopes_not_in_catalog": list(self.untracked),
            "notes": list(self.notes),
        }


def _harness_coverage(required: tuple[CatalogEntry, ...]) -> dict[str, str]:
    """Cómo cubre el harness cada paquete: suite, solo compilación, o nada."""
    coverage: dict[str, str] = {}
    for entry in required:
        if entry.scope == "workspace":
            # `cargo fmt --all` o `clippy --workspace` compilan y analizan todo,
            # pero no ejecutan la suite de ningún paquete.
            continue
        if not entry.package:
            continue
        current = coverage.get(entry.package)
        if entry.kind == "test":
            coverage[entry.package] = "suite"
        elif current != "suite":
            coverage[entry.package] = "check_only"
    workspace_checks = any(entry.scope == "workspace" for entry in required)
    return {"__workspace_checks__": "yes"} | coverage if workspace_checks else coverage


def declared_deviation_for(
    reason: str, *, attributable: bool = False
) -> str | None:
    """¿La diferencia está declarada, y por qué? Se atribuye por el motivo del oráculo.

    Una desviación que exige atribución no se concede por el texto del motivo: si
    el harness no puede demostrar que conoce mejor el path, no hay desviación.
    """
    for prefix, requires_attribution, explanation in DECLARED_DEVIATIONS:
        if not reason.startswith(prefix):
            continue
        if requires_attribution and not attributable:
            return None
        return f"{prefix}: {explanation}"
    return None


def compare(
    decision: SelectorDecision,
    *,
    required: tuple[CatalogEntry, ...],
    catalog: tuple[CatalogEntry, ...],
    attributable: bool = False,
) -> ParityReport:
    """Compara la decisión del oráculo con las obligaciones que exige el harness.

    `attributable` dice si el harness puede situar los paths cambiados en el grafo
    de cargo metadata, que es la condición para que una caída a ALL del oráculo
    cuente como desviación declarada en vez de como debilidad.
    """
    if not decision.ok:
        return ParityReport(
            decision=decision,
            verdict=ORACLE_UNKNOWN,
            notes=(decision.problem or "el oráculo no devolvió una decisión legible",),
        )

    coverage = _harness_coverage(required)
    workspace_checks = coverage.get("__workspace_checks__") == "yes"
    demanded = set(decision.suites)
    catalog_packages = {entry.package for entry in catalog if entry.package}
    catalog_test_packages = {
        entry.package for entry in catalog if entry.package and entry.kind == "test"
    }
    rows: list[ParityRow] = []
    missing: list[str] = []
    extra: list[str] = []
    untracked: list[str] = []

    for scope, packages in CR08_SCOPES.items():
        asked = decision.requires_everything or scope in demanded
        # Un scope solo es exigible en esta lane si **alguna obligación de test**
        # del catálogo pertenece a uno de sus paquetes. Si no, la demanda del
        # oráculo no es accionable aquí: sus suites viven en otra lane (explorer
        # solo corre en integración) y convertir eso en hueco sería inventar una
        # obligación que ninguna pipeline de esta lane ejecuta.
        coverable = any(package in catalog_test_packages for package in packages)
        if not coverable:
            if asked:
                untracked.append(scope)
            rows.append(
                ParityRow(
                    scope=scope,
                    packages=packages,
                    demanded_by_oracle=asked,
                    harness_how="not_in_catalog" if asked else "none",
                    verdict=NOT_IN_CATALOG if asked else AGREE,
                )
            )
            continue
        # Un scope de CR-08 es la unión de sus paquetes (`explorer` implica
        # explorer + core): está cubierto solo si lo están **todos**. Quedarse con
        # el primero convertiría un cambio de mcp en "core cubierto" y taparía
        # justo la diferencia que se quiere medir.
        hows = [coverage.get(package, "none") for package in packages]
        if all(state == "suite" for state in hows):
            how = "suite"
        elif all(state != "none" for state in hows):
            how = "check_only"
        else:
            how = "none"
        if how == "none" and asked:
            missing.append(scope)
        if how != "none" and not asked:
            extra.append(scope)
        if asked and how == "suite":
            verdict = AGREE
        elif asked:
            # Pedido por el oráculo y no ejecutado, o solo compilado: por debajo.
            verdict = HARNESS_WEAKER
        elif how != "none":
            verdict = HARNESS_STRICTER
        else:
            verdict = AGREE
        rows.append(
            ParityRow(
                scope=scope,
                packages=packages,
                demanded_by_oracle=asked,
                harness_how=how,
                verdict=verdict,
            )
        )

    if not missing:
        verdict = HARNESS_STRICTER if extra else AGREE
        deviations: tuple[str, ...] = ()
    else:
        reason = declared_deviation_for(decision.reason, attributable=attributable)
        if reason:
            verdict = DECLARED_DEVIATION
            deviations = (reason,)
        else:
            verdict = HARNESS_WEAKER
            deviations = ()

    # MEDIDO 2026-10-16. Un scope que el oráculo pide y que no está en el mapeo
    # (`CR08_SCOPES`) desaparecía del informe: no generaba fila, no entraba en
    # `untracked` y el veredicto decía AGREE mientras el harness no podía demostrar
    # nada sobre esa demanda. Un mapeo que no conoce una demanda no es una demanda
    # cubierta: se declara y cuenta como debilidad, no como silencio. En el
    # repositorio real esto no cambia nada (CR-08 sólo emite scopes mapeados), y su
    # valor es no tragarse un scope nuevo el día que el selector emita uno.
    unknown_scopes = sorted(
        scope
        for scope in demanded
        if scope not in CR08_SCOPES and not decision.requires_everything
    )
    for scope in unknown_scopes:
        untracked.append(scope)
        missing.append(scope)
        rows.append(
            ParityRow(
                scope=scope,
                packages=(),
                demanded_by_oracle=True,
                harness_how="not_in_catalog",
                verdict=NOT_IN_CATALOG,
            )
        )
    if unknown_scopes and not deviations:
        verdict = HARNESS_WEAKER if missing else verdict

    notes = []
    if decision.problem:
        notes.append(decision.problem)
    if untracked:
        notes.append(
            "el oráculo pide scopes que ninguna obligación de esta lane ejecuta "
            f"({', '.join(sorted(untracked))}): no se cuentan como debilidad, se "
            "declaran como diferencia de inventarios"
        )
    if decision.strategy == "noop" and extra:
        notes.append(
            "el oráculo declara noop y el harness sí exige obligaciones: la "
            "dirección de la diferencia no es la peligrosa"
        )
    return ParityReport(
        decision=decision,
        rows=tuple(rows),
        verdict=verdict,
        deviations=deviations,
        missing=tuple(missing),
        extra=tuple(extra),
        untracked=tuple(sorted(untracked)),
        notes=tuple(notes),
    )


def floor_entries(
    decision: SelectorDecision,
    *,
    required: tuple[CatalogEntry, ...],
    catalog: tuple[CatalogEntry, ...],
) -> tuple[tuple[CatalogEntry, str], ...]:
    """Obligaciones que el harness debe **añadir** para no quedar bajo el oráculo.

    Se usa cuando el plan corre con `--oracle cr08` en una lane donde el selector
    es la autoridad. No sustituye al juicio del harness: lo pisa hacia el lado
    conservador y deja escrito de dónde viene la obligación.
    """
    if not decision.ok:
        return ()
    already = {(entry.suite_id) for entry in required}
    demanded = set(decision.suites)
    if decision.requires_everything:
        packages = {entry.package for entry in catalog if entry.package}
    else:
        packages = set()
        for scope in demanded:
            packages.update(CR08_SCOPES.get(scope, ()))
    additions: list[tuple[CatalogEntry, str]] = []
    for entry in catalog:
        if entry.suite_id in already or entry.advisory:
            continue
        if entry.package in packages:
            additions.append(
                (
                    entry,
                    f"el oráculo CR-08 lo exige ({decision.reason or 'fallback'}) y el "
                    "harness no puede demostrar que sobre",
                )
            )
    return tuple(additions)

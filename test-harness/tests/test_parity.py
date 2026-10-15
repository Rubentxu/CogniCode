"""Paridad con el oráculo CR-08: comparar sin quedarse por debajo.

El valor de este módulo no está en que las dos decisiones coincidan: está en que
cuando no coinciden, la dirección de la diferencia y su motivo queden por escrito.
Un `HARNESS_WEAKER` sin desviación declarada no es un matiz, es un fallo, y el
comparador tiene que decirlo así.
"""

from __future__ import annotations

import tempfile
from pathlib import Path

from harness_support import commit_change, demo_repo  # noqa: F401 - ajusta sys.path

from adapters.cargo_workspace import graph_from_metadata
from adapters.cr08_selector import SelectorDecision, unknown
from adapters.pipeline_catalog import CatalogEntry
from harness.planner import refutable_fallback, select
from harness.parity import compare, declared_deviation_for, floor_entries

MCP_ENTRY = CatalogEntry(
    suite_id="cognicode-mcp:lib#-",
    package="cognicode-mcp",
    target="lib",
    argv=("cargo", "test", "-p", "cognicode-mcp", "--lib"),
    lanes=("change",),
    origin="merge-gate.pipeline.kts",
)

MCP_CORE_ENTRY = CatalogEntry(
    suite_id="cognicode-core:lib#-",
    package="cognicode-core",
    target="lib",
    argv=("cargo", "test", "-p", "cognicode-core", "--lib"),
    lanes=("change",),
    origin="merge-gate.pipeline.kts",
)


LADYBUG_ENTRY = CatalogEntry(
    suite_id="cognicode-ladybug:lib#-",
    package="cognicode-ladybug",
    target="lib",
    argv=("cargo", "test", "-p", "cognicode-ladybug", "--lib"),
    lanes=("change",),
    origin="merge-gate.pipeline.kts",
)


def _decision(**overrides) -> SelectorDecision:
    payload = dict(
        strategy="changed",
        reason="core_changed(crates/cognicode-core/src/lib.rs)",
        suites=("core",),
        ok=True,
    )
    payload.update(overrides)
    return SelectorDecision(**payload)


def test_agree_when_the_harness_covers_what_the_oracle_demands():
    report = compare(
        _decision(), required=(MCP_CORE_ENTRY,), catalog=(MCP_CORE_ENTRY,)
    )
    assert report.verdict == "AGREE", report.to_json()
    assert report.missing == ()
    assert [row.verdict for row in report.rows if row.scope == "core"] == ["AGREE"]


def test_stricter_when_the_harness_adds_scopes_the_oracle_did_not_ask_for():
    report = compare(
        _decision(suites=("core",), reason="core_changed(x)"),
        required=(MCP_CORE_ENTRY, MCP_ENTRY),
        catalog=(MCP_CORE_ENTRY, MCP_ENTRY),
    )
    assert report.verdict == "HARNESS_STRICTER", report.to_json()
    assert "mcp" in report.extra, report.extra


def test_weaker_when_the_oracle_asks_and_the_harness_does_nothing():
    report = compare(
        _decision(),
        required=(MCP_ENTRY,),
        catalog=(MCP_ENTRY, MCP_CORE_ENTRY),
    )
    assert report.verdict == "HARNESS_WEAKER", report.to_json()
    assert report.missing == ("core",)
    rows = {row.scope: row.verdict for row in report.rows}
    assert rows["core"] == "HARNESS_WEAKER"


def test_a_check_alone_is_not_a_suite():
    """`clippy` compila el crate pero no ejecuta su suite: por debajo del oráculo."""
    clippy = CatalogEntry(
        suite_id="workspace:cargo:clippy#-",
        package="",
        target="cargo:clippy",
        argv=("cargo", "clippy", "--workspace"),
        lanes=("change",),
        origin="merge-gate.pipeline.kts",
        scope="workspace",
        kind="check",
    )
    report = compare(_decision(), required=(clippy,), catalog=(clippy, MCP_CORE_ENTRY))
    assert report.verdict == "HARNESS_WEAKER", report.to_json()


def test_declared_deviations_only_apply_to_their_own_reason():
    docs = declared_deviation_for("docs_changed(docs/x.md)")
    assert docs and docs.startswith("docs_changed:")

    # La caída a ALL del oráculo solo se puede refutar con clasificación propia.
    assert declared_deviation_for("unmatched_slashed_rs(a.rs)", attributable=False) is None
    assert declared_deviation_for("unmatched_slashed_rs(a.rs)", attributable=True)
    assert declared_deviation_for("unknown_path(AGENTS.md)", attributable=False) is None
    assert declared_deviation_for("unknown_path(AGENTS.md)", attributable=True)

    # Un motivo que no está declarado nunca concede desviación.
    assert declared_deviation_for("workspace_manifest_changed(Cargo.toml)", attributable=True) is None
    assert declared_deviation_for("ci_script_changed(x)", attributable=True) is None


def test_docs_deviation_is_reported_with_its_reason():
    report = compare(
        _decision(suites=("ladybug",), reason="docs_changed(docs/roadmap/ROADMAP.md)"),
        required=(MCP_CORE_ENTRY,),
        catalog=(MCP_CORE_ENTRY, LADYBUG_ENTRY),
    )
    assert report.verdict == "DECLARED_DEVIATION", report.to_json()
    assert report.missing == ("ladybug",)
    assert report.deviations and "docs_changed" in report.deviations[0]


def test_unreadable_oracle_never_claims_parity():
    report = compare(unknown("no existe el selector"), required=(MCP_CORE_ENTRY,), catalog=(MCP_CORE_ENTRY,))
    assert report.verdict == "ORACLE_UNKNOWN", report.to_json()
    assert report.notes


def test_floor_entries_add_the_oracle_scopes_and_skip_advisory():
    advisory = CatalogEntry(
        suite_id="cognicode-core:e2e#-",
        package="cognicode-core",
        target="e2e",
        argv=("cargo", "test", "-p", "cognicode-core", "--test", "e2e"),
        lanes=("change",),
        origin="merge-gate.pipeline.kts",
        advisory=True,
    )
    additions = floor_entries(
        _decision(),
        required=(MCP_ENTRY,),
        catalog=(MCP_ENTRY, MCP_CORE_ENTRY, advisory),
    )
    added = {entry.suite_id for entry, _ in additions}
    assert added == {"cognicode-core:lib#-"}, added
    assert "CR-08" in additions[0][1]


def test_floor_entries_for_a_fallback_take_every_package():
    additions = floor_entries(
        _decision(strategy="fallback", suites=(), reason="unmatched_slashed_rs(a.rs)"),
        required=(MCP_ENTRY,),
        catalog=(MCP_ENTRY, MCP_CORE_ENTRY),
    )
    assert {entry.suite_id for entry, _ in additions} == {"cognicode-core:lib#-"}


def _metadata_with_mcp_and_core(root: Path) -> dict:
    def package(name: str, deps: list[dict]) -> dict:
        directory = root / "crates" / name
        return {
            "name": name,
            "manifest_path": str(directory / "Cargo.toml"),
            "targets": [{"name": name, "kind": ["lib"], "src_path": str(directory / "src" / "lib.rs")}],
            "dependencies": deps,
        }

    return {
        "packages": [
            package("cognicode-core", []),
            package(
                "cognicode-mcp",
                [{"name": "cognicode-core", "path": str(root / "crates" / "cognicode-core")}],
            ),
        ]
    }


def test_the_harness_respects_the_oracle_coupling_between_mcp_and_core():
    """CR-08 acopla mcp con core: un cambio en el consumidor revalida el proveedor."""
    with demo_repo() as repo:
        graph = graph_from_metadata(repo, _metadata_with_mcp_and_core(repo))
        mcp_path = "crates/cognicode-mcp/src/lib.rs"
        changeset = _changeset(repo, mcp_path)
        selection = select(
            lane="change",
            changeset=changeset,
            graph=graph,
            catalog=(MCP_ENTRY, MCP_CORE_ENTRY),
            repo_root=repo,
        )
        required = {entry.suite_id: reason for entry, reason in selection.required}
        assert set(required) == {"cognicode-mcp:lib#-", "cognicode-core:lib#-"}, required
        assert "acopla" in required["cognicode-core:lib#-"], required


def test_refutable_fallback_requires_a_positive_classification():
    with demo_repo() as repo:
        graph = graph_from_metadata(repo, _metadata_with_mcp_and_core(repo))
        assert refutable_fallback(_changeset(repo, "crates/cognicode-core/src/lib.rs"), graph) is True
        assert refutable_fallback(_changeset(repo, "docs/guia.md"), graph) is True
        assert refutable_fallback(_changeset(repo, "AGENTS.md"), graph) is True
        # Una ruta que el harness tampoco sabe clasificar: no puede refutar nada.
        assert refutable_fallback(_changeset(repo, "lo-que-sea/otro.txt"), graph) is False


def _real_context(repo: Path):
    from adapters.cargo_workspace import load_graph
    from adapters.pipeline_catalog import catalog_for_lane, read_catalog

    return load_graph(repo, offline=True), catalog_for_lane(read_catalog(repo), "change")


def _changeset(repo: Path, path: str):
    from adapters.git_repo import ChangeSet

    return ChangeSet(
        repo_root=repo,
        base_ref=None,
        merge_base=None,
        head="0" * 40,
        changed=(path,),
        dirty=True,
    )


def test_incomplete_citation_scan_does_not_declare_documentation_inert():
    """Un barrido incompleto que dice «nadie lo cita» es un falso negativo caro."""
    from harness import planner

    with demo_repo() as repo:
        graph, catalog = _real_context(repo)
        changeset = _changeset(repo, "docs/guia.md")
        original = planner.DOCS_SCAN_BYTES_CAP
        planner.DOCS_SCAN_BYTES_CAP = 1
        try:
            selection = select(
                lane="change",
                changeset=changeset,
                graph=graph,
                catalog=catalog,
                repo_root=repo,
            )
        finally:
            planner.DOCS_SCAN_BYTES_CAP = original

        assert selection.unknown_reasons, selection
        assert "no se pudo completar" in selection.unknown_reasons[0]
        assert {entry.suite_id for entry, _ in selection.required} == {
            entry.suite_id for entry in catalog
        }


def test_citations_are_attributed_to_every_package_that_mentions_the_file():
    """Quedarse con la primera cita perdía paquetes: la atribución es por paquete."""
    with demo_repo() as repo:
        graph, catalog = _real_context(repo)
        # El fixture ya tiene un test de `core` que cita `docs/guia.md`; se añade
        # una cita en `app` y se comprueba que aparecen los dos, no solo el primero.
        commit_change(repo, "crates/app/tests/referencia.rs", "// ver docs/guia.md\n")
        selection = select(
            lane="change",
            changeset=_changeset(repo, "docs/guia.md"),
            graph=graph,
            catalog=catalog,
            repo_root=repo,
        )
        required = {entry.suite_id for entry, _ in selection.required}
        assert "demo-core:lib#-" in required, required
        assert "demo-app:lib#-" in required, required


def test_a_scope_no_lane_runs_is_not_a_weakness_it_is_a_declared_difference():
    """`explorer` solo corre en integración: en esta lane no es accionable."""
    report = compare(
        _decision(strategy="fallback", suites=(), reason="unknown_path(x)"),
        required=(MCP_CORE_ENTRY,),
        catalog=(MCP_CORE_ENTRY,),
    )
    rows = {row.scope: row.verdict for row in report.rows}
    assert rows["explorer"] == "NOT_IN_CATALOG", rows
    assert "explorer" in report.untracked, report.to_json()
    assert "explorer" not in report.missing, report.missing
    assert report.notes, report.notes


def test_temporary_directory_is_usable_in_these_tests():
    """Guardia barata: las pruebas usan directorios temporales de verdad."""
    with tempfile.TemporaryDirectory(prefix="th-parity-") as tmp:
        assert Path(tmp).is_dir()

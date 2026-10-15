"""Selección: qué obligaciones exige un cambio, y con qué motivo.

Es la parte donde un atajo se convierte en un gate que aprueba sin mirar, así que
los casos que importan son los negativos: una ruta desconocida **no** puede
reducir obligaciones, y un borrado de test **no** puede pasar por "no cambió nada".
"""

from __future__ import annotations

from pathlib import Path

from adapters.cargo_workspace import GRAPH_UNKNOWN, WorkspaceGraph, load_graph
from adapters.git_repo import read_changeset
from adapters.pipeline_catalog import CatalogEntry, catalog_for_lane, read_catalog
from harness.planner import ToolchainInfo, build_plan, select
from harness_support import APP_SUITE, CORE_SUITE, FMT_SUITE, commit_change, demo_repo, git

PLAN_TIME = "2026-01-01T00:00:00Z"


def _context(repo: Path):
    graph = load_graph(repo, offline=True)
    catalog = catalog_for_lane(read_catalog(repo), "change")
    return graph, catalog


def _selection(repo: Path, graph, catalog, base: str = "main"):
    changeset = read_changeset(repo, base)
    return select(lane="change", changeset=changeset, graph=graph, catalog=catalog, repo_root=repo)


def _plan(repo: Path, graph, catalog):
    return build_plan(
        lane="change",
        repo_id="demo",
        changeset=read_changeset(repo, "main"),
        graph=graph,
        catalog=catalog,
        ledger=None,
        toolchain=ToolchainInfo.read(repo),
        run_id="run-test",
        created_at=PLAN_TIME,
    )


def _required_ids(selection) -> set[str]:
    return {entry.suite_id for entry, _ in selection.required}


def _reason_for(selection, suite_id: str) -> str:
    for entry, reason in (*selection.required, *selection.excluded):
        if entry.suite_id == suite_id:
            return reason
    raise AssertionError(f"{suite_id} no aparece ni en requeridas ni en excluidas")


def test_catalog_reads_the_pipeline():
    with demo_repo() as repo:
        catalog = read_catalog(repo)
        by_id = {entry.suite_id: entry for entry in catalog}
        assert {CORE_SUITE, APP_SUITE, FMT_SUITE} <= set(by_id), sorted(by_id)
        assert by_id[CORE_SUITE].argv == ("cargo", "test", "-p", "demo-core", "--lib", "--quiet")
        assert by_id[FMT_SUITE].scope == "workspace"
        assert by_id[FMT_SUITE].kind == "check"


def test_core_change_pulls_in_its_dependents():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert "depende" in _reason_for(selection, APP_SUITE), _reason_for(selection, APP_SUITE)


def test_change_in_one_package_leaves_the_others_out():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// cambio en app\n")

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == {APP_SUITE, FMT_SUITE}, selection
        assert "ningún fichero de su clausura cambió" in _reason_for(selection, CORE_SUITE)


def test_docs_change_alone_selects_nothing():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "docs/otro.md", "# otro\n")

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == set(), selection
        assert "no cita" in _reason_for(selection, CORE_SUITE), _reason_for(selection, CORE_SUITE)
        assert "no hubo cambios de código" in _reason_for(selection, FMT_SUITE)


def test_docs_change_cited_by_a_package_selects_that_package():
    """Solo docs: el paquete que cita el fichero entra, el que no lo cita no."""
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "docs/guia.md", "# guía actualizada\n")

        selection = _selection(repo, graph, catalog)

        assert CORE_SUITE in _required_ids(selection), selection
        assert APP_SUITE not in _required_ids(selection), selection
        assert "cita el doc cambiado" in _reason_for(selection, CORE_SUITE)


def test_manifest_change_forces_everything():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(
            repo,
            "Cargo.toml",
            '[workspace]\nresolver = "2"\nmembers = ["crates/core", "crates/app"]\n',
        )

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert any("Cargo.toml" in reason for reason in selection.global_reasons), selection


def test_unknown_path_forces_everything():
    """Una ruta que ningún paquete posee no puede reducir obligaciones."""
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "test-harness/notas.md", "# notas\n")

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert any("no pertenece a ningún paquete" in reason for reason in selection.global_reasons)


def test_deleted_test_file_forces_everything():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "crates/core/tests/desaparece.rs", "#[test]\nfn t() {}\n")
        (repo / "crates" / "core" / "tests" / "desaparece.rs").unlink()
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "borra el test")

        selection = _selection(repo, graph, catalog)

        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert any("se eliminó un fichero de test" in reason for reason in selection.global_reasons)


def test_advisory_obligations_do_not_block():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        advisory = CatalogEntry(
            suite_id="demo-core:e2e#-",
            package="demo-core",
            target="e2e",
            argv=("cargo", "test", "-p", "demo-core", "--test", "e2e"),
            lanes=("change",),
            origin="product-fast.pipeline.kts",
            advisory=True,
        )
        selection = select(
            lane="change",
            changeset=read_changeset(repo, "main"),
            graph=graph,
            catalog=(*catalog, advisory),
            repo_root=repo,
        )
        assert advisory.suite_id not in _required_ids(selection), selection
        assert "advisory" in _reason_for(selection, advisory.suite_id)


def test_unknown_inputs_force_the_conservative_fallback():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        changeset = read_changeset(repo, None)
        selection = select(
            lane="change", changeset=changeset, graph=graph, catalog=catalog, repo_root=repo
        )
        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert selection.unknown_reasons
        assert selection.fallback_reason


def test_external_suites_are_blocked_not_silently_skipped():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        sandbox = CatalogEntry(
            suite_id="workspace:script:run-sandbox-uat",
            package="",
            target="script:sandbox/run.sh",
            argv=("bash", "sandbox/run.sh"),
            lanes=("change",),
            origin="product-fast.pipeline.kts",
            scope="script",
            kind="script",
        )
        commit_change(repo, "sandbox/run.sh", "echo sandbox\n")

        plan_result = _plan(repo, graph, (*catalog, sandbox))

        actions = {action.suite_id: action for action in plan_result.plan.actions}
        assert actions[sandbox.suite_id].action == "BLOCK", actions[sandbox.suite_id]
        assert "estado externo" in actions[sandbox.suite_id].reason
        assert plan_result.plan.status == "BLOCKED", plan_result.plan.status


def test_plan_is_reproducible_for_the_same_inputs():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio estable\n")
        toolchain = ToolchainInfo.read(repo)

        def _plan_twice():
            return build_plan(
                lane="change",
                repo_id="demo",
                changeset=read_changeset(repo, "main"),
                graph=graph,
                catalog=catalog,
                ledger=None,
                toolchain=toolchain,
                run_id="run-test",
                created_at=PLAN_TIME,
            )

        first, second = _plan_twice(), _plan_twice()
        assert first.plan.required_suites == second.plan.required_suites
        assert first.plan.source_fingerprint == second.plan.source_fingerprint
        assert [s.fingerprint.digest() for s in first.suites] == [
            s.fingerprint.digest() for s in second.suites
        ]
        assert [a.to_json() for a in first.plan.actions] == [
            a.to_json() for a in second.plan.actions
        ]


def test_touching_a_suite_source_invalidates_only_its_fingerprint():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        toolchain = ToolchainInfo.read(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// cambio en app\n")

        def _fingerprints():
            plan_result = build_plan(
                lane="change",
                repo_id="demo",
                changeset=read_changeset(repo, "main"),
                graph=graph,
                catalog=catalog,
                ledger=None,
                toolchain=toolchain,
                run_id="run-test",
                created_at=PLAN_TIME,
            )
            return {suite.entry.suite_id: suite.fingerprint.digest() for suite in plan_result.suites}

        before = _fingerprints()
        commit_change(repo, "crates/app/src/lib.rs", "// cambio distinto en app\n")
        after = _fingerprints()

        assert before[APP_SUITE] != after[APP_SUITE], (before, after)


def test_graph_unknown_forces_everything():
    with demo_repo() as repo:
        graph, catalog = _context(repo)
        unknown = WorkspaceGraph(root=Path(repo), status=GRAPH_UNKNOWN, reason="metadata ilegible")
        selection = _selection(repo, unknown, catalog)
        assert _required_ids(selection) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, selection
        assert "metadata ilegible" in selection.unknown_reasons[0]

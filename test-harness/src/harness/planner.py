"""Planificador: qué hay que ejecutar, por qué, y qué se puede reutilizar.

El plan es un documento con razones, no una lista de comandos. Cada obligación
lleva su motivo (`cambio en X`, `dependiente de Y`, `cambio global en Z`) y, si no
está en el plan, el motivo de su exclusión vive en el mismo objeto. Si algo se
queda fuera, se puede discutir con el plan delante; si el plan fuera una lista
pelada, la única defensa sería la confianza.

Tres reglas de fail-closed que gobiernan la selección:

  1. Sin ChangeSet fiable (sin base, sin merge-base) → **todo** es obligatorio.
  2. Sin grafo de paquetes → **todo** es obligatorio.
  3. Un cambio en infraestructura (manifests, lock, toolchain, pipelines,
     `scripts/ci/`) → **todo** es obligatorio. No existe una clausura fina que
     demuestre lo contrario, y fingirla sería seleccionar de menos.
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from .domain import ExecutionPlan, PlanAction, SuiteRef
from .inventory import Inventory
from .parity import ParityReport, compare, floor_entries
from .policy import POLICY_VERSION, reuse_allowed
from .validity import FingerprintInputs, evaluate_reuse

from adapters.cargo_workspace import WorkspaceGraph, is_global_path
from adapters.cr08_selector import SelectorDecision
from adapters.git_repo import METADATA_ONLY_NAMES, ChangeSet
from adapters.ledger_store import Ledger
from adapters.pipeline_catalog import CatalogEntry

SKIP_DIRS = {"target", ".git", "node_modules", "__pycache__", ".venv", "venv", "dist", "build"}
CLOSURE_FILE_CAP = 4000

# Sólo se leen ficheros de texto: buscar una cita en un binario o en un artefacto
# de build no aporta nada y consume el presupuesto que sí importa.
TEXT_EXTENSIONS = (
    ".rs",
    ".py",
    ".sh",
    ".toml",
    ".kts",
    ".json",
    ".yaml",
    ".yml",
    ".js",
    ".ts",
    ".md",
    ".txt",
    ".cfg",
    ".ini",
)
MAX_SCANNED_FILE_BYTES = 512 * 1024
DOCS_SCAN_BYTES_CAP = 64 << 20
# Tope de citas por documento. Quedarse con la primera era un error caro: la
# atribución por paquete necesita saber **todos** los paquetes que citan el
# fichero, no el primero que aparece en el orden del recorrido.
MAX_CITATIONS_PER_DOC = 500

# Acoplamientos que CR-08 declara explícitamente y que no son dependencias:
# `mcp` consume el contrato de `core` y `explorer` recalibra sus constraints por
# codegen, así que el selector vigente revalida `core` cuando tocan a sus
# consumidores. El harness respeta esa regla mientras no demuestre lo contrario;
# sin ella el plan ignoraría una obligación que el gate de hoy sí exige.
ORACLE_COUPLINGS: dict[str, tuple[str, ...]] = {
    "cognicode-explorer": ("cognicode-core",),
    "cognicode-mcp": ("cognicode-core",),
}


@dataclass(frozen=True)
class ToolchainInfo:
    rustc_version: str
    cargo_version: str
    target_triple: str

    @staticmethod
    def read(repo_root: Path) -> "ToolchainInfo":
        def run(argv: list[str]) -> str:
            try:
                out = subprocess.run(
                    argv, cwd=str(repo_root), capture_output=True, check=False, timeout=60
                )
            except (OSError, subprocess.TimeoutExpired) as exc:
                return f"unknown ({exc})"
            return out.stdout.decode("utf-8", "replace").strip() or "unknown"

        rustc = run(["rustc", "-vV"])
        version = next((line.split(":", 1)[1].strip() for line in rustc.splitlines() if line.startswith("release:")), "unknown")
        host = next((line.split(":", 1)[1].strip() for line in rustc.splitlines() if line.startswith("host:")), "unknown")
        cargo = run(["cargo", "--version"])
        return ToolchainInfo(rustc_version=version, cargo_version=cargo, target_triple=host)


@dataclass(frozen=True)
class Closure:
    files: tuple[str, ...]
    digest: str
    truncated: bool = False


@dataclass
class Selection:
    required: tuple[tuple[CatalogEntry, str], ...] = ()
    excluded: tuple[tuple[CatalogEntry, str], ...] = ()
    fallback_reason: str | None = None
    unknown_reasons: tuple[str, ...] = ()
    global_reasons: tuple[str, ...] = ()


@dataclass
class PlannedSuite:
    entry: CatalogEntry
    reason: str
    fingerprint: FingerprintInputs
    action: str
    action_reason: str
    receipt_id: str | None = None
    blocked: bool = False


@dataclass
class PlanResult:
    plan: ExecutionPlan
    selection: Selection
    suites: tuple[PlannedSuite, ...] = ()
    toolchain: ToolchainInfo | None = None
    changeset: ChangeSet | None = None
    warnings: tuple[str, ...] = field(default_factory=tuple)
    parity: ParityReport | None = None
    oracle: SelectorDecision | None = None

    def minimums(self) -> dict[str, int]:
        """Mínimo de casos exigido por suite (del pin, o el suelo de Cargo)."""
        return {suite.entry.suite_id: suite.fingerprint.expected_min_cases for suite in self.suites}


def _walk_files(root: Path, subdir: str, cap: int = CLOSURE_FILE_CAP) -> tuple[list[str], bool]:
    """Ficheros de un subárbol, en orden estable, saltando artefactos de build."""
    base = root / subdir
    files: list[str] = []
    truncated = False
    if not base.exists():
        return files, truncated
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        for name in sorted(filenames):
            path = Path(dirpath) / name
            rel = str(path.relative_to(root))
            files.append(rel)
            if len(files) >= cap:
                return files, True
    return files, truncated


WORKSPACE_LEVEL_FILES = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".cargo/config.toml",
)


def closure_for(entry: CatalogEntry, *, repo_root: Path, graph: WorkspaceGraph) -> Closure:
    """Ficheros que pueden cambiar el resultado de esta obligación.

    Para una suite de paquete: todo su subárbol (fuentes, tests, fixtures,
    `build.rs`) más los ficheros de nivel workspace que gobiernan la compilación.
    Para una obligación de script: el propio script.
    """
    files: list[str] = []
    truncated = False
    if entry.scope == "workspace":
        # `cargo clippy --workspace` o `cargo fmt --all`: su clausura es todo el
        # workspace, y cualquier byte de cualquier crate la invalida.
        for member in graph.members:
            walked, cut = _walk_files(repo_root, member.dir)
            files.extend(walked)
            truncated = truncated or cut
    elif entry.package:
        member_dir = next((m.dir for m in graph.members if m.name == entry.package), None)
        if member_dir:
            walked, truncated = _walk_files(repo_root, member_dir)
            files.extend(walked)
    elif entry.target.startswith("script:"):
        script = entry.target.split(":", 1)[1]
        if (repo_root / script).exists():
            files.append(script)
    for candidate in WORKSPACE_LEVEL_FILES:
        if (repo_root / candidate).exists():
            files.append(candidate)
    ordered = tuple(sorted(set(files)))
    return Closure(files=ordered, digest=_digest_files(repo_root, ordered), truncated=truncated)


def _digest_files(repo_root: Path, files: tuple[str, ...]) -> str:
    import hashlib

    digest = hashlib.sha256()
    for rel in files:
        digest.update(rel.encode("utf-8", "surrogateescape"))
        digest.update(b"\0")
        path = repo_root / rel
        try:
            if path.is_file():
                with path.open("rb") as handle:
                    for chunk in iter(lambda: handle.read(1 << 20), b""):
                        digest.update(chunk)
            else:
                digest.update(b"<absent>")
        except OSError:
            digest.update(b"<unreadable>")
        digest.update(b"\n")
    return digest.hexdigest()


def _file_digest(path: Path) -> str:
    import hashlib

    if not path.exists():
        return "absent"
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def docs_references(
    changed_docs: tuple[str, ...], roots: tuple[Path, ...], *, max_bytes: int | None = None
) -> tuple[tuple[tuple[str, str], ...], bool]:
    """¿Alguna fuente bajo `roots` menciona textualmente un fichero cambiado?

    Devuelve `(citas, completa)`. `completa=False` significa que el barrido se
    agotó antes de terminar, y quien lo llama **no puede** declarar inerte la
    documentación: una búsqueda incompleta que dice «nadie lo cita» es un falso
    negativo caro (el harness llegó a producir uno: el presupuesto se agotaba a la
    mitad del árbol y `AGENTS.md` aparecía como no citado por nadie).

    Es una clausura textual explícita, no una corazonada: si un test o un script
    cita `docs/roadmap/ROADMAP.md` (o `AGENTS.md`), tocar ese fichero puede
    cambiar su resultado; si nadie lo cita, no hay mecanismo por el que lo
    cambie. Se busca el path relativo completo y el nombre base, que son las dos
    formas en que un fichero se referencia de verdad.

    El límite es conocido y se declara: una cita construida por concatenación de
    cadenas no se detecta, y por eso el conjunto de ficheros que se tratan como
    potencialmente inertes es una lista corta y explícita, no un patrón.
    """
    if not changed_docs:
        return (), True
    needles = {doc: {doc, Path(doc).name} for doc in changed_docs}
    found: dict[str, set[str]] = {doc: set() for doc in changed_docs}
    budget = DOCS_SCAN_BYTES_CAP if max_bytes is None else max_bytes
    for root in roots:
        if not root.exists():
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
            for name in sorted(filenames):
                path = Path(dirpath) / name
                if path.suffix not in TEXT_EXTENSIONS:
                    continue
                try:
                    if path.stat().st_size > MAX_SCANNED_FILE_BYTES:
                        continue
                    data = path.read_bytes()
                except OSError:
                    continue
                budget -= len(data)
                if budget <= 0:
                    return tuple(
                        (doc, ",".join(sorted(found[doc]))) for doc in changed_docs if found[doc]
                    ), False
                text = data.decode("utf-8", "replace")
                for doc, variants in needles.items():
                    if len(found[doc]) >= MAX_CITATIONS_PER_DOC:
                        continue
                    for variant in variants:
                        if variant in text:
                            found[doc].add(str(path))
                            break
    return (
        tuple((doc, ",".join(sorted(found[doc]))) for doc in changed_docs if found[doc]),
        True,
    )


def select(
    *,
    lane: str,
    changeset: ChangeSet,
    graph: WorkspaceGraph,
    catalog: tuple[CatalogEntry, ...],
    repo_root: Path,
) -> Selection:
    """Obligaciones de la lane que este cambio obliga a ejecutar, con motivos."""
    selection = Selection()
    if changeset.is_unknown:
        reason = changeset.fallback_reason or "ChangeSet desconocido"
        selection.fallback_reason = reason
        selection.unknown_reasons = (reason,)
        selection.required = tuple((entry, f"fallback conservador: {reason}") for entry in catalog)
        return selection

    if graph.is_unknown:
        reason = graph.reason or "grafo de paquetes desconocido"
        selection.fallback_reason = reason
        selection.unknown_reasons = (reason,)
        selection.required = tuple((entry, f"fallback conservador: {reason}") for entry in catalog)
        return selection

    paths = changeset.all_paths()
    global_reasons: list[str] = []
    docs_paths: list[str] = []
    inert_paths: list[str] = []
    package_paths: list[str] = []
    for path in paths:
        is_global, why = is_global_path(path)
        if is_global:
            global_reasons.append(f"{path}: {why}")
        elif path.startswith("docs/"):
            docs_paths.append(path)
        elif "/" not in path and Path(path).name in METADATA_ONLY_NAMES:
            # Documentación de raíz: inerte **si nadie la cita**. La cita se
            # comprueba abajo, sobre las fuentes de cada paquete y los scripts.
            inert_paths.append(path)
        elif graph.owner_of(path) is None:
            global_reasons.append(
                f"{path}: no pertenece a ningún paquete del workspace, y el harness "
                "no puede demostrar que su clausura sea inocua"
            )
        else:
            package_paths.append(path)

    if changeset.deleted:
        for path in changeset.deleted:
            if path.startswith("crates/") and "/tests/" in path:
                global_reasons.append(
                    f"{path}: se eliminó un fichero de test, y con él su cobertura"
                )
                break

    if global_reasons:
        selection.global_reasons = tuple(global_reasons)
        selection.required = tuple(
            (entry, f"cambio global: {global_reasons[0]}") for entry in catalog
        )
        return selection

    owners = {owner for owner in (graph.owner_of(path) for path in package_paths) if owner}
    affected = graph.reverse_closure(owners) if owners else set()

    # Un solo barrido de las citas: docs cambiados **más** la documentación de
    # raíz candidata a inerte. Se consulta por paquete y por script, y hacerlo una
    # vez evita releer el mismo árbol decenas de veces.
    cited_paths = tuple(sorted([*docs_paths, *inert_paths]))
    citations: tuple[tuple[str, str], ...] = ()
    if cited_paths:
        roots = [repo_root / member.dir for member in graph.members]
        scripts_dir = repo_root / "scripts"
        if scripts_dir.exists():
            roots.append(scripts_dir)
        citations, complete = docs_references(cited_paths, tuple(roots))
        if not complete:
            # Una búsqueda incompleta que dice «nadie lo cita» es el peor resultado
            # posible: parece una prueba y no lo es. Se cae al fallback conservador.
            reason = (
                "la búsqueda de citas sobre "
                f"{len(cited_paths)} fichero(s) de documentación no se pudo completar "
                f"({DOCS_SCAN_BYTES_CAP >> 20} MB de presupuesto agotados): sin esa prueba no "
                "se puede declarar inerte la documentación tocada"
            )
            selection.fallback_reason = reason
            selection.unknown_reasons = (reason,)
            selection.required = tuple(
                (entry, f"fallback conservador: {reason}") for entry in catalog
            )
            return selection

    def _citation_for(directory: str | None) -> str | None:
        if not directory:
            return None
        prefix = str(repo_root / directory) + os.sep
        for cited, where in citations:
            if any(path.startswith(prefix) for path in where.split(",") if path):
                return cited
        return None

    def _docs_hit_for(package: str) -> str | None:
        return _citation_for(_package_dir(graph, package))

    required: list[tuple[CatalogEntry, str]] = []
    excluded: list[tuple[CatalogEntry, str]] = []
    coupled: dict[str, str] = {}
    for owner in sorted(owners):
        for coupled_package in ORACLE_COUPLINGS.get(owner, ()):
            coupled[coupled_package] = owner
    for entry in catalog:
        if entry.advisory:
            # El gate la deja fallar a propósito (`|| echo ADVISORY`). Copiar esa
            # semántica es más honesto que inventar una obligación que PipelineK
            # no exige, y evita poblar el veredicto con huecos que nadie promete.
            excluded.append(
                (
                    entry,
                    "el gate la declara advisory: su fallo no bloquea, así que no exige prueba",
                )
            )
            continue
        if entry.package and entry.package in affected:
            why = _reason_for(entry, owners, affected, graph, changeset)
            required.append((entry, why))
            continue
        if entry.package in coupled:
            required.append(
                (
                    entry,
                    f"CR-08 acopla {coupled[entry.package]} con {entry.package}: un cambio en el "
                    "consumidor obliga a revalidar el contrato del proveedor",
                )
            )
            continue
        if entry.scope == "workspace":
            # Un `cargo fmt --all` o un `clippy --workspace` cubren todos los
            # paquetes: en cuanto toca cualquier código, la obligación está viva.
            if package_paths:
                required.append(
                    (
                        entry,
                        f"{entry.target} es de alcance workspace y el cambio toca "
                        f"{len(package_paths)} fichero(s) de paquete",
                    )
                )
                continue
            excluded.append((entry, f"{entry.target} es de alcance workspace y no hubo cambios de código"))
            continue
        if entry.package:
            hit = _docs_hit_for(entry.package)
            if hit:
                required.append(
                    (
                        entry,
                        f"su paquete cita el doc cambiado {hit}: un cambio ahí puede mover su resultado",
                    )
                )
                continue
        if entry.scope == "script":
            script_rel = entry.target.split(":", 1)[1] if entry.target.startswith("script:") else ""
            cited = next(
                (
                    cited_name
                    for cited_name, where in citations
                    if script_rel and script_rel in where.split(",")
                ),
                None,
            )
            if cited:
                required.append(
                    (entry, f"su script cita el fichero cambiado {cited}: puede leerlo")
                )
                continue
            excluded.append(
                (entry, "su script no cambió ni cita la documentación modificada")
            )
            continue
        detail = "ningún fichero de su clausura cambió"
        if docs_paths:
            detail += "; solo cambió documentación que su paquete no cita"
        excluded.append((entry, detail))

    selection.required = tuple(sorted(required, key=lambda item: item[0].suite_id))
    selection.excluded = tuple(sorted(excluded, key=lambda item: item[0].suite_id))
    return selection


def _package_dir(graph: WorkspaceGraph, package: str) -> str | None:
    return next((m.dir for m in graph.members if m.name == package), None)


def _reason_for(
    entry: CatalogEntry,
    owners: set[str],
    affected: set[str],
    graph: WorkspaceGraph,
    changeset: ChangeSet,
) -> str:
    if entry.package in owners:
        return (
            f"cambió el paquete {entry.package} "
            f"({_count_paths_for(graph, changeset, entry.package)} ficheros tocados)"
        )
    direct = sorted(set(graph.dependencies.get(entry.package, ())) & owners)
    if direct:
        return f"depende de {', '.join(direct)}, cuyo paquete cambió"
    dependents = graph.dependents.get(entry.package, ())
    if dependents:
        return f"depende (transitivamente) de un paquete tocado; cadena vía {', '.join(dependents)}"
    return "pertenece a la clausura de dependientes de un paquete tocado"


def _count_paths_for(graph: WorkspaceGraph, changeset: ChangeSet, package: str) -> int:
    member_dir = _package_dir(graph, package)
    if not member_dir:
        return 0
    return sum(1 for path in changeset.all_paths() if path.startswith(member_dir.rstrip("/") + "/"))


def build_plan(
    *,
    lane: str,
    repo_id: str,
    changeset: ChangeSet,
    graph: WorkspaceGraph,
    catalog: tuple[CatalogEntry, ...],
    ledger: Ledger | None,
    toolchain: ToolchainInfo,
    allow_reuse: bool = False,
    allow_external: bool = False,
    oracle: SelectorDecision | None = None,
    inventory: Inventory | None = None,
    run_id: str,
    created_at: str,
) -> PlanResult:
    """Construye el plan: selección, huellas, y decisión de reutilizar por suite.

    Con `oracle`, el plan nunca exige menos que el selector vigente: si CR-08 pide
    un scope que la selección por impacto no cubre, se añade con ese motivo. Si el
    oráculo no se puede leer, el plan exige todo y queda indeterminado, porque
    entonces no hay forma de demostrar que no se está por debajo.
    """
    repo_root = changeset.repo_root
    inventory = inventory or Inventory(entries={})
    selection = select(
        lane=lane, changeset=changeset, graph=graph, catalog=catalog, repo_root=repo_root
    )
    if oracle is not None:
        selection = _apply_oracle(selection, oracle, catalog=catalog)
    lane_ok, lane_reason = reuse_allowed(lane, allow_reuse)

    planned: list[PlannedSuite] = []
    actions: list[PlanAction] = []
    lock_digest = _file_digest(repo_root / "Cargo.lock")
    toolchain_digest = _digest_files(
        repo_root, tuple(f for f in WORKSPACE_LEVEL_FILES if (repo_root / f).exists())
    )

    for entry, reason in selection.required:
        closure = closure_for(entry, repo_root=repo_root, graph=graph)
        features = _features_of(entry)
        runner_name = "cargo-test" if entry.argv[:2] == ("cargo", "test") else "script"
        fingerprint = FingerprintInputs(
            suite_id=entry.suite_id,
            argv=entry.argv,
            features=features,
            target_triple=toolchain.target_triple,
            runner_name=runner_name,
            runner_version=(
                toolchain.cargo_version if runner_name == "cargo-test" else "shell"
            ),
            rustc_version=toolchain.rustc_version,
            cargo_lock_digest=lock_digest,
            toolchain_digest=toolchain_digest,
            closure_digest=closure.digest,
            # El mínimo sale del pin versionado cuando existe; si no, Cargo exige
            # al menos un caso y los scripts ninguno (no cuentan casos).
            expected_min_cases=(
                inventory.expected_min_for(entry.suite_id)
                if inventory.expected_min_for(entry.suite_id) > 0
                else (1 if runner_name == "cargo-test" else 0)
            ),
        )
        external = _needs_external_state(entry)
        receipt = None
        integrity_ok = True
        if ledger is not None:
            receipt, problem = ledger.latest_receipt_for_suite(entry.suite_id)
            if problem:
                integrity_ok = False
        decision = evaluate_reuse(
            receipt,
            fingerprint,
            lane_reuse_allowed=lane_ok,
            lane_reason=lane_reason,
            freshness="always-fresh" if external else "reusable",
            external_state=external and not allow_external,
            integrity_ok=integrity_ok,
        )
        action = "REUSE" if decision.kind == "Reusable" else "EXECUTE"
        blocked = False
        reason_text = f"{reason}; {decision.reason}"
        if decision.kind == "Blocked" or (external and not allow_external):
            action = "BLOCK"
            blocked = True
            reason_text = (
                f"{reason}; requiere estado externo (sandbox, contenedor o servicio) "
                "que no se puede capturar en la huella: se ejecuta en su lane, aquí se declara"
            )
        planned.append(
            PlannedSuite(
                entry=entry,
                reason=reason,
                fingerprint=fingerprint,
                action=action,
                action_reason=reason_text,
                receipt_id=decision.receipt_id,
                blocked=blocked,
            )
        )
        actions.append(
            PlanAction(
                suite_id=entry.suite_id,
                action=action,
                reason=reason_text,
                dependencies=(),
                receipt_id=decision.receipt_id if action == "REUSE" else None,
            )
        )

    counts_status = "READY"
    if selection.unknown_reasons:
        counts_status = "UNKNOWN_INPUT"
    elif any(suite.blocked for suite in planned):
        counts_status = "BLOCKED"

    plan = ExecutionPlan(
        plan_id=f"{run_id}-plan",
        repo_id=repo_id,
        source_fingerprint=_source_fingerprint(changeset),
        lane=lane,
        status=counts_status,
        required_suites=tuple(entry.suite_id for entry, _ in selection.required),
        actions=tuple(actions),
        policy_version=POLICY_VERSION,
        unknown_reasons=selection.unknown_reasons,
        created_at=created_at,
    )

    warnings: list[str] = []
    for suite in planned:
        closure = closure_for(suite.entry, repo_root=repo_root, graph=graph)
        if closure.truncated:
            warnings.append(
                f"la clausura de {suite.entry.suite_id} superó el límite de ficheros: "
                "la huella cubre lo leído, no todo"
            )
    parity = None
    if oracle is not None and oracle.ok:
        parity = compare(
            oracle,
            required=tuple(entry for entry, _ in selection.required),
            catalog=catalog,
            attributable=refutable_fallback(changeset, graph),
        )
        if parity.verdict == "HARNESS_WEAKER":
            warnings.append(
                "el plan exige menos que el oráculo CR-08 sin desviación declarada: "
                f"scopes ausentes {', '.join(parity.missing)}. Ejecuta `harness parity`"
            )
    return PlanResult(
        plan=plan,
        selection=selection,
        suites=tuple(planned),
        toolchain=toolchain,
        changeset=changeset,
        warnings=tuple(warnings),
        parity=parity,
        oracle=oracle,
    )


def refutable_fallback(changeset: ChangeSet, graph: WorkspaceGraph) -> bool:
    """¿Puede el harness refutar la caída a ALL del oráculo con su propia clasificación?

    Solo cuando **todos** los paths cambiados tienen clasificación positiva del
    harness: documentación (`docs/**`), documentación de raíz de la lista
    verificada, o un paquete del grafo. Si algún path cae al desconocido, el
    harness no sabe más que el oráculo y la diferencia se reporta como debilidad.
    """
    paths = changeset.all_paths()
    if not paths:
        return False
    for path in paths:
        if path.startswith("docs/"):
            continue
        if "/" not in path and Path(path).name in METADATA_ONLY_NAMES:
            continue
        if graph.owner_of(path) is not None:
            continue
        return False
    return True


def _apply_oracle(
    selection: Selection, oracle: SelectorDecision, *, catalog: tuple[CatalogEntry, ...]
) -> Selection:
    """Aplica el suelo del oráculo: nunca exigir menos que el selector vigente."""
    if not oracle.ok:
        reason = oracle.problem or "el oráculo no devolvió una decisión legible"
        unknown = (f"oráculo CR-08 ilegible ({reason}): sin él no se puede demostrar paridad",)
        return Selection(
            required=tuple((entry, f"fallback conservador: {unknown[0]}") for entry in catalog),
            excluded=selection.excluded,
            fallback_reason=unknown[0],
            unknown_reasons=selection.unknown_reasons + unknown,
            global_reasons=selection.global_reasons,
        )
    additions = floor_entries(
        oracle,
        required=tuple(entry for entry, _ in selection.required),
        catalog=catalog,
    )
    if not additions:
        return selection
    added_ids = {entry.suite_id for entry, _ in additions}
    required = (*selection.required, *additions)
    return Selection(
        required=tuple(sorted(required, key=lambda item: item[0].suite_id)),
        excluded=tuple(
            (entry, f"{reason} (el oráculo no lo exigía)" if entry.suite_id not in added_ids else reason)
            for entry, reason in selection.excluded
        ),
        fallback_reason=selection.fallback_reason,
        unknown_reasons=selection.unknown_reasons,
        global_reasons=selection.global_reasons,
    )


def _features_of(entry: CatalogEntry) -> tuple[str, ...]:
    features: list[str] = []
    argv = list(entry.argv)
    for index, token in enumerate(argv):
        if token == "--features" and index + 1 < len(argv):
            features.extend(argv[index + 1].split(","))
        elif token.startswith("--features="):
            features.extend(token.split("=", 1)[1].split(","))
        elif token == "--all-features":
            features.append("*")
    return tuple(sorted(set(features)))


EXTERNAL_MARKERS = ("sandbox", "podman", "docker", "compose", "pg_", "postgres")
EXTERNAL_PIPELINES = (
    "integration.pipeline.kts",
    "certification.pipeline.kts",
    "release.pipeline.kts",
    "release-candidate.pipeline.kts",
)


def _needs_external_state(entry: CatalogEntry) -> bool:
    if entry.target == "cargo:deny":
        # `cargo deny` consulta una base de avisos externa: su resultado depende de
        # estado que la huella de fuentes no captura.
        return True
    blob = " ".join(entry.argv) + " " + entry.target
    if any(marker in blob for marker in EXTERNAL_MARKERS):
        return True
    return entry.origin in EXTERNAL_PIPELINES


def _source_fingerprint(changeset: ChangeSet) -> str:
    """Huella de la fuente que el plan describe: HEAD + contenido tocado."""
    import hashlib

    digest = hashlib.sha256()
    digest.update((changeset.head or "no-head").encode("utf-8"))
    digest.update(b"\0")
    digest.update(changeset.digest_for(changeset.all_paths()).encode("utf-8"))
    return digest.hexdigest()


def suite_ref_for(suite: PlannedSuite) -> SuiteRef:
    return SuiteRef(
        suite_id=suite.entry.suite_id,
        package=suite.entry.package,
        target=suite.entry.target,
        argv=suite.entry.argv,
        lanes=suite.entry.lanes,
    )

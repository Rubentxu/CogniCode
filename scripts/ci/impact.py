#!/usr/bin/env python3
"""impact.py — que probar para ESTE cambio, y en que orden.

Una sola pregunta: dado el diff contra `origin/main`, ¿que parte del merge gate
puede romperse? La respuesta es un plan con un nivel (`tier`) y un orden:

  docs    solo documentacion            -> docs-gate.pipeline.kts
  impact  codigo con radio acotado       -> impact-gate.pipeline.kts (este plan)
  full    lo que puede cambiarlo todo    -> merge-gate.pipeline.kts

El plan no inventa como se prueba un crate: **ejecuta las mismas etapas de
`merge-gate.pipeline.kts`**, filtradas a los crates afectados. Si el gate
cambia, el plan cambia con el; no hay una segunda definicion que mantener.

Como se decide el radio de impacto:

  1. fichero -> crate que lo contiene (`product/` es de core, que lo compila
     con include_str!; `skills/` y `bundles/` son de la CLI).
  2. crate -> crates que dependen de el, leido de los Cargo.toml (normal, dev
     y build). Un cambio en core alcanza a explorer, mcp, cli...; uno en la CLI
     no alcanza a nadie.
  3. dentro de `cognicode-core` (63 % de los tests), por modulo: modulo
     cambiado -> modulos que lo importan (`use crate::…`, `super::`, `self::`),
     y de ahi los tests unitarios de esos modulos y los ficheros de
     `tests/` que importan alguno (`use cognicode_core::…`).

Orden progresivo (`run`): primero lo mas barato y lo mas cercano al cambio,
para que un rojo llegue pronto:
  anillo 0  fmt, clippy de los crates afectados, contratos
  anillo 1  tests de los crates que cambiaron
  anillo 2  tests de los crates que dependen de ellos

Falla cerrado: lockfile, manifiestos, toolchain, pipelines, scripts que el
gate ejecuta, `lib.rs` de core o cualquier ruta que no sepa clasificar => full.

La red de seguridad NO es este fichero: es el gate completo en `main` cada
noche y antes de cada tag. Si alli sale un rojo que un PR dio por no afectado,
la regla que fallo se corrige aqui, con su caso en `test_impact.py`.

Uso:
  python3 scripts/ci/impact.py plan [--base origin/main] [--sha HEAD] [--json]
  python3 scripts/ci/impact.py plan --files a.rs b.md ...
  python3 scripts/ci/impact.py run  [--base origin/main] [--sha HEAD]
  python3 scripts/ci/impact.py tier [...]          # docs | impact | full
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
import textwrap
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
GATE = REPO / "merge-gate.pipeline.kts"
CORE = "cognicode-core"
CORE_SRC = REPO / "crates" / CORE / "src"
CORE_TESTS = REPO / "crates" / CORE / "tests"

DOCS = re.compile(r"^(docs/|odd/|evidence/|plans/|sdd/|sddk/|openspec/|goal-compass\.json$|[^/]*\.md$)")
# Cambian lo que se compila en todo el workspace: aqui no hay radio que acotar.
FULL = re.compile(r"^(Cargo\.toml$|\.cargo/|rust-toolchain|sandbox/)")
# Solo los leen los contratos: el anillo 0 basta.
CONTRACTS_ONLY = re.compile(
    r"^(scripts/ci/test_[^/]+\.py$|scripts/product/|\.gitignore$|deny\.toml$|"
    r"perf-budget\.toml$|evals/|\.config/nextest\.toml$|apps/|tests/e2e/|"
    r"[^/]*\.pipeline\.kts$|justfile$)"   # pipelines que no son el merge gate: los validan sus contratos
)
# Scripts: los juzgan los contratos y las etapas del gate que los ejecutan.
SCRIPTS = re.compile(r"^scripts/")
MANIFEST = re.compile(r"^crates/([^/]+)/(Cargo\.toml|build\.rs)$")
OWNER_OVERRIDES = (("product/", CORE), ("skills/", "cognicode-cli"), ("bundles/", "cognicode-cli"))


# ----------------------------------------------------------------- crates
def workspace() -> tuple[dict[str, str], dict[str, set[str]]]:
    """(dir -> crate, crate -> crates que dependen de el)."""
    ws = tomllib.loads((REPO / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
    ws_paths = {k for k, v in ws.get("dependencies", {}).items() if isinstance(v, dict) and "path" in v}
    dirs: dict[str, str] = {}
    deps: dict[str, set[str]] = {}
    for member in ws["members"]:
        manifest = tomllib.loads((REPO / member / "Cargo.toml").read_text(encoding="utf-8"))
        name = manifest["package"]["name"]
        dirs[member.rstrip("/") + "/"] = name
        found = set()
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for dep, spec in manifest.get(section, {}).items():
                if isinstance(spec, dict) and ("path" in spec or (spec.get("workspace") and dep in ws_paths)):
                    found.add(dep)
        deps[name] = found
    dependents: dict[str, set[str]] = collections.defaultdict(set)
    for name, ds in deps.items():
        for d in ds:
            if d in deps:
                dependents[d].add(name)
    return dirs, dependents


def closure(start: set[str], edges: dict[str, set[str]]) -> set[str]:
    out, stack = set(start), list(start)
    while stack:
        for nxt in edges.get(stack.pop(), ()):
            if nxt not in out:
                out.add(nxt)
                stack.append(nxt)
    return out


# ------------------------------------------------------------ core modules
PUB_ITEM = re.compile(r"\bpub(?:\([^)]*\))?\s+(?:async\s+|unsafe\s+|const\s+)*(?:struct|enum|trait|fn|type|const|static|union|mod|macro_rules!)\s+([A-Za-z_][A-Za-z0-9_]*)")
USE_STMT = re.compile(r"\buse\s+([^;]+);")
PATH = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+")
IMPORT = re.compile(r"cognicode_core::((?:[A-Za-z_][A-Za-z0-9_]*::)*)(\{[^;]*?\}|[A-Za-z_][A-Za-z0-9_]*)", re.S)
TEST_ATTR = re.compile(r"#\[(?:tokio::)?test")


def code_only(src: str) -> str:
    """El fuente sin comentarios ni literales de cadena: una ruta citada en un
    doc-comment o en un string no es una dependencia."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j == -1 else j
        elif src.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if src.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif src.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c == "r" and re.match(r'r#*"', src[i:i + 8]) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = re.match(r'r(#*)"', src[i:]).group(1)
            end = src.find('"' + hashes, i + 2 + len(hashes))
            i = n if end == -1 else end + 1 + len(hashes)
            out.append('""')
        elif c == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == "\\" else 1
            i += 1
            out.append('""')
        else:
            out.append(c)
            i += 1
    return "".join(out)


INLINE_MOD = re.compile(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{")


def inline_scopes(code: str) -> list[tuple[int, int, str]]:
    """(inicio, fin, nombre) de cada `mod x { ... }` en linea. Dentro de uno,
    `super::` y `self::` cuentan desde ese modulo, no desde el fichero."""
    scopes, i = [], 0
    for m in INLINE_MOD.finditer(code):
        depth, j = 0, m.end() - 1
        while j < len(code):
            if code[j] == "{":
                depth += 1
            elif code[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        scopes.append((m.end(), j, m.group(1)))
    return scopes


def module_of(path: Path) -> str:
    parts = list(path.relative_to(CORE_SRC).with_suffix("").parts)
    if parts and parts[-1] in ("mod", "lib"):
        parts = parts[:-1]
    return "::".join(parts)


class CoreGraph:
    def __init__(self) -> None:
        self.files = {module_of(p): p for p in CORE_SRC.rglob("*.rs")}
        # `#[path = "x_tests.rs"] mod tests;`: el modulo se llama como lo declara
        # su padre, no como su fichero, y asi se llaman sus tests.
        for mod, path in list(self.files.items()):
            src = path.read_text(encoding="utf-8", errors="replace")
            for rel, name in re.findall(r'#\[path\s*=\s*"([^"]+)"\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;', src):
                target = (path.parent / rel).resolve()
                old_name = next((m for m, p in self.files.items() if p.resolve() == target), None)
                if old_name is not None:
                    real = f"{mod}::{name}" if mod else name
                    self.files[real] = self.files.pop(old_name)
        self.code = {m: code_only(p.read_text(encoding="utf-8", errors="replace")) for m, p in self.files.items()}
        # `pub use` de cada modulo: `crate::domain::Foo`, si domain/mod.rs
        # re-exporta Foo, depende del modulo que DEFINE Foo, no de todo domain.
        # (Sin esto, `crate::Result` -anyhow- haria depender a medio crate de
        # todo lo que lib.rs re-exporta.)
        self.reexports: dict[str, dict[str, str | None]] = collections.defaultdict(dict)
        self.defines = {m: set(PUB_ITEM.findall(c)) for m, c in self.code.items()}
        for _ in range(3):  # re-exportaciones encadenadas
            for mod, code in self.code.items():
                here = mod.split("::") if mod else []
                for tree in re.findall(r"\bpub(?:\([^)]*\))?\s+use\s+([^;]+);", code):
                    for path in _expand("", re.sub(r"\s+", "", tree)):
                        parts = path.split("::")
                        name = parts[-1]
                        if name == "*":
                            # `pub use hijo::*`: cada nombre publico del hijo
                            src = self._resolve(parts[:-1] + ["__glob__"], here, allow_root=False)
                            if src is not None:
                                for item in self.defines.get(src, ()):
                                    self.reexports[mod].setdefault(item, src)
                                for item, origin in list(self.reexports.get(src, {}).items()):
                                    self.reexports[mod].setdefault(item, origin)
                            continue
                        if " as " in path or name == "self":
                            continue
                        self.reexports[mod][name] = self._resolve(parts, here, allow_root=False)
        self.importers: dict[str, set[str]] = collections.defaultdict(set)
        self.unit_tests: dict[str, int] = {}
        for mod, path in self.files.items():
            src = path.read_text(encoding="utf-8", errors="replace")
            self.unit_tests[mod] = len(TEST_ATTR.findall(src))
            here = mod.split("::") if mod else []
            for target in self._references(self.code[mod], here):
                if target != mod:
                    self.importers[target].add(mod)
        # `#[macro_export] macro_rules! m` se usa como `m!(...)` sin ruta.
        macros = {name: m for m, code in self.code.items()
                  for name in re.findall(r"macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)", code)}
        for mod, code in self.code.items():
            for name in set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)!", code)) & macros.keys():
                if macros[name] != mod:
                    self.importers[macros[name]].add(mod)
        self.integration = {}
        for path in sorted(CORE_TESTS.glob("*.rs")):
            src = path.read_text(encoding="utf-8", errors="replace")
            self.integration[path.stem] = (self._imports(src), len(TEST_ATTR.findall(src)))

    def _resolve(self, parts: list[str], here: list[str], allow_root: bool = True) -> str | None:
        """Ruta de Rust 2018 -> modulo de core que nombra, o None si es externa."""
        target = self._absolute(parts, here)
        if target is None:
            return None                     # std, serde, otro crate...
        return self._deepest(target, root_ok=allow_root)

    def _references(self, src: str, here: list[str]) -> set[str]:
        found: set[str] = set()
        scopes = inline_scopes(src)

        def scope_at(pos: int) -> list[str]:
            return here + [name for a, b, name in scopes if a <= pos <= b]

        aliases: dict[str, list[str]] = {}
        for m_use in USE_STMT.finditer(src):
            at = scope_at(m_use.start())
            for path in _expand("", re.sub(r"\s+", "", m_use.group(1))):
                parts = path.split("::")
                if parts and parts[-1] == "self":
                    parts = parts[:-1]
                full = self._absolute(parts, at)
                if full is not None and full[-1:] != ["*"] and "::".join(full) in self.files:
                    # importa un modulo como espacio de nombres: la dependencia la
                    # crean los nombres que se usen a traves de el, no el `use`.
                    aliases[parts[-1]] = full
                    continue
                m = self._resolve(parts, at)
                if m is not None and "::".join(at).startswith(m) and parts[-1:] == ["*"] and m == "::".join(here):
                    continue  # `use super::*` de un mod de test en linea: es el propio fichero
                if m is not None:
                    found.add(m)
        for m_path in PATH.finditer(src):
            parts = m_path.group(0).split("::")
            if parts[0] in aliases:
                m = self._deepest(aliases[parts[0]] + parts[1:], root_ok=False)
            else:
                m = self._resolve(parts, scope_at(m_path.start()))
            if m is not None:
                found.add(m)
        return found

    def _absolute(self, parts: list[str], here: list[str]) -> list[str] | None:
        if not parts:
            return None
        if parts[0] == "crate":
            return parts[1:]
        if parts[0] == "self":
            return here + parts[1:]
        if parts[0] == "super":
            base = here[:]
            while parts and parts[0] == "super":
                base, parts = base[:-1], parts[1:]
            return base + parts
        if not here and parts[0] in self.files:
            return parts
        file_here = here[:]
        while file_here and "::".join(file_here) not in self.files:
            file_here = file_here[:-1]
        if "::".join(file_here + parts[:1]) in self.files:
            return file_here + parts
        return None

    def _deepest(self, parts: list[str], root_ok: bool = False) -> str | None:
        """Modulo mas profundo que existe en la ruta. `crate::Foo` (una
        re-exportacion de lib.rs) es la raiz `""` si root_ok."""
        parts = [p for p in parts if p and p not in ("self", "*")]
        k = len(parts)
        while k and "::".join(parts[:k]) not in self.files:
            k -= 1
        mod = "::".join(parts[:k])
        if k < len(parts):
            nxt = parts[k]
            table = getattr(self, "reexports", {}).get(mod, {})
            if nxt in table:
                return table[nxt]           # el modulo que define el nombre (None = externo)
        if k:
            return mod
        return "" if root_ok else None

    def _imports(self, src: str) -> set[str] | None:
        """Modulos de core que importa un test; None = depende de la raiz (todo)."""
        mods: set[str] = set()
        for prefix, tail in IMPORT.findall(src):
            paths = _expand(prefix, tail[1:-1]) if tail.startswith("{") else [prefix + tail]
            for p in paths:
                m = self._deepest(re.sub(r"\s+", "", p).split("::"))
                if m is None:
                    return None
                mods.add(m)
        return mods or None  # sin imports: caja negra (lanza binarios) => siempre

    def select(self, changed_modules: set[str]) -> dict:
        affected = closure(changed_modules, self.importers)
        tests = sorted(
            name for name, (mods, _) in self.integration.items()
            if mods is None or mods & affected
        )
        return {
            "changed_modules": sorted(changed_modules),
            "affected_modules": sorted(affected),
            "unit_tests": sum(self.unit_tests[m] for m in affected),
            "unit_tests_total": sum(self.unit_tests.values()),
            "integration": tests,
            "integration_total": len(self.integration),
        }


def _expand(prefix: str, body: str) -> list[str]:
    out, depth, cur = [], 0, ""
    for ch in body + ",":
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            cur = cur.strip()
            if "{" in cur:
                pre, rest = cur.split("{", 1)
                out += _expand(prefix + pre, rest.rsplit("}", 1)[0])
            elif cur:
                out.append(prefix + cur)
            cur = ""
        else:
            cur += ch
    return out


# -------------------------------------------------------------------- plan
def changed_files(base: str, sha: str) -> list[str]:
    mb = subprocess.run(["git", "merge-base", base, sha], cwd=REPO, capture_output=True, text=True)
    if mb.returncode != 0:
        return []
    diff = subprocess.run(["git", "diff", "--name-only", mb.stdout.strip(), sha],
                          cwd=REPO, capture_output=True, text=True)
    return [f for f in diff.stdout.splitlines() if f]


def lock_packages(text: str) -> dict[str, tuple]:
    out = {}
    for pkg in tomllib.loads(text).get("package", []):
        out[pkg["name"] + "@" + pkg["version"]] = (pkg.get("source"), pkg.get("checksum"), tuple(pkg.get("dependencies", [])))
    return out


def lock_affected(base: str, sha: str) -> set[str] | None:
    """Crates del workspace cuyo arbol de dependencias cambia en Cargo.lock."""
    def show(ref: str) -> str | None:
        r = subprocess.run(["git", "show", f"{ref}:Cargo.lock"], cwd=REPO, capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else None
    old_t, new_t = show(base), show(sha)
    if old_t is None or new_t is None:
        return None
    old, new = lock_packages(old_t), lock_packages(new_t)
    changed = {k.split("@")[0] for k in set(old) ^ set(new)}
    changed |= {k.split("@")[0] for k in set(old) & set(new) if old[k] != new[k]}
    users: dict[str, set[str]] = collections.defaultdict(set)
    for key, (_, _, deps) in new.items():
        for d in deps:
            users[d.split(" ")[0]].add(key.split("@")[0])
    return closure(changed, users)


def plan(files: list[str], base: str = "origin/main", sha: str = "HEAD") -> dict:
    if not files:
        return {"tier": "full", "reason": "diff vacio o ilegible: falla cerrado", "files": files}
    if all(DOCS.match(f) for f in files):
        return {"tier": "docs", "reason": "solo documentacion", "files": files}
    full = [f for f in files if FULL.match(f)]
    if full:
        return {"tier": "full", "reason": f"cambia lo que se compila en todo el workspace: {full[:3]}", "files": files}

    dirs, dependents = workspace()
    members = set(dirs.values())
    changed: set[str] = set()
    scripts: list[str] = []
    gate_changed = False
    why: list[str] = []
    for f in files:
        if f == GATE.name:
            gate_changed = True
            continue
        if DOCS.match(f) or CONTRACTS_ONLY.match(f):
            continue
        if SCRIPTS.match(f):
            scripts.append(f)
            continue
        if f == "Cargo.lock":
            hit = lock_affected(base, sha)
            if hit is None:
                return {"tier": "full", "reason": "Cargo.lock ilegible en la base o en el cambio", "files": files}
            changed |= hit & members
            why.append(f"Cargo.lock cambia el arbol de {sorted(hit & members)}")
            continue
        if f == f"crates/{CORE}/src/lib.rs":
            changed.add(CORE)
            continue
        owner = next((c for p, c in OWNER_OVERRIDES if f.startswith(p)), None)
        owner = owner or next((c for d, c in dirs.items() if f.startswith(d)), None)
        if owner is None:
            return {"tier": "full", "reason": f"ruta sin clasificar: {f}", "files": files}
        changed.add(owner)

    affected = closure(changed, dependents)
    out = {
        "tier": "impact",
        "reason": "; ".join(why + [f"cambian {sorted(changed) or 'ningun crate'}; afectan a {sorted(affected) or 'ninguno'}"]),
        "files": files,
        "changed_crates": sorted(changed),
        "affected_crates": sorted(affected),
        "changed_scripts": scripts,
        "gate_changed": gate_changed,
    }
    if gate_changed or scripts:
        out["reason"] += f"; el propio gate cambia ({'merge-gate.pipeline.kts' if gate_changed else ''} {' '.join(scripts[:3])}): corren sus etapas afectadas"
    if CORE in affected:
        core_files = [f for f in files if f.startswith(f"crates/{CORE}/")]
        src = [f for f in core_files if f.startswith(f"crates/{CORE}/src/") and f.endswith(".rs")]
        tests = [f for f in core_files if re.match(rf"crates/{CORE}/tests/[^/]+\.rs$", f)]
        other = [f for f in core_files if f not in src and f not in tests]
        lock_hit = CORE in changed and not core_files
        if CORE not in changed:
            out["core"] = {"mode": "all", "why": "core no cambia, pero depende de un crate que si"}
        elif lock_hit or other or f"crates/{CORE}/src/lib.rs" in src or any(not (REPO / f).exists() for f in src):
            out["core"] = {"mode": "all", "why": f"cambia lib.rs, un manifiesto, una dependencia o un fichero que no es modulo ni test"}
        else:
            sel = CoreGraph().select({module_of(REPO / f) for f in src})
            sel["integration"] = sorted(set(sel["integration"]) | {Path(f).stem for f in tests if (REPO / f).exists()})
            out["core"] = {"mode": "modules", **sel}
    return out


# --------------------------------------------------------------- gate stages
ALWAYS = {"build-isolation", "toolchain", "repo-contract", "fmt", "contract-toolchain",
          "build-cognicode-for-contracts", "install-cosign", "contracts",
          "select-suites-script", "bin-tracking", "capabilities-drift-lint"}
FULL_ONLY = {"install-cargo-deny", "advisories", "licenses"}   # solo cambian con el lockfile
BY_STAGE = {"build-control-plane-debug": {"cognicode-explorer"},
            "install-rust-analyzer": {CORE}, "rust-analyzer-answers": {CORE},
            "skills-validate": {"cognicode-cli"}, "skills-bundles": {"cognicode-cli"}}
BUILDS = {"build-cognicode-mcp-debug", "build-cognicode-cli-debug", "verify-debug-binaries"}
CORE_EK = re.compile(r"cargo test -p cognicode-core --features evidence-kernel --quiet\s*\\\s*\n\s*-- --skip release_binary_stdio_round_trip_smoke")
SKIP = "--skip release_binary_stdio_round_trip_smoke"


def gate_stages(text: str | None = None) -> list[tuple[str, str]]:
    text = GATE.read_text(encoding="utf-8") if text is None else text
    vals = {"repoRoot": str(REPO)}
    vals["cd"] = f'cd "{vals["repoRoot"]}"'
    vals["logDir"] = os.path.join(os.environ.get("TMPDIR", "/tmp"), "cognicode-logs")
    vals["cdLog"] = f'mkdir -p "{vals["logDir"]}"'
    vals["authority"] = GATE.name
    live = "\n".join(l for l in text.splitlines() if not l.strip().startswith("//"))
    out = []
    for m in re.finditer(r'sh\(\s*(?:"""(.*?)"""|"((?:[^"\\]|\\.)*)")', live, re.S):
        if m.group(1) is not None:
            body = textwrap.dedent(m.group(1)).strip("\n")
        else:
            body = m.group(2).replace('\\"', '"').replace("\\$", "$")
        body = body.replace("${'$'}", "\x00")
        for k in sorted(vals, key=len, reverse=True):
            body = body.replace("$" + k, vals[k])
        body = body.replace("\x00", "$")
        stage = re.findall(r'stage\("([^"]+)"\)', live[: m.start()])[-1]
        out.append((stage, body))
    return out


def stage_crates(stage: str, body: str) -> set[str] | str:
    if stage in ALWAYS:
        return "always"
    if stage in FULL_ONLY:
        return "never"
    if stage == "clippy":
        return "clippy"
    if stage in BUILDS:
        return "builds"
    if stage in BY_STAGE:
        return BY_STAGE[stage]
    code = "\n".join(l for l in body.splitlines() if not l.strip().startswith("#"))
    crates = set(re.findall(r"-p\s+(cognicode[\w-]*)", code))
    return crates or "always"  # una etapa que no se sabe atribuir corre siempre


def core_command(core: dict) -> str | None:
    if core.get("mode") != "modules":
        return None
    base = "cargo test -p cognicode-core --features evidence-kernel --quiet"
    parts = []
    if core["affected_modules"]:
        filters = " ".join(f"'{m}::'" for m in core["affected_modules"] if m)
        parts.append(f"{base} --lib -- {SKIP} {filters}")
    if core["integration"]:
        targets = " ".join(f"--test {t}" for t in core["integration"])
        parts.append(f"{base} {targets} -- {SKIP}")
    return "( " + " && ".join(parts) + " )" if parts else "true"


def schedule(p: dict, base: str = "origin/main") -> list[tuple[int, str, str, str]]:
    """(anillo, etapa, por que, cuerpo) en el orden en que se ejecutan."""
    affected = set(p.get("affected_crates", []))
    changed = set(p.get("changed_crates", []))
    scripts = p.get("changed_scripts", [])
    previous: dict[str, str] = {}
    if p.get("gate_changed"):
        r = subprocess.run(["git", "show", f"{base}:{GATE.name}"], cwd=REPO, capture_output=True, text=True)
        previous = dict(gate_stages(r.stdout)) if r.returncode == 0 else {}
    rows = []
    for stage, body in gate_stages():
        who = stage_crates(stage, body)
        touched = [sc for sc in scripts if sc in body or Path(sc).name in body]
        if p.get("gate_changed") and previous.get(stage) != body:
            rows.append((1, stage, "la etapa cambio en este PR", body)); continue
        if touched:
            rows.append((1, stage, f"ejecuta un script que cambia: {touched[0]}", body)); continue
        if who == "never":
            continue
        if who == "always":
            rows.append((0, stage, "siempre: barato o protege el propio repo", body)); continue
        if who == "clippy":
            if affected:
                pkgs = " ".join(f"-p {c}" for c in sorted(affected))
                rows.append((0, stage, f"solo crates afectados", f'cd "{REPO}" && cargo clippy {pkgs} --all-targets -- -D warnings'))
            continue
        if who == "builds":
            if affected:
                rows.append((1, stage, "los tests de los crates afectados lanzan estos binarios", body))
            continue
        hit = who & affected
        if not hit:
            continue
        ring = 1 if who & changed else 2
        if stage == "core-unrestricted-evidence-kernel" and "core" in p:
            cmd = core_command(p["core"])
            if cmd is not None:
                new_body, n = CORE_EK.subn(cmd, body)
                if n == 1:
                    c = p["core"]
                    why = (f"core por modulo: {len(c['affected_modules'])} modulos, "
                           f"{c['unit_tests']}/{c['unit_tests_total']} tests unitarios, "
                           f"{len(c['integration'])}/{c['integration_total']} ficheros de integracion")
                    rows.append((ring, stage, why, new_body)); continue
                # la etapa del gate cambio de forma: sin filtro, entera (falla cerrado)
        rows.append((ring, stage, f"afecta a {sorted(hit)}", body))
    return sorted(rows, key=lambda r: r[0])  # sort estable: dentro del anillo, orden del gate


# --------------------------------------------------------------------- cli
def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=("tier", "plan", "run"))
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--sha", default="HEAD")
    ap.add_argument("--files", nargs="*")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    files = a.files if a.files is not None else changed_files(a.base, a.sha)
    p = plan(files, a.base, a.sha)

    if a.cmd == "tier":
        print(p["tier"]); return 0
    if a.cmd == "plan":
        if a.json:
            print(json.dumps(p, ensure_ascii=False, indent=2)); return 0
        print(f"tier: {p['tier']} — {p['reason']}")
        if p["tier"] == "impact":
            for ring, stage, why, _ in schedule(p, a.base):
                print(f"  anillo {ring}  {stage:40} {why}")
        return 0

    if p["tier"] != "impact":
        print(f"FAIL: tier '{p['tier']}' ({p['reason']}); impact-gate solo juzga cambios de impacto acotado.")
        return 1
    print(f"==> {p['reason']}")
    for ring, stage, why, body in schedule(p, a.base):
        print(f"\n==> anillo {ring} · {stage} · {why}", flush=True)
        rc = subprocess.run(["bash", "-c", "set -eo pipefail\n" + body], cwd=REPO).returncode
        if rc != 0:
            print(f"\n==> FAIL en {stage} (rc={rc}); las etapas siguientes no corren.")
            return rc
    print("\n==> todas las etapas del plan en verde")
    return 0


if __name__ == "__main__":
    sys.exit(main())

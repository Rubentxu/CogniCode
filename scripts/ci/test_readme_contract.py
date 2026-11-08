#!/usr/bin/env python3
"""README contract: the English and Spanish entry points describe one product.

Three things drift silently between `README.md` and `README.es.md`, and each
one sends a reader to something that does not work:

* the heading outline (a section written in one language only),
* the version that install and `--version` examples tell the reader to expect,
* relative links that point at files that are not in the tree.

The version oracle is `[workspace.package] version` in the root `Cargo.toml`.
The `## Versioning` section is history on purpose (it names old tags), so it is
excluded from the pinned-version check. The contract is its own control: a
README with no headings, no pinned example or no link fails instead of passing
because there was nothing to compare.
"""

import json
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EN = ROOT / "README.md"
ES = ROOT / "README.es.md"
HISTORY_HEADING = "versioning"
FENCE = "```"


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    section = text.split("[workspace.package]", 1)[1]
    match = re.search(r'^version\s*=\s*"([^"]+)"', section, re.MULTILINE)
    assert match, "no version in [workspace.package]"
    return match.group(1)


def outline(path: Path) -> list[int]:
    """Heading levels in order, ignoring `#` lines inside code fences."""
    levels, in_fence = [], False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        elif not in_fence:
            m = re.match(r"(#{2,3}) \S", line)
            if m:
                levels.append(len(m.group(1)))
    return levels


def without_history(path: Path) -> str:
    """README text with the `## Versioning` section removed."""
    kept, skipping, in_fence = [], False, False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        if not in_fence and re.match(r"## ", line):
            skipping = line[3:].strip().lower() == HISTORY_HEADING
        if not skipping:
            kept.append(line)
    return "\n".join(kept)


PINNED = re.compile(
    r"(?:COGNICODE_VERSION=v|@v?|expect: [a-z-]+ )(\d+\.\d+\.\d+)"
)


def pinned_versions(path: Path) -> list[str]:
    return PINNED.findall(without_history(path))


def local_links(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    targets = re.findall(r"\]\(([^)\s]+)\)", text)
    return [
        t.split("#", 1)[0]
        for t in targets
        if not re.match(r"[a-z]+:|#|mailto:", t) and t.split("#", 1)[0]
    ]


def root_markdown() -> list[Path]:
    """Tracked Markdown files at the repository root."""
    return sorted(ROOT / f for f in tracked_files() if "/" not in f and f.endswith(".md"))


def missing_links(path: Path, links: list[str]) -> list[str]:
    return sorted({l for l in links if not (path.parent / l).exists()})


def install_docs() -> list[Path]:
    """Install and contribution guides a reader follows with a pinned version."""
    docs = [ROOT / "INSTALL.md", ROOT / "CONTRIBUTING.md"]
    docs += sorted((ROOT / "docs" / "distribution").glob("*.md"))
    return docs


GUIDE_PINNED = re.compile(r"(?:--version |@)v?(\d+\.\d+\.\d+)")


def guide_versions(path: Path) -> list[str]:
    return PINNED.findall(path.read_text(encoding="utf-8")) + GUIDE_PINNED.findall(
        path.read_text(encoding="utf-8")
    )


ILLUSTRATIVE = "illustrative"
FENCED = re.compile(r"```([^\n]*)\n(.*?)```", re.S)
INLINE = re.compile(r"`([^`\n]+)`")
JUST_CALL = re.compile(r"\bjust ([A-Za-z_][\w-]*)")
RECIPE_DEF = re.compile(r"^@?([A-Za-z_][\w-]*)(?:[ \t][^:\n]*)?:(?!=)", re.M)


def tracked_files() -> set[str]:
    out = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return {line for line in out.splitlines() if line}


def untracked_on_disk(cited: list[str]) -> list[str]:
    """Cited paths that exist in the working tree but are not in the git index."""
    return sorted(p for p in cited if (ROOT / p).exists())


def untracked_message(doc: Path, cited: list[str]) -> str:
    return "; ".join(
        f"{doc.name} cites {p}, which exists but is not in the git index; run git add"
        f" (git add -f if the path is ignored)"
        for p in untracked_on_disk(cited)
    )


def justfile_recipes() -> set[str]:
    return set(RECIPE_DEF.findall((ROOT / "justfile").read_text(encoding="utf-8")))


def code_spans(text: str) -> list[str]:
    """Fenced blocks and inline code; a fence tagged `illustrative` is skipped."""
    spans = [body for info, body in FENCED.findall(text) if ILLUSTRATIVE not in info]
    return spans + INLINE.findall(FENCED.sub("", text))


def dev_references(text: str, tops: set[str]) -> tuple[set[str], set[str]]:
    """(repo paths, `just` recipes) cited in code spans, placeholders excluded."""
    paths, recipes = set(), set()
    for span in code_spans(text):
        recipes.update(JUST_CALL.findall(span))
        for token in re.findall(r"[^\s'\"=]+/[^\s'\"]*|[^\s'\"]+\.(?:sh|py)\b", span):
            token = token.rstrip(".,;:)")
            if "<" in token or "*" in token or "$" in token:
                continue
            if token.split("/", 1)[0] in tops or token.startswith("scripts"):
                paths.add(token)
    return paths, recipes


def missing_references(
    text: str, files: set[str], recipes: set[str]
) -> tuple[list[str], list[str]]:
    tops = {f.split("/", 1)[0] for f in files}
    paths, cited = dev_references(text, tops)
    gone = sorted(
        p for p in paths
        if p not in files and not any(f.startswith(p.rstrip("/") + "/") for f in files)
    )
    return gone, sorted(cited - recipes)


SHELL_INFO = {"bash", "sh", "shell", "console"}
URL = re.compile(r"https?://[^\s)>\]\"'`]+")
FLAG = re.compile(r"(?<![\w-])--?[A-Za-z][\w-]*")
NUMBER = re.compile(r"(?<![\w.])\d+(?:\.\d+)*(?![\w.])")
WORD = re.compile(r"[A-Za-z_][\w-]*")


def readme_sections(path: Path) -> list[tuple[str, str]]:
    """(heading, body) for the intro and every `##`/`###` section, fences respected."""
    out, title, body, in_fence = [], "(intro)", [], False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        m = None if in_fence else re.match(r"#{2,3} (\S.*)", line)
        if m:
            out.append((title, "\n".join(body)))
            title, body = m.group(1), []
        else:
            body.append(line)
    out.append((title, "\n".join(body)))
    return out


def tool_names() -> set[str]:
    catalog = json.loads((ROOT / "product" / "tool-catalog-runtime.json").read_text())
    return {t["name"] for t in catalog}


def block_signature(info: str, body: str, tools: set[str]) -> tuple:
    """What a fenced block says regardless of the language of its prose.

    Comments and translated placeholders (`path` / `ruta`) are not compared;
    the number of live lines, the leading command of each shell line, flags,
    numbers, URLs and published tool names are.
    """
    lines = []
    for raw in body.splitlines():
        line = re.sub(r"\s+#.*$", "", raw).strip()
        if line and not line.startswith("#"):
            lines.append(line)
    text = "\n".join(lines)
    lead = (
        tuple(l.split()[0] for l in lines) if info.strip() in SHELL_INFO else ()
    )
    return (
        info.strip(),
        len(lines),
        lead,
        tuple(sorted(FLAG.findall(text))),
        tuple(sorted(NUMBER.findall(text))),
        tuple(sorted(URL.findall(text))),
        tuple(sorted(w for w in WORD.findall(text) if w in tools)),
    )


def section_facts(text: str, tools: set[str]) -> tuple[list[tuple], list[str]]:
    blocks = [block_signature(i, b, tools) for i, b in FENCED.findall(text)]
    urls = sorted({u.rstrip(".,;:") for u in URL.findall(text)})
    return blocks, urls


def parity_differences(en: Path, es: Path, tools: set[str]) -> list[str]:
    en_s, es_s = readme_sections(en), readme_sections(es)
    if len(en_s) != len(es_s):
        return [f"section count differs: EN {len(en_s)}, ES {len(es_s)}"]
    found = []
    for (t_en, b_en), (t_es, b_es) in zip(en_s, es_s):
        blocks_en, urls_en = section_facts(b_en, tools)
        blocks_es, urls_es = section_facts(b_es, tools)
        if blocks_en != blocks_es:
            found.append(f"'{t_en}' / '{t_es}': fenced blocks differ")
        if urls_en != urls_es:
            found.append(f"'{t_en}' / '{t_es}': URLs differ")
    return found


CLI_SOURCE = ROOT / "crates" / "cognicode-cli" / "src" / "cli" / "commands.rs"
PACKAGE_FLAG = re.compile(r"(?:-p|--package)\s+([\w-]+)")
FEATURE_FLAG = re.compile(r"--features\s+([\w/,-]+)")
BIN_FLAG = re.compile(r"--bin\s+([\w-]+)")
TOOL_FIELD = re.compile(r'"tool":\s*"([\w-]+)"')
# `cognicode <group> <sub>` where the group has its own clap enum.
CLI_GROUPS = {"index": "IndexCommand", "graph": "GraphCommand", "navigate": "NavigateCommand"}


def workspace_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    return json.loads(out.stdout)


def kebab(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def clap_variants(source: str, enum: str) -> set[str]:
    """Kebab-case variant names of a clap enum, plus explicit `name = \"x\"` renames."""
    m = re.search(r"(?:pub )?enum %s \{(.*?)\n\}" % enum, source, re.S)
    if not m:
        return set()
    body = m.group(1)
    names = {kebab(v) for v in re.findall(r"^    ([A-Z]\w*)", body, re.M)}
    return names | set(re.findall(r'name\s*=\s*"([\w-]+)"', body))


def cli_references(text: str) -> tuple[set[str], dict[str, set[str]], bool]:
    """(top-level `cognicode X` words, {group: sub words}, refactor lists operations positionally)."""
    top, subs, positional = set(), {g: set() for g in CLI_GROUPS}, False
    for span in code_spans(text):
        for line in span.splitlines():
            m = re.match(r"\s*(?:\./target/\w+/)?cognicode ([a-z][\w-]*)(?: ([\w|-]+))?", line)
            if not m:
                continue
            top.add(m.group(1))
            if m.group(1) in CLI_GROUPS and m.group(2) and "<" not in m.group(2):
                subs[m.group(1)].update(m.group(2).split("|"))
            if m.group(1) == "refactor" and "|" in (m.group(2) or "") and "--operation" not in line:
                positional = True
    return top, subs, positional


def build_references(text: str) -> tuple[set[str], set[str], set[str], set[str]]:
    spans = "\n".join(code_spans(text))
    feats = {f for g in FEATURE_FLAG.findall(spans) for f in g.split(",")}
    return (
        set(PACKAGE_FLAG.findall(spans)), feats,
        set(BIN_FLAG.findall(spans)), set(TOOL_FIELD.findall(spans)),
    )


def readme_reference_gaps(text: str, meta: dict, files: set[str], tools: set[str]) -> list[str]:
    packages = {p["name"]: p for p in meta["packages"]}
    features = {f for p in packages.values() for f in p["features"]}
    bins = {t["name"] for p in packages.values() for t in p["targets"] if "bin" in t["kind"]}
    pkgs, feats, cited_bins, cited_tools = build_references(text)
    gaps = [f"package {x}" for x in sorted(pkgs - set(packages))]
    gaps += [f"feature {x}" for x in sorted(f for f in feats if f.split("/")[-1] not in features)]
    gaps += [f"bin {x}" for x in sorted(cited_bins - bins)]
    gaps += [f"tool {x}" for x in sorted(cited_tools - tools)]
    gaps += [f"path {x}" for x in missing_references(text, files, set())[0]]
    source = CLI_SOURCE.read_text(encoding="utf-8")
    top, subs, positional = cli_references(text)
    known = clap_variants(source, "CliCommand")
    gaps += [f"cognicode {x}" for x in sorted(top - known - {"help"})]
    for group, enum in CLI_GROUPS.items():
        gaps += [f"cognicode {group} {x}" for x in sorted(subs[group] - clap_variants(source, enum))]
    if positional:
        gaps.append("cognicode refactor lists operations positionally; clap takes --operation")
    return gaps


CLI_SRC = ROOT / "crates" / "cognicode-cli" / "src"
# binary -> (sources defining its clap surface, enum holding its subcommands)
BINARY_SURFACES = {
    "cognicode": ([CLI_SRC / "main.rs", *sorted((CLI_SRC / "cli").glob("*.rs"))], "CliCommand"),
    "cogh": ([CLI_SRC / "bin" / "cogh.rs", *sorted((CLI_SRC / "cmd").glob("*.rs"))], "Command"),
    "cognicode-release": ([CLI_SRC / "bin" / "release.rs"], "Command"),
    "cognicode-mcp": ([ROOT / "crates" / "cognicode-mcp" / "src" / "main.rs"], None),
}
ARG_ATTR = re.compile(r"#\[(?:arg|clap)\(([^\]]*?)\)\]\s*(?:pub\s+)?(\w+)\s*:", re.S)
BINARY_CALL = re.compile(r"(?:^|[\s/])(cogh|cognicode-release|cognicode-mcp|cognicode)(?=\s|$)(.*)$")


def clap_flags(sources: list[Path]) -> set[str]:
    """Long and short flags a clap-derive source declares, plus the implicit ones."""
    flags = {"--help", "-h", "--version", "-V"}
    for src in sources:
        for attr, field in ARG_ATTR.findall(src.read_text(encoding="utf-8")):
            if re.search(r"\blong\b", attr):
                named = re.search(r'long\s*=\s*"([\w-]+)"', attr)
                flags.add("--" + (named.group(1) if named else field.replace("_", "-")))
            if re.search(r"\bshort\b", attr):
                named = re.search(r"short\s*=\s*'(\w)'", attr)
                flags.add("-" + (named.group(1) if named else field[0]))
    return flags


def binary_references(text: str) -> dict[str, tuple[set[str], set[str]]]:
    """{binary: (flags, first subcommand words)} cited in code spans."""
    found = {b: (set(), set()) for b in BINARY_SURFACES}
    for span in code_spans(text):
        for line in span.splitlines():
            line = re.sub(r"\s+#.*$", "", line)
            for binary, rest in BINARY_CALL.findall(line):
                flags, subs = found[binary]
                flags.update(re.findall(r"(?<![\w-])(--?[A-Za-z][\w-]*)", rest))
                word = re.match(r"\s+([a-z][\w-]*)", rest)
                if word:
                    subs.add(word.group(1))
    return found


def binary_surface_gaps(text: str) -> list[str]:
    gaps = []
    for binary, (flags, subs) in binary_references(text).items():
        sources, enum = BINARY_SURFACES[binary]
        known_flags = clap_flags(sources)
        gaps += [f"{binary} {f}" for f in sorted(flags - known_flags)]
        if enum:
            joined = "\n".join(src.read_text(encoding="utf-8") for src in sources)
            known = clap_variants(joined, enum) | {"help"}
            gaps += [f"{binary} {w}" for w in sorted(subs - known)]
    return gaps


COMMANDS_RS = CLI_SRC / "cli" / "commands.rs"
STRATEGY_RS = ROOT / "crates" / "cognicode-core" / "src" / "infrastructure" / "graph" / "strategy.rs"
MERMAID_RS = CLI_SRC / "cli" / "mermaid_format.rs"
FLAG_VALUES = re.compile(
    r"cognicode\s+(refactor|index build|graph mermaid)\b[^\n]*?(--operation|--strategy|--format)"
    r"\s+((?:[a-z_]+\|)+[a-z_]+)"
)


def closed_flag_values() -> dict[tuple[str, str], set[str]]:
    """(command, flag) -> the values the sources accept, read from the sources."""
    strategy = STRATEGY_RS.read_text(encoding="utf-8")
    listed = re.search(r"fn available_strategies\(\)[^{]*\{\s*vec!\[([^\]]*)\]", strategy)
    return {
        ("refactor", "--operation"): clap_variants(COMMANDS_RS.read_text(encoding="utf-8"), "RefactorOperation"),
        ("index build", "--strategy"): set(re.findall(r'"(\w+)"', listed.group(1))) if listed else set(),
        ("graph mermaid", "--format"): set(
            re.findall(r'\("(\w+)",\s*MermaidFormat', MERMAID_RS.read_text(encoding="utf-8"))
        ),
    }


def flag_value_gaps(text: str, closed: dict[tuple[str, str], set[str]]) -> list[str]:
    gaps = []
    for span in code_spans(text):
        for command, flag, values in FLAG_VALUES.findall(span):
            accepted = closed[(command, flag)]
            gaps += [
                f"cognicode {command} {flag} {v}" for v in values.split("|") if v not in accepted
            ]
    return gaps


def tool_schemas() -> dict[str, dict]:
    catalog = json.loads((ROOT / "product" / "tool-catalog-runtime.json").read_text())
    return {entry["name"]: entry["inputSchema"] for entry in catalog}


def tool_example_gaps(text: str, schemas: dict[str, dict]) -> tuple[int, list[str]]:
    """(examples checked, gaps) for the ```json {"tool", "arguments"} examples."""
    checked, gaps = 0, []
    for info, body in re.findall(r"```(\w*)[^\n]*\n(.*?)```", text, re.S):
        if info != "json":
            continue
        try:
            call = json.loads(body)
        except ValueError:
            continue
        if not (isinstance(call, dict) and "tool" in call and "arguments" in call):
            continue
        checked += 1
        name, args = call["tool"], call["arguments"]
        schema = schemas.get(name)
        if schema is None:
            gaps.append(f"{name}: no such tool")
            continue
        props = schema.get("properties", {})
        gaps += [f"{name}: argument '{k}' is not in the inputSchema" for k in sorted(set(args) - set(props))]
        gaps += [f"{name}: required '{k}' is missing" for k in schema.get("required", []) if k not in args]
        for key, value in args.items():
            allowed = props.get(key, {}).get("enum")
            if allowed and value not in allowed:
                gaps.append(f"{name}: {key}={value!r} is not one of {allowed}")
    return checked, gaps


TOOL_GROUP = re.compile(
    r"^#{2,4} (.+?) \((\d+) (?:tools|herramientas)\)\s*\n\n(?:\|[^\n]*\n)+", re.M
)


def tool_table_gaps(text: str, names: set[str]) -> tuple[int, list[str]]:
    """(tool rows checked, gaps) for the grouped tool tables of a README."""
    checked, gaps = 0, []
    for match in TOOL_GROUP.finditer(text):
        group, declared = match.group(1), int(match.group(2))
        rows = re.findall(r"^\|\s*`([\w-]+)`\s*\|", match.group(0), re.M)
        checked += len(rows)
        gaps += [f"{group}: '{r}' is not a published tool" for r in rows if r not in names]
        if len(rows) != declared:
            gaps.append(f"{group}: heading says {declared}, table lists {len(rows)}")
    return checked, gaps


MCP_MAIN = ROOT / "crates" / "cognicode-mcp" / "src" / "main.rs"
SURFACE_RS = ROOT / "crates" / "cognicode-core" / "src" / "interface" / "surface.rs"
MCP_GUIDE = ROOT / "docs" / "distribution" / "INSTALL.md"
MCP_SECTION = "Registering cognicode-mcp in an MCP client"
IMPLICIT_FLAGS = {"--help", "-h", "--version", "-V"}


def guide_section(text: str, title: str) -> str | None:
    """Body of the `## title` section of a guide, or None when it is absent."""
    match = re.search(rf"^## {re.escape(title)}\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    return match.group(1) if match else None


def mcp_section_gaps(body: str | None, flags: set[str], profiles: set[str], env: set[str]) -> list[str]:
    """What the MCP registration section fails to say, given the sources' facts."""
    if body is None:
        return ["section missing"]
    gaps = []
    if not re.search(r'"command":\s*"cognicode-mcp"', body):
        gaps.append("no client configuration with the cognicode-mcp command")
    long_flags = {f for f in flags - IMPLICIT_FLAGS if f.startswith("--")}
    gaps += [f"flag not documented: {f}" for f in sorted(long_flags) if f not in body]
    cited = set(re.findall(r"(?<![\w-])(--?[A-Za-z][\w-]*)", body))
    gaps += [f"unknown flag cited: {f}" for f in sorted(cited - flags)]
    gaps += [f"profile not documented: {p}" for p in sorted(profiles) if f"`{p}`" not in body]
    gaps += [f"environment variable not documented: {v}" for v in sorted(env) if v not in body]
    if not ("stdout" in body and "stderr" in body and "JSON-RPC" in body):
        gaps.append("stdout/stderr rule for JSON-RPC and logs is not stated")
    if not re.search(r"(?i)limitation", body):
        gaps.append("no explicit limitation about published releases")
    return gaps


RELEASE_RS = ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "release_contract.rs"
PLATFORMS_JSON = ROOT / "product" / "platforms.json"
RELEASE_SECTION = "Release assets"
ASSET_NAME = re.compile(r"[\w.-]+\.(?:tar\.gz|yaml|json)\b")


def release_facts() -> dict:
    """What a release publishes, read from the contract source and platforms.json."""
    src = RELEASE_RS.read_text(encoding="utf-8")
    platforms = json.loads(PLATFORMS_JSON.read_text(encoding="utf-8"))["platforms"]
    certified = {p["target"] for p in platforms if p["support_level"] == "certified"}
    arch = lambda target: target.split("-")[0]  # noqa: E731
    uncertified = {arch(p["target"]) for p in platforms} - {arch(t) for t in certified}
    stems = dict(re.findall(r'ArtifactKind::(\w+) => "([^"]+)",', src))
    components = re.search(r"pub const COMPONENTS.*?\n\];", src, re.S).group(0)
    published = re.findall(r"kind: ArtifactKind::(\w+),.*?published: (true|false)", components, re.S)
    bundles = re.search(r"pub const SKILL_BUNDLES.*?\n\];", src, re.S).group(0)
    bundle_ids = [i for i, p in re.findall(r'id: "([^"]+)",.*?published: (true|false)', bundles, re.S) if p == "true"]
    templates = dict(re.findall(r"pub fn (\w+)\([^)]*\) -> String \{\s*format!\(\"([^\"]+)\"", src))
    return {
        "certified": certified,
        "uncertified_arches": uncertified,
        "stems": [stems[k] for k, p in published if p == "true"],
        "bundle_ids": bundle_ids,
        "templates": templates,
    }


def expected_assets(facts: dict, version: str) -> set[str]:
    """Asset file names of one release, rendered from the templates of the source."""
    t, names = facts["templates"], {"SHA256SUMS"}
    for target in facts["certified"]:
        for stem in facts["stems"]:
            names.add(t["artifact_filename"].replace("{stem}", stem).replace("{version}", version).replace("{}", target))
        names.add(t["bundle_manifest_filename"].replace("{version}", version).replace("{}", target))
    names.add(t["release_inventory_filename"].replace("{version}", version))
    for bundle in facts["bundle_ids"]:
        names.add(t["skill_bundle_filename"].replace("{id}", bundle).replace("{version}", version))
    return names


def release_section_gaps(body: str | None, assets: set[str]) -> list[str]:
    """What the release-assets section fails to say, given the contract's names."""
    if body is None:
        return ["section missing"]
    gaps = [f"asset not documented: {a}" for a in sorted(assets) if a not in body]
    gaps += [f"unknown asset cited: {a}" for a in sorted(set(ASSET_NAME.findall(body)) - assets) if a.count(".") > 0 and "-" in a]
    if "sha256sum --check" not in body or "SHA256SUMS" not in body:
        gaps.append("integrity verification against SHA256SUMS is not described")
    if not re.search(r"(?i)limitation", body):
        gaps.append("no explicit limitation about published releases")
    return gaps


def published_matrix_gaps(text: str, certified: set[str], uncertified_arches: set[str]) -> list[str]:
    """The sentence naming the published platform matrix must match platforms.json."""
    sentences = [s for s in re.split(r"(?<=\.)\s+", " ".join(text.split())) if "matriz publicada" in s]
    if not sentences:
        return ["no sentence states the published platform matrix"]
    gaps = []
    for sentence in sentences:
        gaps += [f"matrix names a platform that is not certified: {a}" for a in sorted(uncertified_arches) if a in sentence]
        gaps += [f"matrix omits a certified platform: {t.split('-')[0]}" for t in sorted(certified) if t.split("-")[0] not in sentence]
    return gaps


WORKFLOWS_SECTION = "CLI workflows"
# Commands the section must walk through; the flags and values come from the sources.
WORKFLOW_COMMANDS = ("index build", "graph full", "graph hot-paths", "graph mermaid", "refactor", "doctor")
SHELL_BLOCK = re.compile(r"```(?:bash|sh)\n(.*?)```", re.S)
COGNICODE_LINE = re.compile(r"^\s*cognicode\s+(\S.*)$")


def cli_workflow_gaps(body: str | None, closed: dict[tuple[str, str], set[str]]) -> list[str]:
    """What the CLI workflows section fails to say, given the clap sources' facts."""
    if body is None:
        return ["section missing"]
    gaps = []
    if "written in English" not in body:
        gaps.append("no language notice")
    chains = [
        [m.group(1) for line in block.splitlines() if (m := COGNICODE_LINE.match(line))]
        for block in SHELL_BLOCK.findall(body)
    ]
    if not chains:
        gaps.append("no shell block")
    gaps += [f"block with fewer than two chained commands: {c}" for c in chains if len(c) < 2]
    calls = [call for chain in chains for call in chain]
    for command in WORKFLOW_COMMANDS:
        if not any(call == command or call.startswith(command + " ") for call in calls):
            gaps.append(f"command not walked through: {command}")
    joined = COMMANDS_RS.read_text(encoding="utf-8")
    for group, enum in CLI_GROUPS.items():
        known = clap_variants(joined, enum)
        for call in calls:
            words = call.split()
            if words[0] == group and (len(words) < 2 or words[1] not in known):
                gaps.append(f"unknown subcommand: {group} {words[1] if len(words) > 1 else ''}".rstrip())
    for (command, flag), accepted in sorted(closed.items()):
        for call in calls:
            if call == command or call.startswith(command + " "):
                value = re.search(rf"{re.escape(flag)}[ =](\w+)", call)
                if value and value.group(1) not in accepted:
                    gaps.append(f"value not accepted: {command} {flag} {value.group(1)}")
    for (command, flag), accepted in sorted(closed.items()):
        gaps += [f"value not documented: {command} {flag} {v}" for v in sorted(accepted) if f"`{v}`" not in body]
    gaps += [f"not in the clap sources: {g}" for g in binary_surface_gaps(body)]
    if not re.search(r"(?i)limitation", body):
        gaps.append("no explicit limitation about published releases")
    return gaps


SECURITY_HEADINGS = {EN: "Security and limitations", ES: "Seguridad y límites"}
LINK_TRACE_RS = ROOT / "crates" / "cognicode-core" / "src" / "interface" / "mcp" / "link_trace.rs"


def security_section_gaps(body: str | None, flags: set[str], env: set[str]) -> list[str]:
    """What a README security section fails to say, given facts read from the sources."""
    if body is None:
        return ["section missing"]
    gaps = []
    if "SECURITY.md" not in body:
        gaps.append("no link to SECURITY.md")
    gaps += [f"flag not documented: {f}" for f in sorted(flags) if f"`{f}`" not in body]
    gaps += [f"environment variable not documented: {v}" for v in sorted(env) if v not in body]
    if not ("stdout" in body and "stderr" in body):
        gaps.append("no stdout/stderr rule")
    return gaps

ENV_HEADINGS = {EN: "Environment Variables", ES: "Variables de Entorno"}
TROUBLE_HEADINGS = {EN: "Troubleshooting", ES: "Solución de problemas"}
EXIT_PHRASE = {EN: "exit status {}", ES: "estado de salida {}"}


def subsection(text: str, title: str) -> str | None:
    """Body of the `### title` subsection, or None when it is absent."""
    match = re.search(rf"^### {re.escape(title)}\n(.*?)(?=^#{{2,3}} |\Z)", text, re.S | re.M)
    return match.group(1) if match else None


def env_table_gaps(body: str | None, expected: dict[str, str | None]) -> list[str]:
    """Rows a README environment table lacks; a value, when given, must be in the row."""
    if body is None:
        return ["section missing"]
    rows = {m.group(1): m.group(0) for m in re.finditer(r"^\|\s*`(\w+)`\s*\|.*$", body, re.M)}
    gaps = []
    for name, value in sorted(expected.items()):
        if name not in rows:
            gaps.append(f"row missing: {name}")
        elif value is not None and value not in rows[name]:
            gaps.append(f"value missing in row {name}: {value}")
    if "OTEL_EXPORTER_OTLP_ENDPOINT" in rows and "COGNICODE_TELEMETRY" not in rows["OTEL_EXPORTER_OTLP_ENDPOINT"]:
        gaps.append("OTEL_EXPORTER_OTLP_ENDPOINT row does not say it needs COGNICODE_TELEMETRY")
    return gaps


def troubleshooting_gaps(body: str | None, phrase: str, facts: dict) -> list[str]:
    """What a troubleshooting section fails to say, given facts read from the sources."""
    if body is None:
        return ["section missing"]
    gaps = []
    gaps += [f"exit code not stated: {c}" for c in sorted(facts["exits"]) if phrase.format(c) not in body]
    gaps += [f"flag not documented: {f}" for f in sorted(facts["flags"]) if f"`{f}`" not in body]
    gaps += [f"profile not documented: {p}" for p in sorted(facts["profiles"]) if f"`{p}`" not in body]
    cited = set(re.findall(r"(?<![\w-])(--?[A-Za-z][\w-]*)", "\n".join(re.findall(r"`([^`\n]+)`", body))))
    gaps += [f"unknown flag cited: {f}" for f in sorted(cited - facts["known"])]
    if "cognicode doctor" not in body:
        gaps.append("no cognicode doctor check")
    if not ("stdout" in body and "stderr" in body and "JSON-RPC" in body):
        gaps.append("no stdout/stderr rule for JSON-RPC")
    return gaps


def doctor_flags(block: str) -> set[str]:
    """Flags declared by the fields of the `Doctor` clap variant."""
    flags = set()
    for attr, field in ARG_ATTR.findall(block):
        if re.search(r"\blong\b", attr):
            flags.add("--" + field.replace("_", "-"))
        if re.search(r"\bshort\b", attr):
            flags.add("-" + field[0])
    return flags


def troubleshooting_facts() -> dict:
    main = MCP_MAIN.read_text(encoding="utf-8")
    doctor = re.search(r"Doctor\s*\{(.*?)\n    \}", COMMANDS_RS.read_text(encoding="utf-8"), re.S)
    return {
        "exits": set(re.findall(r"process::exit\((\d+)\)", main)),
        "flags": {"--cwd", "--profile"} & clap_flags([MCP_MAIN]),
        "profiles": set(re.findall(r'"(\w+)" => Some\(SurfaceProfile::', SURFACE_RS.read_text(encoding="utf-8"))),
        "known": clap_flags([MCP_MAIN, COMMANDS_RS]),
        "doctor": doctor.group(1) if doctor else "",
    }


CRATE_HEADINGS = {EN: "Workspace Crates", ES: "Crates del Workspace"}
SDK_GUIDE_LINK = "docs/distribution/INSTALL.md#consuming-the-sdk"


def crate_table_names(body: str | None) -> list[str]:
    """Crate names in the first column of the markdown table of a section."""
    if body is None:
        return []
    return re.findall(r"^\| `([^`]+)` \|", body, re.M)


def crate_table_gaps(body: str | None, packages: set[str], libraries: set[str], text: str) -> list[str]:
    """What a workspace-crates section gets wrong about the real workspace."""
    names = crate_table_names(body)
    if not names:
        return ["table missing or empty"]
    gaps = [f"{n}: not a workspace package" for n in names if n not in packages]
    gaps += [f"{n}: SDK library crate not listed" for n in sorted(libraries - set(names))]
    if SDK_GUIDE_LINK not in text:
        gaps.append("no link to the SDK consumption guide")
    return gaps


def sdk_library_crates() -> set[str]:
    declared = json.loads((ROOT / "docs" / "distribution" / "SDK_SURFACES.json").read_text(encoding="utf-8"))
    return {n for n, info in declared["crates"].items() if info["class"] == "library"}


class ReadmeContract(unittest.TestCase):
    def test_tool_tables_list_only_published_tools_and_their_counts_hold(self):
        names = tool_names()
        for readme in (EN, ES):
            checked, gaps = tool_table_gaps(readme.read_text(encoding="utf-8"), names)
            self.assertGreater(checked, 20, f"{readme.name}: tool tables not found")
            self.assertEqual([], gaps, readme.name)

    def test_tool_table_check_discriminates(self):
        table = "### G ({n} tools)\n\n| Tool | D |\n|---|---|\n{rows}\n"
        good = table.format(n=1, rows="| `a` | x |")
        self.assertEqual((1, []), tool_table_gaps(good, {"a"}))
        ghost = tool_table_gaps(table.format(n=2, rows="| `a` | x |\n| `ghost` | y |"), {"a"})[1]
        self.assertTrue(any("'ghost' is not a published tool" in g for g in ghost), ghost)
        count = tool_table_gaps(table.format(n=3, rows="| `a` | x |"), {"a"})[1]
        self.assertTrue(any("heading says 3, table lists 1" in g for g in count), count)
        es = "### G (1 herramientas)\n\n| Herramienta | D |\n|---|---|\n| `ghost` | y |\n"
        self.assertTrue(any("'ghost'" in g for g in tool_table_gaps(es, {"a"})[1]))

    def test_both_readmes_have_the_same_heading_outline(self):
        en, es = outline(EN), outline(ES)
        self.assertTrue(en and es, "a README without headings proves nothing")
        self.assertEqual(
            en, es, f"heading outline differs: EN has {len(en)}, ES has {len(es)}"
        )

    def test_pinned_versions_are_the_workspace_version(self):
        expected = workspace_version()
        for path in (EN, ES):
            found = pinned_versions(path)
            self.assertTrue(found, f"{path.name} pins no version: nothing checked")
            stale = sorted({v for v in found if v != expected})
            self.assertEqual([], stale, f"{path.name} pins stale versions")

    def test_relative_links_exist(self):
        for path in (EN, ES):
            links = local_links(path)
            self.assertTrue(links, f"{path.name} has no local link: nothing checked")
            missing = sorted({l for l in links if not (ROOT / l).exists()})
            self.assertEqual([], missing, f"{path.name} links to missing files")

    def test_install_guides_pin_the_workspace_version(self):
        expected = workspace_version()
        docs = install_docs()
        self.assertGreaterEqual(len(docs), 3, "install guides not found: nothing checked")
        all_found = []
        for path in docs:
            found = guide_versions(path)
            all_found += found
            stale = sorted({v for v in found if v != expected})
            self.assertEqual([], stale, f"{path.relative_to(ROOT)} pins stale versions")
        self.assertTrue(all_found, "no install guide pins a version: nothing checked")

    def test_install_guide_links_exist_relative_to_the_document(self):
        total = 0
        for path in install_docs():
            links = local_links(path)
            total += len(links)
            missing = sorted(
                {l for l in links if not (path.parent / l).exists()}
            )
            self.assertEqual(
                [], missing, f"{path.relative_to(ROOT)} links to missing files"
            )
        self.assertTrue(total, "no install guide has a local link: nothing checked")

    def test_root_markdown_links_exist_relative_to_the_document(self):
        roots = root_markdown()
        total = 0
        for path in roots:
            links = local_links(path)
            total += len(links)
            self.assertEqual(
                [], missing_links(path, links), f"{path.name} links to missing files"
            )
        self.assertGreaterEqual(len(roots), 8, "too few root documents: nothing checked")
        self.assertTrue(total, "no root document has a local link: nothing checked")

    def test_the_root_link_check_discriminates(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "real.md").write_text("x", encoding="utf-8")
            doc = base / "doc.md"
            doc.write_text(
                "[ok](real.md#a) [gone](docs/ghost.md#b) [web](https://x.y) [a](#top)",
                encoding="utf-8",
            )
            self.assertEqual(["docs/ghost.md"], missing_links(doc, local_links(doc)))
            doc.write_text("[ok](real.md)", encoding="utf-8")
            self.assertEqual([], missing_links(doc, local_links(doc)))

    def test_developer_guides_cite_scripts_paths_and_recipes_that_exist(self):
        files, recipes = tracked_files(), justfile_recipes()
        self.assertTrue(recipes, "justfile has no recipe: nothing to compare against")
        tops = {f.split("/", 1)[0] for f in files}
        cited_paths = cited_recipes = 0
        for path in install_docs():
            text = path.read_text(encoding="utf-8")
            paths, cited = dev_references(text, tops)
            cited_paths += len(paths)
            cited_recipes += len(cited)
            gone, unknown = missing_references(text, files, recipes)
            if untracked_on_disk(gone):
                self.fail(untracked_message(path, gone))
            self.assertEqual([], gone, f"{path.name} cites paths that do not exist")
            self.assertEqual([], unknown, f"{path.name} cites unknown just recipes")
        self.assertTrue(cited_paths, "no guide cites a repo path: nothing checked")
        self.assertTrue(cited_recipes, "no guide cites a just recipe: nothing checked")

    def test_untracked_citation_message_names_the_file_and_the_fix(self):
        real = "scripts/ci/test_readme_contract.py"
        self.assertEqual([real], untracked_on_disk([real, "scripts/ci/ghost_nowhere.py"]))
        msg = untracked_message(Path("GUIDE.md"), [real, "scripts/ci/ghost_nowhere.py"])
        self.assertIn(f"{real}, which exists but is not in the git index; run git add", msg)
        self.assertNotIn("ghost_nowhere", msg)

    def test_guides_do_not_require_pytest(self):
        # The repo declares no pytest and its harness is pytest-free
        # (test_ci_contracts), so a guide that needs it fails on a clean checkout.
        for path in install_docs():
            spans = code_spans(path.read_text(encoding="utf-8"))
            offending = [s for s in spans if re.search(r"\bpytest\b", s)]
            self.assertEqual([], offending, f"{path.name} requires pytest")

    def test_the_reference_check_discriminates_and_honours_illustrative(self):
        files, recipes = {"scripts/real.sh", "docs/a.md"}, {"check"}
        bad = "Run `bash scripts/ghost.sh` then `just nope` and `just check`."
        self.assertEqual(
            (["scripts/ghost.sh"], ["nope"]), missing_references(bad, files, recipes)
        )
        skipped = "```bash illustrative\nbash scripts/ghost.sh\njust nope\n```"
        self.assertEqual(([], []), missing_references(skipped, files, recipes))
        placeholder = "`bash scripts/<name>.sh` and `docs/*.md`"
        self.assertEqual(([], []), missing_references(placeholder, files, recipes))

    def test_fenced_blocks_and_urls_match_section_by_section(self):
        tools = tool_names()
        self.assertTrue(tools, "empty tool catalogue: tool names cannot be compared")
        blocks = urls = named = 0
        for _, body in readme_sections(EN):
            sections_blocks, sections_urls = section_facts(body, tools)
            blocks += len(sections_blocks)
            urls += len(sections_urls)
            named += sum(len(b[-1]) for b in sections_blocks)
        self.assertTrue(blocks and urls, "README has no fenced block or URL: nothing checked")
        self.assertTrue(named, "no block names a published tool: catalogue check is vacuous")
        self.assertEqual([], parity_differences(EN, ES, tools))

    def test_parity_check_discriminates(self):
        import tempfile

        tools = {"build_graph"}
        en = "## A\n```bash\ncargo build --release # comment\nrun --version\n```\nsee https://x.io/a\n"
        ok = "## B\n```bash\ncargo build --release # comentario\nrun --version\n```\nver https://x.io/a\n"
        short = "## B\n```bash\ncargo build --release\n```\nver https://x.io/a\n"
        flag = ok.replace("--release", "--debug")
        url = ok.replace("x.io/a", "x.io/b")
        tool = "## B\n```json\n{ \"tool\": \"build_graph\" }\n```\n"
        tool_en = tool.replace("build_graph", "trace_path")
        with tempfile.TemporaryDirectory() as d:
            def write(name, text):
                path = Path(d) / name
                path.write_text(text, encoding="utf-8")
                return path
            base = write("en.md", en)
            self.assertEqual([], parity_differences(base, write("ok.md", ok), tools))
            for name, text in (("short", short), ("flag", flag), ("url", url)):
                self.assertTrue(
                    parity_differences(base, write(name + ".md", text), tools), name
                )
            self.assertTrue(
                parity_differences(
                    write("t1.md", tool), write("t2.md", tool_en), {"build_graph", "trace_path"}
                )
            )


    def test_readme_cites_only_packages_features_paths_tools_and_commands_that_exist(self):
        meta, files, tools = workspace_metadata(), tracked_files(), tool_names()
        for readme in (EN, ES):
            text = without_history(readme)
            pkgs, feats, _, cited_tools = build_references(text)
            top, subs, _ = cli_references(text)
            self.assertTrue(pkgs and feats and cited_tools, f"{readme.name}: nothing to check")
            self.assertTrue(top and subs["graph"], f"{readme.name}: no cognicode commands found")
            self.assertEqual([], readme_reference_gaps(text, meta, files, tools), readme.name)

    def test_reference_gap_check_discriminates(self):
        meta = {"packages": [{"name": "real-pkg", "features": {"real-feat": []},
                              "targets": [{"name": "real-bin", "kind": ["bin"]}]}]}
        tools, files = {"real_tool"}, {"docs/a.md"}
        good = ("`cargo build -p real-pkg --features real-feat --bin real-bin`\n"
                "`cognicode graph full`\n`docs/a.md`\n"
                '```json\n{ "tool": "real_tool" }\n```\n')
        self.assertEqual([], readme_reference_gaps(good, meta, files, tools))
        for bad, label in (
            ("`cargo build -p ghost-pkg`", "package"),
            ("`cargo build --features ghost-feat`", "feature"),
            ("`cargo build --bin ghost-bin`", "bin"),
            ("`docs/ghost.md`", "path"),
            ('```json\n{ "tool": "ghost_tool" }\n```', "tool"),
            ("`cognicode ghost`", "cognicode ghost"),
            ("`cognicode graph ghost`", "cognicode graph ghost"),
            ("`cognicode refactor rename|extract <symbol>`", "positionally"),
        ):
            gaps = readme_reference_gaps(bad, meta, files, tools)
            self.assertTrue(any(label in g for g in gaps), (label, gaps))


    def test_readme_flags_and_subcommands_exist_in_the_clap_sources(self):
        for readme in (EN, ES):
            text = readme.read_text(encoding="utf-8")
            cited = binary_references(text)
            for binary in ("cognicode", "cogh", "cognicode-mcp"):
                flags, subs = cited[binary]
                self.assertTrue(flags or subs, f"{readme.name}: {binary} is never cited")
            self.assertEqual([], binary_surface_gaps(text), readme.name)

    def test_the_install_guide_explains_how_to_register_the_mcp_server(self):
        flags = clap_flags([MCP_MAIN])
        profiles = set(re.findall(r'"(\w+)" => Some\(SurfaceProfile::', SURFACE_RS.read_text(encoding="utf-8")))
        env = set(re.findall(r'var\("(COGNICODE_\w+)"\)', MCP_MAIN.read_text(encoding="utf-8")))
        self.assertTrue({"--cwd", "--read-only", "--profile"} <= flags, "clap flags not read")
        self.assertTrue(profiles and env, "profiles or environment not read: the check would be vacuous")
        body = guide_section(MCP_GUIDE.read_text(encoding="utf-8"), MCP_SECTION)
        self.assertEqual([], mcp_section_gaps(body, flags, profiles, env))

    def test_the_mcp_section_check_discriminates(self):
        flags, profiles, env = {"--cwd", "--help"}, {"full"}, {"COGNICODE_X"}
        good = (
            '```json\n{"command": "cognicode-mcp"}\n```\n`--cwd` `full` COGNICODE_X\n'
            "JSON-RPC on stdout, logs on stderr.\nLimitation: not published.\n"
        )
        self.assertEqual([], mcp_section_gaps(good, flags, profiles, env))
        self.assertIsNone(guide_section("## Other\nx", MCP_SECTION))
        self.assertEqual(["section missing"], mcp_section_gaps(None, flags, profiles, env))
        for bad, label in (
            (good.replace("cognicode-mcp", "other"), "no client configuration"),
            (good.replace("`--cwd`", ""), "flag not documented: --cwd"),
            (good + " --ghost", "unknown flag cited: --ghost"),
            (good.replace("`full`", ""), "profile not documented: full"),
            (good.replace("COGNICODE_X", ""), "environment variable not documented"),
            (good.replace("stderr", "log"), "stdout/stderr rule"),
            (good.replace("Limitation", "Note"), "no explicit limitation"),
        ):
            self.assertTrue(any(label in g for g in mcp_section_gaps(bad, flags, profiles, env)), label)

    def test_the_install_guide_lists_the_release_assets_and_the_certified_matrix(self):
        facts = release_facts()
        assets = expected_assets(facts, workspace_version())
        self.assertTrue(facts["certified"] and facts["stems"] and facts["bundle_ids"], "facts not read")
        self.assertTrue(facts["uncertified_arches"], "no uncertified arch read: the matrix check would be vacuous")
        self.assertLessEqual(
            {"artifact_filename", "bundle_manifest_filename", "release_inventory_filename", "skill_bundle_filename"},
            set(facts["templates"]), "name templates not read from release_contract.rs",
        )
        self.assertGreater(len(assets), 5, "too few assets derived: the check would be vacuous")
        guide = MCP_GUIDE.read_text(encoding="utf-8")
        self.assertEqual([], release_section_gaps(guide_section(guide, RELEASE_SECTION), assets))
        self.assertEqual([], published_matrix_gaps(guide, facts["certified"], facts["uncertified_arches"]))

    def test_the_release_asset_checks_discriminate(self):
        assets = {"a-1.0.0-t.tar.gz", "SHA256SUMS"}
        good = "`a-1.0.0-t.tar.gz` `SHA256SUMS`\n`sha256sum --check SHA256SUMS`\nLimitation: not published.\n"
        self.assertEqual([], release_section_gaps(good, assets))
        self.assertEqual(["section missing"], release_section_gaps(None, assets))
        for bad, label in (
            (good.replace("`a-1.0.0-t.tar.gz`", ""), "asset not documented: a-1.0.0-t.tar.gz"),
            (good + " `b-1.0.0-t.tar.gz`", "unknown asset cited: b-1.0.0-t.tar.gz"),
            (good.replace("sha256sum --check", "cat"), "integrity verification"),
            (good.replace("Limitation", "Note"), "no explicit limitation"),
        ):
            self.assertTrue(any(label in g for g in release_section_gaps(bad, assets)), label)
        ok = "La matriz publicada es Linux x86_64 GNU. Otra cosa."
        self.assertEqual([], published_matrix_gaps(ok, {"x86_64-unknown-linux-gnu"}, {"aarch64"}))
        self.assertTrue(published_matrix_gaps("La matriz publicada es x86_64/aarch64.", {"x86_64-x"}, {"aarch64"}))
        self.assertTrue(published_matrix_gaps("La matriz publicada es aarch64.", {"x86_64-x"}, set()))
        self.assertTrue(published_matrix_gaps("Sin frase.", {"x86_64-x"}, set()))

    def test_the_install_guide_walks_through_the_cli_workflows(self):
        closed = closed_flag_values()
        for key, values in closed.items():
            self.assertTrue(values, f"closed values of {key} not read: the check would be vacuous")
        body = guide_section(MCP_GUIDE.read_text(encoding="utf-8"), WORKFLOWS_SECTION)
        if body is not None:
            self.assertGreaterEqual(len(SHELL_BLOCK.findall(body)), 3, "too few workflow blocks")
        self.assertEqual([], cli_workflow_gaps(body, closed))

    def test_the_cli_workflow_checks_discriminate(self):
        closed = closed_flag_values()
        good = (
            "This section is written in English; the rest of this guide is in Spanish.\n"
            + " ".join(f"`{v}`" for values in closed.values() for v in sorted(values)) + "\n"
            "```bash\n"
            "cognicode index build . --strategy full\n"
            "cognicode graph full . --rebuild\n"
            "cognicode graph hot-paths . -n 5\n"
            "cognicode graph mermaid . --format svg\n"
            "```\n"
            "```bash\ncognicode refactor old new --operation rename\ncognicode doctor\n```\n"
            "Limitation: not run against a published release.\n"
        )
        self.assertEqual([], cli_workflow_gaps(good, closed))
        self.assertEqual(["section missing"], cli_workflow_gaps(None, closed))
        for bad, label in (
            (good.replace("written in English", "written"), "no language notice"),
            (good.replace("bash", "text"), "no shell block"),
            (good.replace("cognicode graph full . --rebuild\n", ""), "command not walked through: graph full"),
            (good.replace("cognicode doctor\n", ""), "fewer than two chained"),
            (good.replace("hot-paths", "hot-path"), "unknown subcommand: graph hot-path"),
            (good.replace("--strategy full", "--strategy turbo"), "value not accepted: index build --strategy turbo"),
            (good.replace("--operation rename", "--operation vanish"), "value not accepted: refactor --operation vanish"),
            (good.replace("-n 5", "--no-such-flag"), "cognicode --no-such-flag"),
            (good.replace("`png`", ""), "value not documented: graph mermaid --format png"),
            (good.replace("Limitation", "Note"), "no explicit limitation"),
        ):
            with self.subTest(label=label):
                self.assertTrue(any(label in g for g in cli_workflow_gaps(bad, closed)), (label, cli_workflow_gaps(bad, closed)))

    def test_binary_surface_check_discriminates(self):
        self.assertEqual([], binary_surface_gaps("`cogh install --ide x --profile y`\n`cognicode graph hot-paths -n 3`"))
        for bad, label in (
            ("`cogh install --ghost-flag`", "cogh --ghost-flag"),
            ("`cognicode-mcp --ghost`", "cognicode-mcp --ghost"),
            ("`cognicode-release skills --ghost`", "cognicode-release --ghost"),
            ("`cogh ghostcmd`", "cogh ghostcmd"),
            ("`cognicode-release ghostcmd`", "cognicode-release ghostcmd"),
            ("`cognicode ghostcmd`", "cognicode ghostcmd"),
        ):
            self.assertIn(label, binary_surface_gaps(bad))
    def test_closed_flag_values_cited_in_the_readmes_are_accepted(self):
        closed = closed_flag_values()
        for key, accepted in closed.items():
            self.assertTrue(accepted, f"{key}: no values read from the sources")
        for readme in (EN, ES):
            text = readme.read_text(encoding="utf-8")
            self.assertTrue(FLAG_VALUES.search(text), f"{readme.name}: no closed value cited")
            self.assertEqual([], flag_value_gaps(text, closed), readme.name)

    def test_mcp_tool_examples_match_the_published_input_schema(self):
        schemas = tool_schemas()
        for readme in (EN, ES):
            checked, gaps = tool_example_gaps(readme.read_text(encoding="utf-8"), schemas)
            self.assertGreater(checked, 0, f"{readme.name}: no tool example checked")
            self.assertEqual([], gaps, readme.name)

    def test_value_and_example_checks_discriminate(self):
        closed = closed_flag_values()
        self.assertEqual([], flag_value_gaps("`cognicode refactor s --operation rename|move`", closed))
        for bad, label in (
            ("`cognicode refactor s --operation rename|ghost`", "--operation ghost"),
            ("`cognicode index build --strategy full|ghost`", "--strategy ghost"),
            ("`cognicode graph mermaid --format svg|ghost`", "--format ghost"),
        ):
            self.assertTrue(any(label in g for g in flag_value_gaps(bad, closed)), label)
        schemas = {
            "t": {
                "properties": {"a": {"enum": ["x"]}, "b": {}},
                "required": ["a"],
            }
        }

        def example(args: str, tool: str = "t") -> str:
            return '```json\n{"tool": "%s", "arguments": %s}\n```' % (tool, args)

        self.assertEqual((1, []), tool_example_gaps(example('{"a": "x"}'), schemas))
        for args, tool, label in (
            ('{"a": "x", "ghost": 1}', "t", "argument 'ghost'"),
            ('{"b": 1}', "t", "required 'a' is missing"),
            ('{"a": "y"}', "t", "is not one of"),
            ('{"a": "x"}', "nope", "no such tool"),
        ):
            gaps = tool_example_gaps(example(args, tool), schemas)[1]
            self.assertTrue(any(label in g for g in gaps), (label, gaps))


    def test_both_readmes_state_the_security_boundaries_and_limitations(self):
        flags = {"--read-only"} & clap_flags([MCP_MAIN])
        env = set(re.findall(r'var\("(COGNICODE_\w+)"\)', MCP_MAIN.read_text(encoding="utf-8")))
        env.add(re.search(r'LINK_TRACE_VARIABLE: &str = "(\w+)"', LINK_TRACE_RS.read_text(encoding="utf-8")).group(1))
        self.assertTrue(flags and len(env) >= 2, "flags or environment not read: the check would be vacuous")
        for readme, heading in SECURITY_HEADINGS.items():
            with self.subTest(readme=readme.name):
                body = guide_section(readme.read_text(encoding="utf-8"), heading)
                self.assertEqual([], security_section_gaps(body, flags, env))

    def test_the_security_section_check_discriminates(self):
        flags, env = {"--read-only"}, {"COGNICODE_X"}
        good = "[SECURITY.md](SECURITY.md) `--read-only` `COGNICODE_X` stdout carries JSON-RPC, stderr the logs."
        self.assertEqual([], security_section_gaps(good, flags, env))
        self.assertEqual(["section missing"], security_section_gaps(None, flags, env))
        for bad, label in (
            (good.replace("SECURITY.md", "X"), "no link to SECURITY.md"),
            (good.replace("`--read-only`", ""), "flag not documented"),
            (good.replace("`COGNICODE_X`", ""), "environment variable not documented"),
            (good.replace("stderr", "log"), "no stdout/stderr rule"),
        ):
            self.assertTrue(any(label in g for g in security_section_gaps(bad, flags, env)), label)


    def test_the_environment_tables_list_every_variable_the_sources_read(self):
        main = MCP_MAIN.read_text(encoding="utf-8")
        link = re.search(r'LINK_TRACE_VARIABLE: &str = "(\w+)"', LINK_TRACE_RS.read_text(encoding="utf-8")).group(1)
        endpoint = re.search(r"Default endpoint: (http\S+?) ", main).group(1)
        telemetry = re.search(r'var\("(COGNICODE_TELEMETRY)"\)\.as_deref\(\) == Ok\("(\w+)"\)', main)
        otel = "OTEL_EXPORTER_OTLP_ENDPOINT"
        level = re.search(r'EnvFilter::new\("(\w+)"\)', main).group(1)
        self.assertTrue(telemetry and otel in main and level, "sources not read: the check would be vacuous")
        expected = {"RUST_LOG": level, otel: endpoint, telemetry.group(1): f"{telemetry.group(1)}={telemetry.group(2)}", link: None}
        for readme, heading in ENV_HEADINGS.items():
            with self.subTest(readme=readme.name):
                body = subsection(readme.read_text(encoding="utf-8"), heading)
                self.assertEqual([], env_table_gaps(body, expected))

    def test_the_environment_table_check_discriminates(self):
        table = "| `A` | `x` | d |\n| `OTEL_EXPORTER_OTLP_ENDPOINT` | `u` | needs COGNICODE_TELEMETRY |\n"
        expected = {"A": "x", "OTEL_EXPORTER_OTLP_ENDPOINT": "u"}
        self.assertEqual([], env_table_gaps(table, expected))
        self.assertEqual(["section missing"], env_table_gaps(None, expected))
        self.assertTrue(any("row missing" in g for g in env_table_gaps(table, {**expected, "B": None})))
        self.assertTrue(any("value missing" in g for g in env_table_gaps(table, {**expected, "A": "y"})))
        self.assertTrue(any("needs COGNICODE_TELEMETRY" in g for g in env_table_gaps(table.replace("needs COGNICODE_TELEMETRY", "x"), expected)))

    def test_both_readmes_troubleshoot_the_failures_the_sources_produce(self):
        facts = troubleshooting_facts()
        self.assertEqual({"1", "2"}, facts["exits"])
        self.assertTrue(facts["flags"] and facts["profiles"] and facts["doctor"], "facts not read")
        for readme, heading in TROUBLE_HEADINGS.items():
            with self.subTest(readme=readme.name):
                body = guide_section(readme.read_text(encoding="utf-8"), heading)
                self.assertEqual([], troubleshooting_gaps(body, EXIT_PHRASE[readme], facts))

    def test_doctor_invocations_cite_only_the_flags_the_doctor_command_declares(self):
        declared = doctor_flags(troubleshooting_facts()["doctor"])
        self.assertTrue({"--format", "--cwd"} <= declared, f"doctor flags not read: {declared}")
        checked = 0
        for doc in (EN, ES, MCP_GUIDE):
            for line in doc.read_text(encoding="utf-8").splitlines():
                if re.match(r"\s*cognicode doctor\b", line):
                    checked += 1
                    cited = set(re.findall(r"(?<![\w-])(--?[A-Za-z][\w-]*)", line))
                    self.assertEqual(set(), cited - declared - IMPLICIT_FLAGS, f"{doc.name}: {line}")
        self.assertGreater(checked, 0, "no doctor invocation found: the check would be vacuous")

    def test_the_doctor_flag_reader_discriminates(self):
        block = "#[arg(long, default_value = \"text\")]\n format: String,\n #[arg(short, long)]\n cwd: String,"
        self.assertEqual({"--format", "--cwd", "-c"}, doctor_flags(block))

    def test_the_troubleshooting_check_discriminates(self):
        facts = {"exits": {"2"}, "flags": {"--cwd"}, "profiles": {"full"}, "known": {"--cwd"}}
        good = "`--cwd` `full` exit status 2: run `cognicode doctor`; stdout is JSON-RPC, stderr the logs."
        self.assertEqual([], troubleshooting_gaps(good, "exit status {}", facts))
        self.assertEqual(["section missing"], troubleshooting_gaps(None, "exit status {}", facts))
        for bad, label in (
            (good.replace("exit status 2", "oops"), "exit code not stated"),
            (good.replace("`--cwd`", ""), "flag not documented"),
            (good.replace("`full`", ""), "profile not documented"),
            (good + " `--path`", "unknown flag cited"),
            (good.replace("cognicode doctor", "x"), "no cognicode doctor check"),
            (good.replace("stderr", "log"), "no stdout/stderr rule"),
        ):
            self.assertTrue(any(label in g for g in troubleshooting_gaps(bad, "exit status {}", facts)), label)

    def test_workspace_crate_tables_list_real_packages_and_the_sdk_libraries(self):
        packages = {p["name"] for p in workspace_metadata()["packages"]}
        libraries = sdk_library_crates()
        self.assertTrue(packages and libraries, "nothing to compare against: the check would be vacuous")
        for readme, heading in CRATE_HEADINGS.items():
            with self.subTest(readme=readme.name):
                text = readme.read_text(encoding="utf-8")
                self.assertEqual([], crate_table_gaps(guide_section(text, heading), packages, libraries, text))

    def test_the_crate_table_check_discriminates(self):
        body = "| Crate | D |\n|---|---|\n| `a` | x |\n| `b` | y |\n"
        text = "see " + SDK_GUIDE_LINK
        self.assertEqual([], crate_table_gaps(body, {"a", "b"}, {"a"}, text))
        self.assertEqual(["table missing or empty"], crate_table_gaps(None, {"a"}, {"a"}, text))
        self.assertEqual(["b: not a workspace package"], crate_table_gaps(body, {"a"}, {"a"}, text))
        self.assertEqual(["c: SDK library crate not listed"], crate_table_gaps(body, {"a", "b", "c"}, {"c"}, text))
        self.assertEqual(["no link to the SDK consumption guide"], crate_table_gaps(body, {"a", "b"}, {"a"}, "x"))


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Focused black-box tests for the generated MCP tool catalog."""

from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_tool_catalog.py"
SCHEMA = ROOT / "product/schemas/tools.v1.schema.json"
BASELINE = "ebdb62febc0b1a4853cdc6e172d0b6ae1fb49018"


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), *args],
        text=True,
        capture_output=True,
        check=False,
    )


def fixture() -> list[dict]:
    return [
        {
            "name": "zeta_tool",
            "description": "Zeta operation. More details.",
            "inputSchema": {"type": "object", "properties": {"value": {"type": "string"}}},
            "_meta": {
                "cognicode": {
                    "stability": "stable",
                    "category": "graph",
                    "authority": "read",
                    "requires_graph": True,
                    "requires_persistence": False,
                    "estimated_latency_ms": 120,
                }
            },
        },
        {
            "name": "alpha_tool",
            "description": "Alpha operation.",
            "inputSchema": {"type": "object"},
            "_meta": {
                "cognicode": {
                    "stability": "experimental",
                    "category": "aix",
                    "authority": "read",
                    "requires_graph": False,
                    "requires_persistence": False,
                    "estimated_latency_ms": 500,
                }
            },
        },
    ]


def test_generation_is_sorted_and_preserves_runtime_metadata() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        source.write_text(json.dumps(fixture()), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            BASELINE,
        )
        assert result.returncode == 0, result.stderr
        catalog = json.loads(output.read_text(encoding="utf-8"))
        assert catalog["schema_version"] == "cognicode.tools/v1"
        assert catalog["source_commit"] == BASELINE
        assert [tool["name"] for tool in catalog["tools"]] == ["alpha_tool", "zeta_tool"]
        alpha = catalog["tools"][0]
        assert alpha["stability"] == "experimental"
        assert alpha["public_profiles"] == ["experimental"]
        assert alpha["output_schema"] is None
        assert alpha["requirements"]["cache"] is None
        assert alpha["requirements_status"] == "not_declared"
        zeta = catalog["tools"][1]
        assert zeta["input_schema"] == fixture()[0]["inputSchema"]
        assert zeta["public_profiles"] == ["reviewer"]


def test_duplicate_names_are_rejected_without_output() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        duplicate = fixture() + [fixture()[0]]
        source.write_text(json.dumps(duplicate), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            BASELINE,
        )
        assert result.returncode != 0
        assert not output.exists()


def test_invalid_source_commit_is_rejected_without_output() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        source.write_text(json.dumps(fixture()), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            "not-a-sha",
        )
        assert result.returncode != 0
        assert not output.exists()


def test_committed_catalog_is_self_consistent_and_schema_valid() -> None:
    """The published catalog must be internally consistent, schema-valid,
    and describe exactly the surface the runtime exposes.

    MEDIDO 2026-10-15. Este test hacia tres preguntas y contestaba mal las
    tres:

    1. Regeneraba `tools.json` desde la captura commiteada y comparaba byte
       a byte. Eso solo puede pasar si `tools.json` es un artefacto generado,
       y no lo es: `interface/catalog.rs` lo declara fuente de verdad
       ("si se añade una tool, va a `tools.json` primero, luego el loader la
       expone"), y nueve campos —`limitations`, `prerequisites`,
       `follow_ups`, `required_args`, `intent_tags`, `language_scope`,
       `feature`, `impact_direction_floor`, `precision_floor`— no tienen
       origen en `_meta.cognicode`. Un generador que reconstruye el
       documento borra esos nueve y aun así el resultado valida.
    2. La captura commiteada tenía 73 tools de un commit de hace tres
       releases, y el literal `== 73` venía de ella. Ambos envejecidos por
       el mismo motivo y ninguno lo senalaba.
    3. La captura venía de un build por defecto, que oculta los tres tools
       con feature gate. Comparar eso contra el catálogo maestro mide que la
       captura esta mal, no que el catalogo este mal.

    Lo que si se puede afirmar, y es lo que importa, es que el runtime
    publica exactamente el conjunto que el documento declara. Eso se
    comprueba por nombre, que es lo que la arquitectura garantiza; el resto
    del documento es autoria humana y lo vigilan el schema y el conteo.
    """
    catalog = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    capture = json.loads(
        (ROOT / "product/tool-catalog-runtime.json").read_text(encoding="utf-8")
    )

    published = [tool["name"] for tool in catalog["tools"]]
    exposed = [tool["name"] for tool in capture]
    assert sorted(exposed) == sorted(published), (
        "el runtime expone un conjunto de tools distinto del que publica "
        f"product/tools.json. Solo en el runtime: "
        f"{sorted(set(exposed) - set(published))}. Solo en el catalogo: "
        f"{sorted(set(published) - set(exposed))}. Una tool que el documento "
        "publica y el runtime no expone es una promesa que el producto no "
        "cumple; una que el runtime expone y el documento no publica es una "
        "superficie sin contrato."
    )

    assert len(published) == len(set(published)), "duplicate tool names in the catalog"
    assert catalog["runtime_tool_count"] == len(catalog["tools"])

    try:
        import jsonschema
    except ImportError:
        return
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    jsonschema.validate(catalog, schema)


if __name__ == "__main__":
    tests = [
        test_generation_is_sorted_and_preserves_runtime_metadata,
        test_duplicate_names_are_rejected_without_output,
        test_invalid_source_commit_is_rejected_without_output,
        test_committed_catalog_is_self_consistent_and_schema_valid,
    ]
    for test in tests:
        test()
        print(f"PASS {test.__name__}")

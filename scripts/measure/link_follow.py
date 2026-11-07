#!/usr/bin/env python3
"""Cuenta que enlaces de `_links` siguio quien llamo despues.

Entrada: una traza JSONL, una llamada por linea, con `tool`, `arguments`,
`links` (los `_links` de la respuesta: `rel`, `tool`, `required_args`) y,
opcionalmente, `session`. Sin `session`, todas las lineas son una sesion.

Un enlace ofrecido cuenta como seguido si una llamada POSTERIOR de la misma
sesion usa su `tool` con todos sus `required_args`. Cada oferta se cuenta una
vez, aunque varias llamadas la cumplan.

Una tasa solo existe si hubo ofertas: un `rel` nunca ofrecido es «sin datos»,
no un cero. Y un cero solo es candidato a retirar con `MIN_OFFERS` ofertas o
mas; el informe no retira nada, solo lo senala.

Uso: link_follow.py <traza.jsonl>...   (informe JSON por stdout)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

# Los `rel` que el servidor documenta (`Link::rel` en interface/hateoas.rs).
KNOWN_RELS = (
    "resolve-ambiguity",
    "continue",
    "inspect-callers",
    "reindex",
    "inspect-risk",
    "preview-change",
    "retry",
)

# Ofertas minimas para que una tasa cero cuente como evidencia.
MIN_OFFERS = 20


def _follows(offer: dict[str, Any], later: dict[str, Any]) -> bool:
    if later.get("tool") != offer.get("tool"):
        return False
    arguments = later.get("arguments") or {}
    return all(
        key in arguments and arguments[key] == value
        for key, value in (offer.get("required_args") or {}).items()
    )


def follow_counts(sessions: list[list[dict[str, Any]]]) -> dict[str, dict[str, int]]:
    counts: dict[str, dict[str, int]] = {}
    for session in sessions:
        for index, call in enumerate(session):
            for offer in call.get("links") or []:
                entry = counts.setdefault(offer["rel"], {"offered": 0, "followed": 0})
                entry["offered"] += 1
                if any(_follows(offer, later) for later in session[index + 1 :]):
                    entry["followed"] += 1
    return counts


def report(counts: dict[str, dict[str, int]], min_offers: int = MIN_OFFERS) -> list[dict[str, Any]]:
    rows = []
    for rel in (*KNOWN_RELS, *sorted(set(counts) - set(KNOWN_RELS))):
        offered = counts.get(rel, {}).get("offered", 0)
        followed = counts.get(rel, {}).get("followed", 0)
        if offered == 0:
            rate, verdict = None, "sin datos"
        else:
            rate = followed / offered
            if followed > 0:
                verdict = "seguido"
            elif offered >= min_offers:
                verdict = "candidato a retirar"
            else:
                verdict = "pocas ofertas"
        rows.append(
            {"rel": rel, "offered": offered, "followed": followed, "rate": rate, "verdict": verdict}
        )
    return rows


def load_sessions(path: Path) -> list[list[dict[str, Any]]]:
    sessions: dict[str, list[dict[str, Any]]] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            call = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{path}:{number}: no es JSON ({exc.msg})") from exc
        if not isinstance(call, dict) or "tool" not in call:
            raise ValueError(f"{path}:{number}: falta `tool`")
        sessions.setdefault(str(call.get("session", "0")), []).append(call)
    return list(sessions.values())


def main(argv: list[str]) -> int:
    if not argv:
        print("uso: link_follow.py <traza.jsonl>...", file=sys.stderr)
        return 2
    sessions: list[list[dict[str, Any]]] = []
    try:
        for name in argv:
            sessions.extend(load_sessions(Path(name)))
    except (OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({"min_offers": MIN_OFFERS, "links": report(follow_counts(sessions))}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

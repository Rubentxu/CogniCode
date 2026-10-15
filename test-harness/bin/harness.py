#!/usr/bin/env python3
"""Punto de entrada del test harness.

Uso:
    python3 test-harness/bin/harness.py plan --lane change --base origin/main
    python3 test-harness/bin/harness.py run --lane change
    python3 test-harness/bin/harness.py status
    python3 test-harness/bin/harness.py resume --run-id <id>
    python3 test-harness/bin/harness.py explain --suite core-lib
    python3 test-harness/bin/harness.py certify --lane certification --candidate <sha>

Sin dependencias externas: solo la biblioteca estándar, para que funcione en el
mismo entorno que el gate sin instalar nada.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HARNESS_DIR / "src"))
sys.path.insert(0, str(HARNESS_DIR))

from harness.cli import main  # noqa: E402  (tras ajustar sys.path a propósito)

if __name__ == "__main__":
    os.environ.setdefault("PYTHONDONTWRITEBYTECODE", "1")
    raise SystemExit(main())

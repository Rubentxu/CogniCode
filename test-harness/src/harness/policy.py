"""Política de reutilización por lane, en un solo sitio.

TH-S05 dice que la reutilización oficial no existe hasta que una ADR acepte
paridad demostrada, y ADR-TH-003 lo repite: el harness **no** sustituye a
PipelineK ni cambia qué bloquea un merge. Traducido a este módulo:

  * `change`  → reutiliza: es la lane de feedback local, y ahí el recorte de
    tiempo es el valor entero.
  * `merge`   → ejecuta todo por defecto; reutilizar exige `--allow-reuse`
    explícito, para que la paridad del gate se demuestre antes de confiar en
    recibos. Mientras no haya paridad medida, el default es el gate actual.
  * `integration`, `certification`, `release-candidate`, `release` → nunca
    reutilizan: son las lanes donde una huella no puede sustituir a la
    ejecución (artefactos finales, sandbox, certificación).

`POLICY_VERSION` entra en toda huella y en todo veredicto: un cambio de política
invalida la evidencia anterior por construcción, que es más barato que
recordarlo.
"""

from __future__ import annotations

POLICY_VERSION = "th-policy/3"

# Lanes donde reutilizar está permitido por defecto.
REUSE_BY_DEFAULT = ("change",)

# Lanes donde reutilizar exige una bandera explícita del operador.
REUSE_WITH_FLAG = ("merge",)

# Lanes donde nunca se reutiliza.
FRESH_ALWAYS = ("integration", "certification", "release-candidate", "release")


def reuse_allowed(lane: str, allow_reuse_flag: bool = False) -> tuple[bool, str]:
    """¿Puede esta lane reutilizar un recibo válido? Devuelve (permitido, razón)."""
    if lane in REUSE_BY_DEFAULT:
        return True, f"lane '{lane}' reutiliza por política {POLICY_VERSION}"
    if lane in REUSE_WITH_FLAG:
        if allow_reuse_flag:
            return True, (
                f"lane '{lane}' reutiliza porque se pidió --allow-reuse; la paridad con el "
                "gate oficial todavía no está demostrada, así que esto es opt-in del operador"
            )
        return False, (
            f"lane '{lane}' ejecuta todo por defecto: la reutilización es opt-in "
            "(--allow-reuse) hasta que exista paridad demostrada con el gate oficial"
        )
    if lane in FRESH_ALWAYS:
        return False, f"lane '{lane}' exige ejecución fresca por política {POLICY_VERSION}"
    return False, f"lane desconocida '{lane}': se ejecuta todo (fail-closed)"


def certify_requires_fresh(lane: str) -> tuple[bool, str]:
    """`certify` no reutiliza en ninguna lane, ni siquiera `change`."""
    return (
        False,
        f"certify exige ejecución fresca en '{lane}': un recibo reutilizado no certifica",
    )

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

POLICY_VERSION = "th-policy/4"


class ProfileError(Exception):
    """Perfil de selección inválido, o no permitido en esa lane.

    Vive en este módulo, y no en el orquestador, para que la CLI pueda atraparlo
    sin importar una capa de arriba: la política es de aquí.
    """


# Perfiles de selección. El defecto es el estricto: el harness no recorta nada
# salvo que se pida un perfil explícito, y sólo donde puede permitírselo.
#
# MEDIDO 2026-10-16. Sin perfil, la lane `change` exige la clausura de
# dependientes del paquete tocado y las etapas de alcance workspace: es un
# mini-gate, 26 suites donde CR-08 pide una para el mismo cambio, y 337 s de
# pared en frío contra 29 s. Correcto para el gate y caro para el bucle local, así
# que el bucle no lo usaba. El perfil `dev` existe para eso y **no** recorta el
# suelo del oráculo: con `--oracle cr08` sigue exigiendo todo lo que CR-08 pida, y
# el suelo de obligaciones críticas se aplica igual.
GATE_PROFILE = "gate"
DEV_PROFILE = "dev"
PROFILES = (GATE_PROFILE, DEV_PROFILE)

# Lanes donde el perfil `dev` está permitido. Fuera de aquí se rechaza en vez de
# aplicarse: un gate que se recorta a sí mismo con una bandera es exactamente el
# modo de fallo que este paquete existe para no tener.
DEV_PROFILE_LANES = ("change",)


def resolve_profile(profile: str, lane: str) -> tuple[str, str]:
    """Valida el perfil para una lane. Devuelve (perfil efectivo, razón)."""
    if profile not in PROFILES:
        raise ProfileError(f"perfil desconocido: {profile!r} (esperado uno de {PROFILES})")
    if profile == DEV_PROFILE and lane not in DEV_PROFILE_LANES:
        raise ProfileError(
            f"el perfil '{DEV_PROFILE}' sólo se permite en las lanes {DEV_PROFILE_LANES}, "
            f"y se pidió en '{lane}': recortar la selección de un gate con una bandera "
            "convertiría el gate en un adorno"
        )
    if profile == DEV_PROFILE:
        return profile, (
            "perfil 'dev': se ejecuta lo que cambió y lo que el oráculo exija, sin la "
            "clausura de dependientes ni las etapas de alcance workspace (feedback local)"
        )
    return GATE_PROFILE, "perfil 'gate': selección completa por impacto del harness"

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

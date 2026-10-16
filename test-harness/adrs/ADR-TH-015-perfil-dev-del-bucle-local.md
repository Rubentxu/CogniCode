# ADR-TH-015 — Perfil `dev`: el harness puede sustituir a CR-08 en el bucle local

**Estado:** ACCEPTED. **Fecha:** 2026-10-16. **Autoridad del gate:** ninguna. El
perfil `dev` sólo existe en la lane `change` y el gate no puede usarlo.

## Contexto

ADR-TH-013 midió que el harness **no** podía sustituir al wrapper `fast` en el bucle
local: su lane `change` exigía la clausura de dependientes y las etapas de alcance
workspace (26 suites donde CR-08 pide una), y costaba 337 s en frío contra 29 s de
CR-08. La adopción quedó pendiente con el requisito escrito: *un perfil de desarrollo
para el bucle local*, y la decisión de adoptarlo, del operador.

Ese perfil es este ADR.

## Decisión

1. **Existe un perfil explícito.** `--profile gate` (defecto, sin cambios: el harness
   no recorta nada) y `--profile dev`, que sólo se admite en la lane `change`. Pedirlo
   en otra lane es un **error de uso** (`rc=2`, con su motivo): un gate que se recorta
   a sí mismo con una bandera es exactamente el modo de fallo que este paquete existe
   para no tener.

2. **El recorte es el mini-gate, nunca el suelo.** En `dev` no se añaden la clausura
   de dependientes del paquete tocado ni las etapas de alcance workspace
   (`cargo fmt --all`, `clippy --workspace`). Todo lo demás sigue: lo que cambió, los
   acoplamientos del oráculo, la documentación citada, los scripts, el suelo de
   obligaciones críticas y —lo importante— **el suelo del oráculo**: con
   `--oracle cr08`, un plan `dev` sigue exigiendo todo lo que CR-08 demande.

3. **El plan declara el perfil.** `profile` viaja en el documento del plan y en su
   esquema (`enum: gate|dev`, requerido), para que un consumidor no pueda confundir un
   plan recortado a propósito con el del gate. `POLICY_VERSION` sube a `th-policy/4`:
   un cambio de política invalida la evidencia anterior por construcción.

4. **Un scope demandado que el mapeo no conoce ya no desaparece.** Medido mientras se
   escribía el test de esta ADR: `compare()` sólo iteraba los scopes del mapeo, así que
   una demanda con un nombre desconocido no generaba fila, no entraba en `untracked` y
   el veredicto decía `AGREE`. Ahora cuenta como debilidad declarada (fila
   `NOT_IN_CATALOG`, `missing_scopes` y nota). En el repositorio real no cambia nada
   —CR-08 sólo emite scopes mapeados— y su valor aparece el día que emita uno nuevo.

5. **El wrapper usa el perfil, y sigue sin cambiar su defecto.** `test-fast.sh` pasa
   `--profile dev` cuando se le pide el motor harness; sin `TEST_FAST_ENGINE` sigue
   siendo el wrapper de CR-08.

## Medición (2026-10-16, clon, mismo cambio de una línea en `cognicode-ladybug`)

| Motor | Selección | Pared | Veredicto |
|---|---|---|---|
| CR-08, primera pasada (paga la compilación) | 1 suite | 73 s | rc=0 |
| CR-08, segunda pasada (caliente) | 1 suite | **28 s** | rc=0 |
| Harness perfil `dev`, ledger frío | **1 suite** (`cognicode-ladybug:lib#-`, 62 casos) | **29 s** | SATISFIED |
| Harness perfil `dev`, repetición sin cambios | 1 suite, **reutilizada** | **3 s** | SATISFIED |

Lo que dicen los números:

- **La selección coincide.** Con el perfil `dev`, el harness exige exactamente la misma
  suite que CR-08 para ese cambio, y el veredicto es `SATISFIED`.
- **La primera pasada cuesta lo mismo** (29 s contra 28 s en caliente): desaparece la
  regresión de 337 s que bloqueaba la adopción en ADR-TH-013.
- **La repetición es lo que gana**: 3 s contra 28 s, porque el recibo se reutiliza en
  vez de volver a lanzar Cargo. Es el caso real «no toqué nada, ¿sigue verde?».
- **No se pierde ninguna garantía**: el suelo del oráculo y el de obligaciones críticas
  siguen aplicándose, y el wrapper cae al motor CR-08 —diciéndolo— cuando el plan
  declara un scope que el harness no puede ejecutar.

## Consecuencias

- La decisión de adopción que quedaba pendiente tiene ahora datos y una bandera: poner
  `TEST_FAST_ENGINE=harness` como defecto es una línea, y el precio está medido. **No se
  hace aquí**: cambiar el defecto del bucle local es una decisión del operador (fila W6)
  y el ADR lo deja escrito con lo necesario para tomarla.
- El perfil `dev` no toca el gate: ninguna pipeline lo usa, el gate corre `plan` sin
  `--profile` y cualquier intento de usarlo fuera de `change` falla cerrado.
- La evidencia anterior del ledger del harness deja de reutilizarse por el cambio de
  `POLICY_VERSION`; es deliberado y está declarado.

## Lo que este ADR no decide

- Cambiar el defecto de `test-fast.sh` (operador, con esta medición).
- Si el harness sustituye a CR-08 en el *gate*: eso es W6 y exige selección por
  impacto con paridad demostrada, no este perfil.
- El orden de adopción por etapas ni el presupuesto de merge.

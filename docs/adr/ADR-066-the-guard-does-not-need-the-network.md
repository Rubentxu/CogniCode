# ADR-066 — La guarda del toolchain no pregunta a rustc: el host se resuelve una vez y se pasa

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: la guarda de toolchain cruzado que usa la lane de release
(`scripts/ci/check-cross-toolchain.sh`) y su contrato
(`scripts/ci/test_cross_toolchain_guard.py`), y la etapa `contracts` del
`merge-gate`, que es donde el defecto se manifestaba.

---

## Contexto

### Lo que se rompió, y cómo se sabe

El `merge-gate` de la unidad de la identidad del snapshot se puso en rojo en la
etapa `contracts/sh-0`, con `shell exited with code 1`. La etapa había tardado
**662,1 s**; en la corrida verde anterior, la misma etapa había tardado
**84,3 s**. Esa diferencia no la explicaba el código del árbol, que entre una
corrida y otra solo había cambiado un párrafo del Journal.

La etapa `contracts` no guarda su salida, así que el fallo se reprodujo
localmente con su propio comando, `bash scripts/ci/run-all-contracts.sh`:

```
subprocess.TimeoutExpired: Command '[.../check-cross-toolchain.sh',
'aarch64-unknown-linux-gnu']' timed out after 120 seconds
```

Tres tests de `test_cross_toolchain_guard.py` fallaban, y los tres fallaban en
el mismo sitio: `subprocess.run(..., timeout=120)`.

### La causa, medida

La guarda `check-cross-toolchain.sh` resolvia el triple del host con
`rustc -vV`. El `rustc` del `PATH` es el shim de `rustup`, y el shim busca la
toolchain en `$RUSTUP_HOME`, que por defecto es `$HOME/.rustup`.

El contrato que ejercita la guarda **aisla el entorno**: pone `HOME` y `TMPDIR`
en un directorio temporal, para que la guarda no herede nada de quien la
ejecuta. Con ese `HOME` aislado, el shim no encuentra ninguna toolchain
instalada; lee el `rust-toolchain.toml` del repositorio, que fija `channel =
"1.96.0"`, e intenta **instalarla desde la red**:

```
info: syncing channel updates for stable-x86_64-unknown-linux-gnu
info: downloading 6 components
warn: the missing active toolchain `stable-x86_64-unknown-linux-gnu` has been
      auto-installed
```

Medido en esta maquina:

| Medida | Valor |
|---|---|
| `rustc -vV` con `HOME` aislado y toolchain declarada | 40,3 s (descarga) |
| `rustc -vV` con `RUSTUP_HOME` real | 9 ms |
| La guarda completa con `HOME` aislado | 117,7 s |
| El `timeout` del contrato | 120 s |
| La etapa `contracts` del gate en rojo | 662,1 s |

La guarda pagaba esa descarga en **cada** llamada, y el contrato la llama siete
veces. Con la red un poco mas lenta, una sola llamada superaba los 120 s y el
`TimeoutExpired` tumbaba la etapa entera.

La linea temporal encaja: los binarios `release` de la maquina se construyeron
a las 10:07 y las 10:18, y dos corridas del gate de esa mañana fallaron **en la
misma etapa**, con la misma firma. El defecto no era de la unidad de la
identidad del snapshot: era del gate, y llevaba todo el dia ahi.

### El segundo defecto, del mismo origen

Cuando `rustc -vV` fallaba, el host quedaba **vacio**, y el target nativo
—el que se estaba comprobando— se rechazaba como si fuera cruzado, con el
motivo de que faltaba el compilador de C. El veredicto hablaba de una causa
que no se habia mirado, y senalaba un compilador que nunca se pregunto.

---

## Decision

### El host se resuelve una vez, fuera, y se pasa

La guarda acepta el triple del host como segundo argumento:

```
check-cross-toolchain.sh <target-triple> [host-triple]
```

Cuando se le da, la guarda **no ejecuta `rustc` en absoluto**. Cuando no se le
da, lo resuelve con `rustc -vV` como antes, y **falla cerrado** si no puede:
el motivo nombra que no se pudo resolver el host y explica que un veredicto
sobre el target seria falso.

El contrato resuelve el host una sola vez, en el proceso padre y con el
entorno real —9 ms— y se lo pasa a cada llamada. La hermeticidad es
**estructural**, no de disciplina: `run_guard` **rechaza** una llamada sin host
salvo que se pida explicitamente `resolve_host=True`, que es la opcion que
ejercita la ruta de resolucion y que un `rustc` de mentira responde.

### Lo que no se toca

La stage `toolchain-for-$target` de `release-candidate.pipeline.kts` sigue
llamando a la guarda con un solo argumento, asi que sigue resolviendo el host
ella misma. Unico cambio: cuando no puede resolverlo, ahora la guarda falla
cerrado en vez de rechazar el target nativo como si fuera cruzado. La lane de
release gana el arreglo sin tocar el pipeline.

---

## Alternativas consideradas

### (a) Propagar `RUSTUP_HOME` al entorno aislado

Descartada. Arregla el sintoma —la descarga— sin tocar la causa: la guarda
seguiria preguntando a rustc, y la respuesta seguiria dependiendo de como
este configurada la maquina que ejecuta el gate. El defecto de fondo —una
pregunta que puede tardar dos minutos y whose answer is not about the target—
permanece. Ademas, `RUSTUP_HOME` no esta definido en el entorno: su valor por
defecto es `$HOME/.rustup`, que es justamente lo que el aislamiento destruye,
asi que el arreglo tendria que adivinar una ruta.

### (b) Subir el `timeout` del contrato

Descartada por la misma lente de triaje: subir un timeout sin hipotesis medible
alarga el sintoma. El `timeout` no era el defecto; era la linea que el defecto
cruzaba. Con 120 s el contrato caia en 3 de 11 tests; con 300 s pasaria, y la
etapa tardaria 30 minutos en depender de la latencia de una descarga que no
aporta nada a lo que se comprueba.

### (c) Aislar la red en la etapa `contracts`

Descartada. El problema no es que haya red: es que el gate no deberia depender
de ella para decidir si un toolchain responde. Cerrar la red convierte un
defecto en un fallo mas ruido, no lo arregla, y rompe cualquier otro contrato
que si la necesite.

### (d) Quitar la comprobacion de la guarda

Descartada de plano. La guarda existe porque un target instalado que no
compila codigo nativo produce un candidato que parece construido y no lo esta.
Eso se midio en la lane de v0.101.4, que paso la comprobacion de rustup y
cayo veinte minutos despues en `binaries-aarch64` con tres errores distintos.

### (e) Preguntar al compilador y deducir el host de su tripletodo

Descartada por invencion: el compilador cruzado no dice que host es, y
adivinarlo seria el mismo defecto que se quiere cerrar.

---

## Consecuencias

### Positivas

- La etapa `contracts` deja de depender de la red: el contrato que la hace caer
  tarda 0,64 s en vez de 117,7 s por llamada.
- La guarda no puede dar un veredicto falso: sin host, falla diciendo que no
  pudo resolverlo, en vez de rechazar el target nativo como si fuera cruzado.
- La lane de release hereda el arreglo sin tocar el pipeline.
- El contrato tiene tres comprobaciones nuevas: que la guarda no pregunte a
  rustc cuando el host se le dio, que se niegue a dar veredicto cuando no
  puede resolver el host, y que la ruta de resolucion siga viva.

### Negativas

- La guarda tiene un argumento mas. Los llamantes que existian siguen
  funcionando: el segundo es opcional.
- La resolucion del host sigue siendo la de `rustc -vV` cuando no se le pasa.
  El cambio no la hace mejor; hace que el contrato no dependa de ella, y que
  su fallo se declare.

### Mitigaciones

- Las tres comprobaciones nuevas se vieron fallar antes del arreglo: dos
  fallaban con el veredicto falso de la guarda ("no C compiler for
  x86_64-unknown-linux-gnu", siendo el target nativo) y la tercera pasaba,
  porque protege una ruta que ya existia.
- La ruta de resolucion del host dentro de la guarda sigue ejercitada a
  proposito, con un `rustc` de mentira, para que el arreglo no la deje sin
  cobertura.

---

## Referencias

- Contrato de la unidad: `scripts/ci/test_cross_toolchain_guard.py`.
- Guarda: `scripts/ci/check-cross-toolchain.sh`.
- Lane que la usa: `release-candidate.pipeline.kts`, stage
  `toolchain-for-$target`.
- ADR-057 — Una etapa que no puede hacer caer la puerta dice que observa: el
  mismo criterio, aplicado a la etapa `contracts` del gate, que se ponia roja
  por una descarga y no lo decia.
- Unit Candidate C48 en `goal-compass.json`.

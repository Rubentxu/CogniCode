# Production-Ready Foundation (PRF) — CERTIFICATION

## Modelo de certificación de requisitos

PRF adopta el ciclo **`SPECIFIED → IMPLEMENTED → INTEGRATED → ACCEPTED → RELEASED`**.
Cada requisito recorre estos 5 estados; cada transición requiere evidencia
registrada en `evidence/CERTIFICATES.md`.

### Estado 1: SPECIFIED

**Definición**: el requisito está documentado en una especificación
ejecutable (`docs/specs/*` o equivalente) con al menos un escenario
Given/When/Then.

**Evidencia requerida**:

- Identificador (REQ-XXX-NNN).
- Escenario Given/When/Then.
- Enlace a la especificación.

### Estado 2: IMPLEMENTED

**Definición**: existe código fuente que cubre el requisito y supera las
pruebas unitarias.

**Evidencia requerida**:

- Commit hash del código que implementa el requisito.
- Tests unitarios que pasan (referencia + comando).
- Sin regresiones en el resto del crate.

### Estado 3: INTEGRATED

**Definición**: el código se ejecuta en el binario real (`cognicode`,
`cognicode-mcp`, `cogh`, …) y la salida observable coincide con el
contrato.

**Evidencia requerida**:

- Comando ejecutado con el binario real.
- Output capturado (stdout, stderr, exit code).
- Comparación contra el contrato esperado.

### Estado 4: ACCEPTED

**Definición**: la prueba UAT del requisito se ha ejecutado con el
binario real y ha pasado según los criterios de aceptación documentados.

**Evidencia requerida**:

- Procedimiento UAT ejecutado (ver `UAT.md`).
- Resultado observado registrado.
- Aprobación registrada en `evidence/CERTIFICATES.md`.

### Estado 5: RELEASED

**Definición**: el requisito forma parte de un release taggeado y
distribuido.

**Evidencia requerida**:

- Tag del release (`vX.Y.Z`).
- Tag-signed o annotate message verificable.
- Cambio de estado registrado en `evidence/CERTIFICATES.md`.

## Cómo se promueven los estados

Cada cambio de estado se registra en `evidence/CERTIFICATES.md` con:

- REQ-ID
- Estado anterior → estado nuevo
- Commit hash (si aplica)
- Evidencia (enlace a test, output, tag)
- Fecha y actor

Las promociones **no son retroactivas**. No se baja un estado para
esconder un problema; se documenta y se corrige la causa raíz.

## Anti-patrones prohibidos

- Marcar `ACCEPTED` un requisito sin UAT ejecutada.
- Rebajar umbrales para conseguir verde.
- Eliminar pruebas que fallan.
- Reclasificar errores como warnings.
- Publicar un release sin certificaciones completas.

## Auditoría

El estado de las certificaciones es público en `evidence/CERTIFICATES.md`.
Cualquier auditor puede verificarlo cruzando commits, tests, outputs y
tags.

# PRF-STATE — Persistencia, aislamiento y recuperación

- **PRF-STATE-01 MUST:** catalogar cada dato como canónico, derivado/reconstruible o transitorio; cada uno tiene ownership, ubicación y ciclo de vida. No mezclar `CallGraph` histórico con FactStore canónico sin contrato y equivalencia.
- **PRF-STATE-02 MUST:** namespace por `canonical_root + analysis_config_digest`; cada resultado durable conserva snapshot/revisión y fuente manifest; distintos workspaces con símbolos homónimos no contaminan respuestas ni índices.
- **PRF-STATE-03 MUST:** dos procesos sobre mismo HOME y dos proyectos no sobreescriben ni pierden datos; las escrituras de un mismo workspace usan locking/atomicidad y resultados stale quedan identificados.
- **PRF-STATE-04 MUST:** interrupción durante análisis, commit de store o migración no produce evidencia parcial marcada como válida; recuperación automática documentada o error y rollback con estado previo intacto.
- **PRF-STATE-05 MUST:** versión de esquema documentada; update conserva datos y permite rollback/downgrade verificado o rechazo explícito sin corrupción ni pérdida.
- **PRF-STATE-06 MUST:** `cogh uninstall` no elimina datos de usuario por defecto ni rutas ajenas; ownership y separación binario/config/cache/estado se prueban en HOME limpio y HOME existente.
- **PRF-STATE-07 SHOULD:** datos derivados se reconstruyen al no ser compatibles; el usuario recibe aviso de reconstrucción y nunca una revisión antigua presentada como actual.

**Certificación:** C4; U09,U17,U18,U21–U24. No atribuir persistencia a un módulo únicamente por tener tests in-memory.

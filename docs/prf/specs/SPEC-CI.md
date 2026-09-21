# PRF-CI — Gate reproducible por SHA

- **PRF-CI-01 MUST:** cada SHA candidato tiene un recibo independiente con `fmt`, compilación, lint según baseline vigente, tests core, smoke CLI/MCP y contrato de release. `FAIL` aborta merge/publicación; sin skips ocultos, `continue-on-error` o `|| true` sobre gates obligatorios.
- **PRF-CI-02 MUST:** campañas full/nightly ejecutan matriz de features, pruebas de contrato con cliente real, corpus por lenguaje/capability, adversariales y benchmarks; fallos conocidos se comparan con baseline exacto por entorno, no se convierten en GREEN de producto.
- **PRF-CI-03 MUST:** para cada modificación de comportamiento se incorpora test caracterizador y regresión. Cobertura de líneas/ramas se mide con herramienta y baseline; la cobertura de MCP tool y conformance OpenSpec se reporta por separado.
- **PRF-CI-04 MUST:** presupuesto de rendimiento por perfil y corpus fijado **antes de la comparación**, p50/p95, RSS, tiempos frío/caliente, N repeticiones y SHA input/hardware; no aceptar SLO vacío.
- **PRF-CI-05 MUST:** advisories/licencias/SBOM/sha256/provenance de bundle y smoke nativo de cada plataforma anunciada son verificables desde release real; no utilizar un artefacto distinto al aprobado.
- **PRF-CI-06 MUST:** documentar decisión sobre política local-first y su equivalencia a un gate obligatorio antes de activación remota. El workflow actual manual no se cita como protección automática de PR.
- **PRF-CI-07 MUST:** el pipeline detecta artificialmente al menos un test rojo, manifiesto incorrecto y fallo de publicación, negando PASS/merge/release ante cada caso.

**Certificación:** C0/C6/C7; U03,U12,U19,U27. Evidencias: SHA commit y artefactos, suite result, exit code, runner, fecha, baseline y aprobación.

# UAT / Release Admission Checklist

## UAT-01 — Clean clone certification
- [ ] crear directorio temporal vacío;
- [ ] clonar repo;
- [ ] checkout del SHA candidato C8-R;
- [ ] `git status --porcelain` vacío;
- [ ] todos los bins declarados por Cargo existen y están trackeados;
- [ ] todos los docs referenciados por ROADMAP existen y están trackeados;
- [ ] full workspace suite verde;
- [ ] clippy `-D warnings` verde;
- [ ] builds release de `cognicode`, `cognicode-mcp`, `cognicode-control-plane` verdes.

## UAT-02 — Control Plane
- [ ] arrancar binario real;
- [ ] `GET /control-plane/workspaces/<id>/architecture` devuelve 200;
- [ ] `status == evaluated`;
- [ ] IDs de constraints = set canonical esperado;
- [ ] `unevaluated_constraints` vacío;
- [ ] self-host violations coherentes con el baseline declarado;
- [ ] ruta fuera de scope devuelve 404.

## UAT-03 — e91
- [ ] fixture multi-repo materializado de forma determinista;
- [ ] profiling antes del cambio archivado;
- [ ] test semántico antes/después mantiene resultados acordados;
- [ ] perf regression test verde;
- [ ] scorecard G5 verde;
- [ ] no se ha ocultado el problema mediante aumento arbitrario de budget.

## UAT-04 — Architecture boundaries
- [ ] rule `application_no_infrastructure` ejecutable;
- [ ] rule `application_no_interface` ejecutable;
- [ ] violaciones existentes inventariadas;
- [ ] cada excepción temporal tiene owner, rationale y expiry;
- [ ] un import plantado rompe el gate.

## UAT-05 — Supply chain
- [ ] release Actions pinneadas por SHA;
- [ ] `cargo deny check advisories` verde;
- [ ] `RUSTSEC-2024-0437` no figura como ignore al cerrar CR-07;
- [ ] SBOM generado para cada bin publicado;
- [ ] tag/workspace SemVer coherente.

## UAT-06 — CI selection
- [ ] modificación en `api.rs` selecciona CP integration tests;
- [ ] modificación en architecture selecciona self-host + CP tests;
- [ ] modificación en Cargo selecciona advisories/build matrix adecuada;
- [ ] path desconocido activa fallback seguro;
- [ ] release paths nunca dependen solo de selección quirúrgica.

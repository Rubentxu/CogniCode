# cognicode-axiom — Roadmap v2: Quality Engine Production-Ready

> **Versión**: 2.0  
> **Fecha**: Mayo 2026  
> **Estado**: Active  
> **Repositorio**: [Rubentxu/CogniCode](https://github.com/Rubentxu/CogniCode)

---

## 1. Visión

**cognicode-axiom** es un motor de análisis de calidad de código nativo en Rust que compite con SonarQube. Sin JVM, sin PostgreSQL, sin dependencias externas pesadas. Todo en Rust, con tree-sitter, inventory auto-registro, y reglas type-safe.

**Objetivo**: Alcanzar **80%+ de cobertura** del catálogo de SonarQube en **5 lenguajes** (Rust, JavaScript/TypeScript, Java, Python, Go), con **testing basado en fixtures reales** validados mediante `cognicode-rule-test-harness`.

---

## 2. Estado Actual (Mayo 2026)

### 2.1 Reglas por lenguaje

| Lenguaje | Implementadas | SonarQube aplicables | % | Prioridad completar |
|----------|:-------------:|:--------------------:|:--:|:-------------------:|
| **Rust** | 152 | ~200 equivalentes | **76%** | 🔴 P1 — casi completo |
| **JavaScript/TS** | 212 | ~300 | **71%** | 🟡 P3 |
| **Java** | 228 | ~600 (core: ~300) | **38%** (core: 76%) | 🟡 P2 |
| **Python** | 0* | ~170 | **0%*** | 🔴 P1 — gap más grande |
| **Go** | 0* | ~50 | **0%*** | 🟢 P4 — catálogo pequeño |
| **Universal (*)** | 10 | — | — | ✅ Completo |

*\*Python y Go tienen cobertura parcial via reglas universales (`language: "*"`)*

### 2.2 Infraestructura de testing

| Componente | Tests | Estado |
|-----------|:-----:|--------|
| Unit tests (catálogo) | 232 | ✅ |
| Edge cases | 30 | ✅ |
| Fixtures Rust | 30 (15×2 casos) | ✅ |
| Fixtures JS | 30 (15×2 casos) | ✅ |
| Fixtures Java | 30 (15×2 casos) | ✅ |
| Fixtures Python | 30 (15×2 casos) | ⚠️ Fixtures creados, reglas pendientes |
| Fixtures Go | 30 (15×2 casos) | ⚠️ Fixtures creados, reglas pendientes |
| System tests | 55 | ✅ |
| MCP integration | 36 | ✅ |
| **TOTAL** | **357** | ✅ 0 fallos |

### 2.3 Sistema

| Componente | Estado |
|-----------|--------|
| `declare_rule!` macro | ✅ (3 bugs fixeados, 97% adopción) |
| `RuleRegistry` + inventory | ✅ |
| `RuleContext` + CallGraph helpers | ✅ |
| `ParseCache` | ✅ |
| Quality Gates (YAML) | ✅ |
| SQALE Debt + Ratings A-E | ✅ |
| BLAKE3 Duplications | ✅ |
| `cognicode-quality` MCP server | ✅ (18 tools funcionales) |
| `cognicode-rule-test-harness` | ✅ |
| SonarQube API Scraper (Rust) | ✅ (feature-gated) |
| YAML Config system | ✅ |
| CI script (`scripts/test-all.sh`) | ✅ |

---

## 3. Gaps por lenguaje — Plan de implementación

### 3.1 🐍 Python (gap: +136 reglas)

**Estado actual**: 0 reglas dedicadas. Solo 10 reglas universales (*) aplican.

**Plan**: 3 batches × 45 reglas = 135 reglas → 80% de ~170

| Batch | Categoría | Reglas | Fixtures |
|-------|-----------|:------:|:--------:|
| **PY-1** | Security + Bugs (30 security, 15 bugs) | 45 | 30 |
| **PY-2** | Code Smells (30) + Error Handling (15) | 45 | 30 |
| **PY-3** | Performance + Testing + Naming | 45 | 30 |
| **TOTAL** | | **135** | **90** |

**Reglas clave Python** (SonarQube `sonar-python`):

**Security (30)**:
- PY_S2068 — Hardcoded credentials
- PY_S5332 — Clear-text HTTP
- PY_S2077 — SQL injection (f-strings)
- PY_S1523 — `eval()` / `exec()` usage
- PY_S4830 — SSL verification disabled
- PY_S4423 — Weak TLS
- PY_S4784 — ReDoS (regex injection)
- PY_S5247 — XSS in templates
- PY_S5542 — Weak crypto (MD5, SHA1)
- PY_S5547 — Weak cipher (DES, RC4)
- PY_S3649 — SQL via string concat
- PY_S2612 — Weak file permissions (os.chmod 777)
- PY_S2095 — Resource leak (file not closed)
- PY_S5693 — File upload without size limit
- PY_S3330 — Cookie without HttpOnly
- PY_S2092 — Cookie without Secure
- PY_S4502 — CSRF disabled
- PY_S5725 — CSP missing
- PY_S5734 — HSTS missing
- PY_S5736 — X-Content-Type-Options missing
- PY_S1313 — Hardcoded IP
- PY_S3358 — Nested ternary
- PY_S5042 — Zip bomb (tarfile extraction)
- PY_S2755 — XXE (lxml without secure parser)
- PY_S4829 — `print()` in prod
- PY_S1148 — `traceback.print_exc()` instead of logging
- PY_S1165 — Exception swallowed without log
- PY_S1163 — Catch-all `except Exception: pass`
- PY_S112 — Generic exception raised
- PY_S2221 — Catching BaseException

**Bugs (15)**:
- PY_S2259 — None dereference (AttributeError)
- PY_S1244 — Float equality
- PY_S1751 — Loop with single iteration
- PY_S1845 — Dead store (assigned but not read)
- PY_S1854 — Unused import
- PY_S1481 — Unused variable
- PY_S1226 — Parameter reassigned
- PY_S1656 — Self-assignment
- PY_S1764 — Identical operands
- PY_S2589 — Always-true condition
- PY_S2757 — = vs == in condition
- PY_S1994 — Loop counter modified inside
- PY_S1860 — Deadlock (threading.Lock nesting)
- PY_S2201 — Return value ignored
- PY_S2178 — `is` instead of `==` with literals

**Code Smells (30)**:
- PY_S138 — Long function (>50 lines)
- PY_S134 — Deep nesting (>4 levels)
- PY_S107 — Too many parameters (>7)
- PY_S1541 — Too many branches (>10)
- PY_S3776 — Cognitive complexity (>15)
- PY_S1066 — Collapsible if
- PY_S1192 — String literal duplicates
- PY_S1135 — TODO/FIXME tags
- PY_S1134 — Deprecated API usage
- PY_S1142 — Too many returns (>5)
- PY_S1186 — Empty function
- PY_S1871 — Duplicate branches
- PY_S122 — Source file too long (>1000 lines)
- PY_S104 — Module too long
- PY_S1479 — Too many methods in class (>20)
- PY_S1820 — Too many fields in class (>15)
- PY_S154 — High cyclomatic complexity
- PY_S1700 — Mutable default argument (list/dict as default)
- PY_S172 — `print()` in library code
- PY_S100 — Function naming (`snake_case`)
- PY_S101 — Class naming (`PascalCase`)
- PY_S115 — Constant naming (`UPPER_CASE`)
- PY_S117 — Variable naming (`snake_case`)
- PY_S125 — Commented-out code
- PY_S148 — Low comment ratio
- PY_S160 — Function too complex
- PY_S1643 — String concatenation in loop (use `join()`)
- PY_S170 — Unused import
- PY_S173 — Missing type hints on public functions
- PY_S2111 — f-string without interpolation

**Error Handling (15)**:
- PY_S108 — Empty except block
- PY_S1121 — Raise generic Exception
- PY_S1130 — Raise in finally
- PY_S1141 — Nested try-except (>2)
- PY_S1160 — Public function raises generic exception
- PY_S1162 — Exception class naming
- PY_S1164 — Catch-all except
- PY_S2737 — except with pass
- PY_S2225 — Exception message not informative
- PY_S2226 — Logging exception without traceback
- PY_S2227 — Raising Exception without message
- PY_S2228 — Raising string (Python 2 style)
- PY_S2701 — assert with literal (assert True)
- PY_S3415 — assert arg order (assertEqual(actual, expected))
- PY_S1122 — Fallthrough in except

**Performance + Testing (30)**:
- PY_P1 — `range(len(x))` instead of `enumerate`
- PY_P2 — `keys()` iteration instead of direct dict iteration
- PY_P3 — `map/filter` with lambda instead of comprehension
- PY_P4 — `list.append` in loop instead of comprehension
- PY_P5 — `+` string concat in loop instead of `join`
- PY_P6 — `time.sleep()` in test
- PY_P7 — `global` keyword abuse
- PY_P8 — `del` on list element (O(n))
- PY_P9 — `in` on list instead of set
- PY_P10 — Class-level attribute instead of instance
- PY_T1 — Test without assertion
- PY_T2 — Test with `time.sleep()`
- PY_T3 — `assertEqual` vs `assertTrue`
- PY_T4 — `setUp`/`tearDown` vs `setUpClass`/`tearDownClass`
- PY_T5 — `unittest.skip` without reason
- PY_T6 — Test method not starting with `test_`
- PY_T7 — Test fixture too complex (>20 lines setup)
- PY_T8 — Multiple asserts in one test
- PY_T9 — Duplicated test method
- PY_T10 — Test using `random` (non-deterministic)

---

### 3.2 ☕ Java (gap: +52 reglas core → 80%)

**Estado actual**: 228 reglas. Core Java al 76%. Faltan ~52 para 80%.

**Plan**: 2 batches × 26 reglas = 52 reglas

| Batch | Categoría | Reglas | Fixtures |
|-------|-----------|:------:|:--------:|
| **JAVA-1** | Streams + Lambdas + Optional (26) | 26 | 20 |
| **JAVA-2** | Spring Boot essentials (26) | 26 | 20 |
| **TOTAL** | | **52** | **40** |

**Reglas clave faltantes**:

**Streams & Lambdas (20)**:
- JAVA_L16 — Stream.distinct().sorted() → order matters
- JAVA_L17 — Stream.limit() without sorted()
- JAVA_L18 — Stream.findFirst().isPresent() → findFirst().ifPresent()
- JAVA_L19 — .collect(Collectors.toList()).stream() → redundant
- JAVA_L20 — Stream.flatMap(Collection::stream) can be simplified
- JAVA_L21 — Stream.map(x -> x) → redundant identity
- JAVA_L22 — Optional.flatMap(Function.identity()) → Optional.map()
- JAVA_L23 — IntStream.boxed().collect() → box before collect
- JAVA_L24 — Stream.allMatch on empty stream → always true
- JAVA_L25 — Stream.noneMatch on empty stream → always true

**Spring Boot (20)**:
- JAVA_SP1 — @Autowired field injection → constructor injection
- JAVA_SP2 — @Component without interface
- JAVA_SP3 — @Service with state (not thread-safe)
- JAVA_SP4 — @RestController without @ResponseBody
- JAVA_SP5 — @Transactional on private method
- JAVA_SP6 — @Async without thread pool config
- JAVA_SP7 — @Value injection for complex config
- JAVA_SP8 — @Scheduled without fixed delay
- JAVA_SP9 — JpaRepository method naming convention
- JAVA_SP10 — @Entity without @Id

**More Code Smells (12)**:
- JAVA_S218 — Switch with too few cases (<3 → use if)
- JAVA_S219 — For loop with `i` variable use outside
- JAVA_S220 — Private method only called from inner class
- JAVA_S221 — Method returns null (should return Optional)
- JAVA_S222 — Method parameter count mismatch with constructor
- JAVA_S223 — `instanceof` check without cast
- JAVA_S224 — `boolean` parameter in public method (flag argument)
- JAVA_S225 — Public method with >10 parameters
- JAVA_S226 — Loop with `size()` in condition (cache it)
- JAVA_S227 — `for(;;)` instead of `while(true)`
- JAVA_S228 — Thread created but not started
- JAVA_S229 — `finalize()` overridden in non-final class

---

### 3.3 🟡 JavaScript/TypeScript (gap: +28 reglas → 80%)

**Estado actual**: 212 reglas. 71%. Faltan ~28 para 80%.

**Plan**: 1 batch × 28 reglas

| Batch | Categoría | Reglas | Fixtures |
|-------|-----------|:------:|:--------:|
| **JS-1** | React avanado + Testing + TS avanzado | 28 | 20 |

**Reglas clave faltantes**:

**React avanzado (10)**:
- JS_RX41 — Context.Provider without value
- JS_RX42 — useEffect with missing return type for cleanup
- JS_RX43 — useCallback with empty deps (should be ref)
- JS_RX44 — useState initializer function call (useState(fn()) should be useState(fn))
- JS_RX45 — Component with both state and derived values
- JS_RX46 — useEffect with setState without deps (infinite loop)
- JS_RX47 — useRef not used in JSX or effect
- JS_RX48 — useImperativeHandle without display name
- JS_RX49 — lazy() without Suspense wrapper
- JS_RX50 — createContext with undefined default

**Testing avanzado (10)**:
- JS_TEST11 — Test without describe block
- JS_TEST12 — expect.assertions() count mismatch
- JS_TEST13 — beforeAll/afterAll in nested describe
- JS_TEST14 — mockImplementation vs mockReturnValue
- JS_TEST15 — spyOn with original implementation not restored
- JS_TEST16 — act() wrapping missing in async test
- JS_TEST17 — waitFor timeout too short
- JS_TEST18 — fireEvent vs userEvent (prefer userEvent)
- JS_TEST19 — toBeTruthy vs toBe(true) ambiguity
- JS_TEST20 — toEqual vs toStrictEqual (missing undefined fields)

**TypeScript avanzado (8)**:
- TS_ADV1 — `enum` with numeric values → prefer `const enum` or union
- TS_ADV2 — `as` cast without validation
- TS_ADV3 — `any` in generic position
- TS_ADV4 — `NonNullable<T>` vs `T & {}`
- TS_ADV5 — `ReturnType<typeof fn>` misuse
- TS_ADV6 — `Omit<T, K>` when `Pick<T, K>` would work
- TS_ADV7 — `Record<string, T>` instead of indexed type
- TS_ADV8 — `Exclude<T, U>` vs `Extract<T, U>` confusion

---

### 3.4 🔵 Rust (gap: +8 reglas → 80%)

**Estado actual**: 152 reglas. 76%. Faltan ~8 para 80%.

| ID | Nombre | Categoría |
|----|--------|-----------|
| R021 | `Arc<Mutex<T>>` when `Rc<RefCell<T>>` suffices (single-thread) | Performance |
| R022 | `Box<dyn Error>` instead of concrete error type | CodeSmell |
| R023 | `#[derive(Debug)]` on struct with sensitive fields | Security |
| R024 | `impl Drop` without `#[may_dangle]` in generic context | Bug |
| R025 | `mem::forget` used incorrectly (leaking resources) | Bug |
| R026 | `PhantomData` used as field instead of `PhantomData<fn() -> T>` | CodeSmell |
| R027 | `std::mem::transmute` without safety comment | Bug |
| R028 | `Pin` without `Unpin` trait bound | Bug |

---

### 3.5 🔵 Go (gap: +40 reglas → 80%)

**Estado actual**: 0 reglas dedicadas. Solo universales (*).

**Plan**: 1 batch × 40 reglas

| ID | Nombre | Categoría |
|----|--------|-----------|
| GO_S100 — Function naming (`camelCase`) | CodeSmell |
| GO_S101 — Type naming (`PascalCase`) | CodeSmell |
| GO_S107 — Too many params (>6) | CodeSmell |
| GO_S134 — Deep nesting (>3) | CodeSmell |
| GO_S138 — Long function (>60 lines) | CodeSmell |
| GO_S3776 — High complexity | CodeSmell |
| GO_S2068 — Hardcoded secrets | Vulnerability |
| GO_S2077 — SQL injection via `fmt.Sprintf` | Vulnerability |
| GO_S1523 — `os/exec.Command` with user input | Vulnerability |
| GO_S2612 — `os.Chmod(0777)` | Vulnerability |
| GO_S1148 — `panic()` in library code | Bug |
| GO_S108 — Empty error handling (`if err != nil { }`) | Bug |
| GO_S185 — Dead store (assigned but not read) | Bug |
| GO_S1481 — Unused variable (assigned but not read) | Bug |
| GO_S1845 — Variable assigned but never used | Bug |
| GO_S1656 — Self-assignment | Bug |
| GO_S1764 — Identical operands | Bug |
| GO_S2757 — `=` vs `==` in condition | Bug |
| GO_S1244 — Float equality | Bug |
| GO_S2201 — Return value ignored | Bug |
| GO_S2221 — `log.Fatal` in library code | Bug |
| GO_S131 — Switch without default | CodeSmell |
| GO_S1135 — TODO/FIXME | CodeSmell |
| GO_S125 — Commented-out code | CodeSmell |
| GO_S1186 — Empty function | CodeSmell |
| GO_S1871 — Duplicate branches | CodeSmell |
| GO_S122 — File too long | CodeSmell |
| GO_S148 — Low comment ratio | CodeSmell |
| GO_S1700 — String concat in loop → `strings.Builder` | Performance |
| GO_S1736 — `for i := 0; i < len(x); i++` → `for i, v := range x` | Performance |
| GO_S1943 — `append` in loop without pre-allocation | Performance |
| GO_S2111 — `fmt.Sprintf("%s", x)` → `x` | Performance |
| GO_S115 — Constant naming | CodeSmell |
| GO_S117 — Variable naming | CodeSmell |
| GO_S170 — Unused import | CodeSmell |
| GO_S173 — Missing doc comment on exported function | CodeSmell |
| GO_S2095 — `defer file.Close()` missing after open | Bug |
| GO_S1860 — Mutex lock ordering (potential deadlock) | Bug |
| GO_S2259 — Nil pointer dereference | Bug |
| GO_S1160 — Error returned but not checked | Bug |

---

## 4. Plan de ejecución — Fases

### Fase 7: Python Rules (semanas 1-2)

| Batch | Contenido | Reglas | Fixtures | Tests |
|-------|-----------|:------:|:--------:|:-----:|
| PY-1 | Security + Bugs | +45 | +30 | +60 |
| PY-2 | Code Smells + Error Handling | +45 | +30 | +60 |
| PY-3 | Performance + Testing + Naming | +45 | +30 | +60 |
| **Total PY** | | **+135** | **+90** | **+180** |

**Hito**: Python al 80% (135 reglas). Tests: 357 → ~540.

---

### Fase 8: Java Core + Go (semana 3)

| Batch | Contenido | Reglas | Fixtures | Tests |
|-------|-----------|:------:|:--------:|:-----:|
| JAVA-1 | Streams + Lambdas | +26 | +20 | +40 |
| JAVA-2 | Spring Boot essentials | +26 | +20 | +40 |
| GO-1 | 40 reglas Go | +40 | +30 | +60 |
| **Total** | | **+92** | **+70** | **+140** |

**Hito**: Java core al 80% (280 reglas). Go al 80% (40 reglas). Tests: ~540 → ~680.

---

### Fase 9: JS/TS + Rust (semana 4)

| Batch | Contenido | Reglas | Fixtures | Tests |
|-------|-----------|:------:|:--------:|:-----:|
| JS-1 | React + Testing + TS avanzado | +28 | +20 | +40 |
| RUST-1 | 8 reglas Rust avanzadas | +8 | +8 | +16 |
| **Total** | | **+36** | **+28** | **+56** |

**Hito**: JS al 80% (240 reglas). Rust al 80% (160 reglas). Tests: ~680 → ~740.

---

## 5. Métricas objetivo final

| Métrica | Ahora | Objetivo |
|---------|:-----:|:--------:|
| **Reglas totales** | 592 | **855** |
| **Lenguajes al 80%** | 3 (Rust, JS, Java core) | **5** (+Python, +Go) |
| **Fixtures** | 120 | **308** |
| **Tests totales** | 357 | **740+** |
| **MCP tools** | 18 | 18 |
| **Warnings** | 1 | 0 |
| **Cobertura fixture/regla** | 20% | 36% |

---

## 6. Metodología de testing

Cada nueva regla sigue este flujo:

```
1. Implementar regla
   └── declare_rule! { id: "PY_SXXXX", ..., check: => { ... } }

2. Crear fixture
   └── sandbox/fixtures/rules/python/PY_SXXXX/
       ├── requirements.txt
       ├── expected.json
       └── src/
           ├── smelly.py    ← código que DEBE disparar la regla
           └── clean.py     ← código que NO debe disparar la regla

3. Agregar al test de integración
   └── tests/rule_fixtures.rs → test_python_rule_fixtures()

4. Ejecutar y validar
   └── cargo test -p cognicode-axiom --test rule_fixtures

5. Agregar test unitario opcional
   └── tests/catalog_tests.rs → #[test] fn test_py_sxxxx_works()
```

### Criterios de calidad por regla:
- ✅ **True positive**: detecta el smell en smelly.py
- ✅ **True negative**: NO detecta en clean.py
- ✅ **No panic**: maneja entradas inválidas sin crash
- ✅ **Severidad correcta**: coincide con SonarQube
- ✅ **Mensaje descriptivo**: incluye `rule_id`, línea, sugerencia

---

## 7. Timeline estimado

| Semana | Fase | Entregable |
|--------|------|-----------|
| **1** | PY-1 | 45 reglas Python (security + bugs) |
| **2** | PY-2, PY-3 | 90 reglas Python (code smells + performance) |
| **3** | JAVA-1, JAVA-2, GO-1 | 52 Java + 40 Go |
| **4** | JS-1, RUST-1 | 28 JS + 8 Rust |
| **TOTAL** | 4 semanas | +263 reglas, +188 fixtures, +383 tests |

---

## 8. Riesgos y mitigaciones

| Riesgo | Impacto | Mitigación |
|--------|:-------:|-----------|
| Reglas Python difíciles (tree-sitter Python queries complejas) | Alto | Priorizar reglas regex/line-based; 80% son simples |
| Fatiga de batches (muchas reglas similares) | Medio | Automatizar generación con `generate_rust_stubs()` |
| Fixtures que no compilan (Python sintaxis inválida) | Bajo | Usar código Python real y válido |
| Regresión en tests existentes | Alto | CI script `test-all.sh` antes de cada commit |

---

## 9. Referencias

- [SonarQube Rules](https://rules.sonarsource.com/)
- [sonar-python GitHub](https://github.com/SonarSource/sonar-python)
- [sonar-java GitHub](https://github.com/SonarSource/sonar-java)
- [sonar-javascript GitHub](https://github.com/SonarSource/sonar-javascript)
- [sonar-go GitHub](https://github.com/SonarSource/sonar-go)
- [tree-sitter](https://tree-sitter.github.io/)

---

*Roadmap v2.0 — Mayo 2026. Mantenido por CogniCode Team.*

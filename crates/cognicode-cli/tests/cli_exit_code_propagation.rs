//! PRF-CLI-01 (extensión): todo brazo de `CommandExecutor::execute` que
//! recibe un `Err` debe propagarlo, no imprimirlo y continuar.
//!
//! ## El defecto que este contrato mide
//!
//! `CommandExecutor::execute` despacha sobre `CliCommand`. Tres brazos
//! propagan el error con `return Err(e)` — `Analyze`, `Graph`,
//! `FindUsages` — y ocho lo tragan: imprimen con `eprintln!` y dejan que
//! `execute` termine en `Ok(())`. El proceso sale con **0**, así que un
//! script o un agente lee "terminó bien" de una operación que no se
//! hizo. El caso más caro no es imaginario: `cognicode navigate
//! references <symbol>` imprimía
//!
//!     Navigate command failed: Invalid position 'MySymbol': expected file:line:column
//!
//! y salía con 0. Un agente que seguía la skill `cognicode-pr-review`
//! creía haber consultado las referencias del símbolo y no había
//! consultado ninguna.
//!
//! `FindUsages` documenta la política en su propio comentario ("exit 0:
//! éxito; exit 2 (via Err): uso inválido o error de backend"). Este
//! contrato la convierte de prosa en aserción para el resto de la
//! superficie.
//!
//! ## Por qué el argv es lo que es
//!
//! `prf_cli_01_uat::graph_full_nonexistent_path_does_not_exit_zero` pasa
//! `--path /nonexistent/...`, y `graph full` no tiene `--path`: su
//! firma real es `graph full [PATH]`. Clap rechaza el flag con exit 2
//! y el test pasa **sin ejecutar el brazo**. Su comentario dice "already
//! the case; pins the contract", y lo que realmente mide es que clap
//! conoce la aridad. Aquí se usa el positional, que sí llega al brazo.
//! ## Los dos sentidos
//!
//! Un contrato que solo afirma "esto sale distinto de 0" pasa entero si
//! el arreglo convierte *todo* en error, incluido el éxito. Por eso cada
//! caso de error tiene su gemelo de éxito: el mismo comando con
//! entrada válida, que debe seguir saliendo con 0. Si un arreglo rompe
//! el caso feliz, cae esta suite, no la próxima.

use std::path::PathBuf;
use std::process::{Command, Output};

mod common;

fn cognicode_bin() -> PathBuf {
    common::binary_path("cognicode")
}

fn run(args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Un directorio inexistente, con una ruta que ningún sandbox puede tener.
const MISSING: &str = "/nonexistent/prf_cli_01_exit_propagation/missing";

// ---------------------------------------------------------------------------
// 1. Navigate: el caso que la skill `cognicode-pr-review` enseñaba mal.
// ---------------------------------------------------------------------------

/// `navigate` recibe un `position` que no es `file:line:column`.
/// `parse_position` falla, el error se imprime y el proceso sale con 0.
#[test]
fn navigate_references_propagates_invalid_position() {
    let out = run(&["navigate", "references", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate references <symbol>` debe salir distinto de 0: la posición \
         es inválida y la consulta NO se hizo. Salir con 0 hace creer al \
         agente que consultó las referencias. stderr: {}",
        stderr_of(&out)
    );
}

/// El mismo defecto por el brazo `Definition`, que comparte `parse_position`.
#[test]
fn navigate_definition_propagates_invalid_position() {
    let out = run(&["navigate", "definition", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate definition <symbol>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

/// `Hover` es el tercer brazo de `Navigate` y el mismo `parse_position`.
#[test]
fn navigate_hover_propagates_invalid_position() {
    let out = run(&["navigate", "hover", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate hover <symbol>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 2. Index: `build_index` sobre un directorio inexistente.
// ---------------------------------------------------------------------------

#[test]
fn index_build_propagates_missing_directory() {
    let out = run(&["index", "build", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`index build <inexistente>` debe salir distinto de 0: el índice no \
         se construyó. stderr: {}",
        stderr_of(&out)
    );
}

#[test]
fn index_query_propagates_missing_directory() {
    let out = run(&["index", "query", "ZzzNope", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`index query <símbolo> <inexistente>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 3. Graph: el caso que el UAT existente no llegaba a ejecutar.
// ---------------------------------------------------------------------------

/// `graph full` con la ruta como **positional**, que es la firma real.
/// Un grafo sobre un directorio inexistente no es un grafo: sale con 0 y
/// un `Warning: PARTIAL` que ningún script lee.
#[test]
fn graph_full_propagates_missing_directory() {
    let out = run(&["graph", "full", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`graph full <inexistente>` debe salir distinto de 0. Un `PARTIAL` \
         que se emite con exit 0 es indistinguible del éxito para un \
         pipeline. stderr: {}",
        stderr_of(&out)
    );
}

/// `graph mermaid` sobre la misma ruta: mismo PARTIAL, mismo brazo.
#[test]
fn graph_mermaid_propagates_missing_directory() {
    let out = run(&["graph", "mermaid", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`graph mermaid <inexistente>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 4. El gemelo de éxito: la mitad que impide "arreglar" exiting con 0.
//
// ---------------------------------------------------------------------------

/// `analyze` sobre un directorio válido y vacío es una operación
/// realizada: debe seguir saliendo con 0. Si un arreglo de propagación
/// lo rompe, esta aserción cae.
#[test]
fn analyze_valid_empty_dir_still_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`analyze <dir válido>` debe salir con 0. stderr: {}",
        stderr_of(&out)
    );
}

/// El caso feliz del mismo brazo que los tests de error atacan: una
/// posición bien formada llega al backend y sale con 0 aunque no
/// encuentre nada. "Sin resultados" es un resultado, no un error.
///
/// La firma real es `navigate definition <POSITION> [PATH]`: el workspace
/// es un **positional**, no `--path`. Usar `--path` haría que clap
/// rechazara el comando con exit 2 y el test pasaría sin ejecutar el brazo
/// — que es exactamente el defecto que este fichero documenta arriba
/// encuentra en `prf_cli_01_uat`.
#[test]
fn navigate_with_wellformed_position_does_not_fail_on_missing_definition() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("lib.rs");
    std::fs::write(&file, "pub fn alpha() -> u32 { 1 }\n").expect("write fixture");

    // `lib.rs:1:8` es una posición bien formada: el parseo pasa y el
    // fallo, si lo hay, viene del backend LSP, no de la invocación.
    let position = format!("{}:1:8", file.display());
    let out = Command::new(cognicode_bin())
        .args([
            "navigate",
            "definition",
            &position,
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn cognicode binary");

    let err = stderr_of(&out);
    assert!(
        !err.contains("Invalid position"),
        "una posición bien formada no debe fallar en `parse_position`: {err}"
    );
}

// ---------------------------------------------------------------------------
// 5. La auditoría, como contrato y no como prosa.
//
// N+82 midió los 8 brazos que tragan el `Err` y los arregló. La pregunta que
// queda no es "los arreglé todos" sino "queda alguno igual", y una lista
// escrita a mano responde a la primera pregunta solo hasta el día que alguien
// añade un brazo nuevo. Estas aserciones releen el `match` de
// `CommandExecutor::execute` y clasifican cada brazo, de modo que un brazo
// nuevo sin `return Err` es un RED y no una línea más de la enumeración.
// ---------------------------------------------------------------------------

/// El cuerpo de `CommandExecutor::execute`, entre el `match &cli.command` y
/// el `Ok(())` que lo cierra.
fn dispatch_source() -> String {
    let path = common::repo_root().join("crates/cognicode-core/src/interface/cli/commands.rs");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read commands.rs: {e}"));

    let start = text
        .find("match &cli.command {")
        .expect("CommandExecutor::execute no hace match sobre &cli.command");
    let tail = &text[start..];
    let end = tail
        .rfind("\n        Ok(())")
        .expect("no se encuentra el Ok(()) final de execute");
    tail[..end].to_string()
}

/// El nombre de cada brazo `Some(CliCommand::X ...)` del dispatch, en orden.
fn dispatch_arm_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("Some(CliCommand::") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

/// El índice de la línea de cabecera de un brazo, o `None` si no está.
///
/// Se aceptan las dos formas de un variant de clap: con campos
/// (`Some(CliCommand::Index { command })`) y de tupla
/// (`Some(CliCommand::Evidence(cmd))`). Buscar solo la primera deja fuera el
/// brazo `Evidence` sin avisar, que es como un extractor se convierte en una
/// lista que miente.
fn arm_header_line(lines: &[&str], name: &str) -> Option<usize> {
    lines.iter().position(|l| {
        let Some(rest) = l.trim().strip_prefix("Some(CliCommand::") else {
            return false;
        };
        let head: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        head == name
    })
}

/// El cuerpo de un brazo: desde su línea de cabecera hasta la línea que cierra
/// el brazo, por conteo de llaves.
fn arm_body(source: &str, name: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = arm_header_line(&lines, name)
        .unwrap_or_else(|| panic!("no se encuentra el brazo CliCommand::{name} en el dispatch"));

    let mut depth = 0i32;
    let mut started = false;
    let mut out = String::new();
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        out.push_str(line);
        out.push('\n');
        if started && depth == 0 {
            return out;
        }
    }
    panic!("el brazo CliCommand::{name} no cierra");
}

/// Brazos que se declaran exentos, cada uno con su motivo, porque el
/// predicado "contiene `return Err`" no puede ser cierto para ellos.
///
/// Una exención sin motivo escrito es exactamente el hueco que este contrato
/// cierra, así que la lista es corta a propósito: añadir una entrada es una
/// decisión que hay que defender en la revisión.
const EXEMPT: &[(&str, &str)] = &[(
    "Serve",
    "no puede fallar: imprime por stderr que el servidor MCP va aparte, y \
     descarta el puerto con `let _ = port`",
)];

/// Brazos que el build por defecto no compila, porque viven detrás de un
/// `#[cfg(feature = …)]` que `cognicode-cli` no declara.
///
/// No son exentos por merced: el `return Err` se les puso en N+82, y lo que
/// no se puede comprobar desde el build por defecto es **su existencia**, que
/// es justo lo que este contrato documenta. Se listan con su feature para que
/// un día que la feature se declare, alguien sepa que hay dos brazos que
/// aparecen de golpe y hay que revisarlos.
const FEATURE_GATED: &[(&str, &str)] = &[
    ("DocsIngest", "multimodal"),
    ("IssuesIngest", "multimodal"),
    ("Evidence", "evidence-cli-ladybug"),
];

#[test]
fn every_dispatch_arm_is_accounted_for() {
    let arms = dispatch_arm_names(&dispatch_source());

    // La enumeración viene del código, no de una lista mantenida a mano: si
    // mañana hay 13 brazos, este test ve 13. Los tres detrás de `#[cfg]` no
    // aparecen en el build por defecto, así que se cuentan aparte.
    let gated: Vec<&str> = FEATURE_GATED.iter().map(|(n, _)| *n).collect();
    let compiled = arms.iter().filter(|a| !gated.contains(&a.as_str())).count();

    assert_eq!(
        compiled, 9,
        "se esperaban 9 brazos CliCommand compilados en el build por defecto; \
         se encontraron {arms:?}. Si has añadido uno, clasifícalo aquí o exímelo \
         con un motivo."
    );
    assert!(
        !arms.is_empty() && arms.iter().all(|n| !n.is_empty()),
        "no se pudo extraer ningún nombre de brazo: el formato del match cambió"
    );
}

/// Cada brazo `#[cfg]` que este contrato da por ausente tiene que estar
/// detrás de un `#[cfg]` de verdad, y su `return Err` tiene que estar escrito.
///
/// Es la versión estática de lo que el build por defecto no puede ver: si
/// alguien borrase el `return Err` de un brazo que solo compila con una
/// feature, aquí se nota, porque se lee el fichero entero.
#[test]
fn feature_gated_arms_still_propagate_in_source() {
    let path = common::repo_root().join("crates/cognicode-core/src/interface/cli/commands.rs");
    let text = std::fs::read_to_string(&path).expect("read commands.rs");
    let dispatch = dispatch_source();

    for (name, feature) in FEATURE_GATED {
        // El brazo existe en el fichero, aunque el build no lo compile.
        let whole = arm_body_anywhere(&text, name)
            .unwrap_or_else(|| panic!("CliCommand::{name} no aparece en commands.rs"));
        assert!(
            arm_propagates(&whole),
            "CliCommand::{name} (feature {feature}) traga el error en el código fuente: \
             aunque hoy no compile, sería el mismo fallo el día que la feature se declare."
        );
    }

    // Y el dispatch los menciona, aunque el cfg los Quite del build.
    for (name, _) in FEATURE_GATED {
        assert!(
            dispatch.contains(name),
            "CliCommand::{name} figura en FEATURE_GATED pero ya no aparece en el \
             dispatch: la lista describe un brazo que se movió o se eliminó."
        );
    }
}

/// Como `arm_body`, pero busca en el fichero entero en vez de en el tramo del
/// dispatch, para poder leer brazos que un `#[cfg]` deja fuera del build.
fn arm_body_anywhere(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = arm_header_line(&lines, name)?;

    let mut depth = 0i32;
    let mut started = false;
    let mut out = String::new();
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        out.push_str(line);
        out.push('\n');
        if started && depth == 0 {
            return Some(out);
        }
    }
    None
}

#[test]
fn every_dispatch_arm_propagates_or_is_exempt_with_a_reason() {
    let source = dispatch_source();
    for name in dispatch_arm_names(&source) {
        if EXEMPT.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let body = arm_body(&source, &name);
        assert!(
            arm_propagates(&body),
            "el brazo CliCommand::{name} no propaga el error: captura el Err, lo \
             imprime y continúa, así que el proceso sale con 0 de una operación que \
             no se hizo. Escribe `Self::execute_{name}(..).await?`, o devuelve el \
             error, o decláralo exento con un motivo real."
        );
    }
}

/// Si un brazo traga el error.
///
/// La forma buena es `?` (`Self::execute_index(command).await?`), que deja que
/// `main` termine el proceso. La forma mala es `if let Err(e) = … { eprintln!(..) }`
/// **sin** devolver: el error se imprime, se olvida, y `execute` acaba en
/// `Ok(())`.
///
/// El predicado mira el `if let Err` y exige que su cuerpo mencione el error.
/// No comprueba sintaxis concreta —`return Err(e)`, `Err(e)`, `e?`— porque la
/// propiedad que importa es "el error capturado no se pierde", y enumerar las
/// formas válidas sería una lista mantenida a mano.
fn arm_propagates(body: &str) -> bool {
    // Sin captura de error: o hay `?`, o no hay nada que propagar.
    if !body.contains("if let Err") {
        return true;
    }
    // Con captura: cada bloque `if let Err` tiene que reemitir el error.
    for block in if_let_err_blocks(body) {
        if !block.contains("Err") {
            return false;
        }
    }
    true
}

/// Los cuerpos de todos los `if let Err(...) = … { … }` de un fragmento.
fn if_let_err_blocks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0usize;
    while let Some(pos) = body[i..].find("if let Err") {
        let at = i + pos;
        // Buscar la llave que abre el cuerpo del `if`.
        let Some(open_rel) = body[at..].find('{') else {
            break;
        };
        let open = at + open_rel;
        let mut depth = 0i32;
        let mut j = open;
        while j < chars.len() {
            match chars[j] {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(chars[open..=j].iter().collect());
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if j >= chars.len() {
            break;
        }
        i = j + 1;
    }
    out
}

/// El otro sentido de la auditoría: un brazo puede quedar exento por
/// sorpresa. Si un brazo pasa a estar exento, tiene que estar en la lista.
#[test]
fn no_arm_is_exempt_that_does_not_exist() {
    let arms = dispatch_arm_names(&dispatch_source());
    for (name, _) in EXEMPT {
        assert!(
            arms.iter().any(|a| a == name),
            "{name} está en EXEMPT pero no es un brazo del dispatch. Una exención \
             de un brazo que ya no existe esconde el motivo de por qué se añadió."
        );
    }
}

/// El enum y el dispatch no pueden separarse: un `CliCommand` sin brazo
/// aceptaría argumentos y no ejecutaría nada, saliendo con 0.
///
/// Es la misma clase de defecto que el resto de este fichero, un paso más
/// arriba: no es que el brazo trague el error, es que **no hay brazo**.
#[test]
fn every_cli_command_variant_has_a_dispatch_arm() {
    let path = common::repo_root().join("crates/cognicode-core/src/interface/cli/commands.rs");
    let text = std::fs::read_to_string(&path).expect("read commands.rs");

    let enum_start = text
        .find("pub enum CliCommand {")
        .expect("no se encuentra el enum CliCommand");
    let enum_text = &text[enum_start..];

    // El enum se recorta en **su** llave de cierre. Sin este corte, el
    // recorrido sigue hasta el final del fichero y recoge las variantes de
    // `EvidenceCommand` (`List`, `Search`), que no son subcomandos de
    // `cognicode`: son subcomandos de `cognicode evidence`, y un día de
    // estos dos se separan en ficheros distintos. Es el mismo error que
    // cometió N+81 al medir por prosa lo que el código ya decidía.
    let mut depth = 0i32;
    let mut enum_end = enum_text.len();
    for (i, line) in enum_text.lines().enumerate() {
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        enum_end = enum_text
                            .lines()
                            .take(i + 1)
                            .map(|l| l.len() + 1)
                            .sum::<usize>();
                    }
                }
                _ => {}
            }
        }
        if depth == 0 && i > 0 {
            break;
        }
    }
    let enum_text = &enum_text[..enum_end];

    // Un subcomando de clap es una variante del enum. Se identifican por su
    // **nombre de variante a profundidad de llaves 1**, no por "la línea que
    // sigue a un `#[arg]`": dentro de una variante, `#[arg(long)]` precede a
    // cada *campo*, y caminar por atributos encuentra nombres de campo
    // (`path`, `recursive`) que no son subcomandos. La profundidad separa las
    // dos cosas sin depender del formato del atributo.
    let mut variants: Vec<String> = Vec::new();
    let mut depth = 0i32;
    for line in enum_text.lines() {
        let before = depth;
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        // `Foo {` abre la variante: antes de la llave había profundidad 1.
        let opens_variant = before == 1 && depth == 2 && line.trim_end().ends_with('{');
        if opens_variant {
            let t = line.trim();
            let name: String = t
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                variants.push(name);
            }
        }
    }

    assert!(
        variants.len() >= 10,
        "solo se extrajeron {} variantes de CliCommand: el extractor del enum se \
         ha quedado atrás de su formato. Variantes: {variants:?}",
        variants.len()
    );

    let arms = dispatch_arm_names(&dispatch_source());
    let gated: Vec<&str> = FEATURE_GATED.iter().map(|(n, _)| *n).collect();
    for v in &variants {
        assert!(
            arms.iter().any(|a| a == v) || gated.contains(&v.as_str()),
            "CliCommand::{v} acepta argv de clap pero no tiene brazo en el dispatch: \
             el comando se acepta, no hace nada, y sale con 0."
        );
    }
}

/// Un `index build` sobre un directorio real y vacío sí es una operación
/// realizada: el gemelo positivo del `index_build_propagates_missing_directory`.
#[test]
fn index_build_on_real_empty_dir_still_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["index", "build", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`index build <dir válido>` debe salir con 0. stderr: {}",
        stderr_of(&out)
    );
}

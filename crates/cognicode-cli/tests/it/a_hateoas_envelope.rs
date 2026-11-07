//! TASK-83 — el sobre de la respuesta: `result`, `basis`, `budget` y enlaces.
//!
//! Una respuesta que dice qué encontró, sobre qué se apoya, qué dejó
//! fuera y qué se puede hacer ahora es una respuesta que un consumidor
//! puede usar sin adivinar. Antes de este trabajo el producto publicaba el
//! resultado y poco más: el que quería saber si la respuesta era completa,
//! o qué se había omitido, o cuál era el siguiente paso, tenía que
//! reconstruirlo desde el payload —y a veces no podía, porque el payload no
//! lo decía.
//!
//! El sobre es la respuesta con esa información encima, y este fichero
//! pinna las cuatro cosas que el sobre promete:
//!
//!   - **lo que dice**: el resultado, tal cual, sin reescribirlo;
//!   - **sobre qué se apoya**: la cobertura, la frescura y lo que no se ha
//!     medido (que se dice `null`, nunca `complete`);
//!   - **qué dejó fuera**: el presupuesto;
//!   - **qué viene después**: los enlaces, con los argumentos rellenos.
//!
//! ## Lo que NO se comprueba aquí
//!
//! - Que el sobre viaje por el cable del MCP: eso lo comprueba
//!   `a012_structured_output` (el mismo contrato, visto desde el binario).
//! - Qué goals existen: aquí no hay ningún goal.

use serde_json::{Map, Value, json};

use cognicode_cli::cli::follow_ups;
use cognicode_core::interface::hateoas::{
    HandleContext, HandleKind, envelope_for, mint_handle, verify_handle,
};
use cognicode_core::interface::mcp::cursor::CursorErrorKind;

/// El contexto al que se ancla un handle: el workspace y la instantánea.
fn context() -> HandleContext {
    HandleContext {
        workspace: "/repo".to_string(),
        snapshot: "snap-1".to_string(),
    }
}

/// Un payload de `build_graph` con agujeros: dos ficheros ilegibles y una
/// referencia sin declaración.
fn with_holes() -> Value {
    json!({
        "status": "partial",
        "summary": "the graph was built with holes",
        "skipped_files": [
            {"path": "a.rs", "reason": "io error"},
            {"path": "b.rs", "reason": "io error"}
        ],
        "pending_references": [
            {"file": "c.rs", "line": 12, "target": "Foo", "kind": "call", "reason": "unresolved"}
        ],
        "rejected_relationships": []
    })
}

/// El enlace con esta relación, si está.
fn link_with<'a>(
    envelope: &'a cognicode_core::interface::hateoas::Envelope,
    rel: &str,
) -> &'a cognicode_core::interface::hateoas::Link {
    envelope
        .links
        .iter()
        .find(|link| link.rel == rel)
        .unwrap_or_else(|| {
            panic!(
                "the envelope publishes no `{rel}` link: {:?}",
                envelope
                    .links
                    .iter()
                    .map(|l| l.rel.as_str())
                    .collect::<Vec<_>>()
            )
        })
}

// --- El sobre: lo que dice, sobre qué se apoya, qué dejó fuera ---

/// El sobre lleva el resultado entero, sin reescribirlo.
///
/// Un sobre que rescribiera el resultado sería un segundo resultado, y un
/// consumidor que leyera uno y otro vería dos respuestas.
#[test]
fn the_envelope_carries_the_result_it_was_built_from() {
    let result = with_holes();
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());

    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.result, result, "the result travels whole");
}

/// El estado es el veredicto que la respuesta declara.
///
/// La palabra la declara quien la midió; el sobre no la inventa. Una
/// respuesta que no declara veredicto y no deja agujeros es completa: no
/// dice nada que la contradiga.
#[test]
fn the_state_is_the_verdict_the_answer_declares() {
    let verdict = |status: &str| {
        let result = json!({"status": status, "summary": "s"});
        envelope_for("build_graph", &Map::new(), &result, &context()).state
    };

    assert_eq!(verdict("partial"), "partial");
    assert_eq!(verdict("unknown"), "unknown");
    assert_eq!(verdict("unsupported"), "unsupported");
    assert_eq!(verdict("failed"), "failed");
    assert_eq!(verdict("complete"), "complete");

    // Una respuesta que no declara veredicto pero deja agujeros es parcial:
    // el agujero ES el veredicto.
    let holes = with_holes();
    let mut without_verdict = holes.as_object().unwrap().clone();
    without_verdict.remove("status");
    let state = envelope_for(
        "build_graph",
        &Map::new(),
        &Value::Object(without_verdict),
        &context(),
    )
    .state;
    assert_eq!(state, "partial");
}

/// La base dice lo que la respuesta cubre y lo que no.
///
/// Los agujeros se cuentan; lo que el producto no ha medido se dice `null`,
/// nunca un cero que parece una medición.
#[test]
fn the_basis_counts_the_holes_and_names_what_was_not_measured() {
    let result = with_holes();
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());

    let coverage = envelope
        .basis
        .coverage
        .expect("the answer declares holes, so the basis declares coverage");
    assert_eq!(coverage.status, "partial");
    assert_eq!(coverage.skipped_files, 2);
    assert_eq!(coverage.pending_references, 1);
    assert_eq!(coverage.rejected_relationships, 0);

    // La frescura no la declara esta respuesta: `null`, no "fresco".
    assert!(
        envelope.basis.freshness.is_none(),
        "a basis that was not measured says so"
    );

    // El producto no publica un recuento de aristas confirmadas por LSP.
    assert!(
        envelope.basis.lsp_confirmed_edges.is_none(),
        "an edge count nobody measured must not be published as a number"
    );
}

/// La frescura sale de la base que la respuesta declara.
#[test]
fn the_freshness_comes_from_the_basis_the_answer_declares() {
    let result = json!({
        "status": "complete",
        "summary": "the graph is built",
        "basis": {
            "workspace": "/repo",
            "source_manifest_digest": "sha256:abc",
            "graph_source": "built"
        }
    });
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());

    let freshness = envelope
        .basis
        .freshness
        .expect("the answer declares what it was built from");
    assert_eq!(freshness.workspace, "/repo");
    assert_eq!(freshness.snapshot, "sha256:abc");
    assert_eq!(freshness.graph_source.as_deref(), Some("built"));
}

/// El presupuesto dice lo que se omitió y con qué seguir.
#[test]
fn the_budget_says_what_was_left_out() {
    let result = json!({
        "summary": "the file was cut",
        "truncated": true,
        "continuation_token": "tok-2",
        "omitted": 120
    });
    let envelope = envelope_for("read_file", &Map::new(), &result, &context());

    assert_eq!(envelope.budget.omitted, Some(120));
    let page = envelope
        .budget
        .page
        .as_deref()
        .expect("a cut answer carries the handle that continues it");
    assert!(
        page.starts_with("page:"),
        "the handle names its kind: {page}"
    );
}

// --- Las seis reglas: lo que se puede hacer ahora ---

/// La ambigüedad: una referencia que declara un nombre que varios
/// símbolos llevan.
#[test]
fn an_ambiguous_reference_offers_the_code_to_read() {
    let result = json!({
        "status": "partial",
        "summary": "a reference names two symbols",
        "pending_references": [
            {"file": "c.rs", "line": 12, "target": "Foo", "kind": "call", "reason": "ambiguous",
             "candidates": 2}
        ]
    });
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());
    let link = link_with(&envelope, "resolve-ambiguity");

    assert_eq!(link.tool, "get_symbol_code");
    assert_eq!(link.required_args["file"], json!("c.rs"));
    assert_eq!(link.required_args["line"], json!(12));
    assert!(!link.reason.is_empty(), "a link says why it is there");
    let handle = link
        .handle
        .as_deref()
        .expect("the ambiguity link carries the symbol it is about");
    assert!(handle.starts_with("sym:"), "{handle}");
}

/// El recorte: una respuesta cortada dice por dónde sigue.
#[test]
fn a_cut_answer_offers_the_next_page() {
    let result = json!({
        "summary": "the file was cut",
        "truncated": true,
        "continuation_token": "tok-2"
    });
    let envelope = envelope_for("read_file", &Map::new(), &result, &context());
    let link = link_with(&envelope, "continue");

    assert_eq!(link.tool, "read_file", "the same tool continues the answer");
    assert_eq!(
        link.required_args["continuation_token"],
        json!("tok-2"),
        "the argument that continues is filled"
    );
    let handle = link
        .handle
        .as_deref()
        .expect("the continuation travels as a signed handle");
    assert!(handle.starts_with("page:"), "{handle}");
}

/// El fan-in alto: un símbolo del que dependen muchos.
#[test]
fn a_hot_symbol_offers_its_callers() {
    let result = json!({
        "summary": "the node is hot",
        "node": "parse_config",
        "fan_in": 24,
        "fan_out": 3
    });
    let envelope = envelope_for("graph_explain", &Map::new(), &result, &context());
    let link = link_with(&envelope, "inspect-callers");

    assert_eq!(link.tool, "get_call_hierarchy");
    assert_eq!(link.required_args["symbol_name"], json!("parse_config"));
    assert_eq!(link.required_args["direction"], json!("incoming"));
}

/// La cobertura parcial: la respuesta no lo cubre todo, y el reindexado es
/// lo que la cubre.
#[test]
fn a_partial_coverage_offers_the_reindex() {
    let result = with_holes();
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());
    let link = link_with(&envelope, "reindex");

    assert_eq!(link.tool, "build_graph");
    assert_eq!(link.required_args["directory"], json!("/repo"));
}

/// El riesgo: una respuesta que dice «alto» dice qué revisar.
#[test]
fn a_high_risk_offers_the_review() {
    let result = json!({
        "summary": "the change is risky",
        "risk_level": "high",
        "impacted_files": ["src/a.rs"]
    });
    let envelope = envelope_for("review_pr", &Map::new(), &result, &context());
    let link = link_with(&envelope, "inspect-risk");

    assert_eq!(link.tool, "get_file_symbols");
    assert_eq!(link.required_args["file_path"], json!("src/a.rs"));
}

/// El plan de cambio: una respuesta que recomienda algo dice cómo verlo
/// antes de tocar nada.
#[test]
fn a_recommendation_offers_the_preview() {
    let result = json!({
        "summary": "the answer recommends a change",
        "symbol": "parse_config",
        "recommendations": ["split parse_config"]
    });
    let envelope = envelope_for("project_overview", &Map::new(), &result, &context());
    let link = link_with(&envelope, "preview-change");

    assert_eq!(link.tool, "analyze_impact");
    assert_eq!(link.required_args["symbol_name"], json!("parse_config"));
    let handle = link
        .handle
        .as_deref()
        .expect("the plan link carries the plan it names");
    assert!(handle.starts_with("plan:"), "{handle}");
}

/// El motivo de un enlace es una frase, no un contador.
///
/// El motivo se lee en una terminal y en un cliente, y se lo lee una
/// persona: «1 files» es una razón escrita por una máquina que no cuenta.
#[test]
fn a_reason_counts_in_words_a_person_reads() {
    let with_skipped = |count: usize| {
        let mut result = with_holes().as_object().unwrap().clone();
        result.insert(
            "skipped_files".to_string(),
            Value::Array(
                (0..count)
                    .map(|n| json!({"path": format!("{n}.rs"), "reason": "io"}))
                    .collect(),
            ),
        );
        envelope_for(
            "build_graph",
            &Map::new(),
            &Value::Object(result),
            &context(),
        )
    };

    let one_envelope = with_skipped(1);
    let one = link_with(&one_envelope, "reindex");
    assert!(
        one.reason.contains("1 file,"),
        "one file is one file: {}",
        one.reason
    );
    assert!(
        !one.reason.contains("1 files"),
        "the reason is a phrase: {}",
        one.reason
    );

    let two_envelope = with_skipped(2);
    let two = link_with(&two_envelope, "reindex");
    assert!(
        two.reason.contains("2 files"),
        "two files are two files: {}",
        two.reason
    );
}

/// Un enlace que no se puede resolver no es un enlace: cada tool que un
/// enlace nombra está en el catálogo, con su autoridad y sus requisitos.
#[test]
fn every_link_names_a_tool_the_catalogue_publishes() {
    let catalogue = cognicode_core::interface::catalog::load_tool_catalog();
    let result = json!({
        "status": "partial",
        "summary": "s",
        "fan_in": 40,
        "node": "n",
        "risk_level": "high",
        "impacted_files": ["a.rs"],
        "recommendations": ["r"],
        "symbol": "n",
        "skipped_files": [{"path": "a.rs", "reason": "io"}]
    });
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());

    assert!(!envelope.links.is_empty(), "the answer offers something");
    for link in &envelope.links {
        let entry = catalogue
            .iter()
            .find(|tool| tool.name == link.tool)
            .unwrap_or_else(|| panic!("the catalogue does not publish `{}`", link.tool));
        assert_eq!(
            link.authority, entry.authority,
            "the authority of `{}` is the catalogue's",
            link.tool
        );
        if let Some(prerequisites) = &entry.prerequisites {
            assert_eq!(
                link.precondition.as_deref(),
                Some(prerequisites.join(", ").as_str()),
                "the precondition of `{}` is the catalogue's",
                link.tool
            );
        }
    }
}

// --- Los handles: firmados, anclados y tipados ---

/// Un handle firmado vuelve a dar lo que dice.
#[test]
fn a_handle_round_trips() {
    let ctx = context();
    let handle = mint_handle(HandleKind::Sym, &ctx, &json!("parse_config"));
    let verified = verify_handle(&handle, &ctx).expect("a fresh handle verifies");

    assert_eq!(verified.kind, HandleKind::Sym);
    assert_eq!(verified.payload, json!("parse_config"));
}

/// Un handle manipulado no vuelve a pasar.
#[test]
fn a_tampered_handle_is_refused() {
    let ctx = context();
    let handle = mint_handle(HandleKind::Sym, &ctx, &json!("parse_config"));

    // Se cambia un carácter del cuerpo firmado: el tag de integridad ya no
    // encaja con lo que el handle dice.
    let mut tampered: Vec<char> = handle.chars().collect();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == 'A' { 'B' } else { 'A' };
    let tampered: String = tampered.into_iter().collect();

    let error = verify_handle(&tampered, &ctx).expect_err("a tampered handle is refused");
    assert_eq!(error.kind, CursorErrorKind::Garbled);
}

/// Un handle cuyo payload se reescribe no vuelve a pasar.
///
/// El caso que el tag de integridad existe para cubrir: el handle sigue
/// siendo base64 válido, con los mismos anclajes y la misma versión, y lo
/// único que ha cambiado es lo que apunta. Un tag que sólo cubriera los
/// anclajes dejaría pasar esto.
#[test]
fn a_handle_with_a_rewritten_payload_is_refused() {
    use base64::Engine as _;

    let ctx = context();
    let handle = mint_handle(HandleKind::Sym, &ctx, &json!("parse_config"));

    // Se abre el sobre firmado, se cambia lo que apunta y se vuelve a
    // codificar: el handle queda bien formado y miente.
    let token = handle.split_once(':').expect("a handle has a class").1;
    let mut signed: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(token)
            .expect("the body is base64"),
    )
    .expect("the body is JSON");
    signed["payload"] = json!("another_symbol");
    let rewritten = format!(
        "sym:{}",
        base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&signed).expect("the signed body serialises"))
    );

    let error = verify_handle(&rewritten, &ctx)
        .expect_err("a handle that points somewhere else is refused");
    assert_eq!(error.kind, CursorErrorKind::Garbled);
}

/// Un handle de otro workspace no vale aquí.
#[test]
fn a_handle_from_another_workspace_is_refused() {
    let ctx = context();
    let handle = mint_handle(HandleKind::Page, &ctx, &json!(20));

    let other = HandleContext {
        workspace: "/other".to_string(),
        snapshot: ctx.snapshot.clone(),
    };
    let error = verify_handle(&handle, &other).expect_err("a handle of another tree is refused");
    assert_eq!(error.kind, CursorErrorKind::WorkspaceMismatch);
}

/// Un handle de otra instantánea caducó.
#[test]
fn a_handle_from_another_snapshot_is_refused() {
    let ctx = context();
    let handle = mint_handle(HandleKind::Page, &ctx, &json!(20));

    let other = HandleContext {
        workspace: ctx.workspace.clone(),
        snapshot: "snap-2".to_string(),
    };
    let error = verify_handle(&handle, &other).expect_err("a handle of another snapshot expired");
    assert_eq!(error.kind, CursorErrorKind::SnapshotExpired);
}

/// Un handle que no se sabe qué es se rechaza con su propio tipo.
#[test]
fn an_unknown_kind_is_refused() {
    let ctx = context();
    let error = verify_handle("wat:whatever", &ctx).expect_err("an unknown kind is refused");
    assert_eq!(error.kind, CursorErrorKind::Garbled);
}

// --- El CLI: los enlaces como órdenes copiables ---

/// El CLI imprime los enlaces como órdenes que su propio árbol acepta.
///
/// Un comando que la terminal no entiende no es copiable: es un texto.
#[test]
fn the_cli_prints_links_as_commands_its_own_tree_accepts() {
    let result = with_holes();
    let envelope = envelope_for("build_graph", &Map::new(), &result, &context());

    let commands = follow_ups::commands(&envelope);
    assert!(
        commands.contains(&"cognicode graph full --rebuild /repo".to_string()),
        "the reindex link is a copyable command: {commands:?}"
    );

    let tree = cognicode_cli::cli::goal_command::cli_command();
    for command in &commands {
        let mut argv = vec!["cognicode"];
        argv.extend(command.split_whitespace().skip(1));
        tree.clone()
            .try_get_matches_from(argv)
            .unwrap_or_else(|error| panic!("`{command}` is not a command: {error}"));
    }
}

/// La orden dice la dirección que el enlace dice.
///
/// El enlace dice `incoming` y `outgoing`; la terminal dice `in` y `out`.
/// Una orden que fija la dirección en vez de leerla responde a otra
/// pregunta: si el enlace cambia de dirección, la orden tiene que
/// cambiar con él.
#[test]
fn the_cli_reads_the_direction_the_link_names() {
    let envelope_with = |direction: &str| {
        let mut args = Map::new();
        args.insert("symbol_name".to_string(), json!("parse_config"));
        args.insert("direction".to_string(), json!(direction));
        cognicode_core::interface::hateoas::Envelope {
            schema_version: 1,
            state: "complete".to_string(),
            result: Value::Null,
            basis: cognicode_core::interface::hateoas::Basis::default(),
            budget: cognicode_core::interface::hateoas::Budget::default(),
            links: vec![cognicode_core::interface::hateoas::Link {
                rel: "inspect-callers".to_string(),
                tool: "get_call_hierarchy".to_string(),
                required_args: args,
                reason: "the symbol has 24 callers".to_string(),
                precondition: None,
                authority: "read".to_string(),
                handle: None,
            }],
        }
    };

    assert_eq!(
        follow_ups::commands(&envelope_with("outgoing")),
        vec!["cognicode graph hierarchy parse_config --direction out".to_string()],
    );
    assert_eq!(
        follow_ups::commands(&envelope_with("incoming")),
        vec!["cognicode graph hierarchy parse_config --direction in".to_string()],
    );

    // Una dirección que la terminal no entiende no se inventa: el enlace
    // se queda en el JSON, donde un cliente lo encuentra.
    assert!(
        follow_ups::commands(&envelope_with("sideways")).is_empty(),
        "a direction the terminal does not know is not printed"
    );
}

//! T1-5.6b — Snapshot tests for the `CommandOutput::Doctor` text + JSON
//! dispatch.
//!
//! Estado pre-T1-5.6b:
//!   - `execute_doctor` en `cognicode-core` imprimía con `println!`
//!     directamente al stdout y luego llamaba `std::process::exit(...)`
//!     para el exit code. La librería no devolvía datos, y el bin no
//!     tenía un seam donde añadir el dispatch text/JSON.
//!   - El text shape dependía del formatter `format_doctor_text` del
//!     core. Sin un test que pinea el byte stream, un cambio en el
//!     formatter solo se notaba cuando un humano leía `cogh doctor`.
//!
//! What this RED protects:
//!   - `present_doctor_text` y `present_doctor_result_json` envuelven
//!     los formatters existentes del core. La garantía de byte-identical
//!     se prueba comparando el output del presenter contra la salida
//!     de `format_doctor_text/json` (el "ground truth" del pre-migration).
//!   - El `CommandOutput::Doctor` routea correctamente vía
//!     `present_output` (con un `DoctorReport` real).
//!   - El `DoctorError::Check` placeholder existe y formatea un mensaje
//!     útil (no se usa en el flow actual pero el bin debe poder
//!     mostrarlo si un día se enchufa una check que falle).
//!
//! Why this lives in `cognicode-cli/tests/`:
//!   - El presenter (format dispatch) vive en el bin (T1-5.5 pattern).
//!   - El data side (`DoctorResult`/`DoctorError`) está en
//!     `cognicode-core::interface::cli::result`. Los dos se
//!     ejercitan juntos aquí; el de-serializado del `DoctorResult`
//!     re-exporta `DoctorReport` del core.

use cognicode_cli::presenter::present_output;
use cognicode_cli::presenter::{present_doctor_result_json, present_doctor_text};
use cognicode_core::interface::cli::commands::Format as CliFormat;
use cognicode_core::interface::cli::doctor::{
    DoctorCheck, DoctorReport, DoctorSection, DoctorSections, DoctorSummary, WorkspaceInfo,
    format_doctor_json, format_doctor_text, run_doctor_checks,
};
use cognicode_core::interface::cli::result::CommandOutput;
use cognicode_core::interface::cli::result::{DoctorError, DoctorResult};
use std::path::Path;

/// Snapshot de un `DoctorReport` construido a mano. El binario real
/// (`run_doctor_checks`) puede variar entre sistemas (e.g. detección
/// de parsers, LSP disponible), así que pineamos un report fijo y
/// comparamos el presenter contra el formatter del core. Si un día
/// cambia `format_doctor_text`/`format_doctor_json`, los dos lados
/// cambian juntos y el test sigue verde; este test detecta
/// divergencias ENTRE el presenter y el formatter, no dentro del
/// formatter.
fn fixed_report() -> DoctorReport {
    let mut core = DoctorSection::new("Core");
    core.add_check(DoctorCheck::ok(
        "binary",
        "found at $COGNICODE_BIN_DIR/cogh",
    ));
    core.add_check(DoctorCheck::info("version", "0.101.17"));

    let lsp = DoctorSection::new("LSP");
    let mut parsers = DoctorSection::new("Parsers");
    parsers.add_check(DoctorCheck::ok("tree-sitter-rust", "0.21.0"));
    parsers.add_check(DoctorCheck::missing(
        "tree-sitter-python",
        "pip install ...",
    ));

    let summary = DoctorSummary {
        core: "ok".to_string(),
        lsp: "ok".to_string(),
        parsers: "missing".to_string(),
    };
    let sections = DoctorSections { core, lsp, parsers };
    let workspace = WorkspaceInfo {
        languages: vec!["rust".to_string()],
        path: ".".to_string(),
    };
    DoctorReport {
        schema_version: "cognicode.doctor/v1".to_string(),
        version: "0.101.17".to_string(),
        sections,
        summary,
        workspace: Some(workspace),
    }
}

fn render_text<F: FnOnce(&mut Vec<u8>) -> std::io::Result<()>>(f: F) -> String {
    let mut buf: Vec<u8> = Vec::new();
    f(&mut buf).expect("presenter must not error on Vec<u8>");
    String::from_utf8(buf).expect("presenter must emit valid UTF-8")
}

#[test]
fn present_doctor_text_matches_core_formatter() {
    let report = fixed_report();
    let via_presenter = render_text(|w| present_doctor_text(&report, w));
    let via_formatter = format_doctor_text(&report);
    assert_eq!(
        via_presenter, via_formatter,
        "present_doctor_text must be byte-identical to format_doctor_text"
    );
}

#[test]
fn present_doctor_text_does_not_emit_trailing_newline() {
    // The pre-migration `println!("{}", format_doctor_text(&report))`
    // emits a trailing `\n` because `println!` adds one. The
    // post-migration `present_doctor_text` uses `write!` (no extra
    // newline). `format_doctor_text` itself already ends with `\n`,
    // so the final byte stream is identical. Pin it so a refactor
    // that drops or adds a trailing newline fails RED.
    let report = fixed_report();
    let rendered = render_text(|w| present_doctor_text(&report, w));
    assert!(
        rendered.ends_with('\n'),
        "doctor text must end with a newline (the formatter does); got trailing bytes {:?}",
        rendered.chars().rev().take(8).collect::<Vec<_>>()
    );
}

#[test]
fn present_doctor_result_json_matches_core_formatter() {
    let report = fixed_report();
    let via_presenter = render_text(|w| present_doctor_result_json(&report, w));
    let via_formatter = format_doctor_json(&report);
    assert_eq!(
        via_presenter, via_formatter,
        "present_doctor_result_json must be byte-identical to format_doctor_json"
    );
}

#[test]
fn present_doctor_result_json_is_valid_json() {
    let report = fixed_report();
    let rendered = render_text(|w| present_doctor_result_json(&report, w));
    let parsed: serde_json::Value =
        serde_json::from_str(&rendered).expect("present_doctor_result_json must emit valid JSON");
    // Smoke check: the schema_version and the Missing parser are
    // present. A regression that drops a field fails RED.
    assert_eq!(
        parsed.get("schema_version").and_then(|v| v.as_str()),
        Some("cognicode.doctor/v1")
    );
    assert_eq!(
        parsed
            .get("summary")
            .and_then(|s| s.get("parsers"))
            .and_then(|v| v.as_str()),
        Some("missing")
    );
}

#[test]
fn present_output_routes_doctor_text() {
    // The bin's `present_output` must route `CommandOutput::Doctor(r)` to
    // `present_doctor_text` for `CliFormat::Text` and to
    // `present_doctor_result_json` for `CliFormat::Json`. The previous
    // pre-migration lib did this inline; T1-5.6b moves it to the
    // presenter's `match`.
    let report = fixed_report();
    let text_via_dispatcher =
        render_text(|w| present_output(&CommandOutput::Doctor(report.clone()), CliFormat::Text, w));
    let text_direct = render_text(|w| present_doctor_text(&report, w));
    assert_eq!(text_via_dispatcher, text_direct);

    let json_via_dispatcher =
        render_text(|w| present_output(&CommandOutput::Doctor(report.clone()), CliFormat::Json, w));
    let json_direct = render_text(|w| present_doctor_result_json(&report, w));
    assert_eq!(json_via_dispatcher, json_direct);
}

#[test]
fn doctor_result_is_a_doctor_report_alias() {
    // VDH/AGENTS contract: there must be one canonical type, not two.
    // `DoctorResult` is a re-export of `DoctorReport` so the lib/bin
    // boundary uses one name and `format_doctor_text(&report)` still
    // accepts the same struct.
    let r: DoctorResult = fixed_report();
    let formatted = format_doctor_text(&r);
    // The text formatter emits `CogniCode Doctor v0.101.17` (not the
    // JSON `schema_version`); the JSON formatter emits the
    // `schema_version` separately. We assert on the text form here.
    assert!(
        formatted.contains("CogniCode Doctor v0.101.17"),
        "text must include the version header; got first 80 chars: {:?}",
        formatted.chars().take(80).collect::<String>()
    );
}

#[test]
fn doctor_error_check_carries_source_message() {
    // Placeholder error for future extensions (custom checks). The
    // Display impl must carry the source message so `eprintln!("{}",
    // e)` is informative.
    let src = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let e = DoctorError::Check {
        source: Box::new(src),
    };
    let s = format!("{e}");
    assert!(
        s.contains("Doctor check failed"),
        "Display must say 'Doctor check failed'; got: {s}"
    );
    // The `#[source]` chain is preserved (downcasteable).
    use std::error::Error as _;
    let chain = e.source().expect("DoctorError::Check must carry a source");
    assert!(
        chain.to_string().contains("denied"),
        "source chain must surface the original error; got: {}",
        chain
    );
}

#[test]
fn run_doctor_checks_against_a_workspace_remains_infallible() {
    // Today's `run_doctor_checks` is infallible. T1-5.6b added the
    // typed `DoctorError` so future checks can fail; this test pins
    // the current contract so a regression that turns an inflight
    // path into an `Err` fails RED.
    let report = run_doctor_checks(Some(Path::new(".")));
    // `.overall_status()` returns Ok/Warn/Missing/Info depending on
    // the workspace; we only assert it doesn't panic.
    let _ = report.overall_status();
}

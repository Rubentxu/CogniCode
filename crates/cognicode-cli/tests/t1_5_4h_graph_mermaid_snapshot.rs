//! T1-5.4h — Snapshot test for the `GraphResult::Mermaid` presenter.
//!
//! Estado pre-T1-5.4h:
//!   - T1-5.4b designed Mermaid with a `diagram_len: usize` field
//!     (no `diagram` field). The intent was to keep the data
//!     side small (mermaid diagrams can be megabytes for big
//!     repos).
//!   - The T1-5.4h snapshot test exposed the design mistake:
//!     the pre-migration text EMBEDS the full diagram (either
//!     the first 2000 chars + 'truncated' notice, or the full
//!     diagram in a shell snippet for svg/png), and the
//!     presenter needs the content to reproduce the text
//!     byte-identical. The `diagram: String` field was added
//!     in this commit (data side is now larger but
//!     reproducibility is the contract).
//!
//! What this RED protects:
//!   - The exact byte sequence of the result block + summary
//!     for the three format cases (txt non-truncated, txt
//!     truncated, svg/png hint).
//!   - The 'Mermaid diagram (N chars):' header uses the full
//!     diagram length (NOT the truncated 2000). The user
//!     always sees the real size.
//!   - The 'Run: cat << 'EOF' | mermaid -s\n{diagram}\nEOF'
//!     shell snippet for svg/png format, including the
//!     single-quoted heredoc delimiter.
//!   - The '... (truncated, full output in file)' notice
//!     appears only when the diagram is over 2000 chars AND
//!     the format is text (not svg/png).
//!
//! Lo que T1-5.4h NO testea:
//!   - El 'Warning: graph is PARTIAL' eprintln! a stderr.
//!   - El 'Exporting to Mermaid format from: PATH' progress
//!     line (mismo patrón que TracePath: el variant no carga
//!     el project path).

use cognicode_cli::presenter::present_graph_mermaid;
use cognicode_core::interface::cli::result::GraphResult;

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_mermaid(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_mermaid_txt_full() {
    // A short diagram (< 2000 chars). The presenter emits
    // the full content. No 'truncated' notice.
    let result = GraphResult::Mermaid {
        path: "/work/repo".to_string(),
        diagram: "graph TD\n  A --> B\n  B --> C\n".to_string(),
        diagram_len: 28,
        format: "txt".to_string(),
        truncated: false,
        elapsed_ms: 40,
    };
    let out = render(&result);
    // The diagram ends with \n; the presenter adds a
    // trailing \n via writeln!; then a blank line; then
    // the summary. So 3 \n separate the diagram's last
    // content char ('C') from 'Export completed' (diagram's
    // own trailing \n, writeln! trailing \n, blank line).
    assert_eq!(
        out,
        "\n\
         Mermaid diagram (28 chars):\n\
         \n\
         graph TD\n  A --> B\n  B --> C\n\
         \n\
         \n\
         Export completed in 40ms\n",
        "Mermaid txt-full text changed; check the full diagram content and the no-truncation case"
    );
}

#[test]
fn snapshot_mermaid_txt_truncated() {
    // A diagram that exceeds 2000 chars. The presenter
    // emits the first 2000 chars + 'truncated' notice. The
    // header still says '(N chars)' with the FULL length.
    let mut long_diagram = String::with_capacity(2500);
    // 120 lines × 18 chars/line = 2160 chars (> 2000 to
    // trigger the truncation branch). Each line is
    // "  N0000 --> N0001\n".
    for i in 0..120 {
        long_diagram.push_str(&format!("  N{:04} --> N{:04}\n", i, i + 1));
    }
    let result = GraphResult::Mermaid {
        path: "/work/repo".to_string(),
        diagram: long_diagram.clone(),
        diagram_len: long_diagram.len(),
        format: "txt".to_string(),
        truncated: true,
        elapsed_ms: 60,
    };
    let out = render(&result);
    // The header uses the full length (not 2000).
    let header_line = format!("Mermaid diagram ({} chars):", long_diagram.len());
    assert!(
        out.contains(&header_line),
        "Header must use the full diagram length ({header_line:?}): {out:?}"
    );
    // The truncated notice is present.
    assert!(
        out.contains("... (truncated, full output in file)"),
        "Truncated notice must be present: {out:?}"
    );
    // The first 2000 chars of the diagram are emitted
    // (followed by the notice).
    let prefix = &long_diagram[..2000];
    assert!(
        out.contains(prefix),
        "First 2000 chars must be in the output: {out:?}"
    );
    // The 2001st char (and beyond) is NOT in the output.
    let beyond = &long_diagram[2000..2010];
    assert!(
        !out.contains(beyond),
        "Chars beyond position 2000 must NOT be in the output: {out:?}"
    );
}

#[test]
fn snapshot_mermaid_svg_hint() {
    // The svg/png branch emits a 'requires mermaid-cli'
    // notice + a shell snippet. The diagram content is
    // embedded in the snippet (full content, not truncated,
    // because the snippet is meant to be saved to a file
    // and rendered by mermaid-cli).
    let result = GraphResult::Mermaid {
        path: "/work/repo".to_string(),
        diagram: "graph TD\n  A --> B\n".to_string(),
        diagram_len: 20,
        format: "svg".to_string(),
        truncated: false, // svg/png always has truncated=false (the data side sets it that way)
        elapsed_ms: 25,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "\n\
         Mermaid diagram (20 chars):\n\
         \n\
         Note: SVG/PNG export requires mermaid-cli\n\
         Run: cat << 'EOF' | mermaid -s\ngraph TD\n  A --> B\n\n\
         EOF\n\
         \n\
         Export completed in 25ms\n",
        "Mermaid svg/png text changed; check the 'Note: SVG/PNG export requires mermaid-cli' hint and the shell snippet"
    );
}

#[test]
fn snapshot_mermaid_png_uses_same_hint_as_svg() {
    // The svg and png branches share the same hint. Pin
    // that 'png' produces the same shell snippet as 'svg'.
    let result = GraphResult::Mermaid {
        path: "/work/repo".to_string(),
        diagram: "x".to_string(),
        diagram_len: 1,
        format: "png".to_string(),
        truncated: false,
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert!(
        out.contains("Note: SVG/PNG export requires mermaid-cli"),
        "png format must use the same 'SVG/PNG' hint as svg: {out:?}"
    );
    assert!(
        out.contains("Run: cat << 'EOF' | mermaid -s\nx\nEOF"),
        "png format must use the same shell snippet as svg: {out:?}"
    );
}

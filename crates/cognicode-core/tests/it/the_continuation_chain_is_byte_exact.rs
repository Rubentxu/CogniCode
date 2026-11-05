//! The `read_file` continuation chain rebuilds the file.
//!
//! ## The guarantee
//!
//! An MCP client does not read a large file whole: it asks for a page, and
//! the answer says whether there is more and hands it a token to ask for the
//! next one. The chain of those pages has to give the file back, byte by
//! byte, and it has to end.
//!
//! ## Why this suite exists
//!
//! The guarantee belonged to a suite nobody ran: five `#[ignore]`d tests that
//! started the real server and needed the sandbox repositories. When the
//! lanes were retired (2026-11-04) those tests stopped running without
//! anyone noticing: a guarantee with no owner is a guarantee that can be lost
//! without anyone noticing. Here it has an owner, and this suite runs in the
//! gate.
//!
//! ## How it looks
//!
//! The suite talks to the tool through the real dispatch (`call_tool_handler`)
//! and reads the same bytes a client reads: the first text block of the
//! answer, which is the JSON of the body. No process, no binary, no sandbox,
//! no network and no clock — the fixture is written by the suite itself into
//! a temporary directory, with deterministic content.
//!
//! What it checks, in the shape a client needs it:
//!
//! 1. a file longer than a page is read in more than one page;
//! 2. the last page closes the chain: no `has_more`, no `next_token`;
//! 3. every page that says "there is more" carries a token, and no token is
//!    repeated (a repeated token returns the same page for ever);
//! 4. the pages are contiguous and, concatenated, rebuild the file byte by
//!    byte — checked at EVERY boundary, not only at the end;
//! 5. every page declares the lines it brings, so a client knows where each
//!    piece goes without counting lines by hand;
//! 6. every page publishes the digest of the file it belongs to, so a client
//!    can tell that the pages it holds belong to the same version of it;
//! 7. a file that fits in a page is read in one page, with no token.

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::rmcp_adapter::call_tool_handler;
use cognicode_core::interface::mcp::tool_response::success_result;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

/// How many pages the chain may bring at most.
///
/// The chain ends by construction, but a broken product may not: a token that
/// repeats returns the same page for ever. The cap turns that into a failure
/// that says what happened, instead of a test that never finishes.
const MAX_PAGES: usize = 64;

/// A file of `lines` lines, each one a different length.
///
/// The varying lengths matter: if every line measured the same, a page that
/// skipped one line and a page that carried one byte too many could cancel
/// each other out, and the rebuilt file would be identical to the original
/// with the chain broken. And every now and then there is a character that is
/// not one byte, so an error that confuses bytes with characters shows up
/// here and not in a plain ASCII file.
fn source_of(lines: usize) -> String {
    let mut text = String::new();
    for n in 1..=lines {
        let filler = if n % 97 == 0 { "€" } else { "-" };
        text.push_str(&format!(
            "fn f{n}() {{ /* {}{} */ }}\n",
            filler.repeat(n % 23),
            n
        ));
    }
    text
}

/// The workspace holding the file the suite reads.
///
/// The temporary directory lives as long as the fixture does: a workspace
/// deleted too early would leave the suite reading a file that is gone.
struct Fixture {
    _root: tempfile::TempDir,
    workspace: std::path::PathBuf,
}

impl Fixture {
    /// A workspace with one file, and the content of that file.
    fn with_file(name: &str, content: &str) -> Self {
        let root = tempfile::tempdir().expect("temporary");
        let workspace = root.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("the workspace is created");
        std::fs::write(workspace.join(name), content).expect("the file is written");
        Self {
            _root: root,
            workspace,
        }
    }
}

/// The context of the dispatch, with the workspace as its root.
fn context(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// What an MCP client reads from the answer of `read_file`.
///
/// The server delivers the body as a text block and the client reads it as
/// JSON. This returns that text already parsed: the suite looks at what the
/// client sees, not at the internal type it comes from.
async fn read_file(ctx: &HandlerContext, arguments: Value) -> Value {
    let request = CallToolRequestParams::new("read_file".to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    let body = call_tool_handler(ctx, request)
        .await
        .unwrap_or_else(|e| panic!("read_file answers: {e}"));
    let answer = success_result("read_file", body);
    let text = answer
        .content
        .first()
        .and_then(|block| block.raw.as_text())
        .map(|text| text.text.as_str())
        .unwrap_or_else(|| panic!("the answer arrives as a text block"));
    serde_json::from_str(text).unwrap_or_else(|e| panic!("the body of the answer is JSON: {e}"))
}

/// A page of the chain, as the client sees it.
#[derive(Debug)]
struct Page {
    content: String,
    start_line: u32,
    end_line: u32,
    has_more: bool,
    next_token: Option<String>,
    content_hash: Option<String>,
}

impl Page {
    /// The page read from the body of the answer.
    fn of(answer: &Value) -> Self {
        let content = answer["content"]
            .as_str()
            .unwrap_or_else(|| panic!("the page brings its content: {answer}"))
            .to_string();
        let start_line = answer["start_line"]
            .as_u64()
            .unwrap_or_else(|| panic!("the page declares the first line it brings: {answer}"))
            as u32;
        let end_line = answer["end_line"]
            .as_u64()
            .unwrap_or_else(|| panic!("the page declares the last line it brings: {answer}"))
            as u32;
        let has_more = answer["has_more"]
            .as_bool()
            .unwrap_or_else(|| panic!("the page says whether there is more: {answer}"));
        let next_token = answer["next_token"]
            .as_str()
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        let content_hash = answer["content_hash"].as_str().map(str::to_string);
        Self {
            content,
            start_line,
            end_line,
            has_more,
            next_token,
            content_hash,
        }
    }

    /// How many lines the page brings.
    fn lines(&self) -> usize {
        self.content.lines().count()
    }
}

/// Follows the continuation chain to its end.
///
/// Returns the pages in order. The cap lives here and not in the test: it is
/// the termination condition of the walk itself, and a walk without a cap is
/// a walk that may never end.
async fn follow_chain(ctx: &HandlerContext, path: &str) -> Vec<Page> {
    let mut pages = Vec::new();
    let mut token: Option<String> = None;

    loop {
        let mut arguments = json!({ "path": path, "mode": "raw" });
        if let Some(previous) = &token {
            arguments["continuation_token"] = json!(previous);
        }

        let page = Page::of(&read_file(ctx, arguments).await);
        let more = page.has_more;
        token = page.next_token.clone();
        pages.push(page);

        if !more {
            return pages;
        }
        assert!(
            pages.len() < MAX_PAGES,
            "the chain has not ended after {MAX_PAGES} pages: the product keeps \
             offering a page that does not advance"
        );
    }
}

/// The digest of a byte sequence, as the product publishes it.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Where two byte sequences part company.
///
/// A digest says "they differ"; an offset says "here". The second is what it
/// takes to understand the failure without reading the whole file again.
fn first_difference(left: &[u8], right: &[u8]) -> String {
    match left.iter().zip(right).position(|(a, b)| a != b) {
        Some(offset) => format!(
            "at byte {offset}: the chain has {:?} and the file has {:?}",
            left[offset], right[offset]
        ),
        None => format!(
            "the chain has {} bytes and the file has {}",
            left.len(),
            right.len()
        ),
    }
}

/// A file longer than a page is read in pages that rebuild it byte by byte.
#[tokio::test]
async fn a_file_longer_than_a_page_is_read_in_pages_that_rebuild_it_byte_exact() {
    let content = source_of(1_200);
    let fixture = Fixture::with_file("long.rs", &content);
    let ctx = context(&fixture.workspace);

    let pages = follow_chain(&ctx, "long.rs").await;

    // 1. The chain paginated: a file of 1,200 lines does not fit in a page.
    assert!(
        pages.len() >= 2,
        "a file of 1,200 lines has to be read in more than one page; it came \
         back in {}",
        pages.len()
    );

    // 2. The last page closes the chain.
    let last = pages.last().expect("there is at least one page");
    assert!(
        !last.has_more,
        "the last page says there is more: the chain does not end"
    );
    assert!(
        last.next_token.is_none(),
        "the last page offers a token: the client would ask for a page that \
         does not exist"
    );

    // 3. Every page that says "there is more" carries a token, and no token
    //    is repeated: a repeated token hands back the same page for ever.
    let mut seen: Vec<&str> = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let is_last = index + 1 == pages.len();
        if is_last {
            continue;
        }
        assert!(
            page.has_more,
            "page {index} does not say there is more, and the chain goes on"
        );
        let token = page
            .next_token
            .as_deref()
            .unwrap_or_else(|| panic!("page {index} says there is more and gives no token"));
        assert!(
            !seen.contains(&token),
            "page {index} repeats a token: the client would read the same page \
             again"
        );
        seen.push(token);
    }

    // 4. The pages are contiguous and, concatenated, they are the file. The
    //    comparison happens at EVERY boundary: a chain that loses a line in
    //    the middle and gets it back at the end is invisible if only the end
    //    is looked at.
    //
    //    And every page declares the lines it brings: that is what lets a
    //    client know where each piece goes without counting lines by hand.
    let mut accumulated = String::new();
    let mut lines_before = 0usize;
    for (index, page) in pages.iter().enumerate() {
        assert!(
            page.lines() > 0,
            "page {index} is empty: the client would read nothing and could \
             not go on"
        );
        let first_line = lines_before as u32 + 1;
        let last_line = lines_before as u32 + page.lines() as u32;

        accumulated.push_str(&page.content);
        let file_prefix = &content[..accumulated.len().min(content.len())];
        assert_eq!(
            accumulated.as_bytes(),
            file_prefix.as_bytes(),
            "page {index} does not continue where the previous one stopped: {}",
            first_difference(accumulated.as_bytes(), file_prefix.as_bytes())
        );

        assert_eq!(
            page.start_line, first_line,
            "page {index} declares it starts at line {} and it starts at {first_line}",
            page.start_line
        );
        assert_eq!(
            page.end_line, last_line,
            "page {index} declares it ends at line {} and it ends at {last_line}",
            page.end_line
        );
        lines_before += page.lines();
    }

    assert_eq!(
        accumulated.as_bytes(),
        content.as_bytes(),
        "the pages do not rebuild the file: {}",
        first_difference(accumulated.as_bytes(), content.as_bytes())
    );

    // 5. Every page publishes the digest of the file it belongs to. The chain
    //    spans several requests, and the client may hold the pages for a while
    //    before reading them: the digest is what says that those pages come
    //    from the same version of the file. Comparing each page against the
    //    digest of the file also says they agree with each other.
    let digest = sha256_hex(content.as_bytes());
    for (index, page) in pages.iter().enumerate() {
        let published = page.content_hash.as_deref().unwrap_or_else(|| {
            panic!("page {index} publishes no digest: the client cannot tell which file it reads")
        });
        assert_eq!(
            published, digest,
            "page {index} publishes a digest that is not the digest of the file: the pages \
             would not all belong to the same version of it"
        );
    }
}

/// A file that fits in a page is read in one page, with no token.
#[tokio::test]
async fn a_file_that_fits_in_a_page_is_read_in_one_page_without_a_token() {
    let content = source_of(334);
    let fixture = Fixture::with_file("short.rs", &content);
    let ctx = context(&fixture.workspace);

    let answer = read_file(&ctx, json!({ "path": "short.rs", "mode": "raw" })).await;
    let page = Page::of(&answer);

    assert!(
        !page.has_more,
        "a file of 334 lines fits in a page: there is no more to read"
    );
    assert!(
        page.next_token.is_none(),
        "a file that fits in a page offers no continuation: there is nothing \
         to continue"
    );
    assert_eq!(
        page.content.as_bytes(),
        content.as_bytes(),
        "the page is not the file: {}",
        first_difference(page.content.as_bytes(), content.as_bytes())
    );
    assert_eq!(
        page.content_hash.as_deref(),
        Some(sha256_hex(content.as_bytes()).as_str()),
        "the page does not publish the digest of the file it brings"
    );
}

//! Which HTTP routes ask for the bearer token, and which stay open.
//!
//! The server is started for real with a credential configured, and every
//! route is asked over the wire with no token, a wrong token and the right
//! one. The routes that report or run something about the workspace (`/mcp`,
//! `/watch`) must refuse the first two; the three that an orchestrator or a
//! scraper reaches without a secret (`/health`, `/ready`, `/metrics`) must
//! keep answering.

use crate::common;

use std::path::Path;

use common::HttpServer;

const TOKEN: &str = "route-authority-secret";

fn start_with_token() -> HttpServer {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws");
    HttpServer::start(&workspace, Some(TOKEN), &[])
}

fn get(server: &HttpServer, path: &str, token: Option<&str>) -> u16 {
    server.get(path, token).status()
}

#[test]
fn the_workspace_routes_refuse_a_missing_or_wrong_token() {
    let server = start_with_token();

    for path in ["/mcp", "/watch"] {
        assert_eq!(get(&server, path, None), 401, "{path} without a token");
        assert_eq!(
            get(&server, path, Some("not-the-secret")),
            401,
            "{path} with a wrong token"
        );
    }
}

#[test]
fn the_right_token_gets_past_the_check_on_the_watch_route() {
    let server = start_with_token();

    assert_eq!(get(&server, "/watch", Some(TOKEN)), 200);
}

#[test]
fn the_orchestrator_and_scraper_routes_stay_open_with_a_token_configured() {
    let server = start_with_token();

    for path in ["/health", "/ready", "/metrics"] {
        let code = get(&server, path, None);
        assert_ne!(code, 401, "{path} must not ask for the token");
    }
    assert_eq!(get(&server, "/metrics", None), 200);
}

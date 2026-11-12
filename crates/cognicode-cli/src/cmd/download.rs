//! Release downloads that follow redirects by hand, under an explicit policy.
//!
//! reqwest's redirect policy is switched off ([`client`]) and every hop is
//! decided by [`get`], because each decision has three parts that must hold
//! together:
//!
//! - a redirect may only lead to a host the policy names;
//! - it may only lead to `https`, never to plain `http`;
//! - the bearer token only travels to the hosts it is meant for.
//!
//! GitHub answers a release-asset request with a redirect to a signed
//! object-store URL. The signature is the credential on that hop, so the
//! object store receives the request without the GitHub token: a token sent
//! there would be a token handed to a host that never needed it.

use std::time::Duration;

use reqwest::Url;
use reqwest::blocking::{Client, Response};

use crate::error::InstallerError;

/// The most redirects a single download follows.
///
/// GitHub's release-asset chain is at most two hops (github.com →
/// objects.githubusercontent.com); five leaves slack and still ends a loop.
pub(crate) const MAX_REDIRECTS: u8 = 5;

/// Which redirects a download may follow and which hosts get the token.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DownloadPolicy {
    /// Hosts a redirect may lead to. `*.example.com` matches every subdomain
    /// of `example.com`, not `example.com` itself.
    pub(crate) redirect_hosts: &'static [&'static str],
    /// Hosts the bearer token is sent to, with the same pattern syntax. Any
    /// other host receives the request without it.
    pub(crate) bearer_hosts: &'static [&'static str],
    /// Whether a plain-`http` URL may be a redirect target or carry the
    /// token. Only test doubles, which serve loopback over `http`, allow it.
    pub(crate) allow_plain_http: bool,
}

impl DownloadPolicy {
    /// The policy of every real install: GitHub and its object store, over
    /// `https`, with the token kept to GitHub's own hosts.
    pub(crate) const GITHUB: DownloadPolicy = DownloadPolicy {
        redirect_hosts: &["github.com", "api.github.com", "*.githubusercontent.com"],
        bearer_hosts: &["github.com", "api.github.com"],
        allow_plain_http: false,
    };

    /// Whether a redirect to `url` may be followed; the error names why not.
    fn admits_redirect_to(&self, url: &Url) -> Result<(), String> {
        let host = url.host_str().unwrap_or("");
        if !matches_any(self.redirect_hosts, host) {
            return Err(format!("DisallowedRedirectHost {host}"));
        }
        if !self.admits_scheme(url) {
            return Err(format!("InsecureRedirect {}://{host}", url.scheme()));
        }
        Ok(())
    }

    /// Whether the bearer token goes with the request to `url`.
    fn sends_bearer_to(&self, url: &Url) -> bool {
        self.admits_scheme(url) && matches_any(self.bearer_hosts, url.host_str().unwrap_or(""))
    }

    fn admits_scheme(&self, url: &Url) -> bool {
        match url.scheme() {
            "https" => true,
            "http" => self.allow_plain_http,
            _ => false,
        }
    }
}

fn matches_any(patterns: &[&str], host: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| match pattern.strip_prefix('*') {
            Some(suffix) => host.ends_with(suffix),
            None => host == *pattern,
        })
}

/// The client a download goes through. It follows no redirect by itself:
/// [`get`] does, so that every hop passes through the policy.
pub(crate) fn client(timeout: Duration) -> Result<Client, InstallerError> {
    Client::builder()
        .timeout(timeout)
        .user_agent(concat!("cogh/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| InstallerError::Network("reqwest".into(), e.to_string()))
}

/// GET `url`, following redirects under `policy`, and return the final
/// response. The caller checks its status and reads the body.
///
/// The first URL is the one the caller chose (the canonical release URL, or
/// the mirror an operator configured), so only the hops that follow it are
/// held to the policy's hosts and scheme. The token goes only to the hops
/// `policy` names as its audience, the first one included.
pub(crate) fn get(
    client: &Client,
    url: &str,
    bearer: Option<&str>,
    policy: &DownloadPolicy,
) -> Result<Response, InstallerError> {
    let mut current = Url::parse(url)
        .map_err(|e| InstallerError::Network(shown_raw(url), format!("invalid URL: {e}")))?;

    for _ in 0..=MAX_REDIRECTS {
        let mut request = client.get(current.clone());
        if let Some(token) = bearer.filter(|_| policy.sends_bearer_to(&current)) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|e| InstallerError::Network(shown(&current), e.without_url().to_string()))?;

        if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                InstallerError::Network(
                    shown(&current),
                    "redirect status with no Location header".to_string(),
                )
            })?;
        let next = current.join(location).map_err(|e| {
            InstallerError::Network(shown(&current), format!("invalid redirect Location: {e}"))
        })?;
        policy
            .admits_redirect_to(&next)
            .map_err(|reason| InstallerError::Network(shown(&current), reason))?;
        current = next;
    }

    Err(InstallerError::Network(
        shown(&current),
        format!("TooManyRedirects (>{MAX_REDIRECTS})"),
    ))
}

/// A URL as an error may show it: without the query, which on a signed
/// object-store URL is the signature itself, and without any credentials
/// written into it.
fn shown(url: &Url) -> String {
    let mut shown = url.clone();
    shown.set_query(None);
    shown.set_fragment(None);
    let _ = shown.set_username("");
    let _ = shown.set_password(None);
    shown.to_string()
}

/// [`shown`] for a URL that did not parse: the same query, fragment and
/// userinfo are cut from the text itself.
fn shown_raw(raw: &str) -> String {
    let before_query = raw.split(['?', '#']).next().unwrap_or("");
    let (scheme, rest) = match before_query.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, before_query),
    };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let (authority, path) = rest.split_at(authority_end);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match scheme {
        Some(scheme) => format!("{scheme}://{host}{path}"),
        None => format!("{host}{path}"),
    }
}

/// A plain HTTP/1.1 server on loopback for the tests of every download path.
#[cfg(test)]
pub(crate) mod loopback {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    /// Answers one connection per reply, in order, then stops listening, so
    /// a client that asks once more than the test expects gets an error, not
    /// a hang. Each request head is sent on the channel BEFORE its reply is
    /// written: once the client holds a response the request is already in
    /// the channel, and an empty channel means the server was never asked.
    pub(crate) fn serve(replies: Vec<String>) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
        let port = listener.local_addr().expect("local_addr").port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for reply in replies {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let _ = tx.send(read_head(&mut stream));
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        (port, rx)
    }

    /// The request up to the blank line that ends its head; a client that
    /// stops sending ends the read after the deadline instead of hanging it.
    fn read_head(stream: &mut std::net::TcpStream) -> String {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut head = Vec::new();
        let mut buf = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => head.extend_from_slice(&buf[..n]),
            }
        }
        String::from_utf8_lossy(&head).to_string()
    }

    pub(crate) fn ok_with(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// The value of the request's `Authorization` header, if it has one.
    pub(crate) fn authorization(request: &str) -> Option<String> {
        request.split("\r\n").find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::loopback::{authorization, ok_with, serve};
    use super::*;

    use std::sync::mpsc;

    /// Loopback stands in for GitHub: the mock servers are trusted hosts, and
    /// `127.0.0.1` plays the token's audience while `localhost` plays the
    /// object store.
    const LOOPBACK: DownloadPolicy = DownloadPolicy {
        redirect_hosts: &["127.0.0.1", "localhost"],
        bearer_hosts: &["127.0.0.1"],
        allow_plain_http: true,
    };

    /// The same hosts, held to the production rule on scheme.
    const LOOPBACK_HTTPS_ONLY: DownloadPolicy = DownloadPolicy {
        allow_plain_http: false,
        ..LOOPBACK
    };

    /// Only `127.0.0.1` is trusted: `localhost` is a host the policy refuses.
    const IPV4_LOOPBACK_ONLY: DownloadPolicy = DownloadPolicy {
        redirect_hosts: &["127.0.0.1"],
        ..LOOPBACK
    };

    fn redirect(location: &str) -> String {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    }

    fn ok() -> String {
        ok_with("")
    }

    fn redirecting_to(location: &str) -> (u16, mpsc::Receiver<String>) {
        serve(vec![redirect(location)])
    }

    fn answering_ok() -> (u16, mpsc::Receiver<String>) {
        serve(vec![ok()])
    }

    fn test_client() -> Client {
        client(Duration::from_secs(5)).expect("build test client")
    }

    fn url(raw: &str) -> Url {
        Url::parse(raw).expect("valid URL")
    }

    #[test]
    fn a_redirect_to_another_host_does_not_receive_the_bearer() {
        let (port_b, rx_b) = answering_ok();
        let (port_a, rx_a) = redirecting_to(&format!("http://localhost:{port_b}/asset"));

        let response = get(
            &test_client(),
            &format!("http://127.0.0.1:{port_a}/x"),
            Some("secret"),
            &LOOPBACK,
        )
        .expect("the redirect to a trusted host is followed");

        assert_eq!(response.status().as_u16(), 200);
        let to_a = rx_a.try_recv().expect("A was asked");
        assert_eq!(
            authorization(&to_a).as_deref(),
            Some("Bearer secret"),
            "the token's audience gets the token"
        );
        let to_b = rx_b.try_recv().expect("B was asked");
        assert_eq!(
            authorization(&to_b),
            None,
            "a host outside the token's audience never sees it: {to_b}"
        );
    }

    #[test]
    fn the_bearer_follows_a_redirect_that_stays_within_its_audience() {
        let (port_b, rx_b) = answering_ok();
        let (port_a, _rx_a) = redirecting_to(&format!("http://127.0.0.1:{port_b}/asset"));

        get(
            &test_client(),
            &format!("http://127.0.0.1:{port_a}/x"),
            Some("secret"),
            &LOOPBACK,
        )
        .expect("the redirect is followed");

        let to_b = rx_b.try_recv().expect("B was asked");
        assert_eq!(authorization(&to_b).as_deref(), Some("Bearer secret"));
    }

    #[test]
    fn a_relative_location_is_resolved_against_the_current_url() {
        let (port, rx) = serve(vec![redirect("/elsewhere"), ok()]);

        let response = get(
            &test_client(),
            &format!("http://127.0.0.1:{port}/x"),
            None,
            &IPV4_LOOPBACK_ONLY,
        )
        .expect("a relative Location stays on the same origin");

        assert_eq!(response.status().as_u16(), 200);
        let first = rx.try_recv().expect("the first request");
        let second = rx.try_recv().expect("the redirected request");
        assert!(first.starts_with("GET /x "), "{first}");
        assert!(second.starts_with("GET /elsewhere "), "{second}");
    }

    #[test]
    fn a_redirect_to_plain_http_is_refused() {
        let (port_b, rx_b) = answering_ok();
        let (port_a, _rx_a) = redirecting_to(&format!("http://127.0.0.1:{port_b}/asset"));

        let err = get(
            &test_client(),
            &format!("http://127.0.0.1:{port_a}/x"),
            Some("secret"),
            &LOOPBACK_HTTPS_ONLY,
        )
        .expect_err("a hop to http:// is not followed");

        assert!(
            err.to_string()
                .contains("InsecureRedirect http://127.0.0.1"),
            "{err}"
        );
        assert!(
            rx_b.try_recv().is_err(),
            "the http:// target was never asked"
        );
    }

    #[test]
    fn the_bearer_never_travels_over_plain_http() {
        let (port, rx) = answering_ok();

        get(
            &test_client(),
            &format!("http://127.0.0.1:{port}/x"),
            Some("secret"),
            &LOOPBACK_HTTPS_ONLY,
        )
        .expect("the first URL is the caller's choice and is fetched");

        let request = rx.try_recv().expect("the server was asked");
        assert_eq!(
            authorization(&request),
            None,
            "an audience host reached over http:// does not get the token: {request}"
        );
    }

    #[test]
    fn a_redirect_to_an_untrusted_host_is_refused() {
        let (port, _rx) = redirecting_to("https://attacker.example.com/asset");

        let err = get(
            &test_client(),
            &format!("http://127.0.0.1:{port}/x"),
            Some("secret"),
            &LOOPBACK,
        )
        .expect_err("attacker.example.com is not a trusted host");

        assert!(
            err.to_string()
                .contains("DisallowedRedirectHost attacker.example.com"),
            "{err}"
        );
    }

    #[test]
    fn a_userinfo_prefix_does_not_disguise_the_redirect_host() {
        let (port_b, rx_b) = answering_ok();
        let (port_a, _rx_a) = redirecting_to(&format!(
            "http://127.0.0.1:{port_b}@localhost:{port_b}/asset"
        ));

        let err = get(
            &test_client(),
            &format!("http://127.0.0.1:{port_a}/x"),
            Some("secret"),
            &IPV4_LOOPBACK_ONLY,
        )
        .expect_err("the host of that Location is localhost, not 127.0.0.1");

        assert!(
            err.to_string().contains("DisallowedRedirectHost localhost"),
            "{err}"
        );
        assert!(
            rx_b.try_recv().is_err(),
            "the disguised host was never asked"
        );
    }

    /// `hops` redirects in a row on one server, each to the next relative
    /// path, and then `last`. The server answers exactly that many requests.
    fn a_chain_of(hops: usize, last: Option<String>) -> (u16, mpsc::Receiver<String>) {
        let mut replies: Vec<String> = (1..=hops).map(|n| redirect(&format!("/hop{n}"))).collect();
        replies.extend(last);
        serve(replies)
    }

    #[test]
    fn a_download_follows_as_many_redirects_as_the_limit() {
        let hops = usize::from(MAX_REDIRECTS);
        let (port, rx) = a_chain_of(hops, Some(ok()));

        let response = get(
            &test_client(),
            &format!("http://127.0.0.1:{port}/start"),
            None,
            &LOOPBACK,
        )
        .expect("the limit is a number of redirects that is still followed");

        assert_eq!(response.status().as_u16(), 200);
        let asked: Vec<String> = rx.try_iter().collect();
        assert_eq!(asked.len(), hops + 1, "the start and every hop: {asked:?}");
        assert!(
            asked[hops].starts_with(&format!("GET /hop{hops} ")),
            "a relative Location resolves against the hop that sent it: {}",
            asked[hops]
        );
    }

    #[test]
    fn one_redirect_past_the_limit_ends_the_download() {
        // The server answers only the redirects; a download that asked once
        // more would get a refused connection, not this error.
        let (port, rx) = a_chain_of(usize::from(MAX_REDIRECTS) + 1, None);

        let err = get(
            &test_client(),
            &format!("http://127.0.0.1:{port}/start"),
            Some("secret"),
            &LOOPBACK,
        )
        .expect_err("a redirect past the limit is not followed");

        assert!(err.to_string().contains("TooManyRedirects (>5)"), "{err}");
        assert_eq!(rx.try_iter().count(), usize::from(MAX_REDIRECTS) + 1);
    }

    #[test]
    fn a_url_that_does_not_parse_is_shown_without_its_secrets() {
        let err = get(
            &test_client(),
            "https://user:hunter2@exa mple.com/a.tar.gz?X-Amz-Signature=sig#frag",
            None,
            &LOOPBACK,
        )
        .expect_err("a host with a space does not parse");

        let shown = err.to_string();
        for secret in ["hunter2", "user:", "X-Amz-Signature", "sig", "frag"] {
            assert!(!shown.contains(secret), "{secret} leaked: {shown}");
        }
        assert!(shown.contains("https://exa mple.com/a.tar.gz"), "{shown}");
    }

    #[test]
    fn a_raw_url_keeps_an_at_sign_in_its_path() {
        assert_eq!(
            shown_raw("https://host/pkg/@scope/a?sig=1"),
            "https://host/pkg/@scope/a"
        );
        assert_eq!(shown_raw("host:8080/a#x"), "host:8080/a");
        assert_eq!(shown_raw("u@host"), "host");
    }

    #[test]
    fn production_redirects_reach_only_github_over_https() {
        for admitted in [
            "https://github.com/o/r/releases/download/v1/a.tar.gz",
            "https://objects.githubusercontent.com/x",
            "https://release-assets.githubusercontent.com/x",
        ] {
            assert!(
                DownloadPolicy::GITHUB
                    .admits_redirect_to(&url(admitted))
                    .is_ok(),
                "{admitted} is part of GitHub's release chain"
            );
        }
        for refused in [
            "https://127.0.0.1/x",
            "https://localhost/x",
            "https://someone.github.io/x",
            "https://evilgithubusercontent.com/x",
            "https://githubusercontent.com.evil.example/x",
            "http://github.com/x",
            "http://objects.githubusercontent.com/x",
        ] {
            assert!(
                DownloadPolicy::GITHUB
                    .admits_redirect_to(&url(refused))
                    .is_err(),
                "{refused} is not a production redirect target"
            );
        }
    }

    #[test]
    fn production_sends_the_token_only_to_github_over_https() {
        for audience in ["https://github.com/x", "https://api.github.com/x"] {
            assert!(
                DownloadPolicy::GITHUB.sends_bearer_to(&url(audience)),
                "{audience}"
            );
        }
        for outsider in [
            "https://objects.githubusercontent.com/x?X-Amz-Signature=s",
            "https://raw.githubusercontent.com/x",
            "http://github.com/x",
            "https://127.0.0.1/x",
        ] {
            assert!(
                !DownloadPolicy::GITHUB.sends_bearer_to(&url(outsider)),
                "{outsider}"
            );
        }
    }

    #[test]
    fn an_error_does_not_show_the_signature_nor_the_credentials_of_a_url() {
        let shown = shown(&url(
            "https://user:pass@objects.githubusercontent.com/a?X-Amz-Signature=s#f",
        ));

        assert_eq!(shown, "https://objects.githubusercontent.com/a");
    }
}

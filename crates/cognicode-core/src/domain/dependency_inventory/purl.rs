//! Package URL (PURL) identity for Cargo packages.
//!
//! A PURL names a package: `pkg:cargo/serde@1.0.200`. It is an identity and
//! nothing else. It is not a digest of the bytes that were built and it is
//! not provenance: two artifacts with the same PURL can differ, and the same
//! source can be published under several names. The inventory therefore
//! carries the PURL next to the origin and the lockfile checksum and never
//! in place of them.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::source::PackageSource;

/// The PURL type of Cargo packages.
const TYPE: &str = "cargo";

/// Percent-encodes everything outside the unreserved set plus `/` and `:`,
/// the characters a PURL component or qualifier value keeps verbatim here.
fn encode(text: &str, keep: &[char]) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let character = char::from(byte);
        if character.is_ascii_alphanumeric()
            || matches!(character, '-' | '.' | '_' | '~')
            || keep.contains(&character)
        {
            out.push(character);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The PURL of one resolved package, when its source has a distributable
/// identity.
///
/// * A package from the default registry (crates.io) is
///   `pkg:cargo/<name>@<version>`.
/// * A package from another registry adds `repository_url=<index url>`.
/// * A Git package adds `vcs_url=git+<url>@<commit>`, the commit being the
///   one the lockfile pins.
/// * A path package has no PURL: its identity is a location in a tree, not
///   a published name, and inventing one would claim a registry
///   provenance it does not have.
/// * A source in a grammar this crate does not read has no PURL either.
pub fn cargo_purl(name: &str, version: &str, source: &PackageSource) -> Option<String> {
    let base = format!("pkg:{TYPE}/{}@{}", encode(name, &[]), encode(version, &[]));
    match source {
        PackageSource::Registry { .. } if source.is_default_registry() => Some(base),
        PackageSource::Registry { url } => Some(format!(
            "{base}?repository_url={}",
            encode(index_url(url), &['/', ':'])
        )),
        PackageSource::Git { url, commit, .. } => Some(format!(
            "{base}?vcs_url={}",
            encode(&format!("git+{url}@{commit}"), &['/', ':'])
        )),
        PackageSource::Path | PackageSource::Unknown { .. } => None,
    }
}

/// The registry index URL without the lockfile's protocol prefix.
fn index_url(url: &str) -> &str {
    url.strip_prefix("registry+")
        .or_else(|| url.strip_prefix("sparse+"))
        .unwrap_or(url)
}

/// A Package URL read from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPurl {
    /// The package type, lowercased (`cargo`, `npm`, `deb`).
    pub purl_type: String,
    /// The package name, percent-decoded.
    pub name: String,
    /// The version, percent-decoded, when the PURL carries one.
    pub version: Option<String>,
    /// The qualifiers, percent-decoded, with lowercased keys.
    pub qualifiers: BTreeMap<String, String>,
}

/// Why a text is not a Package URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurlError {
    /// The text does not start with `pkg:`.
    NotAPurl,
    /// The type or the name is missing.
    Incomplete,
    /// A percent escape is not valid.
    BadEscape,
}

fn decode(text: &str) -> Result<String, PurlError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .and_then(|pair| std::str::from_utf8(pair).ok())
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(PurlError::BadEscape)?;
            out.push(pair);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| PurlError::BadEscape)
}

/// Reads a Package URL. The reader is lenient about namespaces and subpaths
/// (it keeps neither) and strict about the escapes it decodes.
pub fn parse_purl(raw: &str) -> Result<ParsedPurl, PurlError> {
    let rest = raw
        .trim()
        .strip_prefix("pkg:")
        .ok_or(PurlError::NotAPurl)?
        .trim_start_matches('/');
    let rest = rest.split('#').next().unwrap_or_default();
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (path, version) = match path.rsplit_once('@') {
        Some((path, version)) => (path, Some(decode(version)?)),
        None => (path, None),
    };
    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let purl_type = segments.next().ok_or(PurlError::Incomplete)?;
    let name = segments.next_back().ok_or(PurlError::Incomplete)?;
    let mut qualifiers = BTreeMap::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        qualifiers.insert(key.to_ascii_lowercase(), decode(value)?);
    }
    Ok(ParsedPurl {
        purl_type: purl_type.to_ascii_lowercase(),
        name: decode(name)?,
        version,
        qualifiers,
    })
}

/// Where a Package URL says a Cargo package comes from.
///
/// A PURL qualifier is a claim by whoever wrote it. `Unspecified` means the
/// PURL names no source at all, which is not the same as the default
/// registry: producers omit the qualifier for packages that are not from
/// crates.io.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PurlOrigin {
    /// No source qualifier.
    Unspecified,
    /// `repository_url`: an explicit registry index.
    Registry {
        /// The index URL as the PURL writes it.
        url: String,
    },
    /// `vcs_url=git+<url>@<commit>`, or the `source=git&url=..&rev=..` form
    /// some exporters write.
    Git {
        /// The repository URL, without the `git+` prefix.
        url: String,
        /// The commit or revision the PURL pins, when it says one.
        commit: Option<String>,
    },
    /// `download_url=file://...`: a package built from the local tree.
    Local,
}

fn looks_like_commit(text: &str) -> bool {
    (7..=64).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The origin a Cargo PURL names through its qualifiers.
pub fn purl_origin(purl: &ParsedPurl) -> PurlOrigin {
    let qualifier = |key: &str| purl.qualifiers.get(key).map(String::as_str);
    if let Some(vcs) = qualifier("vcs_url") {
        let vcs = vcs.strip_prefix("git+").unwrap_or(vcs);
        return match vcs.rsplit_once('@') {
            Some((url, commit)) if looks_like_commit(commit) => PurlOrigin::Git {
                url: url.to_string(),
                commit: Some(commit.to_string()),
            },
            _ => PurlOrigin::Git {
                url: vcs.to_string(),
                commit: None,
            },
        };
    }
    if qualifier("source") == Some("git")
        && let Some(url) = qualifier("url")
    {
        let commit = qualifier("rev")
            .filter(|rev| looks_like_commit(rev))
            .map(str::to_string);
        return PurlOrigin::Git {
            url: url.trim_start_matches("git+").to_string(),
            commit,
        };
    }
    if let Some(url) = qualifier("repository_url") {
        return PurlOrigin::Registry {
            url: url.to_string(),
        };
    }
    if qualifier("download_url").is_some_and(|url| url.starts_with("file:")) {
        return PurlOrigin::Local;
    }
    PurlOrigin::Unspecified
}

#[cfg(test)]
mod tests {
    use super::super::source::GitReference;
    use super::*;

    fn registry(url: &str) -> PackageSource {
        PackageSource::Registry {
            url: url.to_string(),
        }
    }

    #[test]
    fn a_crates_io_package_is_name_and_version_only() {
        let source = registry("registry+https://github.com/rust-lang/crates.io-index");
        assert_eq!(
            cargo_purl("serde", "1.0.200", &source).as_deref(),
            Some("pkg:cargo/serde@1.0.200")
        );
        let sparse = registry("sparse+https://index.crates.io/");
        assert_eq!(
            cargo_purl("serde", "1.0.200", &sparse).as_deref(),
            Some("pkg:cargo/serde@1.0.200")
        );
    }

    #[test]
    fn build_metadata_in_a_version_is_percent_encoded() {
        let source = registry("sparse+https://index.crates.io/");
        assert_eq!(
            cargo_purl("openssl-src", "300.2.3+3.2.1", &source).as_deref(),
            Some("pkg:cargo/openssl-src@300.2.3%2B3.2.1")
        );
    }

    #[test]
    fn another_registry_names_its_index_as_a_qualifier() {
        let source = registry("sparse+https://registry.example.test/index/");
        assert_eq!(
            cargo_purl("internal", "0.4.0", &source).as_deref(),
            Some("pkg:cargo/internal@0.4.0?repository_url=https://registry.example.test/index/")
        );
    }

    #[test]
    fn a_git_package_pins_the_commit_in_a_vcs_url() {
        let source = PackageSource::Git {
            url: "https://github.com/example/crate".to_string(),
            reference: Some(GitReference::Tag("v1.0.0".to_string())),
            commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
        };
        assert_eq!(
            cargo_purl("crate", "1.0.0", &source).as_deref(),
            Some(
                "pkg:cargo/crate@1.0.0?vcs_url=git%2Bhttps://github.com/example/crate%400123456789abcdef0123456789abcdef01234567"
            )
        );
    }

    #[test]
    fn a_path_package_has_no_purl_because_it_has_no_published_identity() {
        assert_eq!(cargo_purl("local", "0.1.0", &PackageSource::Path), None);
        let unknown = PackageSource::Unknown {
            raw: "weird+scheme".to_string(),
        };
        assert_eq!(cargo_purl("odd", "0.1.0", &unknown), None);
    }

    #[test]
    fn a_purl_is_read_into_type_name_version_and_qualifiers() {
        let parsed =
            parse_purl("pkg:cargo/serde@1.0.5?repository_url=https%3A%2F%2Fr.example%2Findex")
                .expect("valid");

        assert_eq!(parsed.purl_type, "cargo");
        assert_eq!(parsed.name, "serde");
        assert_eq!(parsed.version.as_deref(), Some("1.0.5"));
        assert_eq!(
            parsed.qualifiers.get("repository_url").map(String::as_str),
            Some("https://r.example/index")
        );
    }

    #[test]
    fn a_namespace_is_dropped_and_an_encoded_version_is_decoded() {
        let parsed = parse_purl("pkg:cargo/ns/openssl-src@300.2.3%2B3.2.1").expect("valid");

        assert_eq!(parsed.name, "openssl-src");
        assert_eq!(parsed.version.as_deref(), Some("300.2.3+3.2.1"));
    }

    #[test]
    fn text_that_is_not_a_purl_is_refused_with_the_reason() {
        assert_eq!(parse_purl("cargo/serde@1"), Err(PurlError::NotAPurl));
        assert_eq!(parse_purl("pkg:cargo"), Err(PurlError::Incomplete));
        assert_eq!(parse_purl("pkg:cargo/a@1%zz"), Err(PurlError::BadEscape));
    }

    fn origin_of(raw: &str) -> PurlOrigin {
        purl_origin(&parse_purl(raw).expect("valid"))
    }

    #[test]
    fn a_purl_with_no_source_qualifier_names_no_origin() {
        assert_eq!(origin_of("pkg:cargo/serde@1.0.5"), PurlOrigin::Unspecified);
    }

    #[test]
    fn a_repository_url_names_a_registry() {
        assert_eq!(
            origin_of("pkg:cargo/a@1?repository_url=https://r.example/index"),
            PurlOrigin::Registry {
                url: "https://r.example/index".to_string()
            }
        );
    }

    #[test]
    fn a_vcs_url_names_a_repository_and_its_commit() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            origin_of(&format!(
                "pkg:cargo/a@1?vcs_url=git%2Bhttps://g.example/a.git%40{commit}"
            )),
            PurlOrigin::Git {
                url: "https://g.example/a.git".to_string(),
                commit: Some(commit.to_string())
            }
        );
        assert_eq!(
            origin_of("pkg:cargo/a@1?vcs_url=git%2Bhttps://g.example/a.git"),
            PurlOrigin::Git {
                url: "https://g.example/a.git".to_string(),
                commit: None
            }
        );
    }

    #[test]
    fn the_guard_git_form_names_a_commit_only_when_it_looks_like_one() {
        assert_eq!(
            origin_of("pkg:cargo/a@1?source=git&url=https://g.example/a&rev=0123456"),
            PurlOrigin::Git {
                url: "https://g.example/a".to_string(),
                commit: Some("0123456".to_string())
            }
        );
        assert_eq!(
            origin_of("pkg:cargo/a@1?source=git&url=https://g.example/a&rev=main"),
            PurlOrigin::Git {
                url: "https://g.example/a".to_string(),
                commit: None
            }
        );
    }

    #[test]
    fn a_file_download_url_names_a_local_origin() {
        assert_eq!(
            origin_of("pkg:cargo/a@1?download_url=file://."),
            PurlOrigin::Local
        );
    }
}

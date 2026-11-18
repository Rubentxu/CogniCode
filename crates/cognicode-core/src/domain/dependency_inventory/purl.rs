//! Package URL (PURL) identity for Cargo packages.
//!
//! A PURL names a package: `pkg:cargo/serde@1.0.200`. It is an identity and
//! nothing else. It is not a digest of the bytes that were built and it is
//! not provenance: two artifacts with the same PURL can differ, and the same
//! source can be published under several names. The inventory therefore
//! carries the PURL next to the origin and the lockfile checksum and never
//! in place of them.

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
}

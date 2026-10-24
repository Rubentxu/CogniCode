//! Every identity the product writes must be an identity the product reads
//! back.
//!
//! Measured: a symbol names itself `"{file}:{name}:{line}"`, and the readers
//! of infrastructure files name what they find with their own grammar — a
//! Terraform resource behind `tf:`, an Ansible declaration behind `ansible:`.
//! Those names are identities the product publishes: a resource and a play
//! are nodes of the graph, and the graph calls them by that name. The one
//! parser of the identity grammar knew only the symbol shape, so every
//! consumer that met the other shapes answered with something else: a piece
//! of the identity (the file the thing lives in), a fabricated position, or
//! nothing at all. A product that cannot name its own declarations cannot
//! answer a question about them.
//!
//! The law this file pins:
//!
//! - the identities the product writes are read by the reader in the domain,
//!   and
//! - that reader says the name and the file the identity carries.
//!
//! The identities are not written by hand here: they are the ones the
//! product's own readers produce over a corpus, so the property cannot pass
//! on a shape the product stopped writing.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::domain::evidence_kernel::identity::Identity;
use cognicode_core::domain::value_objects::{Language, NodeKind, SymbolKind};
use cognicode_core::infrastructure::parser::language_config::LanguageConfig;

/// One file of the corpus, and the identity the product writes for the thing
/// that file declares.
struct Case {
    language: Language,
    file: &'static str,
    /// The identity the product writes for the declared thing.
    identity: &'static str,
    /// The file the reader must say the thing lives in.
    lives_in: &'static str,
    /// The name the reader must say the thing carries.
    called: &'static str,
}

const CASES: &[Case] = &[
    Case {
        language: Language::Rust,
        file: "src/lib.rs",
        identity: "src/lib.rs:helper:1",
        lives_in: "src/lib.rs",
        called: "helper",
    },
    Case {
        language: Language::Hcl,
        file: "main.tf",
        identity: "tf:main.tf:aws_s3_bucket.app_bucket",
        lives_in: "main.tf",
        called: "aws_s3_bucket.app_bucket",
    },
    Case {
        language: Language::Yaml,
        file: "site.yml",
        identity: "ansible:site.yml:play:Configure_infrastructure",
        lives_in: "site.yml",
        called: "Configure_infrastructure",
    },
];

/// The corpus: one file per grammar the product writes.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/cognicode-core/testdata/identity_corpus")
}

/// The identities the product writes for one file of the corpus.
///
/// The path handed to the readers is the repo-relative one, the way the
/// product names a file: an identity carrying an absolute path would be a
/// different identity.
fn written_identities(language: Language, file: &str) -> Vec<String> {
    let source = std::fs::read_to_string(corpus().join(file)).expect("the corpus file is readable");
    let config = LanguageConfig::for_language(language);
    // The reading already went through the reader of the language, so the
    // identities here are the ones the product writes.
    let enriched = extract_file(config, std::path::Path::new(file), &source, "content-hash");
    enriched
        .nodes
        .iter()
        // The file itself is named by its path, not by an identity.
        .filter(|node| node.kind != NodeKind::Symbol(SymbolKind::File))
        .map(|node| node.id.as_str().to_string())
        .collect()
}

#[test]
fn every_identity_the_product_writes_is_read_back() {
    for case in CASES {
        let identities = written_identities(case.language, case.file);
        assert!(
            !identities.is_empty(),
            "{} must declare something, or the property would pass on nothing",
            case.file
        );
        for identity in &identities {
            assert!(
                Identity::parse(identity).is_some(),
                "the product writes the identity {identity:?} for {} and the reader \
                 does not read it back",
                case.file
            );
        }
    }
}

#[test]
fn the_reader_says_the_name_and_the_file_the_identity_carries() {
    for case in CASES {
        let identities = written_identities(case.language, case.file);
        assert!(
            identities.iter().any(|identity| identity == case.identity),
            "the product must write the identity {:?} for {}, got {:?}",
            case.identity,
            case.file,
            identities
        );

        let read = Identity::parse(case.identity)
            .unwrap_or_else(|| panic!("the reader must read {:?}", case.identity));
        assert_eq!(
            read.file(),
            case.lives_in,
            "the reader must say which file {:?} lives in",
            case.identity
        );
        assert_eq!(
            read.name(),
            case.called,
            "the reader must say what {:?} is called",
            case.identity
        );
    }
}

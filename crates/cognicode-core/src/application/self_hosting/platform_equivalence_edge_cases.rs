//! Edge case acceptance tests for platform equivalence (e76 WU4).
//!
//! Probes likely failure modes that could invalidate the
//! equivalence verdict if the normaliser regressed:
//!
//! - UTF-8 BOM at file start (some Windows tools emit one).
//! - Mixed CRLF/LF within a single buffer.
//! - Empty raw payload (should be Equivalent trivially).
//! - Single platform only (no divergence possible).

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::platform_equivalence::{
    DefaultNormaliser, Equivalence, PlatformKind, PlatformObservation, compare_platforms,
    default_canonicalize,
};

#[test]
fn acceptance_default_canonicalize_preserves_utf8_bom() {
    // The UTF-8 BOM (0xEF 0xBB 0xBF) is not a platform mechanic
    // (both Linux and Windows tools can produce files with it).
    // The default normaliser should pass it through unchanged.
    let with_bom: &[u8] = &[0xEF, 0xBB, 0xBF, b'a', b'\n', b'b', b'\n'];
    let canonical = default_canonicalize(with_bom);
    assert_eq!(canonical, vec![0xEF, 0xBB, 0xBF, b'a', b'\n', b'b', b'\n']);
}

#[test]
fn acceptance_default_canonicalize_handles_mixed_line_endings() {
    // Some legacy files mix CRLF and LF; the normaliser MUST
    // canonicalise both consistently.
    let mixed = b"line1\r\nline2\nline3\r\nline4\n";
    let canonical = default_canonicalize(mixed);
    assert_eq!(canonical, b"line1\nline2\nline3\nline4\n");
}

#[test]
fn acceptance_compare_platforms_with_empty_payload_is_equivalent() {
    // Empty raw bytes on all platforms must produce Equivalent
    // (both canonicalise to empty).
    let n = DefaultNormaliser;
    let observations = vec![
        PlatformObservation {
            platform: PlatformKind::Linux,
            id: "empty.fact".into(),
            raw: Vec::new(),
        },
        PlatformObservation {
            platform: PlatformKind::Windows,
            id: "empty.fact".into(),
            raw: Vec::new(),
        },
    ];
    let verdicts = compare_platforms(&n, &observations);
    assert_eq!(verdicts.get("empty.fact"), Some(&Equivalence::Equivalent));
}

#[test]
fn acceptance_compare_platforms_with_single_platform_returns_equivalent() {
    // Only one platform observed → no divergence possible.
    let n = DefaultNormaliser;
    let observations = vec![PlatformObservation {
        platform: PlatformKind::MacOs,
        id: "single.fact".into(),
        raw: b"any content\n".to_vec(),
    }];
    let verdicts = compare_platforms(&n, &observations);
    assert_eq!(
        verdicts.get("single.fact"),
        Some(&Equivalence::Equivalent),
        "a single-platform observation cannot diverge"
    );
}

#[test]
fn acceptance_compare_platforms_surfaces_divergence_even_when_only_two_platforms_differ() {
    // Three platforms; one differs from the others. The divergent
    // platform set must include exactly that one.
    let n = DefaultNormaliser;
    let observations = vec![
        PlatformObservation {
            platform: PlatformKind::Linux,
            id: "two_of_three.fact".into(),
            raw: b"the agreed value\n".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::MacOs,
            id: "two_of_three.fact".into(),
            raw: b"the agreed value\n".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::Windows,
            id: "two_of_three.fact".into(),
            raw: b"a different value\n".to_vec(),
        },
    ];
    let verdicts = compare_platforms(&n, &observations);
    match verdicts.get("two_of_three.fact") {
        Some(Equivalence::Divergent { platforms }) => {
            assert_eq!(platforms.len(), 2);
            assert!(platforms.contains(&PlatformKind::Linux));
            assert!(platforms.contains(&PlatformKind::Windows));
            assert!(
                !platforms.contains(&PlatformKind::MacOs),
                "macOS agreed with linux, must NOT be flagged as divergent"
            );
        }
        other => panic!("expected Divergent, got {:?}", other),
    }
}

//! How a language says that a function is a test — and how that test is run.
//!
//! There are two questions here, and they have two different answers, so
//! they live in two different places:
//!
//! - **Is this symbol a test?** The answer belongs to the language, and it
//!   is not always the same kind of answer. Rust, Java, C# and Kotlin MARK
//!   their tests (`#[test]`, `@Test`, `[Fact]`); the reader that saw the
//!   mark is the only one that can say, so the mark is what travels.
//!   Python, Go and JavaScript/TypeScript NAME their tests: a file that is
//!   a file of tests, and a name that is a name of a test. There the
//!   convention is the language's, and applying it is the reader's job.
//!
//! - **How is this test run?** The answer belongs to the language too, but
//!   not every language decides it alone: a Java test is run by Maven or
//!   by Gradle, and the language does not say which. Where the language
//!   does decide, the command is exact; where it does not, there is no
//!   command and the answer says so rather than inventing one.
//!
//! The table lives in the domain because it IS the language's rule, and
//! the reader consults it: the reader knows the language and the mark it
//! saw, and the domain knows what the mark means. Nothing here reads a
//! file or a tree; the tree is the reader's, and the mark it carries is
//! the reader's to find.
//!
//! # What the graph cannot say
//!
//! The command for a Rust test that lives in `src/` filters by the test's
//! NAME, not by its full path: the graph carries the file and the name,
//! and the chain of modules inside the file is not in it. The filter is
//! the runner's (a substring match), and it can select more than one test
//! when two tests share a name. For a test that lives in `tests/`, the
//! module path IS derivable — it is the path of the file under the
//! target — so the filter there is exact.

use crate::domain::value_objects::Language;

/// How a language declares the tests it holds.
#[derive(Debug, Clone, Copy)]
pub enum TestDeclaration {
    /// The language marks the test, and the mark is what the reader saw.
    ///
    /// The names are the marks that declare a test, in their last segment:
    /// `#[test]`, `#[tokio::test]` and `#[actix_web::test]` all end in
    /// `test`, and one name covers them. A mark that does NOT declare a
    /// test is not in the list: `#[cfg(test)]` says that a block is only
    /// compiled for tests, not that the function inside it is one.
    Attribute(&'static [&'static str]),

    /// The language names its tests.
    ///
    /// `file` says whether a path is a file of tests. `name` says whether
    /// a name is the name of a test; `None` means the language declares the
    /// test by the file it lives in, which is the case of the languages
    /// whose test names are strings and not symbols.
    Convention {
        /// Whether the file is a file of tests.
        file: fn(&str) -> bool,
        /// Whether the name is the name of a test.
        name: Option<fn(&str) -> bool>,
    },
}

/// The declaration of `language`, or `None` when the language says nothing
/// about its tests.
///
/// A language that is not here has no rule, and a symbol of that language
/// is not a test: the answer is "no", not "probably".
pub fn declaration_for(language: Language) -> Option<TestDeclaration> {
    match language {
        Language::Rust => Some(TestDeclaration::Attribute(&[
            "test",
            "rstest",
            "test_case",
            "wasm_bindgen_test",
        ])),
        Language::Java => Some(TestDeclaration::Attribute(&[
            "Test",
            "ParameterizedTest",
            "RepeatedTest",
            "TestFactory",
            "TestTemplate",
        ])),
        Language::CSharp => Some(TestDeclaration::Attribute(&["Fact", "Theory"])),
        Language::Kotlin => Some(TestDeclaration::Attribute(&["Test"])),
        Language::Python => Some(TestDeclaration::Convention {
            file: python_test_file,
            name: Some(python_test_name),
        }),
        Language::Go => Some(TestDeclaration::Convention {
            file: go_test_file,
            name: Some(go_test_name),
        }),
        Language::JavaScript | Language::TypeScript => Some(TestDeclaration::Convention {
            file: javascript_test_file,
            name: None,
        }),
        _ => None,
    }
}

/// Whether the symbol `name` of `file` is a test of `language`.
///
/// `mark` is the mark the reader saw, and only the reader saw it: the
/// name of the attribute, without its delimiters (`test` for `#[test]`,
/// `Test` for `@Test`, `Fact` for `[Fact]`). A language that declares by
/// convention does not need it, and a language that declares by attribute
/// cannot answer without it.
pub fn is_test(language: Language, file: &str, name: &str, mark: Option<&str>) -> bool {
    match declaration_for(language) {
        Some(TestDeclaration::Attribute(marks)) => match mark {
            Some(mark) => marks.iter().any(|declared| last_segment(mark) == *declared),
            None => false,
        },
        Some(TestDeclaration::Convention {
            file: is_test_file,
            name: is_test_name,
        }) => is_test_file(file) && is_test_name.is_none_or(|rule| rule(name)),
        None => false,
    }
}

/// The exact command that runs the test `name` of `file`, when the
/// language's runner is decided by the language.
///
/// `None` means the language does not decide it: the answer then carries no
/// command, and the consumer knows it has to find the runner itself. An
/// invented command is worse than none.
pub fn command_for(language: Language, file: &str, name: &str) -> Option<String> {
    match language {
        Language::Rust => rust_command(file, name),
        Language::Python => Some(format!("pytest {file}::{name}")),
        Language::JavaScript | Language::TypeScript => {
            Some(format!("npx vitest run {file} -t '{name}'"))
        }
        Language::Go => {
            let package = parent_of(file);
            Some(format!("go test ./{package} -run '{name}'"))
        }
        _ => None,
    }
}

/// The last segment of a mark: the name after the last `::`.
///
/// A mark is a path in the languages that qualify it (`tokio::test`), and
/// the name that matters is the last one.
fn last_segment(mark: &str) -> &str {
    mark.rsplit("::").next().unwrap_or(mark)
}

/// The name of the file, without its directory.
fn file_name_of(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

/// The directory of the file, without the file.
fn parent_of(file: &str) -> &str {
    match file.rfind('/') {
        Some(at) => &file[..at],
        None => ".",
    }
}

/// Whether a path has a directory component with this name.
fn has_component(file: &str, component: &str) -> bool {
    file.split('/').any(|part| part == component)
}

/// A Python file of tests: `test_*.py`, `*_test.py`, or a file under a
/// `tests` directory.
fn python_test_file(file: &str) -> bool {
    let name = file_name_of(file);
    name.starts_with("test_") || name.ends_with("_test.py") || has_component(file, "tests")
}

/// A Python name of a test: `test_*`.
fn python_test_name(name: &str) -> bool {
    name.starts_with("test_")
}

/// A Go file of tests: `*_test.go`.
fn go_test_file(file: &str) -> bool {
    file_name_of(file).ends_with("_test.go")
}

/// A Go name of a test: `Test*`.
fn go_test_name(name: &str) -> bool {
    name.starts_with("Test")
}

/// A JavaScript or TypeScript file of tests: `*.test.*`, `*.spec.*`, or a
/// file under a `__tests__` directory.
fn javascript_test_file(file: &str) -> bool {
    let name = file_name_of(file);
    name.contains(".test.") || name.contains(".spec.") || has_component(file, "__tests__")
}

/// The command that runs a Rust test.
///
/// The package is the directory that holds `src` or `tests`, which is what
/// cargo says a package is; the target is the file under `tests`, or the
/// directory under `tests` that is a target of its own (`tests/it/`), or
/// the library when the test lives in `src/`.
fn rust_command(file: &str, name: &str) -> Option<String> {
    let parts: Vec<&str> = file.split('/').filter(|part| !part.is_empty()).collect();
    let root = parts
        .iter()
        .position(|part| *part == "src" || *part == "tests")?;
    if root == 0 {
        // The path does not say which package the file belongs to.
        return None;
    }
    let manifest = format!("{}/Cargo.toml", parts[..root].join("/"));

    match parts[root] {
        // A unit test: the binary is the library, and the chain of modules
        // inside the file is not in the graph, so the filter names the test.
        "src" => Some(format!(
            "cargo nextest run --manifest-path {manifest} --lib -E 'test({name})'"
        )),
        // An integration test: the target is the file, or the directory
        // that is a target of its own; the module path is the path of the
        // file under that target.
        "tests" => {
            let rest = &parts[root + 1..];
            let (target, module) = match rest {
                [only] => (only.trim_end_matches(".rs"), None),
                [target, under @ ..] => {
                    let path = under
                        .iter()
                        .map(|part| part.trim_end_matches(".rs"))
                        .collect::<Vec<_>>()
                        .join("::");
                    let path = (path != "main").then_some(path);
                    (*target, path)
                }
                _ => return None,
            };
            let selector = match module {
                Some(module) => format!("{module}::{name}"),
                None => name.to_string(),
            };
            Some(format!(
                "cargo nextest run --manifest-path {manifest} --test {target} \
                 -E 'test(={selector})'"
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_declares_its_tests_with_a_mark() {
        assert!(matches!(
            declaration_for(Language::Rust),
            Some(TestDeclaration::Attribute(_))
        ));
    }

    #[test]
    fn a_mark_that_declares_a_test_says_so() {
        assert!(is_test(Language::Rust, "a.rs", "whatever", Some("test")));
        assert!(is_test(
            Language::Rust,
            "a.rs",
            "whatever",
            Some("tokio::test")
        ));
        assert!(is_test(Language::Java, "A.java", "whatever", Some("Test")));
        assert!(is_test(Language::CSharp, "A.cs", "whatever", Some("Fact")));
    }

    #[test]
    fn a_mark_that_does_not_declare_a_test_does_not() {
        // `cfg(test)` says the block is only compiled for tests; it does
        // not say the function inside it is one.
        assert!(!is_test(Language::Rust, "a.rs", "whatever", Some("cfg")));
        assert!(!is_test(Language::Rust, "a.rs", "whatever", Some("allow")));
        assert!(!is_test(
            Language::Java,
            "A.java",
            "whatever",
            Some("Override")
        ));
    }

    #[test]
    fn a_language_that_marks_its_tests_answers_no_without_the_mark() {
        assert!(!is_test(Language::Rust, "a.rs", "whatever", None));
    }

    #[test]
    fn python_names_its_tests() {
        assert!(is_test(
            Language::Python,
            "tests/test_thing.py",
            "test_it",
            None
        ));
        assert!(!is_test(Language::Python, "src/thing.py", "test_it", None));
        assert!(!is_test(
            Language::Python,
            "tests/test_thing.py",
            "helper",
            None
        ));
    }

    #[test]
    fn go_names_its_tests() {
        assert!(is_test(
            Language::Go,
            "pkg/thing_test.go",
            "TestThing",
            None
        ));
        assert!(!is_test(Language::Go, "pkg/thing.go", "TestThing", None));
    }

    #[test]
    fn javascript_declares_by_the_file_it_lives_in() {
        assert!(is_test(
            Language::TypeScript,
            "src/thing.test.ts",
            "anything",
            None
        ));
        assert!(!is_test(
            Language::TypeScript,
            "src/thing.ts",
            "anything",
            None
        ));
    }

    #[test]
    fn a_language_without_a_rule_says_no() {
        assert!(!is_test(Language::Hcl, "main.tf", "anything", None));
    }

    #[test]
    fn the_rust_command_of_an_integration_test_is_exact() {
        assert_eq!(
            command_for(Language::Rust, "crates/demo/tests/it/checks.rs", "the_test").as_deref(),
            Some(
                "cargo nextest run --manifest-path crates/demo/Cargo.toml --test it \
                 -E 'test(=checks::the_test)'"
            )
        );
    }

    #[test]
    fn the_rust_command_of_a_test_that_is_a_target_of_its_own_is_exact() {
        assert_eq!(
            command_for(Language::Rust, "crates/demo/tests/checks.rs", "the_test").as_deref(),
            Some(
                "cargo nextest run --manifest-path crates/demo/Cargo.toml --test checks \
                 -E 'test(=the_test)'"
            )
        );
    }

    #[test]
    fn the_rust_command_of_a_unit_test_names_the_library() {
        assert_eq!(
            command_for(Language::Rust, "crates/demo/src/thing.rs", "the_test").as_deref(),
            Some(
                "cargo nextest run --manifest-path crates/demo/Cargo.toml --lib \
                 -E 'test(the_test)'"
            )
        );
    }

    #[test]
    fn a_path_that_does_not_say_its_package_has_no_command() {
        assert_eq!(
            command_for(Language::Rust, "src/thing.rs", "the_test"),
            None
        );
    }

    #[test]
    fn the_other_languages_have_their_own_commands() {
        assert_eq!(
            command_for(Language::Python, "tests/test_thing.py", "test_it").as_deref(),
            Some("pytest tests/test_thing.py::test_it")
        );
        assert_eq!(
            command_for(Language::TypeScript, "src/thing.test.ts", "it works").as_deref(),
            Some("npx vitest run src/thing.test.ts -t 'it works'")
        );
        assert_eq!(
            command_for(Language::Go, "pkg/thing_test.go", "TestThing").as_deref(),
            Some("go test ./pkg -run 'TestThing'")
        );
    }

    #[test]
    fn a_language_whose_runner_it_does_not_decide_has_no_command() {
        assert_eq!(command_for(Language::Java, "A.java", "testIt"), None);
        assert_eq!(command_for(Language::CSharp, "A.cs", "TestIt"), None);
    }
}

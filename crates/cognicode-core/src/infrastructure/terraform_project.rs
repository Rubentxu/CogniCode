//! Loads a Terraform project from disk into the IR, without running anything.
//!
//! Only text is read: `*.tf` files, the root `terraform.tfvars` and
//! `*.auto.tfvars`. No `terraform` binary, provider plugin, network access or
//! credential is involved, and module code is never executed: a module is a
//! directory of declarations, read the same way as the root.
//!
//! Local modules are followed only inside the project root. A source that is
//! remote, climbs out of the root or resolves (through a symlink) outside it
//! is not opened, and the call stays a hole in the graph. Whatever could not
//! be read is listed in [`TfProject::unread`], which makes the analysis
//! incomplete instead of silently smaller.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::iac::terraform::{
    Bindings, TfModule, TfProject, UnreadSource, constant, resolve_local_source,
};
use crate::infrastructure::parser::terraform_reader::{read_terraform_file, read_tfvars};
use crate::infrastructure::parser::tree::build_tree;

/// The most module directories one project load opens.
const MAX_MODULE_DIRS: usize = 256;

/// The largest file the loader reads.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// A project read from disk, with the inputs its variable files bind.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedTerraform {
    /// The declarations, and what could not be read.
    pub project: TfProject,
    /// The root variables bound by `terraform.tfvars` and `*.auto.tfvars`.
    pub bindings: Bindings,
}

/// Reads the Terraform project rooted at `root`.
///
/// # Errors
/// When `root` is not a readable directory.
pub fn load_terraform_project(root: &Path) -> std::io::Result<LoadedTerraform> {
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("{} is not a directory", root.display()),
        ));
    }
    let mut loader = Loader {
        root,
        project: TfProject::default(),
        seen: BTreeSet::new(),
    };
    loader.load_dir("");
    let bindings = loader.read_inputs();
    loader.project.unread.sort();
    Ok(LoadedTerraform {
        project: loader.project,
        bindings,
    })
}

/// Whether `name` is an override file, which Terraform merges into the
/// declarations it names instead of reading as a module of its own.
fn is_override(name: &str) -> bool {
    name == "override.tf" || name.ends_with("_override.tf")
}

struct Loader {
    root: PathBuf,
    project: TfProject,
    seen: BTreeSet<String>,
}

impl Loader {
    fn unread(&mut self, path: &str, reason: impl Into<String>) {
        self.project.unread.push(UnreadSource {
            path: path.to_string(),
            reason: reason.into(),
        });
    }

    /// The path of `name` inside the module directory `key`, relative to the
    /// root.
    fn relative(key: &str, name: &str) -> String {
        if key.is_empty() {
            name.to_string()
        } else {
            format!("{key}/{name}")
        }
    }

    /// Resolves a project-relative path on disk, and only if it stays inside
    /// the root once symlinks are followed.
    fn inside(&self, relative: &str) -> Option<PathBuf> {
        let path = fs::canonicalize(self.root.join(relative)).ok()?;
        path.starts_with(&self.root).then_some(path)
    }

    fn load_dir(&mut self, key: &str) {
        if !self.seen.insert(key.to_string()) {
            return;
        }
        if self.seen.len() > MAX_MODULE_DIRS {
            self.unread(
                key,
                format!("more than {MAX_MODULE_DIRS} module directories"),
            );
            return;
        }
        let Some(dir) = self.inside(key).filter(|d| d.is_dir()) else {
            self.unread(key, "not a directory inside the project root");
            return;
        };
        let names = match self.list_names(key, &dir, |name| {
            name.ends_with(".tf") || name.ends_with(".tf.json")
        }) {
            Ok(names) => names,
            Err(error) => {
                self.unread(key, format!("directory not readable: {error}"));
                return;
            }
        };
        let mut module = TfModule::default();
        let mut read_any = false;
        for name in names {
            let relative = Self::relative(key, &name);
            if name.ends_with(".tf.json") {
                self.unread(&relative, "JSON syntax is not read");
            } else if is_override(&name) {
                self.unread(
                    &relative,
                    "override file: Terraform merges it into the declarations it names, \
                     which is not modeled",
                );
            } else if name.ends_with(".tf") {
                read_any = true;
                if let Some(file_module) = self.read_file(&relative) {
                    module.absorb(file_module);
                }
            }
        }
        if !read_any {
            self.unread(key, "no .tf files");
            return;
        }
        self.list_duplicates(&module);
        let sources: Vec<String> = module.calls.values().map(|c| c.source.clone()).collect();
        self.project.modules.insert(key.to_string(), module);
        for source in sources {
            // A remote source, or a path out of the root, has no directory
            // here: the call stays unresolved.
            if let Some(child) = resolve_local_source(key, &source) {
                self.load_dir(&child);
            }
        }
    }

    /// The UTF-8 names in `dir`, sorted. A name that is not UTF-8 cannot be
    /// handed on as a path, so when it is one the loader would read (`wanted`
    /// decides, on its lossy rendering) it is listed as unread instead of
    /// being skipped.
    fn list_names(
        &mut self,
        key: &str,
        dir: &Path,
        wanted: impl Fn(&str) -> bool,
    ) -> std::io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(dir)?.filter_map(Result::ok) {
            match entry.file_name().into_string() {
                Ok(name) => names.push(name),
                Err(raw) => {
                    let lossy = raw.to_string_lossy().into_owned();
                    if wanted(&lossy) {
                        let relative = Self::relative(key, &lossy);
                        self.unread(&relative, "file name is not valid UTF-8");
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Lists, per file, the declarations that repeat one the module already
    /// has: Terraform rejects such a directory, and the repeat is not read.
    fn list_duplicates(&mut self, module: &TfModule) {
        let mut by_file: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for duplicate in &module.duplicates {
            by_file
                .entry(duplicate.file.as_str())
                .or_default()
                .push(format!("{} `{}`", duplicate.kind, duplicate.name));
        }
        for (file, repeated) in by_file {
            self.unread(
                file,
                format!(
                    "declared more than once: {}; the first declaration is kept",
                    repeated.join(", ")
                ),
            );
        }
    }

    fn read_text(&mut self, relative: &str) -> Option<String> {
        let Some(path) = self.inside(relative) else {
            self.unread(relative, "resolves outside the project root");
            return None;
        };
        match fs::metadata(&path) {
            Ok(meta) if meta.is_file() && meta.len() <= MAX_FILE_BYTES => {}
            Ok(meta) if meta.is_file() => {
                self.unread(relative, format!("larger than {MAX_FILE_BYTES} bytes"));
                return None;
            }
            _ => {
                self.unread(relative, "not a regular file");
                return None;
            }
        }
        match fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(error) => {
                self.unread(relative, format!("not readable as UTF-8 text: {error}"));
                None
            }
        }
    }

    fn read_file(&mut self, relative: &str) -> Option<TfModule> {
        let text = self.read_text(relative)?;
        let Some(tree) = build_tree(&tree_sitter_hcl::LANGUAGE.into(), &text) else {
            self.unread(relative, "the HCL parser produced no tree");
            return None;
        };
        match read_terraform_file(relative, &text, &tree) {
            Ok(module) => Some(module),
            Err(reason) => {
                self.unread(relative, reason);
                None
            }
        }
    }

    /// The inputs of the root module: `terraform.tfvars`, then the
    /// `*.auto.tfvars` in lexical order, later files overriding earlier ones.
    /// A value that is not a constant stays unbound, and says so.
    fn read_inputs(&mut self) -> Bindings {
        let root = self.root.clone();
        let is_input = |n: &str| n == "terraform.tfvars" || n.ends_with(".auto.tfvars");
        let mut names = self.list_names("", &root, is_input).unwrap_or_default();
        names.retain(|n| is_input(n));
        names.sort_by_key(|n| (n != "terraform.tfvars", n.clone()));
        let mut bindings = Bindings::none();
        for name in names {
            let Some(text) = self.read_text(&name) else {
                continue;
            };
            let Some(tree) = build_tree(&tree_sitter_hcl::LANGUAGE.into(), &text) else {
                self.unread(&name, "the HCL parser produced no tree");
                continue;
            };
            match read_tfvars(&name, &text, &tree) {
                Ok(attributes) => {
                    for attribute in attributes {
                        match constant(&attribute.expr) {
                            Some(value) => bindings = bindings.with(attribute.name, value),
                            None => self.unread(
                                &name,
                                format!("`{}` is not a constant value", attribute.name),
                            ),
                        }
                    }
                }
                Err(reason) => self.unread(&name, reason),
            }
        }
        bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::iac::terraform::{TfAnalysis, TfExpr, TfValue};

    fn write(root: &Path, path: &str, text: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }

    #[test]
    fn local_modules_are_followed_and_remote_ones_are_holes() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "main.tf",
            r#"
module "local" { source = "./modules/bucket" }
module "remote" { source = "terraform-aws-modules/vpc/aws" }
"#,
        );
        write(
            dir.path(),
            "modules/bucket/main.tf",
            r#"resource "aws_s3_bucket" "b" { acl = "private" }"#,
        );
        let loaded = load_terraform_project(dir.path()).unwrap();
        assert_eq!(
            loaded.project.modules.keys().collect::<Vec<_>>(),
            ["", "modules/bucket"]
        );
        assert!(loaded.project.unread.is_empty());
        let coverage = TfAnalysis::run(&loaded.project, "", loaded.bindings).coverage();
        assert_eq!(coverage.unresolved_modules.len(), 1);
        assert!(!coverage.is_complete());
    }

    #[test]
    fn a_module_that_climbs_out_of_the_root_is_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        write(
            dir.path(),
            "outside/main.tf",
            r#"resource "aws_s3_bucket" "secret" {}"#,
        );
        write(
            &project,
            "main.tf",
            r#"module "x" { source = "../outside" }"#,
        );
        let loaded = load_terraform_project(&project).unwrap();
        assert_eq!(loaded.project.modules.keys().collect::<Vec<_>>(), [""]);
        let coverage = TfAnalysis::run(&loaded.project, "", loaded.bindings).coverage();
        assert_eq!(coverage.unresolved_modules.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_that_leaves_the_root_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        write(
            dir.path(),
            "elsewhere/main.tf",
            r#"resource "aws_s3_bucket" "secret" {}"#,
        );
        write(&project, "main.tf", r#"module "x" { source = "./linked" }"#);
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), project.join("linked")).unwrap();
        let loaded = load_terraform_project(&project).unwrap();
        assert_eq!(loaded.project.modules.keys().collect::<Vec<_>>(), [""]);
        assert!(loaded.project.unread.iter().any(|u| u.path == "linked"));
        assert!(
            !TfAnalysis::run(&loaded.project, "", loaded.bindings)
                .coverage()
                .is_complete()
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_makes_the_analysis_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.tf", r#"resource "aws_s3_bucket" "b" {}"#);
        write(dir.path(), "broken.tf", "resource \"a\" {");
        write(dir.path(), "extra.tf.json", "{}");
        let loaded = load_terraform_project(dir.path()).unwrap();
        let paths: Vec<&str> = loaded
            .project
            .unread
            .iter()
            .map(|u| u.path.as_str())
            .collect();
        assert_eq!(paths, ["broken.tf", "extra.tf.json"]);
        let coverage = TfAnalysis::run(&loaded.project, "", loaded.bindings).coverage();
        assert!(!coverage.is_complete());
        assert_eq!(coverage.unread.len(), 2);
    }

    #[test]
    fn variable_files_bind_constants_and_later_files_override() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.tf", r#"variable "a" {}"#);
        write(
            dir.path(),
            "terraform.tfvars",
            "a = \"first\"\nb = 1\nc = var.a\n",
        );
        write(dir.path(), "z.auto.tfvars", "a = \"second\"\n");
        let loaded = load_terraform_project(dir.path()).unwrap();
        assert_eq!(
            loaded.bindings.values().get("a"),
            Some(&TfValue::Str("second".into()))
        );
        assert_eq!(
            loaded.bindings.values().get("b"),
            Some(&TfValue::Number("1".into()))
        );
        assert!(loaded.bindings.values().get("c").is_none());
        assert!(
            loaded
                .project
                .unread
                .iter()
                .any(|u| u.path == "terraform.tfvars" && u.reason.contains("`c`"))
        );
    }

    #[test]
    fn a_directory_without_tf_files_is_not_a_project() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "notes.txt", "hello");
        let loaded = load_terraform_project(dir.path()).unwrap();
        assert!(loaded.project.modules.is_empty());
        assert_eq!(loaded.project.unread.len(), 1);
    }

    fn unread_of<'a>(loaded: &'a LoadedTerraform, path: &str) -> Option<&'a UnreadSource> {
        loaded.project.unread.iter().find(|u| u.path == path)
    }

    fn acl_of(loaded: &LoadedTerraform, address: &str) -> Option<TfExpr> {
        loaded.project.modules[""].resources[address]
            .body
            .attribute("acl")
            .map(|a| a.expr.clone())
    }

    #[test]
    fn an_override_file_is_not_merged_and_leaves_the_analysis_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let bucket = |acl: &str| format!("resource \"aws_s3_bucket\" \"b\" {{ acl = \"{acl}\" }}");
        write(dir.path(), "main.tf", &bucket("private"));
        write(dir.path(), "override.tf", &bucket("public-read"));
        write(dir.path(), "prod_override.tf", &bucket("public-read-write"));
        let loaded = load_terraform_project(dir.path()).unwrap();
        for name in ["override.tf", "prod_override.tf"] {
            let unread = unread_of(&loaded, name).unwrap_or_else(|| panic!("{name} is listed"));
            assert!(unread.reason.contains("override"), "{}", unread.reason);
        }
        assert_eq!(
            acl_of(&loaded, "aws_s3_bucket.b"),
            Some(TfExpr::Literal(TfValue::Str("private".into()))),
            "an override never replaces the declaration it would merge into"
        );
        assert!(
            !TfAnalysis::run(&loaded.project, "", loaded.bindings)
                .coverage()
                .is_complete()
        );
    }

    #[test]
    fn a_file_that_only_looks_like_an_override_is_read_normally() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.tf", r#"resource "aws_s3_bucket" "a" {}"#);
        write(
            dir.path(),
            "overrides.tf",
            r#"resource "aws_s3_bucket" "b" {}"#,
        );
        write(
            dir.path(),
            "my-override.tf",
            r#"resource "aws_s3_bucket" "c" {}"#,
        );
        let loaded = load_terraform_project(dir.path()).unwrap();
        assert_eq!(loaded.project.modules[""].resources.len(), 3);
        assert!(loaded.project.unread.is_empty());
    }

    #[test]
    fn a_declaration_made_twice_keeps_the_first_and_is_listed_as_unread() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a.tf",
            "resource \"aws_s3_bucket\" \"b\" { acl = \"private\" }\nvariable \"v\" {}\nvariable \"v\" {}\n",
        );
        write(
            dir.path(),
            "b.tf",
            "resource \"aws_s3_bucket\" \"b\" { acl = \"public-read\" }\n",
        );
        let loaded = load_terraform_project(dir.path()).unwrap();
        assert_eq!(
            acl_of(&loaded, "aws_s3_bucket.b"),
            Some(TfExpr::Literal(TfValue::Str("private".into()))),
            "the later declaration does not replace the earlier one"
        );
        let across = unread_of(&loaded, "b.tf").expect("the second declaration is listed");
        assert!(
            across.reason.contains("aws_s3_bucket.b"),
            "{}",
            across.reason
        );
        assert!(
            across.reason.contains("more than once"),
            "{}",
            across.reason
        );
        let within = unread_of(&loaded, "a.tf").expect("a repeat in one file is listed");
        assert!(within.reason.contains("`v`"), "{}", within.reason);
        assert!(
            !TfAnalysis::run(&loaded.project, "", loaded.bindings)
                .coverage()
                .is_complete()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_source_whose_name_is_not_utf8_is_listed_as_unread() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.tf", r#"resource "aws_s3_bucket" "b" {}"#);
        for name in [&b"caf\xe9.tf"[..], b"x\xff.tf.json", b"y\xfe.auto.tfvars"] {
            fs::write(dir.path().join(OsStr::from_bytes(name)), "").unwrap();
        }
        fs::write(dir.path().join(OsStr::from_bytes(b"notes\xff.txt")), "").unwrap();
        let loaded = load_terraform_project(dir.path()).unwrap();
        let listed: Vec<&UnreadSource> = loaded
            .project
            .unread
            .iter()
            .filter(|u| u.reason.contains("UTF-8"))
            .collect();
        assert_eq!(listed.len(), 3, "{:?}", loaded.project.unread);
        assert!(
            listed
                .iter()
                .any(|u| u.path.starts_with("caf") && u.path.ends_with(".tf"))
        );
        assert!(
            !TfAnalysis::run(&loaded.project, "", loaded.bindings)
                .coverage()
                .is_complete()
        );
    }

    #[test]
    fn a_hostile_nesting_depth_is_refused_and_listed_instead_of_exhausting_the_stack() {
        let depth = 200_000;
        let deep = |open: &str, close: &str| {
            format!(
                "resource \"r\" \"x\" {{ a = {}1{} }}\n",
                open.repeat(depth),
                close.repeat(depth)
            )
        };
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.tf", r#"resource "aws_s3_bucket" "b" {}"#);
        write(dir.path(), "tuple.tf", &deep("[", "]"));
        write(dir.path(), "call.tf", &deep("f(", ")"));
        write(dir.path(), "object.tf", &deep("{k=", "}"));
        write(
            dir.path(),
            "blocks.tf",
            &format!("{}{}", "b {\n".repeat(depth), "}\n".repeat(depth)),
        );
        write(
            dir.path(),
            "terraform.tfvars",
            &format!("a = {}1{}", "[".repeat(depth), "]".repeat(depth)),
        );
        let loaded = load_terraform_project(dir.path()).unwrap();
        for name in [
            "tuple.tf",
            "call.tf",
            "object.tf",
            "blocks.tf",
            "terraform.tfvars",
        ] {
            let unread = unread_of(&loaded, name).unwrap_or_else(|| panic!("{name} is listed"));
            assert!(unread.reason.contains("nest"), "{name}: {}", unread.reason);
        }
        assert!(
            loaded.project.modules[""]
                .resources
                .contains_key("aws_s3_bucket.b")
        );
    }
}

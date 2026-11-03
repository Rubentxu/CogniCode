//! `refactor --apply` must apply exactly what the preview showed, inside
//! the workspace the operator named — not what the plan file claims.

use std::fs;
use std::path::Path;

use clap::Parser;
use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::interface::cli::{Cli, CommandExecutor};
use cognicode_core::refactor::apply::compute_workspace_state_digest;
use cognicode_core::refactor::plan::{PlannedEdit, RefactorPlan, build_plan, save_plan_to_file};
use tempfile::TempDir;

fn plan_for(workspace: &Path, file: &Path) -> RefactorPlan {
    let edit = PlannedEdit {
        file: file.to_path_buf(),
        byte_range: 0..3,
        replacement: "new".to_string(),
        deterministic: true,
    };
    build_plan(
        workspace.to_path_buf(),
        AnalysisBasis::for_workspace(workspace.to_path_buf()),
        vec![file.to_path_buf()],
        vec![edit],
        vec![],
        vec![],
    )
    .expect("build_plan")
}

struct Fixture {
    _dir: TempDir,
    workspace: std::path::PathBuf,
    file: std::path::PathBuf,
    plan_file: std::path::PathBuf,
    preview_digest: String,
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let workspace = dir.path().canonicalize().unwrap();
    let file = workspace.join("a.txt");
    fs::write(&file, "old body").unwrap();
    let plan = plan_for(&workspace, &file);
    let plan_file = workspace.join("plan.json");
    save_plan_to_file(&plan, &plan_file).unwrap();
    let preview_digest = compute_workspace_state_digest(&plan);
    Fixture {
        _dir: dir,
        workspace,
        file,
        plan_file,
        preview_digest,
    }
}

async fn run_apply(f: &Fixture, digest: Option<&str>, workspace: &Path) -> bool {
    let mut args = vec![
        "cognicode".to_string(),
        "refactor".into(),
        "foo".into(),
        "--apply".into(),
        "--plan-file".into(),
        f.plan_file.display().to_string(),
        "--authorization-token".into(),
        "tk-operator".into(),
        "--workspace".into(),
        workspace.display().to_string(),
    ];
    if let Some(d) = digest {
        args.push("--expected-digest".into());
        args.push(d.to_string());
    }
    let cli = Cli::try_parse_from(args).expect("clap parse");
    CommandExecutor::execute(cli).await.is_ok()
}

#[tokio::test]
async fn apply_without_the_preview_digest_is_refused() {
    let f = fixture();
    assert!(!run_apply(&f, None, &f.workspace).await);
    assert_eq!(fs::read_to_string(&f.file).unwrap(), "old body");
}

#[tokio::test]
async fn a_file_changed_after_the_preview_is_not_applied() {
    let f = fixture();
    fs::write(&f.file, "old body, edited by someone else").unwrap();
    assert!(!run_apply(&f, Some(&f.preview_digest), &f.workspace).await);
    assert_eq!(
        fs::read_to_string(&f.file).unwrap(),
        "old body, edited by someone else"
    );
}

#[tokio::test]
async fn a_plan_pointing_outside_the_named_workspace_is_refused() {
    let f = fixture();
    let other = TempDir::new().unwrap();
    let other_root = other.path().canonicalize().unwrap();
    assert!(!run_apply(&f, Some(&f.preview_digest), &other_root).await);
    assert_eq!(fs::read_to_string(&f.file).unwrap(), "old body");
}

#[tokio::test]
async fn the_previewed_plan_in_its_own_workspace_is_applied() {
    let f = fixture();
    assert!(run_apply(&f, Some(&f.preview_digest), &f.workspace).await);
    assert_eq!(fs::read_to_string(&f.file).unwrap(), "new body");
}

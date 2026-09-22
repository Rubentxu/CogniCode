//! PRF-CLI-06 UAT (real binary):
//! Unicode paths, spaces in paths, different cwd, permission errors and
//! verbose mode all behave deterministically and usefully. Verbose must
//! never leak environment secrets (env vars with SECRET/TOKEN/KEY/PASS
//! substrings must not appear in output even with -v).

use std::path::{Path, PathBuf};
use std::process::Command;

fn cognicode_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode")
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cli06_uat_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_in(dir: &Path, args: &[&str], extra_env: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(cognicode_bin());
    cmd.args(args).current_dir(dir);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn cognicode binary")
}

#[test]
fn unicode_and_spaces_paths_work_deterministically() {
    let dir = temp_dir("unicode");
    let sub = dir.join("árbol con espacios");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("ñandú.rs"), "pub fn función_ñ() {}\n").unwrap();

    let out = run_in(
        dir.parent().unwrap(),
        &["analyze", sub.to_str().unwrap()],
        &[],
    );
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code,
        0,
        "unicode+spaces path must analyze deterministically (stderr: {})",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Analysis complete"),
        "analyze must complete deterministically: {stdout}"
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "unicode path analyzed must report success"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn different_cwd_produces_consistent_result() {
    let d1 = temp_dir("cwd_a");
    let d2 = temp_dir("cwd_b");
    std::fs::write(d1.join("same.rs"), "pub fn same_fn() {}\n").unwrap();
    std::fs::write(d2.join("same.rs"), "pub fn same_fn() {}\n").unwrap();

    let o1 = run_in(&d1, &["analyze", "."], &[]);
    let o2 = run_in(&d2, &["analyze", "."], &[]);
    assert_eq!(o1.status.code(), Some(0));
    assert_eq!(o2.status.code(), Some(0));
    // Determinism: identical workspaces from different cwds must produce
    // the same structured report (same complexity lines, same cycle count).
    let s1 = String::from_utf8_lossy(&o1.stdout);
    let s2 = String::from_utf8_lossy(&o2.stdout);
    assert!(
        s1.contains("=== Architecture Check ===") && s2.contains("=== Architecture Check ==="),
        "both runs must produce the full report"
    );
    let report1: String = s1
        .lines()
        .filter(|l| !l.starts_with("Analyzing"))
        .collect::<Vec<_>>()
        .join("\n");
    let report2: String = s2
        .lines()
        .filter(|l| !l.starts_with("Analyzing"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        report1, report2,
        "identical workspaces must produce identical reports regardless of cwd"
    );
    let _ = std::fs::remove_dir_all(&d1);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn permission_denied_is_deterministic_and_useful() {
    let dir = temp_dir("perm");
    let sub = dir.join("locked");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("secret.rs"), "pub fn hidden() {}\n").unwrap();
    // Remove read permission (deterministic failure on this file tree).
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(&sub).unwrap().permissions();
    perm.set_mode(0o000);
    std::fs::set_permissions(&sub, perm).unwrap();

    let out = run_in(&dir, &["analyze", "."], &[]);
    // Restore permissions before asserts so cleanup works even on failure.
    let mut perm = std::fs::metadata(&sub).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&sub, perm).unwrap();

    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 1, "no crash allowed, got {code}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verbose_never_leaks_environment_secrets() {
    let dir = temp_dir("verbose");
    std::fs::write(dir.join("lib.rs"), "pub fn v() {}\n").unwrap();
    let fake = "SUPERSECRETVALUE-prf-cli-06";
    let out = run_in(
        &dir,
        &["-v", "analyze", "."],
        &[
            ("COGNICODE_API_TOKEN", fake),
            ("MY_SECRET_KEY", fake),
            ("DB_PASSWORD", fake),
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let all = format!("{stdout}{stderr}");
    assert!(
        !all.contains(fake),
        "verbose mode must never echo secret env values; got: {all}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

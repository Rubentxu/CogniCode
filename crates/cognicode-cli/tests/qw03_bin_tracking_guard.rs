//! QW-03 — bin tracking guard test (contractual).
//!
//! Pinea el contrato del guard `scripts/ci/check-bin-tracking.sh`:
//!   1. Sobre el repo real con su `Cargo.toml` actual y sus bins
//!      ya tracked: el guard retorna exit 0.
//!   2. Sobre un repo temporal clonado donde añadimos un bin
//!      `[[bin]]` declarado en `Cargo.toml` pero cuyo source file
//!      queda UNTRACKED (no presente en `git ls-files`): el guard
//!      retorna exit 1 con un mensaje que nombra el bin.
//!   3. Sobre el mismo repo temporal con el bin tracked: el guard
//!      retorna exit 0 (cleanup path).
//!
//! El test es RED si el guard no detecta el drift. Esto es lo que
//! lo hace contractual: plantar el caso RED y verificar que el guard
//! lo detecta (no solo verificar que sobre HEAD limpio pasa).
//!
//! Históricamente, el bin `cognicode-control-plane` se creó en entry 9
//! pero su `src/bin/control_plane.rs` quedó untracked porque `.gitignore`
//! silenciaba `bin/` a nivel de crate. CI falló al primer release.
//! Este test evita que vuelva a ocurrir silenciosamente.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use common::repo_root;

/// Run the bin-tracking guard against the supplied root directory.
/// The `--root` flag selects which git repo the guard inspects; this
/// makes the test hermetic (tmp repos never touch the real repo's
/// tracked-files state).
fn run_guard(working_dir: &Path) -> std::process::Output {
    let script = repo_root().join("scripts/ci/check-bin-tracking.sh");
    assert!(
        script.exists(),
        "QW-03 guard missing at {}",
        script.display()
    );
    Command::new("bash")
        .arg(&script)
        .arg("--root")
        .arg(working_dir)
        .output()
        .expect("failed to spawn check-bin-tracking.sh")
}

/// Initialise `tmp` as a fresh git repository and copy every crate's
/// real `Cargo.toml` AND every existing bin source verbatim into
/// `tmp/crates/<name>/`. This gives the guard a realistic baseline
/// (`10 OK:` lines) before the test plants an additional drift.
///
/// Tracking is done explicitly: we `git add` every file we copied
/// so `git ls-files --error-unmatch` accepts them. The
/// `additional_drift` callback is then free to declare a NEW bin
/// whose source it deliberately does NOT track.
fn setup_full_tmp_repo<F>(tmp: &Path, additional_drift: F)
where
    F: FnOnce(&Path),
{
    fs::create_dir_all(tmp).expect("mkdir tmp");

    // The global git wrapper (`~/.local/bin/git`) refuses to sign
    // commits in repos whose host is not in the identity map (the
    // /tmp paths the test uses fall under "sin clasificar").
    // RANDOM_GIT_COMMITTER_DISABLED=1 is the documented escape
    // hatch: it delegates everything to /usr/bin/git and bypasses
    // the wrapper. We also set an explicit throwaway identity so
    // the commits carry a non-empty Author line.
    let env_overrides: [(&str, &str); 3] = [
        ("RANDOM_GIT_COMMITTER_DISABLED", "1"),
        ("GIT_AUTHOR_NAME", "qw03-test"),
        ("GIT_AUTHOR_EMAIL", "qw03-test@cognicode.local"),
    ];

    let init = Command::new("git")
        .args(["init", "-q", "--initial-branch=main"])
        .current_dir(tmp)
        .envs(env_overrides)
        .status()
        .expect("git init");
    assert!(init.success(), "git init failed in {}", tmp.display());

    for (k, v) in [
        ("user.email", "qw03-test@cognicode.local"),
        ("user.name", "qw03-test"),
    ] {
        Command::new("git")
            .args(["config", k, v])
            .current_dir(tmp)
            .env("RANDOM_GIT_COMMITTER_DISABLED", "1")
            .status()
            .expect("git config");
    }

    let real = repo_root();
    let crate_names = [
        "cognicode-cli",
        "cognicode-explorer",
        "cognicode-mcp",
        "cognicode-runtime",
        "cognicode-sandbox",
    ];
    for cname in crate_names {
        let real_crate = real.join("crates").join(cname);
        let tmp_crate = tmp.join("crates").join(cname);
        fs::create_dir_all(&tmp_crate).expect("mkdir crate");

        // Cargo.toml
        fs::copy(real_crate.join("Cargo.toml"), tmp_crate.join("Cargo.toml"))
            .expect("copy Cargo.toml");

        // Copy every .rs file under src/ recursively so any
        // declared [[bin]] finds its source on disk. We DON'T
        // track these yet — we add them all in one shot below.
        let src = real_crate.join("src");
        if src.exists() {
            copy_tree(&src, &tmp_crate.join("src"));
        }
    }

    // Plant the additional drift BEFORE staging so the test owns
    // the order of operations.
    additional_drift(tmp);

    // We use `git add -f` because the user's GLOBAL gitignore
    // (`~/.config/git/ignore`) excludes `bin/` — exactly the
    // surface we are exercising. Without `-f` the bin sources
    // would silently stay untracked and the guard would fail for
    // reasons unrelated to its contract. The global ignore is a
    // CI/workspace hygiene preference, not a project rule, so
    // forcing it inside the tmp repo is correct.
    let add = Command::new("git")
        .args(["add", "-A", "-f"])
        .current_dir(tmp)
        .env("RANDOM_GIT_COMMITTER_DISABLED", "1")
        .status()
        .expect("git add");
    assert!(add.success(), "git add failed in {}", tmp.display());

    let commit = Command::new("git")
        .args(["commit", "-q", "-m", "qw03 baseline"])
        .current_dir(tmp)
        .envs([
            ("RANDOM_GIT_COMMITTER_DISABLED", "1"),
            ("GIT_AUTHOR_NAME", "qw03-test"),
            ("GIT_AUTHOR_EMAIL", "qw03-test@cognicode.local"),
        ])
        .status()
        .expect("git commit");
    assert!(commit.success(), "git commit failed in {}", tmp.display());
}

fn copy_tree(src: &Path, dst: &Path) {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::copy(src, dst).ok();
        return;
    }
    fs::create_dir_all(dst).ok();
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            copy_tree(&entry.path(), &dst.join(&name));
        }
    }
}

#[test]
fn qw03_bin_tracking_guard_passes_on_clean_real_repo() {
    // The real repo, with all bin sources already tracked: guard is
    // expected to pass. This is the GREEN baseline.
    let out = run_guard(&repo_root());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "QW-03 guard unexpectedly failed on the clean real repo:\n\
         --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stdout.contains("OK: todos los bin sources están tracked."),
        "guard output missing success banner; got:\n{stdout}"
    );
}

#[test]
fn qw03_bin_tracking_guard_fails_when_a_bin_source_is_untracked() {
    // Hermetic RED scenario: declare a fake bin in the tmp
    // Cargo.toml whose source file is NOT created. The guard must
    // detect this and return exit 1.
    let tmp = tempdir();
    setup_full_tmp_repo(&tmp, |root| {
        let tmp_crate = root.join("crates/cognicode-cli");
        let cargo_toml_path = tmp_crate.join("Cargo.toml");
        let mut cargo_toml = fs::read_to_string(&cargo_toml_path).unwrap();
        cargo_toml.push_str(
            "\n[[bin]]\nname = \"qw03-fake-untracked\"\npath = \"src/bin/qw03_fake_untracked.rs\"\n",
        );
        fs::write(&cargo_toml_path, cargo_toml).unwrap();
        // IMPORTANT: do NOT create the source file. The guard
        // must complain about "path missing" or "untracked".
    });

    let out = run_guard(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "QW-03 guard did NOT detect the planted untracked bin. \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("qw03-fake-untracked") || stdout.contains("qw03_fake_untracked"),
        "guard output did not name the planted bin. Got:\n{stdout}"
    );
}

#[test]
fn qw03_bin_tracking_guard_passes_after_tracking_the_missing_file() {
    // Companion to the RED scenario: once we create AND track the
    // planted file, the guard must turn GREEN. This pins that the
    // guard isn't permanently broken by the previous test and that
    // the fix path is observable.
    let tmp = tempdir();
    setup_full_tmp_repo(&tmp, |root| {
        let tmp_crate = root.join("crates/cognicode-cli");
        let cargo_toml_path = tmp_crate.join("Cargo.toml");
        let mut cargo_toml = fs::read_to_string(&cargo_toml_path).unwrap();
        cargo_toml.push_str(
            "\n[[bin]]\nname = \"qw03-recoverable\"\npath = \"src/bin/qw03_recoverable.rs\"\n",
        );
        fs::write(&cargo_toml_path, cargo_toml).unwrap();
        // Create AND track the source file so the guard passes.
        let bin = tmp_crate.join("src/bin/qw03_recoverable.rs");
        fs::create_dir_all(bin.parent().unwrap()).unwrap();
        fs::write(
            &bin,
            b"// qw03 recoverable bin source (tracked)\nfn main() {}\n",
        )
        .unwrap();
    });

    let out = run_guard(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "QW-03 guard failed on a tracked tmp repo:\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}

/// Build a unique temp directory under `std::env::temp_dir()` and
/// return its path. The directory is NOT cleaned automatically — we
/// rely on the OS tmp cleaner or the test runner to garbage-collect.
fn tempdir() -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "qw03-bin-tracking-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = base.join(unique);
    fs::create_dir_all(&path).expect("create tempdir");
    path
}

"""ChangeSet desde git: base, working tree, untracked, espacios y borrados."""

from __future__ import annotations

from harness_support import commit_change, demo_repo, git  # noqa: F401 - ajusta sys.path

from adapters.git_repo import CHANGESET_UNKNOWN, read_changeset


def test_merge_base_sees_every_commit_on_the_branch():
    """Con `HEAD~1` como base, el primer commit de la rama desaparecería."""
    with demo_repo() as repo:
        commit_change(repo, "crates/core/src/lib.rs", "// primer cambio\n", "primero")
        commit_change(repo, "crates/app/src/lib.rs", "// segundo cambio\n", "segundo")

        changeset = read_changeset(repo, "main")

        assert changeset.status != CHANGESET_UNKNOWN, changeset.fallback_reason
        assert changeset.merge_base and changeset.merge_base != changeset.head
        assert "crates/core/src/lib.rs" in changeset.changed
        assert "crates/app/src/lib.rs" in changeset.changed
        assert changeset.dirty is False


def test_missing_base_is_unknown_not_empty():
    """Sin base no se responde "no cambió nada": se responde que no se sabe."""
    with demo_repo() as repo:
        changeset = read_changeset(repo, None)
        assert changeset.is_unknown
        assert "no se pidió base" in changeset.fallback_reason
        assert changeset.all_paths() == ()


def test_unresolvable_base_is_unknown():
    with demo_repo() as repo:
        changeset = read_changeset(repo, "no-existe-esta-rama")
        assert changeset.is_unknown
        assert "no resuelve" in changeset.fallback_reason


def test_unrelated_history_is_unknown():
    with demo_repo() as repo:
        git(repo, "checkout", "-q", "--orphan", "huerfana")
        (repo / "otro.txt").write_text("x", encoding="utf-8")
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "raíz sin ancestro común")
        changeset = read_changeset(repo, "main")
        assert changeset.is_unknown
        assert "ancestro" in changeset.fallback_reason


def test_working_tree_and_untracked_files_with_spaces():
    """Un `-z` mal parseado convierte este caso en "nada cambió"."""
    with demo_repo() as repo:
        tracked = repo / "crates" / "core" / "src" / "con espacios.rs"
        tracked.write_text("// sin tocar\n", encoding="utf-8")
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "fichero con espacios")

        tracked.write_text("// modificado\n", encoding="utf-8")
        new_test = repo / "crates" / "core" / "tests" / "nuevo test.rs"
        new_test.write_text("// test nuevo\n", encoding="utf-8")

        changeset = read_changeset(repo, "main")

        assert "crates/core/src/con espacios.rs" in changeset.changed
        assert "crates/core/tests/nuevo test.rs" in changeset.untracked
        assert changeset.dirty is True


def test_deleted_test_file_is_reported():
    with demo_repo() as repo:
        victim = repo / "crates" / "core" / "tests" / "viejo.rs"
        victim.write_text("// test viejo\n", encoding="utf-8")
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "añade test")
        victim.unlink()
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "borra test")

        changeset = read_changeset(repo, "main")

        assert "crates/core/tests/viejo.rs" in changeset.deleted, changeset
        assert "crates/core/tests/viejo.rs" in changeset.all_paths()


def test_metadata_only_is_verified_narrowly():
    with demo_repo() as repo:
        (repo / "README.md").write_text("# readme\n", encoding="utf-8")
        only_readme = read_changeset(repo, "main")
        assert only_readme.touches_only_metadata() is True

        (repo / "docs" / "nueva.md").write_text("# nueva\n", encoding="utf-8")
        with_docs = read_changeset(repo, "main")
        assert with_docs.touches_only_metadata() is False


def test_content_digest_changes_with_bytes():
    with demo_repo() as repo:
        target = repo / "crates" / "core" / "src" / "lib.rs"
        before = read_changeset(repo, "main").digest_for(("crates/core/src/lib.rs",))
        target.write_text("// otra cosa\n", encoding="utf-8")
        after = read_changeset(repo, "main").digest_for(("crates/core/src/lib.rs",))
        assert before != after
        assert read_changeset(repo, "main").digest_for(("no/existe.rs",)) != before

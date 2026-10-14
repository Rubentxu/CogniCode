#!/usr/bin/env python3
"""Que `publish-merge-gate.sh` no pueda publicar un veredicto sobre un SHA que no probo.

MEDIDO 2026-10-14 (CD-2). El puente que publica el contexto `merge-gate` resuelve
el SHA, ejecuta la autoridad de merge y publica el resultado **bajo el nombre de
ese SHA**:

    resolved="$(git rev-parse "$sha")"           # linea 94
    pipelinek run --db "$PIPELINE_DB" merge-gate.pipeline.kts   # linea 100
    gh api ... "repos/$repo/statuses/$resolved" -f "context=merge-gate"

`$resolved` aparece en la peticion de publicacion y en la descripcion. **No
aparece en ninguna parte de lo que ejecuta la autoridad.** El pipeline corre
sobre el arbol de trabajo tal y como esta: nunca se hace checkout del SHA, nunca
se le pasa, y `merge-gate.pipeline.kts` no lo menciona.

El resultado es una firma falsa por construccion:

    $ bash scripts/ci/publish-merge-gate.sh v0.101.15     # un tag antiguo
    ==> running the merge authority for <sha-de-v0.101.15>
    ...gate verde sobre el arbol actual...
    ==> publishing merge-gate=success to owner/name@<sha-de-v0.101.15>

GitHub ve un `merge-gate` verde **en ese commit**, y el gate no lo miro. Con
`strict: true` eso es suficiente para desbloquear un merge. Es la forma mas
directa de saltarse el gate: no hace falta untruear nada, basta con apuntar a un
SHA que ya se sabe bueno mientras el arbol que se prueba es otro.

Este es el estado que el propio script documenta como coste aceptable ("a status
posted by a script is an assertion"). CD-1 elimino el falso verde de `fast`;
este es el falso verde del gate de merge, y es mas grave porque bloquea el
producto y no solo el feedback del desarrollador.

METODO. No hay red aqui y no debe haberla: los tests ejecutan el puente de
verdad contra `pipelinek` y `gh` de mentira, que registran lo que reciben. El
test afirma sobre el **SHA publicado** y sobre **que arbol se probo**, no sobre
el texto del script. Un test que greppara `git checkout` pasaria con la linea
presente pero sin usarse, que es justo el defecto que ya se produjo.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_REL = "scripts/ci/publish-merge-gate.sh"

# `pipelinek` de mentira: registra el hash del arbol que se probo y sale 0.
FAKE_PIPELINEK = """#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$(dirname "$0")/pipelinek.args"
# El contenido exacto del arbol probado es lo que hay que poder comparar con
# el SHA que despues se publica.
git -C "$PWD" rev-parse HEAD >> "$(dirname "$0")/pipelinek.args" 2>/dev/null
exit 0
"""

# `gh` de mentira: registra la llamada completa, incluido a que SHA se publica.
FAKE_GH = """#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$(dirname "$0")/gh.args"
exit 0
"""


class Harness:
    """Ejecuta el puente real contra `pipelinek`/`gh` falsos en un repo de dos SHAs."""

    def __init__(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="cc-merge-gate-"))
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        for cmd in (
            ["git", "init", "-q", "-b", "main"],
            ["git", "config", "user.email", "harness@example.invalid"],
            ["git", "config", "user.name", "harness"],
            ["git", "remote", "add", "origin",
             "git@github.com:acme/cognicode.git"],
        ):
            subprocess.run(cmd, cwd=self.repo, check=True, capture_output=True)

        # Dos commits con contenido distinguible: el SHA que se publica y el
        # arbol que sera HEAD cuando corra el puente.
        (self.repo / "marker.txt").write_text("OLD-SHA\n", encoding="utf-8")
        subprocess.run(["git", "add", "-A"], cwd=self.repo, check=True)
        subprocess.run(["git", "commit", "-q", "-m", "old"],
                       cwd=self.repo, check=True)
        self.old_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.repo,
            capture_output=True, text=True, check=True,
        ).stdout.strip()

        (self.repo / "marker.txt").write_text("NEW-SHA\n", encoding="utf-8")
        subprocess.run(["git", "add", "-A"], cwd=self.repo, check=True)
        subprocess.run(["git", "commit", "-q", "-m", "new"],
                       cwd=self.repo, check=True)
        self.head_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.repo,
            capture_output=True, text=True, check=True,
        ).stdout.strip()

        # El puente, en su ubicacion real relativa al repo. Se versionan los
        # dos ficheros que el script necesita ANTES de medir nada: si quedaran
        # sin commitear, el guard de arbol sucio (CD-2) rechazaria el puente y
        # los tests fallarian por un motivo que no es el que persiguen.
        scripts = self.repo / "scripts" / "ci"
        scripts.mkdir(parents=True)
        shutil.copy2(REPO_ROOT / SCRIPT_REL, scripts / "publish-merge-gate.sh")
        shutil.copy2(REPO_ROOT / "merge-gate.pipeline.kts",
                     self.repo / "merge-gate.pipeline.kts")
        subprocess.run(["git", "add", "-A"], cwd=self.repo, check=True)
        subprocess.run(["git", "commit", "-q", "-m", "add bridge and authority"],
                       cwd=self.repo, check=True)
        self.head_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.repo,
            capture_output=True, text=True, check=True,
        ).stdout.strip()

        self.bindir = self.tmp / "bin"
        self.bindir.mkdir()
        for name, body in (("pipelinek", FAKE_PIPELINEK), ("gh", FAKE_GH)):
            p = self.bindir / name
            p.write_text(body, encoding="utf-8")
            os.chmod(p, 0o755)
        self.pipelinek_args = self.bindir / "pipelinek.args"
        self.gh_args = self.bindir / "gh.args"

    def run(self, sha: str) -> tuple[int, str, str]:
        env = dict(os.environ)
        env["PATH"] = f"{self.bindir}{os.pathsep}{env.get('PATH', '')}"
        env["TMPDIR"] = str(self.tmp)
        proc = subprocess.run(
            ["bash", str(self.repo / SCRIPT_REL), sha],
            cwd=self.repo, capture_output=True, text=True,
            timeout=120, env=env,
        )
        return proc.returncode, proc.stdout, proc.stderr

    def pipelinek_calls(self) -> list[str]:
        if not self.pipelinek_args.is_file():
            return []
        return [l for l in self.pipelinek_args.read_text().splitlines() if l]

    def gh_calls(self) -> list[str]:
        if not self.gh_args.is_file():
            return []
        return [l for l in self.gh_args.read_text().splitlines() if l]

    def published_shas(self) -> list[str]:
        """SHA contra el que se publico, tal y como lo recibio `gh`.

        El path llega como `repos/<repo>/statuses/<sha>` **sin** barra final, asi
        que no basta con buscar un token que empiece por `statuses/`: hay que
        partirlo. Una version anterior de este helper buscaba el prefijo y
        devolvia lista vacia, lo que hacia que el test de la firma falsa
        fallara por un motivo equivocado y tapara el hallazgo.
        """
        out = []
        for call in self.gh_calls():
            for token in call.split():
                if "statuses/" in token:
                    out.append(token.split("statuses/", 1)[1].strip("/"))
        return out

    def tested_head(self) -> str | None:
        """Ultima linea de `pipelinek.args`: el HEAD que vio la autoridad."""
        calls = self.pipelinek_calls()
        if not calls:
            return None
        return calls[-1].strip()

    def cleanup(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)


def test_publishing_another_sha_is_refused() -> None:
    """Un veredicto no puede publicarse para un commit que no se probo.

    MEDIDO: el puente publicaba el veredicto de `367813eb...` mientras la
    autoridad probaba `7cfe749e...`. Con `strict: true`, ese `success` bastaba
    para desbloquear un merge de un commit que nadie habia examinado.

    La correccion es un rechazo, no un checkout: la autoridad de merge prueba el
    arbol en el que corre, asi que la unica forma de que el veredicto describa
    el commit nombrado es no correr para ningun otro commit.
    """
    h = Harness()
    try:
        rc, out, err = h.run(h.old_sha)
        combined = out + err
        assert rc == 2, (
            f"FAIL: publicar para otro commit deberia rechazarse con rc=2, "
            f"salio {rc}.\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert not h.published_shas(), (
            f"FAIL: se publico un estado para un commit que no se probo.\n"
            f"gh: {h.gh_calls()}"
        )
        assert h.old_sha in combined, (
            f"FAIL: el rechazo debe nombrar el commit pedido.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


def test_head_sha_publishes_its_own_verdict() -> None:
    """El caso legitimo: publicar el veredicto del HEAD si se prueba el HEAD.

    Control positivo. Sin el, un arreglo que rechazase siempre pasaria todos
        los tests de rechazo y dejaria el puente inservible: nadie podria
        publicar nunca un veredicto real.
    """
    h = Harness()
    try:
        rc, out, err = h.run(h.head_sha)
        published = h.published_shas()
        assert rc == 0, (
            f"FAIL: publicar el veredicto de HEAD debe funcionar (rc={rc}).\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
        assert published == [h.head_sha], (
            f"FAIL: se esperaba publicar solo {h.head_sha}, se publico "
            f"{published}.\ngh: {h.gh_calls()}"
        )
        tested = h.tested_head()
        assert tested == h.head_sha, (
            f"FAIL: se publico para {h.head_sha} pero la autoridad probo "
            f"{tested}. El estado no describe lo que se probo.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


def test_gate_failure_is_not_reported_as_success() -> None:
    """Un gate rojo tiene que publicarse como rojo, para ese mismo SHA.

    Control de la honestidad del bridge en la direccion contraria. CD-2 no es
    solo "no publiques verde de mas": es "publica lo que se probo".
    """
    h = Harness()
    try:
        # PipelineK que falla.
        p = h.bindir / "pipelinek"
        p.write_text(
            "#!/usr/bin/env bash\nexit 7\n", encoding="utf-8"
        )
        os.chmod(p, 0o755)
        rc, out, err = h.run(h.head_sha)
        assert rc == 7, (
            f"FAIL: el puente debe salir con el codigo del gate (7), salio "
            f"{rc}.\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert any("state=failure" in c for c in h.gh_calls()), (
            f"FAIL: un gate rojo se publico como verde.\ngh: {h.gh_calls()}"
        )
    finally:
        h.cleanup()


def test_refuses_uncommitted_trees() -> None:
    """Un arbol sucio no puede producir un veredicto publicable.

    MEDIDO: `merge-gate.pipeline.kts` compila y prueba el arbol de trabajo, no un
    commit. Con cambios sin commitear, el gate puede pasar sobre un estado que
    no existe en ningun sitio y que nadie podra reconstruir despues. Publicar eso
    bajo un SHA es afirmar algo que no se puede comprobar.
    """
    h = Harness()
    try:
        (h.repo / "marker.txt").write_text("DIRTY\n", encoding="utf-8")
        rc, out, err = h.run(h.head_sha)
        assert rc != 0, (
            f"FAIL: el puente publico un veredicto con el arbol sucio "
            f"(rc=0).\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert not any("state=success" in c for c in h.gh_calls()), (
            f"FAIL: se publico success con cambios sin commitear.\n"
            f"gh: {h.gh_calls()}"
        )
    finally:
        h.cleanup()


def test_unknown_sha_is_refused() -> None:
    """Un SHA que no existe no puede producir un estado publicado.

    `git rev-parse` acepta cualquier cosa que se parezca a un hash, y resuelve
    abreviados. Sin `--verify` y sin comprobar el tipo de objeto, un argumento
    equivocado puede acabar siendo el nombre de otra cosa.
    """
    h = Harness()
    try:
        rc, out, err = h.run("0" * 40)
        assert rc != 0, (
            f"FAIL: se publico un veredicto para un SHA inexistente (rc={rc}).\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


def main() -> int:
    tests = (
        test_publishing_another_sha_is_refused,
        test_head_sha_publishes_its_own_verdict,
        test_gate_failure_is_not_reported_as_success,
        test_refuses_uncommitted_trees,
        test_unknown_sha_is_refused,
    )
    failures: list[str] = []
    for fn in tests:
        try:
            fn()
            print(f"  ok   {fn.__name__}")
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
            print(f"  FAIL {fn.__name__}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")
            print(f"  ERR  {fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("merge-gate SHA authority contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(f"merge-gate SHA authority contract: OK ({len(tests)} checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
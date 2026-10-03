//! Un candidato sin los bundles de skills publicados no es un candidato.
//!
//! MEDIDO 2026-10-03. La stage `skill-bundles` de
//! `release-candidate.pipeline.kts` leía la lista de bundles publicados con
//!
//!     done < <(".../cognicode-release" skills --published)
//!
//! y ese subcomando **no existía**. El binario salía con 2 y
//! `unrecognized subcommand`, el bucle no consumía nada, y la stage terminaba
//! con 0 habiendo staged cero bundles en `staging/`.
//!
//! Lo que hacia el resto de la cadena no era un silencio, sino un diagnostico
//! equivocado. `generate_release` ya exige que cada bundle publicado tenga su
//! payload (`anyhow::ensure!` sobre `src.is_file()`), y `verify_release` exige
//! que cada bundle *declarado* en el manifiesto tenga el suyo. La invariante
//! estaba cubierta: published ⊆ staged la impone generate, declared ⊆ staged
//! la impone verify.
//!
//! El fallo real era que la stage se llevaba el golpe dos etapas mas tarde y
//! con un mensaje que senalaba el sitio equivocado — `stage it via
//! \`just bundle-skills\`` es un comando del workflow antiguo, no de esta lane.
//! Quien depurase el fallo habria buscas bundles en un justfile que esta lane
//! no ejecuta.
//!
//! Por eso este test fija las dos mitades, y ninguna de las dos sola sirve:
//! el subcommand tiene que existir para que la stage reciba una respuesta, y
//! `generate` tiene que seguir rechazando la ausencia para que una respuesta
//! vacia no llegue a ser una release. Un arreglo que hiciera fallar siempre
//! pasaria la primera mitad; por eso la segunda mitad se comprueba con el mismo
//! arbol que antes fallaba.

mod common;

use std::path::Path;
use std::process::Command;

use common::{release_bin_path, repo_root, workspace_tag, workspace_version};

const PLATFORM: &str = "x86_64-unknown-linux-gnu";

/// Los tres componentes publicados, como lo que el contrato llama `cogh`,
/// `cognicode` y `cognicode-mcp`. Los nombres salen del staging, no de una
/// lista escrita aqui: el proposito del test es que el contrato decida.
fn stage_components(stage: &Path, version: &str, bins: &Path) {
    std::fs::create_dir_all(stage).unwrap();
    std::fs::create_dir_all(bins).unwrap();
    for stem in ["cogh", "cognicode", "cognicode-mcp"] {
        let bin = bins.join(stem);
        std::fs::write(&bin, format!("#!/bin/sh\necho {stem}\n")).unwrap();
        let status = Command::new("tar")
            .arg("-czf")
            .arg(stage.join(format!("{stem}-{version}-{PLATFORM}.tar.gz")))
            .arg("-C")
            .arg(bins)
            .arg(stem)
            .status()
            .unwrap();
        assert!(status.success(), "tar failed for {stem}");
    }
}

fn stage_bundle(root: &Path, stage: &Path, version: &str, bundle: &str) {
    let dir = root.join("skills").join(bundle);
    assert!(
        dir.join("manifest.yaml").exists(),
        "skill `{bundle}` must have a versioned manifest.yaml at {}",
        dir.display()
    );
    let status = Command::new("tar")
        .arg("-czf")
        .arg(stage.join(format!("{bundle}-{version}.tar.gz")))
        .arg("-C")
        .arg(dir)
        .arg(".")
        .status()
        .unwrap();
    assert!(status.success(), "tar failed for skill bundle {bundle}");
}

fn generate(stage: &Path, out: &Path, version: &str, tag: &str) -> std::process::Output {
    let head = Command::new("git")
        .args(["-C", repo_root().to_str().unwrap(), "rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head.status.success());
    let sha = String::from_utf8(head.stdout).unwrap().trim().to_string();
    Command::new(release_bin_path())
        .args([
            "generate",
            "--staging",
            stage.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--version",
            version,
            "--tag",
            tag,
            "--source-commit",
            &sha,
            "--platform",
            PLATFORM,
        ])
        .output()
        .unwrap()
}

#[test]
fn the_release_tool_can_report_the_published_skill_bundles() {
    // Sin este subcommand la stage no tiene de donde sacar su lista. El fallo
    // medido no fue que imprimiera lo que no era: fue que no existia, y la
    // stage no lo pregunto.
    let out = Command::new(release_bin_path())
        .args(["skills", "--published"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "`skills --published` debe existir y salir 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ids: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    assert!(
        !ids.is_empty(),
        "el contrato publica bundles: una lista vacia significa que la \
         herramienta y el contrato discrepan"
    );

    // Y debe distinguir publicados de no publicados: `cognicode-developer`
    // esta en la tabla pero no se publica.
    let all = Command::new(release_bin_path())
        .args(["skills"])
        .output()
        .unwrap();
    assert!(all.status.success());
    let all_ids: Vec<String> = String::from_utf8(all.stdout)
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    assert!(
        all_ids.len() > ids.len(),
        "`skills` sin `--published` debe listar mas: la bandera tiene que \
         filtrar de verdad. published={:?} all={:?}",
        ids,
        all_ids
    );
    for id in &ids {
        assert!(
            all_ids.contains(id),
            "todo bundle publicado aparece en la tabla completa: {id}"
        );
    }
}

#[test]
fn a_candidate_missing_its_published_skill_bundles_is_rejected() {
    let root = repo_root();
    let version = workspace_version();
    let tag = workspace_tag();

    let tmp = std::env::temp_dir().join(format!("skills-orphan-{}", std::process::id()));
    let stage = tmp.join("stage");
    let bins = tmp.join("bins");
    let out = tmp.join("out");
    std::fs::create_dir_all(&tmp).unwrap();

    // Componentes completos, bundles ausentes: exactamente lo que la lane
    // dejaba cuando el subcommand fallaba en silencio.
    stage_components(&stage, &version, &bins);

    let result = generate(&stage, &out, &version, &tag);
    assert!(
        !result.status.success(),
        "un candidato sin los bundles publicados tiene que rechazarse.\n\
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("cognicode") && stderr.to_lowercase().contains("bundle"),
        "el motivo tiene que nombrar el bundle que falta y decir que es un \
         bundle; un error generico obligaria a leer el codigo para \
         descubrir que faltaba una skill.\nstderr: {stderr}"
    );

    // Y con los bundles presentes, el mismo arbol pasa. Sin esta mitad, un
    // arreglo que hiciera fallar siempre tambien saldria verde.
    stage_bundle(&root, &stage, &version, "cognicode");
    stage_bundle(&root, &stage, &version, "cognicode-mcp");
    let ok = generate(&stage, &out, &version, &tag);
    assert!(
        ok.status.success(),
        "con los bundles publicados staged, el mismo arbol tiene que pasar.\n\
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&ok.stdout),
        String::from_utf8_lossy(&ok.stderr)
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

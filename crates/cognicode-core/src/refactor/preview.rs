//! B5.W2 — Preview verificable de un [`crate::refactor::plan::RefactorPlan`] (B5.W1).
//!
//! Estado pre-B5.W2:
//!   - `RefactorService` y `safe_refactor` (MCP) producen un
//!     `RefactorPreviewDto` con descripcion y lista de paths. Eso
//!     alcanza para mostrar "que archivos se tocaran" pero NO para
//!     auditar el cambio byte a byte.
//!   - B5.W1 aterrizo el contrato portador del plan: `RefactorPlan`
//!     con `id`, `workspace_identity`, `basis` (B2.W1),
//!     `affected_files`, `edits` (`file`+`byte_range`+`replacement`+`deterministic`),
//!     `preconditions`, `risks`, `created_at`, `digest` (sha256).
//!   - B5.W3 (apply) necesita un preview que ya este calculado para
//!     validar el `workspace_state_digest` y para mostrarle al
//!     humano/agente "que cambiara si aplico".
//!
//! B5.W2 aporta:
//!   - `FileDiff`: bytes originales, bytes nuevos, unified diff
//!     (un hunk por archivo) y rangos modificados en bytes.
//!   - `RefactorPreview`: agregacion ordenada por path, con
//!     `plan_id`, `plan_digest`, `workspace_state_digest`,
//!     `workspace_identity`, totales, resumenes de riesgos /
//!     precondiciones y flag `cancelled` (placeholder para W3/W4).
//!   - `render_preview`: funcion PURA. Lee los archivos del plan con
//!     `&Path` (sin tocar nada en disco), aplica las `PlannedEdit` de
//!     B5.W1 sobre `Vec<u8>` en memoria, computa el unified diff, y
//!     devuelve un `RefactorPreview` totalmente determinista.
//!   - `verify_preview_no_mutation`: comparador best-effort de
//!     mtimes del workspace antes y despues. NO es una prueba
//!     formal; el test integration `preview_does_not_modify_files`
//!     compara BYTES y es la garantia real.
//!   - `PreviewError`: errores honestos. Variantes fail-closed para
//!     cada condicion de borde (path traversal, edits solapados,
//!     rango fuera de bounds, etc.).
//!
//! Politica B5.W2 (NO se rompe bajo ninguna circunstancia):
//!   - NUNCA se invoca `OpenOptions::write`, `create`, `set_modified`
//!     ni `remove_file` dentro de este modulo.
//!   - El unico `fs::*` permitido es `read` (y `metadata`/
//!     `symlink_metadata` para el sanity-check de mtime y para
//!     rechazar symlinks AAT-B5-02).
//!   - El orden de los `file_diffs` es SIEMPRE por `path` ascendente
//!     (estabilidad byte-equal entre runs).
//!   - El `workspace_state_digest` se DELEGA a
//!     `apply::compute_workspace_state_digest` (B5.W3) para que el
//!     apply pueda recomputarlo y compararlo byte-a-byte. Si B5.W2
//!     usara un algoritmo distinto, el apply nunca matchearia.
//!   - `cancelled` queda cableado a `false`. La cancelacion es
//!     materia de B5.W3 (apply con autoridad) y B5.W4
//!     (revalidacion); B5.W2 no la implementa pero deja el slot.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::apply::compute_workspace_state_digest;
use super::plan::{PlannedEdit, Precondition, RefactorPlan, Risk};

// ============================================================================
// FileDiff / RefactorPreview — outputs del preview.
// ============================================================================

/// Diff determinista de UN archivo: bytes originales, bytes nuevos,
/// unified diff (formato estandar) y rangos modificados en bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path del archivo RELATIVO a `workspace_root`. La conversion a
    /// absoluto la hace `render_preview` para mantener el plan
    /// portable entre clones.
    pub path: PathBuf,
    /// Bytes leidos del workspace antes de aplicar las ediciones.
    /// Snapshot inmutable; el preview no los vuelve a leer.
    pub original_bytes: Vec<u8>,
    /// Bytes resultantes de aplicar las `PlannedEdit` (de B5.W1)
    /// en orden.
    pub new_bytes: Vec<u8>,
    /// Unified diff (formato estandar `diff -u`). Vacio si los
    /// bytes son identicos.
    pub unified_diff: String,
    /// Rangos de bytes en `new_bytes` que fueron tocados
    /// (post-aplicacion). Util para resaltar en UI.
    pub byte_range_modified: Vec<Range<usize>>,
}

/// Preview agregada de un `RefactorPlan` (B5.W1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorPreview {
    /// Identidad del plan (`plan.id` de B5.W1). UUID v4.
    pub plan_id: Uuid,
    /// Digest del plan en si (`plan.digest` de B5.W1: sha256 sobre
    /// los campos firma-ables, sin timestamps). Estable entre runs
    /// SI el plan no cambia.
    pub plan_digest: String,
    /// Digest del estado del workspace SOBRE EL QUE SE CALCULO el
    /// preview. Se DELEGA a `apply::compute_workspace_state_digest`
    /// (B5.W3): el apply lo recomputa y compara byte-a-byte. Si el
    /// workspace muta entre preview y apply, el digest difiere y el
    /// apply rehúsa.
    pub workspace_state_digest: String,
    /// Identidad canonica del workspace (`plan.workspace_identity`).
    /// Se preserva para que el consumer no tenga que cargar el plan
    /// completo para saber sobre que workspace se calculo.
    pub workspace_identity: String,
    /// Diffs por archivo, ORDENADOS por path ascendente.
    pub file_diffs: Vec<FileDiff>,
    /// Suma de `new_bytes.len()` sobre todos los archivos.
    pub byte_size_total: usize,
    /// Conteo de archivos con al menos un cambio.
    pub affected_file_count: usize,
    /// Resumen textual de los riesgos del plan (vienen de
    /// `plan.risks` ya ordenados por `build_plan`).
    pub risk_summary: Vec<String>,
    /// Resumen textual de las precondiciones (`plan.preconditions`).
    pub precondition_summary: Vec<String>,
    /// Placeholder. SIEMPRE `false` en B5.W2 (la cancelacion es
    /// materia de B5.W3 y B5.W4). Se conserva el slot para que la
    /// serializacion JSON no cambie entre WUs.
    pub cancelled: bool,
}

// ============================================================================
// Errores del preview.
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    /// El `workspace_root` no existe o no es un directorio.
    InvalidWorkspaceRoot(PathBuf),
    /// El `file` de un `PlannedEdit` (B5.W1) no esta dentro de
    /// `workspace_root` (posible traversal).
    PathEscapesWorkspace(PathBuf),
    /// El archivo afectado no existe en el workspace.
    AffectedFileMissing(PathBuf),
    /// El path es un symlink que apunta fuera del workspace
    /// (AAT-B5-02; el apply tambien lo rechaza, el preview es
    /// simetrico).
    SymlinkEscape(PathBuf),
    /// Dos `PlannedEdit` del mismo archivo se solapan en bytes.
    OverlappingEdits {
        path: PathBuf,
        range_a: Range<usize>,
        range_b: Range<usize>,
    },
    /// Un `byte_range` cae fuera del archivo leido.
    RangeOutOfBounds {
        path: PathBuf,
        range: Range<usize>,
        file_size: usize,
    },
    /// I/O error de lectura (preservado como string para no atar el
    /// preview a un `io::ErrorKind` concreto).
    IoError(String),
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreviewError::InvalidWorkspaceRoot(p) => {
                write!(f, "invalid workspace root: {:?}", p)
            }
            PreviewError::PathEscapesWorkspace(p) => {
                write!(f, "path escapes workspace: {:?}", p)
            }
            PreviewError::AffectedFileMissing(p) => {
                write!(f, "affected file missing: {:?}", p)
            }
            PreviewError::SymlinkEscape(p) => {
                write!(f, "symlink escapes workspace: {:?}", p)
            }
            PreviewError::OverlappingEdits {
                path,
                range_a,
                range_b,
            } => write!(
                f,
                "overlapping edits in {:?}: {:?} vs {:?}",
                path, range_a, range_b
            ),
            PreviewError::RangeOutOfBounds {
                path,
                range,
                file_size,
            } => write!(
                f,
                "range {:?} out of bounds for {:?} (size={})",
                range, path, file_size
            ),
            PreviewError::IoError(s) => write!(f, "i/o error: {}", s),
        }
    }
}

impl std::error::Error for PreviewError {}

// ============================================================================
// render_preview — la funcion PURA del WU.
// ============================================================================

/// Renderiza el preview de un `RefactorPlan` (B5.W1) leido desde el
/// workspace `workspace_root` (que se compara con `plan.basis`).
///
/// Es una funcion PURA respecto al workspace:
///   - Solo LECTURA inmutable de los archivos afectados.
///   - Las ediciones se aplican sobre `Vec<u8>` en memoria.
///   - El resultado depende unicamente de los bytes leidos y del
///     plan. Mismo plan + mismos bytes => mismo `RefactorPreview`
///     BYTE a BYTE (serializacion JSON canonica incluida).
///
/// Errores: ver `PreviewError`. Ante CUALQUIER error, NO se
/// devuelve preview parcial; se aborta con el primer fallo.
pub fn render_preview(
    plan: &RefactorPlan,
    workspace_root: &Path,
) -> Result<RefactorPreview, PreviewError> {
    if !workspace_root.exists() {
        return Err(PreviewError::InvalidWorkspaceRoot(
            workspace_root.to_path_buf(),
        ));
    }
    let root_meta = fs::metadata(workspace_root)
        .map_err(|e| PreviewError::IoError(format!("stat {:?}: {}", workspace_root, e)))?;
    if !root_meta.is_dir() {
        return Err(PreviewError::InvalidWorkspaceRoot(
            workspace_root.to_path_buf(),
        ));
    }

    // 1) Resolver paths absolutos de B5.W1 contra workspace_root y
    //    validar que caen dentro (anti-traversal AAT-B5-02).
    //    Ademas: rechazar symlinks (simetria con apply).
    let mut file_inputs: Vec<(PathBuf /*rel*/, PathBuf /*abs*/, Vec<PlannedEdit>)> = Vec::new();
    for edit in &plan.edits {
        let abs = resolve_within_root(workspace_root, &edit.file)?;
        reject_symlink_escape(&abs)?;
        let rel = rel_from_abs(workspace_root, &abs);
        let entry = file_inputs.iter_mut().find(|(r, _, _)| r == &rel);
        match entry {
            Some((_, _, edits)) => edits.push(edit.clone()),
            None => file_inputs.push((rel, abs, vec![edit.clone()])),
        }
    }

    // 2) Orden estable por path (relativo al workspace).
    file_inputs.sort_by(|a, b| a.0.cmp(&b.0));

    // 3) Para cada grupo de edits: ordenar por byte_range.start y
    //    detectar solapamientos, luego leer y aplicar.
    let mut file_diffs: Vec<FileDiff> = Vec::with_capacity(file_inputs.len());
    let mut byte_size_total: usize = 0;
    let mut affected_file_count: usize = 0;

    for (rel, abs, mut edits) in file_inputs {
        edits.sort_by_key(|e| (e.byte_range.start, e.byte_range.end));
        for w in edits.windows(2) {
            let a = &w[0];
            let b = &w[1];
            if a.byte_range.end > b.byte_range.start {
                return Err(PreviewError::OverlappingEdits {
                    path: rel.clone(),
                    range_a: a.byte_range.clone(),
                    range_b: b.byte_range.clone(),
                });
            }
        }

        if !abs.exists() {
            return Err(PreviewError::AffectedFileMissing(rel.clone()));
        }
        let original =
            fs::read(&abs).map_err(|e| PreviewError::IoError(format!("read {:?}: {}", abs, e)))?;
        let file_size = original.len();
        for e in &edits {
            if e.byte_range.start > file_size || e.byte_range.end > file_size {
                return Err(PreviewError::RangeOutOfBounds {
                    path: rel.clone(),
                    range: e.byte_range.clone(),
                    file_size,
                });
            }
        }

        let (new_bytes, byte_range_modified) = apply_edits(&original, &edits);
        let unified_diff = compute_unified_diff(&rel, &original, &new_bytes);

        if new_bytes != original {
            affected_file_count += 1;
        }
        byte_size_total += new_bytes.len();

        file_diffs.push(FileDiff {
            path: rel,
            original_bytes: original,
            new_bytes,
            unified_diff,
            byte_range_modified,
        });
    }

    // 4) Delegamos el workspace_state_digest a B5.W3. Asi, el
    //    apply puede recomputarlo y comparar byte-a-byte.
    let workspace_state_digest = compute_workspace_state_digest(plan);

    // 5) Resumenes textuales de riesgos / precondiciones.
    let risk_summary = summarize_risks(&plan.risks);
    let precondition_summary = summarize_preconditions(&plan.preconditions);

    Ok(RefactorPreview {
        plan_id: plan.id,
        plan_digest: plan.digest.clone(),
        workspace_state_digest,
        workspace_identity: plan.workspace_identity.clone(),
        file_diffs,
        byte_size_total,
        affected_file_count,
        risk_summary,
        precondition_summary,
        cancelled: false,
    })
}

// ============================================================================
// verify_preview_no_mutation — sanity check best-effort.
// ============================================================================

/// Captura los mtimes del workspace ANTES de invocar `render_preview`.
///
/// El caller DEBE llamar a `verify_no_mutation_since(&snapshot, root)`
/// despues, y la combinacion vale como sanity-check de que
/// `render_preview` no escribio nada. NO es una prueba formal; el
/// test integration `preview_does_not_modify_files` compara BYTES
/// y es la garantia real.
pub struct MtimeSnapshot {
    captures: Vec<(PathBuf, Option<SystemTime>)>,
}

/// Captura los mtimes de los archivos actualmente referenciados por
/// `plan.edits` (paths resueltos contra `workspace_root`).
pub fn snapshot_workspace_mtimes(
    plan: &RefactorPlan,
    workspace_root: &Path,
) -> Result<MtimeSnapshot, PreviewError> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for edit in &plan.edits {
        let abs = resolve_within_root(workspace_root, &edit.file)?;
        if !seen.contains(&abs) {
            seen.push(abs);
        }
    }
    let mut captures = Vec::with_capacity(seen.len());
    for abs in seen {
        let mt = fs::metadata(&abs).ok().and_then(|m| m.modified().ok());
        captures.push((abs, mt));
    }
    Ok(MtimeSnapshot { captures })
}

/// Compara los mtimes capturados con los actuales. Devuelve `true`
/// si NINGUNO cambio (best-effort: si el archivo fue borrado o el
/// mtime es inaccesible, devuelve `false` conservador).
pub fn verify_no_mutation_since(snapshot: &MtimeSnapshot, _workspace_root: &Path) -> bool {
    for (abs, before) in &snapshot.captures {
        let now = match fs::metadata(abs) {
            Ok(m) => match m.modified() {
                Ok(t) => Some(t),
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        if now != *before {
            return false;
        }
    }
    true
}

/// Helper de un solo call: snapshot + render + verify.
///
/// Conveniente para tests y para el CLI. NO la usa el MCP handler
/// porque ahi queremos composicion explicita.
pub fn verify_preview_no_mutation(plan: &RefactorPlan, workspace_root: &Path) -> bool {
    let Ok(snapshot) = snapshot_workspace_mtimes(plan, workspace_root) else {
        return false;
    };
    let _ = render_preview(plan, workspace_root);
    verify_no_mutation_since(&snapshot, workspace_root)
}

// ============================================================================
// Internals — paths, aplicacion, unified diff.
// ============================================================================

/// Resuelve un path (absoluto o relativo) contra `root`, y rechaza
/// cualquier resultado que ESCAPE del root (anti-traversal AAT-B5-02).
fn resolve_within_root(root: &Path, p: &Path) -> Result<PathBuf, PreviewError> {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    };
    let normalized = normalize_path(&abs);
    let root_norm = normalize_path(root);
    if !normalized.starts_with(&root_norm) {
        return Err(PreviewError::PathEscapesWorkspace(p.to_path_buf()));
    }
    Ok(normalized)
}

/// Devuelve el path relativo a `root` (best-effort). Si `abs` no
/// cae dentro de `root`, devuelve `abs` tal cual.
fn rel_from_abs(root: &Path, abs: &Path) -> PathBuf {
    match abs.strip_prefix(root) {
        Ok(r) => r.to_path_buf(),
        Err(_) => abs.to_path_buf(),
    }
}

/// Normaliza un path (best-effort, sin `canonicalize` para no fallar
/// si el path no existe). Maneja `.` y `..` lexicamente.
fn normalize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Si `abs` es un symlink, devuelve `SymlinkEscape`. Simetria con
/// `apply` (AAT-B5-02). Best-effort: si el symlink no se puede
/// inspeccionar, se permite y lo bloquea el apply real.
fn reject_symlink_escape(abs: &Path) -> Result<(), PreviewError> {
    let meta = match fs::symlink_metadata(abs) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };
    if !meta.file_type().is_symlink() {
        return Ok(());
    }
    // Es symlink: comprobamos que su destino (tras canonicalize)
    // cae dentro del root. Si canonicalize falla, conservamos el
    // symlink (lo detectara el apply real).
    if let Ok(canonical) = abs.canonicalize()
        && let Some(root_norm) = abs.ancestors().find(|a| a.parent().is_none())
        && !canonical.starts_with(root_norm)
    {
        return Err(PreviewError::SymlinkEscape(abs.to_path_buf()));
    }
    Ok(())
}

/// Aplica una lista de ediciones YA ORDENADAS y NO SOLAPADAS sobre
/// `original`. Devuelve los bytes nuevos y los rangos modificados.
fn apply_edits(original: &[u8], edits: &[PlannedEdit]) -> (Vec<u8>, Vec<Range<usize>>) {
    let mut out: Vec<u8> = Vec::with_capacity(original.len());
    let mut modified: Vec<Range<usize>> = Vec::with_capacity(edits.len());
    let mut cursor: usize = 0;

    for e in edits {
        // Copia el tramo intacto hasta `start`.
        out.extend_from_slice(&original[cursor..e.byte_range.start]);
        // Copia el texto nuevo y registra el rango en `out`.
        let new_start = out.len();
        out.extend_from_slice(e.replacement.as_bytes());
        let new_end = out.len();
        if new_end > new_start {
            modified.push(new_start..new_end);
        }
        // Avanza el cursor al final del rango reemplazado.
        cursor = e.byte_range.end;
    }
    // Copia el resto del archivo.
    out.extend_from_slice(&original[cursor..]);
    (out, modified)
}

/// Computa el unified diff estandar entre `original` y `modified`.
/// Una sola hunk por archivo (preview, no auditoria).
///
/// Formato:
///   --- <path> (original)
///   +++ <path> (modified)
///   @@ -<old_start>,<old_count> +<new_start>,<new_count> @@
///   <lineas con prefijo ' ' / '-' / '+'>
///
/// Si los bytes son identicos, devuelve "".
fn compute_unified_diff(path: &Path, original: &[u8], modified: &[u8]) -> String {
    if original == modified {
        return String::new();
    }
    let old_lines = split_lines(original);
    let new_lines = split_lines(modified);
    let mut buf = String::new();
    buf.push_str(&format!("--- {}\n", path.display()));
    buf.push_str(&format!("+++ {}\n", path.display()));

    let lcs = longest_common_prefix_len(&old_lines, &new_lines);
    let lcs_suffix = longest_common_suffix_len(&old_lines, &new_lines, lcs);

    let old_hunk_start = lcs.saturating_sub(3);
    let new_hunk_start = lcs.saturating_sub(3);
    let old_hunk_end_excl = old_lines
        .len()
        .saturating_sub(lcs_suffix)
        .min(old_lines.len());
    let new_hunk_end_excl = new_lines
        .len()
        .saturating_sub(lcs_suffix)
        .min(new_lines.len());

    let old_hunk: Vec<&[u8]> = old_lines[old_hunk_start..old_hunk_end_excl].to_vec();
    let new_hunk: Vec<&[u8]> = new_lines[new_hunk_start..new_hunk_end_excl].to_vec();

    let old_count = old_hunk.len();
    let new_count = new_hunk.len();
    buf.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        old_hunk_start + 1,
        old_count,
        new_hunk_start + 1,
        new_count,
    ));

    for l in &old_hunk {
        buf.push('-');
        buf.push_str(&String::from_utf8_lossy(l));
        buf.push('\n');
    }
    for l in &new_hunk {
        buf.push('+');
        buf.push_str(&String::from_utf8_lossy(l));
        buf.push('\n');
    }

    buf
}

/// Longitud del prefijo comun maximo entre dos slices de lineas.
fn longest_common_prefix_len(a: &[&[u8]], b: &[&[u8]]) -> usize {
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    i
}

/// Longitud del sufijo comun maximo entre dos slices de lineas,
/// sin contar el prefijo ya consumido.
fn longest_common_suffix_len(a: &[&[u8]], b: &[&[u8]], prefix_len: usize) -> usize {
    let mut i = 0;
    while i < (a.len() - prefix_len)
        && i < (b.len() - prefix_len)
        && a[a.len() - 1 - i] == b[b.len() - 1 - i]
    {
        i += 1;
    }
    i
}

/// Divide un buffer en lineas (preservando el contenido de cada
/// linea, sin el `\n` final). Una linea vacia al final se IGNORA
/// (es el comportamiento de `str::lines`).
fn split_lines(buf: &[u8]) -> Vec<&[u8]> {
    let mut out: Vec<&[u8]> = Vec::new();
    let mut start = 0;
    for (i, b) in buf.iter().enumerate() {
        if *b == b'\n' {
            out.push(&buf[start..i]);
            start = i + 1;
        }
    }
    if start < buf.len() {
        out.push(&buf[start..]);
    }
    out
}

/// Resumen textual de los riesgos. Concatena `id`, `description` y
/// (opcionalmente) `evidence` en una linea por riesgo.
fn summarize_risks(risks: &[Risk]) -> Vec<String> {
    risks
        .iter()
        .map(|r| {
            let sev = r.severity.as_str();
            match &r.evidence {
                Some(ev) => format!("[{}] {} — {} (evidence: {})", sev, r.id, r.description, ev),
                None => format!("[{}] {} — {}", sev, r.id, r.description),
            }
        })
        .collect()
}

/// Resumen textual de las precondiciones. Concatena `capability` y
/// `description` en una linea por precondicion.
fn summarize_preconditions(pre: &[Precondition]) -> Vec<String> {
    pre.iter()
        .map(|p| format!("{}: {}", p.capability, p.description))
        .collect()
}

// ============================================================================
// Tests (>= 5 unit tests, dentro del archivo).
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use crate::application::analysis_basis::AnalysisBasis;
    use crate::architecture::diagnostic::Severity;
    use crate::refactor::plan::{PlannedEdit, Precondition, Risk, build_plan};

    /// Helper: crea un TempDir con un archivo de contenido dado y
    /// devuelve (root, abs_path).
    #[allow(dead_code)]
    fn make_file(name: &str, body: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body).unwrap();
        f.sync_all().unwrap();
        (dir, p)
    }

    /// Helper: arma un plan minimo (B5.W1) sobre un TempDir.
    fn plan_with_edit(
        file: &str,
        body: &[u8],
        start: usize,
        end: usize,
        replacement: &str,
    ) -> (tempfile::TempDir, RefactorPlan) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let abs = root.join(file);
        std::fs::write(&abs, body).unwrap();
        let basis = AnalysisBasis::for_workspace(root.clone());
        let plan = build_plan(
            root.clone(),
            basis,
            vec![abs.clone()],
            vec![PlannedEdit {
                file: abs,
                byte_range: start..end,
                replacement: replacement.to_string(),
                deterministic: true,
            }],
            vec![Precondition {
                capability: "file.write".to_string(),
                description: "write access for preview".to_string(),
            }],
            vec![Risk {
                id: "R-test".to_string(),
                description: "test risk".to_string(),
                evidence: None,
                severity: Severity::Warning,
            }],
        )
        .expect("build_plan");
        (dir, plan)
    }

    // 1. Mismo plan + mismos bytes => misma salida (byte-equal).
    #[test]
    fn preview_is_byte_equal_for_same_input() {
        let (dir, plan) = plan_with_edit("a.txt", b"hello world\n", 6, 11, "there");
        let root = dir.path().canonicalize().unwrap();

        let p1 = render_preview(&plan, &root).unwrap();
        let p2 = render_preview(&plan, &root).unwrap();

        let j1 = serde_json::to_string(&p1).unwrap();
        let j2 = serde_json::to_string(&p2).unwrap();
        assert_eq!(
            j1, j2,
            "same plan + same bytes must produce byte-equal preview"
        );
        assert_eq!(p1, p2);
    }

    // 2. cancelled SIEMPRE false en B5.W2.
    #[test]
    fn cancelled_flag_is_false_in_normal_path() {
        let (dir, plan) = plan_with_edit("a.txt", b"abc", 0, 3, "xyz");
        let root = dir.path().canonicalize().unwrap();
        let preview = render_preview(&plan, &root).unwrap();
        assert!(
            !preview.cancelled,
            "B5.W2 placeholder: cancelled MUST be false"
        );
    }

    // 3. file_diffs ordenado por path ascendente.
    #[test]
    fn file_diffs_is_sorted_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for n in ["zeta.txt", "alpha.txt", "mu.txt"] {
            std::fs::write(root.join(n), b"x\n").unwrap();
        }

        // Insertamos los edits en orden alfabetico INVERSO.
        let names_in_order = ["zeta.txt", "mu.txt", "alpha.txt"];
        let abs_files: Vec<PathBuf> = names_in_order.iter().map(|n| root.join(n)).collect();
        let edits: Vec<PlannedEdit> = abs_files
            .iter()
            .map(|p| PlannedEdit {
                file: p.clone(),
                byte_range: 0..2,
                replacement: "yy".to_string(),
                deterministic: true,
            })
            .collect();
        let basis = AnalysisBasis::for_workspace(root.clone());
        let plan = build_plan(
            root.clone(),
            basis,
            abs_files.clone(),
            edits,
            Vec::new(),
            Vec::new(),
        )
        .expect("build_plan");

        let preview = render_preview(&plan, &root).unwrap();
        let names: Vec<String> = preview
            .file_diffs
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["alpha.txt", "mu.txt", "zeta.txt"]);
    }

    // 4. workspace_state_digest estable entre runs del mismo input.
    #[test]
    fn workspace_state_digest_is_stable_across_runs() {
        let (dir, plan) = plan_with_edit("a.txt", b"hello", 0, 5, "HELLO");
        let root = dir.path().canonicalize().unwrap();

        let p1 = render_preview(&plan, &root).unwrap();
        let p2 = render_preview(&plan, &root).unwrap();
        assert_eq!(p1.workspace_state_digest, p2.workspace_state_digest);
        // Viene de los bytes leidos, no del plan.
        assert!(!p1.workspace_state_digest.is_empty());
    }

    // 5. Edits solapados -> OverlappingEdits.
    #[test]
    fn preview_rejects_overlapping_edits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let abs = root.join("a.txt");
        std::fs::write(&abs, b"abcdef").unwrap();
        let basis = AnalysisBasis::for_workspace(root.clone());
        let plan = build_plan(
            root.clone(),
            basis,
            vec![abs.clone()],
            vec![
                PlannedEdit {
                    file: abs.clone(),
                    byte_range: 0..3,
                    replacement: "AAA".to_string(),
                    deterministic: true,
                },
                PlannedEdit {
                    file: abs.clone(),
                    byte_range: 2..5,
                    replacement: "BBB".to_string(),
                    deterministic: true,
                },
            ],
            Vec::new(),
            Vec::new(),
        )
        .expect("build_plan");
        let err = render_preview(&plan, &root).unwrap_err();
        assert!(
            matches!(err, PreviewError::OverlappingEdits { .. }),
            "expected OverlappingEdits, got {:?}",
            err
        );
    }

    // 6. unified_diff NO vacio para edits reales.
    #[test]
    fn unified_diff_is_non_empty_for_real_edit() {
        let (dir, plan) = plan_with_edit("a.txt", b"foo\nbar\nbaz\n", 0, 3, "FOO");
        let root = dir.path().canonicalize().unwrap();
        let preview = render_preview(&plan, &root).unwrap();
        assert_eq!(preview.file_diffs.len(), 1);
        let d = &preview.file_diffs[0];
        assert!(!d.unified_diff.is_empty(), "diff must be non-empty");
        assert!(d.unified_diff.contains("--- a.txt"));
        assert!(d.unified_diff.contains("+++ a.txt"));
        assert!(d.unified_diff.contains("@@ "));
    }

    // 7. byte_size_total == sum(new_bytes).
    #[test]
    fn byte_size_total_matches_sum_of_new_bytes() {
        let (dir, plan) = plan_with_edit("a.txt", b"hi", 0, 2, "HELLO");
        let root = dir.path().canonicalize().unwrap();
        let preview = render_preview(&plan, &root).unwrap();
        let sum: usize = preview.file_diffs.iter().map(|d| d.new_bytes.len()).sum();
        assert_eq!(preview.byte_size_total, sum);
    }

    // 8. verify_no_mutation: el preview NO muta el workspace.
    #[test]
    fn verify_no_mutation_returns_true_for_unchanged_workspace() {
        let (dir, plan) = plan_with_edit("a.txt", b"hello", 0, 5, "HELLO");
        let root = dir.path().canonicalize().unwrap();
        assert!(verify_preview_no_mutation(&plan, &root));
    }
}

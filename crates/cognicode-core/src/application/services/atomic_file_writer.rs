//! Una escritura que deja el fichero entero o no lo deja.
//!
//! El destino no se toca hasta que el contenido nuevo está escrito y
//! durable; el cambio de nombre es lo que lo publica. Si algo falla antes,
//! el temporal desaparece y el destino sigue como estaba.
//!
//! Tres garantías, y las tres se sostienen sin depender de que alguien se
//! acuerde de limpiar:
//!
//! - El temporal se **crea en exclusiva**: un nombre que ya existe no se
//!   abre ni se trunca, se busca el siguiente.
//! - El temporal **se borra solo** en cualquier camino que no llegue al
//!   cambio de nombre, porque la limpieza está en `Drop`.
//! - El contenido está **durable** antes de que exista el destino: se
//!   sincroniza el fichero y, después, el directorio.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Cuántos nombres prueba el escritor antes de rendirse.
///
/// Cada intento es un nombre distinto; el contador está en el proceso, así
/// que el número de intentos está acotado y el error que sale dice por qué.
const MAX_TEMP_ATTEMPTS: u32 = 16;

/// El nombre del temporal para un intento dado.
///
/// El nombre lleva el identificador del proceso, de modo que dos procesos
/// distintos no se pisan, y el intento, de modo que dos escrituras del
/// mismo proceso tampoco.
fn temp_path_for(target: &Path, attempt: u32) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(format!(".tmp.{}", std::process::id()));
    if attempt > 0 {
        name.push(format!(".{attempt}"));
    }
    PathBuf::from(name)
}

/// Un fichero que se escribe entero o no se escribe.
///
/// El temporal se crea en exclusiva, se escribe, se sincroniza, y sólo
/// entonces el cambio de nombre lo publica. Si el escritor se suelta sin
/// publicar —porque una escritura falló, o porque el cambio de nombre
/// falló—, el temporal se borra.
pub(crate) struct AtomicFileWriter {
    target: PathBuf,
    temp: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl AtomicFileWriter {
    /// Prepara la escritura: un temporal junto al destino, en exclusiva.
    ///
    /// Si ya hay un fichero con ese nombre, no se abre: se busca el
    /// siguiente nombre libre, y el fichero que estaba ahí conserva su
    /// contenido.
    pub(crate) fn create(target: &Path) -> io::Result<Self> {
        for attempt in 0..MAX_TEMP_ATTEMPTS {
            let temp = temp_path_for(target, attempt);
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => {
                    return Ok(Self {
                        target: target.to_path_buf(),
                        temp,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "no free temporary name for {} after {} attempts",
                target.display(),
                MAX_TEMP_ATTEMPTS
            ),
        ))
    }

    /// Escribe el contenido y lo hace durable.
    ///
    /// La sincronización va aquí, antes de que el destino exista: cuando el
    /// cambio de nombre llegue, el contenido ya está en el disco.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("the temporary file is already closed"))?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    /// Publica el temporal con un cambio de nombre y sincroniza el
    /// directorio.
    ///
    /// El cambio de nombre es lo que hace que el destino cambie de golpe:
    /// nadie ve un fichero a medio escribir. La sincronización del
    /// directorio es lo que hace que el cambio sobreviva a un corte de luz.
    pub(crate) fn commit(mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.sync_all()?;
        }
        // The handle is closed before the rename: on some platforms a file
        // that is still open cannot be replaced.
        self.file = None;
        fs::rename(&self.temp, &self.target)?;
        self.committed = true;
        sync_directory(&self.target);
        Ok(())
    }
}

impl Drop for AtomicFileWriter {
    /// Lo que no se publicó no se queda en el disco.
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.temp);
        }
    }
}

/// Sincroniza el directorio que contiene al destino.
///
/// Es lo que hace que el cambio de nombre sobreviva a un corte de luz: el
/// nombre nuevo tiene que estar en el disco, no sólo el contenido. En
/// plataformas donde el directorio no se puede abrir para lectura, la
/// sincronización se salta: el contenido queda durable, el nombre puede
/// tardar.
fn sync_directory(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn target_in(dir: &TempDir) -> PathBuf {
        dir.path().join("destino.txt")
    }

    #[test]
    fn el_temporal_se_crea_en_exclusiva_y_no_trunca_lo_que_ya_estaba() {
        let dir = TempDir::new().expect("a temporary directory");
        let target = target_in(&dir);

        // A file that is already there with the name the writer wants.
        let squatted = temp_path_for(&target, 0);
        fs::write(&squatted, "PRECIOUS").expect("the squatted file is written");

        let writer = AtomicFileWriter::create(&target).expect("the writer is created");
        assert_ne!(
            writer.temp,
            squatted.as_path(),
            "the writer must not use a name that is already taken"
        );
        assert_eq!(
            fs::read_to_string(&squatted).expect("the squatted file is readable"),
            "PRECIOUS",
            "a file that was already there must not be truncated"
        );
        writer.commit().expect("the write is published");
    }

    #[test]
    fn lo_que_no_se_publica_no_se_queda_en_el_disco() {
        let dir = TempDir::new().expect("a temporary directory");
        let target = target_in(&dir);

        let temp = {
            let mut writer = AtomicFileWriter::create(&target).expect("the writer is created");
            writer.write(b"contenido").expect("the content is written");
            writer.temp.to_path_buf()
        };

        assert!(
            !temp.exists(),
            "a writer that is dropped without publishing must leave nothing behind"
        );
        assert!(!target.exists(), "the target was never published");
    }

    #[test]
    fn un_cambio_de_nombre_fallido_no_deja_temporal() {
        let dir = TempDir::new().expect("a temporary directory");
        // The target is a directory, so the rename cannot succeed.
        let target = dir.path().join("ocupado");
        fs::create_dir(&target).expect("the directory is created");

        let mut writer = AtomicFileWriter::create(&target).expect("the writer is created");
        let temp = writer.temp.to_path_buf();
        writer.write(b"contenido").expect("the content is written");
        assert!(
            writer.commit().is_err(),
            "renaming a file over a directory must fail"
        );
        assert!(!temp.exists(), "the temporary file must be gone");
    }

    #[test]
    fn publicar_deja_el_destino_con_el_contenido_y_sin_temporal() {
        let dir = TempDir::new().expect("a temporary directory");
        let target = target_in(&dir);

        let mut writer = AtomicFileWriter::create(&target).expect("the writer is created");
        let temp = writer.temp.to_path_buf();
        writer.write(b"contenido").expect("the content is written");
        writer.commit().expect("the write is published");

        assert_eq!(
            fs::read_to_string(&target).expect("the target is readable"),
            "contenido"
        );
        assert!(!temp.exists(), "the temporary file must be gone");
    }

    #[test]
    fn un_destino_que_no_existe_todavia_se_puede_publicar() {
        let dir = TempDir::new().expect("a temporary directory");
        let target = target_in(&dir);

        let mut writer = AtomicFileWriter::create(&target).expect("the writer is created");
        writer.write(b"nuevo").expect("the content is written");
        writer.commit().expect("the write is published");

        assert_eq!(
            fs::read_to_string(&target).expect("the target is readable"),
            "nuevo"
        );
    }
}

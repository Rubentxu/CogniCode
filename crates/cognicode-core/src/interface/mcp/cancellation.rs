//! El registro de cancelaciones, una por peticion.
//!
//! Una cancelacion pertenece a la peticion que la pidio. Este modulo es el
//! unico sitio donde se guarda esa pertenencia: una tabla que asocia el
//! identificador de la peticion con su propia bandera de cancelacion.
//!
//! La tabla se vacia sola. Cuando una peticion termina, su entrada
//! desaparece, de modo que una notificacion de cancelacion tardia —que el
//! protocolo admite— no encuentra a quien marcar: no hay a quien cancelar
//! y la peticion siguiente no hereda nada.
//!
//! Antes, el servidor guardaba una sola bandera para toda la sesion y la
//! ponia a `true` para siempre. Una cancelacion envenenaba la sesion entera:
//! toda peticion posterior se anunciaba como cancelada sin que nadie la
//! hubiera cancelado.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rmcp::model::RequestId;

/// La bandera de cancelacion de una peticion.
///
/// Se comparte entre quien la crea y quien la marca, y por eso es un
/// `Arc`: el manejador la lee sin conocer el registro.
pub type CancellationFlag = Arc<AtomicBool>;

/// The live requests and each one's flag.
#[derive(Debug, Clone, Default)]
pub struct CancellationRegistry {
    entries: Arc<Mutex<HashMap<RequestId, CancellationFlag>>>,
}

impl CancellationRegistry {
    /// Un registro vacio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra una peticion viva con su bandera y devuelve el guardia
    /// que borra la entrada cuando la peticion termina.
    ///
    /// La bandera la crea quien pide el registro: es la misma que lleva el
    /// contexto de la peticion, y por eso quien la marca y quien la lee
    /// estan mirando la misma variable y no dos que se parecen.
    pub fn register(&self, request_id: RequestId, flag: CancellationFlag) -> Registration {
        self.entries().insert(request_id.clone(), flag);
        Registration {
            entries: self.entries.clone(),
            request_id,
        }
    }

    /// Marca la peticion indicada como cancelada.
    ///
    /// Devuelve si habia una peticion viva con ese identificador. Una
    /// notificacion tardia —la que llega cuando la peticion ya termino— no
    /// encuentra a nadie y no hace nada.
    pub fn cancel(&self, request_id: &RequestId) -> bool {
        let entries = self.entries();
        match entries.get(request_id) {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Si la peticion sigue viva en el registro.
    pub fn is_registered(&self, request_id: &RequestId) -> bool {
        self.entries().contains_key(request_id)
    }

    /// Cuantas peticiones siguen registradas.
    pub fn registered_count(&self) -> usize {
        self.entries().len()
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<RequestId, CancellationFlag>> {
        // El lock solo protege estas operaciones, que no ejecutan codigo
        // ajeno: que se envenene exige un panic dentro, y ese panic seria
        // de otro. Recuperarlo deja el registro en pie.
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// El guardia que borra el registro de una peticion cuando esta termina.
///
/// Vive junto al despacho: al caer, la peticion ya no esta en vuelo y su
/// cancelacion deja de existir.
#[derive(Debug)]
pub struct Registration {
    entries: Arc<Mutex<HashMap<RequestId, CancellationFlag>>>,
    request_id: RequestId,
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.request_id);
    }
}

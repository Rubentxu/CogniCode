//! El recorrido del árbol que hace la búsqueda de contenido, y el hilo
//! en el que ocurre.
//!
//! ## Por qué tiene su propia casa
//!
//! La búsqueda recorre el árbol y lee cada fichero: cientos de
//! milisegundos de disco que nunca ceden. Los handlers son `async`, así
//! que hacer eso en la tarea que los sirve retiene el hilo del runtime
//! mientras dura, y el servidor no contesta a nadie meanwhile —ni a una
//! llamada que no necesita disco—.
//!
//! Peor: el plazo que debería cortar la llamada no puede dispararse
//! mientras el sondeo está metido en la búsqueda: un plazo se observa
//! cuando se vuelve a sondear el futuro, y el sondeo está dentro de la
//! búsqueda hasta que termina. La respuesta llega entonces después del
//! plazo que debía sustituirla, y el llamante se queda con un timeout y
//! con una búsqueda cuyo trabajo se tiró.
//!
//! ## Qué vive aquí
//!
//! El movimiento de la búsqueda fuera del runtime, y el único camino
//! para hacerlo. Un handler que llamara a la aplicación por su cuenta
//! traería la parada de vuelta.

use crate::application::dto::{SearchContentRequest, SearchContentResult};
use crate::application::error::{AppError, AppResult};
use crate::application::services::file_operations::FileOperationsService;
use std::sync::Arc;

impl FileOperationsService {
    /// La búsqueda del contenido del workspace, fuera del runtime que la
    /// sirve.
    ///
    /// El recorrido es el mismo que el servicio siempre hizo; lo que
    /// cambia es el hilo en el que ocurre. El llamante entrega el
    /// servicio y la petición, y recibe la respuesta cuando el
    /// recorrido ha terminado.
    pub(crate) async fn search_content_off_the_runtime(
        self: &Arc<Self>,
        input: SearchContentRequest,
    ) -> AppResult<SearchContentResult> {
        let service = Arc::clone(self);
        tokio::task::spawn_blocking(move || service.search_content(input))
            .await
            .map_err(|e| AppError::InternalError(format!("the content search panicked: {e}")))?
    }
}

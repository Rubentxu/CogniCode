//! La marca de tiempo de un fichero, tal y como el producto puede decirla.
//!
//! ## Por que vive aqui
//!
//! Medir una fecha es un hecho del dominio: dice cuando se escribio el
//! fichero, no cuando lo lee el sistema de ficheros. El modulo vive en el
//! dominio porque el dominio es quien decide que significa "no hay fecha".
//!
//! ## La distincion que el tipo obliga a hacer
//!
//! Hay tres hechos distintos, y se confundian en uno:
//!
//! - el fichero se escribio en un instante medido, que puede ser anterior
//!   a la epoca;
//! - el fichero se escribio exactamente en la epoca;
//! - el sistema de ficheros no publico ninguna fecha.
//!
//! Los tres se publicaban como `0`. El segundo es una medicion; el tercero
//! no es una medicion, y por eso no puede vestirse de una. De ahi el tipo
//! `Option` con signo: `None` es la ausencia de una fecha, `Some(0)` es la
//! epoca, y `Some(-1)` es un instante anterior a ella. Tres hechos, tres
//! representaciones, y el consumidor puede ver cual es cual.
//!
//! El signo no es decorativo: un fichero puede traer una fecha anterior a
//! la epoca —un tar que conserva sus marcas de tiempo, una copia de
//! seguridad restaurada— y un contador sin signo no la puede representar.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milisegundos desde la epoca UNIX, con signo.
///
/// Un instante anterior a la epoca es un numero negativo, no un numero
/// pequeño: la distancia a la epoca no es la fecha.
pub fn epoch_millis(instant: SystemTime) -> i64 {
    let (sign, delta) = match instant.duration_since(UNIX_EPOCH) {
        Ok(delta) => (1i64, delta),
        Err(before) => (-1i64, before.duration()),
    };
    let millis = i64::try_from(delta.as_millis()).unwrap_or(i64::MAX);
    sign.saturating_mul(millis)
}

/// Nanoseconds desde la epoca UNIX, con signo.
pub fn epoch_nanos(instant: SystemTime) -> i64 {
    let (sign, delta) = match instant.duration_since(UNIX_EPOCH) {
        Ok(delta) => (1i64, delta),
        Err(before) => (-1i64, before.duration()),
    };
    let nanos = i64::try_from(delta.as_nanos()).unwrap_or(i64::MAX);
    sign.saturating_mul(nanos)
}

/// Segundos desde la epoca UNIX, con signo.
///
/// El corte es hacia el suelo: un instante que cae dentro del segundo
/// anterior a la epoca cuenta como `-1`, no como `0`. Si contara como `0`
/// seria indistinguible de la epoca, que es justo la confusion que este
/// modulo existe para evitar.
pub fn epoch_seconds(instant: SystemTime) -> i64 {
    floor_to_secs(epoch_millis(instant))
}

/// La medida de una fecha, o ninguna si no hay fecha que medir.
///
/// `None` es la ausencia de una medida. Un `Some(0)` es un fichero escrito
/// en la epoca, que es una medida; los dos no son lo mismo.
pub fn measured_millis(instant: Option<SystemTime>) -> Option<i64> {
    instant.map(epoch_millis)
}

/// La medida de una fecha en nanosegundos, o ninguna si no hay fecha que
/// medir.
pub fn measured_nanos(instant: Option<SystemTime>) -> Option<i64> {
    instant.map(epoch_nanos)
}

/// La medida de una fecha en segundos, o ninguna si no hay fecha que medir.
pub fn measured_seconds(instant: Option<SystemTime>) -> Option<i64> {
    instant.map(epoch_seconds)
}

/// Segundos de una medida ya expresada en milisegundos.
///
/// El corte hacia el suelo vive aqui y en ningun otro sitio, para que las
/// dos rutas que llevan la medida —el reloj y la medida ya hecha— digan lo
/// mismo del mismo instante.
pub fn seconds_of_millis(millis: Option<i64>) -> Option<i64> {
    millis.map(floor_to_secs)
}

/// El corte hacia el suelo de una medida en milisegundos.
fn floor_to_secs(millis: i64) -> i64 {
    millis.div_euclid(1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_date_that_does_not_exist_is_not_the_epoch() {
        assert_eq!(measured_seconds(None), None);
        assert_eq!(measured_millis(None), None);
    }

    #[test]
    fn the_epoch_is_a_measured_date() {
        assert_eq!(measured_seconds(Some(UNIX_EPOCH)), Some(0));
        assert_eq!(measured_millis(Some(UNIX_EPOCH)), Some(0));
    }

    #[test]
    fn a_date_before_the_epoch_keeps_its_sign() {
        let before = UNIX_EPOCH - Duration::from_secs(60);
        assert_eq!(measured_seconds(Some(before)), Some(-60));
        assert_eq!(measured_millis(Some(before)), Some(-60_000));
    }

    #[test]
    fn a_date_after_the_epoch_is_positive() {
        let after = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(measured_seconds(Some(after)), Some(1_700_000_000));
        assert_eq!(measured_millis(Some(after)), Some(1_700_000_000_000));
    }

    #[test]
    fn a_date_inside_the_second_before_the_epoch_is_not_the_epoch() {
        // Medio segundo antes de la epoca. El corte hacia el suelo lo
        // cuenta como -1, para que no se confunda con la epoca.
        let before = UNIX_EPOCH - Duration::from_millis(500);
        assert_eq!(measured_seconds(Some(before)), Some(-1));
    }

    #[test]
    fn a_measure_already_in_millis_says_the_same_as_the_clock() {
        let before = UNIX_EPOCH - Duration::from_millis(500);
        assert_eq!(
            seconds_of_millis(measured_millis(Some(before))),
            measured_seconds(Some(before)),
            "las dos rutas que llevan la medida tienen que decir lo mismo"
        );
    }
}

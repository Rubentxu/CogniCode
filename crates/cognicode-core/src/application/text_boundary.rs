//! Fronteras de caracter en texto codificado en UTF-8.
//!
//! Un corte por indice de byte parte un caracter cuando el indice cae dentro
//! de la codificacion multibyte de ese caracter. Estas funciones son la via
//! por la que el producto recorta texto: todas devuelven un corte que cae
//! entre dos caracteres, nunca dentro de uno.
//!
//! El panico que evita este modulo no es teorico: un corte por indice de
//! byte sobre texto con caracteres multibyte aborta el proceso que lo
//! intenta, y en un servidor de protocolo ese proceso es el que atiende las
//! peticiones de todos los agentes conectados.

/// El mayor indice que no supera `index` y cae entre dos caracteres.
///
/// Un indice mayor que la longitud del texto se recorta a la longitud; un
/// indice dentro de un caracter retrocede hasta el inicio de ese caracter.
pub fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Si `index` cae entre dos caracteres del texto.
///
/// El final del texto es una frontera valida: un corte ahi no parte nada.
pub fn is_char_boundary(text: &str, index: usize) -> bool {
    index <= text.len() && text.is_char_boundary(index)
}

/// Recorta `text` a como mucho `max_bytes` bytes sin partir un caracter.
///
/// Devuelve el prefijo mas largo que no supera `max_bytes` y que termina
/// entre dos caracteres. El resultado puede medir menos de `max_bytes`
/// porque el caracter siguiente no cabia entero.
pub fn truncate_at_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    &text[..floor_char_boundary(text, max_bytes)]
}

/// Corta `text` desde `start` hasta `end`, o `None` si el corte parte un caracter.
///
/// A diferencia de [`truncate_at_boundary`], aqui los dos extremos son
/// decisiones del llamante: si uno de ellos cae dentro de un caracter, no
/// hay un recorte obvio que los conserve a los dos, asi que se devuelve
/// `None` y deja que quien llama decida.
pub fn slice_at_boundaries(text: &str, start: usize, end: usize) -> Option<&str> {
    if start > end || end > text.len() {
        return None;
    }
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dos bytes: la 'e' acentuada ocupa los bytes 0..2.
    const DOS_BYTES: &str = "é";
    /// Cuatro bytes: el emoji ocupa los bytes 0..4.
    const CUATRO_BYTES: &str = "😀";

    #[test]
    fn floor_lands_before_the_character_it_was_inside() {
        assert_eq!(floor_char_boundary(DOS_BYTES, 0), 0);
        assert_eq!(floor_char_boundary(DOS_BYTES, 1), 0);
        assert_eq!(floor_char_boundary(DOS_BYTES, 2), 2);
        assert_eq!(floor_char_boundary(CUATRO_BYTES, 1), 0);
        assert_eq!(floor_char_boundary(CUATRO_BYTES, 3), 0);
        assert_eq!(floor_char_boundary(CUATRO_BYTES, 4), 4);
    }

    #[test]
    fn floor_clamps_an_index_past_the_end() {
        assert_eq!(floor_char_boundary(DOS_BYTES, 99), DOS_BYTES.len());
    }

    #[test]
    fn boundary_agrees_with_the_text() {
        assert!(is_char_boundary(DOS_BYTES, 0));
        assert!(!is_char_boundary(DOS_BYTES, 1));
        assert!(is_char_boundary(DOS_BYTES, 2));
        assert!(!is_char_boundary(DOS_BYTES, 3));
    }

    #[test]
    fn truncation_never_splits_a_character() {
        assert_eq!(truncate_at_boundary("héllo", 2), "h");
        assert_eq!(truncate_at_boundary("héllo", 3), "hé");
        assert_eq!(truncate_at_boundary("héllo", 10), "héllo");
        assert_eq!(truncate_at_boundary("😀x", 1), "");
    }

    #[test]
    fn a_slice_inside_a_character_is_refused() {
        assert_eq!(slice_at_boundaries("héllo", 0, 3), Some("hé"));
        assert_eq!(slice_at_boundaries("héllo", 0, 2), None);
        assert_eq!(slice_at_boundaries("héllo", 3, 1), None);
        assert_eq!(slice_at_boundaries("héllo", 0, 99), None);
    }
}

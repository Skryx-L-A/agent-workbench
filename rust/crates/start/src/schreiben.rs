use einstellungen::json::text_schreiben;

use crate::Startdatei;
use crate::form::Startform;

/// Writes a start file in the compact form that [`crate::lesen_datei`] reads back unchanged, in the version it was read
/// in: version 1 without `effort`, `name` and `fortsetzung_von`, version 2 with all three (`null` where unset).
#[must_use]
pub fn schreiben(start: &Startdatei) -> Vec<u8> {
    let kunde = oder_null(start.kunde());
    let zwei = if start.fassung() == 2 {
        format!(
            ",\"effort\":{},\"name\":{},\"fortsetzung_von\":{}",
            oder_null(start.effort()),
            oder_null(start.name()),
            oder_null(start.fortsetzung_von()),
        )
    } else {
        String::new()
    };
    let marker = start.marker().map_or_else(String::new, |marker| {
        let liste: Vec<String> = marker.liste().iter().map(|m| text_schreiben(m)).collect();
        format!(
            ",\"marker\":{{\"liste\":[{}],\"ordner\":{}}}",
            liste.join(","),
            text_schreiben(marker.ordner()),
        )
    });
    format!(
        "{{\"fassung\":{},\"sitzung\":{},\"harness\":{},\"modell\":{},\"route\":{},\"kunde\":{},\"arbeitsordner\":{}{}{}}}",
        start.fassung(),
        text_schreiben(start.sitzung()),
        text_schreiben(start.harness()),
        text_schreiben(start.modell()),
        text_schreiben(start.route()),
        kunde,
        text_schreiben(start.arbeitsordner()),
        zwei,
        marker,
    )
    .into_bytes()
}

fn oder_null(wert: Option<&str>) -> String {
    wert.map_or_else(|| "null".to_owned(), text_schreiben)
}

// Zeilenform (Vertrag `rasterauszug`, Abschnitt 4: Laeufe `[fg, bg, fl, text]`) nach DOM-Knoten.
//
// Der Text eines Laufs wird nur als Textknoten eingefuegt (`createTextNode`), nie als HTML zusammengesetzt; `<`, `>`, `&` und jedes
// Steuerzeichen bleiben Text. Farben und Stile sind CSS-Klassen (Datei `raster.css`):
//   Vordergrund `f-N`, Hintergrund `b-N` mit N von 0 bis 255 (Palette: Farbe c von 1 bis 256 ist N = c - 1; 0 bis 15 die benannten Farben),
//   Attribute `fett`, `dimm`, `kursiv`, `unterstrichen`, `verdeckt`, `durchgestrichen` (Bits 1, 2, 4, 8, 32, 64).
//   Invers (Bit 16) tauscht Vordergrund und Hintergrund; die Vorgabefarben heissen dann `f-grund` (Grundfarbe des Hintergrunds als
//   Schrift) und `b-vorn` (Schriftfarbe als Hintergrund).
// Truecolor (0x01000000 | rgb) kennt keine endliche Klassenmenge; dafuer setzt die Funktion `color` oder `background-color` ueber
// `style.setProperty` (CSSOM, kein HTML-Text) auf den Wert `#rrggbb`.
// Ein Lauf im Vorgabestil ist ein blosser Textknoten ohne `span`.

export const ATTRIBUTE = [
  [1, 'fett'],
  [2, 'dimm'],
  [4, 'kursiv'],
  [8, 'unterstrichen'],
  [32, 'verdeckt'],
  [64, 'durchgestrichen'],
];
const INVERS = 16;
const TRUECOLOR = 0x01000000;

function hex(c) {
  return '#' + (c & 0xffffff).toString(16).padStart(6, '0');
}

// Klassen und Inline-Farben eines Stils: { klassen: [..], farbe: '#rrggbb' | null, grund: '#rrggbb' | null }
export function stilKlassen(fg, bg, fl) {
  const klassen = [];
  for (const [bit, name] of ATTRIBUTE) if (fl & bit) klassen.push(name);
  let vorn = fg;
  let grund = bg;
  if (fl & INVERS) {
    // Tausch: die Schrift bekommt die Farbe des Hintergrunds, der Grund die der Schrift; eine Vorgabe bleibt eine Vorgabe der anderen Seite.
    vorn = bg;
    grund = fg;
    if (vorn === 0) klassen.push('f-grund');
    if (grund === 0) klassen.push('b-vorn');
  }
  let farbe = null;
  let hintergrund = null;
  if (vorn >= TRUECOLOR) farbe = hex(vorn);
  else if (vorn > 0) klassen.push(`f-${vorn - 1}`);
  if (grund >= TRUECOLOR) hintergrund = hex(grund);
  else if (grund > 0) klassen.push(`b-${grund - 1}`);
  return { klassen, farbe, grund: hintergrund };
}

// `zeile`: Liste von Laeufen. Liefert ein DocumentFragment (eine leere Zeile ergibt ein leeres Fragment).
// `doc` ist das Dokument, das die Knoten erzeugt; Vorgabe ist `document`.
export function zeileZuHtml(zeile, doc = globalThis.document) {
  const frag = doc.createDocumentFragment();
  for (const [fg, bg, fl, text] of zeile) {
    const textKnoten = doc.createTextNode(text);
    if (fg === 0 && bg === 0 && fl === 0) {
      frag.appendChild(textKnoten);
      continue;
    }
    const span = doc.createElement('span');
    const { klassen, farbe, grund } = stilKlassen(fg, bg, fl);
    if (klassen.length) span.className = klassen.join(' ');
    if (farbe) span.style.setProperty('color', farbe);
    if (grund) span.style.setProperty('background-color', grund);
    span.appendChild(textKnoten);
    frag.appendChild(span);
  }
  return frag;
}

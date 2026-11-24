// Erzeugt von buendeln.mjs aus den Modulen in rust/seite/; nicht von Hand aendern.

// ---- inflate.js ----
const __m_inflate = (() => {
// Deflate-Entpacker (RFC 1951) fuer die Rasterrahmen, ohne Abhaengigkeiten.
//
// Warum nicht `DecompressionStream('deflate-raw')`: Der Vertrag `rasterauszug` (RA11) verlangt je Rahmen genau die Bytes des Rahmens
// zurueck, einen Fehlercode je Rahmen, einen Abbruch bei mehr als 16 MiB Ausgabe und denselben Kontext ueber alle Rahmen einer Verbindung.
// Der Strom-Entpacker meldet kein Ende der Ausgabe eines Rahmens (gemessen, Node 26: `write()` kehrt zurueck, bevor die Ausgabe sichtbar
// ist; sie kommt erst spaeter), er kennt keine Obergrenze je Rahmen und seine Fehler tragen keinen Text. Dieser Entpacker ist
// gleichzeitig synchron, hat die Obergrenze und prueft den Rueckverweis wie zlib (ein Abstand ueber den Anfang des Stroms hinaus ist
// ein Fehler, "invalid distance too far back"). Er haelt den Kontext (32 KiB Verlauf, offener Block, ungelesene Bits) zwischen den Aufrufen.
//
// Fehler: `InflateFehler` mit `code` gleich `inflate_fehler` (ungueltiger Strom) oder `rahmen_zu_gross` (mehr als `max` Byte Ausgabe).
// Nach einem dieser Fehler bleibt der Entpacker unbrauchbar und wirft den Fehler erneut (wie zlib im Zustand BAD).

const MAX_RAHMEN = 16777216;

class InflateFehler extends Error {
  constructor(code, detail) {
    super(detail ? `${code}: ${detail}` : code);
    this.code = code;
    this.detail = detail || '';
  }
}

const FENSTER = 32768;
const UNGUELTIG = 0x7ff; // Symbolwert eines nicht belegten Codes in der Tabelle
const LBASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const LAENGEN_REIHE = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
const CODES = 0;
const LENS = 1;
const NEED = { need: true }; // nicht genug Bits (nur im Kopf eines dynamischen Blocks geworfen)

function umkehren(code, len) {
  let r = 0;
  for (let i = 0; i < len; i++) {
    r = (r << 1) | (code & 1);
    code >>= 1;
  }
  return r;
}

// Baut die Nachschlagetabelle eines Huffman-Codes. Die Pruefungen sind die von zlib (`inflate_table`):
// uebervoll ist immer ein Fehler; unvollstaendig nur bei LENS/DISTS mit genau einem Code der Laenge 1.
function baue(lens, n, typ) {
  const count = new Uint16Array(16);
  for (let i = 0; i < n; i++) count[lens[i]]++;
  let max = 15;
  while (max >= 1 && count[max] === 0) max--;
  if (max === 0) {
    // kein Symbol: zlib legt einen Eintrag an, der bei CODES als Symbol 0 mit einem Bit gilt, sonst ungueltig ist
    return { tab: new Int32Array(2).fill(((typ === CODES ? 0 : UNGUELTIG) << 5) | 1), bits: 1 };
  }
  let left = 1;
  for (let len = 1; len <= 15; len++) {
    left <<= 1;
    left -= count[len];
    if (left < 0) throw new InflateFehler('inflate_fehler', 'over-subscribed code');
  }
  if (left > 0 && (typ === CODES || max !== 1)) throw new InflateFehler('inflate_fehler', 'incomplete code');
  const offs = new Uint16Array(16);
  for (let len = 1; len < 15; len++) offs[len + 1] = offs[len] + count[len];
  const sortiert = new Uint16Array(n);
  for (let sym = 0; sym < n; sym++) if (lens[sym]) sortiert[offs[lens[sym]]++] = sym;
  const groesse = 1 << max;
  const tab = new Int32Array(groesse).fill((UNGUELTIG << 5) | 1);
  let code = 0;
  let k = 0;
  for (let len = 1; len <= max; len++) {
    for (let c = count[len]; c > 0; c--) {
      const sym = sortiert[k++];
      const eintrag = (sym << 5) | len;
      for (let j = umkehren(code, len); j < groesse; j += 1 << len) tab[j] = eintrag;
      code++;
    }
    code <<= 1;
  }
  return { tab, bits: max };
}

let festeTabellen = null;
function feste() {
  if (!festeTabellen) {
    const l = new Uint8Array(288);
    l.fill(8, 0, 144);
    l.fill(9, 144, 256);
    l.fill(7, 256, 280);
    l.fill(8, 280, 288);
    festeTabellen = { lit: baue(l, 288, LENS), dist: baue(new Uint8Array(32).fill(5), 32, LENS) };
  }
  return festeTabellen;
}

class Inflater {
  constructor() {
    this.modus = 'kopf'; // 'kopf' (naechster Blockkopf), 'gespeichert', 'huffman', 'ende' (letzter Block gelesen)
    this.letzter = false;
    this.rest = 0; // Byte des gespeicherten Blocks, die noch fehlen
    this.lit = null;
    this.dist = null;
    this.verlauf = new Uint8Array(0); // letzte bis zu 32 KiB Ausgabe
    this.offen = new Uint8Array(0); // ungelesene Eingabe ab dem letzten sicheren Punkt
    this.bitVersatz = 0; // schon gelesene Bits im ersten Byte von `offen`
    this.defekt = null; // Fehlercode nach einem Fehler
  }

  // Eine unabhaengige Kopie des Zustands (die Puffer werden nie an Ort und Stelle geaendert). Fuer Proben mit beschaedigten Rahmen.
  klon() {
    return Object.assign(new Inflater(), this);
  }

  // Entpackt `eingabe` (Teil des Stroms, auch ein Rahmen samt angehaengtem 00 00 ff ff) und liefert die Ausgabe, die sich daraus
  // vollstaendig ergibt. Mehr als `max` Byte Ausgabe: Fehler `rahmen_zu_gross`.
  inflate(eingabe, max = MAX_RAHMEN) {
    if (this.defekt) throw new InflateFehler(this.defekt, 'Entpacker unbrauchbar');
    if (this.modus === 'ende') return new Uint8Array(0);
    let daten = eingabe;
    if (this.offen.length) {
      daten = new Uint8Array(this.offen.length + eingabe.length);
      daten.set(this.offen, 0);
      daten.set(eingabe, this.offen.length);
    }
    const gesamtBits = daten.length * 8;
    let bp = this.bitVersatz;
    const h0 = this.verlauf.length;
    let w = new Uint8Array(h0 + Math.min(max + 1, Math.max(1024, eingabe.length * 4)) + 258);
    w.set(this.verlauf, 0);
    let pos = h0;
    const sicher = (n) => {
      if (pos + n > w.length) {
        const neu = new Uint8Array(Math.min(w.length * 2, h0 + max + 1 + 258 + 65536) + n);
        neu.set(w.subarray(0, pos), 0);
        w = neu;
      }
    };
    const peek = (n) => {
      const i = bp >> 3;
      return (((daten[i] | (daten[i + 1] << 8) | (daten[i + 2] << 16)) >>> (bp & 7)) & ((1 << n) - 1));
    };
    const lies = (n) => {
      if (gesamtBits - bp < n) throw NEED;
      const v = peek(n);
      bp += n;
      return v;
    };
    const decodiere = (t) => {
      const e = t.tab[peek(t.bits)];
      const l = e & 31;
      if (l > gesamtBits - bp) throw NEED;
      bp += l;
      return e >> 5;
    };
    const fehler = (text) => {
      this.defekt = 'inflate_fehler';
      return new InflateFehler('inflate_fehler', text);
    };
    const zuGross = () => {
      this.defekt = 'rahmen_zu_gross';
      return new InflateFehler('rahmen_zu_gross', `mehr als ${max} Byte`);
    };

    {
      for (;;) {
        if (this.modus === 'kopf') {
          const start = bp;
          try {
            const letzter = lies(1);
            const typ = lies(2);
            if (typ === 0) {
              bp = (bp + 7) & ~7;
              const len = lies(16);
              const nlen = lies(16);
              if ((len ^ 0xffff) !== nlen) throw fehler('invalid stored block lengths');
              this.rest = len;
              this.modus = 'gespeichert';
            } else if (typ === 1) {
              const f = feste();
              this.lit = f.lit;
              this.dist = f.dist;
              this.modus = 'huffman';
            } else if (typ === 2) {
              const nlen = lies(5) + 257;
              const ndist = lies(5) + 1;
              const ncode = lies(4) + 4;
              if (nlen > 286 || ndist > 30) throw fehler('too many length or distance symbols');
              const cl = new Uint8Array(19);
              for (let i = 0; i < ncode; i++) cl[LAENGEN_REIHE[i]] = lies(3);
              let ct;
              try {
                ct = baue(cl, 19, CODES);
              } catch (e) {
                throw fehler('invalid code lengths set');
              }
              const lens = new Uint8Array(nlen + ndist);
              let have = 0;
              while (have < nlen + ndist) {
                const sym = decodiere(ct);
                if (sym < 16) {
                  lens[have++] = sym;
                } else {
                  let kopie;
                  let wert = 0;
                  if (sym === 16) {
                    if (have === 0) throw fehler('invalid bit length repeat');
                    wert = lens[have - 1];
                    kopie = 3 + lies(2);
                  } else if (sym === 17) {
                    kopie = 3 + lies(3);
                  } else {
                    kopie = 11 + lies(7);
                  }
                  if (have + kopie > nlen + ndist) throw fehler('invalid bit length repeat');
                  while (kopie--) lens[have++] = wert;
                }
              }
              if (lens[256] === 0) throw fehler('invalid code -- missing end-of-block');
              try {
                this.lit = baue(lens.subarray(0, nlen), nlen, LENS);
              } catch (e) {
                throw fehler('invalid literal/lengths set');
              }
              try {
                this.dist = baue(lens.subarray(nlen), ndist, LENS);
              } catch (e) {
                throw fehler('invalid distances set');
              }
              this.modus = 'huffman';
            } else {
              throw fehler('invalid block type');
            }
            this.letzter = letzter === 1;
          } catch (e) {
            if (e === NEED) {
              bp = start;
              break;
            }
            throw e;
          }
        } else if (this.modus === 'gespeichert') {
          const noch = Math.min(this.rest, (gesamtBits - bp) >> 3);
          sicher(noch);
          w.set(daten.subarray(bp >> 3, (bp >> 3) + noch), pos);
          pos += noch;
          bp += noch * 8;
          this.rest -= noch;
          if (pos - h0 > max) throw zuGross();
          if (this.rest > 0) break;
          this.modus = this.letzter ? 'ende' : 'kopf';
          if (this.modus === 'ende') break;
        } else if (this.modus === 'huffman') {
          const lit = this.lit;
          const dist = this.dist;
          for (;;) {
            const avail = gesamtBits - bp;
            const e = lit.tab[peek(lit.bits)];
            const l = e & 31;
            if (l > avail) break;
            const sym = e >> 5;
            if (sym < 256) {
              bp += l;
              sicher(1);
              w[pos++] = sym;
              if (pos - h0 > max) throw zuGross();
              continue;
            }
            if (sym === 256) {
              bp += l;
              this.modus = this.letzter ? 'ende' : 'kopf';
              break;
            }
            if (sym > 285) throw fehler('invalid literal/length code');
            const li = sym - 257;
            let b = bp + l;
            const lextra = LEXT[li];
            if (gesamtBits - b < lextra) break;
            const laenge = LBASE[li] + (lextra ? peekAt(daten, b, lextra) : 0);
            b += lextra;
            const de = dist.tab[peekAt(daten, b, dist.bits)];
            const dl = de & 31;
            if (dl > gesamtBits - b) break;
            const dsym = de >> 5;
            if (dsym > 29) throw fehler('invalid distance code');
            b += dl;
            const dextra = DEXT[dsym];
            if (gesamtBits - b < dextra) break;
            const abstand = DBASE[dsym] + (dextra ? peekAt(daten, b, dextra) : 0);
            b += dextra;
            if (abstand > pos) throw fehler('invalid distance too far back');
            bp = b;
            sicher(laenge);
            for (let k = 0; k < laenge; k++, pos++) w[pos] = w[pos - abstand];
            if (pos - h0 > max) throw zuGross();
          }
          if (this.modus === 'huffman') break; // Eingabe zu Ende mitten im Block
          if (this.modus === 'ende') break;
        } else {
          break;
        }
      }
    }
    const aus = w.slice(h0, pos);
    this.verlauf = w.slice(Math.max(0, pos - FENSTER), pos);
    if (this.modus === 'ende') {
      this.offen = new Uint8Array(0);
      this.bitVersatz = 0;
    } else {
      this.offen = daten.slice(bp >> 3);
      this.bitVersatz = bp & 7;
    }
    return aus;
  }
}

function peekAt(daten, bp, n) {
  const i = bp >> 3;
  return ((daten[i] | (daten[i + 1] << 8) | (daten[i + 2] << 16)) >>> (bp & 7)) & ((1 << n) - 1);
}
return Object.freeze({ MAX_RAHMEN, InflateFehler, Inflater });
})();

// ---- html.js ----
const __m_html = (() => {
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

const ATTRIBUTE = [
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
function stilKlassen(fg, bg, fl) {
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
function zeileZuHtml(zeile, doc = globalThis.document) {
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
return Object.freeze({ ATTRIBUTE, stilKlassen, zeileZuHtml });
})();

// ---- raster.js ----
const __m_raster = (() => {
// Dekoder der Rasterrahmen (Vertrag `rasterauszug`, Byte-Form A-B) fuer die Browser-Seite. Ein ES-Modul ohne Abhaengigkeiten.
//
//   import { Empfaenger, zeileZuHtml } from './raster.js';
//   const e = new Empfaenger({ deflate: true });      // ein Empfaenger je Verbindung
//   const { auszug, abbild } = e.nimm(bytes);          // Bytes eines Rahmens von der Leitung; wirft RasterFehler
//
// Der Empfaenger ist die Empfangsseite von `ra_ref.Verbindung` (VERTRAG RA9 bis RA11, RA13): Er haelt je Sitzungsnummer das Abbild,
// prueft jeden Rahmen streng und aendert sein Abbild nur, wenn der ganze Rahmen gueltig ist. Fehler tragen dieselben Codes wie die
// Referenz. Zeilen stehen als Laeufe `[fg, bg, fl, text]` (Zeilenform, Abschnitt 4 des Vertrags), nicht als SGR-Text.

const { Inflater, InflateFehler, MAX_RAHMEN } = __m_inflate;


const MAX_GROESSE = 1000;
const MIN_SPALTEN = 2;
const MIN_ZEILEN = 1;
const MAX_SICHER = Number.MAX_SAFE_INTEGER;
const ENDE_STROM = new Uint8Array([0, 0, 0xff, 0xff]);

// Ein Fehler eines Rahmens; `code` ist einer der Codes aus Abschnitt 5 des Vertrags.
class RasterFehler extends Error {
  constructor(code) {
    super(code);
    this.name = 'RasterFehler';
    this.code = code;
  }
}

const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
// C0 (ohne ESC, den der Rahmen nie enthaelt), DEL und C1 sind im Text nie erlaubt (RA4)
const UNGUELTIGE_ZEICHEN = /[\u0000-\u001f\u007f-\u009f]/;

function farbeOk(c) {
  return (c >= 0 && c <= 256) || (c >= 0x01000000 && c <= 0x01ffffff);
}

function stilGueltig(fg, bg, fl) {
  return farbeOk(fg) && farbeOk(bg) && fl >= 0 && fl < 128;
}

// Kanonische Zeile (RA2): kein leerer Lauf, keine zwei Nachbarn gleichen Stils, kein Vorgabestil-Lauf mit Leerzeichen am Ende.
function nichtKanonisch(z) {
  for (let k = 0; k < z.length; k++) {
    const l = z[k];
    if (l[3] === '') return true;
    if (k > 0) {
      const v = z[k - 1];
      if (v[0] === l[0] && v[1] === l[1] && v[2] === l[2]) return true;
    }
  }
  if (z.length) {
    const l = z[z.length - 1];
    if (l[0] === 0 && l[1] === 0 && l[2] === 0 && l[3].endsWith(' ')) return true;
  }
  return false;
}

// Leser der Rahmenbytes. Zahlen bis 2^53 - 1 sind Number, groessere BigInt (Stand reicht bis 2^64 - 1).
class Leser {
  constructor(b) {
    this.b = b;
    this.p = 0;
  }

  rest() {
    return this.b.length - this.p;
  }

  byte() {
    if (this.p >= this.b.length) throw new RasterFehler('rahmen_abgeschnitten');
    return this.b[this.p++];
  }

  vu() {
    let n = 0;
    let gross = null;
    let sh = 0;
    for (let k = 0; k < 10; k++) {
      const x = this.byte();
      if (gross === null) {
        if (sh < 49) n += (x & 0x7f) * 2 ** sh;
        else gross = BigInt(n) | (BigInt(x & 0x7f) << BigInt(sh));
      } else {
        gross |= BigInt(x & 0x7f) << BigInt(sh);
      }
      if (!(x & 0x80)) {
        if (k > 0 && x === 0) throw new RasterFehler('vu_ungueltig');
        if (gross === null) return n;
        if (gross >= 1n << 64n) throw new RasterFehler('vu_ungueltig');
        return gross <= BigInt(MAX_SICHER) ? Number(gross) : gross;
      }
      sh += 7;
    }
    throw new RasterFehler('vu_ungueltig');
  }

  // Eine Anzahl: kann nicht groesser sein als die noch ungelesenen Byte (jedes Element braucht mindestens eins).
  zahl() {
    const n = this.vu();
    if (n > this.rest()) throw new RasterFehler('rahmen_abgeschnitten');
    return n;
  }

  bytes(n) {
    if (n > this.rest()) throw new RasterFehler('rahmen_abgeschnitten');
    const x = this.b.subarray(this.p, this.p + n);
    this.p += n;
    return x;
  }
}

function cursorPruefen(z, s, spalten, zeilen) {
  if (!(z >= 0 && z < zeilen && s >= 0 && s < spalten)) throw new RasterFehler('cursor_ungueltig');
}

// Liest und prueft einen Rahmen gegen die Abbilder des Empfaengers. Aendert `abbilder` nie; liefert { auszug, abbild } des neuen Zustands.
function rahmenDekodieren(b, abbilder) {
  const p = new Leser(b);
  if (p.byte() !== 5) throw new RasterFehler('art_ungueltig');
  const nr = p.byte();
  const flags = p.byte();
  if (flags & ~15) throw new RasterFehler('flags_unbekannt');
  const stand = p.vu();
  const auszug = { nr, stand, groesse: null, invers: !!(flags & 8), schub: null, cursor: null, zeilen: [] };
  const voll = !!(flags & 4);
  let alt = null;
  let spalten;
  let zeilen;
  if (voll) {
    const sp = p.vu();
    const ze = p.vu();
    if (!(sp >= MIN_SPALTEN && sp <= MAX_GROESSE && ze >= MIN_ZEILEN && ze <= MAX_GROESSE)) throw new RasterFehler('groesse_ungueltig');
    if (flags & 1) throw new RasterFehler('rahmen_ungueltig');
    auszug.groesse = [sp, ze];
    spalten = sp;
    zeilen = ze;
  } else {
    alt = abbilder.get(nr);
    if (alt === undefined) throw new RasterFehler('kein_abzug');
    if (stand <= alt.stand) throw new RasterFehler('stand_alt');
    spalten = alt.spalten;
    zeilen = alt.zeilen;
  }
  if (flags & 1) {
    const o = p.vu();
    const u = p.vu();
    const n = p.vu();
    if (!(n >= 1 && BigInt(o) + BigInt(n) <= BigInt(u) && u <= zeilen)) throw new RasterFehler('schub_ungueltig');
    auszug.schub = [o, u, n];
  }
  if (flags & 2) {
    const z = p.vu();
    const s = p.vu();
    const v = p.vu();
    if (v > 1) throw new RasterFehler('cursor_ungueltig');
    cursorPruefen(z, s, spalten, zeilen);
    auszug.cursor = [z, s, v === 1];
  }
  const ns = p.zahl();
  const stile = [];
  for (let i = 0; i < ns; i++) {
    const fg = p.vu();
    const bg = p.vu();
    const fl = p.vu();
    if (!stilGueltig(fg, bg, fl)) throw new RasterFehler('stil_ungueltig');
    stile.push([fg, bg, fl]);
  }
  const nz = p.zahl();
  let letzte = -1;
  for (let i = 0; i < nz; i++) {
    const idx = p.vu();
    if (idx >= zeilen) throw new RasterFehler('zeile_ausserhalb');
    if (idx <= letzte) throw new RasterFehler('zeile_reihenfolge');
    letzte = idx;
    const nl = p.zahl();
    const z = [];
    for (let k = 0; k < nl; k++) {
      const sn = p.vu();
      if (sn >= stile.length) throw new RasterFehler('stilnr_ungueltig');
      const roh = p.bytes(p.zahl());
      let text;
      try {
        text = utf8.decode(roh);
      } catch (e) {
        throw new RasterFehler('utf8_ungueltig');
      }
      if (UNGUELTIGE_ZEICHEN.test(text)) throw new RasterFehler('zeichen_ungueltig');
      const st = stile[sn];
      z.push([st[0], st[1], st[2], text]);
    }
    if (nichtKanonisch(z)) throw new RasterFehler('zeile_nicht_kanonisch');
    auszug.zeilen.push([idx, z]);
  }
  if (p.rest()) throw new RasterFehler('rahmen_rest');

  // Anwenden auf eine Kopie: Vollbild auf ein leeres Abbild, sonst auf eine Kopie des alten.
  const abbild = voll
    ? { nr, spalten, zeilen, lines: new Array(zeilen).fill(null).map(() => []), invers: false, cursor: [0, 0, false], stand: 0 }
    : { nr, spalten, zeilen, lines: alt.lines.slice(), invers: false, cursor: alt.cursor, stand: 0 };
  if (auszug.schub) {
    const [o, u, n] = auszug.schub;
    for (let i = o; i < u - n; i++) abbild.lines[i] = abbild.lines[i + n];
    for (let i = u - n; i < u; i++) abbild.lines[i] = [];
  }
  for (const [i, z] of auszug.zeilen) abbild.lines[i] = z;
  if (auszug.cursor) abbild.cursor = auszug.cursor;
  abbild.invers = auszug.invers;
  abbild.stand = stand;
  return { auszug, abbild };
}

// Der Empfaenger einer Verbindung: ein Deflate-Kontext je Verbindung, je Sitzungsnummer ein Abbild (RA13).
class Empfaenger {
  // `deflate`: kommen die Rahmen durch Deflate (RA11)?
  constructor({ deflate = false } = {}) {
    this.deflate = !!deflate;
    this.inflater = deflate ? new Inflater() : null;
    this.abbilder = new Map();
  }

  // Bytes eines Rahmens von der Leitung -> { auszug, abbild }. Bei einem Fehler wirft die Methode `RasterFehler`; das Abbild bleibt unveraendert.
  // `inflate_fehler` und `rahmen_zu_gross` machen den Empfaenger der Verbindung unbrauchbar (RA11).
  nimm(draht) {
    if (draht instanceof ArrayBuffer) draht = new Uint8Array(draht);
    let roh;
    if (!this.deflate) {
      if (draht.length > MAX_RAHMEN) throw new RasterFehler('rahmen_zu_gross');
      roh = draht;
    } else {
      const eingabe = new Uint8Array(draht.length + ENDE_STROM.length);
      eingabe.set(draht, 0);
      eingabe.set(ENDE_STROM, draht.length);
      try {
        roh = this.inflater.inflate(eingabe, MAX_RAHMEN);
      } catch (e) {
        if (e instanceof InflateFehler) throw new RasterFehler(e.code);
        throw e;
      }
    }
    const { auszug, abbild } = rahmenDekodieren(roh, this.abbilder);
    this.abbilder.set(abbild.nr, abbild);
    return { auszug, abbild };
  }

  // Eine unabhaengige Kopie des ganzen Zustands (Deflate-Kontext und Abbilder), etwa um einen verdaechtigen Rahmen zu probieren.
  klon() {
    const k = new Empfaenger({ deflate: this.deflate });
    k.inflater = this.inflater ? this.inflater.klon() : null;
    k.abbilder = new Map(this.abbilder);
    return k;
  }

  // Die Sitzungsnummer vergessen (RA13); eine unbekannte Nummer ist kein Fehler.
  vergessen(nr) {
    this.abbilder.delete(nr);
  }

  // Das jetzige Abbild einer Sitzungsnummer oder `undefined`.
  abbild(nr) {
    return this.abbilder.get(nr);
  }
}
return Object.freeze({ zeileZuHtml: __m_html.zeileZuHtml, MAX_RAHMEN, RasterFehler, Empfaenger });
})();

// ---- seite-tasten.js ----
const __m_seite_tasten = (() => {
// Tastatur des Terminals (Vertrag `seite`, SE1 und SE2 Punkt 4): ein `keydown` wird zu einem Ereignis der Ansicht, zu einem
// Fokuswechsel oder zu nichts. Die Funktion uebersetzt nie in Bytes; das tut der Kern (`VT` R1). Ohne DOM, damit sie fuer sich pruefbar ist.

// Benannte Tasten mit den Namen von `VT` R1.
const TASTEN = {
  Enter: 'Enter', Tab: 'Tab', Escape: 'Escape', Backspace: 'Ruecktaste', Delete: 'Entf',
  ArrowUp: 'Hoch', ArrowDown: 'Runter', ArrowLeft: 'Links', ArrowRight: 'Rechts',
  Home: 'Pos1', End: 'Ende', PageUp: 'BildAuf', PageDown: 'BildAb',
};

// Umschalter immer in der Reihenfolge umschalt, strg, alt (SE1 Punkt 7).
function umschalterVon(e) {
  const u = [];
  if (e.shiftKey) u.push('umschalt');
  if (e.ctrlKey) u.push('strg');
  if (e.altKey) u.push('alt');
  return u;
}

// Liefert eines von:
//   null                                  nichts schicken, nichts verhindern (dem Browser bleiben, tote Taste, Komposition)
//   { fokus: 1 } oder { fokus: -1 }       F6 oder Umschalt+F6: das Terminal verlassen
//   { art: 'Taste', taste, umschalter }   eine benannte Taste oder Strg/Alt mit Buchstabe
//   { art: 'Text', text }                 ein druckbares Zeichen (auch AltGr und Wahltaste)
function deuteTaste(e) {
  if (e.isComposing || e.key === 'Dead' || e.key === 'Process' || e.keyCode === 229) return null;
  if (e.key === 'F6') return { fokus: e.shiftKey ? -1 : 1 };
  if (e.metaKey) return null;
  const u = umschalterVon(e);
  if (Object.prototype.hasOwnProperty.call(TASTEN, e.key)) return { art: 'Taste', taste: TASTEN[e.key], umschalter: u };
  if ([...e.key].length !== 1) return null;
  const buchstabe = /^[a-zA-Z]$/.test(e.key);
  if (e.ctrlKey && !e.altKey) {
    // Strg+V und Strg+Umschalt+Buchstabe gehoeren dem Browser (SE1 Punkt 8), Strg mit anderen Zeichen ebenso.
    if (!buchstabe || e.shiftKey || e.key.toLowerCase() === 'v') return null;
    return { art: 'Taste', taste: e.key.toLowerCase(), umschalter: ['strg'] };
  }
  // Alt mit einem unveraenderten Buchstaben ist eine Taste; was die Belegung mit Alt oder AltGr liefert, ist Text (SE1 Punkt 2 und 6).
  if (e.altKey && /^[a-z]$/.test(e.key)) return { art: 'Taste', taste: e.key, umschalter: u.filter((x) => x !== 'umschalt') };
  return { art: 'Text', text: e.key };
}
return Object.freeze({ TASTEN, umschalterVon, deuteTaste });
})();

// ---- seite-netz.js ----
const __m_seite_netz = (() => {
// Netz der Seite (Vertrag `seite` SE6, Vertrag `bruecke` Abschnitt 5): nur `GET /strom`, `POST /eingabe` und `POST /starten`
// desselben Ursprungs, das Token nur im Kopf `Authorization`, nie in der Adresse.

// Das Token steht im Fragment als `#t=<64 Kleinbuchstaben-Hex>`; sonst gibt es keines.
function tokenAusFragment(hash) {
  const r = /^#t=([0-9a-f]{64})$/.exec(hash || '');
  return r ? r[1] : '';
}

// Schickt die Ereignisse der Ansicht der Reihe nach: das naechste `POST /eingabe` erst nach der Antwort auf das vorige (SE1 Punkt 11).
class Sender {
  constructor(token) {
    this.token = token;
    this.verbindung = null;
    this.kette = Promise.resolve();
  }

  sende(ereignis) {
    const verbindung = this.verbindung;
    if (!verbindung) return;
    const body = JSON.stringify(ereignis);
    this.kette = this.kette.then(() => fetch('/eingabe', {
      method: 'POST',
      headers: { 'Authorization': 'Bearer ' + this.token, 'Content-Type': 'application/json', 'X-Verbindung': verbindung },
      body,
    }).then(() => undefined, () => undefined));
  }
}

// `POST /starten` mit der Start-Struktur. Liefert { status, antwort } oder { status: 0 } ohne Verbindung.
async function starten(token, start) {
  let r;
  try {
    r = await fetch('/starten', {
      method: 'POST',
      headers: { 'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json' },
      body: JSON.stringify(start),
    });
  } catch {
    return { status: 0, antwort: null };
  }
  let antwort = null;
  try {
    antwort = await r.json();
  } catch {
    antwort = null;
  }
  return { status: r.status, antwort };
}

// Liest den Ereignisstrom: Rahmen `[n: u32 big endian][art: u8][nutzlast]` (Vertrag `dienst` Abschnitt 5), in beliebigen Stuecken.
// `beiOffen({ verbindung, deflate })` nach den Koepfen, `beiRahmen(art, nutzlast)` je Rahmen, `beiEnde()` genau einmal am Schluss.
// Liefert eine Funktion, die den Strom abbricht.
function oeffneStrom(token, { beiOffen, beiRahmen, beiEnde }) {
  const abbruch = new AbortController();
  let beendet = false;
  const ende = () => {
    if (beendet) return;
    beendet = true;
    beiEnde();
  };
  (async () => {
    let antwort;
    try {
      antwort = await fetch('/strom', { headers: { 'Authorization': 'Bearer ' + token }, signal: abbruch.signal });
    } catch {
      ende();
      return;
    }
    if (!antwort.ok || !antwort.body) {
      ende();
      return;
    }
    beiOffen({ verbindung: antwort.headers.get('X-Verbindung'), deflate: antwort.headers.get('X-Deflate') === '1' });
    const leser = antwort.body.getReader();
    let puffer = new Uint8Array(0);
    for (;;) {
      let r;
      try {
        r = await leser.read();
      } catch {
        break;
      }
      if (r.done || beendet) break;
      const neu = new Uint8Array(puffer.length + r.value.length);
      neu.set(puffer);
      neu.set(r.value, puffer.length);
      puffer = neu;
      let p = 0;
      while (puffer.length - p >= 4) {
        const n = puffer[p] * 0x1000000 + (puffer[p + 1] << 16) + (puffer[p + 2] << 8) + puffer[p + 3];
        if (n < 1 || puffer.length - p < 4 + n) break;
        beiRahmen(puffer[p + 4], puffer.slice(p + 5, p + 4 + n));
        p += 4 + n;
        if (beendet) break;
      }
      puffer = puffer.slice(p);
      if (beendet) break;
    }
    ende();
  })();
  return () => {
    abbruch.abort();
    ende();
  };
}
return Object.freeze({ tokenAusFragment, Sender, starten, oeffneStrom });
})();

// ---- seite-ansagen.js ----
const __m_seite_ansagen = (() => {
// Ansagen fuer den Vorleser (Vertrag `seite` SE3 Punkt 3 und 4): eine hoefliche Live-Region; die Ausgabe des Terminals gesammelt,
// hoechstens eine Ausgabe-Ansage je 1000 ms Seitenuhr und hoechstens die letzten 5 Zeilen je Ansage.

const SPERRZEIT_MS = 1000;
const HOECHSTENS_ZEILEN = 5;
const BEHALTEN = 20;

class Ansager {
  constructor(region) {
    this.region = region;
    this.sperreBis = -Infinity;
    this.sammlung = new Map();
    this.wecker = null;
  }

  // Eine Ansage sofort: ein neuer Knoten in der Region.
  sage(text) {
    const d = this.region.ownerDocument.createElement('div');
    d.textContent = text;
    this.region.append(d);
    while (this.region.childElementCount > BEHALTEN) this.region.firstElementChild.remove();
  }

  // Neue Zeilen der Ausgabe (Index -> Text). Ohne Sperre sofort, sonst am Ende der Sperrzeit.
  zeilen(neu) {
    for (const [i, t] of neu) this.sammlung.set(i, t);
    if (!this.sammlung.size) return;
    const jetzt = performance.now();
    if (jetzt >= this.sperreBis) this.ausgabe();
    else if (this.wecker === null) this.wecker = setTimeout(() => this.ausgabe(), this.sperreBis - jetzt);
  }

  ausgabe() {
    this.wecker = null;
    if (!this.sammlung.size) return;
    const idx = [...this.sammlung.keys()].sort((a, b) => a - b).slice(-HOECHSTENS_ZEILEN);
    const text = idx.map((i) => this.sammlung.get(i)).join('\n');
    this.sammlung = new Map();
    this.sperreBis = performance.now() + SPERRZEIT_MS;
    this.sage(text);
  }
}
return Object.freeze({ SPERRZEIT_MS, HOECHSTENS_ZEILEN, Ansager });
})();

// ---- seite-terminal.js ----
const __m_seite_terminal = (() => {
// Anzeige des Terminals (Vertrag `seite` SE4, SE5 Punkt 2, SE7 Punkt 3): das Raster als Text im DOM, je Zeile ein Element
// `terminal-zeile-<n>`, der Cursor als Attribut `data-cursor` und als Markierung, die Groesse aus der Flaeche gemessen.

const { zeileZuHtml } = __m_html;

const KLEINSTE_SPALTEN = 2;
const KLEINSTE_ZEILEN = 1;
const GROESSTE = 1000;

// Text einer Zeile der Zeilenform: die Texte ihrer Laeufe aneinandergehaengt (SE4 Punkt 1).
function zeilenText(zeile) {
  let s = '';
  for (const l of zeile) s += l[3];
  return s;
}

// Spalten und Zeilen aus der Flaeche, geklemmt auf 2 bis 1000 Spalten und 1 bis 1000 Zeilen (SE5 Punkt 2).
function klemme(breite, hoehe, zeichenBreite, zeilenHoehe) {
  const sp = Math.floor(breite / (zeichenBreite > 0 ? zeichenBreite : 8));
  const ze = Math.floor(hoehe / (zeilenHoehe > 0 ? zeilenHoehe : 17));
  return [
    Math.max(KLEINSTE_SPALTEN, Math.min(GROESSTE, Number.isFinite(sp) ? sp : KLEINSTE_SPALTEN)),
    Math.max(KLEINSTE_ZEILEN, Math.min(GROESSTE, Number.isFinite(ze) ? ze : KLEINSTE_ZEILEN)),
  ];
}

class TerminalAnsicht {
  // `teile`: { terminal, raster, inhalt, cursor, messung, eingang }
  constructor(teile) {
    Object.assign(this, teile);
    this.elemente = [];
    this.gezeigt = null; // das zuletzt gezeigte Abbild
  }

  // Zeigt ein Abbild (oder keines). `geaendert` sind die Zeilen, die der Rahmen setzt; ohne Angabe wird alles neu gezeichnet.
  zeige(abbild, geaendert = null) {
    const doc = this.inhalt.ownerDocument;
    if (!abbild) {
      for (const e of this.elemente) e.remove();
      this.elemente = [];
      this.gezeigt = null;
      this.setzeCursor([0, 0, false]);
      this.terminal.classList.remove('invers');
      return;
    }
    const neuAufbauen = geaendert === null || !this.gezeigt || this.elemente.length !== abbild.zeilen;
    if (neuAufbauen) {
      for (const e of this.elemente) e.remove();
      this.elemente = [];
      const frag = doc.createDocumentFragment();
      for (let i = 0; i < abbild.zeilen; i++) {
        const d = doc.createElement('div');
        d.className = 'raster-zeile';
        d.dataset.testid = 'terminal-zeile-' + i;
        d.append(zeileZuHtml(abbild.lines[i], doc));
        this.elemente.push(d);
        frag.append(d);
      }
      this.inhalt.insertBefore(frag, this.cursor);
    } else {
      for (const i of geaendert) {
        const d = this.elemente[i];
        d.textContent = '';
        d.append(zeileZuHtml(abbild.lines[i], doc));
      }
    }
    this.gezeigt = abbild;
    this.terminal.classList.toggle('invers', !!abbild.invers);
    this.setzeCursor(abbild.cursor);
  }

  setzeCursor(c) {
    const [z, s, sichtbar] = c;
    this.terminal.dataset.cursor = `${z},${s},${sichtbar ? 'true' : 'false'}`;
    const { zw, zh } = this.zeichen();
    const links = s * zw;
    const oben = z * zh;
    this.cursor.style.setProperty('transform', `translate(${links}px, ${oben}px)`);
    this.cursor.style.setProperty('width', `${zw}px`);
    this.cursor.style.setProperty('height', `${zh}px`);
    this.cursor.hidden = !sichtbar || !this.gezeigt;
    // Das Eingabefeld steht am Cursor: so erscheint die Auswahl einer Komposition dort, wo der Text entsteht.
    this.eingang.style.setProperty('transform', `translate(${links}px, ${oben}px)`);
  }

  // Breite eines Zeichens und Hoehe einer Zeile in der Schrift des Rasters.
  zeichen() {
    const r = this.messung.getBoundingClientRect();
    return { zw: r.width / 10 || 8, zh: r.height || 17 };
  }

  // Spalten und Zeilen, die in die Flaeche des Rasters passen (ohne Innenabstand, unabhaengig von Rollbalken).
  messen() {
    const r = this.raster.getBoundingClientRect();
    const cs = getComputedStyle(this.raster);
    const breite = r.width - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight) - parseFloat(cs.borderLeftWidth) - parseFloat(cs.borderRightWidth);
    const hoehe = r.height - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom) - parseFloat(cs.borderTopWidth) - parseFloat(cs.borderBottomWidth);
    const { zw, zh } = this.zeichen();
    return klemme(breite, hoehe, zw, zh);
  }
}
return Object.freeze({ KLEINSTE_SPALTEN, KLEINSTE_ZEILEN, GROESSTE, zeilenText, klemme, TerminalAnsicht });
})();

// ---- seite-start.js ----
const __m_seite_start = (() => {
// Startformular (Vertrag `seite` SE6 Punkt 3, Vertrag `dienst` DI12 Punkt 10): die Wahlfelder fuellt die Nachricht `Startwahl`;
// das Absenden ergibt die Start-Struktur der Fassung 1, „ohne Kunde“ als `null`.

const OHNE_KUNDE = 'ohne Kunde';

// Setzt die Optionen eines Auswahlfelds. `eintraege`: Liste von { wert, text }. Die bisherige Wahl bleibt, wenn es sie noch gibt;
// sonst gilt `vorgabe` (ein Wert) oder der erste Eintrag.
function setzeOptionen(select, eintraege, vorgabe = null) {
  const bisher = select.options.length ? select.value : null;
  const doc = select.ownerDocument;
  select.textContent = '';
  for (const { wert, text } of eintraege) {
    const o = doc.createElement('option');
    o.value = wert;
    o.textContent = text;
    select.append(o);
  }
  const werte = eintraege.map((e) => e.wert);
  if (bisher !== null && werte.includes(bisher)) select.value = bisher;
  else if (vorgabe !== null && werte.includes(vorgabe)) select.value = vorgabe;
}

const text = (x) => typeof x === 'string' && x !== '';

// Das Modell, das ohne Angabe der Route gilt (SE6 Punkt 3, SE7 Punkt 2).
const MODELL_AUTO = 'auto';

// Die Eintraege der Wahlfelder aus einer Nachricht `Startwahl`, oder null, wenn sie nicht die Form der Fassung 1 hat.
// `modelle` ordnet jeder Route ihr Modell zu (der Text der Route oder `null`).
function eintraegeAusStartwahl(n) {
  if (!n || n.fassung !== 1 || !Array.isArray(n.harness) || !Array.isArray(n.routen) || !Array.isArray(n.kunden)) return null;
  const harness = n.harness.filter((h) => h && text(h.name)).map((h) => ({ wert: h.name, text: h.name }));
  const routen = n.routen.filter((r) => r && text(r.schluessel)).map((r) => ({ wert: r.schluessel, text: r.schluessel }));
  const modelle = new Map(n.routen.filter((r) => r && text(r.schluessel)).map((r) => [r.schluessel, text(r.modell) ? r.modell : null]));
  // „ohne Kunde“ steht zuletzt, auch wenn `null` in der Liste weiter vorn steht oder fehlt.
  const kunden = n.kunden.filter(text).map((k) => ({ wert: k, text: k }));
  kunden.push({ wert: '', text: OHNE_KUNDE });
  return { harness, routen, kunden, modelle };
}

// Der einzige Eintrag des Felds `Modell` fuer die gewaehlte Route: ihr Modell, sonst `auto`.
function modellEintraege(modelle, route) {
  const m = modelle.get(route);
  const wert = typeof m === 'string' && m !== '' ? m : MODELL_AUTO;
  return [{ wert, text: wert }];
}

// Die Start-Struktur aus den Werten des Formulars (SE6 Punkt 3).
function startStruktur(werte) {
  return {
    fassung: 1,
    harness: werte.harness,
    modell: werte.modell,
    route: werte.route,
    kunde: werte.kunde === '' ? null : werte.kunde,
    arbeitsordner: werte.arbeitsordner,
  };
}
return Object.freeze({ OHNE_KUNDE, setzeOptionen, MODELL_AUTO, eintraegeAusStartwahl, modellEintraege, startStruktur });
})();

// ---- seite-main.js ----
const __m_seite_main = (() => {
// Die Seite des Durchstichs (Vertrag `seite`): Sitzungsliste, Terminal, Eingabezeile und Startformular an der Bruecke.
// Einstieg des Buendlers (`buendeln.mjs`); alle anderen Module kommen ueber `import`.

const { Empfaenger, RasterFehler } = __m_raster;
const { deuteTaste } = __m_seite_tasten;
const { tokenAusFragment, Sender, starten, oeffneStrom } = __m_seite_netz;
const { Ansager } = __m_seite_ansagen;
const { TerminalAnsicht, zeilenText } = __m_seite_terminal;
const { setzeOptionen, eintraegeAusStartwahl, modellEintraege, startStruktur } = __m_seite_start;

// Texte der Ablehnungen ohne eigenes Feld `text` (SE3 Punkt 3); jeder andere Grund steht mit seinem Code.
const TEXTE = {
  taste_unbekannt: 'Taste nicht belegt',
  sitzung_beendet: 'Sitzung beendet',
  kein_abo: 'kein Abo',
  pane_busy: 'Dialog offen',
  steuerzeichen_im_text: 'Steuerzeichen im Text',
};
// Fehler des Rasters, nach denen der Empfaenger der Verbindung unbrauchbar ist (Vertrag `rasterauszug` RA11).
const UNHEILBAR = new Set(['inflate_fehler', 'rahmen_zu_gross']);

const el = (id) => document.getElementById(id);
const liste = el('liste');
const listeLeer = el('liste-leer');
const terminal = el('terminal');
const eingang = el('eingang');
const eingabe = el('eingabe');
const startForm = el('start');
const verbindungEl = el('verbindung');
const wahl = {
  harness: el('wahl-harness'),
  modell: el('wahl-modell'),
  route: el('wahl-route'),
  kunde: el('wahl-kunde'),
};
const arbeitsordner = el('wahl-arbeitsordner');

const token = tokenAusFragment(location.hash);
const sender = new Sender(token);
const ansager = new Ansager(el('ansagen'));
const ansicht = new TerminalAnsicht({
  terminal, raster: el('raster'), inhalt: el('raster-inhalt'), cursor: el('cursor'), messung: el('messung'), eingang,
});

let empfaenger = null;
let stromZu = null;
let sitzungen = [];
let bekannt = new Map(); // Kennung -> Zustand der vorigen Liste
let ersteListe = true;
let aktiv = null; // Kennung der abonnierten Sitzung
const gemeldet = new Map(); // Kennung -> [spalten, zeilen], zuletzt gemeldet

// Sitzungsnummer nach `DI` DI12 Punkt 2: `s-<n>` hat die Nummer n, die Sitzung aus `kind` die 0.
const nrVon = (id) => {
  const r = /^s-(\d+)$/.exec(id);
  return r ? Number(r[1]) : 0;
};

function verbindungZeigen(zustand, text) {
  verbindungEl.dataset.zustand = zustand;
  verbindungEl.querySelector('.verbindung-text').textContent = text;
}

// ---- Ereignisse an die Bruecke ----
function sendeText(t) {
  if (aktiv === null || t === '') return;
  sender.sende({ art: 'Text', sitzung: aktiv, text: t });
}
function sendeTaste(taste, umschalter) {
  if (aktiv === null) return;
  sender.sende({ art: 'Taste', sitzung: aktiv, taste, umschalter });
}

// ---- Fehlerfeld und Ablehnungen (SE3 Punkt 3, SE7 Punkt 5) ----
function zeigeFehler(code, text) {
  el('fehler-code').textContent = code;
  el('fehler-text').textContent = text;
  el('fehler').hidden = false;
}
function ablehnung(j) {
  const code = j && typeof j.code === 'string' && j.code ? j.code : (j && typeof j.grund === 'string' ? j.grund : '');
  const t = j && typeof j.text === 'string' && j.text ? j.text : (TEXTE[code] || code);
  zeigeFehler(code, t);
  ansager.sage('Abgelehnt: ' + t);
}

// ---- Groesse (SE5 Punkt 2) ----
function abo(id) {
  const [sp, ze] = ansicht.messen();
  gemeldet.set(id, [sp, ze]);
  sender.sende({ art: 'Abo', sitzung: id, an: true, spalten: sp, zeilen: ze });
}
function groesseMelden() {
  if (aktiv === null || !gemeldet.has(aktiv)) return;
  const [sp, ze] = ansicht.messen();
  const alt = gemeldet.get(aktiv);
  if (sp === alt[0] && ze === alt[1]) return;
  gemeldet.set(aktiv, [sp, ze]);
  sender.sende({ art: 'Groesse', sitzung: aktiv, spalten: sp, zeilen: ze });
}
new ResizeObserver(() => groesseMelden()).observe(el('raster'));

// ---- Sitzungsliste (SE2 Punkt 5, SE3, SE7 Punkt 1) ----
function zustandText(s) {
  return s.zustand === 'laeuft' ? 'läuft' : `beendet, Code ${s.code}`;
}
function terminalBenennen() {
  eingang.setAttribute('aria-label', aktiv === null ? 'Terminal, keine Sitzung' : 'Terminal ' + aktiv);
  terminal.setAttribute('aria-label', aktiv === null ? 'Terminal' : 'Terminal ' + aktiv);
  el('terminal-titel').textContent = aktiv === null ? 'Keine Sitzung gewählt' : aktiv;
}
function waehle(id, fokus) {
  if (id === aktiv) {
    if (fokus) eingang.focus({ preventScroll: true });
    return;
  }
  if (aktiv !== null) sender.sende({ art: 'Abo', sitzung: aktiv, an: false });
  aktiv = id;
  terminalBenennen();
  zeigeListe();
  ansicht.zeige(empfaenger ? empfaenger.abbild(nrVon(id)) || null : null);
  abo(id);
  if (fokus) eingang.focus({ preventScroll: true });
}
function zeigeListe() {
  const doc = liste.ownerDocument;
  liste.textContent = '';
  for (const s of sitzungen) {
    const li = doc.createElement('li');
    const b = doc.createElement('button');
    b.type = 'button';
    b.className = 'sitzung';
    b.dataset.testid = 'sitzung-' + s.id;
    b.dataset.zustand = s.zustand === 'laeuft' ? 'laeuft' : 'beendet';
    if (s.zustand !== 'laeuft') b.dataset.code = String(s.code);
    const teile = [s.harness, s.modell].filter((x) => typeof x === 'string' && x);
    b.setAttribute('aria-label', `${[s.id, ...teile].join(' ')}, ${zustandText(s)}`);
    if (s.id === aktiv) b.setAttribute('aria-current', 'true');
    const punkt = doc.createElement('span');
    punkt.className = 'sitzung-punkt';
    const name = doc.createElement('span');
    name.className = 'sitzung-name';
    name.textContent = s.id;
    const art = doc.createElement('span');
    art.className = 'sitzung-art';
    art.textContent = [...teile, typeof s.kunde === 'string' && s.kunde ? s.kunde : ''].filter(Boolean).join(' · ');
    const zustand = doc.createElement('span');
    zustand.className = 'sitzung-zustand';
    zustand.textContent = zustandText(s);
    b.append(punkt, name, zustand, art);
    b.addEventListener('click', () => waehle(s.id, true));
    li.append(b);
    liste.append(li);
  }
  listeLeer.hidden = sitzungen.length > 0;
}
function sitzungsliste(n) {
  sitzungen = Array.isArray(n.sitzungen) ? n.sitzungen.filter((s) => s && typeof s.id === 'string') : [];
  for (const s of sitzungen) {
    if (s.zustand !== 'laeuft' && bekannt.get(s.id) === 'laeuft') ansager.sage(`Sitzung ${s.id} beendet, Code ${s.code}`);
  }
  bekannt = new Map(sitzungen.map((s) => [s.id, s.zustand]));
  zeigeListe();
  if (ersteListe) {
    ersteListe = false;
    const erste = sitzungen.find((s) => s.zustand === 'laeuft');
    if (erste) waehle(erste.id, true);
    else wahl.harness.focus();
  }
}

// ---- Startwahl (DI12 Punkt 10) ----
let routenModelle = new Map();
let startwahlDa = false; // bis zur ersten `Startwahl` bleiben die festen Beispiel-Eintraege des Felds `Modell`
// Das Feld `Modell` hat genau einen Eintrag: das Modell der gewaehlten Route, sonst `auto`. Es folgt jeder Wahl der Route.
function fuelleModell() {
  const eintraege = modellEintraege(routenModelle, wahl.route.value);
  setzeOptionen(wahl.modell, eintraege, eintraege[0].wert);
  wahl.modell.value = eintraege[0].wert;
}
function startwahl(n) {
  const e = eintraegeAusStartwahl(n);
  if (!e) return;
  if (e.harness.length) setzeOptionen(wahl.harness, e.harness);
  if (e.routen.length) setzeOptionen(wahl.route, e.routen);
  setzeOptionen(wahl.kunde, e.kunden, '');
  routenModelle = e.modelle;
  startwahlDa = true;
  fuelleModell();
}
wahl.route.addEventListener('change', () => { if (startwahlDa) fuelleModell(); });

// ---- Rasterrahmen (SE3 Punkt 3, SE4) ----
function rasterRahmen(nutz) {
  let ergebnis;
  try {
    ergebnis = empfaenger.nimm(nutz);
  } catch (e) {
    if (e instanceof RasterFehler && UNHEILBAR.has(e.code) && stromZu) {
      zeigeFehler(e.code, 'Bild der Verbindung nicht lesbar');
      stromZu();
    }
    return; // ein ungueltiger Rahmen aendert nichts (RA10)
  }
  const { auszug, abbild } = ergebnis;
  if (aktiv === null || abbild.nr !== nrVon(aktiv)) return;
  const voll = auszug.groesse !== null;
  ansicht.zeige(abbild, voll || auszug.schub ? null : auszug.zeilen.map(([i]) => i));
  if (voll) return; // ein Vollbild sagt nichts an
  const neu = new Map();
  for (const [i] of auszug.zeilen) {
    const t = zeilenText(abbild.lines[i]);
    if (!t || i === abbild.cursor[0]) continue;
    neu.set(i, t);
  }
  ansager.zeilen(neu);
}

// ---- Strom ----
function nachricht(n) {
  if (!n || typeof n !== 'object') return;
  if (n.art === 'Sitzungsliste') sitzungsliste(n);
  else if (n.art === 'Startwahl') startwahl(n);
  else if (n.art === 'Hinweis' && n.hinweis === 'osc52_geschrieben' && (n.sitzung === undefined || n.sitzung === aktiv)) {
    ansager.sage('Das Programm hat in die Zwischenablage geschrieben');
  }
}
function rahmen(art, nutz) {
  if (art === 5) {
    rasterRahmen(nutz);
    return;
  }
  let j;
  try {
    j = JSON.parse(new TextDecoder().decode(nutz));
  } catch {
    return;
  }
  if (art === 6) nachricht(j);
  else if (art === 7) ablehnung(j);
  else if (art === 8 && j && typeof j.fehler === 'string') zeigeFehler(j.fehler, 'Verbindung mit Fehler beendet');
}
function verbinden() {
  verbindungZeigen('warte', 'verbindet');
  stromZu = oeffneStrom(token, {
    beiOffen: ({ verbindung, deflate }) => {
      sender.verbindung = verbindung;
      empfaenger = new Empfaenger({ deflate });
      verbindungZeigen('offen', 'verbunden');
    },
    beiRahmen: rahmen,
    beiEnde: () => {
      sender.verbindung = null;
      stromZu = null;
      verbindungZeigen('zu', 'getrennt, Seite neu laden');
      ansager.sage('Verbindung getrennt');
    },
  });
}

// ---- Tastatur im Terminal (SE1, SE2 Punkt 3 und 4) ----
function haltestellen() {
  return [...document.querySelectorAll('button, input, select, textarea')]
    .filter((x) => !x.disabled && x.tabIndex >= 0 && !x.closest('[hidden]'));
}
eingang.addEventListener('keydown', (e) => {
  const d = deuteTaste(e);
  if (!d) return;
  e.preventDefault();
  if (d.fokus) {
    const halte = haltestellen();
    const ziel = halte[halte.indexOf(eingang) + d.fokus];
    if (ziel) ziel.focus();
    return;
  }
  if (d.art === 'Taste') sendeTaste(d.taste, d.umschalter);
  else sendeText(d.text);
});
eingang.addEventListener('compositionend', (e) => {
  sendeText(e.data);
  eingang.value = '';
});
eingang.addEventListener('input', (e) => {
  if (e.isComposing) return;
  if ((e.inputType === 'insertText' || e.inputType === 'insertReplacementText') && e.data) sendeText(e.data);
  eingang.value = '';
});
eingang.addEventListener('paste', (e) => {
  e.preventDefault();
  const t = e.clipboardData ? e.clipboardData.getData('text/plain') : '';
  if (!t || aktiv === null) return;
  sender.sende({ art: 'Einfuegen', sitzung: aktiv, text: t });
});
eingang.addEventListener('focus', () => terminal.classList.add('fokus'));
eingang.addEventListener('blur', () => terminal.classList.remove('fokus'));
// Klick oder Tippen in das Raster setzt den Fokus ins Terminal, ausser beim Markieren von Text.
el('raster').addEventListener('click', () => {
  const sel = document.getSelection();
  if (sel && !sel.isCollapsed) return;
  eingang.focus({ preventScroll: true });
});

// ---- Eingabezeile (SE5 Punkt 3) ----
el('zeile-eingabe').addEventListener('submit', (e) => {
  e.preventDefault();
  const t = eingabe.value;
  eingabe.value = '';
  if (t) sendeText(t);
  sendeTaste('Enter', []);
});

// ---- Startformular (SE6 Punkt 3) ----
startForm.addEventListener('submit', async (e) => {
  e.preventDefault();
  if (!token) return;
  const start = startStruktur({
    harness: wahl.harness.value, modell: wahl.modell.value, route: wahl.route.value, kunde: wahl.kunde.value, arbeitsordner: arbeitsordner.value,
  });
  const { status, antwort } = await starten(token, start);
  if (status === 200 && antwort && typeof antwort.sitzung === 'string') {
    el('fehler').hidden = true;
    ansager.sage(`Sitzung ${antwort.sitzung} gestartet`);
  } else if (status === 422) {
    ablehnung(antwort || {});
  } else {
    zeigeFehler(status ? 'http_' + status : 'netz', 'Start gescheitert');
    ansager.sage('Start gescheitert');
  }
});

// ---- Anfang ----
terminalBenennen();
if (token) verbinden();
else verbindungZeigen('zu', 'Adresse ohne Zugangsschlüssel');
return Object.freeze({});
})();

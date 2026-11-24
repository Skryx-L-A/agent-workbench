// Dekoder der Rasterrahmen (Vertrag `rasterauszug`, Byte-Form A-B) fuer die Browser-Seite. Ein ES-Modul ohne Abhaengigkeiten.
//
//   import { Empfaenger, zeileZuHtml } from './raster.js';
//   const e = new Empfaenger({ deflate: true });      // ein Empfaenger je Verbindung
//   const { auszug, abbild } = e.nimm(bytes);          // Bytes eines Rahmens von der Leitung; wirft RasterFehler
//
// Der Empfaenger ist die Empfangsseite von `ra_ref.Verbindung` (VERTRAG RA9 bis RA11, RA13): Er haelt je Sitzungsnummer das Abbild,
// prueft jeden Rahmen streng und aendert sein Abbild nur, wenn der ganze Rahmen gueltig ist. Fehler tragen dieselben Codes wie die
// Referenz. Zeilen stehen als Laeufe `[fg, bg, fl, text]` (Zeilenform, Abschnitt 4 des Vertrags), nicht als SGR-Text.

import { Inflater, InflateFehler, MAX_RAHMEN } from './inflate.js';

export { zeileZuHtml } from './html.js';
export { MAX_RAHMEN };

const MAX_GROESSE = 1000;
const MIN_SPALTEN = 2;
const MIN_ZEILEN = 1;
const MAX_SICHER = Number.MAX_SAFE_INTEGER;
const ENDE_STROM = new Uint8Array([0, 0, 0xff, 0xff]);

// Ein Fehler eines Rahmens; `code` ist einer der Codes aus Abschnitt 5 des Vertrags.
export class RasterFehler extends Error {
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
export class Empfaenger {
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

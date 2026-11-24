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

export const MAX_RAHMEN = 16777216;

export class InflateFehler extends Error {
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

export class Inflater {
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

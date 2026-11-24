// Test des Entpackers `inflate.js` gegen zlib (das in Node eingebaute). Fester Samen; Exit 0 nur bei null Abweichungen.
//   1. Strom je Stufe 0 bis 9 und Strategie 0 bis 4: voll entpackt und an zufaelligen Stellen geteilt (der Entpacker haelt den Zustand).
//   2. Rahmen einer Verbindung mit Sync-Flush und einem Kontext (RA11), an zufaelligen Stellen geteilt.
//   3. Beschaedigte und gekuerzte Stroeme: gleicher Ausgang wie zlib (Fehler oder dieselbe Ausgabe).
//   4. Grenzen: 16 MiB genau, ein Byte mehr, Rueckverweis vor den Anfang, Zustand nach einem Fehler.
import zlib from 'node:zlib';
import { Inflater, InflateFehler, MAX_RAHMEN } from '../inflate.js';

function mulberry32(a) {
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const rnd = mulberry32(20261120);
const ganz = (n) => Math.floor(rnd() * n);
const zaehler = { strom: 0, geteilt: 0, rahmen: 0, beschaedigt: 0, beschaedigtFehler: 0, abweichungen: 0 };
const meldungen = [];

function abweichung(text) {
  zaehler.abweichungen++;
  if (meldungen.length < 15) meldungen.push(text);
}

function daten(len) {
  const b = Buffer.alloc(len);
  const modus = ganz(4);
  const wort = Buffer.from('raster alpha beta gamma ~/AI $ ');
  for (let i = 0; i < len; i++) {
    if (modus === 0) b[i] = rnd() < 0.7 ? 65 + ganz(6) : ganz(256);
    else if (modus === 1) b[i] = wort[(i + ganz(3)) % wort.length];
    else if (modus === 2) b[i] = ganz(256);
    else b[i] = i % 97 < 60 ? 0x20 : 0x41 + (i % 5);
  }
  return b;
}

// zlib als Pruefer: Ausgabe oder Fehler eines Teilstroms
function zlibErgebnis(buf) {
  try {
    return { ok: true, aus: zlib.inflateRawSync(buf, { finishFlush: zlib.constants.Z_SYNC_FLUSH }) };
  } catch (e) {
    return { ok: false };
  }
}

function jsErgebnis(buf, teile = 1) {
  const inf = new Inflater();
  try {
    const stuecke = [];
    let von = 0;
    for (let t = 1; t <= teile; t++) {
      const bis = t === teile ? buf.length : von + ganz(buf.length - von + 1);
      stuecke.push(Buffer.from(inf.inflate(buf.subarray(von, bis))));
      von = bis;
    }
    return { ok: true, aus: Buffer.concat(stuecke) };
  } catch (e) {
    if (!(e instanceof InflateFehler)) throw e;
    return { ok: false, code: e.code };
  }
}

// Ein Packer mit einem Kontext je Aufruf-Folge: Sync-Flush je Rahmen, die vier Endbytes entfernt (RFC 7692). Der Kontext ist das voreingestellte
// Woerterbuch (die letzten 32 KiB der bisherigen Eingabe): so verweist ein Rahmen auf die Rahmen davor, ohne dass ein zlib-Strom laeuft
// (die Strom-Schnittstelle von Node ist fuer diesen Zweck nicht verlaesslich: gemessen, sie bleibt ohne Fehlermeldung haengen).
function packer(optionen) {
  let verlauf = Buffer.alloc(0);
  return (buf) => {
    const o = { ...optionen, finishFlush: zlib.constants.Z_SYNC_FLUSH };
    if (verlauf.length) o.dictionary = verlauf;
    const b = zlib.deflateRawSync(buf, o);
    verlauf = Buffer.concat([verlauf, buf]).subarray(-32768);
    if (!b.subarray(b.length - 4).equals(Buffer.from([0, 0, 0xff, 0xff]))) throw new Error('Rahmen endet nicht auf 00 00 ff ff');
    return b.subarray(0, b.length - 4);
  };
}

// ---- 1. Stroeme
for (let stufe = 0; stufe <= 9; stufe++) {
  for (let strategie = 0; strategie <= 4; strategie++) {
    for (const len of [0, 1, 10, 300, 5000, 70000, 200000]) {
      const quelle = daten(len);
      const strom = zlib.deflateRawSync(quelle, { level: stufe, strategy: strategie, memLevel: 1 + ganz(9) });
      zaehler.strom++;
      const g = jsErgebnis(strom);
      if (!g.ok || !g.aus.equals(quelle)) abweichung(`Strom Stufe ${stufe} Strategie ${strategie} Laenge ${len}: nicht gleich`);
      const t = jsErgebnis(strom, 4);
      zaehler.geteilt++;
      if (!t.ok || !t.aus.equals(quelle)) abweichung(`geteilter Strom Stufe ${stufe} Strategie ${strategie} Laenge ${len}: nicht gleich`);
    }
  }
}

// ---- 2. Rahmen einer Verbindung (Sync-Flush, ein Kontext)
for (let stufe = 0; stufe <= 9; stufe++) {
  for (const strategie of [0, 1, 2, 3, 4]) {
    const pack = packer({ level: stufe, strategy: strategie });
    const inf = new Inflater();
    for (let f = 0; f < 10; f++) {
      const quelle = daten(1 + ganz(6000));
      const draht = pack(quelle);
      const voll = Buffer.concat([draht, Buffer.from([0, 0, 0xff, 0xff])]);
      const schnitt = ganz(voll.length + 1);
      const aus = Buffer.concat([Buffer.from(inf.inflate(voll.subarray(0, schnitt))), Buffer.from(inf.inflate(voll.subarray(schnitt)))]);
      zaehler.rahmen++;
      if (!aus.equals(quelle)) abweichung(`Rahmen ${f} Stufe ${stufe} Strategie ${strategie}: nicht gleich`);
    }
  }
}

// ---- 3. beschaedigte Stroeme
for (let k = 0; k < 6000; k++) {
  const quelle = daten(1 + ganz(3000));
  const strom = Buffer.from(zlib.deflateRawSync(quelle, { level: ganz(10), strategy: ganz(5) }));
  const b = Buffer.from(strom);
  const art = ganz(4);
  let probe = b;
  if (art === 0) {
    b[ganz(b.length)] ^= 1 << ganz(8);
  } else if (art === 1) {
    probe = b.subarray(0, ganz(b.length));
  } else if (art === 2) {
    b[ganz(b.length)] = ganz(256);
  } else {
    probe = Buffer.concat([b, Buffer.from([ganz(256), ganz(256)])]);
  }
  zaehler.beschaedigt++;
  const z = zlibErgebnis(probe);
  const j = jsErgebnis(probe);
  if (!z.ok) zaehler.beschaedigtFehler++;
  if (z.ok !== j.ok || (z.ok && !z.aus.equals(j.aus))) {
    abweichung(`beschaedigt ${k} (Art ${art}, ${probe.length} Byte): zlib ${z.ok ? `${z.aus.length} Byte` : 'Fehler'}, JS ${j.ok ? `${j.aus.length} Byte` : j.code}`);
  }
}

// ---- 4. Grenzen
const grenzen = [];
function pruefe(name, bedingung) {
  grenzen.push(name);
  if (!bedingung) abweichung(`Grenze: ${name}`);
}
{
  const nullen = zlib.deflateRawSync(Buffer.alloc(MAX_RAHMEN));
  const a = new Inflater().inflate(nullen);
  pruefe('genau 16 MiB Ausgabe sind erlaubt', a.length === MAX_RAHMEN);
  const mehr = zlib.deflateRawSync(Buffer.alloc(MAX_RAHMEN + 1));
  const inf = new Inflater();
  let code = null;
  try {
    inf.inflate(mehr);
  } catch (e) {
    code = e.code;
  }
  pruefe('16 MiB plus ein Byte ist rahmen_zu_gross', code === 'rahmen_zu_gross');
  let wieder = null;
  try {
    inf.inflate(new Uint8Array(0));
  } catch (e) {
    wieder = e.code;
  }
  pruefe('danach bleibt der Entpacker unbrauchbar', wieder === 'rahmen_zu_gross');
  // Die Obergrenze zaehlt je Aufruf: zwei Aufrufe mit je 8 MiB sind erlaubt
  const halb = packer({});
  const inf2 = new Inflater();
  let summe = 0;
  for (let i = 0; i < 3; i++) {
    const draht = halb(Buffer.alloc(MAX_RAHMEN / 2));
    summe += inf2.inflate(Buffer.concat([draht, Buffer.from([0, 0, 0xff, 0xff])])).length;
  }
  pruefe('die Obergrenze gilt je Aufruf (3 mal 8 MiB)', summe === 3 * (MAX_RAHMEN / 2));
}
{
  // fester Huffman-Block: Laenge 3, Abstand 1, ohne vorherige Ausgabe -> Rueckverweis vor den Anfang
  const bits = [];
  const schreibe = (wert, n, msbErst) => {
    for (let i = 0; i < n; i++) bits.push(msbErst ? (wert >> (n - 1 - i)) & 1 : (wert >> i) & 1);
  };
  schreibe(1, 1); // letzter Block
  schreibe(1, 2); // fest
  schreibe(1, 7, true); // Symbol 257 (Laenge 3): Code 0000001
  schreibe(0, 5, true); // Abstand-Code 0 (Abstand 1)
  schreibe(0, 7, true); // Ende des Blocks
  const roh = Buffer.alloc(Math.ceil(bits.length / 8));
  bits.forEach((b, i) => {
    roh[i >> 3] |= b << (i & 7);
  });
  const z = zlibErgebnis(roh);
  const inf = new Inflater();
  let fehler = null;
  try {
    inf.inflate(roh);
  } catch (e) {
    fehler = e;
  }
  pruefe('zlib lehnt den Rueckverweis vor den Anfang ab', !z.ok);
  pruefe('JS: inflate_fehler "invalid distance too far back"', fehler && fehler.code === 'inflate_fehler' && /too far back/.test(fehler.detail));
  let wieder = null;
  try {
    inf.inflate(new Uint8Array(0));
  } catch (e) {
    wieder = e.code;
  }
  pruefe('danach wirft er den Fehler erneut', wieder === 'inflate_fehler');
  // dasselbe mitten im Strom: ein Abstand ueber das schon Ausgegebene hinaus
  const inf2 = new Inflater();
  const pack2 = packer({ level: 6 });
  pack2(Buffer.from('xyz xyz xyz xyz xyz xyz'));
  const zweiter = pack2(Buffer.from('xyz xyz xyz xyz xyz xyz'));
  let code2 = null;
  try {
    inf2.inflate(Buffer.concat([zweiter, Buffer.from([0, 0, 0xff, 0xff])])); // zweiter Rahmen ohne den ersten: verweist auf Ausgabe, die dieser Entpacker nie sah
  } catch (e) {
    code2 = e.code;
  }
  pruefe('zweiter Rahmen ohne den ersten ist ein Fehler', code2 === 'inflate_fehler');
}

console.log(`inflate: ${zaehler.strom} Stroeme (${zaehler.geteilt} davon geteilt), ${zaehler.rahmen} Rahmen mit einem Kontext, ${zaehler.beschaedigt} beschaedigte Stroeme (${zaehler.beschaedigtFehler} davon Fehler bei zlib), ${grenzen.length} Grenzen, ${zaehler.abweichungen} Abweichungen`);
for (const m of meldungen) console.log(`  ABWEICHUNG ${m}`);
process.exitCode = zaehler.abweichungen === 0 ? 0 : 1;

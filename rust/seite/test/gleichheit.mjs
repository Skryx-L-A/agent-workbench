// Gleichheit mit der Referenz: Jeder Rahmen aus daten/gleichheit.json.gz (erzeugt von referenz_erzeugen.py aus `ra_ref.py`) geht durch den
// JS-Empfaenger; Bild, Auszug und Fehlercode muessen die der Referenz sein. Ausgabe: Zahl der Faelle, Rahmen und Abweichungen.
// Exit 0 nur bei null Abweichungen.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { Empfaenger, RasterFehler } from '../raster.js';

const hier = fileURLToPath(new URL('.', import.meta.url));
const pfad = process.argv[2] || `${hier}daten/gleichheit.json.gz`;
const daten = JSON.parse(gunzipSync(readFileSync(pfad)).toString('utf-8'));

function bytesAus(hex) {
  const b = new Uint8Array(hex.length / 2);
  for (let i = 0; i < b.length; i++) b[i] = parseInt(hex.substr(i * 2, 2), 16);
  return b;
}

function auszugJson(a) {
  return {
    nr: a.nr,
    stand: String(a.stand),
    groesse: a.groesse,
    invers: a.invers,
    schub: a.schub,
    cursor: a.cursor,
    zeilen: a.zeilen,
  };
}

function abbildJson(ab) {
  return {
    nr: ab.nr,
    stand: String(ab.stand),
    groesse: [ab.spalten, ab.zeilen],
    invers: ab.invers,
    cursor: ab.cursor,
    zeilen: ab.lines,
  };
}

const zaehler = { faelle: 0, vertragsfaelle: 0, zufall: 0, rahmen: 0, vertragsrahmen: 0, zufallsrahmen: 0, ok: 0, fehler: 0, abweichungen: 0, isoliert: 0 };
const codes = new Map();
const abweichungen = [];

for (const fall of daten.faelle) {
  zaehler.faelle++;
  const zufall = fall.name.startsWith('zufall/');
  if (zufall) zaehler.zufall++;
  else zaehler.vertragsfaelle++;
  const empf = new Map();
  let isoliert = false;
  fall.ereignisse.forEach((e, k) => {
    if (e.art === 'isoliert') {
      isoliert = true;
    } else if (e.art === 'verbinden') {
      empf.set(e.verbindung, new Empfaenger({ deflate: e.deflate }));
    } else if (e.art === 'vergessen') {
      empf.get(e.verbindung).vergessen(e.nr);
    } else {
      zaehler.rahmen++;
      if (zufall) zaehler.zufallsrahmen++;
      else zaehler.vertragsrahmen++;
      let ist;
      if (isoliert) zaehler.isoliert++;
      try {
        const ziel = isoliert ? empf.get(e.verbindung).klon() : empf.get(e.verbindung);
        isoliert = false;
        const r = ziel.nimm(bytesAus(e.draht));
        ist = { ok: true, auszug: auszugJson(r.auszug), abbild: abbildJson(r.abbild) };
      } catch (x) {
        if (!(x instanceof RasterFehler)) {
          ist = { ok: false, fehler: `AUSNAHME ${x && x.stack}` };
        } else {
          ist = { ok: false, fehler: x.code };
        }
      }
      if (e.erwartet.ok) zaehler.ok++;
      else {
        zaehler.fehler++;
        codes.set(e.erwartet.fehler, (codes.get(e.erwartet.fehler) || 0) + 1);
      }
      try {
        assert.deepStrictEqual(ist, e.erwartet);
      } catch (x) {
        zaehler.abweichungen++;
        if (abweichungen.length < 15) {
          abweichungen.push(`${fall.name} Ereignis ${k}: erwartet ${e.erwartet.ok ? 'Bild' : e.erwartet.fehler}, ist ${ist.ok ? 'Bild' : ist.fehler}`);
        }
      }
    }
  });
}

console.log(`gleichheit: ${zaehler.faelle} Faelle (${zaehler.vertragsfaelle} des Vertrags, ${zaehler.zufall} Zufallsverbindungen), ${zaehler.rahmen} Rahmen`
  + ` (${zaehler.vertragsrahmen} Vertrag, ${zaehler.zufallsrahmen} Zufall; ${zaehler.ok} gueltig, ${zaehler.fehler} mit Fehler; davon ${zaehler.isoliert} beschaedigte Proben an einer Kopie), ${zaehler.abweichungen} Abweichungen`);
console.log(`gleichheit: Fehlercodes der Referenz in der Probe (${codes.size}): ${[...codes].sort().map(([c, n]) => `${c}=${n}`).join(' ')}`);
for (const a of abweichungen) console.log(`  ABWEICHUNG ${a}`);
process.exitCode = zaehler.abweichungen === 0 ? 0 : 1;

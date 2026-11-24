// Test des Buendlers: zwei Laeufe von `buendeln.mjs` (als eigene Prozesse, je in einen frischen Ordner ausserhalb des Repos) ergeben
// byte-gleiche Ausgaben, gleich den eingecheckten Dateien in `ausgabe/`; in keiner steht `://`. Exit ueber process.exitCode.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HIER = path.dirname(fileURLToPath(import.meta.url));
const SEITE = path.join(HIER, '..');
const DATEIEN = ['index.html', 'seite.js'];
const ADRESSE = ':' + '//';
let rot = 0;
const pruefe = (ok, text) => {
  if (ok) return;
  rot++;
  console.log('ROT buendel: ' + text);
};

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'seite-buendel-'));
try {
  const laeufe = [];
  for (const n of ['eins', 'zwei']) {
    const ziel = path.join(tmp, n);
    execFileSync(process.execPath, [path.join(SEITE, 'buendeln.mjs'), '--aus', ziel], { stdio: 'pipe', timeout: 30000 });
    laeufe.push(Object.fromEntries(DATEIEN.map((d) => [d, fs.readFileSync(path.join(ziel, d))])));
  }
  for (const d of DATEIEN) {
    pruefe(laeufe[0][d].length > 0, `${d} ist leer`);
    pruefe(laeufe[0][d].equals(laeufe[1][d]), `${d}: der zweite Lauf ergibt andere Bytes`);
    const eingecheckt = fs.readFileSync(path.join(SEITE, 'ausgabe', d));
    pruefe(laeufe[0][d].equals(eingecheckt), `${d}: ausgabe/${d} ist nicht der Stand der Quellen (node buendeln.mjs laufen lassen)`);
    pruefe(!laeufe[0][d].toString('utf8').includes(ADRESSE), `${d} enthaelt ${ADRESSE}`);
  }
  const js = laeufe[0]['seite.js'].toString('utf8');
  pruefe(!/^\s*(import|export)\b/m.test(js), 'seite.js enthaelt ein import oder export');
  const html = laeufe[0]['index.html'].toString('utf8');
  pruefe(html.includes('<link rel="icon" href="data:,">'), 'index.html ohne <link rel="icon" href="data:,">');
  pruefe(!html.includes('/*STILE*/'), 'index.html enthaelt noch die Marke der Stile');
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
console.log(rot ? `buendel: ${rot} rot` : 'buendel: gruen');
process.exitCode = rot ? 1 : 0;

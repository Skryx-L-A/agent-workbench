// Buendler der Seite: schreibt die zwei Dateien, die die Bruecke mit `include_bytes!` einbindet.
//
//   node buendeln.mjs                 schreibt ausgabe/index.html und ausgabe/seite.js neben dieser Datei
//   node buendeln.mjs --aus <ordner>  schreibt dieselben zwei Dateien in einen anderen Ordner (fuer den Test)
//
// Nur `node`, keine Pakete. `index.html` ist `seite.html` mit `raster.css` und `seite.css` eingebettet. `seite.js` sind alle Module ab
// `seite-main.js` in einer Datei: jedes Modul in einer eigenen Funktion, seine `import` werden zu Zugriffen auf die Ausfuhren des anderen
// Moduls, zur Laufzeit gibt es kein `import`. Gleiche Eingaben ergeben gleiche Bytes (keine Zeit, kein Pfad, feste Reihenfolge).
// Steht in einer der zwei Ausgaben die Zeichenfolge `://` (eine Adresse, Vertrag `bruecke` Abschnitt 5), bricht der Buendler ab.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HIER = path.dirname(fileURLToPath(import.meta.url));
const EINSTIEG = 'seite-main.js';
const STILE = ['raster.css', 'seite.css'];
const ADRESSE = ':' + '//';

class BuendelFehler extends Error {}

const lies = (name) => fs.readFileSync(path.join(HIER, name), 'utf8').replace(/\r\n/g, '\n');
const kennung = (datei) => '__m_' + datei.replace(/\.js$/, '').replace(/[^A-Za-z0-9]/g, '_');

// Nur relative Module dieses Ordners: './name.js'.
function modulName(quelle, von) {
  const r = /^\.\/([A-Za-z0-9_-]+\.js)$/.exec(quelle);
  if (!r) throw new BuendelFehler(`${von}: Import ${quelle} ist kein Modul dieses Ordners`);
  return r[1];
}

// Liste der Namen in `{ a, b as c }`: [[aussen, innen]].
function namensliste(teil, von) {
  return teil.split(',').map((x) => x.trim()).filter(Boolean).map((x) => {
    const r = /^([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/.exec(x);
    if (!r) throw new BuendelFehler(`${von}: Name ${x} nicht lesbar`);
    return [r[1], r[2] || r[1]];
  });
}

// Wandelt ein Modul: liefert { koerper, importe: [modul], ausfuhren: [[name, ausdruck]] }.
function wandle(datei) {
  const zeilen = lies(datei).split('\n');
  const importe = [];
  const ausfuhren = [];
  const aus = [];
  for (const zeile of zeilen) {
    let r;
    if ((r = /^import\s*\{([^}]*)\}\s*from\s*'([^']+)';\s*$/.exec(zeile))) {
      const m = modulName(r[2], datei);
      importe.push(m);
      const teile = namensliste(r[1], datei).map(([a, i]) => (a === i ? a : `${a}: ${i}`));
      aus.push(`const { ${teile.join(', ')} } = ${kennung(m)};`);
    } else if ((r = /^export\s*\{([^}]*)\}\s*from\s*'([^']+)';\s*$/.exec(zeile))) {
      const m = modulName(r[2], datei);
      importe.push(m);
      for (const [a, i] of namensliste(r[1], datei)) ausfuhren.push([i, `${kennung(m)}.${a}`]);
    } else if ((r = /^export\s*\{([^}]*)\};\s*$/.exec(zeile))) {
      for (const [a, i] of namensliste(r[1], datei)) ausfuhren.push([i, a]);
    } else if ((r = /^export\s+((?:async\s+)?function\*?|class|const|let)\s+([A-Za-z_$][\w$]*)/.exec(zeile))) {
      ausfuhren.push([r[2], r[2]]);
      aus.push(zeile.replace(/^export\s+/, ''));
    } else if (/^\s*(import|export)\b/.test(zeile)) {
      throw new BuendelFehler(`${datei}: Zeile nicht lesbar: ${zeile}`);
    } else {
      aus.push(zeile);
    }
  }
  return { koerper: aus.join('\n').replace(/\s+$/, ''), importe, ausfuhren };
}

// Alle Module ab dem Einstieg, jedes nach seinen Abhaengigkeiten (Tiefensuche in der Reihenfolge der Importe).
function module() {
  const fertig = new Map();
  const unterwegs = new Set();
  const besuche = (datei) => {
    if (fertig.has(datei)) return;
    if (unterwegs.has(datei)) throw new BuendelFehler(`Kreis der Importe bei ${datei}`);
    unterwegs.add(datei);
    const m = wandle(datei);
    for (const i of m.importe) besuche(i);
    unterwegs.delete(datei);
    fertig.set(datei, m);
  };
  besuche(EINSTIEG);
  return fertig;
}

export function buendle() {
  const teile = ['// Erzeugt von buendeln.mjs aus den Modulen in rust/seite/; nicht von Hand aendern.'];
  for (const [datei, m] of module()) {
    const ausfuhr = m.ausfuhren.map(([n, a]) => (n === a ? n : `${n}: ${a}`)).join(', ');
    teile.push(`// ---- ${datei} ----\nconst ${kennung(datei)} = (() => {\n${m.koerper}\nreturn Object.freeze({${ausfuhr ? ` ${ausfuhr} ` : ''}});\n})();`);
  }
  const js = teile.join('\n\n') + '\n';
  const vorlage = lies('seite.html');
  const marke = '/*STILE*/';
  if (vorlage.split(marke).length !== 2) throw new BuendelFehler('seite.html: die Marke /*STILE*/ steht nicht genau einmal da');
  const stile = STILE.map((s) => lies(s).replace(/\s+$/, '')).join('\n');
  if (stile.includes('</style')) throw new BuendelFehler('Stile enthalten </style');
  const html = vorlage.replace(marke, () => stile);
  for (const [name, inhalt] of [['index.html', html], ['seite.js', js]]) {
    if (inhalt.includes(ADRESSE)) throw new BuendelFehler(`${name}: enthaelt die Zeichenfolge ${ADRESSE} (eine Adresse)`);
  }
  return { 'index.html': html, 'seite.js': js };
}

function main() {
  const args = process.argv.slice(2);
  const i = args.indexOf('--aus');
  const ziel = i >= 0 ? path.resolve(args[i + 1] || '') : path.join(HIER, 'ausgabe');
  let dateien;
  try {
    dateien = buendle();
  } catch (e) {
    if (e instanceof BuendelFehler) {
      console.error('buendeln: ' + e.message);
      process.exitCode = 1;
      return;
    }
    throw e;
  }
  fs.mkdirSync(ziel, { recursive: true });
  for (const [name, inhalt] of Object.entries(dateien)) fs.writeFileSync(path.join(ziel, name), inhalt);
  console.log(`buendeln: ${Object.keys(dateien).join(', ')} nach ${ziel}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();

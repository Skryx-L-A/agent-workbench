// buendeln.mjs <roh> <aus>: macht aus der Ausgabe von `wasm-bindgen --target web --out-name ansicht` (Ordner <roh>) den
// flachen Ordner <aus> mit genau index.html, ansicht.js und ansicht_bg.wasm (Nachtrag vom 21.11.2026, Form fest).
//   ansicht.js      die Datei von wasm-bindgen; jeder Import aus ./snippets/ wird durch den Inhalt der Datei ersetzt, in einer
//                   eigenen Funktion, die genau die importierten Namen zurueckgibt. Am Ende startet das Modul sich selbst.
//   ansicht_bg.wasm unveraendert.
//   index.html      vorlage.html mit ansicht.css und der Terminalpalette eingebettet.
// Nur node, keine Pakete. Bricht mit Exit 1 ab, wenn ein Import bleibt, ein Snippet selbst etwas importiert, eine Datei
// eine Adresse mit "://" nennt (ausser den Namensraeumen von SVG, XLink, MathML und XHTML im wasm) oder etwas anderes
// als die drei Dateien entstuende.
import fs from 'node:fs';
import path from 'node:path';

const [, , ROH, AUS] = process.argv;
if (!ROH || !AUS) {
  console.error('Aufruf: node buendeln.mjs <roh> <aus>');
  process.exit(64);
}
const HIER = path.dirname(new URL(import.meta.url).pathname);

function fehler(t) {
  console.error('buendeln.mjs: ' + t);
  process.exitCode = 1;
  throw new Error(t);
}

// Die Exporte eines Snippets: `export function|class|const|let NAME` und `export { a, b as c }`.
function snippetAlsFunktion(datei, namen) {
  // Reine Kommentarzeilen fallen weg (eine nennt eine Adresse im Netz: das Ticket von dioxus-interpreter-js 0.7.10); Code
  // und Zeichenketten bleiben unberuehrt, weil nur ganze Zeilen gehen, die mit // beginnen.
  let q = fs.readFileSync(datei, 'utf8').replace(/^[ \t]*\/\/[^\n]*$/gm, '');
  if (/^\s*import\s/m.test(q) || /\bimport\s*\(/.test(q)) fehler(`${datei}: das Snippet importiert selbst`);
  const exporte = new Map();
  q = q.replace(/\bexport\s+(async\s+function|function\*?|class|const|let|var)\s+([A-Za-z_$][\w$]*)/g, (_, art, name) => {
    exporte.set(name, name);
    return `${art} ${name}`;
  });
  q = q.replace(/\bexport\s*\{([^}]*)\}\s*;?/g, (_, liste) => {
    for (const teil of liste.split(',').map((s) => s.trim()).filter(Boolean)) {
      const m = /^([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/.exec(teil);
      if (!m) fehler(`${datei}: Export ${teil} nicht lesbar`);
      exporte.set(m[2] || m[1], m[1]);
    }
    return '';
  });
  if (/\bexport\b\s/.test(q.replace(/(["'`])(?:\\.|(?!\1)[^\\])*\1/g, ''))) fehler(`${datei}: ein Export bleibt`);
  const rueck = namen.map((n) => {
    const lokal = exporte.get(n.importiert);
    if (!lokal) fehler(`${datei}: ${n.importiert} wird nicht exportiert`);
    return `${n.importiert}: ${lokal}`;
  });
  return `(() => {\n${q}\nreturn { ${rueck.join(', ')} };\n})()`;
}

function buendeln() {
  // Auch im Klebecode von wasm-bindgen fallen reine Kommentarzeilen weg (eine nennt eine Seite von MDN).
  let js = fs.readFileSync(path.join(ROH, 'ansicht.js'), 'utf8').replace(/^[ \t]*\/\/[^\n]*$/gm, '');
  const wasm = fs.readFileSync(path.join(ROH, 'ansicht_bg.wasm'));
  let n = 0;
  js = js.replace(/^import\s*\{([^}]*)\}\s*from\s*'(\.\/snippets\/[^']+)';\s*$/gm, (_, liste, ziel) => {
    const namen = liste.split(',').map((s) => s.trim()).filter(Boolean).map((t) => {
      const m = /^([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/.exec(t);
      if (!m) fehler(`Import ${t} nicht lesbar`);
      return { importiert: m[1], lokal: m[2] || m[1] };
    });
    n += 1;
    const ziele = namen.map((x) => (x.lokal === x.importiert ? x.lokal : `${x.importiert}: ${x.lokal}`));
    return `const { ${ziele.join(', ')} } = ${snippetAlsFunktion(path.join(ROH, ziel), namen)};`;
  });
  if (/^\s*import\s/m.test(js) || js.includes('snippets/')) fehler('ansicht.js: ein Import bleibt');
  if (!/\bexport \{ initSync, __wbg_init as default \};\s*$/.test(js)) fehler('ansicht.js: Ende der Datei unbekannt');
  // Die Seite darf kein Skript im HTML tragen (CSP script-src 'self'): das Modul startet sich selbst.
  js += '\n__wbg_init();\n';

  const css = fs.readFileSync(path.join(HIER, 'ansicht.css'), 'utf8').replace('/*PALETTE*/', palette());
  const vorlage = fs.readFileSync(path.join(HIER, 'vorlage.html'), 'utf8');
  if (!vorlage.includes('/*STILE*/')) fehler('vorlage.html ohne Marke STILE');
  const html = vorlage.replace('/*STILE*/', () => css.trimEnd());

  for (const [name, inhalt] of [['index.html', html], ['ansicht.js', js]]) {
    const i = inhalt.indexOf('://');
    if (i >= 0) fehler(`${name} nennt eine Adresse: ${JSON.stringify(inhalt.slice(Math.max(0, i - 40), i + 40))}`);
  }
  const ns = ['http://www.w3.org/2000/svg', 'http://www.w3.org/1999/xlink', 'http://www.w3.org/1998/Math/MathML',
    'http://www.w3.org/1999/xhtml', 'http://www.w3.org/XML/1998/namespace', 'http://www.w3.org/2000/xmlns/'];
  const text = wasm.toString('latin1');
  for (let i = text.indexOf('://'); i >= 0; i = text.indexOf('://', i + 3)) {
    const vorn = text.lastIndexOf('http', i);
    const stueck = text.slice(vorn, vorn + 40);
    if (vorn < 0 || i - vorn > 5 || !ns.some((x) => stueck.startsWith(x))) {
      fehler(`ansicht_bg.wasm nennt eine Adresse: ${JSON.stringify(text.slice(Math.max(0, i - 20), i + 40))}`);
    }
  }

  fs.mkdirSync(AUS, { recursive: true });
  for (const alt of fs.readdirSync(AUS)) {
    if (!['index.html', 'ansicht.js', 'ansicht_bg.wasm'].includes(alt)) fehler(`${AUS}: fremde Datei ${alt}`);
  }
  fs.writeFileSync(path.join(AUS, 'index.html'), html);
  fs.writeFileSync(path.join(AUS, 'ansicht.js'), js);
  fs.writeFileSync(path.join(AUS, 'ansicht_bg.wasm'), wasm);
  console.log(`buendeln.mjs: ${n} Snippets eingebettet; index.html ${Buffer.byteLength(html)} B, ansicht.js ${Buffer.byteLength(js)} B, ansicht_bg.wasm ${wasm.length} B`);
}

// Klassen der Zeilenform (Vertrag rasterauszug Abschnitt 4): 0 bis 15 aus den Rollen des Themas, 16 bis 255 die feste
// Palette von xterm (Wuerfel 6 x 6 x 6 und 24 Graustufen).
function palette() {
  const hex = (r, g, b) => '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('');
  const stufen = [0, 95, 135, 175, 215, 255];
  const farbe = (i) => {
    if (i < 16) return `var(--term-${i})`;
    if (i < 232) {
      const j = i - 16;
      return hex(stufen[Math.floor(j / 36)], stufen[Math.floor(j / 6) % 6], stufen[j % 6]);
    }
    const g = 8 + 10 * (i - 232);
    return hex(g, g, g);
  };
  const z = ['/* Palette der Zeilenform, erzeugt von buendeln.mjs */',
    '.f-grund { color: var(--raster-grund); }', '.b-vorn { background-color: var(--raster-vorn); }'];
  for (let i = 0; i < 256; i++) z.push(`.f-${i} { color: ${farbe(i)}; }`);
  for (let i = 0; i < 256; i++) z.push(`.b-${i} { background-color: ${farbe(i)}; }`);
  z.push('.fett { font-weight: bold; }', '.dimm { opacity: 0.6; }', '.kursiv { font-style: italic; }',
    '.unterstrichen { text-decoration-line: underline; }', '.verdeckt { color: transparent; }',
    '.durchgestrichen { text-decoration-line: line-through; }',
    '.unterstrichen.durchgestrichen { text-decoration-line: underline line-through; }');
  return z.join('\n');
}

try {
  buendeln();
} catch (e) {
  if (!process.exitCode) {
    console.error('buendeln.mjs: ' + e.message);
    process.exitCode = 1;
  }
}

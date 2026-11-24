// Test von `zeileZuHtml` (Zeilenform nach DOM) und der Klassen in raster.css. Exit 0 nur wenn alles stimmt.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { zeileZuHtml } from '../raster.js';
import { css } from '../erzeuge-css.mjs';
import { alsDaten, dokument } from './dom_attrappe.mjs';

const hier = fileURLToPath(new URL('.', import.meta.url));
const daten = JSON.parse(gunzipSync(readFileSync(`${hier}daten/gleichheit.json.gz`)).toString('utf-8'));
let pruefungen = 0;
function gleich(ist, erwartet, text) {
  pruefungen++;
  assert.deepStrictEqual(ist, erwartet, text);
}

const html = (zeile) => alsDaten(zeileZuHtml(zeile, dokument));

// ---- jede SGR-Nummer aus RA1: die Referenz liest den Text, die Klassen folgen der Nummer (hier unabhaengig von html.js aufgestellt)
const ATTR = { 1: 'fett', 2: 'dimm', 3: 'kursiv', 4: 'unterstrichen', 8: 'verdeckt', 9: 'durchgestrichen' };
function klassenVonNummer(folge) {
  // nur einzelne Anweisungen; Rueckgabe { klassen, stil } oder null (Vorgabestil: kein span)
  const p = folge.split(';').map(Number);
  const n = p[0];
  if (p.length === 1) {
    if ([0, 22, 23, 24, 27, 28, 29, 39, 49].includes(n)) return null;
    if (ATTR[n]) return { klassen: [ATTR[n]], stil: {} };
    if (n === 7) return { klassen: ['f-grund', 'b-vorn'], stil: {} };
    if (n >= 30 && n <= 37) return { klassen: [`f-${n - 30}`], stil: {} };
    if (n >= 40 && n <= 47) return { klassen: [`b-${n - 40}`], stil: {} };
    if (n >= 90 && n <= 97) return { klassen: [`f-${n - 90 + 8}`], stil: {} };
    if (n >= 100 && n <= 107) return { klassen: [`b-${n - 100 + 8}`], stil: {} };
  } else if (p[1] === 5) {
    return { klassen: [`${n === 38 ? 'f' : 'b'}-${p[2]}`], stil: {} };
  } else if (p[1] === 2) {
    const hex = '#' + p.slice(2, 5).map((x) => x.toString(16).padStart(2, '0')).join('');
    return { klassen: [], stil: { [n === 38 ? 'color' : 'background-color']: hex } };
  }
  throw new Error(`keine Erwartung fuer ${folge}`);
}

const FOLGEN = ['1;31;44', '7;31', '7;41', '7;38;5;100;48;5;200', '7;38;2;1;2;3;48;2;4;5;6', '4;9', '1;2;3;4;7;8;9', '31;1;0', '1;22', '2;22', '7;27', '8;28', '9;29', '4;24', '3;23'];
const EINZELN = /^\x1b\[([0-9;]+)mX$/;
let einzel = 0;
for (const e of daten.sgr) {
  const m = EINZELN.exec(e.sgr);
  if (!m || FOLGEN.includes(m[1])) continue;
  const erw = klassenVonNummer(m[1]);
  einzel++;
  gleich(html(e.laeufe), erw === null ? ['X'] : [{ tag: 'SPAN', klassen: erw.klassen, stil: erw.stil, kinder: ['X'] }], `SGR ${m[1]}`);
}

// ---- Folgen: Erwartung von Hand (Reihenfolge der Klassen: Attribute, dann Vordergrund, dann Hintergrund; bei invers zuerst die Vorgabeseiten)
const span = (klassen, kinder, stil = {}) => ({ tag: 'SPAN', klassen, stil, kinder });
const lauf = (sgr) => daten.sgr.find((e) => e.sgr === sgr).laeufe;
gleich(html(lauf('\x1b[1;31;44mX')), [span(['fett', 'f-1', 'b-4'], ['X'])], '1;31;44');
gleich(html(lauf('\x1b[7;31mX')), [span(['f-grund', 'b-1'], ['X'])], '7;31: Tausch, Hintergrund wird rot, Schrift die Grundfarbe');
gleich(html(lauf('\x1b[7;41mX')), [span(['b-vorn', 'f-1'], ['X'])], '7;41: Schrift rot, Grund die Vordergrundfarbe');
gleich(html(lauf('\x1b[7;38;5;100;48;5;200mX')), [span(['f-200', 'b-100'], ['X'])], '7 mit zwei Paletten');
gleich(html(lauf('\x1b[7;38;2;1;2;3;48;2;4;5;6mX')), [span([], ['X'], { color: '#040506', 'background-color': '#010203' })], '7 mit Truecolor');
gleich(html(lauf('\x1b[4;9mX')), [span(['unterstrichen', 'durchgestrichen'], ['X'])], '4;9');
gleich(html(lauf('\x1b[1;2;3;4;7;8;9mX')), [span(['fett', 'dimm', 'kursiv', 'unterstrichen', 'verdeckt', 'durchgestrichen', 'f-grund', 'b-vorn'], ['X'])], 'alle Attribute');
gleich(html(lauf('\x1b[31;1;0mX')), ['X'], 'Reset am Ende der Folge: Vorgabestil, kein span');
gleich(html(lauf('\x1b[1;22mX')), ['X'], '22 loescht fett');
gleich(html(lauf('a\x1b[1mb\x1b[0mc')), ['a', span(['fett'], ['b']), 'c'], 'Vorgabestil als Textknoten, Rest als span');
gleich(html(lauf('\x1b[31m  \x1b[0m')), [span(['f-1'], ['  '])], 'Leerzeichen im Stil bleiben');
gleich(html(lauf('ab\x1b[44m  ')), ['ab', span(['b-4'], ['  '])], 'Leerzeichen im Stil am Zeilenende bleiben');

// ---- Truecolor und die Grenzen der Palette
gleich(html([[0x01ffffff, 0, 0, 'w']]), [span([], ['w'], { color: '#ffffff' })], 'Truecolor weiss');
gleich(html([[0x01000000, 0x01123456, 0, 'k']]), [span([], ['k'], { color: '#000000', 'background-color': '#123456' })], 'Truecolor schwarz');
gleich(html([[1, 0, 0, 'a'], [256, 256, 0, 'b']]), [span(['f-0'], ['a']), span(['f-255', 'b-255'], ['b'])], 'Palette 1 und 256');
gleich(html([]), [], 'leere Zeile');
gleich(html([[0, 0, 0, 'nur Text']]), ['nur Text'], 'Vorgabestil');

// ---- Text bleibt Text: <, >, &, Anfuehrungszeichen, Steuerzeichen, Zeilentrenner, Bidi, Netz-Adressen
const TEXTE = ['<b>fett</b>', '<script>alert(1)</script>', '&amp; &lt; &#x3c; &nbsp;', '"><img src=x onerror=alert(1)>', "'\" `${x}`", 'a\x00b\x07c\x1bd\x7f', '\u0085  ‮​﻿', 'x\ty\nz\r', '<!-- -->', ']]>', '漢字 👍🏽 é  '];
for (const t of TEXTE) {
  gleich(html([[0, 0, 0, t]]), [t], `Vorgabestil: ${JSON.stringify(t)}`);
  gleich(html([[3, 4, 1, t]]), [span(['fett', 'f-2', 'b-3'], [t])], `Stil: ${JSON.stringify(t)}`);
  gleich(html([[0x01ff0000, 0x0100ff00, 64, t]]), [span(['durchgestrichen'], [t], { color: '#ff0000', 'background-color': '#00ff00' })], `Truecolor: ${JSON.stringify(t)}`);
}
// Der Text steckt nie in einem anderen Knoten als einem Textknoten; es entstehen nur `span` und Text
const mix = html([[1, 0, 0, '<i>'], [0, 2, 0, '&'], [0, 0, 0, '>']]);
gleich(mix.map((k) => (typeof k === 'string' ? 'text' : k.tag)), ['SPAN', 'SPAN', 'text'], 'nur span und Text');
// keine Erzeugung ueber HTML-Text: die Quelle benutzt weder innerHTML noch outerHTML noch insertAdjacentHTML noch document.write
const quelle = readFileSync(`${hier}../html.js`, 'utf-8');
for (const verboten of ['innerHTML', 'outerHTML', 'insertAdjacentHTML', 'document.write', 'eval(', 'Function(']) {
  pruefungen++;
  assert.ok(!quelle.includes(verboten), `html.js enthaelt ${verboten}`);
}

// ---- Die Klassen von raster.css: jede Klasse, die zeileZuHtml erzeugen kann, ist definiert; die Datei ist die erzeugte
const cssText = readFileSync(`${hier}../raster.css`, 'utf-8');
gleich(cssText, css(), 'raster.css gleicht der Ausgabe von erzeuge-css.mjs');
const klassen = new Set();
for (let c = 1; c <= 256; c++) {
  for (const fl of [0, 16]) for (const z of [[c, 0, fl, 'x'], [0, c, fl, 'x']]) for (const k of html([z])[0].klassen) klassen.add(k);
}
for (let fl = 1; fl < 128; fl++) for (const k of html([[0, 0, fl, 'x']])[0].klassen) klassen.add(k);
for (const k of klassen) {
  pruefungen++;
  // Jedes Sonderzeichen eines regulaeren Ausdrucks maskieren, auch den Backslash
  assert.ok(new RegExp(`\\.${k.replace(/[\\^$.*+?()[\]{}|\/-]/g, '\\$&')} \\{`).test(cssText), `Klasse ${k} fehlt in raster.css`);
}

console.log(`html: ${einzel} einzelne SGR-Nummern und Farben, ${TEXTE.length} Texte in je drei Stilen, ${klassen.size} Klassen in raster.css, ${pruefungen} Pruefungen, alles gleich`);

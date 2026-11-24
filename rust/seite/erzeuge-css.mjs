// Schreibt raster.css (die Klassen, die html.js verwendet). Aufruf: node erzeuge-css.mjs > raster.css
// Die Palette ist die uebliche xterm-Palette: 0 bis 15 benannt, 16 bis 231 ein Farbwuerfel 6x6x6, 232 bis 255 Graustufen.
import { ATTRIBUTE } from './html.js';

const BENANNT = [
  '#000000', '#cd0000', '#00cd00', '#cdcd00', '#0000ee', '#cd00cd', '#00cdcd', '#e5e5e5',
  '#7f7f7f', '#ff0000', '#00ff00', '#ffff00', '#5c5cff', '#ff00ff', '#00ffff', '#ffffff',
];

const ATTRIBUT_REGELN = {
  fett: 'font-weight: bold;',
  dimm: 'opacity: 0.6;',
  kursiv: 'font-style: italic;',
  unterstrichen: 'text-decoration-line: underline;',
  verdeckt: 'color: transparent;',
  durchgestrichen: 'text-decoration-line: line-through;',
};

export function palette(n) {
  if (n < 16) return BENANNT[n];
  if (n < 232) {
    const i = n - 16;
    const stufe = (x) => (x === 0 ? 0 : 55 + 40 * x);
    const h = (v) => stufe(v).toString(16).padStart(2, '0');
    return `#${h(Math.floor(i / 36))}${h(Math.floor(i / 6) % 6)}${h(i % 6)}`;
  }
  const g = (8 + 10 * (n - 232)).toString(16).padStart(2, '0');
  return `#${g}${g}${g}`;
}

export function css() {
  const z = [];
  z.push('/* Erzeugt von erzeuge-css.mjs; nicht von Hand aendern. */');
  z.push(':root { --raster-vorn: #e5e5e5; --raster-grund: #000000; }');
  z.push('.raster { background: var(--raster-grund); color: var(--raster-vorn); font-family: ui-monospace, Menlo, Consolas, monospace; }');
  z.push('.raster-zeile { white-space: pre; min-height: 1lh; }');
  z.push('.f-grund { color: var(--raster-grund); }');
  z.push('.b-vorn { background-color: var(--raster-vorn); }');
  for (let n = 0; n < 256; n++) z.push(`.f-${n} { color: ${palette(n)}; }`);
  for (let n = 0; n < 256; n++) z.push(`.b-${n} { background-color: ${palette(n)}; }`);
  // Attribute zuletzt: `verdeckt` muss die Farbklassen uebersteuern.
  for (const [, name] of ATTRIBUTE) z.push(`.${name} { ${ATTRIBUT_REGELN[name]} }`);
  z.push('.unterstrichen.durchgestrichen { text-decoration-line: underline line-through; }');
  return z.join('\n') + '\n';
}

if (import.meta.url === `file://${process.argv[1]}`) process.stdout.write(css());

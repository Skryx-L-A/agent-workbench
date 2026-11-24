// Anzeige des Terminals (Vertrag `seite` SE4, SE5 Punkt 2, SE7 Punkt 3): das Raster als Text im DOM, je Zeile ein Element
// `terminal-zeile-<n>`, der Cursor als Attribut `data-cursor` und als Markierung, die Groesse aus der Flaeche gemessen.

import { zeileZuHtml } from './html.js';

export const KLEINSTE_SPALTEN = 2;
export const KLEINSTE_ZEILEN = 1;
export const GROESSTE = 1000;

// Text einer Zeile der Zeilenform: die Texte ihrer Laeufe aneinandergehaengt (SE4 Punkt 1).
export function zeilenText(zeile) {
  let s = '';
  for (const l of zeile) s += l[3];
  return s;
}

// Spalten und Zeilen aus der Flaeche, geklemmt auf 2 bis 1000 Spalten und 1 bis 1000 Zeilen (SE5 Punkt 2).
export function klemme(breite, hoehe, zeichenBreite, zeilenHoehe) {
  const sp = Math.floor(breite / (zeichenBreite > 0 ? zeichenBreite : 8));
  const ze = Math.floor(hoehe / (zeilenHoehe > 0 ? zeilenHoehe : 17));
  return [
    Math.max(KLEINSTE_SPALTEN, Math.min(GROESSTE, Number.isFinite(sp) ? sp : KLEINSTE_SPALTEN)),
    Math.max(KLEINSTE_ZEILEN, Math.min(GROESSTE, Number.isFinite(ze) ? ze : KLEINSTE_ZEILEN)),
  ];
}

export class TerminalAnsicht {
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

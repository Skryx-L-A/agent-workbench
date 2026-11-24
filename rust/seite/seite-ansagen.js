// Ansagen fuer den Vorleser (Vertrag `seite` SE3 Punkt 3 und 4): eine hoefliche Live-Region; die Ausgabe des Terminals gesammelt,
// hoechstens eine Ausgabe-Ansage je 1000 ms Seitenuhr und hoechstens die letzten 5 Zeilen je Ansage.

export const SPERRZEIT_MS = 1000;
export const HOECHSTENS_ZEILEN = 5;
const BEHALTEN = 20;

export class Ansager {
  constructor(region) {
    this.region = region;
    this.sperreBis = -Infinity;
    this.sammlung = new Map();
    this.wecker = null;
  }

  // Eine Ansage sofort: ein neuer Knoten in der Region.
  sage(text) {
    const d = this.region.ownerDocument.createElement('div');
    d.textContent = text;
    this.region.append(d);
    while (this.region.childElementCount > BEHALTEN) this.region.firstElementChild.remove();
  }

  // Neue Zeilen der Ausgabe (Index -> Text). Ohne Sperre sofort, sonst am Ende der Sperrzeit.
  zeilen(neu) {
    for (const [i, t] of neu) this.sammlung.set(i, t);
    if (!this.sammlung.size) return;
    const jetzt = performance.now();
    if (jetzt >= this.sperreBis) this.ausgabe();
    else if (this.wecker === null) this.wecker = setTimeout(() => this.ausgabe(), this.sperreBis - jetzt);
  }

  ausgabe() {
    this.wecker = null;
    if (!this.sammlung.size) return;
    const idx = [...this.sammlung.keys()].sort((a, b) => a - b).slice(-HOECHSTENS_ZEILEN);
    const text = idx.map((i) => this.sammlung.get(i)).join('\n');
    this.sammlung = new Map();
    this.sperreBis = performance.now() + SPERRZEIT_MS;
    this.sage(text);
  }
}

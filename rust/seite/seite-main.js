// Die Seite des Durchstichs (Vertrag `seite`): Sitzungsliste, Terminal, Eingabezeile und Startformular an der Bruecke.
// Einstieg des Buendlers (`buendeln.mjs`); alle anderen Module kommen ueber `import`.

import { Empfaenger, RasterFehler } from './raster.js';
import { deuteTaste } from './seite-tasten.js';
import { tokenAusFragment, Sender, starten, oeffneStrom } from './seite-netz.js';
import { Ansager } from './seite-ansagen.js';
import { TerminalAnsicht, zeilenText } from './seite-terminal.js';
import { setzeOptionen, eintraegeAusStartwahl, modellEintraege, startStruktur } from './seite-start.js';

// Texte der Ablehnungen ohne eigenes Feld `text` (SE3 Punkt 3); jeder andere Grund steht mit seinem Code.
const TEXTE = {
  taste_unbekannt: 'Taste nicht belegt',
  sitzung_beendet: 'Sitzung beendet',
  kein_abo: 'kein Abo',
  pane_busy: 'Dialog offen',
  steuerzeichen_im_text: 'Steuerzeichen im Text',
};
// Fehler des Rasters, nach denen der Empfaenger der Verbindung unbrauchbar ist (Vertrag `rasterauszug` RA11).
const UNHEILBAR = new Set(['inflate_fehler', 'rahmen_zu_gross']);

const el = (id) => document.getElementById(id);
const liste = el('liste');
const listeLeer = el('liste-leer');
const terminal = el('terminal');
const eingang = el('eingang');
const eingabe = el('eingabe');
const startForm = el('start');
const verbindungEl = el('verbindung');
const wahl = {
  harness: el('wahl-harness'),
  modell: el('wahl-modell'),
  route: el('wahl-route'),
  kunde: el('wahl-kunde'),
};
const arbeitsordner = el('wahl-arbeitsordner');

const token = tokenAusFragment(location.hash);
const sender = new Sender(token);
const ansager = new Ansager(el('ansagen'));
const ansicht = new TerminalAnsicht({
  terminal, raster: el('raster'), inhalt: el('raster-inhalt'), cursor: el('cursor'), messung: el('messung'), eingang,
});

let empfaenger = null;
let stromZu = null;
let sitzungen = [];
let bekannt = new Map(); // Kennung -> Zustand der vorigen Liste
let ersteListe = true;
let aktiv = null; // Kennung der abonnierten Sitzung
const gemeldet = new Map(); // Kennung -> [spalten, zeilen], zuletzt gemeldet

// Sitzungsnummer nach `DI` DI12 Punkt 2: `s-<n>` hat die Nummer n, die Sitzung aus `kind` die 0.
const nrVon = (id) => {
  const r = /^s-(\d+)$/.exec(id);
  return r ? Number(r[1]) : 0;
};

function verbindungZeigen(zustand, text) {
  verbindungEl.dataset.zustand = zustand;
  verbindungEl.querySelector('.verbindung-text').textContent = text;
}

// ---- Ereignisse an die Bruecke ----
function sendeText(t) {
  if (aktiv === null || t === '') return;
  sender.sende({ art: 'Text', sitzung: aktiv, text: t });
}
function sendeTaste(taste, umschalter) {
  if (aktiv === null) return;
  sender.sende({ art: 'Taste', sitzung: aktiv, taste, umschalter });
}

// ---- Fehlerfeld und Ablehnungen (SE3 Punkt 3, SE7 Punkt 5) ----
function zeigeFehler(code, text) {
  el('fehler-code').textContent = code;
  el('fehler-text').textContent = text;
  el('fehler').hidden = false;
}
function ablehnung(j) {
  const code = j && typeof j.code === 'string' && j.code ? j.code : (j && typeof j.grund === 'string' ? j.grund : '');
  const t = j && typeof j.text === 'string' && j.text ? j.text : (TEXTE[code] || code);
  zeigeFehler(code, t);
  ansager.sage('Abgelehnt: ' + t);
}

// ---- Groesse (SE5 Punkt 2) ----
function abo(id) {
  const [sp, ze] = ansicht.messen();
  gemeldet.set(id, [sp, ze]);
  sender.sende({ art: 'Abo', sitzung: id, an: true, spalten: sp, zeilen: ze });
}
function groesseMelden() {
  if (aktiv === null || !gemeldet.has(aktiv)) return;
  const [sp, ze] = ansicht.messen();
  const alt = gemeldet.get(aktiv);
  if (sp === alt[0] && ze === alt[1]) return;
  gemeldet.set(aktiv, [sp, ze]);
  sender.sende({ art: 'Groesse', sitzung: aktiv, spalten: sp, zeilen: ze });
}
new ResizeObserver(() => groesseMelden()).observe(el('raster'));

// ---- Sitzungsliste (SE2 Punkt 5, SE3, SE7 Punkt 1) ----
function zustandText(s) {
  return s.zustand === 'laeuft' ? 'läuft' : `beendet, Code ${s.code}`;
}
function terminalBenennen() {
  eingang.setAttribute('aria-label', aktiv === null ? 'Terminal, keine Sitzung' : 'Terminal ' + aktiv);
  terminal.setAttribute('aria-label', aktiv === null ? 'Terminal' : 'Terminal ' + aktiv);
  el('terminal-titel').textContent = aktiv === null ? 'Keine Sitzung gewählt' : aktiv;
}
function waehle(id, fokus) {
  if (id === aktiv) {
    if (fokus) eingang.focus({ preventScroll: true });
    return;
  }
  if (aktiv !== null) sender.sende({ art: 'Abo', sitzung: aktiv, an: false });
  aktiv = id;
  terminalBenennen();
  zeigeListe();
  ansicht.zeige(empfaenger ? empfaenger.abbild(nrVon(id)) || null : null);
  abo(id);
  if (fokus) eingang.focus({ preventScroll: true });
}
function zeigeListe() {
  const doc = liste.ownerDocument;
  liste.textContent = '';
  for (const s of sitzungen) {
    const li = doc.createElement('li');
    const b = doc.createElement('button');
    b.type = 'button';
    b.className = 'sitzung';
    b.dataset.testid = 'sitzung-' + s.id;
    b.dataset.zustand = s.zustand === 'laeuft' ? 'laeuft' : 'beendet';
    if (s.zustand !== 'laeuft') b.dataset.code = String(s.code);
    const teile = [s.harness, s.modell].filter((x) => typeof x === 'string' && x);
    b.setAttribute('aria-label', `${[s.id, ...teile].join(' ')}, ${zustandText(s)}`);
    if (s.id === aktiv) b.setAttribute('aria-current', 'true');
    const punkt = doc.createElement('span');
    punkt.className = 'sitzung-punkt';
    const name = doc.createElement('span');
    name.className = 'sitzung-name';
    name.textContent = s.id;
    const art = doc.createElement('span');
    art.className = 'sitzung-art';
    art.textContent = [...teile, typeof s.kunde === 'string' && s.kunde ? s.kunde : ''].filter(Boolean).join(' · ');
    const zustand = doc.createElement('span');
    zustand.className = 'sitzung-zustand';
    zustand.textContent = zustandText(s);
    b.append(punkt, name, zustand, art);
    b.addEventListener('click', () => waehle(s.id, true));
    li.append(b);
    liste.append(li);
  }
  listeLeer.hidden = sitzungen.length > 0;
}
function sitzungsliste(n) {
  sitzungen = Array.isArray(n.sitzungen) ? n.sitzungen.filter((s) => s && typeof s.id === 'string') : [];
  for (const s of sitzungen) {
    if (s.zustand !== 'laeuft' && bekannt.get(s.id) === 'laeuft') ansager.sage(`Sitzung ${s.id} beendet, Code ${s.code}`);
  }
  bekannt = new Map(sitzungen.map((s) => [s.id, s.zustand]));
  zeigeListe();
  if (ersteListe) {
    ersteListe = false;
    const erste = sitzungen.find((s) => s.zustand === 'laeuft');
    if (erste) waehle(erste.id, true);
    else wahl.harness.focus();
  }
}

// ---- Startwahl (DI12 Punkt 10) ----
let routenModelle = new Map();
let startwahlDa = false; // bis zur ersten `Startwahl` bleiben die festen Beispiel-Eintraege des Felds `Modell`
// Das Feld `Modell` hat genau einen Eintrag: das Modell der gewaehlten Route, sonst `auto`. Es folgt jeder Wahl der Route.
function fuelleModell() {
  const eintraege = modellEintraege(routenModelle, wahl.route.value);
  setzeOptionen(wahl.modell, eintraege, eintraege[0].wert);
  wahl.modell.value = eintraege[0].wert;
}
function startwahl(n) {
  const e = eintraegeAusStartwahl(n);
  if (!e) return;
  if (e.harness.length) setzeOptionen(wahl.harness, e.harness);
  if (e.routen.length) setzeOptionen(wahl.route, e.routen);
  setzeOptionen(wahl.kunde, e.kunden, '');
  routenModelle = e.modelle;
  startwahlDa = true;
  fuelleModell();
}
wahl.route.addEventListener('change', () => { if (startwahlDa) fuelleModell(); });

// ---- Rasterrahmen (SE3 Punkt 3, SE4) ----
function rasterRahmen(nutz) {
  let ergebnis;
  try {
    ergebnis = empfaenger.nimm(nutz);
  } catch (e) {
    if (e instanceof RasterFehler && UNHEILBAR.has(e.code) && stromZu) {
      zeigeFehler(e.code, 'Bild der Verbindung nicht lesbar');
      stromZu();
    }
    return; // ein ungueltiger Rahmen aendert nichts (RA10)
  }
  const { auszug, abbild } = ergebnis;
  if (aktiv === null || abbild.nr !== nrVon(aktiv)) return;
  const voll = auszug.groesse !== null;
  ansicht.zeige(abbild, voll || auszug.schub ? null : auszug.zeilen.map(([i]) => i));
  if (voll) return; // ein Vollbild sagt nichts an
  const neu = new Map();
  for (const [i] of auszug.zeilen) {
    const t = zeilenText(abbild.lines[i]);
    if (!t || i === abbild.cursor[0]) continue;
    neu.set(i, t);
  }
  ansager.zeilen(neu);
}

// ---- Strom ----
function nachricht(n) {
  if (!n || typeof n !== 'object') return;
  if (n.art === 'Sitzungsliste') sitzungsliste(n);
  else if (n.art === 'Startwahl') startwahl(n);
  else if (n.art === 'Hinweis' && n.hinweis === 'osc52_geschrieben' && (n.sitzung === undefined || n.sitzung === aktiv)) {
    ansager.sage('Das Programm hat in die Zwischenablage geschrieben');
  }
}
function rahmen(art, nutz) {
  if (art === 5) {
    rasterRahmen(nutz);
    return;
  }
  let j;
  try {
    j = JSON.parse(new TextDecoder().decode(nutz));
  } catch {
    return;
  }
  if (art === 6) nachricht(j);
  else if (art === 7) ablehnung(j);
  else if (art === 8 && j && typeof j.fehler === 'string') zeigeFehler(j.fehler, 'Verbindung mit Fehler beendet');
}
function verbinden() {
  verbindungZeigen('warte', 'verbindet');
  stromZu = oeffneStrom(token, {
    beiOffen: ({ verbindung, deflate }) => {
      sender.verbindung = verbindung;
      empfaenger = new Empfaenger({ deflate });
      verbindungZeigen('offen', 'verbunden');
    },
    beiRahmen: rahmen,
    beiEnde: () => {
      sender.verbindung = null;
      stromZu = null;
      verbindungZeigen('zu', 'getrennt, Seite neu laden');
      ansager.sage('Verbindung getrennt');
    },
  });
}

// ---- Tastatur im Terminal (SE1, SE2 Punkt 3 und 4) ----
function haltestellen() {
  return [...document.querySelectorAll('button, input, select, textarea')]
    .filter((x) => !x.disabled && x.tabIndex >= 0 && !x.closest('[hidden]'));
}
eingang.addEventListener('keydown', (e) => {
  const d = deuteTaste(e);
  if (!d) return;
  e.preventDefault();
  if (d.fokus) {
    const halte = haltestellen();
    const ziel = halte[halte.indexOf(eingang) + d.fokus];
    if (ziel) ziel.focus();
    return;
  }
  if (d.art === 'Taste') sendeTaste(d.taste, d.umschalter);
  else sendeText(d.text);
});
eingang.addEventListener('compositionend', (e) => {
  sendeText(e.data);
  eingang.value = '';
});
eingang.addEventListener('input', (e) => {
  if (e.isComposing) return;
  if ((e.inputType === 'insertText' || e.inputType === 'insertReplacementText') && e.data) sendeText(e.data);
  eingang.value = '';
});
eingang.addEventListener('paste', (e) => {
  e.preventDefault();
  const t = e.clipboardData ? e.clipboardData.getData('text/plain') : '';
  if (!t || aktiv === null) return;
  sender.sende({ art: 'Einfuegen', sitzung: aktiv, text: t });
});
eingang.addEventListener('focus', () => terminal.classList.add('fokus'));
eingang.addEventListener('blur', () => terminal.classList.remove('fokus'));
// Klick oder Tippen in das Raster setzt den Fokus ins Terminal, ausser beim Markieren von Text.
el('raster').addEventListener('click', () => {
  const sel = document.getSelection();
  if (sel && !sel.isCollapsed) return;
  eingang.focus({ preventScroll: true });
});

// ---- Eingabezeile (SE5 Punkt 3) ----
el('zeile-eingabe').addEventListener('submit', (e) => {
  e.preventDefault();
  const t = eingabe.value;
  eingabe.value = '';
  if (t) sendeText(t);
  sendeTaste('Enter', []);
});

// ---- Startformular (SE6 Punkt 3) ----
startForm.addEventListener('submit', async (e) => {
  e.preventDefault();
  if (!token) return;
  const start = startStruktur({
    harness: wahl.harness.value, modell: wahl.modell.value, route: wahl.route.value, kunde: wahl.kunde.value, arbeitsordner: arbeitsordner.value,
  });
  const { status, antwort } = await starten(token, start);
  if (status === 200 && antwort && typeof antwort.sitzung === 'string') {
    el('fehler').hidden = true;
    ansager.sage(`Sitzung ${antwort.sitzung} gestartet`);
  } else if (status === 422) {
    ablehnung(antwort || {});
  } else {
    zeigeFehler(status ? 'http_' + status : 'netz', 'Start gescheitert');
    ansager.sage('Start gescheitert');
  }
});

// ---- Anfang ----
terminalBenennen();
if (token) verbinden();
else verbindungZeigen('zu', 'Adresse ohne Zugangsschlüssel');

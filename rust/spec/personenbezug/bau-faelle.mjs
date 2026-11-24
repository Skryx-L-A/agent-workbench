// Erzeugt die sichtbaren Faelle von Vertrag 9 (README.md in diesem Ordner) nach faelle/.
// Bytes aus der JCS-Referenz (RFC 8785) und node:crypto. Ein eigenes Referenzmodell (Speicher im Speicher,
// Kette, Pruefer fuer Fassung 2 wie Vertrag 5) rechnet jeden Schritt nach; die Absichten der Faelle stehen als
// Behauptungen im Skript. Weicht das Modell von einer Absicht ab, bricht das Skript ab.
// Aufruf: node rust/spec/personenbezug/bau-faelle.mjs (leicht, ohne Abhaengigkeit). Gleicher Stand ergibt gleiche Bytes.
// Alle Kennungen, Schluessel, Geheimnisse und Texte sind erfunden.

import { createHash, createHmac } from 'node:crypto';
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs } from '../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const AUS = join(HIER, 'faelle');

const NULL64 = '0'.repeat(64);
const KENNUNG_FORM = /^[a-z0-9][a-z0-9._-]{0,63}$/; // Kennung nach Vertrag 2
const KENNUNG = { test: (k) => KENNUNG_FORM.test(k) && !/^(ps|ft)_/.test(k) }; // Person: dazu nie die Vorsilbe eines Pseudonyms oder Freitexts
const HEX64 = /^[0-9a-f]{64}$/;
const PS = /^ps_[0-9a-f]{32}$/;
const FT = /^ft_[0-9a-f]{64}$/;
const ZEIT = /^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$/;
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const TEXT_MAX = 4096;
const NICHT_ZUZUORDNEN = 'nicht_zuzuordnen';
const ANDERER = 'anderer';
const KETTE = 'stelle-2026-11.jsonl';

const ZWECK_PS = Buffer.from('wb-person-pseudonym\n', 'ascii');
const ZWECK_FT = Buffer.from('wb-person-freitext\n', 'ascii');
const ZWECK_LM = Buffer.from('wb-person-loeschung\n', 'ascii');

const sha = (s) => createHash('sha256').update(Buffer.from(s, 'utf8')).digest('hex');
const hmac = (k, teile) => createHmac('sha256', Buffer.from(k, 'hex')).update(Buffer.concat(teile)).digest('hex');
class Fehler extends Error {}
const fehler = (code) => new Fehler(code);
const behaupte = (wahr, was) => { if (!wahr) throw new Error(`Absicht verfehlt: ${was}`); };
const zufall = (wort) => sha(`zufall:${wort}`); // feste, lesbare Herkunft der Geheimnisse und Schluessel in den Faellen

// --- Bibliothek (Abschnitt 2), rein ---

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw fehler('zeit_ungueltig');
  return new Date(ms).toISOString();
}

function pseudonym(k, g) {
  if (!HEX64.test(k)) throw fehler('feld_ungueltig:schluessel');
  if (!HEX64.test(g)) throw fehler('feld_ungueltig:geheimnis');
  return `ps_${hmac(k, [ZWECK_PS, Buffer.from(g, 'hex')]).slice(0, 32)}`;
}

function freitext(k, g, text) {
  if (!HEX64.test(k)) throw fehler('feld_ungueltig:schluessel');
  if (!HEX64.test(g)) throw fehler('feld_ungueltig:geheimnis');
  if (typeof text !== 'string' || !text.isWellFormed()) throw fehler('text_ungueltig');
  const zeichen = [...text].length; // Unicode-Skalarwerte
  if (zeichen < 1 || zeichen > TEXT_MAX) throw fehler('text_ungueltig');
  return `ft_${hmac(k, [ZWECK_FT, Buffer.from(g, 'hex'), Buffer.from(text, 'utf8')])}`;
}

function loeschmarke(k, g) {
  if (!HEX64.test(k)) throw fehler('feld_ungueltig:schluessel');
  if (!HEX64.test(g)) throw fehler('feld_ungueltig:geheimnis');
  return hmac(k, [ZWECK_LM, Buffer.from(g, 'hex')]);
}

function zeileGeloescht({ ergebnis, handelnd, marke, zeit_ms }, vorher) {
  if (ergebnis !== 'geloescht' && ergebnis !== 'unbekannt') throw fehler('feld_ungueltig:ergebnis');
  if (handelnd !== null && !(typeof handelnd === 'string' && PS.test(handelnd))) throw fehler('feld_ungueltig:handelnd');
  if (ergebnis === 'geloescht' ? !(typeof marke === 'string' && HEX64.test(marke)) : marke !== null) throw fehler('feld_ungueltig:loeschmarke');
  if (typeof vorher !== 'string' || !HEX64.test(vorher)) throw fehler('feld_ungueltig:vorher');
  const zeit = zeitText(zeit_ms);
  return jcs({ aktionsname: 'person.geloescht', ergebnis, fassung: 2, handelnd, loeschmarke: marke, vorher, zeit });
}

function zeileAuskunft({ ergebnis, handelnd, zeilen, zeit_ms }, vorher) {
  if (ergebnis !== 'geliefert' && ergebnis !== 'unbekannt') throw fehler('feld_ungueltig:ergebnis');
  if (!(typeof handelnd === 'string' && PS.test(handelnd))) throw fehler('feld_ungueltig:handelnd');
  if (!Number.isInteger(zeilen) || zeilen < 0 || zeilen > 4294967295 || (ergebnis === 'unbekannt' && zeilen !== 0)) throw fehler('feld_ungueltig:zeilen');
  if (typeof vorher !== 'string' || !HEX64.test(vorher)) throw fehler('feld_ungueltig:vorher');
  const zeit = zeitText(zeit_ms);
  return jcs({ aktionsname: 'person.auskunft', ergebnis, fassung: 2, handelnd, vorher, zeilen, zeit });
}

// Ersetzt jeden Textwert in jeder Tiefe; Schluessel bleiben.
function abbilden(w, f) {
  if (typeof w === 'string') return f(w);
  if (Array.isArray(w)) return w.map((x) => abbilden(x, f));
  if (w !== null && typeof w === 'object') return Object.fromEntries(Object.entries(w).map(([k, v]) => [k, abbilden(v, f)]));
  return w;
}
function werte(w, aus = []) {
  if (typeof w === 'string') aus.push(w);
  else if (Array.isArray(w)) w.forEach((x) => werte(x, aus));
  else if (w !== null && typeof w === 'object') Object.values(w).forEach((x) => werte(x, aus));
  return aus;
}

// auskunft(zeilen, eigene) rein: zeilen [{datei, nr, roh}], eigene {pseudonyme:Set, freitexte:Set}
function auszugZeilen(zeilen, eigene) {
  const treffer = [];
  for (const z of zeilen) {
    let w;
    try { w = JSON.parse(z.roh); } catch { throw fehler(`zeile_unlesbar:${z.datei}:${z.nr}`); }
    if (w === null || typeof w !== 'object' || Array.isArray(w)) throw fehler(`zeile_unlesbar:${z.datei}:${z.nr}`);
    if (!werte(w).some((v) => eigene.pseudonyme.has(v) || eigene.freitexte.has(v))) continue;
    const maskiert = abbilden(w, (v) => {
      if (PS.test(v) && !eigene.pseudonyme.has(v)) return ANDERER;
      if (FT.test(v) && !eigene.freitexte.has(v)) return ANDERER;
      return v;
    });
    treffer.push({ datei: z.datei, nr: z.nr, sha256: sha(z.roh), zeile: maskiert });
  }
  return treffer;
}

// --- Pruefer der Kette, Fassung 2, nach Vertrag 5 Abschnitt 3 (nur die Schritte, die diese Faelle beruehren) ---

function pruefe(name, inhalt) {
  let prev = NULL64;
  let n = 0;
  const rot = (zeile, grund) => ({ exit: 1, stdout: `${jcs({ bruch: { datei: name, grund, zeile }, letzte: null, ok: false, zeilen: n })}\n` });
  if (inhalt.length > 0 && !inhalt.endsWith('\n')) return rot(inhalt.split('\n').length, 'letzte_zeile_unvollstaendig');
  const stuecke = inhalt.length ? inhalt.slice(0, -1).split('\n') : [];
  for (let i = 0; i < stuecke.length; i += 1) {
    const text = stuecke[i];
    let w;
    try { w = JSON.parse(text); } catch { return rot(i + 1, 'zeile_unlesbar'); }
    if (w === null || typeof w !== 'object' || Array.isArray(w)) return rot(i + 1, 'zeile_unlesbar');
    if (w.fassung !== 2) return rot(i + 1, 'form_falsch');
    if (!('vorher' in w)) return rot(i + 1, 'vorher_fehlt');
    if (w.vorher !== prev) return rot(i + 1, 'kette_gebrochen');
    if (jcs(w) !== text) return rot(i + 1, 'nicht_kanonisch');
    if (typeof w.zeit !== 'string' || !ZEIT.test(w.zeit) || typeof w.aktionsname !== 'string' || typeof w.ergebnis !== 'string') return rot(i + 1, 'form_falsch');
    behaupte(w.aktionsname !== 'ausgabe.gesperrt', 'keine Zeile ausgabe.gesperrt in diesen Faellen');
    prev = sha(text);
    n += 1;
  }
  return { exit: 0, stdout: `${jcs({ bruch: null, letzte: n ? prev : null, ok: true, zeilen: n })}\n` };
}

// --- Referenzmodell der Welt: Speicher (Abschnitt 4), Kette, Ablauf ---

class Welt {
  constructor() {
    this.schluessel = null; // [{ab_ms, wert}]
    this.personen = null; // Map kennung -> {geheimnis, seit_ms, texte: Map ft -> {text, zuletzt_ms}}
    this.kette = '';
  }

  aktuell() { return this.schluessel[this.schluessel.length - 1].wert; }

  oeffnen(zufallHex, ab_ms) {
    zeitText(ab_ms);
    behaupte(this.schluessel === null, 'oeffnen nur einmal je Fall');
    this.schluessel = [{ ab_ms, wert: zufallHex }];
    this.personen = new Map();
  }

  zuordnungText() {
    const personen = {};
    for (const [k, p] of this.personen) {
      const texte = {};
      for (const [ft, t] of p.texte) texte[ft] = { text: t.text, zuletzt_ms: t.zuletzt_ms };
      personen[k] = { geheimnis: p.geheimnis, seit_ms: p.seit_ms, texte };
    }
    return `${jcs({ fassung: 1, personen })}\n`;
  }

  schluesselText() { return `${jcs({ fassung: 1, schluessel: this.schluessel })}\n`; }

  alleVon(g) { return this.schluessel.map((s) => pseudonym(s.wert, g)); }

  anhaengen(roh) {
    behaupte(!roh.includes('\n'), 'Zeile ohne LF');
    this.kette += `${roh}\n`;
    return sha(roh);
  }

  letzteHash() {
    if (this.kette === '') return NULL64;
    const z = this.kette.slice(0, -1).split('\n');
    return sha(z[z.length - 1]);
  }

  zeilen() {
    if (this.kette === '') return [];
    return this.kette.slice(0, -1).split('\n').map((roh, i) => ({ datei: KETTE, nr: i + 1, roh }));
  }

  person(kennung, zufallHex, jetzt_ms) {
    if (typeof kennung !== 'string' || !KENNUNG.test(kennung)) throw fehler('feld_ungueltig:kennung');
    zeitText(jetzt_ms);
    const da = this.personen.get(kennung);
    if (da) return { neu: false, pseudonym: pseudonym(this.aktuell(), da.geheimnis) };
    behaupte(HEX64.test(zufallHex), 'Zufall 64 hex');
    for (const p of this.personen.values()) if (p.geheimnis === zufallHex) throw fehler('geheimnis_doppelt');
    this.personen.set(kennung, { geheimnis: zufallHex, seit_ms: jetzt_ms, texte: new Map() });
    return { neu: true, pseudonym: pseudonym(this.aktuell(), zufallHex) };
  }

  text(kennung, text, jetzt_ms) {
    if (typeof kennung !== 'string' || !KENNUNG.test(kennung)) throw fehler('feld_ungueltig:kennung');
    zeitText(jetzt_ms);
    const p = this.personen.get(kennung);
    if (!p) throw fehler('person_unbekannt');
    const ft = freitext(this.aktuell(), p.geheimnis, text);
    p.texte.set(ft, { text, zuletzt_ms: jetzt_ms }); // derselbe Text steht einmal, mit der Zeit seiner letzten Ablage
    return { freitext: ft };
  }

  auskunft(kennung, handelnd, jetzt_ms) {
    if (typeof kennung !== 'string' || !KENNUNG.test(kennung)) throw fehler('feld_ungueltig:kennung');
    const p = this.personen.get(kennung);
    let antwort;
    if (!p) {
      antwort = { gefunden: false, pseudonyme: [], texte: [], zeilen: [] };
    } else {
      const eigene = { pseudonyme: new Set(this.alleVon(p.geheimnis)), freitexte: new Set(p.texte.keys()) };
      const zeilen = auszugZeilen(this.zeilen(), eigene);
      const texte = [...p.texte].map(([ft, t]) => ({ freitext: ft, text: t.text, zuletzt: zeitText(t.zuletzt_ms) }))
        .sort((a, b) => (a.freitext < b.freitext ? -1 : 1));
      antwort = { gefunden: true, pseudonyme: this.alleVon(p.geheimnis), texte, zeilen };
    }
    // Zeile vor Antwort (E2 von Vertrag 6)
    const roh = zeileAuskunft({ ergebnis: p ? 'geliefert' : 'unbekannt', handelnd, zeilen: antwort.zeilen.length, zeit_ms: jetzt_ms }, this.letzteHash());
    this.anhaengen(roh);
    return { antwort, zeile: roh };
  }

  loeschen(kennung, handelnd, jetzt_ms) {
    if (typeof kennung !== 'string' || !KENNUNG.test(kennung)) throw fehler('feld_ungueltig:kennung');
    const p = this.personen.get(kennung);
    let h = handelnd;
    if (p && h !== null && this.alleVon(p.geheimnis).includes(h)) h = null; // Selbstloeschung nennt keinen Handelnden
    const roh = zeileGeloescht({ ergebnis: p ? 'geloescht' : 'unbekannt', handelnd: h, marke: p ? loeschmarke(this.aktuell(), p.geheimnis) : null, zeit_ms: jetzt_ms }, this.letzteHash());
    this.anhaengen(roh); // erst die Zeile, dann der Speicher (Abschnitt 6)
    if (p) this.personen.delete(kennung);
    return { ergebnis: p ? 'geloescht' : 'unbekannt', zeile: roh };
  }

  zuordnen(wert) {
    for (const [k, p] of this.personen) {
      if (PS.test(wert) && this.alleVon(p.geheimnis).includes(wert)) return k;
      if (FT.test(wert) && p.texte.has(wert)) return k;
    }
    return null;
  }

  einsicht() {
    const karte = new Map();
    for (const [k, p] of this.personen) {
      for (const ps of this.alleVon(p.geheimnis)) karte.set(ps, k);
      for (const [ft, t] of p.texte) karte.set(ft, t.text);
    }
    return this.zeilen().map((z) => ({
      datei: z.datei,
      nr: z.nr,
      zeile: abbilden(JSON.parse(z.roh), (v) => ((PS.test(v) || FT.test(v)) ? (karte.get(v) ?? NICHT_ZUZUORDNEN) : v)),
    }));
  }

  wechseln(wert, ab_ms) {
    zeitText(ab_ms);
    if (!HEX64.test(wert) || this.schluessel.some((s) => s.wert === wert)) throw fehler('schluessel_ungueltig');
    if (ab_ms <= this.schluessel[this.schluessel.length - 1].ab_ms) throw fehler('zeit_ungueltig');
    this.schluessel.push({ ab_ms, wert });
  }

  abgleich() {
    const marken = new Set();
    for (const z of this.zeilen()) {
      const w = JSON.parse(z.roh);
      if (w.aktionsname === 'person.geloescht' && w.ergebnis === 'geloescht') marken.add(w.loeschmarke);
    }
    let entfernt = 0;
    for (const [k, p] of [...this.personen]) {
      if (this.schluessel.some((s) => marken.has(loeschmarke(s.wert, p.geheimnis)))) { this.personen.delete(k); entfernt += 1; }
    }
    return { entfernt };
  }

  zuordnungSetzen(text) {
    const w = JSON.parse(text);
    behaupte(jcs(w) === text.slice(0, -1), 'Speicher kanonisch');
    this.personen = new Map(Object.entries(w.personen).map(([k, p]) => [k, {
      geheimnis: p.geheimnis, seit_ms: p.seit_ms, texte: new Map(Object.entries(p.texte).map(([ft, t]) => [ft, { text: t.text, zuletzt_ms: t.zuletzt_ms }])),
    }]));
  }
}

// Speicher::oeffnen auf einem beschriebenen Ordner (Abschnitt 4): Reihenfolge der Pruefungen wie im Vertrag.
function oeffnenModell({ ordner_modus, dateien, git_darueber = null }) {
  const offen = (m) => (parseInt(m, 8) & 0o077) !== 0;
  if (offen(ordner_modus)) return { fehler: 'speicher_unsicher' };
  if (git_darueber !== null) return { fehler: 'speicher_im_repo' }; // .git im Ordner oder darueber, gleich welcher Art
  for (const n of ['schluessel.json', 'zuordnung.json']) {
    const f = dateien[n];
    if (f && (f.verweis_auf !== undefined || offen(f.modus))) return { fehler: 'speicher_unsicher' };
  }
  const s = dateien['schluessel.json'];
  const z = dateien['zuordnung.json'];
  if (!s && !z) return { ok: true, angelegt: ['schluessel.json', 'zuordnung.json', 'zuordnung.sperre'] };
  if (!s) return { fehler: 'schluessel_fehlt' };
  if (!z) return { fehler: 'zuordnung_fehlt' };
  const lies = (text) => { try { const w = JSON.parse(text); return jcs(w) === text.slice(0, -1) && text.endsWith('\n') ? w : null; } catch { return null; } };
  const sw = lies(s.inhalt);
  const zw = lies(z.inhalt);
  if (!sw || sw.fassung !== 1 || !Array.isArray(sw.schluessel) || sw.schluessel.length === 0) return { fehler: 'speicher_ungueltig' };
  for (let i = 0; i < sw.schluessel.length; i += 1) {
    const e = sw.schluessel[i];
    if (!HEX64.test(e.wert) || !Number.isInteger(e.ab_ms) || (i > 0 && e.ab_ms <= sw.schluessel[i - 1].ab_ms)) return { fehler: 'speicher_ungueltig' };
  }
  if (new Set(sw.schluessel.map((e) => e.wert)).size !== sw.schluessel.length) return { fehler: 'speicher_ungueltig' };
  if (!zw || zw.fassung !== 1 || typeof zw.personen !== 'object' || zw.personen === null) return { fehler: 'speicher_ungueltig' };
  const g = Object.values(zw.personen).map((p) => p.geheimnis);
  if (!Object.keys(zw.personen).every((k) => KENNUNG.test(k)) || !g.every((x) => HEX64.test(x)) || new Set(g).size !== g.length) return { fehler: 'speicher_ungueltig' };
  for (const p of Object.values(zw.personen)) {
    if (!Number.isInteger(p.seit_ms) || typeof p.texte !== 'object' || p.texte === null || Array.isArray(p.texte)) return { fehler: 'speicher_ungueltig' };
    for (const [ft, t] of Object.entries(p.texte)) if (!FT.test(ft) || typeof t.text !== 'string' || !Number.isInteger(t.zuletzt_ms)) return { fehler: 'speicher_ungueltig' };
  }
  return { ok: true, angelegt: ['zuordnung.sperre'] };
}

// --- Ablauf: fuehrt Schritte aus und schreibt die Erwartung in jeden Schritt ---

function ablauf(name, beschreibung, schritte, absichten) {
  const welt = new Welt();
  const aus = [];
  for (const s of schritte) {
    const e = { ...s };
    try {
      switch (s.schritt) {
        case 'oeffnen': welt.oeffnen(s.zufall, s.jetzt_ms); e.erwartet = { ok: true }; break;
        case 'person': e.erwartet = welt.person(s.kennung, s.zufall, s.jetzt_ms); break;
        case 'text': e.erwartet = welt.text(s.kennung, s.text, s.jetzt_ms); break;
        case 'zeile': {
          const obj = { ...s.felder, vorher: welt.letzteHash() };
          const roh = jcs(obj);
          e.erwartet = { zeile: roh, sha256: welt.anhaengen(roh) };
          break;
        }
        case 'auskunft': e.erwartet = welt.auskunft(s.kennung, s.handelnd, s.jetzt_ms); break;
        case 'loeschen': e.erwartet = welt.loeschen(s.kennung, s.handelnd, s.jetzt_ms); break;
        case 'einsicht': e.erwartet = { zeilen: welt.einsicht() }; break;
        case 'zuordnen': e.erwartet = { kennung: welt.zuordnen(s.wert) }; break;
        case 'pruefe': e.erwartet = pruefe(KETTE, welt.kette); break;
        case 'wechseln': welt.wechseln(s.wert, s.jetzt_ms); e.erwartet = { ok: true }; break;
        case 'abgleich': e.erwartet = welt.abgleich(); break;
        case 'speicher': e.erwartet = { schluessel: welt.schluesselText(), zuordnung: welt.zuordnungText() }; break;
        case 'speicher_setzen': welt.zuordnungSetzen(s.zuordnung); e.erwartet = { ok: true }; break;
        default: throw new Error(`unbekannter Schritt ${s.schritt}`);
      }
    } catch (err) {
      if (!(err instanceof Fehler)) throw err;
      e.erwartet = { fehler: err.message };
    }
    aus.push(e);
  }
  absichten(aus, welt);
  return { name, datei: `${name}.json`, inhalt: { beschreibung, kette: KETTE, schritte: aus } };
}

// --- Bausteine der Faelle ---

const T0 = Date.UTC(2026, 10, 20, 13, 0, 0, 0);
const K1 = zufall('schluessel-1');
const K2 = zufall('schluessel-2');
const G = (k) => zufall(`geheimnis-${k}`);
const zt = (ms) => zeitText(ms);

function zeileMcp(person, entscheidend, n, ms) {
  return {
    aktionsname: 'mcp.schreibaufruf', anfrage: `mf-${T0}-${n}`, digest: sha(`argumente-${n}`), entscheidend, ergebnis: 'eingeloest',
    fassung: 2, person, server: 'tickets', sitzung: 's-1', werkzeug: 'update', zeit: zt(ms), zeit_anfrage: zt(ms - 7000), zeit_entscheidung: zt(ms - 3000),
  };
}
function zeileText(person, ft, ms) {
  return { aktionsname: 'fall.freitext', ergebnis: 'ok', fassung: 2, person, text: ft, zeit: zt(ms) };
}
const nimm = (aus, i) => aus[i].erwartet;
const textIn = (wert, nadel) => JSON.stringify(wert).includes(nadel);

// Vorbereitung, die in mehreren Faellen gleich ist: Admin p-adm01, Personen p-a01 und p-b02, je zwei Zeilen und ein Freitext.
const TEXT_A = 'Bitte das Ticket 4711 mit Prioritaet hoch anlegen.';
const TEXT_B = 'Freigabe nach Ruecksprache mit dem Team.';
function grundbestand(k = K1) {
  const psA = pseudonym(k, G('p-a01'));
  const psB = pseudonym(k, G('p-b02'));
  const psAdm = pseudonym(k, G('p-adm01'));
  const ftA = freitext(k, G('p-a01'), TEXT_A);
  const ftB = freitext(k, G('p-b02'), TEXT_B);
  return {
    psA, psB, psAdm, ftA, ftB,
    schritte: [
      { schritt: 'oeffnen', zufall: k, jetzt_ms: T0 },
      { schritt: 'person', kennung: 'p-adm01', zufall: G('p-adm01'), jetzt_ms: T0 },
      { schritt: 'person', kennung: 'p-a01', zufall: G('p-a01'), jetzt_ms: T0 + 1000 },
      { schritt: 'person', kennung: 'p-b02', zufall: G('p-b02'), jetzt_ms: T0 + 2000 },
      { schritt: 'text', kennung: 'p-a01', text: TEXT_A, jetzt_ms: T0 + 3000 },
      { schritt: 'text', kennung: 'p-b02', text: TEXT_B, jetzt_ms: T0 + 4000 },
      { schritt: 'zeile', felder: zeileMcp(psA, psB, 1, T0 + 10000) },
      { schritt: 'zeile', felder: zeileText(psA, ftA, T0 + 11000) },
      { schritt: 'zeile', felder: zeileMcp(psB, psAdm, 2, T0 + 12000) },
      { schritt: 'zeile', felder: zeileText(psB, ftB, T0 + 13000) },
      { schritt: 'zeile', felder: zeileMcp(psB, psA, 3, T0 + 14000) },
    ],
  };
}

const faelle = [];

// 1. Pseudonym und Freitext als Vektoren (reine Bibliothek)
{
  const vek = [];
  for (const [kn, k] of [['K1', K1], ['K2', K2]]) {
    for (const p of ['p-a01', 'p-b02', 'p-adm01']) {
      vek.push({ schluessel: k, geheimnis: G(p), pseudonym: pseudonym(k, G(p)), loeschmarke: loeschmarke(k, G(p)), herkunft: `${kn}/${p}` });
    }
  }
  const nach = (h) => vek.find((v) => v.herkunft === h).pseudonym;
  behaupte(nach('K1/p-a01') === pseudonym(K1, G('p-a01')), 'gleiche Person, gleiches Pseudonym');
  behaupte(new Set(vek.filter((v) => v.herkunft.startsWith('K1/')).map((v) => v.pseudonym)).size === 3, 'verschiedene Personen, verschiedene Pseudonyme');
  for (const p of ['p-a01', 'p-b02', 'p-adm01']) behaupte(nach(`K1/${p}`) !== nach(`K2/${p}`), 'anderer Schluessel, anderes Pseudonym');
  for (const v of vek) behaupte(PS.test(v.pseudonym) && HEX64.test(v.loeschmarke), 'Form');
  const fehlerfaelle = [
    { schluessel: 'ab', geheimnis: G('p-a01'), fehler: 'feld_ungueltig:schluessel' },
    { schluessel: K1, geheimnis: G('p-a01').toUpperCase(), fehler: 'feld_ungueltig:geheimnis' },
  ];
  for (const f of fehlerfaelle) { try { pseudonym(f.schluessel, f.geheimnis); behaupte(false, 'Fehler erwartet'); } catch (e) { behaupte(e.message === f.fehler, f.fehler); } }
  faelle.push({ datei: 'pseudonym.json', inhalt: { beschreibung: 'pseudonym und loeschmarke aus Schluessel und Personengeheimnis: gleiche Person gleiches Pseudonym, verschiedene Personen verschiedene, anderer Schluessel andere; Fehler der Form', vektoren: vek, fehler: fehlerfaelle } });

  const lang = 'ä'.repeat(TEXT_MAX);
  const ftv = [
    { schluessel: K1, geheimnis: G('p-a01'), text: TEXT_A },
    { schluessel: K1, geheimnis: G('p-b02'), text: TEXT_A },
    { schluessel: K2, geheimnis: G('p-a01'), text: TEXT_A },
    { schluessel: K1, geheimnis: G('p-a01'), text: 'x' },
    { schluessel: K1, geheimnis: G('p-a01'), text: lang },
    { schluessel: K1, geheimnis: G('p-a01'), text: '\u{1F600}'.repeat(TEXT_MAX) },
  ].map((v) => ({ ...v, freitext: freitext(v.schluessel, v.geheimnis, v.text) }));
  behaupte(ftv[0].freitext !== ftv[1].freitext, 'gleicher Text, andere Person, anderer Hash');
  behaupte(ftv[0].freitext !== ftv[2].freitext, 'gleicher Text, anderer Schluessel, anderer Hash');
  const ftFehler = [
    { schluessel: K1, geheimnis: G('p-a01'), text: '', fehler: 'text_ungueltig' },
    { schluessel: K1, geheimnis: G('p-a01'), text: `${lang}a`, fehler: 'text_ungueltig' },
    { schluessel: K1, geheimnis: G('p-a01'), text: '\u{1F600}'.repeat(TEXT_MAX + 1), fehler: 'text_ungueltig' },
  ];
  for (const f of ftFehler) { try { freitext(f.schluessel, f.geheimnis, f.text); behaupte(false, 'Fehler erwartet'); } catch (e) { behaupte(e.message === f.fehler, f.fehler); } }
  faelle.push({ datei: 'freitext.json', inhalt: { beschreibung: 'freitext aus Schluessel, Personengeheimnis und Text; Grenze 1 bis 4096 Unicode-Skalarwerte', vektoren: ftv, fehler: ftFehler } });
}

// 2. Zeilen person.geloescht und person.auskunft
{
  const psAdm = pseudonym(K1, G('p-adm01'));
  const lm = loeschmarke(K1, G('p-a01'));
  const gut = [
    { eingabe: { ergebnis: 'geloescht', handelnd: psAdm, marke: lm, zeit_ms: T0 }, vorher: NULL64 },
    { eingabe: { ergebnis: 'geloescht', handelnd: null, marke: lm, zeit_ms: T0 }, vorher: NULL64 },
    { eingabe: { ergebnis: 'unbekannt', handelnd: psAdm, marke: null, zeit_ms: T0 }, vorher: sha('x') },
  ].map((f) => ({ ...f, zeile: zeileGeloescht(f.eingabe, f.vorher) }));
  const schlecht = [
    [{ ergebnis: 'weg', handelnd: psAdm, marke: lm, zeit_ms: T0 }, NULL64],
    [{ ergebnis: 'geloescht', handelnd: 'p-adm01', marke: lm, zeit_ms: T0 }, NULL64],
    [{ ergebnis: 'geloescht', handelnd: psAdm, marke: null, zeit_ms: T0 }, NULL64],
    [{ ergebnis: 'unbekannt', handelnd: psAdm, marke: lm, zeit_ms: T0 }, NULL64],
    [{ ergebnis: 'geloescht', handelnd: psAdm, marke: lm, zeit_ms: T0 }, 'abc'],
    [{ ergebnis: 'geloescht', handelnd: psAdm, marke: lm, zeit_ms: -1 }, NULL64],
  ].map(([eingabe, vorher]) => { try { zeileGeloescht(eingabe, vorher); throw new Error('Fehler erwartet'); } catch (e) { if (!(e instanceof Fehler)) throw e; return { eingabe, vorher, fehler: e.message }; } });
  behaupte(schlecht.map((s) => s.fehler).join() === 'feld_ungueltig:ergebnis,feld_ungueltig:handelnd,feld_ungueltig:loeschmarke,feld_ungueltig:loeschmarke,feld_ungueltig:vorher,zeit_ungueltig', 'Reihenfolge der Feldfehler');
  for (const g of gut) behaupte(!/p-a01|p-adm01/.test(g.zeile), 'keine Kennung in der Zeile');
  const aus = [
    { eingabe: { ergebnis: 'geliefert', handelnd: psAdm, zeilen: 3, zeit_ms: T0 }, vorher: NULL64 },
    { eingabe: { ergebnis: 'unbekannt', handelnd: psAdm, zeilen: 0, zeit_ms: T0 }, vorher: NULL64 },
  ].map((f) => ({ ...f, zeile: zeileAuskunft(f.eingabe, f.vorher) }));
  const ausSchlecht = [
    [{ ergebnis: 'unbekannt', handelnd: psAdm, zeilen: 2, zeit_ms: T0 }, NULL64],
    [{ ergebnis: 'geliefert', handelnd: null, zeilen: 0, zeit_ms: T0 }, NULL64],
  ].map(([eingabe, vorher]) => { try { zeileAuskunft(eingabe, vorher); throw new Error('Fehler erwartet'); } catch (e) { if (!(e instanceof Fehler)) throw e; return { eingabe, vorher, fehler: e.message }; } });
  faelle.push({ datei: 'zeilen.json', inhalt: { beschreibung: 'zeile_geloescht und zeile_auskunft: Bytes oder erster Fehlercode', geloescht: { gut, schlecht }, auskunft: { gut: aus, schlecht: ausSchlecht } } });
}

// 3. Ablauf: Auskunft, Loeschen, Kette gruen, Freitext nicht mehr zuzuordnen
{
  const b = grundbestand();
  const s = [
    ...b.schritte,
    { schritt: 'auskunft', kennung: 'p-a01', handelnd: b.psAdm, jetzt_ms: T0 + 20000 },
    { schritt: 'einsicht' },
    { schritt: 'zuordnen', wert: b.ftA },
    { schritt: 'loeschen', kennung: 'p-a01', handelnd: b.psAdm, jetzt_ms: T0 + 30000 },
    { schritt: 'auskunft', kennung: 'p-a01', handelnd: b.psAdm, jetzt_ms: T0 + 40000 },
    { schritt: 'einsicht' },
    { schritt: 'zuordnen', wert: b.ftA },
    { schritt: 'zuordnen', wert: b.psA },
    { schritt: 'zuordnen', wert: b.ftB },
    { schritt: 'auskunft', kennung: 'p-b02', handelnd: b.psAdm, jetzt_ms: T0 + 50000 },
    { schritt: 'pruefe' },
    { schritt: 'speicher' },
  ];
  faelle.push(ablauf('ablauf-auskunft-und-loeschen', 'Zwei Personen und ein Admin; Auskunft vor und nach dem Loeschen von p-a01; Einsicht; die Kette prueft danach gruen; der Freitext von p-a01 ist nicht mehr zuzuordnen', s, (aus, welt) => {
    const a1 = nimm(aus, 11).antwort;
    behaupte(a1.gefunden && a1.zeilen.map((z) => z.nr).join() === '1,2,5', 'Auskunft findet alle Zeilen von p-a01 (Person, Freitext, Entscheidende)');
    behaupte(a1.texte.length === 1 && a1.texte[0].text === TEXT_A, 'Auskunft liefert ihren Freitext');
    behaupte(!textIn(a1, b.psB) && !textIn(a1, b.ftB) && !textIn(a1, TEXT_B) && !textIn(a1, b.psAdm), 'Auskunft ohne Inhalte anderer');
    behaupte(textIn(nimm(aus, 12), TEXT_A), 'Einsicht zeigt den Freitext vor dem Loeschen');
    behaupte(nimm(aus, 13).kennung === 'p-a01', 'Freitext vorher zuzuordnen');
    behaupte(nimm(aus, 14).ergebnis === 'geloescht', 'geloescht');
    const a2 = nimm(aus, 15).antwort;
    behaupte(!a2.gefunden && a2.zeilen.length === 0 && a2.texte.length === 0, 'Auskunft findet nach dem Loeschen nichts');
    const ein = nimm(aus, 16);
    behaupte(!textIn(ein, TEXT_A) && !textIn(ein, 'p-a01'), 'Einsicht findet keinen Freitext und keine Kennung der Person');
    behaupte(textIn(ein, TEXT_B) && textIn(ein, 'p-b02'), 'Einsicht zeigt die andere Person weiter');
    behaupte(nimm(aus, 17).kennung === null && nimm(aus, 18).kennung === null, 'Freitext-Hash und Pseudonym nicht mehr zuzuordnen');
    // auch wer den Text kennt und alle Schluessel hat, findet mit keinem verbliebenen Geheimnis den Hash wieder
    for (const p of welt.personen.values()) for (const k of welt.schluessel) behaupte(freitext(k.wert, p.geheimnis, TEXT_A) !== b.ftA, 'Rateversuch mit Text');
    behaupte(nimm(aus, 19).kennung === 'p-b02', 'andere Person bleibt zuzuordnen');
    behaupte(nimm(aus, 20).antwort.zeilen.length === 4, 'Auskunft p-b02: Zeilen 1, 3, 4, 5');
    const pr = nimm(aus, 21);
    behaupte(pr.exit === 0 && pr.stdout.includes('"zeilen":9'), 'Kette gruen mit allen Zeilen, auch Auskunft und Loeschen');
    behaupte(!welt.kette.includes('p-a01') && !welt.kette.includes(TEXT_A) && !welt.kette.includes(G('p-a01')), 'keine Kennung, kein Text, kein Geheimnis in der Kette');
    behaupte(!nimm(aus, 22).zuordnung.includes('p-a01') && !nimm(aus, 22).zuordnung.includes(TEXT_A), 'Speicher ohne die Person');
  }));
}

// 4. Loeschen einer unbekannten Person und zweimal Loeschen
{
  const b = grundbestand();
  const s = [
    ...b.schritte,
    { schritt: 'speicher' },
    { schritt: 'loeschen', kennung: 'p-x99', handelnd: b.psAdm, jetzt_ms: T0 + 30000 },
    { schritt: 'speicher' },
    { schritt: 'loeschen', kennung: 'P-A01', handelnd: b.psAdm, jetzt_ms: T0 + 31000 },
    { schritt: 'pruefe' },
  ];
  faelle.push(ablauf('loeschen-unbekannt', 'Loeschen einer Kennung, die der Speicher nicht kennt: Zeile mit ergebnis unbekannt und loeschmarke null, Speicher unveraendert; eine Kennung ausser Form ist ein Aufruffehler ohne Zeile', s, (aus) => {
    behaupte(nimm(aus, 12).ergebnis === 'unbekannt' && nimm(aus, 12).zeile.includes('"loeschmarke":null'), 'unbekannt');
    behaupte(nimm(aus, 11).zuordnung === nimm(aus, 13).zuordnung, 'Speicher unveraendert');
    behaupte(nimm(aus, 14).fehler === 'feld_ungueltig:kennung', 'Form der Kennung');
    behaupte(nimm(aus, 15).exit === 0 && nimm(aus, 15).stdout.includes('"zeilen":6'), 'Kette gruen, keine Zeile fuer den Aufruffehler');
  }));

  const b2 = grundbestand();
  const s2 = [
    ...b2.schritte,
    { schritt: 'loeschen', kennung: 'p-a01', handelnd: b2.psAdm, jetzt_ms: T0 + 30000 },
    { schritt: 'loeschen', kennung: 'p-a01', handelnd: b2.psAdm, jetzt_ms: T0 + 31000 },
    { schritt: 'loeschen', kennung: 'p-x99', handelnd: b2.psAdm, jetzt_ms: T0 + 32000 },
    { schritt: 'person', kennung: 'p-a01', zufall: zufall('geheimnis-p-a01-neu'), jetzt_ms: T0 + 40000 },
    { schritt: 'auskunft', kennung: 'p-a01', handelnd: b2.psAdm, jetzt_ms: T0 + 41000 },
    { schritt: 'pruefe' },
  ];
  faelle.push(ablauf('loeschen-zweimal', 'Zweimal Loeschen derselben Person: der zweite Aufruf gleicht dem Loeschen einer unbekannten Person bis auf die Zeit; eine spaeter neu angelegte Person mit derselben Kennung bekommt ein neues Pseudonym und erbt keine alte Zeile', s2, (aus) => {
    const z2 = JSON.parse(nimm(aus, 12).zeile);
    const z3 = JSON.parse(nimm(aus, 13).zeile);
    behaupte(nimm(aus, 11).ergebnis === 'geloescht' && nimm(aus, 12).ergebnis === 'unbekannt', 'erst geloescht, dann unbekannt');
    behaupte(z2.loeschmarke === null && z2.handelnd === z3.handelnd && z2.ergebnis === z3.ergebnis, 'zweimal gleicht unbekannt');
    behaupte(nimm(aus, 14).neu && nimm(aus, 14).pseudonym !== b2.psA, 'neue Person, neues Pseudonym');
    behaupte(nimm(aus, 15).antwort.zeilen.length === 0, 'neue Person erbt keine Zeile');
    behaupte(nimm(aus, 16).exit === 0, 'Kette gruen');
  }));
}

// 5. Selbstloeschung: die Zeile nennt keinen Handelnden
{
  const b = grundbestand();
  const s = [...b.schritte, { schritt: 'loeschen', kennung: 'p-a01', handelnd: b.psA, jetzt_ms: T0 + 30000 }, { schritt: 'pruefe' }];
  faelle.push(ablauf('loeschen-selbst', 'Die Person loescht sich selbst: handelnd ist null, damit die Zeile ihr Pseudonym nicht mit der Loeschung verbindet', s, (aus) => {
    behaupte(JSON.parse(nimm(aus, 11).zeile).handelnd === null && !nimm(aus, 11).zeile.includes(b.psA), 'kein Pseudonym der geloeschten Person');
    behaupte(nimm(aus, 12).exit === 0, 'Kette gruen');
  }));
}

// 6. Schluesselwechsel: andere Pseudonyme, Auskunft findet beide, Loeschen wirkt auf beide
{
  const b = grundbestand();
  const psA2 = pseudonym(K2, G('p-a01'));
  const psB2 = pseudonym(K2, G('p-b02'));
  const s = [
    ...b.schritte,
    { schritt: 'wechseln', wert: K1, jetzt_ms: T0 + 20000 },
    { schritt: 'wechseln', wert: K2, jetzt_ms: T0 },
    { schritt: 'wechseln', wert: K2, jetzt_ms: T0 + 20000 },
    { schritt: 'person', kennung: 'p-a01', zufall: zufall('unbenutzt'), jetzt_ms: T0 + 21000 },
    { schritt: 'text', kennung: 'p-a01', text: TEXT_A, jetzt_ms: T0 + 22000 },
    { schritt: 'zeile', felder: zeileMcp(psA2, psB2, 4, T0 + 23000) },
    { schritt: 'zeile', felder: zeileText(psA2, freitext(K2, G('p-a01'), TEXT_A), T0 + 24000) },
    { schritt: 'auskunft', kennung: 'p-a01', handelnd: pseudonym(K2, G('p-adm01')), jetzt_ms: T0 + 25000 },
    { schritt: 'speicher' },
    { schritt: 'loeschen', kennung: 'p-a01', handelnd: pseudonym(K2, G('p-adm01')), jetzt_ms: T0 + 30000 },
    { schritt: 'zuordnen', wert: b.psA },
    { schritt: 'zuordnen', wert: psA2 },
    { schritt: 'auskunft', kennung: 'p-a01', handelnd: pseudonym(K2, G('p-adm01')), jetzt_ms: T0 + 31000 },
    { schritt: 'pruefe' },
  ];
  faelle.push(ablauf('schluessel-wechsel', 'Wechsel des Schluessels: neue Pseudonyme fuer dieselben Personen; die Auskunft findet Zeilen unter beiden Schluesseln; ein alter Schluessel und eine Zeit, die nicht steigt, werden abgelehnt; das Loeschen trennt beide Pseudonyme von der Person', s, (aus) => {
    behaupte(nimm(aus, 11).fehler === 'schluessel_ungueltig' && nimm(aus, 12).fehler === 'zeit_ungueltig', 'Wechsel abgelehnt');
    behaupte(nimm(aus, 14).pseudonym === psA2 && psA2 !== b.psA, 'anderer Schluessel, anderes Pseudonym');
    const a = nimm(aus, 18).antwort;
    behaupte(a.pseudonyme.join() === [b.psA, psA2].join() && a.zeilen.map((z) => z.nr).join() === '1,2,5,6,7', 'Auskunft ueber beide Schluessel');
    behaupte(a.texte.length === 2, 'beide Hashes desselben Texts');
    behaupte(nimm(aus, 21).kennung === null && nimm(aus, 22).kennung === null, 'beide Pseudonyme nicht mehr zuzuordnen');
    behaupte(!nimm(aus, 23).antwort.gefunden, 'Auskunft leer');
    behaupte(nimm(aus, 24).exit === 0, 'Kette gruen');
  }));
}

// 7. Wiederherstellung eines alten Speichers: der Abgleich mit den Loeschmarken der Kette loescht erneut
{
  const b = grundbestand();
  const vorher = new Welt();
  for (const st of b.schritte) {
    if (st.schritt === 'oeffnen') vorher.oeffnen(st.zufall, st.jetzt_ms);
    if (st.schritt === 'person') vorher.person(st.kennung, st.zufall, st.jetzt_ms);
    if (st.schritt === 'text') vorher.text(st.kennung, st.text, st.jetzt_ms);
  }
  const sicherung = vorher.zuordnungText();
  const s = [
    ...b.schritte,
    { schritt: 'loeschen', kennung: 'p-a01', handelnd: b.psAdm, jetzt_ms: T0 + 30000 },
    { schritt: 'speicher_setzen', zuordnung: sicherung },
    { schritt: 'zuordnen', wert: b.psA },
    { schritt: 'abgleich' },
    { schritt: 'zuordnen', wert: b.psA },
    { schritt: 'zuordnen', wert: b.ftA },
    { schritt: 'abgleich' },
    { schritt: 'speicher' },
    { schritt: 'pruefe' },
  ];
  faelle.push(ablauf('abgleich-nach-wiederherstellung', 'Ein Speicher aus einer Sicherung von vor dem Loeschen kommt zurueck; vor der ersten Zeile gleicht die Stelle ihn mit den Loeschmarken der Kette ab und entfernt die Person erneut; ein zweiter Abgleich entfernt nichts', s, (aus) => {
    behaupte(nimm(aus, 13).kennung === 'p-a01', 'die alte Sicherung ordnet wieder zu (Grenze)');
    behaupte(nimm(aus, 14).entfernt === 1 && nimm(aus, 17).entfernt === 0, 'Abgleich entfernt genau einmal');
    behaupte(nimm(aus, 15).kennung === null && nimm(aus, 16).kennung === null, 'danach nicht mehr zuzuordnen');
    behaupte(!nimm(aus, 18).zuordnung.includes('p-a01'), 'Speicher ohne die Person');
    behaupte(nimm(aus, 19).exit === 0, 'Kette gruen');
  }));
}

// 8. Freitext und Person: Fehlerwege des Speichers
{
  const s = [
    { schritt: 'oeffnen', zufall: K1, jetzt_ms: T0 },
    { schritt: 'text', kennung: 'p-a01', text: TEXT_A, jetzt_ms: T0 },
    { schritt: 'person', kennung: 'p-a01', zufall: G('p-a01'), jetzt_ms: T0 },
    { schritt: 'person', kennung: 'p-a01', zufall: zufall('anderes'), jetzt_ms: T0 + 1 },
    { schritt: 'person', kennung: 'p-b02', zufall: G('p-a01'), jetzt_ms: T0 + 2 },
    { schritt: 'person', kennung: 'ps_00000000000000000000000000000001', zufall: G('p-x'), jetzt_ms: T0 + 3 },
    { schritt: 'text', kennung: 'p-a01', text: '', jetzt_ms: T0 + 4 },
    { schritt: 'text', kennung: 'p-a01', text: TEXT_A, jetzt_ms: T0 + 5 },
    { schritt: 'text', kennung: 'p-a01', text: TEXT_A, jetzt_ms: T0 + 6 },
    { schritt: 'speicher' },
  ];
  faelle.push(ablauf('speicher-person-und-text', 'Ein Freitext nur fuer eine bekannte Person; eine bekannte Person behaelt ihr Geheimnis; ein Geheimnis gilt nur fuer eine Person; eine Kennung in Pseudonymform ist keine Kennung; derselbe Text steht einmal mit der Zeit seiner letzten Ablage', s, (aus) => {
    behaupte(nimm(aus, 1).fehler === 'person_unbekannt', 'Text ohne Person');
    behaupte(nimm(aus, 3).neu === false && nimm(aus, 3).pseudonym === nimm(aus, 2).pseudonym, 'Person behaelt Geheimnis');
    behaupte(nimm(aus, 4).fehler === 'geheimnis_doppelt', 'Geheimnis doppelt');
    behaupte(nimm(aus, 5).fehler === 'feld_ungueltig:kennung', 'ps_ ist keine Kennung');
    behaupte(nimm(aus, 6).fehler === 'text_ungueltig', 'leerer Text');
    behaupte(nimm(aus, 7).freitext === nimm(aus, 8).freitext, 'gleicher Text, gleicher Hash');
    behaupte(nimm(aus, 9).zuordnung.includes(`"zuletzt_ms":${T0 + 6}`) && !nimm(aus, 9).zuordnung.includes(`"zuletzt_ms":${T0 + 5}`), 'letzte Zeit gilt');
  }));
}

// 9. Oeffnen des Speichers: Rechte, Form, fehlende Teile (Faelle fuer die Huelle mit Dateisystem)
{
  const z = new Welt();
  z.oeffnen(K1, T0);
  z.person('p-a01', G('p-a01'), T0);
  const zuo = z.zuordnungText();
  const sch = z.schluesselText();
  const d = (o) => ({ ordner_modus: '0700', ...o });
  const oeffnen = [
    { name: 'neu', vorher: d({ dateien: {} }), erwartet: { ok: true, angelegt: ['schluessel.json', 'zuordnung.json', 'zuordnung.sperre'] } },
    { name: 'gut', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: zuo, modus: '0600' } } }), erwartet: { ok: true, angelegt: ['zuordnung.sperre'] } },
    { name: 'schluessel-fehlt', vorher: d({ dateien: { 'zuordnung.json': { inhalt: zuo, modus: '0600' } } }), erwartet: { fehler: 'schluessel_fehlt' } },
    { name: 'zuordnung-fehlt', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' } } }), erwartet: { fehler: 'zuordnung_fehlt' } },
    { name: 'datei-fuer-andere-lesbar', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: zuo, modus: '0640' } } }), erwartet: { fehler: 'speicher_unsicher' } },
    { name: 'ordner-fuer-andere-offen', vorher: d({ ordner_modus: '0750', dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: zuo, modus: '0600' } } }), erwartet: { fehler: 'speicher_unsicher' } },
    { name: 'verweis', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { verweis_auf: 'anderswo.json', modus: '0600' } } }), erwartet: { fehler: 'speicher_unsicher' } },
    { name: 'nicht-kanonisch', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: zuo.replace('{"fassung":1,', '{"fassung": 1,'), modus: '0600' } } }), erwartet: { fehler: 'speicher_ungueltig' } },
    { name: 'geheimnis-doppelt', vorher: d({ dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: `${jcs({ fassung: 1, personen: { 'p-a01': { geheimnis: G('p-a01'), seit_ms: T0, texte: {} }, 'p-b02': { geheimnis: G('p-a01'), seit_ms: T0, texte: {} } } })}\n`, modus: '0600' } } }), erwartet: { fehler: 'speicher_ungueltig' } },
    { name: 'im-git-arbeitsbaum', vorher: d({ git_darueber: { ebene: 2, art: 'ordner' }, dateien: {} }), erwartet: { fehler: 'speicher_im_repo' } },
    { name: 'im-git-arbeitsbaum-gitdatei', vorher: d({ git_darueber: { ebene: 0, art: 'datei' }, dateien: { 'schluessel.json': { inhalt: sch, modus: '0600' }, 'zuordnung.json': { inhalt: zuo, modus: '0600' } } }), erwartet: { fehler: 'speicher_im_repo' } },
    { name: 'schluessel-zeit-faellt', vorher: d({ dateien: { 'schluessel.json': { inhalt: `${jcs({ fassung: 1, schluessel: [{ ab_ms: T0, wert: K1 }, { ab_ms: T0, wert: K2 }] })}\n`, modus: '0600' }, 'zuordnung.json': { inhalt: zuo, modus: '0600' } } }), erwartet: { fehler: 'speicher_ungueltig' } },
  ];
  for (const f of oeffnen) behaupte(JSON.stringify(oeffnenModell(f.vorher)) === JSON.stringify(f.erwartet), `oeffnen ${f.name}`);
  faelle.push({ datei: 'speicher-oeffnen.json', inhalt: { beschreibung: 'Speicher::oeffnen auf einem Ordner mit Anfangsinhalt; git_darueber nennt einen Eintrag .git im Ordner (ebene 0) oder ebene Ordner darueber, art ordner, datei (Arbeitsbaum eines Worktrees) oder verweis; bei neu legt der Aufruf beide Dateien mit dem uebergebenen Zufall als Schluessel an (zufall, ab_ms)', zufall: K1, ab_ms: T0, faelle: oeffnen } });
}

// --- Schreiben ---
rmSync(AUS, { recursive: true, force: true });
mkdirSync(AUS, { recursive: true });
for (const f of faelle) writeFileSync(join(AUS, f.datei), `${JSON.stringify(f.inhalt, null, 1)}\n`);
const namen = readdirSync(AUS).sort();
console.log(`${namen.length} Dateien in faelle/: ${namen.join(' ')}`);

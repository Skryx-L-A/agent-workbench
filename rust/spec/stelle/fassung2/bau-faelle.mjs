// Erzeugt die sichtbaren Faelle von Vertrag 6, Fassung 2 (README.md in diesem Ordner) nach fassung2/faelle/.
// Das Referenzmodell baut auf dem Modell von Fassung 1 auf (../bau-faelle.mjs): Laden, Start, Form,
// Fristen und die vier Zeilenarten sind von dort uebernommen und um die Teile von Fassung 2 erweitert.
// Zwei Pruefungen laufen bei jedem Aufruf:
//   1. Gleichlauf: Jeder Fall von Fassung 1 (../faelle/*.json) laeuft durch das Modell von Fassung 2. Antworten, Zeilenbytes und
//      Endzustand muessen gleich sein; einzige erlaubte Abweichung ist das neue Feld entscheidend in den Antworten von
//      mcp.stand und mcp.einloesen (Nachtrag 1, README Abschnitt 4.6).
//   2. Absicht: Jeder Fall von Fassung 2 traegt die Absicht von Hand (Antwort, Zeilenarten, Endzustand). Weicht das Modell ab, bricht das Skript ab.
// Aufruf: node rust/spec/stelle/fassung2/bau-faelle.mjs (leicht, ohne Abhaengigkeit, ohne Netz). Gleicher Stand ergibt gleiche Bytes.
// Pruefschalter: node rust/spec/stelle/fassung2/bau-faelle.mjs --pruefen erzeugt im Speicher neu, schreibt nichts und vergleicht
// Byte fuer Byte mit faelle/ (gleiche Namen, gleiche Bytes). Exit 0 nur bei Gleichheit, sonst Exit 1 mit den abweichenden Namen.

import { createHash, createHmac } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs } from '../../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const AUS = join(HIER, 'faelle');
const F1_FAELLE = join(HIER, '..', 'faelle');
const sha = (b) => createHash('sha256').update(typeof b === 'string' ? Buffer.from(b, 'utf8') : b).digest('hex');
const NULL64 = '0'.repeat(64);
const SCHLUESSEL = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const WERKZEUG = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const PSEUDONYM = /^ps_[0-9a-f]{32}$/;
const ANFRAGE = /^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$/;
const ANTRAG = /^an-[0-9]{1,15}-[1-9][0-9]{0,9}$/;
const ZUGANG = /^hz-[0-9]{1,15}-[1-9][0-9]{0,9}$/;
const NONCE = /^[0-9a-f]{32}$/;
const MONAT = /^[0-9]{4}-(0[1-9]|1[0-2])$/;
const ZEIT = /^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$/;
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const T0 = Date.UTC(2026, 10, 20, 13, 0, 0);
const LADEN = T0 - 60000;
const LADEN2 = T0 + 2500;
const mf = (n, laden = LADEN) => `mf-${laden}-${n}`;
const an = (n, laden = LADEN) => `an-${laden}-${n}`;
const hz = (n, laden = LADEN) => `hz-${laden}-${n}`;
const ARTEN = ['kern', 'hub', 'brain', 'brain_mensch', 'mensch', 'hersteller', 'agent', 'takt', 'unbekannt'];
const ERLAUBT = {
  'sitzung.starten': ['kern'], 'mcp.anfragen': ['hub', 'brain', 'brain_mensch'], 'mcp.entscheiden': ['mensch'], 'mcp.einloesen': ['hub', 'brain', 'brain_mensch'],
  'mcp.offene': ['mensch'], 'mcp.stand': ['hub', 'brain', 'brain_mensch'], 'ablauf.pruefen': ['takt'],
  'einsicht.anfragen': ['mensch'], 'pflicht.aendern': ['mensch'], 'antrag.bestaetigen': ['mensch'], 'antrag.offene': ['mensch'], 'antrag.ergebnis': ['mensch'],
  'hersteller.gewaehren': ['mensch'], 'hersteller.beenden': ['mensch'], 'hersteller.zugriff': ['hersteller'],
};
const VORGAENGE = Object.keys(ERLAUBT);
const QUELLEN = ['gateway', 'stelle', 'brain', 'halter'];
const ROLLEN = ['admin', 'einsicht'];
const FAELLE_HERSTELLER = ['kein', 'befristet', 'dauerhaft'];
// B08: wer ein Protokoll einsehen darf (Block stelle, einsicht.regel).
const EINSICHT_REGELN = ['niemand', 'rolle', 'rolle-vier-augen'];
const SERVER_BRAIN = 'brain';
const WERKZEUG_SENKEN = 'brain_senken';

const GRUENDE = {
  'stelle.aufrufer_nicht_erlaubt': 'Aufrufer nicht erlaubt',
  'stelle.eingabe_ungueltig': 'Eingabe ungueltig',
  'stelle.sitzung_vergeben': 'Sitzung schon vergeben',
  'stelle.server_unbekannt': 'Server unbekannt',
  'stelle.sitzung_unbekannt': 'Sitzung unbekannt',
  'stelle.frist_ungueltig': 'Frist ausserhalb der Vorgabe',
  'stelle.anfrage_unbekannt': 'Anfrage unbekannt',
  'stelle.anfrage_abgelaufen': 'Anfrage abgelaufen',
  'stelle.bereits_entschieden': 'Anfrage schon entschieden',
  'stelle.nicht_freigegeben': 'Anfrage nicht freigegeben',
  'stelle.anfrage_abgelehnt': 'Anfrage abgelehnt',
  'stelle.bereits_eingeloest': 'Freigabe schon eingeloest',
  'stelle.digest_abweichend': 'Aufruf weicht von der Anfrage ab',
  'stelle.nachweis_ungueltig': 'Nachweis ungueltig',
  'stelle.zeit_ungueltig': 'Zeit ungueltig',
  // neu in Fassung 2
  'stelle.server_nicht_erlaubt': 'Server fuer diesen Aufrufer nicht erlaubt',
  'stelle.zu_viele_offen': 'Zu viele offene Anfragen',
  'stelle.selbe_person': 'Dieselbe Person darf hier nicht entscheiden',
  'stelle.zweite_person_fehlt': 'Zweite Person fehlt',
  'stelle.rolle_fehlt': 'Rolle fehlt',
  'stelle.zweck_ungueltig': 'Zweck fehlt oder unbekannt',
  'stelle.nonce_benutzt': 'Nachweis schon benutzt',
  'stelle.pflicht_unbekannt': 'Pflicht unbekannt',
  'stelle.pflicht_unveraendert': 'Wert schon gesetzt',
  'stelle.antrag_laeuft': 'Antrag laeuft schon',
  'stelle.antrag_unbekannt': 'Antrag unbekannt',
  'stelle.antrag_abgelaufen': 'Antrag abgelaufen',
  'stelle.antrag_nicht_bestaetigt': 'Antrag nicht bestaetigt',
  'stelle.nicht_antragsteller': 'Nur der Antragsteller holt ab',
  'stelle.bereits_abgeholt': 'Ergebnis schon abgeholt',
  'stelle.hersteller_aus': 'Herstellerzugang aus',
  'stelle.zugang_aktiv': 'Zugang schon aktiv',
  'stelle.zugang_unbekannt': 'Zugang unbekannt',
  'stelle.zugang_abgelaufen': 'Zugang abgelaufen',
  'stelle.zugang_beendet': 'Zugang beendet',
  'start.zu_gross': 'Start zu gross',
  'start.json_ungueltig': 'kein gueltiges JSON',
  'start.schluessel_unbekannt': 'unbekannter Schluessel',
  'start.schluessel_fehlt': 'Pflichtfeld fehlt',
  'start.typ_falsch': 'falscher Typ',
  'start.fassung_unbekannt': 'Fassung unbekannt',
  'start.kennung_form': 'Kennung hat falsche Form',
  'start.modell_form': 'Modell hat falsche Form',
  'start.ordner_ungueltig': 'Arbeitsordner ungueltig',
  'start.harness_unbekannt': 'Harness unbekannt',
  'start.route_unbekannt': 'Route unbekannt',
  'start.kunde_unbekannt': 'Kunde unbekannt',
  'start.harness_route_unvertraeglich': 'Harness passt nicht zur Route',
  'einst.route_ohne_klasse': 'Route ohne Klasse',
  'einst.schluessel_unbekannt': 'unbekannter Schluessel',
  'einst.schluessel_fehlt': 'Pflichtfeld fehlt',
  'einst.typ_falsch': 'falscher Typ',
  'einst.anzahl': 'Anzahl ausserhalb der Grenzen',
  'einst.kennung_form': 'Kennung hat falsche Form',
  'einst.kennung_doppelt': 'Kennung doppelt',
  'einst.frist_bereich': 'Frist ausserhalb des Bereichs',
  'einst.mcp_doppelt': 'Werkzeug in lesen und schreiben',
  'einst.stelle_wert': 'Wert ausserhalb der Vorgabe',
  'einst.json_ungueltig': 'kein gueltiges JSON',
  'stelle.regel_kette_weicht_ab': 'Regeldatei weicht von der Kette ab',
  'stelle.einsicht_aus': 'Einsicht aus',
  'stelle.rollen_ungueltig': 'Rollen der Firmendatei ungueltig',
};

const fehlerAntwort = (code) => ({ ok: false, code, grund: GRUENDE[code] });
const sicher = (v) => (typeof v === 'string' && SCHLUESSEL.test(v) ? v : null);
const istObjekt = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const bytesLaenge = (s) => Buffer.byteLength(s, 'utf8');
const ganzIn = (v, a, b) => Number.isInteger(v) && v >= a && v <= b;

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw new Error('zeit_ungueltig');
  return new Date(ms).toISOString();
}

// ---------- Laden: Block mcp (Fassung 1 plus zweite_person) und Block stelle (neu), Stufen in fester Reihenfolge ueber die ganze Datei ----------

const STELLE_VORGABE = {
  vier_augen: false, senken_zweite_person: false, halter_bescheinigung: true, antrag_frist_ms: 600000,
  einsicht: { regel: 'rolle', zwecke: ['pruefung', 'vorfall', 'auskunft'] },
  hersteller: { fall: 'kein', hoechstfrist_ms: 28800000, rollen: ['admin'] },
  flut: { offene_je_sitzung: 16, zeilen_je_fenster: 32, fenster_ms: 60000 },
};
const STELLE_FELDER = { '': ['vier_augen', 'senken_zweite_person', 'halter_bescheinigung', 'antrag_frist_ms', 'einsicht', 'hersteller', 'flut'],
  einsicht: ['regel', 'zwecke'], hersteller: ['fall', 'hoechstfrist_ms', 'rollen'], flut: ['offene_je_sitzung', 'zeilen_je_fenster', 'fenster_ms'] };

function laden2(d) {
  const hatMcp = d.mcp !== undefined;
  const hatStelle = d.stelle !== undefined;
  if ((hatMcp || hatStelle) && d.fassung !== 2) return { fehler: 'einst.schluessel_unbekannt' };
  if (hatMcp && !istObjekt(d.mcp)) return { fehler: 'einst.typ_falsch' };
  if (hatStelle && !istObjekt(d.stelle)) return { fehler: 'einst.typ_falsch' };
  const srv = hatMcp ? Object.entries(d.mcp) : [];
  const objs = srv.filter(([, x]) => istObjekt(x)).map(([, x]) => x);
  const liste = (l) => Array.isArray(l) && l.every((w) => typeof w === 'string');
  const ganz = (v) => v === undefined || Number.isInteger(v);
  const s = hatStelle ? d.stelle : {};
  const unter = (k) => s[k];
  const unterObj = (k) => s[k] === undefined || istObjekt(s[k]);
  const sub = (k) => (istObjekt(s[k]) ? s[k] : {});
  const zwecke = sub('einsicht').zwecke;
  const regel = sub('einsicht').regel;
  const hRollen = sub('hersteller').rollen;
  const stufen = [
    ['einst.schluessel_unbekannt', () => objs.some((x) => Object.keys(x).some((k) => !['lesen', 'schreiben', 'freigabe', 'frist_ms', 'einloesefrist_ms', 'zweite_person'].includes(k)))
      || Object.keys(s).some((k) => !STELLE_FELDER[''].includes(k))
      || ['einsicht', 'hersteller', 'flut'].some((b) => istObjekt(s[b]) && Object.keys(s[b]).some((k) => !STELLE_FELDER[b].includes(k)))],
    ['einst.schluessel_fehlt', () => objs.some((x) => !('lesen' in x) || !('schreiben' in x))],
    ['einst.typ_falsch', () => srv.some(([, x]) => !istObjekt(x)) || objs.some((x) => !liste(x.lesen) || !liste(x.schreiben) || (x.zweite_person !== undefined && !liste(x.zweite_person))
        || (x.freigabe !== undefined && typeof x.freigabe !== 'boolean') || !ganz(x.frist_ms) || !ganz(x.einloesefrist_ms))
      || ['vier_augen', 'senken_zweite_person', 'halter_bescheinigung'].some((k) => unter(k) !== undefined && typeof unter(k) !== 'boolean')
      || !ganz(s.antrag_frist_ms) || !unterObj('einsicht') || !unterObj('hersteller') || !unterObj('flut')
      || (regel !== undefined && typeof regel !== 'string') || (zwecke !== undefined && !liste(zwecke)) || (hRollen !== undefined && !liste(hRollen))
      || (sub('hersteller').fall !== undefined && typeof sub('hersteller').fall !== 'string')
      || !ganz(sub('hersteller').hoechstfrist_ms) || ['offene_je_sitzung', 'zeilen_je_fenster', 'fenster_ms'].some((k) => !ganz(sub('flut')[k]))],
    ['einst.anzahl', () => srv.length > 64 || objs.some((x) => x.lesen.length > 256 || x.schreiben.length > 256 || (x.zweite_person ?? []).length > 256)
      || (zwecke !== undefined && (zwecke.length < 1 || zwecke.length > 32))
      || (hRollen !== undefined && (hRollen.length < 1 || hRollen.length > 2))],
    ['einst.kennung_form', () => srv.some(([k]) => !SCHLUESSEL.test(k)) || objs.some((x) => [...x.lesen, ...x.schreiben, ...(x.zweite_person ?? [])].some((w) => !WERKZEUG.test(w)))
      || (zwecke ?? []).some((z) => !SCHLUESSEL.test(z))],
    ['einst.kennung_doppelt', () => objs.some((x) => [x.lesen, x.schreiben, x.zweite_person ?? []].some((l) => new Set(l).size !== l.length))
      || (zwecke !== undefined && new Set(zwecke).size !== zwecke.length)
      || (hRollen !== undefined && new Set(hRollen).size !== hRollen.length)],
    ['einst.frist_bereich', () => objs.some((x) => (x.frist_ms !== undefined && (x.frist_ms < 1 || x.frist_ms > 86400000)) || (x.einloesefrist_ms !== undefined && (x.einloesefrist_ms < 1 || x.einloesefrist_ms > 86400000)))
      || (s.antrag_frist_ms !== undefined && !ganzIn(s.antrag_frist_ms, 1, 86400000))
      || (sub('hersteller').hoechstfrist_ms !== undefined && !ganzIn(sub('hersteller').hoechstfrist_ms, 1, 2592000000))
      || (sub('flut').fenster_ms !== undefined && !ganzIn(sub('flut').fenster_ms, 1000, 86400000))],
    ['einst.mcp_doppelt', () => objs.some((x) => x.lesen.some((w) => x.schreiben.includes(w) || (x.zweite_person ?? []).includes(w)))],
    ['einst.stelle_wert', () => (hRollen ?? []).some((x) => !ROLLEN.includes(x)) || (regel !== undefined && !EINSICHT_REGELN.includes(regel))
      || (sub('hersteller').fall !== undefined && !FAELLE_HERSTELLER.includes(sub('hersteller').fall))
      || (sub('flut').offene_je_sitzung !== undefined && !ganzIn(sub('flut').offene_je_sitzung, 1, 1024))
      || (sub('flut').zeilen_je_fenster !== undefined && !ganzIn(sub('flut').zeilen_je_fenster, 1, 10000))],
  ];
  for (const [code, trifft] of stufen) if (trifft()) return { fehler: code };
  const mcp = {};
  for (const [name, x] of srv) mcp[name] = { lesen: x.lesen, freigabe: x.freigabe ?? true, frist_ms: x.frist_ms ?? 60000, einloesefrist_ms: x.einloesefrist_ms ?? 300000, zweite_person: x.zweite_person ?? [] };
  const st = {
    vier_augen: s.vier_augen ?? STELLE_VORGABE.vier_augen, senken_zweite_person: s.senken_zweite_person ?? STELLE_VORGABE.senken_zweite_person,
    halter_bescheinigung: s.halter_bescheinigung ?? STELLE_VORGABE.halter_bescheinigung,
    antrag_frist_ms: s.antrag_frist_ms ?? STELLE_VORGABE.antrag_frist_ms,
    einsicht_regel: regel ?? STELLE_VORGABE.einsicht.regel, zwecke: zwecke ?? STELLE_VORGABE.einsicht.zwecke,
    hersteller: { fall: sub('hersteller').fall ?? 'kein', hoechstfrist_ms: sub('hersteller').hoechstfrist_ms ?? 28800000, rollen: hRollen ?? ['admin'] },
    flut: { offene_je_sitzung: sub('flut').offene_je_sitzung ?? 16, zeilen_je_fenster: sub('flut').zeilen_je_fenster ?? 32, fenster_ms: sub('flut').fenster_ms ?? 60000 },
  };
  return { mcp, st };
}

// B09: Form der Rollen aus der Firmendatei: Pseudonym -> 1 bis 2 aus admin und einsicht, ohne Doppel, hoechstens 1024 Eintraege.
const rollenGueltig = (x) => istObjekt(x) && Object.keys(x).length <= 1024
  && Object.entries(x).every(([p, l]) => PSEUDONYM.test(p) && Array.isArray(l) && l.length >= 1 && l.length <= 2 && l.every((y) => ROLLEN.includes(y)) && new Set(l).size === l.length);
function modellLaden(text, ladenMs, regelNeu = null, rollen = {}) {
  let d;
  try { d = JSON.parse(text); } catch { return { fehler: 'einst.json_ungueltig' }; }
  if (!istObjekt(d)) return { fehler: 'einst.json_ungueltig' };
  for (const r of d.routen ?? []) if (!istObjekt(r) || typeof r.klasse !== 'string') return { fehler: 'einst.route_ohne_klasse' };
  const routen = {};
  for (const r of d.routen) routen[r.schluessel] = r.art;
  const kunden = (d.kunden ?? []).map((k) => k.schluessel);
  const m = laden2(d);
  if (m.fehler) return { fehler: m.fehler };
  // B09: Die Rollen kommen aus der Firmendatei (Regelebenen); die Huelle uebergibt sie, die Stelle prueft ihre Form noch einmal.
  if (!rollenGueltig(rollen)) return { fehler: 'stelle.rollen_ungueltig' };
  m.st.rollen = rollen;
  // Nachtrag 5: Die Huelle uebergibt neu der letzten Zeile regel.geaendert dieser Ebene; weicht die gelesene Datei ab, startet die Stelle nicht.
  if (regelNeu !== null && sha(jcs(d)) !== regelNeu) return { fehler: 'stelle.regel_kette_weicht_ab' };
  // Pflichten zur Laufzeit (Abschnitt 5): Startwert aus der Datei, Aenderung nur ueber pflicht.aendern, ein Neustart nimmt wieder den Wert der Datei.
  const pflichten = { vier_augen: m.st.vier_augen, senken_zweite_person: m.st.senken_zweite_person, halter_bescheinigung: m.st.halter_bescheinigung };
  for (const [name, x] of Object.entries(m.mcp)) pflichten[`mcp.${name}.freigabe`] = x.freigabe;
  return {
    z: {
      routen, kunden, mcp: m.mcp, st: m.st, pflichten, datei: d, ebene: 'firma', sitzungen: {}, anfragen: {}, zaehler: 0, laden_ms: ladenMs, zeit_max: ladenMs,
      antraege: {}, zaehler_antrag: 0, zugaenge: {}, zaehler_zugang: 0, nonces: {},
      flut: { fenster: 0, zeilen: {}, unterdrueckt: {} },
    },
  };
}

// ---------- Pruefer h1 und Herausforderungen ----------

const digestVon = (server, werkzeug, argumente) => sha(jcs({ argumente, server, werkzeug }));
const herausforderung = (anfrage, digest, entscheidung, vorschauSha) => jcs({ anfrage, art: 'mcp.entscheidung', digest, entscheidung, vorschau_sha256: vorschauSha });
const H_EINSICHT = (i) => jcs({ art: 'einsicht.anfragen', monat: i.monat, nonce: i.nonce, quelle: i.quelle, zweck: i.zweck });
const H_PFLICHT = (i) => jcs({ art: 'pflicht.aendern', nonce: i.nonce, pflicht: i.pflicht, wert: i.wert });
const H_BESTAETIGEN = (antrag, inhaltSha) => jcs({ antrag, art: 'antrag.bestaetigen', inhalt_sha256: inhaltSha });
const H_HANDELN = (digest, nonce, sitzung) => jcs({ art: 'mcp.handeln', digest, nonce, sitzung });
const H_ERGEBNIS = (antrag) => jcs({ antrag, art: 'antrag.ergebnis' });
const H_GEWAEHREN = (i) => jcs({ art: 'hersteller.gewaehren', frist_ms: i.frist_ms, nonce: i.nonce });
const H_BEENDEN = (zugang) => jcs({ art: 'hersteller.beenden', zugang });
const vorschauHash = (v) => (v === null ? null : sha(v));
const SCHLUESSEL_1 = Buffer.alloc(32, 0x01);
const SCHLUESSEL_2 = Buffer.alloc(32, 0x02);
const h1Hex = (schluessel, person, herausf) => createHmac('sha256', schluessel).update(Buffer.concat([Buffer.from(`${person}\n`, 'utf8'), Buffer.from(herausf, 'utf8')])).digest('hex');
function pruefer(herausf, nachweis) {
  const m = /^h1:(ps_[0-9a-f]{32}):([0-9a-f]{64})$/.exec(nachweis);
  return m && m[2] === h1Hex(SCHLUESSEL_1, m[1], herausf) ? m[1] : null;
}
const signiere = (person, herausf, schluessel = SCHLUESSEL_1) => `h1:${person}:${h1Hex(schluessel, person, herausf)}`;
const nachweisFuer = (person, anfrage, digest, entscheidung, vorschau, schluessel = SCHLUESSEL_1) => signiere(person, herausforderung(anfrage, digest, entscheidung, vorschauHash(vorschau)), schluessel);
const inhaltVon = (a) => (a.art === 'einsicht'
  ? { antragsteller: a.antragsteller, art: 'einsicht', monat: a.monat, quelle: a.quelle, zweck: a.zweck }
  : { antragsteller: a.antragsteller, art: 'pflicht', pflicht: a.pflicht, wert: a.wert });

// ---------- Start (Vertrag 1), unveraendert aus Fassung 1 ----------

const HARNESS = { claude: 'cloud', pi: 'lokal' };
function startPruefen(bytes, z) {
  const nein = (code, d) => ({ code, d });
  if (bytes.length > 4096) return nein('start.zu_gross', null);
  let d;
  try { d = JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes)); } catch { return nein('start.json_ungueltig', null); }
  if (!istObjekt(d)) return nein('start.json_ungueltig', null);
  const alle = ['fassung', 'sitzung', 'harness', 'modell', 'route', 'kunde', 'arbeitsordner'];
  if (Object.keys(d).some((k) => !alle.includes(k))) return nein('start.schluessel_unbekannt', d);
  if (alle.some((k) => !(k in d))) return nein('start.schluessel_fehlt', d);
  const txt = ['sitzung', 'harness', 'modell', 'route', 'arbeitsordner'];
  if (!Number.isInteger(d.fassung) || txt.some((k) => typeof d[k] !== 'string') || (d.kunde !== null && typeof d.kunde !== 'string')) return nein('start.typ_falsch', d);
  if (d.fassung !== 1) return nein('start.fassung_unbekannt', d);
  if (!SCHLUESSEL.test(d.sitzung) || !SCHLUESSEL.test(d.harness) || !SCHLUESSEL.test(d.route) || (d.kunde !== null && !SCHLUESSEL.test(d.kunde))) return nein('start.kennung_form', d);
  if (!/^[A-Za-z0-9._:/@+-]{1,128}$/.test(d.modell)) return nein('start.modell_form', d);
  const o = d.arbeitsordner;
  if (bytesLaenge(o) > 1024 || !o.startsWith('/') || o === '/' || o.includes('//') || o.endsWith('/') || /[\u0000-\u001f\u007f]/.test(o) || o.split('/').some((x) => x === '.' || x === '..')) return nein('start.ordner_ungueltig', d);
  if (!(d.harness in HARNESS)) return nein('start.harness_unbekannt', d);
  if (!(d.route in z.routen)) return nein('start.route_unbekannt', d);
  if (d.kunde !== null && !z.kunden.includes(d.kunde)) return nein('start.kunde_unbekannt', d);
  if (HARNESS[d.harness] !== z.routen[d.route]) return nein('start.harness_route_unvertraeglich', d);
  return { ok: true, d, art: z.routen[d.route] };
}
function startForm(eingabe) {
  if (!istObjekt(eingabe) || Object.keys(eingabe).length !== 1 || !istObjekt(eingabe.start)) return false;
  const k = Object.keys(eingabe.start);
  if (k.length !== 1) return false;
  if (k[0] === 'text') return typeof eingabe.start.text === 'string';
  if (k[0] === 'b64') return typeof eingabe.start.b64 === 'string' && /^[A-Za-z0-9+/]*={0,2}$/.test(eingabe.start.b64);
  return false;
}

// ---------- Form der Eingaben (Stufe 3) ----------

const stellenText = (v) => typeof v === 'string';
const P = {
  anfrage: (v) => stellenText(v) && ANFRAGE.test(v), antrag: (v) => stellenText(v) && ANTRAG.test(v), zugang: (v) => stellenText(v) && ZUGANG.test(v),
  argumente: (v) => istObjekt(v) && bytesLaenge(jcs(v)) <= 1048576, server: (v) => stellenText(v) && SCHLUESSEL.test(v),
  werkzeug: (v) => stellenText(v) && WERKZEUG.test(v), nachweis: (v) => stellenText(v) && v.length >= 1 && v.length <= 4096,
  nonce: (v) => stellenText(v) && NONCE.test(v),
};
function formPruefen(eingabe, felder, wahl = []) {
  if (!istObjekt(eingabe)) return false;
  const k = Object.keys(eingabe);
  if (k.some((x) => !(x in felder))) return false;
  if (Object.keys(felder).some((x) => !wahl.includes(x) && !(x in eingabe))) return false;
  return k.every((name) => felder[name](eingabe[name]));
}
const FORM = {
  'mcp.anfragen': [{
    argumente: P.argumente, frist_ms: (v) => ganzIn(v, 1, 86400000), person: (v) => stellenText(v) && PSEUDONYM.test(v), server: P.server,
    sitzung: (v) => v === null || P.server(v), vorschau: (v) => v === null || (stellenText(v) && bytesLaenge(v) <= 65536), werkzeug: P.werkzeug, zweite_person: (v) => typeof v === 'boolean',
    nachweis: P.nachweis, nonce: P.nonce,
  }, ['zweite_person', 'nachweis', 'nonce']],
  'mcp.entscheiden': [{ anfrage: P.anfrage, entscheidung: (v) => v === 'erteilen' || v === 'ablehnen', nachweis: P.nachweis }],
  'mcp.einloesen': [{ anfrage: P.anfrage, argumente: P.argumente, server: P.server, werkzeug: P.werkzeug }],
  'mcp.offene': [{}],
  'mcp.stand': [{ anfrage: P.anfrage }],
  'ablauf.pruefen': [{}],
  'einsicht.anfragen': [{ monat: (v) => stellenText(v) && MONAT.test(v), nachweis: P.nachweis, nonce: P.nonce, quelle: (v) => QUELLEN.includes(v), zweck: (v) => v === null || (stellenText(v) && SCHLUESSEL.test(v)) }],
  'pflicht.aendern': [{ nachweis: P.nachweis, nonce: P.nonce, pflicht: (v) => stellenText(v) && SCHLUESSEL.test(v), wert: (v) => typeof v === 'boolean' }],
  'antrag.bestaetigen': [{ antrag: P.antrag, nachweis: P.nachweis }],
  'antrag.offene': [{}],
  'antrag.ergebnis': [{ antrag: P.antrag, nachweis: P.nachweis }],
  'hersteller.gewaehren': [{ frist_ms: (v) => v === null || ganzIn(v, 0, 2592000000), nachweis: P.nachweis, nonce: P.nonce }],
  'hersteller.beenden': [{ nachweis: P.nachweis, zugang: P.zugang }],
  'hersteller.zugriff': [{ zugang: P.zugang }],
};

// ---------- Zeilen ----------

function endzeile(r, ergebnis) {
  const w = {
    aktionsname: 'mcp.schreibaufruf', anfrage: r.id, digest: r.digest, entscheidend: r.entscheidend, ergebnis, person: r.person,
    server: r.server, sitzung: r.sitzung, werkzeug: r.werkzeug, zeit_anfrage: zeitText(r.zeit_anfrage),
    zeit_entscheidung: r.zeit_entscheidung === null ? null : zeitText(r.zeit_entscheidung),
  };
  // Vier Augen (Abschnitt 4.2): nur bei Anfragen mit zwei noetigen Freigaben zwei Felder mehr.
  if (r.noetig === 2) { w.mitentscheidend = r.mit; w.zeit_mitentscheidung = r.zeit_mit === null ? null : zeitText(r.zeit_mit); }
  return w;
}
const zugangZeile = (g, ergebnis, person) => ({ aktionsname: 'hersteller.zugang', ergebnis, fall: g.fall, person, zugang: g.id, bis: g.bis === null ? null : zeitText(g.bis) });
const einsichtZeile = (ergebnis, a) => ({ aktionsname: 'protokoll.einsicht', antrag: a.id ?? null, ergebnis, monat: a.monat, person: a.antragsteller, quelle: a.quelle, zweck: a.zweck, zweiter: a.zweiter ?? null });
const pflichtZeile = (ergebnis, a) => ({ aktionsname: 'pflicht.aenderung', antrag: a.id ?? null, ergebnis, person: a.antragsteller, pflicht: a.pflicht, wert: a.wert, zweiter: a.zweiter ?? null });
const nachZahl = (a, b) => Number(a.split('-')[2]) - Number(b.split('-')[2]);

// ---------- entscheide ----------

function entscheide(z0, e, jetzt) {
  if (!Number.isInteger(jetzt) || jetzt < 0 || jetzt > MAX_MS) return { z: z0, antwort: fehlerAntwort('stelle.zeit_ungueltig'), zeilen: [], zeit_ms: null };
  const z = structuredClone(z0);
  const eff = Math.max(jetzt, z.zeit_max);
  z.zeit_max = eff;
  const zeilen = [];
  // Stufe 1a: Fenster der Flutbegrenzung (Abschnitt 6.2). Endet ein Fenster mit unterdrueckten Ablehnungen, je Gruppe eine Sammelzeile.
  const fl = z.st.flut;
  const fenster = Math.floor((eff - z.laden_ms) / fl.fenster_ms);
  if (fenster !== z.flut.fenster) {
    const von = z.laden_ms + z.flut.fenster * fl.fenster_ms;
    for (const key of Object.keys(z.flut.unterdrueckt).sort()) {
      const [aufrufer, vorgang, grund] = key.split('|');
      zeilen.push({ aktionsname: 'vorgang.gedrosselt', anzahl: z.flut.unterdrueckt[key], aufrufer, ergebnis: 'abgelehnt', fenster_bis: zeitText(Math.min(von + fl.fenster_ms, MAX_MS)), fenster_von: zeitText(von), grund, vorgang });
    }
    z.flut = { fenster, zeilen: {}, unterdrueckt: {} };
  }
  // Stufe 1b: Fristen. Anfragen, dann Antraege, dann Zugaenge, je aufsteigend nach Kennung.
  let abgelaufen = 0;
  const ids = Object.keys(z.anfragen).sort(nachZahl);
  for (const id of ids) {
    const r = z.anfragen[id];
    if (['offen', 'erste_freigabe', 'erteilt'].includes(r.zustand) && eff >= r.bis) { r.zustand = 'abgelaufen'; r.vorschau = null; zeilen.push(endzeile(r, 'abgelaufen')); abgelaufen += 1; }
  }
  for (const id of Object.keys(z.antraege).sort(nachZahl)) {
    const a = z.antraege[id];
    if (a.zustand === 'offen' && eff >= a.bis) { a.zustand = 'abgelaufen'; zeilen.push(a.art === 'einsicht' ? einsichtZeile('abgelaufen', a) : pflichtZeile('abgelaufen', a)); abgelaufen += 1; }
    // Abholfrist (Nachtrag 3): eine bestaetigte Einsicht, die niemand abholt, verfaellt.
    if (a.art === 'einsicht' && a.zustand === 'bestaetigt' && eff >= a.bis) { a.zustand = 'verfallen'; zeilen.push(einsichtZeile('verfallen', a)); abgelaufen += 1; }
  }
  for (const id of Object.keys(z.zugaenge).sort(nachZahl)) {
    const g = z.zugaenge[id];
    if (g.zustand === 'aktiv' && g.bis !== null && eff >= g.bis) { g.zustand = 'abgelaufen'; zeilen.push(zugangZeile(g, 'abgelaufen', null)); abgelaufen += 1; }
  }
  let datei = null;
  const aus = (antwort) => ({ z, antwort, zeilen, zeit_ms: eff, ...(datei ? { datei } : {}) });
  // Nachtrag 5: Eine wirksame Pflichtaenderung schreibt die Stelle selbst in die Datei der Ebene. Erst die Zeile regel.geaendert, dann die Datei (Huelle).
  const regelSchreiben = (pf, wert, handelnd, zweiter, antrag) => {
    const alt = sha(jcs(z.datei));
    const neuDatei = structuredClone(z.datei);
    if (pf.startsWith('mcp.')) neuDatei.mcp[pf.slice(4, -'.freigabe'.length)].freigabe = wert;
    else neuDatei.stelle = { ...(neuDatei.stelle ?? {}), [pf]: wert };
    z.datei = neuDatei;
    z.pflichten[pf] = wert;
    const text = jcs(neuDatei);
    zeilen.push({ aktionsname: 'regel.geaendert', alt, antrag, code: null, ebene: z.ebene, ergebnis: 'geaendert', handelnd, neu: sha(text), pflicht: pf, ziel: null, zweiter });
    datei = { ebene: z.ebene, text };
  };
  const nein = (code, sicht = {}) => {
    const zeile = e.vorgang === 'sitzung.starten'
      ? { aktionsname: 'sitzung.abgelehnt', ergebnis: 'abgelehnt', grund: code, ...stelleSicht(sicht.d ?? null) }
      : { aktionsname: 'vorgang.abgelehnt', anfrage: kennung, aufrufer: e.aufrufer, ergebnis: 'abgelehnt', grund: code, vorgang: e.vorgang };
    // Flutbegrenzung je Aufruferart (Abschnitt 6.2)
    const n = z.flut.zeilen[e.aufrufer] ?? 0;
    if (n < fl.zeilen_je_fenster) { z.flut.zeilen[e.aufrufer] = n + 1; zeilen.push(zeile); } else {
      const key = `${e.aufrufer}|${e.vorgang}|${code}`;
      z.flut.unterdrueckt[key] = (z.flut.unterdrueckt[key] ?? 0) + 1;
    }
    return aus(fehlerAntwort(code));
  };
  const ein = e.eingabe;
  const kennung = istObjekt(ein) ? ([['anfrage', ANFRAGE], ['antrag', ANTRAG], ['zugang', ZUGANG]].map(([f, re]) => (typeof ein[f] === 'string' && re.test(ein[f]) ? ein[f] : null)).find((x) => x !== null) ?? null) : null;
  if (!(ERLAUBT[e.vorgang] ?? []).includes(e.aufrufer)) return nein('stelle.aufrufer_nicht_erlaubt');
  const form = e.vorgang === 'sitzung.starten' ? startForm(ein) : formPruefen(ein, ...FORM[e.vorgang]);
  if (!form) return nein('stelle.eingabe_ungueltig');
  // Nachtrag 7: Mensch gegen Modell folgt aus dem Kanal, nie aus einem Feld. Nur brain_mensch traegt nachweis und nonce (beide zusammen) und darf ohne Sitzung fragen.
  if (e.vorgang === 'mcp.anfragen' && (('nachweis' in ein) !== ('nonce' in ein) || (e.aufrufer !== 'brain_mensch' && ('nachweis' in ein || ein.sitzung === null)))) return nein('stelle.eingabe_ungueltig');

  if (e.vorgang === 'ablauf.pruefen') return aus({ ok: true, ergebnis: 'geprueft', abgelaufen });
  if (e.vorgang === 'mcp.offene') {
    const offene = ids.filter((id) => ['offen', 'erste_freigabe'].includes(z.anfragen[id].zustand)).map((id) => {
      const r = z.anfragen[id];
      const inhalt = { digest: r.digest, server: r.server, vorschau_sha256: r.vorschau_sha256, werkzeug: r.werkzeug };
      // Nachtrag 3, I2: einheitliche Felder in mcp.offene und antrag.offene
      return {
        anfrage: id, server: r.server, werkzeug: r.werkzeug, vorschau: r.vorschau, vorschau_sha256: r.vorschau_sha256, digest: r.digest, frist_bis_ms: r.bis,
        antragsteller: r.person, art: 'mcp.schreibaufruf', erste_freigabe: r.mit === null ? null : { person: r.mit, zeit_ms: r.zeit_mit }, inhalt, inhalt_sha256: sha(jcs(inhalt)),
      };
    });
    return aus({ ok: true, ergebnis: 'offene', offene });
  }
  if (e.vorgang === 'antrag.offene') {
    const liste = Object.keys(z.antraege).sort(nachZahl).filter((id) => z.antraege[id].zustand === 'offen').map((id) => {
      const a = z.antraege[id];
      const inhalt = inhaltVon(a);
      return { antrag: id, antragsteller: a.antragsteller, art: a.art, erste_freigabe: null, frist_bis_ms: a.bis, inhalt, inhalt_sha256: sha(jcs(inhalt)) };
    });
    return aus({ ok: true, ergebnis: 'antraege', antraege: liste });
  }

  if (e.vorgang === 'sitzung.starten') {
    const bytes = ein.start.text !== undefined ? Buffer.from(ein.start.text, 'utf8') : Buffer.from(ein.start.b64, 'base64');
    const r = startPruefen(bytes, z);
    if (!r.ok) return nein(r.code, { d: r.d });
    if (r.d.sitzung in z.sitzungen) return nein('stelle.sitzung_vergeben', { d: r.d });
    z.sitzungen[r.d.sitzung] = { route: r.d.route, kunde: r.d.kunde, harness: r.d.harness };
    zeilen.push({ aktionsname: 'sitzung.gestartet', ergebnis: 'gestartet', grund: null, ...stelleSicht(r.d) });
    return aus({ ok: true, ergebnis: 'gestartet', sitzung: r.d.sitzung, route: r.d.route, route_art: r.art });
  }

  const i = ein;
  // Gemeinsame Stufen der Vorgaenge mit Nachweis: pruefen, Nonce verbrauchen, Rolle.
  const person = (herausf) => pruefer(herausf, i.nachweis);
  const hatRolle = (p, rollen) => (z.st.rollen[p] ?? []).some((x) => rollen.includes(x));
  const nonceNehmen = () => { if (z.nonces[i.nonce]) return false; z.nonces[i.nonce] = true; return true; };

  if (e.vorgang === 'mcp.anfragen') {
    const istBrain = i.server === SERVER_BRAIN;
    if ((e.aufrufer === 'brain' || e.aufrufer === 'brain_mensch') !== istBrain) return nein('stelle.server_nicht_erlaubt');
    const s = z.mcp[i.server];
    if (!s) return nein('stelle.server_unbekannt');
    if (i.sitzung !== null && !(i.sitzung in z.sitzungen)) return nein('stelle.sitzung_unbekannt');
    if (s.lesen.includes(i.werkzeug)) return aus({ ok: true, ergebnis: 'lesend' });
    const zweite = i.zweite_person === true || (z.pflichten.senken_zweite_person && s.zweite_person.includes(i.werkzeug));
    const digest = digestVon(i.server, i.werkzeug, i.argumente);
    const r = { art: e.aufrufer, server: i.server, werkzeug: i.werkzeug, sitzung: i.sitzung, person: i.person, digest, vorschau: i.vorschau, vorschau_sha256: vorschauHash(i.vorschau), zeit_anfrage: eff, entscheidend: null, zeit_entscheidung: null };
    const mensch = e.aufrufer === 'brain_mensch';
    if (mensch) {
      // Nachtrag 7: Der Mensch handelt selbst. Gueltiger Nachweis ueber genau diesen Aufruf, Pseudonym gleich person, Nonce einmal.
      const h = 'nachweis' in i ? pruefer(H_HANDELN(digest, i.nonce, i.sitzung), i.nachweis) : null;
      if (h === null || h !== i.person) return nein('stelle.nachweis_ungueltig');
      if (!nonceNehmen()) return nein('stelle.nonce_benutzt');
    }
    // Modellweg (Nachtrag 10, feste Regel): Art brain mit Server brain und Werkzeug brain_senken ist immer offen (A38),
    // unabhaengig von freigabe und von zweite_person; sonst folgt der Modellweg der Freigabe des Servers.
    // Mensch: ohne Pflicht zur zweiten Person ohne Freigabe; mit Pflicht offen fuer eine andere Person.
    const senken = e.aufrufer === 'brain' && i.server === SERVER_BRAIN && i.werkzeug === WERKZEUG_SENKEN;
    const braucht = mensch ? zweite : (zweite || senken || z.pflichten[`mcp.${i.server}.freigabe`]);
    if (!braucht) {
      z.zaehler += 1; r.id = `mf-${z.laden_ms}-${z.zaehler}`;
      if (mensch) { r.entscheidend = i.person; r.zeit_entscheidung = eff; }
      zeilen.push(endzeile(r, 'ohne_freigabe'));
      return aus({ ok: true, ergebnis: 'ohne_freigabe', anfrage: r.id, digest, entscheidend: r.entscheidend, mitentscheidend: null, werkzeug: r.werkzeug, zweite_person: false });
    }
    const laufend = Object.values(z.anfragen).filter((q) => q.sitzung === i.sitzung && ['offen', 'erste_freigabe', 'erteilt'].includes(q.zustand)).length;
    if (laufend >= fl.offene_je_sitzung) return nein('stelle.zu_viele_offen');
    if (i.frist_ms > s.frist_ms) return nein('stelle.frist_ungueltig');
    z.zaehler += 1; r.id = `mf-${z.laden_ms}-${z.zaehler}`; r.bis = eff + i.frist_ms; r.zustand = 'offen';
    r.noetig = z.pflichten.vier_augen ? 2 : 1; r.mit = null; r.zeit_mit = null; r.zweite_person = zweite;
    z.anfragen[r.id] = r;
    return aus({ ok: true, ergebnis: 'offen', anfrage: r.id, digest, frist_bis_ms: r.bis, vorschau_sha256: r.vorschau_sha256, entscheidend: null, mitentscheidend: null, werkzeug: r.werkzeug, zweite_person: zweite });
  }

  if (['mcp.entscheiden', 'mcp.einloesen', 'mcp.stand'].includes(e.vorgang)) {
    const r = z.anfragen[i.anfrage];
    if (!r) return nein('stelle.anfrage_unbekannt');
    // Eine Anfrage gehoert zur Art, die sie stellte: brain sieht nur Server brain, hub nur die anderen (Abschnitt 4.4).
    if (e.vorgang !== 'mcp.entscheiden' && e.aufrufer !== r.art) return nein('stelle.anfrage_unbekannt');
    if (e.vorgang === 'mcp.entscheiden') {
      if (r.zustand === 'abgelaufen') return nein('stelle.anfrage_abgelaufen');
      if (!['offen', 'erste_freigabe'].includes(r.zustand)) return nein('stelle.bereits_entschieden');
      const p = person(herausforderung(r.id, r.digest, i.entscheidung, r.vorschau_sha256));
      if (p === null) return nein('stelle.nachweis_ungueltig');
      if (i.entscheidung === 'ablehnen') {
        r.entscheidend = p; r.zeit_entscheidung = eff; r.vorschau = null; r.zustand = 'abgelehnt';
        zeilen.push(endzeile(r, 'abgelehnt'));
        return aus({ ok: true, ergebnis: 'abgelehnt', anfrage: r.id });
      }
      if (r.zweite_person && p === r.person) return nein('stelle.selbe_person');
      if (r.zustand === 'erste_freigabe' && p === r.mit) return nein('stelle.selbe_person');
      if (r.noetig === 2 && r.zustand === 'offen') {
        r.mit = p; r.zeit_mit = eff; r.zustand = 'erste_freigabe';
        return aus({ ok: true, ergebnis: 'erste_freigabe', anfrage: r.id });
      }
      r.entscheidend = p; r.zeit_entscheidung = eff; r.vorschau = null;
      r.zustand = 'erteilt'; r.bis = eff + z.mcp[r.server].einloesefrist_ms;
      return aus({ ok: true, ergebnis: 'erteilt', anfrage: r.id, einloesen_bis_ms: r.bis });
    }
    if (e.vorgang === 'mcp.stand') {
      const lauf = ['offen', 'erste_freigabe', 'erteilt'].includes(r.zustand);
      return aus({ ok: true, ergebnis: 'stand', anfrage: r.id, zustand: r.zustand, frist_bis_ms: lauf ? r.bis : null, ...personen(r) });
    }
    const stop = { offen: 'stelle.nicht_freigegeben', erste_freigabe: 'stelle.zweite_person_fehlt', abgelehnt: 'stelle.anfrage_abgelehnt', abgelaufen: 'stelle.anfrage_abgelaufen', eingeloest: 'stelle.bereits_eingeloest' }[r.zustand];
    if (stop) return nein(stop);
    if (digestVon(i.server, i.werkzeug, i.argumente) !== r.digest) return nein('stelle.digest_abweichend');
    r.zustand = 'eingeloest';
    zeilen.push(endzeile(r, 'eingeloest'));
    return aus({ ok: true, ergebnis: 'eingeloest', anfrage: r.id, digest: r.digest, ...personen(r) });
  }

  if (e.vorgang === 'einsicht.anfragen') {
    // B08: einsicht.regel der Stelle; niemand sperrt jede Einsicht vor dem Nachweis (wie hersteller_aus).
    if (z.st.einsicht_regel === 'niemand') return nein('stelle.einsicht_aus');
    const p = person(H_EINSICHT(i));
    if (p === null) return nein('stelle.nachweis_ungueltig');
    if (!nonceNehmen()) return nein('stelle.nonce_benutzt');
    if (!hatRolle(p, ['einsicht'])) return nein('stelle.rolle_fehlt');
    const vier = z.st.einsicht_regel === 'rolle-vier-augen';
    if ((vier && i.zweck === null) || (i.zweck !== null && !z.st.zwecke.includes(i.zweck))) return nein('stelle.zweck_ungueltig');
    const a = { art: 'einsicht', antragsteller: p, monat: i.monat, quelle: i.quelle, zweck: i.zweck };
    if (!vier) { zeilen.push(einsichtZeile('erlaubt', a)); return aus({ ok: true, ergebnis: 'einsicht_erlaubt', antrag: null, monat: i.monat, quelle: i.quelle }); }
    z.zaehler_antrag += 1; a.id = `an-${z.laden_ms}-${z.zaehler_antrag}`; a.bis = eff + z.st.antrag_frist_ms; a.zustand = 'offen';
    z.antraege[a.id] = a;
    return aus({ ok: true, ergebnis: 'antrag_offen', antrag: a.id, frist_bis_ms: a.bis });
  }

  if (e.vorgang === 'pflicht.aendern') {
    if (!(i.pflicht in z.pflichten)) return nein('stelle.pflicht_unbekannt');
    const p = person(H_PFLICHT(i));
    if (p === null) return nein('stelle.nachweis_ungueltig');
    if (!nonceNehmen()) return nein('stelle.nonce_benutzt');
    if (!hatRolle(p, ['admin'])) return nein('stelle.rolle_fehlt');
    if (z.pflichten[i.pflicht] === i.wert) return nein('stelle.pflicht_unveraendert');
    if (Object.values(z.antraege).some((a) => a.art === 'pflicht' && a.zustand === 'offen' && a.pflicht === i.pflicht)) return nein('stelle.antrag_laeuft');
    const a = { art: 'pflicht', antragsteller: p, pflicht: i.pflicht, wert: i.wert };
    if (i.wert === true) { regelSchreiben(i.pflicht, true, p, null, null); return aus({ ok: true, ergebnis: 'geaendert', antrag: null, pflicht: i.pflicht, wert: true }); }
    z.zaehler_antrag += 1; a.id = `an-${z.laden_ms}-${z.zaehler_antrag}`; a.bis = eff + z.st.antrag_frist_ms; a.zustand = 'offen';
    z.antraege[a.id] = a;
    zeilen.push(pflichtZeile('antrag', a));
    return aus({ ok: true, ergebnis: 'antrag_offen', antrag: a.id, frist_bis_ms: a.bis });
  }

  if (e.vorgang === 'antrag.bestaetigen') {
    const a = z.antraege[i.antrag];
    if (!a) return nein('stelle.antrag_unbekannt');
    if (a.zustand === 'abgelaufen') return nein('stelle.antrag_abgelaufen');
    if (a.zustand !== 'offen') return nein('stelle.bereits_entschieden');
    const p = person(H_BESTAETIGEN(a.id, sha(jcs(inhaltVon(a)))));
    if (p === null) return nein('stelle.nachweis_ungueltig');
    if (p === a.antragsteller) return nein('stelle.selbe_person');
    if (!hatRolle(p, a.art === 'einsicht' ? ['einsicht', 'admin'] : ['admin'])) return nein('stelle.rolle_fehlt');
    a.zweiter = p; a.zustand = 'bestaetigt';
    if (a.art === 'einsicht') {
      // Nachtrag 3, I1: Die zweite Person sieht nie die Zeilen; der Antragsteller holt sie mit antrag.ergebnis ab, einmal, innerhalb der Abholfrist.
      a.bis = eff + z.st.antrag_frist_ms;
      zeilen.push(einsichtZeile('erlaubt', a));
      return aus({ ok: true, ergebnis: 'bestaetigt', antrag: a.id, abholen_bis_ms: a.bis });
    }
    regelSchreiben(a.pflicht, a.wert, a.antragsteller, p, a.id);
    return aus({ ok: true, ergebnis: 'geaendert', antrag: a.id, pflicht: a.pflicht, wert: a.wert });
  }

  if (e.vorgang === 'antrag.ergebnis') {
    const a = z.antraege[i.antrag];
    if (!a || a.art !== 'einsicht') return nein('stelle.antrag_unbekannt');
    const p = person(H_ERGEBNIS(a.id));
    if (p === null) return nein('stelle.nachweis_ungueltig');
    if (p !== a.antragsteller) return nein('stelle.nicht_antragsteller');
    const stop = { offen: 'stelle.antrag_nicht_bestaetigt', abgelaufen: 'stelle.antrag_abgelaufen', verfallen: 'stelle.antrag_abgelaufen', abgeholt: 'stelle.bereits_abgeholt' }[a.zustand];
    if (stop) return nein(stop);
    a.zustand = 'abgeholt';
    zeilen.push(einsichtZeile('abgeholt', a));
    return aus({ ok: true, ergebnis: 'einsicht_erlaubt', antrag: a.id, monat: a.monat, quelle: a.quelle });
  }

  if (e.vorgang === 'hersteller.gewaehren') {
    const h = z.st.hersteller;
    if (h.fall === 'kein') return nein('stelle.hersteller_aus');
    const f = i.frist_ms;
    if (f === 0 || (f === null && h.fall !== 'dauerhaft') || (f !== null && (f > h.hoechstfrist_ms || eff + f > MAX_MS))) return nein('stelle.frist_ungueltig');
    const p = person(H_GEWAEHREN(i));
    if (p === null) return nein('stelle.nachweis_ungueltig');
    if (!nonceNehmen()) return nein('stelle.nonce_benutzt');
    if (!hatRolle(p, h.rollen)) return nein('stelle.rolle_fehlt');
    if (Object.values(z.zugaenge).some((g) => g.zustand === 'aktiv')) return nein('stelle.zugang_aktiv');
    z.zaehler_zugang += 1;
    const g = { id: `hz-${z.laden_ms}-${z.zaehler_zugang}`, fall: f === null ? 'dauerhaft' : 'befristet', bis: f === null ? null : eff + f, zustand: 'aktiv', person: p };
    z.zugaenge[g.id] = g;
    zeilen.push(zugangZeile(g, 'gewaehrt', p));
    return aus({ ok: true, ergebnis: 'gewaehrt', zugang: g.id, bis_ms: g.bis });
  }
  // hersteller.beenden und hersteller.zugriff
  const g = z.zugaenge[i.zugang];
  if (!g) return nein('stelle.zugang_unbekannt');
  if (g.zustand === 'abgelaufen') return nein('stelle.zugang_abgelaufen');
  if (g.zustand === 'beendet') return nein('stelle.zugang_beendet');
  if (e.vorgang === 'hersteller.zugriff') {
    zeilen.push({ aktionsname: 'hersteller.zugriff', ergebnis: 'erlaubt', zugang: g.id });
    return aus({ ok: true, ergebnis: 'erlaubt', zugang: g.id, bis_ms: g.bis });
  }
  const p = person(H_BEENDEN(g.id));
  if (p === null) return nein('stelle.nachweis_ungueltig');
  if (!hatRolle(p, z.st.hersteller.rollen)) return nein('stelle.rolle_fehlt');
  g.zustand = 'beendet';
  zeilen.push(zugangZeile(g, 'beendet', p));
  return aus({ ok: true, ergebnis: 'beendet', zugang: g.id });
}
const personen = (r) => ({ entscheidend: r.entscheidend, mitentscheidend: r.mit, werkzeug: r.werkzeug, zweite_person: r.zweite_person });
const stelleSicht = (d) => ({ harness: d ? sicher(d.harness) : null, kunde: d ? sicher(d.kunde) : null, route: d ? sicher(d.route) : null, sitzung: d ? sicher(d.sitzung) : null });

// ---------- Eigener Zeilenpruefer (alle Zeilenarten von Fassung 1 und 2) ----------

const SCHL = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
const SCHL0 = (v) => v === null || SCHL(v);
const PS = (v) => typeof v === 'string' && PSEUDONYM.test(v);
const PS0 = (v) => v === null || PS(v);
const Z0 = (v) => v === null || (typeof v === 'string' && ZEIT.test(v));
const KENNUNG0 = (v) => v === null || (typeof v === 'string' && (ANFRAGE.test(v) || ANTRAG.test(v) || ZUGANG.test(v)));
const SCHREIB = {
  anfrage: (v) => ANFRAGE.test(v), digest: (v) => /^[0-9a-f]{64}$/.test(v), entscheidend: PS0,
  ergebnis: (v) => ['eingeloest', 'abgelehnt', 'abgelaufen', 'ohne_freigabe'].includes(v), person: PS, server: SCHL, sitzung: SCHL0,
  werkzeug: (v) => typeof v === 'string' && WERKZEUG.test(v), zeit_anfrage: (v) => ZEIT.test(v), zeit_entscheidung: Z0,
};
const FORMEN = {
  'sitzung.gestartet': { ergebnis: (v) => v === 'gestartet', grund: (v) => v === null, harness: SCHL0, kunde: SCHL0, route: SCHL0, sitzung: SCHL0 },
  'sitzung.abgelehnt': { ergebnis: (v) => v === 'abgelehnt', grund: SCHL, harness: SCHL0, kunde: SCHL0, route: SCHL0, sitzung: SCHL0 },
  'vorgang.abgelehnt': { anfrage: KENNUNG0, aufrufer: (v) => ARTEN.includes(v), ergebnis: (v) => v === 'abgelehnt', grund: SCHL, vorgang: (v) => VORGAENGE.includes(v) },
  'mcp.schreibaufruf': SCHREIB,
  'mcp.schreibaufruf+': { ...SCHREIB, mitentscheidend: PS0, zeit_mitentscheidung: Z0 },
  'regel.geaendert': { alt: (v) => /^[0-9a-f]{64}$/.test(v), antrag: (v) => v === null || ANTRAG.test(v), code: (v) => v === null, ebene: (v) => v === 'firma', ergebnis: (v) => v === 'geaendert', handelnd: PS, neu: (v) => /^[0-9a-f]{64}$/.test(v), pflicht: SCHL, ziel: (v) => v === null, zweiter: PS0 },
  'vorgang.gedrosselt': { anzahl: (v) => ganzIn(v, 1, 4294967295), aufrufer: (v) => ARTEN.includes(v), ergebnis: (v) => v === 'abgelehnt', fenster_bis: (v) => ZEIT.test(v), fenster_von: (v) => ZEIT.test(v), grund: SCHL, vorgang: (v) => VORGAENGE.includes(v) },
  'hersteller.zugang': { bis: Z0, ergebnis: (v) => ['gewaehrt', 'beendet', 'abgelaufen'].includes(v), fall: (v) => ['befristet', 'dauerhaft'].includes(v), person: PS0, zugang: (v) => ZUGANG.test(v) },
  'hersteller.zugriff': { ergebnis: (v) => v === 'erlaubt', zugang: (v) => ZUGANG.test(v) },
  'protokoll.einsicht': { antrag: (v) => v === null || ANTRAG.test(v), ergebnis: (v) => ['erlaubt', 'abgeholt', 'abgelaufen', 'verfallen'].includes(v), monat: (v) => MONAT.test(v), person: PS, quelle: (v) => QUELLEN.includes(v), zweck: SCHL0, zweiter: PS0 },
  'pflicht.aenderung': { antrag: (v) => v === null || ANTRAG.test(v), ergebnis: (v) => ['antrag', 'abgelaufen'].includes(v), person: PS, pflicht: SCHL, wert: (v) => typeof v === 'boolean', zweiter: PS0 },
};

function zeilePruefen(text, vorher) {
  const w = JSON.parse(text);
  if (jcs(w) !== text) throw new Error('nicht kanonisch');
  if (w.fassung !== 2 || typeof w.zeit !== 'string' || !ZEIT.test(w.zeit) || w.vorher !== vorher) throw new Error('Kopf oder Kette falsch');
  const art = w.aktionsname === 'mcp.schreibaufruf' && 'mitentscheidend' in w ? 'mcp.schreibaufruf+' : w.aktionsname;
  const form = FORMEN[art];
  if (!form) throw new Error(`Zeilenart unbekannt: ${w.aktionsname}`);
  const soll = [...Object.keys(form), 'aktionsname', 'fassung', 'vorher', 'zeit'].sort();
  if (JSON.stringify(Object.keys(w).sort()) !== JSON.stringify(soll)) throw new Error(`Felder nicht geschlossen: ${text}`);
  for (const [k, p] of Object.entries(form)) if (!p(w[k])) throw new Error(`Feld ${k} falsch: ${text}`);
  if (w.aktionsname === 'mcp.schreibaufruf') {
    if ((w.entscheidend === null) !== (w.zeit_entscheidung === null)) throw new Error('entscheidend und zeit_entscheidung gehoeren zusammen');
    if ((w.ergebnis === 'abgelehnt' || w.ergebnis === 'eingeloest') && w.entscheidend === null) throw new Error('Ergebnis ohne Freigebenden');
    if (art === 'mcp.schreibaufruf+') {
      if ((w.mitentscheidend === null) !== (w.zeit_mitentscheidung === null)) throw new Error('mitentscheidend und Zeit gehoeren zusammen');
      if (w.ergebnis === 'eingeloest' && (w.mitentscheidend === null || w.mitentscheidend === w.entscheidend)) throw new Error('Vier Augen ohne zwei Personen eingeloest');
    }
  }
  if (w.aktionsname === 'protokoll.einsicht' && w.antrag !== null && ['erlaubt', 'abgeholt', 'verfallen'].includes(w.ergebnis) && (w.zweiter === null || w.zweiter === w.person)) throw new Error('Einsicht mit Antrag ohne zweite Person');
  if (w.aktionsname === 'regel.geaendert' && (w.antrag === null) !== (w.zweiter === null)) throw new Error('Antrag und zweite Person gehoeren zusammen');
  if (w.aktionsname === 'regel.geaendert' && w.zweiter === w.handelnd) throw new Error('Lockerung durch eine Person');
  if (w.aktionsname === 'hersteller.zugang' && (w.ergebnis === 'abgelaufen') !== (w.person === null)) throw new Error('Person beim Zugang falsch');
  return sha(text);
}

// ---------- Lauf eines Falls ----------

const gleich = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const ende = (z) => ({
  sitzungen: Object.keys(z.sitzungen).sort(), anfragen: Object.fromEntries(Object.keys(z.anfragen).map((k) => [k, z.anfragen[k].zustand])), zaehler: z.zaehler,
});
const ende2 = (z) => ({
  ...ende(z), antraege: Object.fromEntries(Object.keys(z.antraege).map((k) => [k, z.antraege[k].zustand])),
  zugaenge: Object.fromEntries(Object.keys(z.zugaenge).map((k) => [k, z.zugaenge[k].zustand])), pflichten: Object.fromEntries(Object.keys(z.pflichten).sort().map((k) => [k, z.pflichten[k]])),
});
const MARKER = ['GEHEIM-ARG-7Q', 'VORSCHAU-5K'];
function zeilenSchreiben(r, vorher, wo) {
  const texte = [];
  for (const w of r.zeilen) {
    const text = jcs({ ...w, fassung: 2, vorher, zeit: zeitText(r.zeit_ms) });
    vorher = zeilePruefen(text, vorher);
    for (const m of MARKER) if (text.includes(m)) throw new Error(`${wo}: Inhalt in der Zeile: ${m}`);
    if (text.includes('Beispiel ') || text.includes('Max ') || text.includes('/home/')) throw new Error(`${wo}: Name oder Pfad in der Zeile`);
    texte.push({ text, sha256: vorher });
  }
  return { texte, vorher };
}

// ---------- Pruefung 1: Gleichlauf mit allen Faellen von Fassung 1 ----------

let gleichlauf = 0;
for (const datei of readdirSync(F1_FAELLE).filter((f) => f.endsWith('.json')).sort()) {
  const f = JSON.parse(readFileSync(join(F1_FAELLE, datei), 'utf8'));
  const wo0 = `Gleichlauf ${datei}`;
  const geladen = modellLaden(f.einstellungen.text, f.laden_ms);
  if (f.erwartet_laden) {
    if (geladen.fehler !== f.erwartet_laden.fehler) throw new Error(`${wo0}: Laden ${geladen.fehler ?? 'ok'}, Fassung 1 ${f.erwartet_laden.fehler}`);
    gleichlauf += 1;
    continue;
  }
  if (geladen.fehler) throw new Error(`${wo0}: laedt nicht (${geladen.fehler})`);
  let z = geladen.z;
  let vorher = NULL64;
  let zeilen = 0;
  for (const [n, s] of f.schritte.entries()) {
    if (s.neustart) { z = modellLaden(f.einstellungen.text, s.neustart.laden_ms).z; continue; }
    const r = entscheide(z, { aufrufer: s.aufrufer, vorgang: s.vorgang, eingabe: s.eingabe }, s.jetzt_ms);
    const antwort = { ...r.antwort };
    if (['mcp.anfragen', 'mcp.stand', 'mcp.einloesen'].includes(s.vorgang) && antwort.ok) { delete antwort.entscheidend; delete antwort.zweite_person; delete antwort.mitentscheidend; delete antwort.werkzeug; }
    if (s.vorgang === 'mcp.offene' && antwort.ok) antwort.offene = antwort.offene.map(({ antragsteller, art, erste_freigabe, inhalt, inhalt_sha256, ...rest }) => rest);
    if (jcs(antwort) !== jcs(s.erwartet.antwort)) throw new Error(`${wo0} Schritt ${n + 1}: Antwort ${jcs(antwort)} statt ${jcs(s.erwartet.antwort)}`);
    const t = zeilenSchreiben(r, vorher, wo0);
    if (!gleich(t.texte, s.erwartet.zeilen)) throw new Error(`${wo0} Schritt ${n + 1}: Zeilenbytes weichen ab`);
    vorher = t.vorher; zeilen += t.texte.length; z = r.z;
  }
  const e = { ...ende(z), zeilen, letzte: zeilen ? vorher : null };
  if (!gleich(e, f.ende)) throw new Error(`${wo0}: Endzustand ${JSON.stringify(e)} statt ${JSON.stringify(f.ende)}`);
  gleichlauf += 1;
}

// ---------- Fall-Bausteine ----------

const P1 = `ps_${'0'.repeat(31)}1`;
const H1 = `ps_${'0'.repeat(29)}a01`;
const H2 = `ps_${'0'.repeat(29)}a02`;
const A1 = `ps_${'0'.repeat(29)}b01`; // admin
const A2 = `ps_${'0'.repeat(29)}b02`; // admin
const E1 = `ps_${'0'.repeat(29)}e01`; // einsicht
const E2 = `ps_${'0'.repeat(29)}e02`; // einsicht
const ARG = { feld: 'GEHEIM-ARG-7Q', id: 'T-100', status: 'erledigt' };
const VOR = 'Vorher: offen, Nachher: erledigt VORSCHAU-5K';
const nonce = (n) => n.toString(16).padStart(32, '0');

const E_BASIS = {
  fassung: 2, klassen: ['oeffentlich', 'intern', 'kundendaten'], vorgabestufe: 'kundendaten',
  routen: [
    { schluessel: 'cloud-beispiel', art: 'cloud', klasse: 'intern', netzziele: [
      { host: 'api.beispiel.invalid', port: 443, zugriff: 'kopf-einsetzen', schluessel: true, pfade: [{ methode: 'GET', pfad: '/modelle/*' }] }] },
    { schluessel: 'lokal-beispiel', art: 'lokal', klasse: 'kundendaten', ziel: 'http://127.0.0.1:11434/v1', modell: 'qwen3:8b', netzziele: [] },
  ],
  kunden: [{ schluessel: 'k-beispiel', erlaubt: ['lokal-beispiel'], verboten: [] }],
  kundendaten_an_cloud: { vorgabe: 'nie', freigaben: [] },
  mcp: {
    tickets: { lesen: ['get'], schreiben: ['update'], freigabe: true, frist_ms: 60000, einloesefrist_ms: 30000 },
    notizen: { lesen: [], schreiben: ['anlegen'], freigabe: false },
    brain: { lesen: ['brain_suche'], schreiben: ['brain_schreiben', 'brain_senken'], zweite_person: ['brain_senken'], frist_ms: 60000, einloesefrist_ms: 30000 },
  },
  stelle: {},
};
// B09: Rollen der Firmendatei (Regelebenen); die Huelle uebergibt sie an laden.
const ROLLEN_BASIS = { [A1]: ['admin'], [A2]: ['admin'], [E1]: ['einsicht'], [E2]: ['einsicht'] };
const einst = (stelle = {}, mcp = null) => JSON.stringify({ ...E_BASIS, ...(mcp ? { mcp } : {}), stelle: { ...E_BASIS.stelle, ...stelle } });
const S_CLOUD = { fassung: 1, sitzung: 's-1', harness: 'claude', modell: 'claude-sonnet-5-5', route: 'cloud-beispiel', kunde: 'k-beispiel', arbeitsordner: '/srv/beispiel/projekt' };
const startE = (obj) => ({ start: { text: JSON.stringify(obj) } });
const anfr = (o = {}) => ({ argumente: ARG, frist_ms: 60000, person: P1, server: 'tickets', sitzung: 's-1', vorschau: VOR, werkzeug: 'update', ...o });
const ent = (anfrage, entscheidung, person = H1, o = {}) => {
  const a = { server: 'tickets', werkzeug: 'update', argumente: ARG, vorschau: VOR, ...o };
  return { anfrage, entscheidung, nachweis: nachweisFuer(person, anfrage, digestVon(a.server, a.werkzeug, a.argumente), entscheidung, a.vorschau) };
};
const loes = (anfrage, o = {}) => ({ anfrage, argumente: ARG, server: 'tickets', werkzeug: 'update', ...o });
const BR = { server: 'brain', werkzeug: 'brain_senken', argumente: { einheit: 'GEHEIM-ARG-7Q', stufe: 'intern' }, vorschau: 'Klasse kundendaten nach intern VORSCHAU-5K' };
const einsicht = (person, n, o = {}, schluessel = SCHLUESSEL_1) => {
  const i = { monat: '2026-11', nonce: nonce(n), quelle: 'stelle', zweck: null, ...o };
  return { ...i, nachweis: signiere(person, H_EINSICHT(i), schluessel) };
};
const pflicht = (person, n, pf, wert) => { const i = { nonce: nonce(n), pflicht: pf, wert }; return { ...i, nachweis: signiere(person, H_PFLICHT(i)) }; };
const bestaetige = (person, antrag, inhalt) => ({ antrag, nachweis: signiere(person, H_BESTAETIGEN(antrag, sha(jcs(inhalt)))) });
const gewaehre = (person, n, frist) => { const i = { frist_ms: frist, nonce: nonce(n) }; return { ...i, nachweis: signiere(person, H_GEWAEHREN(i)) }; };
const beende = (person, zugang) => ({ nachweis: signiere(person, H_BEENDEN(zugang)), zugang });
const PI = (pf, wert, antragsteller) => ({ antragsteller, art: 'pflicht', pflicht: pf, wert });
const EI = (antragsteller, zweck, o = {}) => ({ antragsteller, art: 'einsicht', monat: '2026-11', quelle: 'stelle', zweck, ...o });

const st = (aufrufer, vorgang, t, eingabe, absicht) => ({ aufrufer, vorgang, t, eingabe, absicht });
const START1 = () => st('kern', 'sitzung.starten', 0, startE(S_CLOUD), { ok: true, ergebnis: 'gestartet', zeilen: ['sitzung.gestartet:gestartet'] });
const ABG = (code, vor = []) => ({ ok: false, code, zeilen: [...vor, 'vorgang.abgelehnt:abgelehnt'] });
const STILL = (code) => ({ ok: false, code, zeilen: [] }); // abgelehnt, Zeile unterdrueckt (Flut)
const ZU = (ergebnis) => `mcp.schreibaufruf:${ergebnis}`;
const OFFEN = (id) => ({ ok: true, ergebnis: 'offen', anfrage: id, zeilen: [] });
const ERTEILT = { ok: true, ergebnis: 'erteilt', zeilen: [] };
const ERSTE = { ok: true, ergebnis: 'erste_freigabe', zeilen: [] };

const FAELLE = [];
const fall = (name, regeln, beschreibung, einstellungen, schritte, endeAbsicht, extra = {}) => FAELLE.push({ name, regeln, beschreibung, einstellungen, schritte, ende: endeAbsicht, ...extra });
const PF = (o = {}) => { const x = { vier_augen: false, senken_zweite_person: false, halter_bescheinigung: true, 'mcp.tickets.freigabe': true, 'mcp.notizen.freigabe': false, 'mcp.brain.freigabe': true, ...o }; return Object.fromEntries(Object.keys(x).sort().map((k) => [k, x[k]])); };
const E0 = (o) => ({ sitzungen: ['s-1'], anfragen: {}, zaehler: 0, antraege: {}, zugaenge: {}, pflichten: PF(), ...o });

// ----- 1. Herstellerzugang -----
fall('hersteller-kein-zugang', ['A11', 'Issue 24', 'Issue 12'], 'Vorgabe ist kein Zugang: Auch ein Admin mit gueltigem Nachweis kann keinen Zugang gewaehren; ein Zugriff des Herstellers findet keinen Zugang. Beides steht in der Kette.',
  einst(), [START1(),
    st('mensch', 'hersteller.gewaehren', 1000, gewaehre(A1, 1, 3600000), ABG('stelle.hersteller_aus')),
    st('hersteller', 'hersteller.zugriff', 1001, { zugang: hz(1) }, ABG('stelle.zugang_unbekannt'))],
  E0({}));
fall('hersteller-frist-grenzen', ['A11', 'Issue 24'], 'Befristet: Frist 0 wird abgelehnt, ebenso eine Frist ueber der Hoechstfrist und eine Frist null (dauerhaft ist nicht eingestellt); die Hoechstfrist selbst gilt.',
  einst({ hersteller: { fall: 'befristet', hoechstfrist_ms: 3600000 } }), [START1(),
    st('mensch', 'hersteller.gewaehren', 1000, gewaehre(A1, 1, 0), ABG('stelle.frist_ungueltig')),
    st('mensch', 'hersteller.gewaehren', 1001, gewaehre(A1, 2, 3600001), ABG('stelle.frist_ungueltig')),
    st('mensch', 'hersteller.gewaehren', 1002, gewaehre(A1, 3, null), ABG('stelle.frist_ungueltig')),
    st('mensch', 'hersteller.gewaehren', 1003, gewaehre(A1, 4, 3600000), { ok: true, ergebnis: 'gewaehrt', zugang: hz(1), bis_ms: T0 + 3601003, zeilen: ['hersteller.zugang:gewaehrt'] })],
  E0({ zugaenge: { [hz(1)]: 'aktiv' } }));
fall('hersteller-befristet-endet-zur-frist', ['A11', 'A102', 'Issue 24'], 'Ein befristeter Zugang erlaubt Zugriffe bis eine Millisekunde vor der Frist; jeder Zugriff steht in der Kette; zur Frist endet er mit einer Zeile abgelaufen, und der naechste Zugriff wird abgelehnt.',
  einst({ hersteller: { fall: 'befristet', hoechstfrist_ms: 3600000 } }), [START1(),
    st('mensch', 'hersteller.gewaehren', 1000, gewaehre(A1, 1, 60000), { ok: true, ergebnis: 'gewaehrt', zugang: hz(1), bis_ms: T0 + 61000, zeilen: ['hersteller.zugang:gewaehrt'] }),
    st('hersteller', 'hersteller.zugriff', 2000, { zugang: hz(1) }, { ok: true, ergebnis: 'erlaubt', zeilen: ['hersteller.zugriff:erlaubt'] }),
    st('hersteller', 'hersteller.zugriff', 60999, { zugang: hz(1) }, { ok: true, ergebnis: 'erlaubt', zeilen: ['hersteller.zugriff:erlaubt'] }),
    st('hersteller', 'hersteller.zugriff', 61000, { zugang: hz(1) }, ABG('stelle.zugang_abgelaufen', ['hersteller.zugang:abgelaufen'])),
    st('mensch', 'hersteller.gewaehren', 62000, gewaehre(A2, 2, 1000), { ok: true, ergebnis: 'gewaehrt', zugang: hz(2), zeilen: ['hersteller.zugang:gewaehrt'] }),
    st('takt', 'ablauf.pruefen', 63000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: ['hersteller.zugang:abgelaufen'] })],
  E0({ zugaenge: { [hz(1)]: 'abgelaufen', [hz(2)]: 'abgelaufen' } }));
fall('hersteller-dauerhaft-und-beenden', ['A11', 'Issue 24'], 'Dauerhaft ist der Grenzfall ohne Frist: Der Zugang gilt auch nach Tagen, bis ein Admin ihn beendet; danach wird jeder Zugriff abgelehnt. Eine Frist ist auch im Fall dauerhaft erlaubt.',
  einst({ hersteller: { fall: 'dauerhaft' } }), [START1(),
    st('mensch', 'hersteller.gewaehren', 1000, gewaehre(A1, 1, null), { ok: true, ergebnis: 'gewaehrt', zugang: hz(1), bis_ms: null, zeilen: ['hersteller.zugang:gewaehrt'] }),
    st('hersteller', 'hersteller.zugriff', 864000000, { zugang: hz(1) }, { ok: true, ergebnis: 'erlaubt', zeilen: ['hersteller.zugriff:erlaubt'] }),
    st('mensch', 'hersteller.beenden', 864000001, beende(A2, hz(1)), { ok: true, ergebnis: 'beendet', zeilen: ['hersteller.zugang:beendet'] }),
    st('hersteller', 'hersteller.zugriff', 864000002, { zugang: hz(1) }, ABG('stelle.zugang_beendet')),
    st('mensch', 'hersteller.beenden', 864000003, beende(A2, hz(1)), ABG('stelle.zugang_beendet')),
    st('mensch', 'hersteller.gewaehren', 864000004, gewaehre(A1, 2, 5000), { ok: true, ergebnis: 'gewaehrt', zugang: hz(2), zeilen: ['hersteller.zugang:gewaehrt'] })],
  E0({ zugaenge: { [hz(1)]: 'beendet', [hz(2)]: 'aktiv' } }));
fall('hersteller-gruende', ['A11', 'A92', 'Prueffolge'], 'Herstellerzugang: falsche Aufrufer, fehlende Rolle, falscher Nachweis, benutzte Nonce, ein zweiter Zugang neben einem aktiven und Beenden ohne Rolle werden abgelehnt und protokolliert.',
  einst({ hersteller: { fall: 'befristet' } }), [START1(),
    ...['agent', 'hub', 'hersteller', 'unbekannt'].map((a, n) => st(a, 'hersteller.gewaehren', 1000 + n, gewaehre(A1, 1, 60000), ABG('stelle.aufrufer_nicht_erlaubt'))),
    st('mensch', 'hersteller.gewaehren', 1100, gewaehre(E1, 2, 60000), ABG('stelle.rolle_fehlt')),
    st('mensch', 'hersteller.gewaehren', 1101, { ...gewaehre(A1, 3, 60000), frist_ms: 50000 }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'hersteller.gewaehren', 1102, gewaehre(A1, 2, 60000), ABG('stelle.nonce_benutzt')),
    st('mensch', 'hersteller.gewaehren', 1103, gewaehre(A1, 4, 60000), { ok: true, ergebnis: 'gewaehrt', zugang: hz(1), zeilen: ['hersteller.zugang:gewaehrt'] }),
    st('mensch', 'hersteller.gewaehren', 1104, gewaehre(A2, 5, 60000), ABG('stelle.zugang_aktiv')),
    ...['mensch', 'agent', 'hub'].map((a, n) => st(a, 'hersteller.zugriff', 1200 + n, { zugang: hz(1) }, ABG('stelle.aufrufer_nicht_erlaubt'))),
    st('hersteller', 'hersteller.zugriff', 1300, { zugang: hz(1), befehl: 'x' }, ABG('stelle.eingabe_ungueltig')),
    st('mensch', 'hersteller.beenden', 1400, beende(E1, hz(1)), ABG('stelle.rolle_fehlt'))],
  E0({ zugaenge: { [hz(1)]: 'aktiv' } }));

// ----- 2. Vier Augen fuer Schreibaufrufe -----
fall('vier-augen-schreibaufruf', ['A105', 'A102', 'Issue 24'], 'Vier Augen an: Die erste Freigabe allein reicht nicht; Einloesen und eine zweite Freigabe derselben Person werden abgelehnt und protokolliert; nach der zweiten Person loest der Hub ein, die Zeile nennt beide.',
  einst({ vier_augen: true }), [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), OFFEN(mf(1))),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen', H1), ERSTE),
    st('hub', 'mcp.einloesen', 2500, loes(mf(1)), ABG('stelle.zweite_person_fehlt')),
    st('mensch', 'mcp.entscheiden', 3000, ent(mf(1), 'erteilen', H1), ABG('stelle.selbe_person')),
    st('hub', 'mcp.stand', 3500, { anfrage: mf(1) }, { ok: true, ergebnis: 'stand', zustand: 'erste_freigabe', frist_bis_ms: T0 + 61000, entscheidend: null, mitentscheidend: H1, zeilen: [] }),
    st('mensch', 'mcp.offene', 3600, {}, { ok: true, ergebnis: 'offene', offene_ids: [mf(1)], offene_erste: [H1], offene_person: [P1], zeilen: [] }),
    st('mensch', 'mcp.entscheiden', 4000, ent(mf(1), 'erteilen', H2), { ...ERTEILT, einloesen_bis_ms: T0 + 34000 }),
    st('hub', 'mcp.einloesen', 5000, loes(mf(1)), { ok: true, ergebnis: 'eingeloest', entscheidend: H2, mitentscheidend: H1, zweite_person: false, zeilen: [ZU('eingeloest')] })],
  E0({ anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1, pflichten: PF({ vier_augen: true }) }));
fall('vier-augen-ablehnen-und-ablauf', ['A105', 'A91'], 'Vier Augen an: Eine Person allein darf ablehnen, auch nach der ersten Freigabe einer anderen; eine Anfrage, die nur eine Freigabe bekam, verfaellt mit einer Zeile, die die erste Person nennt und keinen Entscheidenden.',
  einst({ vier_augen: true }), [START1(),
    st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), OFFEN(mf(1))), st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 } }), OFFEN(mf(2))),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: { n: 3 } }), OFFEN(mf(3))),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'ablehnen', H1, { argumente: { n: 1 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('mensch', 'mcp.entscheiden', 2001, ent(mf(2), 'erteilen', H1, { argumente: { n: 2 } }), ERSTE),
    st('mensch', 'mcp.entscheiden', 2002, ent(mf(2), 'ablehnen', H2, { argumente: { n: 2 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('mensch', 'mcp.entscheiden', 2003, ent(mf(3), 'erteilen', H2, { argumente: { n: 3 } }), ERSTE),
    st('takt', 'ablauf.pruefen', 61002, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: [ZU('abgelaufen')] })],
  E0({ anfragen: { [mf(1)]: 'abgelehnt', [mf(2)]: 'abgelehnt', [mf(3)]: 'abgelaufen' }, zaehler: 3, pflichten: PF({ vier_augen: true }) }));

// ----- 2b. Zweite Person je Werkzeug (Nachtrag 1 und 2, Vault E6) -----
const BRN = (o = {}) => anfr({ server: 'brain', werkzeug: 'brain_senken', argumente: BR.argumente, vorschau: BR.vorschau, ...o });
const entB = (anfrage, entscheidung, person, o = {}) => ent(anfrage, entscheidung, person, { server: 'brain', werkzeug: 'brain_senken', argumente: BR.argumente, vorschau: BR.vorschau, ...o });
const loesB = (anfrage, o = {}) => loes(anfrage, { server: 'brain', werkzeug: 'brain_senken', argumente: BR.argumente, ...o });
fall('zweite-person-senken', ['Nachtrag 1', 'Vault E6', 'A105'], 'Option senken_zweite_person an: brain_senken braucht die Freigabe einer anderen Person als der Person der Anfrage. Dieselbe Person erteilt: stelle.selbe_person, die Anfrage bleibt offen; eine zweite Person erteilt; stand und einloesen nennen den Entscheidenden.',
  einst({ senken_zweite_person: true }), [START1(), st('brain', 'mcp.anfragen', 1000, BRN(), OFFEN(mf(1))),
    st('mensch', 'mcp.offene', 1500, {}, { ok: true, ergebnis: 'offene', offene_ids: [mf(1)], offene_erste: [null], offene_person: [P1], zeilen: [] }),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', P1), ABG('stelle.selbe_person')),
    st('brain', 'mcp.stand', 2500, { anfrage: mf(1) }, { ok: true, ergebnis: 'stand', zustand: 'offen', entscheidend: null, zeilen: [] }),
    st('mensch', 'mcp.entscheiden', 3000, entB(mf(1), 'erteilen', H1), ERTEILT),
    st('brain', 'mcp.stand', 3500, { anfrage: mf(1) }, { ok: true, ergebnis: 'stand', zustand: 'erteilt', entscheidend: H1, zeilen: [] }),
    st('brain', 'mcp.einloesen', 4000, loesB(mf(1)), { ok: true, ergebnis: 'eingeloest', entscheidend: H1, zeilen: [ZU('eingeloest')] })],
  E0({ anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1, pflichten: PF({ senken_zweite_person: true }) }));
fall('zweite-person-ablehnen-selbst', ['Nachtrag 1', 'Vault E6'], 'Mit der Pflicht zur zweiten Person darf dieselbe Person ablehnen; die Zeile nennt sie als Entscheidende.',
  einst({ senken_zweite_person: true }), [START1(), st('brain', 'mcp.anfragen', 1000, BRN(), OFFEN(mf(1))),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'ablehnen', P1), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('brain', 'mcp.stand', 2500, { anfrage: mf(1) }, { ok: true, ergebnis: 'stand', zustand: 'abgelehnt', entscheidend: P1, zeilen: [] })],
  E0({ anfragen: { [mf(1)]: 'abgelehnt' }, zaehler: 1, pflichten: PF({ senken_zweite_person: true }) }));
fall('zweite-person-aus-einstellungen', ['Nachtrag 1', 'fail closed'], 'Die Pflicht kommt aus den Einstellungen, nicht aus der Anfrage: ohne Feld zweite_person gilt sie, ein Feld false schaltet sie nicht aus; brain_schreiben steht nicht in der Liste und braucht keine zweite Person.',
  einst({ senken_zweite_person: true }), [START1(),
    st('brain', 'mcp.anfragen', 1000, BRN({ argumente: { n: 1 } }), OFFEN(mf(1))),
    st('brain', 'mcp.anfragen', 1001, BRN({ argumente: { n: 2 }, zweite_person: false }), OFFEN(mf(2))),
    st('brain', 'mcp.anfragen', 1002, BRN({ werkzeug: 'brain_schreiben', argumente: { n: 3 } }), OFFEN(mf(3))),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', P1, { argumente: { n: 1 } }), ABG('stelle.selbe_person')),
    st('mensch', 'mcp.entscheiden', 2001, entB(mf(2), 'erteilen', P1, { argumente: { n: 2 } }), ABG('stelle.selbe_person')),
    st('mensch', 'mcp.entscheiden', 2002, entB(mf(3), 'erteilen', P1, { werkzeug: 'brain_schreiben', argumente: { n: 3 } }), ERTEILT)],
  E0({ anfragen: { [mf(1)]: 'offen', [mf(2)]: 'offen', [mf(3)]: 'erteilt' }, zaehler: 3, pflichten: PF({ senken_zweite_person: true }) }));
fall('zweite-person-feld-bei-option-aus', ['Nachtrag 1', 'fail closed'], 'Option aus: brain_senken braucht keine zweite Person. Ein Feld zweite_person true schaltet die Pflicht fuer diese Anfrage ein, auch auf einem Server mit Freigabe aus; dann gilt die Freigabe.',
  einst(), [START1(),
    st('brain', 'mcp.anfragen', 1000, BRN({ argumente: { n: 1 } }), OFFEN(mf(1))),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', P1, { argumente: { n: 1 } }), ERTEILT),
    st('brain', 'mcp.anfragen', 3000, BRN({ argumente: { n: 2 }, zweite_person: true }), OFFEN(mf(2))),
    st('mensch', 'mcp.entscheiden', 4000, entB(mf(2), 'erteilen', P1, { argumente: { n: 2 } }), ABG('stelle.selbe_person')),
    st('hub', 'mcp.anfragen', 5000, anfr({ server: 'notizen', werkzeug: 'anlegen', zweite_person: true }), OFFEN(mf(3))),
    st('mensch', 'mcp.offene', 5500, {}, { ok: true, ergebnis: 'offene', offene_ids: [mf(2), mf(3)], zeilen: [] })],
  E0({ anfragen: { [mf(1)]: 'erteilt', [mf(2)]: 'offen', [mf(3)]: 'offen' }, zaehler: 3 }));

// ----- 2b2. Mensch gegen Modell aus dem Kanal (Nachtrag 6, 7, 8) -----
const MCP_BRAIN_AUS = { ...E_BASIS.mcp, brain: { ...E_BASIS.mcp.brain, freigabe: false } };
const handeln = (person, n, o = {}) => {
  const i = BRN({ person, ...o });
  const digest = digestVon(i.server, i.werkzeug, i.argumente);
  return { ...i, nonce: nonce(n), nachweis: signiere(person, H_HANDELN(digest, nonce(n), i.sitzung)) };
};
const OHNE = (id, entscheidend) => ({ ok: true, ergebnis: 'ohne_freigabe', anfrage: id, entscheidend, zweite_person: false, zeilen: [ZU('ohne_freigabe')] });
fall('mensch-senkt-pflicht-aus', ['Nachtrag 7', 'Vault E7', 'A91'], 'brain_mensch senkt mit gueltigem Nachweis, Pflicht zur zweiten Person aus: ohne Freigabe, auch bei Freigabe des Servers an; entscheidend ist der Mensch aus dem Nachweis; ohne Sitzung erlaubt.',
  einst(), [START1(),
    st('brain_mensch', 'mcp.anfragen', 1000, handeln(H1, 1), OHNE(mf(1), H1)),
    st('brain_mensch', 'mcp.anfragen', 1001, handeln(H1, 2, { sitzung: null, argumente: { n: 2 } }), OHNE(mf(2), H1))],
  E0({ zaehler: 2 }));
fall('mensch-senkt-pflicht-an', ['Nachtrag 7', 'Vault E7', 'A105'], 'Pflicht an: brain_mensch senkt, die Anfrage ist offen mit zweite_person true; dieselbe Person erteilt nicht (stelle.selbe_person), eine andere schon; einloesen und stand nennen beide Personen fuer das Tor.',
  einst({ senken_zweite_person: true }), [START1(),
    st('brain_mensch', 'mcp.anfragen', 1000, handeln(H1, 1), { ...OFFEN(mf(1)), zweite_person: true, entscheidend: null }),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', H1), ABG('stelle.selbe_person')),
    st('mensch', 'mcp.entscheiden', 3000, entB(mf(1), 'erteilen', H2), ERTEILT),
    st('brain', 'mcp.stand', 3500, { anfrage: mf(1) }, ABG('stelle.anfrage_unbekannt')),
    st('brain_mensch', 'mcp.stand', 3600, { anfrage: mf(1) }, { ok: true, ergebnis: 'stand', zustand: 'erteilt', entscheidend: H2, zweite_person: true, zeilen: [] }),
    st('brain_mensch', 'mcp.einloesen', 4000, loesB(mf(1)), { ok: true, ergebnis: 'eingeloest', entscheidend: H2, mitentscheidend: null, zweite_person: true, zeilen: [ZU('eingeloest')] })],
  E0({ anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1, pflichten: PF({ senken_zweite_person: true }) }));
fall('modell-senkt-immer-offen', ['Nachtrag 6', 'A38'], 'Modellweg (Art brain, ohne Nachweis): brain_senken ist immer offen, auch bei Freigabe des Servers aus und Pflicht aus; brain_schreiben folgt der Freigabe und laeuft ohne Freigabe mit entscheidend null.',
  einst({}, MCP_BRAIN_AUS), [START1(),
    st('brain', 'mcp.anfragen', 1000, BRN(), { ...OFFEN(mf(1)), zweite_person: false, entscheidend: null }),
    st('brain', 'mcp.anfragen', 1001, BRN({ werkzeug: 'brain_schreiben' }), OHNE(mf(2), null))],
  E0({ anfragen: { [mf(1)]: 'offen' }, zaehler: 2, pflichten: PF({ 'mcp.brain.freigabe': false }) }));
const MCP_BRAIN_LEER = { ...E_BASIS.mcp, brain: { lesen: ['brain_suche'], schreiben: ['brain_schreiben'], freigabe: false } };
fall('modell-senkt-feste-regel', ['Nachtrag 10', 'A38'], 'Feste Regel: Art brain, Server brain, Werkzeug brain_senken ist immer offen, auch wenn brain_senken weder in zweite_person noch in schreiben steht und die Freigabe des Servers aus ist; ohne Pflicht zur zweiten Person darf die Person der Anfrage selbst erteilen.',
  einst({}, MCP_BRAIN_LEER), [START1(),
    st('brain', 'mcp.anfragen', 1000, BRN(), { ...OFFEN(mf(1)), zweite_person: false, entscheidend: null }),
    st('brain', 'mcp.anfragen', 1001, BRN({ werkzeug: 'brain_schreiben' }), OHNE(mf(2), null)),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', P1), ERTEILT)],
  E0({ anfragen: { [mf(1)]: 'erteilt' }, zaehler: 2, pflichten: PF({ 'mcp.brain.freigabe': false }) }));
fall('mensch-und-modell-gruende', ['Nachtrag 7', 'A92'], 'Mensch gegen Modell folgt aus dem Kanal: brain mit Nachweis ist eine ungueltige Eingabe; brain_mensch ohne Nachweis, mit falschem Nachweis, mit fremdem Pseudonym in person oder mit benutzter Nonce wird abgelehnt; brain_mensch fragt nur den Server brain und entscheidet nie.',
  einst(), [START1(),
    st('brain', 'mcp.anfragen', 1000, handeln(H1, 1), ABG('stelle.eingabe_ungueltig')),
    st('brain', 'mcp.anfragen', 1001, BRN({ sitzung: null }), ABG('stelle.eingabe_ungueltig')),
    st('brain_mensch', 'mcp.anfragen', 1002, BRN({ person: H1 }), ABG('stelle.nachweis_ungueltig')),
    st('brain_mensch', 'mcp.anfragen', 1003, { ...handeln(H1, 2), argumente: { anders: 1 } }, ABG('stelle.nachweis_ungueltig')),
    st('brain_mensch', 'mcp.anfragen', 1004, { ...handeln(H1, 3), person: H2 }, ABG('stelle.nachweis_ungueltig')),
    st('brain_mensch', 'mcp.anfragen', 1005, (({ nonce: _n, ...x }) => x)(handeln(H1, 4)), ABG('stelle.eingabe_ungueltig')),
    st('brain_mensch', 'mcp.anfragen', 1006, handeln(H1, 5), OHNE(mf(1), H1)),
    st('brain_mensch', 'mcp.anfragen', 1007, handeln(H1, 5), ABG('stelle.nonce_benutzt')),
    st('brain_mensch', 'mcp.anfragen', 1008, anfr({ person: H1 }), ABG('stelle.server_nicht_erlaubt')),
    st('hub', 'mcp.anfragen', 1009, handeln(H1, 6), ABG('stelle.eingabe_ungueltig')),
    st('brain_mensch', 'mcp.entscheiden', 1010, entB(mf(1), 'erteilen', H1), ABG('stelle.aufrufer_nicht_erlaubt'))],
  E0({ zaehler: 1 }));
fall('verschaerfen-wirkt-sofort', ['Nachtrag 6', 'A09', 'Nachtrag 5'], 'Ein Admin schaltet senken_zweite_person zur Laufzeit ein; die Stelle schreibt die Regeldatei, und die naechste Anfrage des Menschen folgt sofort der neuen Pflicht (offen statt ohne Freigabe). Kein Tor haelt eine Kopie.',
  einst(), [START1(),
    st('brain_mensch', 'mcp.anfragen', 1000, handeln(H1, 1), OHNE(mf(1), H1)),
    st('mensch', 'pflicht.aendern', 2000, pflicht(A1, 2, 'senken_zweite_person', true), { ok: true, ergebnis: 'geaendert', zeilen: ['regel.geaendert:geaendert'], datei: true }),
    st('brain_mensch', 'mcp.anfragen', 3000, handeln(H1, 3, { argumente: { n: 3 } }), { ...OFFEN(mf(2)), zweite_person: true })],
  E0({ anfragen: { [mf(2)]: 'offen' }, zaehler: 2, pflichten: PF({ senken_zweite_person: true }) }));

// ----- 2c. Pflichten aendern (A09) -----
fall('pflicht-schwach-braucht-zwei', ['A09', 'Nachtrag 9'], 'Die Bescheinigungspflicht des Halters (halter_bescheinigung, Vorgabe an) abzuschalten, also schwach zu erlauben, wird bei einer Person ein Antrag mit Zeile; ihre eigene Bestaetigung wird abgelehnt und protokolliert; erst ein zweiter Admin schaltet ab, die Stelle schreibt Zeile und Datei. Wieder einschalten darf ein Admin allein.',
  einst(), [START1(),
    st('mensch', 'pflicht.aendern', 1000, pflicht(A1, 1, 'halter_bescheinigung', false), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), zeilen: ['pflicht.aenderung:antrag'] }),
    st('mensch', 'antrag.bestaetigen', 2000, bestaetige(A1, an(1), PI('halter_bescheinigung', false, A1)), ABG('stelle.selbe_person')),
    st('mensch', 'antrag.bestaetigen', 3000, bestaetige(A2, an(1), PI('halter_bescheinigung', false, A1)), { ok: true, ergebnis: 'geaendert', antrag: an(1), zeilen: ['regel.geaendert:geaendert'], datei: true }),
    st('mensch', 'pflicht.aendern', 4000, pflicht(A2, 2, 'halter_bescheinigung', true), { ok: true, ergebnis: 'geaendert', antrag: null, zeilen: ['regel.geaendert:geaendert'], datei: true })],
  E0({ antraege: { [an(1)]: 'bestaetigt' }, pflichten: PF({ halter_bescheinigung: true }) }));
fall('keine-sammelfreigabe', ['A105 (6)', 'Nachtrag 9'], 'mcp.entscheiden gilt fuer genau eine Anfrage: eine Liste von Kennungen oder ein Zusatzfeld ist eine ungueltige Eingabe, beide Anfragen bleiben offen; einzeln entschieden gelingt es.',
  einst(), [START1(),
    st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), OFFEN(mf(1))), st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 } }), OFFEN(mf(2))),
    st('mensch', 'mcp.entscheiden', 2000, { ...ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), anfrage: [mf(1), mf(2)] }, ABG('stelle.eingabe_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2001, { ...ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), anfragen: [mf(2)] }, ABG('stelle.eingabe_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2002, { ...ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), alle: true }, ABG('stelle.eingabe_ungueltig')),
    st('mensch', 'mcp.offene', 2500, {}, { ok: true, ergebnis: 'offene', offene_ids: [mf(1), mf(2)], zeilen: [] }),
    st('mensch', 'mcp.entscheiden', 3000, ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), ERTEILT),
    st('mensch', 'mcp.entscheiden', 3001, ent(mf(2), 'erteilen', H1, { argumente: { n: 2 } }), ERTEILT)],
  E0({ anfragen: { [mf(1)]: 'erteilt', [mf(2)]: 'erteilt' }, zaehler: 2 }));
fall('pflicht-lockern-braucht-zwei', ['A09', 'Issue 24'], 'Vier Augen abschalten: Der Versuch einer Person wird Antrag (Zeile), ihre eigene Bestaetigung wird abgelehnt (Zeile), erst ein zweiter Admin schaltet ab (Zeile mit beiden). Danach reicht eine Freigabe.',
  einst({ vier_augen: true }), [START1(),
    st('mensch', 'pflicht.aendern', 1000, pflicht(A1, 1, 'vier_augen', false), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), zeilen: ['pflicht.aenderung:antrag'] }),
    st('hub', 'mcp.anfragen', 1500, anfr(), OFFEN(mf(1))),
    st('mensch', 'antrag.bestaetigen', 2000, bestaetige(A1, an(1), PI('vier_augen', false, A1)), ABG('stelle.selbe_person')),
    st('mensch', 'antrag.offene', 2500, {}, { ok: true, ergebnis: 'antraege', zeilen: [] }),
    st('mensch', 'antrag.bestaetigen', 3000, bestaetige(A2, an(1), PI('vier_augen', false, A1)), { ok: true, ergebnis: 'geaendert', antrag: an(1), zeilen: ['regel.geaendert:geaendert'], datei: true }),
    st('mensch', 'antrag.bestaetigen', 3500, bestaetige(A2, an(1), PI('vier_augen', false, A1)), ABG('stelle.bereits_entschieden')),
    st('hub', 'mcp.anfragen', 4000, anfr({ argumente: { n: 2 } }), OFFEN(mf(2))),
    st('mensch', 'mcp.entscheiden', 5000, ent(mf(2), 'erteilen', H1, { argumente: { n: 2 } }), ERTEILT),
    st('mensch', 'mcp.entscheiden', 5001, ent(mf(1), 'erteilen', H1), ERSTE)],
  E0({ anfragen: { [mf(1)]: 'erste_freigabe', [mf(2)]: 'erteilt' }, zaehler: 2, antraege: { [an(1)]: 'bestaetigt' }, pflichten: PF({ vier_augen: false }) }));
fall('pflicht-freigabe-je-server', ['A09', 'A90', 'O3'], 'Die Freigabe eines Servers abzuschalten braucht zwei Admins; ein Antrag ohne zweite Person verfaellt mit Zeile. Einschalten (verschaerfen) darf ein Admin allein; danach wartet der Server notizen auf Freigabe.',
  einst(), [START1(),
    st('mensch', 'pflicht.aendern', 1000, pflicht(A1, 1, 'mcp.tickets.freigabe', false), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), frist_bis_ms: T0 + 601000, zeilen: ['pflicht.aenderung:antrag'] }),
    st('mensch', 'pflicht.aendern', 1001, pflicht(A2, 2, 'mcp.tickets.freigabe', false), ABG('stelle.antrag_laeuft')),
    st('mensch', 'pflicht.aendern', 2000, pflicht(A1, 3, 'mcp.notizen.freigabe', true), { ok: true, ergebnis: 'geaendert', antrag: null, zeilen: ['regel.geaendert:geaendert'], datei: true }),
    st('hub', 'mcp.anfragen', 3000, anfr({ server: 'notizen', werkzeug: 'anlegen' }), OFFEN(mf(1))),
    st('takt', 'ablauf.pruefen', 601000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 2, zeilen: [ZU('abgelaufen'), 'pflicht.aenderung:abgelaufen'] }),
    st('mensch', 'antrag.bestaetigen', 601001, bestaetige(A2, an(1), PI('mcp.tickets.freigabe', false, A1)), ABG('stelle.antrag_abgelaufen'))],
  E0({ anfragen: { [mf(1)]: 'abgelaufen' }, zaehler: 1, antraege: { [an(1)]: 'abgelaufen' }, pflichten: PF({ 'mcp.notizen.freigabe': true }) }));
fall('pflicht-gruende', ['A09', 'Prueffolge'], 'pflicht.aendern und antrag.bestaetigen: unbekannte Pflicht, falscher Nachweis, benutzte Nonce, fehlende Rolle, unveraenderter Wert, fremder Inhalt beim Bestaetigen, unbekannter Antrag und falsche Aufrufer werden abgelehnt und protokolliert.',
  einst({ vier_augen: true }), [START1(),
    st('mensch', 'pflicht.aendern', 1000, pflicht(A1, 1, 'mcp.fremd.freigabe', false), ABG('stelle.pflicht_unbekannt')),
    st('mensch', 'pflicht.aendern', 1001, { ...pflicht(A1, 2, 'vier_augen', false), wert: true }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'pflicht.aendern', 1002, pflicht(E1, 3, 'vier_augen', false), ABG('stelle.rolle_fehlt')),
    st('mensch', 'pflicht.aendern', 1003, pflicht(A1, 3, 'vier_augen', false), ABG('stelle.nonce_benutzt')),
    st('mensch', 'pflicht.aendern', 1004, pflicht(A1, 4, 'vier_augen', true), ABG('stelle.pflicht_unveraendert')),
    st('mensch', 'pflicht.aendern', 1005, pflicht(A1, 5, 'vier_augen', false), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), zeilen: ['pflicht.aenderung:antrag'] }),
    st('mensch', 'antrag.bestaetigen', 1006, bestaetige(A2, an(1), PI('vier_augen', true, A1)), ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'antrag.bestaetigen', 1007, bestaetige(E1, an(1), PI('vier_augen', false, A1)), ABG('stelle.rolle_fehlt')),
    st('mensch', 'antrag.bestaetigen', 1008, bestaetige(A2, an(9), PI('vier_augen', false, A1)), ABG('stelle.antrag_unbekannt')),
    ...['hub', 'agent', 'brain', 'hersteller'].map((a, n) => st(a, 'pflicht.aendern', 1100 + n, pflicht(A1, 6, 'vier_augen', false), ABG('stelle.aufrufer_nicht_erlaubt'))),
    ...['hub', 'agent'].map((a, n) => st(a, 'antrag.bestaetigen', 1200 + n, bestaetige(A2, an(1), PI('vier_augen', false, A1)), ABG('stelle.aufrufer_nicht_erlaubt')))],
  E0({ antraege: { [an(1)]: 'offen' }, pflichten: PF({ vier_augen: true }) }));
fall('pflicht-ueberlebt-neustart', ['A09', 'E1', 'Nachtrag 5'], 'Die Stelle schreibt eine wirksame Pflichtaenderung selbst in die Datei der Ebene. Ein Neustart mit dieser Datei und neu der letzten Zeile regel.geaendert laedt sie; die Lockerung gilt weiter.',
  einst({ vier_augen: true }), [START1(),
    st('mensch', 'pflicht.aendern', 1000, pflicht(A1, 1, 'vier_augen', false), { ok: true, ergebnis: 'antrag_offen', zeilen: ['pflicht.aenderung:antrag'] }),
    st('mensch', 'antrag.bestaetigen', 2000, bestaetige(A2, an(1), PI('vier_augen', false, A1)), { ok: true, ergebnis: 'geaendert', zeilen: ['regel.geaendert:geaendert'], datei: true }),
    { neustart: { laden_ms: LADEN2, geschrieben: true } },
    st('kern', 'sitzung.starten', 3000, startE(S_CLOUD), { ok: true, ergebnis: 'gestartet', zeilen: ['sitzung.gestartet:gestartet'] }),
    st('hub', 'mcp.anfragen', 3001, anfr(), OFFEN(mf(1, LADEN2))),
    st('mensch', 'mcp.entscheiden', 3002, ent(mf(1, LADEN2), 'erteilen', H1), ERTEILT)],
  { sitzungen: ['s-1'], anfragen: { [mf(1, LADEN2)]: 'erteilt' }, zaehler: 1, antraege: {}, zugaenge: {}, pflichten: PF({ vier_augen: false }) });

// ----- 3. Einsicht ins Protokoll -----
fall('einsicht-mit-rolle', ['A102', 'A103'], 'Einsichtsregel rolle (Vorgabe): Einsicht mit Rolle einsicht gelingt ohne zweite Person und ohne Zweck, die Einsicht steht in der Kette; ohne Rolle (auch als Admin) wird sie abgelehnt und protokolliert; ein Zweck ausserhalb der Liste wird abgelehnt, einer aus der Liste steht in der Zeile.',
  einst(), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1), { ok: true, ergebnis: 'einsicht_erlaubt', antrag: null, zeilen: ['protokoll.einsicht:erlaubt'] }),
    st('mensch', 'einsicht.anfragen', 1001, einsicht(H1, 2), ABG('stelle.rolle_fehlt')),
    st('mensch', 'einsicht.anfragen', 1002, einsicht(A1, 3), ABG('stelle.rolle_fehlt')),
    st('mensch', 'einsicht.anfragen', 1003, einsicht(E1, 4, { zweck: 'neugier' }), ABG('stelle.zweck_ungueltig')),
    st('mensch', 'einsicht.anfragen', 1004, einsicht(E1, 5, { zweck: 'vorfall', quelle: 'gateway', monat: '2026-10' }), { ok: true, ergebnis: 'einsicht_erlaubt', zeilen: ['protokoll.einsicht:erlaubt'] }),
    st('mensch', 'einsicht.anfragen', 1005, einsicht(E1, 5), ABG('stelle.nonce_benutzt')),
    st('mensch', 'einsicht.anfragen', 1006, einsicht(E1, 6, {}, SCHLUESSEL_2), ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'einsicht.anfragen', 1007, einsicht(E1, 7, { quelle: 'fremd' }), ABG('stelle.eingabe_ungueltig')),
    ...['hub', 'agent', 'brain', 'hersteller', 'takt'].map((a, n) => st(a, 'einsicht.anfragen', 1100 + n, einsicht(E1, 8), ABG('stelle.aufrufer_nicht_erlaubt')))],
  E0({}));
fall('einsicht-vier-augen', ['A102', 'A103', 'B08'], 'Einsichtsregel rolle-vier-augen: ohne Zweck abgelehnt; mit Zweck entsteht ein Antrag; dieselbe Person und eine Person ohne Rolle bestaetigen nicht; ein Admin bestaetigt, die Zeile nennt beide und den Zweck; ein zweites Bestaetigen scheitert.',
  einst({ einsicht: { regel: 'rolle-vier-augen' } }), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1), ABG('stelle.zweck_ungueltig')),
    st('mensch', 'einsicht.anfragen', 1001, einsicht(E1, 2, { zweck: 'pruefung' }), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), frist_bis_ms: T0 + 601001, zeilen: [] }),
    st('mensch', 'antrag.offene', 1500, {}, { ok: true, ergebnis: 'antraege', zeilen: [] }),
    st('mensch', 'antrag.bestaetigen', 2000, bestaetige(E1, an(1), EI(E1, 'pruefung')), ABG('stelle.selbe_person')),
    st('mensch', 'antrag.bestaetigen', 2001, bestaetige(H1, an(1), EI(E1, 'pruefung')), ABG('stelle.rolle_fehlt')),
    st('mensch', 'antrag.bestaetigen', 2002, bestaetige(A1, an(1), EI(E1, 'vorfall')), ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'antrag.bestaetigen', 3000, bestaetige(A1, an(1), EI(E1, 'pruefung')), { ok: true, ergebnis: 'bestaetigt', antrag: an(1), abholen_bis_ms: T0 + 603000, zeilen: ['protokoll.einsicht:erlaubt'] }),
    st('mensch', 'antrag.bestaetigen', 3001, bestaetige(E2, an(1), EI(E1, 'pruefung')), ABG('stelle.bereits_entschieden')),
    st('mensch', 'einsicht.anfragen', 4000, einsicht(E2, 3, { zweck: 'auskunft', quelle: 'brain' }), { ok: true, ergebnis: 'antrag_offen', antrag: an(2), zeilen: [] }),
    st('mensch', 'antrag.bestaetigen', 4001, bestaetige(E1, an(2), EI(E2, 'auskunft', { quelle: 'brain' })), { ok: true, ergebnis: 'bestaetigt', zeilen: ['protokoll.einsicht:erlaubt'] })],
  E0({ antraege: { [an(1)]: 'bestaetigt', [an(2)]: 'bestaetigt' }, pflichten: PF() }));
const holen = (person, antrag) => ({ antrag, nachweis: signiere(person, H_ERGEBNIS(antrag)) });
fall('einsicht-abholen', ['A102', 'A103', 'Nachtrag 3'], 'Einsichtsregel rolle-vier-augen, Vier Augen an: Der Antragsteller holt das Ergebnis der Einsicht selbst ab, genau einmal. Abholen vor der zweiten Freigabe, durch die zweite Person, ein zweites Mal und nach der Abholfrist wird abgelehnt und protokolliert; die zweite Person sieht nie die Zeilen.',
  einst({ vier_augen: true, antrag_frist_ms: 10000, einsicht: { regel: 'rolle-vier-augen' } }), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1, { zweck: 'pruefung' }), { ok: true, ergebnis: 'antrag_offen', antrag: an(1), zeilen: [] }),
    st('mensch', 'antrag.offene', 1500, {}, { ok: true, ergebnis: 'antraege', zeilen: [] }),
    st('mensch', 'antrag.ergebnis', 2000, holen(E1, an(1)), ABG('stelle.antrag_nicht_bestaetigt')),
    st('mensch', 'antrag.bestaetigen', 3000, bestaetige(A1, an(1), EI(E1, 'pruefung')), { ok: true, ergebnis: 'bestaetigt', abholen_bis_ms: T0 + 13000, zeilen: ['protokoll.einsicht:erlaubt'] }),
    st('mensch', 'antrag.ergebnis', 3500, holen(A1, an(1)), ABG('stelle.nicht_antragsteller')),
    st('mensch', 'antrag.ergebnis', 3501, { antrag: an(1), nachweis: signiere(E1, H_ERGEBNIS(an(1)), SCHLUESSEL_2) }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'antrag.ergebnis', 4000, holen(E1, an(1)), { ok: true, ergebnis: 'einsicht_erlaubt', antrag: an(1), monat: '2026-11', zeilen: ['protokoll.einsicht:abgeholt'] }),
    st('mensch', 'antrag.ergebnis', 4001, holen(E1, an(1)), ABG('stelle.bereits_abgeholt')),
    st('mensch', 'pflicht.aendern', 4500, pflicht(A1, 7, 'vier_augen', false), { ok: true, ergebnis: 'antrag_offen', antrag: an(2), zeilen: ['pflicht.aenderung:antrag'] }),
    st('mensch', 'antrag.ergebnis', 4501, holen(A1, an(2)), ABG('stelle.antrag_unbekannt')),
    st('mensch', 'einsicht.anfragen', 5000, einsicht(E2, 2, { zweck: 'vorfall' }), { ok: true, ergebnis: 'antrag_offen', antrag: an(3), zeilen: [] }),
    st('mensch', 'antrag.bestaetigen', 5001, bestaetige(E1, an(3), EI(E2, 'vorfall')), { ok: true, ergebnis: 'bestaetigt', abholen_bis_ms: T0 + 15001, zeilen: ['protokoll.einsicht:erlaubt'] }),
    st('mensch', 'antrag.ergebnis', 15001, holen(E2, an(3)), ABG('stelle.antrag_abgelaufen', ['pflicht.aenderung:abgelaufen', 'protokoll.einsicht:verfallen'])),
    ...['hub', 'agent', 'brain'].map((a, n) => st(a, 'antrag.ergebnis', 16000 + n, holen(E1, an(1)), ABG('stelle.aufrufer_nicht_erlaubt')))],
  E0({ antraege: { [an(1)]: 'abgeholt', [an(2)]: 'abgelaufen', [an(3)]: 'verfallen' }, pflichten: PF({ vier_augen: true }) }), { offenePruefung: true });
fall('einsicht-antrag-verfaellt', ['A102'], 'Ein Antrag auf Einsicht ohne zweite Person verfaellt zur Frist mit einer Zeile; danach scheitert das Bestaetigen.',
  einst({ antrag_frist_ms: 10000, einsicht: { regel: 'rolle-vier-augen' } }), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1, { zweck: 'pruefung' }), { ok: true, ergebnis: 'antrag_offen', frist_bis_ms: T0 + 11000, zeilen: [] }),
    st('mensch', 'antrag.bestaetigen', 11000, bestaetige(A1, an(1), EI(E1, 'pruefung')), ABG('stelle.antrag_abgelaufen', ['protokoll.einsicht:abgelaufen']))],
  E0({ antraege: { [an(1)]: 'abgelaufen' }, pflichten: PF() }));
fall('einsicht-niemand', ['A102', 'A103', 'B08'], 'Einsichtsregel niemand: Jede Einsicht wird vor dem Nachweis abgelehnt und protokolliert, auch mit Rolle und Zweck und auch fuer einen Admin.',
  einst({ einsicht: { regel: 'niemand' } }), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1), ABG('stelle.einsicht_aus')),
    st('mensch', 'einsicht.anfragen', 1001, einsicht(E1, 2, { zweck: 'pruefung' }), ABG('stelle.einsicht_aus')),
    st('mensch', 'einsicht.anfragen', 1002, einsicht(A1, 3), ABG('stelle.einsicht_aus'))],
  E0({}));
fall('einsicht-vier-augen-nur-schreiben', ['A102', 'A105', 'B08'], 'vier_augen an und Einsichtsregel rolle: Vier Augen gelten nur fuer Schreibaufrufe; die Einsicht mit Rolle gelingt ohne zweite Person und ohne Zweck.',
  einst({ vier_augen: true }), [START1(),
    st('mensch', 'einsicht.anfragen', 1000, einsicht(E1, 1), { ok: true, ergebnis: 'einsicht_erlaubt', antrag: null, zeilen: ['protokoll.einsicht:erlaubt'] })],
  E0({ pflichten: PF({ vier_augen: true }) }));

// ----- 4. Aufrufer brain -----
fall('brain-nur-server-brain', ['Wunsch Vault', 'A92'], 'Der Such-Prozess (brain) fragt nur den Server brain an, der Hub nie; eine Anfrage von brain ist fuer den Hub unbekannt und umgekehrt; lesend bleibt lesend.',
  einst(), [START1(),
    st('brain', 'mcp.anfragen', 1000, anfr(), ABG('stelle.server_nicht_erlaubt')),
    st('hub', 'mcp.anfragen', 1001, BRN(), ABG('stelle.server_nicht_erlaubt')),
    st('brain', 'mcp.anfragen', 1002, BRN({ werkzeug: 'brain_suche' }), { ok: true, ergebnis: 'lesend', zeilen: [] }),
    st('brain', 'mcp.anfragen', 1003, BRN({ werkzeug: 'brain_schreiben' }), OFFEN(mf(1))),
    st('hub', 'mcp.anfragen', 1004, anfr(), OFFEN(mf(2))),
    st('mensch', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', H1, { werkzeug: 'brain_schreiben' }), ERTEILT),
    st('mensch', 'mcp.entscheiden', 2001, ent(mf(2), 'erteilen', H1), ERTEILT),
    st('hub', 'mcp.stand', 3000, { anfrage: mf(1) }, ABG('stelle.anfrage_unbekannt')),
    st('hub', 'mcp.einloesen', 3001, loesB(mf(1), { werkzeug: 'brain_schreiben' }), ABG('stelle.anfrage_unbekannt')),
    st('brain', 'mcp.einloesen', 3002, loes(mf(2)), ABG('stelle.anfrage_unbekannt')),
    st('brain', 'mcp.einloesen', 3003, loesB(mf(1), { werkzeug: 'brain_schreiben' }), { ok: true, ergebnis: 'eingeloest', entscheidend: H1, zeilen: [ZU('eingeloest')] }),
    st('hub', 'mcp.einloesen', 3004, loes(mf(2)), { ok: true, ergebnis: 'eingeloest', entscheidend: H1, zeilen: [ZU('eingeloest')] })],
  E0({ anfragen: { [mf(1)]: 'eingeloest', [mf(2)]: 'eingeloest' }, zaehler: 2 }));
fall('brain-sonst-nichts', ['Wunsch Vault', 'A92'], 'brain darf ausser anfragen, einloesen und stand nichts: weder entscheiden noch die offene Liste noch starten noch Einsicht.',
  einst(), [START1(), st('brain', 'mcp.anfragen', 1000, BRN({ werkzeug: 'brain_schreiben' }), OFFEN(mf(1))),
    st('brain', 'mcp.entscheiden', 2000, entB(mf(1), 'erteilen', H1, { werkzeug: 'brain_schreiben' }), ABG('stelle.aufrufer_nicht_erlaubt')),
    st('brain', 'mcp.offene', 2001, {}, ABG('stelle.aufrufer_nicht_erlaubt')),
    st('brain', 'sitzung.starten', 2002, startE(S_CLOUD), { ok: false, code: 'stelle.aufrufer_nicht_erlaubt', zeilen: ['sitzung.abgelehnt:abgelehnt'] }),
    st('brain', 'ablauf.pruefen', 2003, {}, ABG('stelle.aufrufer_nicht_erlaubt')),
    st('brain', 'antrag.offene', 2004, {}, ABG('stelle.aufrufer_nicht_erlaubt'))],
  E0({ anfragen: { [mf(1)]: 'offen' }, zaehler: 1 }));

// ----- 5. Flut -----
fall('flut-offene-je-sitzung', ['Flut', 'A90'], 'Hoechstens zwei laufende Anfragen je Sitzung (Einstellung): die dritte wird abgelehnt; eine entschiedene macht Platz; lesend und ohne_freigabe zaehlen nicht.',
  einst({ flut: { offene_je_sitzung: 2 } }), [START1(),
    st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), OFFEN(mf(1))),
    st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 } }), OFFEN(mf(2))),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: { n: 3 } }), ABG('stelle.zu_viele_offen')),
    st('hub', 'mcp.anfragen', 1003, anfr({ werkzeug: 'get' }), { ok: true, ergebnis: 'lesend', zeilen: [] }),
    st('hub', 'mcp.anfragen', 1004, anfr({ server: 'notizen', werkzeug: 'anlegen' }), { ok: true, ergebnis: 'ohne_freigabe', anfrage: mf(3), zeilen: [ZU('ohne_freigabe')] }),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'ablehnen', H1, { argumente: { n: 1 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('hub', 'mcp.anfragen', 2001, anfr({ argumente: { n: 3 } }), OFFEN(mf(4)))],
  E0({ anfragen: { [mf(1)]: 'abgelehnt', [mf(2)]: 'offen', [mf(4)]: 'offen' }, zaehler: 4 }));
fall('flut-ablehnungen-zusammengefasst', ['Flut', 'A92', 'A102'], 'Hoechstens drei Ablehnungszeilen je Aufruferart und Fenster von 10 s: Weitere Ablehnungen des Agenten bekommen dieselbe Antwort, aber keine eigene Zeile; der Hub ist davon nicht betroffen; nach dem Fenster steht je Gruppe eine Sammelzeile mit anzahl.',
  einst({ flut: { zeilen_je_fenster: 3, fenster_ms: 10000 } }), [START1(),
    ...[0, 1, 2].map((n) => st('agent', 'mcp.entscheiden', 1000 + n, ent(mf(1), 'erteilen'), ABG('stelle.aufrufer_nicht_erlaubt'))),
    ...[3, 4].map((n) => st('agent', 'mcp.entscheiden', 1000 + n, ent(mf(1), 'erteilen'), STILL('stelle.aufrufer_nicht_erlaubt'))),
    ...[5, 6, 7].map((n) => st('agent', 'mcp.offene', 1000 + n, {}, STILL('stelle.aufrufer_nicht_erlaubt'))),
    st('agent', 'sitzung.starten', 1008, startE(S_CLOUD), STILL('stelle.aufrufer_nicht_erlaubt')),
    st('hub', 'mcp.stand', 1009, { anfrage: mf(1) }, ABG('stelle.anfrage_unbekannt')),
    st('takt', 'ablauf.pruefen', 9999, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 0, zeilen: [] }),
    st('takt', 'ablauf.pruefen', 10000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 0, zeilen: ['vorgang.gedrosselt:abgelehnt', 'vorgang.gedrosselt:abgelehnt', 'vorgang.gedrosselt:abgelehnt'] }),
    st('agent', 'mcp.offene', 10001, {}, ABG('stelle.aufrufer_nicht_erlaubt')),
    st('takt', 'ablauf.pruefen', 25000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 0, zeilen: [] })],
  E0({}), { flutPruefung: true });

// ----- Laden -----
const LADE = (name, regeln, beschreibung, text, fehler, regelNeu = null, rollen = ROLLEN_BASIS) => FAELLE.push({ name, regeln, beschreibung, laden: { text, fehler, regelNeu, rollen } });
const mit = (stelle, mcp = null) => JSON.stringify({ ...E_BASIS, ...(mcp ? { mcp } : {}), stelle });
LADE('laden-stelle-unbekannter-schluessel', ['V2 F2'], 'Ein unbekannter Schluessel im Block stelle: einst.schluessel_unbekannt.', mit({ vier_augen: true, notfall: true }), 'einst.schluessel_unbekannt');
LADE('laden-stelle-typ-falsch', ['V2 F2', 'L-34'], 'vier_augen als Text ist ein Typfehler und faellt nie auf die Vorgabe: einst.typ_falsch.', mit({ vier_augen: 'ja' }), 'einst.typ_falsch');
LADE('laden-stelle-fall-unbekannt', ['V2 F2', 'Issue 12'], 'Ein Fall des Herstellerzugangs ausserhalb kein, befristet, dauerhaft: einst.stelle_wert.', mit({ hersteller: { fall: 'immer' } }), 'einst.stelle_wert');
LADE('laden-stelle-rolle-unbekannt', ['B09', 'A102'], 'Eine Rolle ausserhalb admin und einsicht in den Rollen der Firmendatei: stelle.rollen_ungueltig.', mit({}), 'stelle.rollen_ungueltig', null, { [A1]: ['root'] });
LADE('laden-stelle-pseudonym-form', ['B09', 'A12'], 'Ein Name statt eines Pseudonyms in den Rollen der Firmendatei: stelle.rollen_ungueltig.', mit({}), 'stelle.rollen_ungueltig', null, { 'max.beispiel': ['admin'] });
LADE('laden-stelle-rollen-drei', ['B09'], 'Drei Rollen fuer ein Pseudonym in den Rollen der Firmendatei: stelle.rollen_ungueltig.', mit({}), 'stelle.rollen_ungueltig', null, { [A1]: ['admin', 'einsicht', 'admin'] });
LADE('laden-stelle-rollen-im-block', ['B09'], 'Rollen im Block stelle: Die Rollen stehen nur in der Firmendatei (einst.schluessel_unbekannt).', mit({ rollen: { [A1]: ['admin'] } }), 'einst.schluessel_unbekannt');
LADE('laden-stelle-einsicht-regel-unbekannt', ['B08', 'A102'], 'Eine Einsichtsregel ausserhalb niemand, rolle und rolle-vier-augen: einst.stelle_wert.', mit({ einsicht: { regel: 'alle' } }), 'einst.stelle_wert');
LADE('laden-stelle-einsicht-regel-typ', ['B08', 'L-34'], 'Eine Einsichtsregel, die kein Text ist, faellt nie auf die Vorgabe: einst.typ_falsch.', mit({ einsicht: { regel: true } }), 'einst.typ_falsch');
LADE('laden-stelle-hoechstfrist', ['V2 F2', 'A11'], 'Eine Hoechstfrist 0 ist kein Weg zu kein Zugang, sondern ein Ladefehler: einst.frist_bereich.', mit({ hersteller: { fall: 'befristet', hoechstfrist_ms: 0 } }), 'einst.frist_bereich');
LADE('laden-stelle-zwecke-leer', ['V2 F2', 'A102'], 'Eine leere Liste der Zwecke: einst.anzahl.', mit({ einsicht: { zwecke: [] } }), 'einst.anzahl');
LADE('laden-stelle-flut-bereich', ['V2 F2', 'Flut'], 'offene_je_sitzung 0: einst.stelle_wert.', mit({ flut: { offene_je_sitzung: 0 } }), 'einst.stelle_wert');
// Nachtrag 5, Absturz nach der Zeile: Die Kette nennt die neue Datei, auf der Platte liegt die alte.
const ALT_TEXT = einst({ vier_augen: true });
const NEU_TEXT = (() => { const d = JSON.parse(ALT_TEXT); d.stelle.vier_augen = false; return jcs(d); })();
LADE('laden-regel-absturz-nach-zeile', ['Nachtrag 5', 'A09'], 'Absturz nach der Zeile regel.geaendert: Die Datei ist noch die alte, die Kette nennt den Hash der neuen. Die Stelle startet nicht (stelle.regel_kette_weicht_ab).', ALT_TEXT, 'stelle.regel_kette_weicht_ab', sha(NEU_TEXT));
LADE('laden-mcp-zweite-person-lesend', ['V2 F2', 'Nachtrag 1'], 'Ein Werkzeug in lesen und zweite_person zugleich: einst.mcp_doppelt.', mit({}, { brain: { lesen: ['brain_suche'], schreiben: [], zweite_person: ['brain_suche'] } }), 'einst.mcp_doppelt');

// ---------- Pruefung 2: Absicht der Faelle von Fassung 2 ----------

const PRUEFEN = process.argv.slice(2).includes('--pruefen');
if (process.argv.slice(2).some((a) => a !== '--pruefen')) { console.error('Aufruf: bau-faelle.mjs [--pruefen]'); process.exit(64); }
const erzeugt = new Map();
let zeilenGesamt = 0;
let schritteGesamt = 0;
const namen = new Set();
for (const f of FAELLE) {
  if (namen.has(f.name)) throw new Error(`Fall doppelt: ${f.name}`);
  namen.add(f.name);
  const text = f.laden ? f.laden.text : f.einstellungen;
  const rollen = f.laden ? f.laden.rollen : (f.rollen ?? ROLLEN_BASIS);
  const ausgabe = { beschreibung: f.beschreibung, laden_ms: LADEN, regeln: f.regeln, einstellungen: { text, rollen }, pruefer: { art: 'h1', schluessel_hex: SCHLUESSEL_1.toString('hex') } };
  if (f.laden) {
    const r = modellLaden(text, LADEN, f.laden.regelNeu, rollen);
    if (f.laden.regelNeu !== null) ausgabe.regel_neu = f.laden.regelNeu;
    if (r.fehler !== f.laden.fehler) throw new Error(`${f.name}: Laden ergab ${r.fehler ?? 'ok'}, Absicht ${f.laden.fehler}`);
    ausgabe.erwartet_laden = { fehler: f.laden.fehler, grund: GRUENDE[f.laden.fehler] };
  } else {
    const geladen = modellLaden(text, LADEN, null, rollen);
    if (geladen.fehler) throw new Error(`${f.name}: laedt nicht (${geladen.fehler})`);
    let z = geladen.z;
    let vorher = NULL64;
    ausgabe.schritte = [];
    const alle = [];
    let letzteDatei = null;
    let aktuelleDatei = jcs(JSON.parse(text));
    for (const [n, s] of f.schritte.entries()) {
      if (s.neustart) {
        if (!s.neustart.geschrieben) { z = modellLaden(text, s.neustart.laden_ms, null, rollen).z; ausgabe.schritte.push({ neustart: { laden_ms: s.neustart.laden_ms } }); continue; }
        const regelNeu = sha(letzteDatei);
        const g = modellLaden(letzteDatei, s.neustart.laden_ms, regelNeu, rollen);
        if (g.fehler) throw new Error(`${f.name}: Neustart mit geschriebener Datei laedt nicht (${g.fehler})`);
        z = g.z;
        ausgabe.schritte.push({ neustart: { laden_ms: s.neustart.laden_ms, einstellungen: { text: letzteDatei, rollen }, regel_neu: regelNeu } });
        continue;
      }
      const wo = `${f.name} Schritt ${n + 1}`;
      if (!ARTEN.includes(s.aufrufer) || !VORGAENGE.includes(s.vorgang)) throw new Error(`${wo}: Aufrufer oder Vorgang unbekannt`);
      const jetzt = T0 + s.t;
      const vorZ = JSON.stringify(z);
      const r = entscheide(z, { aufrufer: s.aufrufer, vorgang: s.vorgang, eingabe: s.eingabe }, jetzt);
      if (JSON.stringify(z) !== vorZ) throw new Error(`${wo}: entscheide hat den alten Zustand veraendert`);
      const a = s.absicht;
      for (const k of ['ok', 'code', 'ergebnis', 'anfrage', 'antrag', 'abholen_bis_ms', 'monat', 'zugang', 'bis_ms', 'abgelaufen', 'frist_bis_ms', 'einloesen_bis_ms', 'zustand', 'entscheidend', 'mitentscheidend', 'zweite_person'].filter((x) => x in a)) {
        if (!gleich(r.antwort[k], a[k])) throw new Error(`${wo}: Antwort ${k} = ${JSON.stringify(r.antwort[k])}, Absicht ${JSON.stringify(a[k])}`);
      }
      if ('offene_ids' in a && !gleich(r.antwort.offene.map((o) => o.anfrage), a.offene_ids)) throw new Error(`${wo}: offene ${JSON.stringify(r.antwort.offene.map((o) => o.anfrage))}`);
      if ('offene_erste' in a && !gleich(r.antwort.offene.map((o) => o.erste_freigabe?.person ?? null), a.offene_erste)) throw new Error(`${wo}: erste_freigabe in offene falsch`);
      if ('offene_person' in a && !gleich(r.antwort.offene.map((o) => o.antragsteller), a.offene_person)) throw new Error(`${wo}: person in offene falsch`);
      for (const q of Object.values(r.z.anfragen)) if (!['offen', 'erste_freigabe'].includes(q.zustand) && q.vorschau !== null) throw new Error(`${wo}: Vorschau nach dem Ende der Offenheit im Zustand`);
      if (!a.ok && r.antwort.grund !== GRUENDE[a.code]) throw new Error(`${wo}: Grundtext falsch`);
      const arten = r.zeilen.map((w) => `${w.aktionsname}:${w.ergebnis}`);
      if (!gleich(arten, a.zeilen)) throw new Error(`${wo}: Zeilenarten ${JSON.stringify(arten)}, Absicht ${JSON.stringify(a.zeilen)}`);
      // Nachtrag 5: Datei genau dann, wenn eine Zeile regel.geaendert entsteht; neu der Zeile ist der Hash der Datei.
      const rz = r.zeilen.find((w) => w.aktionsname === 'regel.geaendert');
      if (Boolean(r.datei) !== Boolean(rz) || Boolean(r.datei) !== Boolean(a.datei)) throw new Error(`${wo}: Datei und Zeile regel.geaendert passen nicht zusammen`);
      if (r.datei) {
        if (sha(jcs(JSON.parse(r.datei.text))) !== rz.neu || jcs(JSON.parse(r.datei.text)) !== r.datei.text) throw new Error(`${wo}: neu ist nicht der Hash der Datei`);
        const neuGeladen = modellLaden(r.datei.text, LADEN, null, rollen);
        if (neuGeladen.fehler) throw new Error(`${wo}: geschriebene Datei laedt nicht`);
        if (neuGeladen.z.pflichten[rz.pflicht] !== r.z.pflichten[rz.pflicht] || !gleich(Object.keys(neuGeladen.z.pflichten).sort().map((k) => [k, neuGeladen.z.pflichten[k]]), Object.keys(r.z.pflichten).sort().map((k) => [k, r.z.pflichten[k]]))) throw new Error(`${wo}: Datei und Zustand nennen verschiedene Pflichten`);
        if (Object.keys(JSON.parse(aktuelleDatei)).some((k) => !(k in JSON.parse(r.datei.text)))) throw new Error(`${wo}: Datei verliert Felder`);
        if (rz.alt !== sha(aktuelleDatei)) throw new Error(`${wo}: alt ist nicht der Hash der vorigen Datei`);
        letzteDatei = r.datei.text;
        aktuelleDatei = r.datei.text;
      }
      const t = zeilenSchreiben(r, vorher, wo);
      vorher = t.vorher;
      for (const x of t.texte) alle.push(x.text);
      z = r.z;
      ausgabe.schritte.push({ aufrufer: s.aufrufer, vorgang: s.vorgang, jetzt_ms: jetzt, eingabe: s.eingabe, erwartet: { antwort: r.antwort, zeilen: t.texte, ...(r.datei ? { datei: r.datei } : {}) } });
      schritteGesamt += 1;
    }
    const e = ende2(z);
    if (!gleich(e, f.ende)) throw new Error(`${f.name}: Endzustand ${JSON.stringify(e)}, Absicht ${JSON.stringify(f.ende)}`);
    // Handgeschriebene Pruefungen einzelner Zeilen
    if (f.name === 'vier-augen-schreibaufruf') {
      const w = JSON.parse(alle.at(-1));
      if (w.mitentscheidend !== H1 || w.entscheidend !== H2) throw new Error('Vier Augen: Zeile nennt nicht beide');
    }
    if (f.name === 'einsicht-vier-augen') {
      const w = JSON.parse(alle.find((x) => x.includes('"protokoll.einsicht"')));
      if (w.zweck !== 'pruefung' || w.person !== E1 || w.zweiter !== A1) throw new Error('Einsicht: Zweck oder Personen fehlen in der Zeile');
    }
    if (f.offenePruefung) {
      const o = ausgabe.schritte.find((x) => x.vorgang === 'antrag.offene').erwartet.antwort.antraege[0];
      if (o.antragsteller !== E1 || o.art !== 'einsicht' || o.erste_freigabe !== null || o.inhalt.zweck !== 'pruefung' || 'zeilen' in o) throw new Error('antrag.offene: Felder falsch');
      const b = ausgabe.schritte.find((x) => x.vorgang === 'antrag.bestaetigen').erwartet.antwort;
      if ('quelle' in b || 'monat' in b || 'zeilen' in b) throw new Error('Bestaetigen zeigt der zweiten Person mehr als den Antrag');
    }
    if (f.flutPruefung) {
      const sammel = alle.filter((x) => x.includes('"vorgang.gedrosselt"')).map((x) => JSON.parse(x));
      const soll = [['mcp.entscheiden', 2], ['mcp.offene', 3], ['sitzung.starten', 1]];
      if (!gleich(sammel.map((w) => [w.vorgang, w.anzahl]), soll)) throw new Error(`Flut: Sammelzeilen ${JSON.stringify(sammel)}`);
      const einzeln = alle.filter((x) => x.includes('"aufrufer":"agent"') && x.includes('"vorgang.abgelehnt"')).length;
      if (einzeln + sammel.reduce((s, w) => s + w.anzahl, 0) !== 10) throw new Error('Flut: nicht jede Ablehnung ist gezaehlt');
      const erste = alle.findIndex((x) => x.includes('"vorgang.gedrosselt"'));
      const gold = `{"aktionsname":"vorgang.gedrosselt","anzahl":2,"aufrufer":"agent","ergebnis":"abgelehnt","fassung":2,"fenster_bis":"2026-11-20T13:00:10.000Z","fenster_von":"2026-11-20T13:00:00.000Z","grund":"stelle.aufrufer_nicht_erlaubt","vorgang":"mcp.entscheiden","vorher":"${sha(alle[erste - 1])}","zeit":"2026-11-20T13:00:10.000Z"}`;
      if (alle[erste] !== gold) throw new Error('Handgeschriebene Sammelzeile weicht ab');
    }
    ausgabe.ende = { ...e, zeilen: alle.length, letzte: alle.length ? vorher : null };
    zeilenGesamt += alle.length;
  }
  erzeugt.set(`${f.name}.json`, `${JSON.stringify(ausgabe, null, 2)}\n`);
}
if (PRUEFEN) {
  let vorhanden = [];
  try { vorhanden = readdirSync(AUS).filter((f) => f.endsWith('.json')); } catch { vorhanden = []; }
  const abweichend = [];
  for (const [name, text] of erzeugt) {
    let alt = null;
    try { alt = readFileSync(join(AUS, name)); } catch { alt = null; }
    if (alt === null || !alt.equals(Buffer.from(text, 'utf8'))) abweichend.push(name);
  }
  for (const name of vorhanden) if (!erzeugt.has(name)) abweichend.push(`${name} (ueberzaehlig)`);
  if (abweichend.length) { console.error(`rot: ${abweichend.length} Dateien weichen ab: ${abweichend.sort().join(', ')}`); process.exit(1); }
  console.log(`gleich: ${erzeugt.size} Faelle Byte fuer Byte wie in faelle/`);
} else {
  mkdirSync(AUS, { recursive: true });
  for (const f of readdirSync(AUS)) if (f.endsWith('.json')) rmSync(join(AUS, f));
  for (const [name, text] of erzeugt) writeFileSync(join(AUS, name), text);
}
console.log(`ok: Gleichlauf ${gleichlauf} Faelle von Fassung 1; ${FAELLE.length} Faelle von Fassung 2, ${schritteGesamt} Schritte, ${zeilenGesamt} Zeilen`);

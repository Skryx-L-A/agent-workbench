// Erzeugt die sichtbaren Faelle von Vertrag 6 (README.md in diesem Ordner) nach faelle/.
// Das Skript enthaelt ein eigenes kleines Referenzmodell der Stelle (entscheide) und einen eigenen Zeilenpruefer.
// Jeder Fall traegt die Absicht von Hand (Antwort, Zeilenarten, Endzustand). Das Modell rechnet jeden Schritt nach;
// weicht es von der Absicht ab, bricht das Skript ab. Bytes der Zeilen kommen aus der JCS-Referenz (RFC 8785) und node:crypto.
// Aufruf: node rust/spec/stelle/bau-faelle.mjs (leicht, ohne Abhaengigkeit). Gleicher Stand ergibt gleiche Bytes.

import { createHash, createHmac } from 'node:crypto';
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs } from '../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const AUS = join(HIER, 'faelle');
const sha = (b) => createHash('sha256').update(typeof b === 'string' ? Buffer.from(b, 'utf8') : b).digest('hex');
const NULL64 = '0'.repeat(64);
const SCHLUESSEL = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const WERKZEUG = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const PSEUDONYM = /^ps_[0-9a-f]{32}$/;
const ANFRAGE = /^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$/;
const ZEIT = /^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$/;
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const T0 = Date.UTC(2026, 10, 20, 13, 0, 0);
const LADEN = T0 - 60000; // fester laden_ms der Faelle
const LADEN2 = T0 + 2500; // zweiter Ladezeitpunkt (Neustart)
const mf = (n, laden = LADEN) => `mf-${laden}-${n}`;
const ARTEN = ['kern', 'hub', 'mensch', 'agent', 'takt', 'unbekannt'];
const VORGAENGE = ['sitzung.starten', 'mcp.anfragen', 'mcp.entscheiden', 'mcp.einloesen', 'mcp.offene', 'mcp.stand', 'ablauf.pruefen'];
const ERLAUBT = { 'sitzung.starten': 'kern', 'mcp.anfragen': 'hub', 'mcp.entscheiden': 'mensch', 'mcp.einloesen': 'hub', 'mcp.offene': 'mensch', 'mcp.stand': 'hub', 'ablauf.pruefen': 'takt' };

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
  'einst.json_ungueltig': 'kein gueltiges JSON',
};

const fehlerAntwort = (code) => ({ ok: false, code, grund: GRUENDE[code] });
const sicher = (v) => (typeof v === 'string' && SCHLUESSEL.test(v) ? v : null);
const istObjekt = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const bytesLaenge = (s) => Buffer.byteLength(s, 'utf8');

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw new Error('zeit_ungueltig');
  return new Date(ms).toISOString();
}

// ---------- Referenzmodell: Laden (Vertrag 2 Fassung 2, angekuendigt, nur soweit die Faelle es brauchen) ----------


// Block mcp (Vertrag 2, Fassung 2, vereinbart am 20.11.2026, geschrieben mit Beginn von B): Stufen in fester Reihenfolge ueber alle Server.
function mcpLaden(d) {
  if (d.mcp === undefined) return { mcp: {} };
  if (d.fassung !== 2) return { fehler: 'einst.schluessel_unbekannt' };
  if (!istObjekt(d.mcp)) return { fehler: 'einst.typ_falsch' };
  const srv = Object.entries(d.mcp);
  const objs = srv.filter(([, x]) => istObjekt(x)).map(([, x]) => x);
  const liste = (l) => Array.isArray(l) && l.every((w) => typeof w === 'string');
  const ganz = (v) => v === undefined || Number.isInteger(v);
  const stufen = [
    ['einst.schluessel_unbekannt', () => objs.some((x) => Object.keys(x).some((k) => !['lesen', 'schreiben', 'freigabe', 'frist_ms', 'einloesefrist_ms'].includes(k)))],
    ['einst.schluessel_fehlt', () => objs.some((x) => !('lesen' in x) || !('schreiben' in x))],
    ['einst.typ_falsch', () => srv.some(([, x]) => !istObjekt(x)) || objs.some((x) => !liste(x.lesen) || !liste(x.schreiben) || (x.freigabe !== undefined && typeof x.freigabe !== 'boolean') || !ganz(x.frist_ms) || !ganz(x.einloesefrist_ms))],
    ['einst.anzahl', () => srv.length > 64 || objs.some((x) => x.lesen.length > 256 || x.schreiben.length > 256)],
    ['einst.kennung_form', () => srv.some(([k]) => !SCHLUESSEL.test(k)) || objs.some((x) => [...x.lesen, ...x.schreiben].some((w) => !WERKZEUG.test(w)))],
    ['einst.kennung_doppelt', () => objs.some((x) => new Set(x.lesen).size !== x.lesen.length || new Set(x.schreiben).size !== x.schreiben.length)],
    ['einst.frist_bereich', () => objs.some((x) => (x.frist_ms !== undefined && (x.frist_ms < 1 || x.frist_ms > 86400000)) || (x.einloesefrist_ms !== undefined && (x.einloesefrist_ms < 1 || x.einloesefrist_ms > 86400000)))],
    ['einst.mcp_doppelt', () => objs.some((x) => x.lesen.some((w) => x.schreiben.includes(w)))],
  ];
  for (const [code, trifft] of stufen) if (trifft()) return { fehler: code };
  const mcp = {};
  for (const [name, x] of srv) mcp[name] = { lesen: x.lesen, freigabe: x.freigabe ?? true, frist_ms: x.frist_ms ?? 60000, einloesefrist_ms: x.einloesefrist_ms ?? 300000 };
  return { mcp };
}

function modellLaden(text, ladenMs) {
  let d;
  try { d = JSON.parse(text); } catch { return { fehler: 'einst.json_ungueltig' }; }
  if (!istObjekt(d)) return { fehler: 'einst.json_ungueltig' };
  for (const r of d.routen ?? []) if (!istObjekt(r) || typeof r.klasse !== 'string') return { fehler: 'einst.route_ohne_klasse' };
  const routen = {};
  for (const r of d.routen) routen[r.schluessel] = r.art;
  const kunden = (d.kunden ?? []).map((k) => k.schluessel);
  const m = mcpLaden(d);
  if (m.fehler) return { fehler: m.fehler };
  const mcp = m.mcp;
  return { z: { routen, kunden, mcp, sitzungen: {}, anfragen: {}, zaehler: 0, laden_ms: ladenMs, zeit_max: ladenMs } };
}

// ---------- Referenzmodell: Pruefer der Tests und Bindung ----------

const digestVon = (server, werkzeug, argumente) => sha(jcs({ argumente, server, werkzeug }));
const herausforderung = (anfrage, digest, entscheidung, vorschauSha) => jcs({ anfrage, art: 'mcp.entscheidung', digest, entscheidung, vorschau_sha256: vorschauSha });
const vorschauHash = (v) => (v === null ? null : sha(v));
const SCHLUESSEL_1 = Buffer.alloc(32, 0x01); // fester Testschluessel der Faelle
const SCHLUESSEL_2 = Buffer.alloc(32, 0x02);
const h1Hex = (schluessel, person, herausf) => createHmac('sha256', schluessel).update(Buffer.concat([Buffer.from(`${person}\n`, 'utf8'), Buffer.from(herausf, 'utf8')])).digest('hex');
function pruefer(herausf, nachweis) {
  const m = /^h1:(ps_[0-9a-f]{32}):([0-9a-f]{64})$/.exec(nachweis);
  return m && m[2] === h1Hex(SCHLUESSEL_1, m[1], herausf) ? m[1] : null;
}
const nachweisFuer = (person, anfrage, digest, entscheidung, vorschau, schluessel = SCHLUESSEL_1) => `h1:${person}:${h1Hex(schluessel, person, herausforderung(anfrage, digest, entscheidung, vorschauHash(vorschau)))}`;

// ---------- Referenzmodell: Start (Vertrag 1, Stufen 1 bis 14, soweit nachgerechnet) ----------

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
  if (bytesLaenge(o) > 1024 || !o.startsWith('/') || o === '/' || o.includes('//') || o.endsWith('/') || /[\u0000-\u001f\u007f]/.test(o) || o.split('/').some((s) => s === '.' || s === '..')) return nein('start.ordner_ungueltig', d);
  if (!(d.harness in HARNESS)) return nein('start.harness_unbekannt', d);
  if (!(d.route in z.routen)) return nein('start.route_unbekannt', d);
  if (d.kunde !== null && !z.kunden.includes(d.kunde)) return nein('start.kunde_unbekannt', d);
  if (HARNESS[d.harness] !== z.routen[d.route]) return nein('start.harness_route_unvertraeglich', d);
  return { ok: true, d, art: z.routen[d.route] };
}

// ---------- Referenzmodell: entscheide ----------

const stelleSicht = (d) => ({
  harness: d ? sicher(d.harness) : null, kunde: d ? sicher(d.kunde) : null, route: d ? sicher(d.route) : null, sitzung: d ? sicher(d.sitzung) : null,
});

function formPruefen(eingabe, felder) {
  if (!istObjekt(eingabe)) return false;
  const k = Object.keys(eingabe);
  if (k.length !== Object.keys(felder).length || k.some((x) => !(x in felder))) return false;
  return Object.entries(felder).every(([name, pruef]) => pruef(eingabe[name]));
}
const FORM = {
  'mcp.anfragen': {
    argumente: (v) => istObjekt(v) && bytesLaenge(jcs(v)) <= 1048576,
    frist_ms: (v) => Number.isInteger(v) && v >= 1 && v <= 86400000,
    person: (v) => typeof v === 'string' && PSEUDONYM.test(v),
    server: (v) => typeof v === 'string' && SCHLUESSEL.test(v),
    sitzung: (v) => typeof v === 'string' && SCHLUESSEL.test(v),
    vorschau: (v) => v === null || (typeof v === 'string' && bytesLaenge(v) <= 65536),
    werkzeug: (v) => typeof v === 'string' && WERKZEUG.test(v),
  },
  'mcp.entscheiden': {
    anfrage: (v) => typeof v === 'string' && ANFRAGE.test(v),
    entscheidung: (v) => v === 'erteilen' || v === 'ablehnen',
    nachweis: (v) => typeof v === 'string' && v.length >= 1 && v.length <= 4096,
  },
  'mcp.einloesen': {
    anfrage: (v) => typeof v === 'string' && ANFRAGE.test(v),
    argumente: (v) => istObjekt(v) && bytesLaenge(jcs(v)) <= 1048576,
    server: (v) => typeof v === 'string' && SCHLUESSEL.test(v),
    werkzeug: (v) => typeof v === 'string' && WERKZEUG.test(v),
  },
  'mcp.offene': {},
  'mcp.stand': { anfrage: (v) => typeof v === 'string' && ANFRAGE.test(v) },
  'ablauf.pruefen': {},
};

function endzeile(r, ergebnis) {
  return {
    aktionsname: 'mcp.schreibaufruf', anfrage: r.id, digest: r.digest, entscheidend: r.entscheidend, ergebnis, person: r.person,
    server: r.server, sitzung: r.sitzung, werkzeug: r.werkzeug, zeit_anfrage: zeitText(r.zeit_anfrage),
    zeit_entscheidung: r.zeit_entscheidung === null ? null : zeitText(r.zeit_entscheidung),
  };
}

function entscheide(z0, e, jetzt) {
  if (!Number.isInteger(jetzt) || jetzt < 0 || jetzt > MAX_MS) return { z: z0, antwort: fehlerAntwort('stelle.zeit_ungueltig'), zeilen: [], zeit_ms: null };
  const z = structuredClone(z0);
  const eff = Math.max(jetzt, z.zeit_max);
  z.zeit_max = eff;
  const zeilen = [];
  let abgelaufen = 0;
  const ids = Object.keys(z.anfragen).sort((a, b) => Number(a.split('-')[2]) - Number(b.split('-')[2]));
  for (const id of ids) {
    const r = z.anfragen[id];
    if ((r.zustand === 'offen' || r.zustand === 'erteilt') && eff >= r.bis) { r.zustand = 'abgelaufen'; r.vorschau = null; zeilen.push(endzeile(r, 'abgelaufen')); abgelaufen += 1; }
  }
  const aus = (antwort) => ({ z, antwort, zeilen, zeit_ms: eff });
  const nein = (code, sicht = {}) => {
    zeilen.push(e.vorgang === 'sitzung.starten'
      ? { aktionsname: 'sitzung.abgelehnt', ergebnis: 'abgelehnt', grund: code, ...stelleSicht(sicht.d ?? null) }
      : { aktionsname: 'vorgang.abgelehnt', anfrage: sicht.anfrage ?? null, aufrufer: e.aufrufer, ergebnis: 'abgelehnt', grund: code, vorgang: e.vorgang });
    return aus(fehlerAntwort(code));
  };
  const anfrageKey = typeof e.eingabe?.anfrage === 'string' && ANFRAGE.test(e.eingabe.anfrage) ? e.eingabe.anfrage : null;
  if (e.aufrufer !== ERLAUBT[e.vorgang]) return nein('stelle.aufrufer_nicht_erlaubt', { anfrage: anfrageKey });
  const form = e.vorgang === 'sitzung.starten' ? startForm(e.eingabe) : formPruefen(e.eingabe, FORM[e.vorgang]);
  if (!form) return nein('stelle.eingabe_ungueltig', { anfrage: anfrageKey });

  if (e.vorgang === 'ablauf.pruefen') return aus({ ok: true, ergebnis: 'geprueft', abgelaufen });
  if (e.vorgang === 'mcp.offene') {
    const offene = ids.filter((id) => z.anfragen[id].zustand === 'offen').map((id) => {
      const r = z.anfragen[id];
      return { anfrage: id, server: r.server, werkzeug: r.werkzeug, vorschau: r.vorschau, vorschau_sha256: r.vorschau_sha256, digest: r.digest, frist_bis_ms: r.bis };
    });
    return aus({ ok: true, ergebnis: 'offene', offene });
  }

  if (e.vorgang === 'sitzung.starten') {
    const bytes = e.eingabe.start.text !== undefined ? Buffer.from(e.eingabe.start.text, 'utf8') : Buffer.from(e.eingabe.start.b64, 'base64');
    const r = startPruefen(bytes, z);
    if (!r.ok) return nein(r.code, { d: r.d });
    if (r.d.sitzung in z.sitzungen) return nein('stelle.sitzung_vergeben', { d: r.d });
    z.sitzungen[r.d.sitzung] = { route: r.d.route, kunde: r.d.kunde, harness: r.d.harness };
    zeilen.push({ aktionsname: 'sitzung.gestartet', ergebnis: 'gestartet', grund: null, ...stelleSicht(r.d) });
    return aus({ ok: true, ergebnis: 'gestartet', sitzung: r.d.sitzung, route: r.d.route, route_art: r.art });
  }

  const i = e.eingabe;
  if (e.vorgang === 'mcp.anfragen') {
    const s = z.mcp[i.server];
    if (!s) return nein('stelle.server_unbekannt');
    if (!(i.sitzung in z.sitzungen)) return nein('stelle.sitzung_unbekannt');
    if (s.lesen.includes(i.werkzeug)) return aus({ ok: true, ergebnis: 'lesend' });
    const digest = digestVon(i.server, i.werkzeug, i.argumente);
    const r = { server: i.server, werkzeug: i.werkzeug, sitzung: i.sitzung, person: i.person, digest, vorschau: i.vorschau, vorschau_sha256: vorschauHash(i.vorschau), zeit_anfrage: eff, entscheidend: null, zeit_entscheidung: null };
    if (!s.freigabe) {
      z.zaehler += 1; r.id = `mf-${z.laden_ms}-${z.zaehler}`;
      zeilen.push(endzeile(r, 'ohne_freigabe'));
      return aus({ ok: true, ergebnis: 'ohne_freigabe', anfrage: r.id, digest });
    }
    if (i.frist_ms > s.frist_ms) return nein('stelle.frist_ungueltig');
    z.zaehler += 1; r.id = `mf-${z.laden_ms}-${z.zaehler}`; r.bis = eff + i.frist_ms; r.zustand = 'offen';
    z.anfragen[r.id] = r;
    return aus({ ok: true, ergebnis: 'offen', anfrage: r.id, digest, frist_bis_ms: r.bis, vorschau_sha256: r.vorschau_sha256 });
  }

  const r = z.anfragen[i.anfrage];
  if (!r) return nein('stelle.anfrage_unbekannt', { anfrage: i.anfrage });
  const sicht = { anfrage: i.anfrage };
  if (e.vorgang === 'mcp.entscheiden') {
    if (r.zustand === 'abgelaufen') return nein('stelle.anfrage_abgelaufen', sicht);
    if (r.zustand !== 'offen') return nein('stelle.bereits_entschieden', sicht);
    const person = pruefer(herausforderung(r.id, r.digest, i.entscheidung, r.vorschau_sha256), i.nachweis);
    if (person === null) return nein('stelle.nachweis_ungueltig', sicht);
    r.entscheidend = person; r.zeit_entscheidung = eff; r.vorschau = null;
    if (i.entscheidung === 'ablehnen') {
      r.zustand = 'abgelehnt';
      zeilen.push(endzeile(r, 'abgelehnt'));
      return aus({ ok: true, ergebnis: 'abgelehnt', anfrage: r.id });
    }
    r.zustand = 'erteilt'; r.bis = eff + z.mcp[r.server].einloesefrist_ms;
    return aus({ ok: true, ergebnis: 'erteilt', anfrage: r.id, einloesen_bis_ms: r.bis });
  }
  if (e.vorgang === 'mcp.stand') {
    const laufend = r.zustand === 'offen' || r.zustand === 'erteilt';
    return aus({ ok: true, ergebnis: 'stand', anfrage: r.id, zustand: r.zustand, frist_bis_ms: laufend ? r.bis : null });
  }
  // mcp.einloesen
  const stop = { offen: 'stelle.nicht_freigegeben', abgelehnt: 'stelle.anfrage_abgelehnt', abgelaufen: 'stelle.anfrage_abgelaufen', eingeloest: 'stelle.bereits_eingeloest' }[r.zustand];
  if (stop) return nein(stop, sicht);
  if (digestVon(i.server, i.werkzeug, i.argumente) !== r.digest) return nein('stelle.digest_abweichend', sicht);
  r.zustand = 'eingeloest';
  zeilen.push(endzeile(r, 'eingeloest'));
  return aus({ ok: true, ergebnis: 'eingeloest', anfrage: r.id, digest: r.digest });
}

function startForm(eingabe) {
  if (!istObjekt(eingabe) || Object.keys(eingabe).length !== 1 || !istObjekt(eingabe.start)) return false;
  const k = Object.keys(eingabe.start);
  if (k.length !== 1) return false;
  if (k[0] === 'text') return typeof eingabe.start.text === 'string';
  if (k[0] === 'b64') return typeof eingabe.start.b64 === 'string' && /^[A-Za-z0-9+/]*={0,2}$/.test(eingabe.start.b64);
  return false;
}

// ---------- Eigener Zeilenpruefer (Form der drei Zeilenarten, Kette, Kanonik, Inhaltssperre) ----------

const SCHL = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
const SCHL0 = (v) => v === null || SCHL(v);
const FORMEN = {
  'sitzung.gestartet': { ergebnis: (v) => v === 'gestartet', grund: (v) => v === null, harness: SCHL0, kunde: SCHL0, route: SCHL0, sitzung: SCHL0 },
  'sitzung.abgelehnt': { ergebnis: (v) => v === 'abgelehnt', grund: SCHL, harness: SCHL0, kunde: SCHL0, route: SCHL0, sitzung: SCHL0 },
  'vorgang.abgelehnt': { anfrage: (v) => v === null || ANFRAGE.test(v), aufrufer: (v) => ARTEN.includes(v), ergebnis: (v) => v === 'abgelehnt', grund: SCHL, vorgang: (v) => VORGAENGE.includes(v) },
  'mcp.schreibaufruf': {
    anfrage: (v) => ANFRAGE.test(v), digest: (v) => /^[0-9a-f]{64}$/.test(v), entscheidend: (v) => v === null || PSEUDONYM.test(v),
    ergebnis: (v) => ['eingeloest', 'abgelehnt', 'abgelaufen', 'ohne_freigabe'].includes(v), person: (v) => PSEUDONYM.test(v), server: SCHL, sitzung: SCHL,
    werkzeug: (v) => typeof v === 'string' && WERKZEUG.test(v), zeit_anfrage: (v) => ZEIT.test(v), zeit_entscheidung: (v) => v === null || ZEIT.test(v),
  },
};

function zeilePruefen(text, vorher) {
  const w = JSON.parse(text);
  if (jcs(w) !== text) throw new Error('nicht kanonisch');
  if (w.fassung !== 2 || typeof w.zeit !== 'string' || !ZEIT.test(w.zeit) || w.vorher !== vorher) throw new Error('Kopf oder Kette falsch');
  const form = FORMEN[w.aktionsname];
  if (!form) throw new Error(`Zeilenart unbekannt: ${w.aktionsname}`);
  const soll = [...Object.keys(form), 'aktionsname', 'fassung', 'vorher', 'zeit'].sort();
  if (JSON.stringify(Object.keys(w).sort()) !== JSON.stringify(soll)) throw new Error(`Felder nicht geschlossen: ${text}`);
  for (const [k, p] of Object.entries(form)) if (!p(w[k])) throw new Error(`Feld ${k} falsch: ${text}`);
  // Einmaligkeit des Endes: ein Ergebnis mit Entscheidung hat entscheidend und zeit_entscheidung zusammen
  if (w.aktionsname === 'mcp.schreibaufruf' && (w.entscheidend === null) !== (w.zeit_entscheidung === null)) throw new Error('entscheidend und zeit_entscheidung gehoeren zusammen');
  if (w.aktionsname === 'mcp.schreibaufruf' && (w.ergebnis === 'abgelehnt' || w.ergebnis === 'eingeloest') && w.entscheidend === null) throw new Error('Ergebnis ohne Freigebenden');
  if (w.aktionsname === 'mcp.schreibaufruf' && w.ergebnis === 'ohne_freigabe' && w.entscheidend !== null) throw new Error('ohne_freigabe mit Freigebendem');
  return sha(text);
}

// ---------- Fall-Bausteine ----------

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
    'such-mcp': { lesen: ['brain_suche'], schreiben: [] },
  },
};
const EINST = JSON.stringify(E_BASIS);
const S_CLOUD = { fassung: 1, sitzung: 's-1', harness: 'claude', modell: 'claude-sonnet-5-5', route: 'cloud-beispiel', kunde: 'k-beispiel', arbeitsordner: '/home/beispiel/projekt' };
const S_LOKAL = { fassung: 1, sitzung: 's-2', harness: 'pi', modell: 'qwen3:8b', route: 'lokal-beispiel', kunde: null, arbeitsordner: '/home/beispiel/projekt' };
const P1 = `ps_${'0'.repeat(31)}1`;
const H1 = `ps_${'0'.repeat(29)}a01`;
const H2 = `ps_${'0'.repeat(29)}a02`;
const ARG = { feld: 'GEHEIM-ARG-7Q', id: 'T-100', status: 'erledigt' };
const VOR = 'Vorher: offen, Nachher: erledigt VORSCHAU-5K';
const D1 = digestVon('tickets', 'update', ARG);
const MARKER = ['GEHEIM-ARG-7Q', 'VORSCHAU-5K'];

const startE = (obj) => ({ start: { text: JSON.stringify(obj) } });
const anfr = (o = {}) => ({ argumente: ARG, frist_ms: 60000, person: P1, server: 'tickets', sitzung: 's-1', vorschau: VOR, werkzeug: 'update', ...o });
const ent = (anfrage, entscheidung, person = H1, o = {}) => {
  const a = { server: 'tickets', werkzeug: 'update', argumente: ARG, vorschau: VOR, ...o };
  return { anfrage, entscheidung, nachweis: nachweisFuer(person, anfrage, digestVon(a.server, a.werkzeug, a.argumente), entscheidung, a.vorschau) };
};
const loes = (anfrage, o = {}) => ({ anfrage, argumente: ARG, server: 'tickets', werkzeug: 'update', ...o });
const st = (aufrufer, vorgang, t, eingabe, absicht) => ({ aufrufer, vorgang, t, eingabe, absicht });
const K = (t, obj, absicht) => st('kern', 'sitzung.starten', t, startE(obj), absicht);
const GESTARTET = (art = 'cloud') => ({ ok: true, ergebnis: 'gestartet', route_art: art, zeilen: ['sitzung.gestartet:gestartet'] });
const START1 = () => K(0, S_CLOUD, GESTARTET());
const ABG = (code, kind = 'vorgang.abgelehnt', vor = []) => ({ ok: false, code, zeilen: [...vor, `${kind}:abgelehnt`] });
const ZU = (ergebnis) => `mcp.schreibaufruf:${ergebnis}`;

const FAELLE = [];
const fall = (name, regeln, beschreibung, schritte, ende, extra = {}) => FAELLE.push({ name, regeln, beschreibung, schritte, ende, ...extra });

// Start
fall('start-cloud-gueltig', ['A43', 'A91'], 'Die Startdatei mit Harness claude und Cloud-Route wird angenommen; eine Zeile sitzung.gestartet.',
  [K(0, S_CLOUD, GESTARTET('cloud'))], { sitzungen: ['s-1'], anfragen: {}, zaehler: 0 });
fall('start-lokal-gueltig', ['A43'], 'Harness pi mit lokaler Route und kunde null wird angenommen.',
  [K(0, S_LOKAL, GESTARTET('lokal'))], { sitzungen: ['s-2'], anfragen: {}, zaehler: 0 });
fall('start-route-unbekannt', ['A43', 'V1 Stufe 12'], 'Eine Route ausserhalb der Einstellungen wird mit dem Code aus Vertrag 1 abgelehnt und als sitzung.abgelehnt protokolliert.',
  [K(0, { ...S_CLOUD, route: 'cloud-fremd' }, ABG('start.route_unbekannt', 'sitzung.abgelehnt'))], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-ohne-route', ['A43', 'V1 Stufe 5'], 'Ohne Feld route lehnt der Start ab; die Zeile traegt route null.',
  [K(0, (({ route, ...r }) => r)(S_CLOUD), ABG('start.schluessel_fehlt', 'sitzung.abgelehnt'))], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-kunde-unbekannt', ['V1 Stufe 13'], 'Ein Kunde ausserhalb der Einstellungen wird abgelehnt.',
  [K(0, { ...S_CLOUD, kunde: 'k-fremd' }, ABG('start.kunde_unbekannt', 'sitzung.abgelehnt'))], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-harness-passt-nicht', ['V1 Stufe 14'], 'claude auf einer lokalen Route wird abgelehnt.',
  [K(0, { ...S_CLOUD, route: 'lokal-beispiel', modell: 'qwen3:8b' }, ABG('start.harness_route_unvertraeglich', 'sitzung.abgelehnt'))], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-kennung-schreibt-null', ['A104', 'V1 Stufe 8'], 'Eine Route mit Leerzeichen verletzt die Kennungsform; die Zeile nimmt den Wert nicht auf, sondern null.',
  [K(0, { ...S_CLOUD, route: 'Cloud Beispiel' }, ABG('start.kennung_form', 'sitzung.abgelehnt'))], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-bytes', ['V1 Stufe 1', 'V1 Stufe 2'], 'Zu grosse Startdatei, ein BOM und Text nach dem Objekt ergeben die Codes aus Vertrag 1; die Zeilen tragen nur Nullwerte.',
  [
    st('kern', 'sitzung.starten', 0, { start: { b64: Buffer.alloc(4097, 0x20).toString('base64') } }, ABG('start.zu_gross', 'sitzung.abgelehnt')),
    st('kern', 'sitzung.starten', 1, { start: { b64: Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(JSON.stringify(S_CLOUD))]).toString('base64') } }, ABG('start.json_ungueltig', 'sitzung.abgelehnt')),
    st('kern', 'sitzung.starten', 2, { start: { text: `${JSON.stringify(S_CLOUD)} x` } }, ABG('start.json_ungueltig', 'sitzung.abgelehnt')),
  ], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('start-sitzung-vergeben', ['V1 Kern-Dienst'], 'Dieselbe Sitzungskennung wird nur einmal angenommen.',
  [START1(), K(1, S_CLOUD, ABG('stelle.sitzung_vergeben', 'sitzung.abgelehnt'))], { sitzungen: ['s-1'], anfragen: {}, zaehler: 0 });
fall('start-nur-der-kern', ['A92', 'Prueffolge'], 'Nur der Kern-Dienst startet. Agent, Hub und unbekannter Aufrufer scheitern vor jeder Pruefung der Eingabe; ungueltige Eingabe des Kerns scheitert danach.',
  [
    st('agent', 'sitzung.starten', 0, startE(S_CLOUD), ABG('stelle.aufrufer_nicht_erlaubt', 'sitzung.abgelehnt')),
    st('hub', 'sitzung.starten', 1, startE(S_CLOUD), ABG('stelle.aufrufer_nicht_erlaubt', 'sitzung.abgelehnt')),
    st('unbekannt', 'sitzung.starten', 2, { start: 5 }, ABG('stelle.aufrufer_nicht_erlaubt', 'sitzung.abgelehnt')),
    st('kern', 'sitzung.starten', 3, { start: 5 }, ABG('stelle.eingabe_ungueltig', 'sitzung.abgelehnt')),
  ], { sitzungen: [], anfragen: {}, zaehler: 0 });
fall('laden-route-ohne-klasse', ['A42', 'V2 Stufe 4'], 'Einstellungen mit einer Route ohne Klasse laden nicht; es gibt keine Stelle und keinen Start.',
  [], null, { laden: { text: JSON.stringify({ ...E_BASIS, routen: [{ schluessel: 'cloud-beispiel', art: 'cloud' }, E_BASIS.routen[1]] }), fehler: 'einst.route_ohne_klasse' } });
const mitMcp = (mcp, o = {}) => JSON.stringify({ ...E_BASIS, ...o, mcp });
fall('laden-mcp-doppelt', ['A89', 'V2 F2'], 'Ein Werkzeug in lesen und schreiben zugleich: die Stelle laedt nicht (einst.mcp_doppelt).',
  [], null, { laden: { text: mitMcp({ tickets: { lesen: ['update'], schreiben: ['update'] } }), fehler: 'einst.mcp_doppelt' } });
fall('laden-mcp-frist-bereich', ['A90', 'V2 F2'], 'Eine Frist ausserhalb 1 bis 86400000 laedt nicht (einst.frist_bereich), auch die Einloesefrist.',
  [], null, { laden: { text: mitMcp({ tickets: { lesen: [], schreiben: ['update'], einloesefrist_ms: 86400001 } }), fehler: 'einst.frist_bereich' } });
fall('laden-mcp-unbekannter-schluessel', ['V2 F2'], 'Ein unbekannter Schluessel im Block mcp faellt in die vorhandene Stufe von Vertrag 2 (einst.schluessel_unbekannt).',
  [], null, { laden: { text: mitMcp({ tickets: { lesen: [], schreiben: [], frist: 5 } }), fehler: 'einst.schluessel_unbekannt' } });
fall('laden-mcp-anzahl', ['V2 F2'], 'Mehr als 256 Werkzeuge in einer Liste (Vorschlag der Grenze) lehnt mit der vorhandenen Stufe einst.anzahl ab.',
  [], null, { laden: { text: mitMcp({ tickets: { lesen: Array.from({ length: 257 }, (_, n) => `w${n}`), schreiben: [] } }), fehler: 'einst.anzahl' } });
fall('laden-mcp-typ-falsch', ['V2 F2'], 'freigabe als Text faellt in die vorhandene Stufe einst.typ_falsch.',
  [], null, { laden: { text: mitMcp({ tickets: { lesen: [], schreiben: [], freigabe: 'ja' } }), fehler: 'einst.typ_falsch' } });

// MCP
const M_OFFEN = (id = mf(1)) => ({ ok: true, ergebnis: 'offen', anfrage: id, zeilen: [] });
const M_ERTEILT = { ok: true, ergebnis: 'erteilt', zeilen: [] };
fall('freigabe-erteilt-und-eingeloest', ['A90', 'A91', 'A105'], 'Anfrage, Erteilung durch einen Menschen und Einloesen durch den Hub; eine Zeile mit dem Freigebenden.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 5000, ent(mf(1), 'erteilen'), M_ERTEILT),
    st('hub', 'mcp.einloesen', 8000, loes(mf(1)), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1 });
fall('einloesen-zweimal', ['A105 (2)'], 'Dieselbe Freigabe loest nur einmal ein; der zweite Versuch steht als vorgang.abgelehnt im Protokoll.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), M_ERTEILT),
    st('hub', 'mcp.einloesen', 3000, loes(mf(1)), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] }),
    st('hub', 'mcp.einloesen', 4000, loes(mf(1)), ABG('stelle.bereits_eingeloest'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1 });
fall('einloesen-nach-frist', ['A105 (3)'], 'Genau am Ende der Einloesefrist ist die Freigabe abgelaufen: eine Zeile abgelaufen mit dem Freigebenden, dazu die Ablehnung des Einloesens.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 5000, ent(mf(1), 'erteilen'), { ...M_ERTEILT, einloesen_bis_ms: T0 + 35000 }),
    st('hub', 'mcp.einloesen', 35000, loes(mf(1)), ABG('stelle.anfrage_abgelaufen', 'vorgang.abgelehnt', [ZU('abgelaufen')]))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelaufen' }, zaehler: 1 });
fall('frist-grenzen', ['A90', 'A105'], 'Eine Millisekunde vor dem Fristende gilt, am Fristende nicht: Entscheidung und Einloesen vor, Entscheidung am Ende.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), { ...M_OFFEN(mf(1)), frist_bis_ms: T0 + 61000 }), st('hub', 'mcp.anfragen', 1000, anfr({ werkzeug: 'update' }), M_OFFEN(mf(2))),
    st('mensch', 'mcp.entscheiden', 60999, ent(mf(1), 'erteilen'), { ...M_ERTEILT, einloesen_bis_ms: T0 + 90999 }),
    st('mensch', 'mcp.entscheiden', 61000, ent(mf(2), 'erteilen'), ABG('stelle.anfrage_abgelaufen', 'vorgang.abgelehnt', [ZU('abgelaufen')])),
    st('hub', 'mcp.einloesen', 90998, loes(mf(1)), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest', [mf(2)]: 'abgelaufen' }, zaehler: 2 });
fall('einloesen-anderes-argument', ['A105 (1)'], 'Anderes Argument, anderes Werkzeug oder anderer Server beim Einloesen: abgelehnt, die Freigabe bleibt gueltig und loest mit dem richtigen Aufruf ein.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), M_ERTEILT),
    st('hub', 'mcp.einloesen', 3000, loes(mf(1), { argumente: { ...ARG, status: 'geloescht' } }), ABG('stelle.digest_abweichend')),
    st('hub', 'mcp.einloesen', 3001, loes(mf(1), { werkzeug: 'neu' }), ABG('stelle.digest_abweichend')),
    st('hub', 'mcp.einloesen', 3002, loes(mf(1), { server: 'notizen' }), ABG('stelle.digest_abweichend')),
    st('hub', 'mcp.einloesen', 4000, loes(mf(1)), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest' }, zaehler: 1 });
fall('agent-und-hub-geben-nicht-frei', ['A92'], 'Agent, Worker-Prozess, Hub, Kern und unbekannter Aufrufer koennen nicht entscheiden, auch nicht mit gueltigem Nachweis; die Anfrage bleibt offen und ein Mensch gibt danach frei.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()),
    ...['agent', 'hub', 'kern', 'takt', 'unbekannt'].map((a, n) => st(a, 'mcp.entscheiden', 2000 + n, ent(mf(1), 'erteilen'), ABG('stelle.aufrufer_nicht_erlaubt'))),
    st('mensch', 'mcp.entscheiden', 3000, ent(mf(1), 'erteilen'), M_ERTEILT)],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'erteilt' }, zaehler: 1 });
fall('nur-der-hub-loest-ein', ['A92', 'Prueffolge'], 'Ein Mensch, ein Agent oder ein unbekannter Aufrufer loest nicht ein; die Aufruferpruefung kommt vor der Pruefung der Eingabe.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), M_ERTEILT),
    st('mensch', 'mcp.einloesen', 3000, loes(mf(1)), ABG('stelle.aufrufer_nicht_erlaubt')),
    st('agent', 'mcp.einloesen', 3001, loes(mf(1)), ABG('stelle.aufrufer_nicht_erlaubt')),
    st('unbekannt', 'mcp.einloesen', 3002, loes(mf(1)), ABG('stelle.aufrufer_nicht_erlaubt')),
    st('mensch', 'mcp.einloesen', 3003, {}, ABG('stelle.aufrufer_nicht_erlaubt')),
    st('hub', 'mcp.einloesen', 3004, {}, ABG('stelle.eingabe_ungueltig'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'erteilt' }, zaehler: 1 });
fall('ablehnung-durch-mensch', ['A91', 'A105'], 'Eine Ablehnung durch einen Menschen schreibt eine Zeile mit dem Entscheidenden; Einloesen und zweite Entscheidung scheitern danach.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'ablehnen'), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('hub', 'mcp.einloesen', 3000, loes(mf(1)), ABG('stelle.anfrage_abgelehnt')),
    st('mensch', 'mcp.entscheiden', 4000, ent(mf(1), 'erteilen'), ABG('stelle.bereits_entschieden'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelehnt' }, zaehler: 1 });
fall('einloesen-ohne-freigabe', ['A105'], 'Eine offene Anfrage loest nicht ein.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('hub', 'mcp.einloesen', 2000, loes(mf(1)), ABG('stelle.nicht_freigegeben')),
    st('hub', 'mcp.einloesen', 2001, loes(mf(7)), ABG('stelle.anfrage_unbekannt'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
fall('werkzeug-ohne-zuordnung', ['A89'], 'Ein Werkzeug, das in keiner Liste steht, gilt als schreibend und wartet auf die Freigabe.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr({ werkzeug: 'neu' }), M_OFFEN())],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
fall('lesendes-werkzeug-frei', ['A89'], 'Lesende Werkzeuge (get, brain_suche) brauchen keine Freigabe: keine Anfrage, keine Zeile, kein Zaehlerschritt.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr({ werkzeug: 'get' }), { ok: true, ergebnis: 'lesend', zeilen: [] }),
    st('hub', 'mcp.anfragen', 1001, anfr({ server: 'such-mcp', werkzeug: 'brain_suche', argumente: { frage: 'GEHEIM-ARG-7Q' }, vorschau: null }), { ok: true, ergebnis: 'lesend', zeilen: [] }),
    st('hub', 'mcp.anfragen', 1002, anfr({ server: 'such-mcp', werkzeug: 'brain_schreibe' }), M_OFFEN())],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
fall('server-ohne-freigabe-option', ['A91', 'O3'], 'Ist die Option fuer den Server aus, laeuft der Schreibaufruf ohne Freigabe, steht aber mit ohne_freigabe im Protokoll.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr({ server: 'notizen', werkzeug: 'anlegen' }), { ok: true, ergebnis: 'ohne_freigabe', anfrage: mf(1), zeilen: [ZU('ohne_freigabe')] })],
  { sitzungen: ['s-1'], anfragen: {}, zaehler: 1 });
fall('nachweis-ungueltig', ['A92', 'A105'], 'Pruefer h1: Nachweis fuer einen anderen Aufruf oder eine andere Entscheidung (erteilen fuer ablehnen und umgekehrt), falsches Pseudonym im Text, falscher Schluessel, fremdes Format, Grossbuchstaben: jeweils abgelehnt; die Anfrage bleibt offen, danach gilt der richtige Nachweis.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen', H1, { argumente: { ...ARG, status: 'geloescht' } }), ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2001, { anfrage: mf(1), entscheidung: 'erteilen', nachweis: nachweisFuer(H1, mf(1), digestVon('tickets', 'update', ARG), 'ablehnen', VOR) }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2002, ent(mf(1), 'erteilen', H1, { vorschau: 'andere Vorschau' }), ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2003, { anfrage: mf(1), entscheidung: 'erteilen', nachweis: 'egal' }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2004, { anfrage: mf(1), entscheidung: 'erteilen', nachweis: nachweisFuer(H1, mf(1), D1, 'erteilen', VOR).replace(`h1:${H1}:`, `h1:${H2}:`) }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2005, { anfrage: mf(1), entscheidung: 'erteilen', nachweis: nachweisFuer(H1, mf(1), D1, 'erteilen', VOR, SCHLUESSEL_2) }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2006, { anfrage: mf(1), entscheidung: 'ablehnen', nachweis: nachweisFuer(H1, mf(1), D1, 'erteilen', VOR) }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 2007, { anfrage: mf(1), entscheidung: 'erteilen', nachweis: nachweisFuer(H1, mf(1), D1, 'erteilen', VOR).toUpperCase() }, ABG('stelle.nachweis_ungueltig')),
    st('mensch', 'mcp.entscheiden', 3000, ent(mf(1), 'erteilen'), M_ERTEILT)],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'erteilt' }, zaehler: 1 });
fall('ablauf-ohne-entscheidung', ['A90'], 'Eine unbeantwortete Anfrage verfaellt zur Frist; der Taktgeber loest die Zeile aus, entscheidend ist null.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()),
    st('takt', 'ablauf.pruefen', 60999, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 0, zeilen: [] }),
    st('takt', 'ablauf.pruefen', 61000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: [ZU('abgelaufen')] }),
    st('takt', 'ablauf.pruefen', 61001, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 0, zeilen: [] }),
    st('mensch', 'mcp.entscheiden', 62000, ent(mf(1), 'erteilen'), ABG('stelle.anfrage_abgelaufen')),
    st('mensch', 'ablauf.pruefen', 62001, {}, ABG('stelle.aufrufer_nicht_erlaubt'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelaufen' }, zaehler: 1 });
fall('ablauf-erteilt-nicht-eingeloest', ['A90', 'A105'], 'Eine erteilte, aber nicht eingeloeste Freigabe verfaellt; die Zeile nennt den Freigebenden.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), M_ERTEILT),
    st('takt', 'ablauf.pruefen', 40000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: [ZU('abgelaufen')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelaufen' }, zaehler: 1 });
fall('uhr-geht-zurueck', ['A105'], 'Die Zeit geht nie zurueck: Ein Ereignis mit kleinerer Zeit gilt mit der hoechsten bisher gesehenen; die abgelaufene Anfrage bleibt abgelaufen.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('takt', 'ablauf.pruefen', 70000, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: [ZU('abgelaufen')] }),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), ABG('stelle.anfrage_abgelaufen')),
    st('hub', 'mcp.anfragen', 2001, anfr(), { ...M_OFFEN(mf(2)), frist_bis_ms: T0 + 70000 + 60000 })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelaufen', [mf(2)]: 'offen' }, zaehler: 2 });
fall('neustart-macht-alles-ungueltig', ['E1', 'A105'], 'Nach einem Neustart (laden mit anderem laden_ms) gibt es keine Sitzung und keine Anfrage mehr: Einloesen und Anfragen scheitern, die Sitzung muss neu starten; die neue Kennung traegt den neuen laden_ms und kollidiert nie mit einer alten.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()), st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen'), M_ERTEILT),
    { neustart: { laden_ms: LADEN2 } },
    st('hub', 'mcp.einloesen', 3000, loes(mf(1)), ABG('stelle.anfrage_unbekannt')),
    st('hub', 'mcp.anfragen', 3001, anfr(), ABG('stelle.sitzung_unbekannt')),
    K(3000, S_CLOUD, GESTARTET()),
    st('hub', 'mcp.anfragen', 3002, anfr(), { ...M_OFFEN(mf(1, LADEN2)), frist_bis_ms: T0 + 63002 }),
    st('hub', 'mcp.einloesen', 3003, loes(mf(1)), ABG('stelle.anfrage_unbekannt'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1, LADEN2)]: 'offen' }, zaehler: 1 });
const OFF = (ids) => ({ ok: true, ergebnis: 'offene', offene_ids: ids, zeilen: [] });
fall('offene-liste', ['A90', 'Nachtrag 4'], 'mcp.offene zeigt dem Menschen die offenen Anfragen mit Vorschau, sortiert nach Kennung, ohne Zeile im Protokoll; erteilte, abgelehnte und abgelaufene stehen nicht mehr darin, ihre Vorschau ist aus dem Zustand.',
  [START1(), st('mensch', 'mcp.offene', 500, {}, OFF([])),
    st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), M_OFFEN(mf(1))), st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 }, vorschau: null }), M_OFFEN(mf(2))),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: { n: 3 } }), M_OFFEN(mf(3))),
    st('mensch', 'mcp.offene', 1500, {}, OFF([mf(1), mf(2), mf(3)])),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), M_ERTEILT),
    st('mensch', 'mcp.entscheiden', 2001, ent(mf(3), 'ablehnen', H1, { argumente: { n: 3 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('mensch', 'mcp.offene', 2500, {}, OFF([mf(2)])),
    st('mensch', 'mcp.offene', 62000, {}, { ...OFF([]), zeilen: [ZU('abgelaufen'), ZU('abgelaufen')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'abgelaufen', [mf(2)]: 'abgelaufen', [mf(3)]: 'abgelehnt' }, zaehler: 3 });
fall('offene-nur-der-mensch', ['A92', 'Nachtrag 4'], 'mcp.offene liefert nur dem Menschen; jede andere Art scheitert vor der Eingabe, eine Eingabe mit Feld scheitert danach.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()),
    ...['hub', 'agent', 'kern', 'takt', 'unbekannt'].map((a, n) => st(a, 'mcp.offene', 2000 + n, {}, ABG('stelle.aufrufer_nicht_erlaubt'))),
    st('mensch', 'mcp.offene', 3000, { anfrage: mf(1) }, ABG('stelle.eingabe_ungueltig')),
    st('mensch', 'mcp.offene', 3001, {}, OFF([mf(1)]))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
const STAND = (id, zustand, bis, zeilen = []) => ({ ok: true, ergebnis: 'stand', anfrage: id, zustand, frist_bis_ms: bis, zeilen });
fall('stand-fragen', ['A105', 'Nachtrag 6'], 'mcp.stand sagt dem Hub den Zustand einer Anfrage, damit er mcp.einloesen nicht wiederholen muss: offen, erteilt, eingeloest, abgelehnt, nach Ablauf; frist_bis_ms nur in den laufenden Zustaenden. Ein erfolgreicher Aufruf schreibt keine Zeile.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), M_OFFEN(mf(1))), st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 } }), M_OFFEN(mf(2))),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: { n: 3 } }), M_OFFEN(mf(3))),
    st('hub', 'mcp.stand', 1500, { anfrage: mf(1) }, STAND(mf(1), 'offen', T0 + 61000)),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), M_ERTEILT),
    st('mensch', 'mcp.entscheiden', 2001, ent(mf(3), 'ablehnen', H1, { argumente: { n: 3 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('hub', 'mcp.stand', 2500, { anfrage: mf(1) }, STAND(mf(1), 'erteilt', T0 + 32000)),
    st('hub', 'mcp.stand', 2501, { anfrage: mf(3) }, STAND(mf(3), 'abgelehnt', null)),
    st('hub', 'mcp.einloesen', 3000, loes(mf(1), { argumente: { n: 1 } }), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] }),
    st('hub', 'mcp.stand', 3500, { anfrage: mf(1) }, STAND(mf(1), 'eingeloest', null)),
    st('hub', 'mcp.stand', 62000, { anfrage: mf(2) }, STAND(mf(2), 'abgelaufen', null, [ZU('abgelaufen')]))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest', [mf(2)]: 'abgelaufen', [mf(3)]: 'abgelehnt' }, zaehler: 3 });
fall('stand-unbekannt-und-aufrufer', ['A92', 'Nachtrag 6'], 'mcp.stand: unbekannte Anfrage und falsche Aufrufer werden abgelehnt und protokolliert wie jede Ablehnung; nur der Hub fragt den Stand.',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr(), M_OFFEN()),
    st('hub', 'mcp.stand', 2000, { anfrage: mf(9) }, ABG('stelle.anfrage_unbekannt')),
    ...['mensch', 'agent', 'kern', 'takt', 'unbekannt'].map((a, n) => st(a, 'mcp.stand', 2001 + n, { anfrage: mf(1) }, ABG('stelle.aufrufer_nicht_erlaubt'))),
    st('hub', 'mcp.stand', 3000, { anfrage: 'mf-1' }, ABG('stelle.eingabe_ungueltig')),
    st('hub', 'mcp.stand', 3001, { anfrage: mf(1), x: 1 }, ABG('stelle.eingabe_ungueltig'))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
fall('anfragen-gruende', ['Prueffolge', 'A93'], 'Die Gruende der Ablehnung einer Anfrage in fester Reihenfolge; keine Anfrage entsteht. Ein Feld aufrufer in der Eingabe ist ungueltig.',
  [START1(),
    st('hub', 'mcp.anfragen', 1000, { ...anfr(), aufrufer: 'mensch' }, ABG('stelle.eingabe_ungueltig')),
    st('hub', 'mcp.anfragen', 1001, anfr({ person: 'Max Beispiel' }), ABG('stelle.eingabe_ungueltig')),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: [1] }), ABG('stelle.eingabe_ungueltig')),
    st('hub', 'mcp.anfragen', 1003, anfr({ server: 'fremd' }), ABG('stelle.server_unbekannt')),
    st('hub', 'mcp.anfragen', 1004, anfr({ sitzung: 's-9' }), ABG('stelle.sitzung_unbekannt')),
    st('hub', 'mcp.anfragen', 1005, anfr({ frist_ms: 60001 }), ABG('stelle.frist_ungueltig')),
    st('hub', 'mcp.anfragen', 1006, anfr({ frist_ms: 0 }), ABG('stelle.eingabe_ungueltig')),
    st('hub', 'mcp.anfragen', 1007, anfr({ frist_ms: 60001, werkzeug: 'get' }), { ok: true, ergebnis: 'lesend', zeilen: [] }),
    st('hub', 'mcp.anfragen', 1008, anfr({ frist_ms: 1 }), M_OFFEN(mf(1)))],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'offen' }, zaehler: 1 });
fall('drei-schreibaufrufe-im-protokoll', ['A91'], 'Drei Schreibaufrufe: freigegeben und eingeloest, abgelehnt, abgelaufen. Drei Zeilen, je mit Server, Werkzeug, Ausgang, Zeit und dem Entscheidenden (beim Ablauf null).',
  [START1(), st('hub', 'mcp.anfragen', 1000, anfr({ argumente: { n: 1 } }), M_OFFEN(mf(1))), st('hub', 'mcp.anfragen', 1001, anfr({ argumente: { n: 2 } }), M_OFFEN(mf(2))),
    st('hub', 'mcp.anfragen', 1002, anfr({ argumente: { n: 3 } }), M_OFFEN(mf(3))),
    st('mensch', 'mcp.entscheiden', 2000, ent(mf(1), 'erteilen', H1, { argumente: { n: 1 } }), M_ERTEILT),
    st('hub', 'mcp.einloesen', 2500, loes(mf(1), { argumente: { n: 1 } }), { ok: true, ergebnis: 'eingeloest', zeilen: [ZU('eingeloest')] }),
    st('mensch', 'mcp.entscheiden', 3000, ent(mf(2), 'ablehnen', H2, { argumente: { n: 2 } }), { ok: true, ergebnis: 'abgelehnt', zeilen: [ZU('abgelehnt')] }),
    st('takt', 'ablauf.pruefen', 61002, {}, { ok: true, ergebnis: 'geprueft', abgelaufen: 1, zeilen: [ZU('abgelaufen')] })],
  { sitzungen: ['s-1'], anfragen: { [mf(1)]: 'eingeloest', [mf(2)]: 'abgelehnt', [mf(3)]: 'abgelaufen' }, zaehler: 3 });
fall('zeit-ungueltig', ['A102'], 'Eine Zeit ausserhalb 1970 bis 9999 wird ohne Zeile und ohne Zustandsaenderung abgelehnt.',
  [START1(), { ...st('kern', 'sitzung.starten', 0, startE({ ...S_CLOUD, sitzung: 's-3' }), { ok: false, code: 'stelle.zeit_ungueltig', zeilen: [] }), abs: -1 },
    { ...st('takt', 'ablauf.pruefen', 0, {}, { ok: false, code: 'stelle.zeit_ungueltig', zeilen: [] }), abs: MAX_MS + 1 }],
  { sitzungen: ['s-1'], anfragen: {}, zaehler: 0 });

// ---------- Lauf ----------

function ende(z) {
  return { sitzungen: Object.keys(z.sitzungen).sort(), anfragen: Object.fromEntries(Object.keys(z.anfragen).map((k) => [k, z.anfragen[k].zustand])), zaehler: z.zaehler };
}
const gleich = (a, b) => JSON.stringify(a) === JSON.stringify(b);

mkdirSync(AUS, { recursive: true });
for (const f of readdirSync(AUS)) if (f.endsWith('.json')) rmSync(join(AUS, f));
let zeilenGesamt = 0;
let schritteGesamt = 0;
const namen = new Set();
for (const f of FAELLE) {
  if (namen.has(f.name)) throw new Error(`Fall doppelt: ${f.name}`);
  namen.add(f.name);
  const ausgabe = { beschreibung: f.beschreibung, laden_ms: LADEN, regeln: f.regeln, einstellungen: f.laden ? { text: f.laden.text } : { text: EINST }, pruefer: { art: 'h1', schluessel_hex: SCHLUESSEL_1.toString('hex') } };
  if (f.laden) {
    const r = modellLaden(f.laden.text, LADEN);
    if (r.fehler !== f.laden.fehler) throw new Error(`${f.name}: Laden ergab ${r.fehler ?? 'ok'}, Absicht ${f.laden.fehler}`);
    ausgabe.erwartet_laden = { fehler: f.laden.fehler, grund: GRUENDE[f.laden.fehler] };
  } else {
    let z = modellLaden(EINST, LADEN).z;
    let vorher = NULL64;
    ausgabe.schritte = [];
    const alleZeilen = [];
    for (const [n, s] of f.schritte.entries()) {
      if (s.neustart) {
        z = modellLaden(EINST, s.neustart.laden_ms).z;
        ausgabe.schritte.push({ neustart: { laden_ms: s.neustart.laden_ms } });
        continue;
      }
      const wo = `${f.name} Schritt ${n + 1}`;
      if (!ARTEN.includes(s.aufrufer) || !VORGAENGE.includes(s.vorgang)) throw new Error(`${wo}: Aufrufer oder Vorgang unbekannt`);
      const jetzt = s.abs !== undefined ? s.abs : T0 + s.t;
      const vorZ = JSON.stringify(z);
      const r = entscheide(z, { aufrufer: s.aufrufer, vorgang: s.vorgang, eingabe: s.eingabe }, jetzt);
      if (JSON.stringify(z) !== vorZ) throw new Error(`${wo}: entscheide hat den alten Zustand veraendert`);
      const a = s.absicht;
      for (const k of ['ok', 'code', 'ergebnis', 'anfrage', 'route_art', 'abgelaufen', 'frist_bis_ms', 'einloesen_bis_ms', 'zustand'].filter((k) => k in a)) {
        if (!gleich(r.antwort[k], a[k])) throw new Error(`${wo}: Antwort ${k} = ${JSON.stringify(r.antwort[k])}, Absicht ${JSON.stringify(a[k])}`);
      }
      if ('offene_ids' in a && !gleich(r.antwort.offene.map((o) => o.anfrage), a.offene_ids)) throw new Error(`${wo}: offene ${JSON.stringify(r.antwort.offene.map((o) => o.anfrage))}, Absicht ${JSON.stringify(a.offene_ids)}`);
      for (const q of Object.values(r.z.anfragen)) if (q.zustand !== 'offen' && q.vorschau !== null) throw new Error(`${wo}: Vorschau nach dem Ende der Offenheit im Zustand`);
      if (!a.ok && r.antwort.grund !== GRUENDE[a.code]) throw new Error(`${wo}: Grundtext falsch`);
      const arten = r.zeilen.map((w) => `${w.aktionsname}:${w.ergebnis}`);
      if (!gleich(arten, a.zeilen)) throw new Error(`${wo}: Zeilenarten ${JSON.stringify(arten)}, Absicht ${JSON.stringify(a.zeilen)}`);
      const texte = [];
      for (const w of r.zeilen) {
        const text = jcs({ ...w, fassung: 2, vorher, zeit: zeitText(r.zeit_ms) });
        vorher = zeilePruefen(text, vorher);
        for (const m of MARKER) if (text.includes(m)) throw new Error(`${wo}: Inhalt in der Zeile: ${m}`);
        if (text.includes('Beispiel ') || text.includes('Max ') || text.includes('/home/')) throw new Error(`${wo}: Name oder Pfad in der Zeile`);
        texte.push({ text, sha256: vorher });
        alleZeilen.push(text);
      }
      z = r.z;
      ausgabe.schritte.push({ aufrufer: s.aufrufer, vorgang: s.vorgang, jetzt_ms: jetzt, eingabe: s.eingabe, erwartet: { antwort: r.antwort, zeilen: texte } });
      schritteGesamt += 1;
    }
    const e = ende(z);
    if (!gleich(e, f.ende)) throw new Error(`${f.name}: Endzustand ${JSON.stringify(e)}, Absicht ${JSON.stringify(f.ende)}`);
    ausgabe.ende = { ...e, zeilen: alleZeilen.length, letzte: alleZeilen.length ? vorher : null };
    zeilenGesamt += alleZeilen.length;
    if (f.name === 'start-cloud-gueltig') {
      const gold = `{"aktionsname":"sitzung.gestartet","ergebnis":"gestartet","fassung":2,"grund":null,"harness":"claude","kunde":"k-beispiel","route":"cloud-beispiel","sitzung":"s-1","vorher":"${NULL64}","zeit":"2026-11-20T13:00:00.000Z"}`;
      if (alleZeilen[0] !== gold) throw new Error('Handgeschriebene Zeile weicht ab');
    }
  }
  writeFileSync(join(AUS, `${f.name}.json`), `${JSON.stringify(ausgabe, null, 2)}\n`);
}
console.log(`ok: ${FAELLE.length} Faelle, ${schritteGesamt} Schritte, ${zeilenGesamt} Zeilen`);

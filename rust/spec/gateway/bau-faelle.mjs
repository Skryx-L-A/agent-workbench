// Erzeugt die sichtbaren Faelle von Vertrag 7 (README.md in diesem Ordner) nach faelle/ und die Beispiel-Registry registry.beispiel.json.
// Bytes der Kostenzeile aus der JCS-Referenz (RFC 8785) und node:crypto. Ein eigenes Referenzmodell in diesem Skript rechnet
// jeden Fall nach; weicht es von der Absicht im Fall ab, bricht das Skript mit Exit 1 ab und schreibt nichts.
// Aufruf: node rust/spec/gateway/bau-faelle.mjs (leicht, ohne Abhaengigkeit). Gleicher Stand ergibt gleiche Bytes.
// Mit --pruefen schreibt das Skript nichts, sondern vergleicht die Dateien auf der Platte Byte fuer Byte mit dem neu erzeugten Stand (Exit 0 gleich, 1 verschieden).

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs } from '../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const AUS = join(HIER, 'faelle');
const KETTE_VEK = join(HIER, '../../../spec/protokoll/vektoren/kette-vektoren.jsonl');

const NULL64 = '0'.repeat(64);
const KENNUNG = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const MODELL = /^[A-Za-z0-9._:/@+-]{1,128}$/;
const HEX = /^[0-9a-f]{64}$/;
const WAEHRUNG = /^[A-Z]{3}$/;
const SCHEIN = /^wbs_[A-Za-z0-9_-]{43}$/;
const SID = /^S-1-15-2(-[0-9]{1,10}){7}$/;
const ZONENGRUND = ['nutzer-fehlt', 'route-ohne-zone', 'zone-klasse', 'zone-gesperrt', 'zone-nachrangig', 'regel-nutzer', 'regel-gruppe'];
const ZIEL_HTTPS = /^https:\/\/([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*)(?::([0-9]{1,5}))?$/;
const ZIEL_HTTP = /^http:\/\/(127\.0\.0\.1|\[::1\])(?::([0-9]{1,5}))?$/;
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const U32 = 4294967295;
const PREIS_MAX = 1000000;
const REG_MAX = 65536;
const KOERPER_MAX = { chat: 2 * 1024 * 1024, messages: 32 * 1024 * 1024 };
const PFADE = { '/gw/v1/modelle/chat/completions': 'chat', '/gw/v1/modelle/v1/messages': 'messages' };
const ANBIETER_PFAD = { chat: '/v1/chat/completions', messages: '/v1/messages' };
const FELDER = {
  chat: ['model', 'messages', 'stream', 'stream_options', 'temperature', 'top_p', 'max_tokens', 'max_completion_tokens', 'stop', 'n',
    'presence_penalty', 'frequency_penalty', 'logit_bias', 'logprobs', 'top_logprobs', 'tools', 'tool_choice', 'parallel_tool_calls',
    'response_format', 'seed', 'user', 'functions', 'function_call', 'reasoning_effort'],
  messages: ['model', 'messages', 'system', 'max_tokens', 'stop_sequences', 'stream', 'temperature', 'top_p', 'top_k', 'tools', 'tool_choice',
    'thinking', 'metadata', 'output_config', 'context_management'],
};
const ZEILE_FELDER = ['aktionsname', 'anbieter', 'einheit', 'ergebnis', 'fassung', 'grund', 'kosten', 'kunde', 'modell', 'route', 'sitzung',
  'status', 'token_aus', 'token_ein', 'vorher', 'waehrung', 'zeit'];
const ERGEBNISSE = ['ok', 'ohne_angabe', 'anbieter_fehler'];
// Gruende der Kostenzeile (Abschnitt 6): nur bei anbieter_fehler, sonst null
const GRUENDE = ['gw.anbieter_fehler', 'gw.halter', 'gw.frist'];

const sha = (b) => createHash('sha256').update(typeof b === 'string' ? Buffer.from(b, 'utf8') : b).digest('hex');
const scheinVon = (name) => 'wbs_' + createHash('sha256').update(`schein-${name}`).digest('base64url').slice(0, 43);
class Fehler extends Error {}
const fehler = (code) => new Fehler(code);
const abbruch = (satz) => { process.stderr.write(`bau-faelle: ${satz}\n`); process.exit(1); };

// --- strenges JSON-Lesen (RFC 8259): UTF-8 ohne BOM, doppelte Schluessel auf jeder Ebene verboten, Zahlen mit Ganzzahl-Merkmal ---

class Zahl { constructor(wert, ganz) { this.wert = wert; this.ganz = ganz; } }

function jsonLesen(bytes) {
  const b = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes, 'utf8');
  if (b.length >= 3 && b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) throw fehler('json');
  let t;
  try { t = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(b); } catch { throw fehler('json'); }
  let i = 0;
  const ws = () => { while (i < t.length && ' \t\n\r'.includes(t[i])) i++; };
  const wert = () => {
    ws();
    const c = t[i];
    if (c === '{') {
      i++; const o = new Map(); ws();
      if (t[i] === '}') { i++; return o; }
      for (;;) {
        ws(); if (t[i] !== '"') throw fehler('json');
        const k = text(); ws(); if (t[i] !== ':') throw fehler('json'); i++;
        if (o.has(k)) throw fehler('json');
        o.set(k, wert()); ws();
        if (t[i] === ',') { i++; continue; }
        if (t[i] === '}') { i++; return o; }
        throw fehler('json');
      }
    }
    if (c === '[') {
      i++; const a = []; ws();
      if (t[i] === ']') { i++; return a; }
      for (;;) {
        a.push(wert()); ws();
        if (t[i] === ',') { i++; continue; }
        if (t[i] === ']') { i++; return a; }
        throw fehler('json');
      }
    }
    if (c === '"') return text();
    for (const [w, v] of [['true', true], ['false', false], ['null', null]]) if (t.startsWith(w, i)) { i += w.length; return v; }
    const m = /^-?(?:0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?/.exec(t.slice(i));
    if (!m || m[0] === '' || m[0] === '-') throw fehler('json');
    i += m[0].length;
    const v = Number(m[0]);
    if (!Number.isFinite(v)) throw fehler('json');
    return new Zahl(v, m[1] === undefined && m[2] === undefined);
  };
  const text = () => {
    i++; let s = '';
    for (;;) {
      if (i >= t.length) throw fehler('json');
      const c = t[i++];
      if (c === '"') return s;
      if (c < ' ') throw fehler('json');
      if (c !== '\\') { s += c; continue; }
      const e = t[i++];
      const einfach = { '"': '"', '\\': '\\', '/': '/', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t' };
      if (e in einfach) { s += einfach[e]; continue; }
      if (e !== 'u' || !/^[0-9a-fA-F]{4}$/.test(t.slice(i, i + 4))) throw fehler('json');
      s += String.fromCharCode(parseInt(t.slice(i, i + 4), 16)); i += 4;
    }
  };
  const v = wert(); ws();
  if (i !== t.length) throw fehler('json');
  return v;
}

const istObj = (v) => v instanceof Map;
const ganz = (v) => v instanceof Zahl && v.ganz && Number.isSafeInteger(v.wert);

// --- Registry (Abschnitt 3, Annahme R): Lader mit Stufen ---

const REG_FORM = {
  oben: { pflicht: ['fassung', 'waehrung', 'einheit', 'optionen', 'anbieter', 'routen'], frei: [] },
  optionen: { pflicht: ['kosten_mitschreiben', 'kostenauswertung', 'modellwahl_nach_preis'], frei: [] },
  anbieter: { pflicht: ['schluessel', 'art', 'protokoll', 'ziel', 'halter'], frei: [] },
  route: { pflicht: ['route', 'modelle'], frei: [] },
  modell: { pflicht: ['modell', 'anbieter', 'stufe', 'preis_ein', 'preis_aus'], frei: [] },
};

function regLesen(bytes) {
  const b = Buffer.from(bytes, 'utf8');
  if (b.length > REG_MAX) throw fehler('reg.zu_gross');
  let o;
  try { o = jsonLesen(b); } catch { throw fehler('reg.json_ungueltig'); }
  if (!istObj(o)) throw fehler('reg.json_ungueltig');
  // Ebenen, soweit die Container den erwarteten Typ haben (falsche Typen faengt Stufe 5)
  const ebenen = [];
  const sammeln = (obj, form) => { if (istObj(obj)) ebenen.push([obj, REG_FORM[form]]); };
  sammeln(o, 'oben');
  sammeln(o.get('optionen'), 'optionen');
  if (Array.isArray(o.get('anbieter'))) for (const a of o.get('anbieter')) sammeln(a, 'anbieter');
  if (Array.isArray(o.get('routen'))) for (const r of o.get('routen')) {
    sammeln(r, 'route');
    if (istObj(r) && Array.isArray(r.get('modelle'))) for (const m of r.get('modelle')) sammeln(m, 'modell');
  }
  for (const [obj, f] of ebenen) for (const k of obj.keys()) if (!f.pflicht.includes(k) && !f.frei.includes(k)) throw fehler('reg.schluessel_unbekannt');
  for (const [obj, f] of ebenen) for (const k of f.pflicht) if (!obj.has(k)) throw fehler('reg.schluessel_fehlt');
  // Stufe 5: Typen
  const typ = (ok) => { if (!ok) throw fehler('reg.typ_falsch'); };
  typ(ganz(o.get('fassung')) && typeof o.get('waehrung') === 'string' && ganz(o.get('einheit')));
  typ(istObj(o.get('optionen')) && Array.isArray(o.get('anbieter')) && Array.isArray(o.get('routen')));
  for (const k of REG_FORM.optionen.pflicht) typ(typeof o.get('optionen').get(k) === 'boolean');
  for (const a of o.get('anbieter')) {
    typ(istObj(a));
    for (const k of ['schluessel', 'art', 'protokoll']) typ(typeof a.get(k) === 'string');
    for (const k of ['ziel', 'halter']) typ(a.get(k) === null || typeof a.get(k) === 'string');
  }
  for (const r of o.get('routen')) {
    typ(istObj(r) && typeof r.get('route') === 'string' && Array.isArray(r.get('modelle')));
    for (const m of r.get('modelle')) {
      typ(istObj(m));
      for (const k of ['modell', 'anbieter', 'stufe']) typ(typeof m.get(k) === 'string');
      typ(ganz(m.get('preis_ein')) && ganz(m.get('preis_aus')));
    }
  }
  if (o.get('fassung').wert !== 1) throw fehler('reg.fassung_unbekannt');
  const an = o.get('anbieter'); const ro = o.get('routen');
  if (an.length < 1 || an.length > 32 || ro.length < 1 || ro.length > 64) throw fehler('reg.anzahl');
  for (const r of ro) if (r.get('modelle').length < 1 || r.get('modelle').length > 16) throw fehler('reg.anzahl');
  const kenn = (v) => { if (!KENNUNG.test(v)) throw fehler('reg.kennung_form'); };
  for (const a of an) { kenn(a.get('schluessel')); if (a.get('halter') !== null) kenn(a.get('halter')); }
  for (const r of ro) { kenn(r.get('route')); for (const m of r.get('modelle')) { kenn(m.get('anbieter')); kenn(m.get('stufe')); } }
  for (const r of ro) for (const m of r.get('modelle')) if (!MODELL.test(m.get('modell'))) throw fehler('reg.modell_form');
  for (const a of an) if (!['cloud', 'lokal'].includes(a.get('art'))) throw fehler('reg.art_unbekannt');
  for (const a of an) if (!['chat', 'messages'].includes(a.get('protokoll'))) throw fehler('reg.protokoll_unbekannt');
  for (const a of an) if ((a.get('ziel') === null) === (a.get('halter') === null)) throw fehler('reg.weg_form');
  for (const a of an) if (a.get('art') === 'cloud' && a.get('halter') === null) throw fehler('reg.halter_pflicht');
  for (const a of an) {
    const z = a.get('ziel');
    if (z === null) continue;
    const h = ZIEL_HTTPS.exec(z); const l = ZIEL_HTTP.exec(z);
    const port = h ? h[2] : l ? l[2] : undefined;
    const gut = (h && h[1].length <= 253) || (l && a.get('art') === 'lokal');
    if (!gut || (port !== undefined && (Number(port) < 1 || Number(port) > 65535 || /^0/.test(port)))) throw fehler('reg.ziel_form');
  }
  if (!WAEHRUNG.test(o.get('waehrung'))) throw fehler('reg.waehrung_form');
  if (o.get('einheit').wert < 0 || o.get('einheit').wert > 9) throw fehler('reg.einheit_grenze');
  for (const r of ro) for (const m of r.get('modelle')) for (const k of ['preis_ein', 'preis_aus']) {
    if (m.get(k).wert < 0 || m.get(k).wert > PREIS_MAX) throw fehler('reg.preis_grenze');
  }
  const doppelt = (liste) => new Set(liste).size !== liste.length;
  if (doppelt(an.map((a) => a.get('schluessel'))) || doppelt(ro.map((r) => r.get('route')))) throw fehler('reg.kennung_doppelt');
  for (const r of ro) if (doppelt(r.get('modelle').map((m) => m.get('modell')))) throw fehler('reg.kennung_doppelt');
  const anb = new Map(an.map((a) => [a.get('schluessel'), a]));
  for (const r of ro) for (const m of r.get('modelle')) if (!anb.has(m.get('anbieter'))) throw fehler('reg.anbieter_unbekannt');
  return {
    waehrung: o.get('waehrung'), einheit: o.get('einheit').wert,
    optionen: Object.fromEntries([...o.get('optionen')]),
    anbieter: new Map([...anb].map(([k, a]) => [k, { schluessel: k, art: a.get('art'), protokoll: a.get('protokoll'), ziel: a.get('ziel'), halter: a.get('halter') }])),
    routen: new Map(ro.map((r) => [r.get('route'), r.get('modelle').map((m) => ({
      modell: m.get('modell'), anbieter: m.get('anbieter'), stufe: m.get('stufe'), preis_ein: m.get('preis_ein').wert, preis_aus: m.get('preis_aus').wert,
    }))])),
  };
}

// --- Kern: entscheide(lage, anfrage) (Abschnitt 2) ---

function peerGleich(erwartet, gemessen) {
  if (gemessen.art !== erwartet.art) return false;
  if (erwartet.art === 'unix') return gemessen.socket === erwartet.socket && gemessen.uid === erwartet.uid;
  // macOS (A143): die Verbindung gehoert einer PID, die das Merkmal der Sitzung traegt
  if (erwartet.art === 'mac') return gemessen.sitzung === erwartet.sitzung && gemessen.merkmal === true && gemessen.uid === erwartet.uid;
  return gemessen.sid === erwartet.sid && gemessen.job === erwartet.job && gemessen.lauf === erwartet.lauf && gemessen.im_job === true;
}

function entscheide(lage, anfrage) {
  if (lage.einstellungen.fehler) throw fehler('gw.einstellungen_ungueltig');
  let reg;
  try { reg = regLesen(lage.registry_text); } catch { throw fehler('gw.registry_ungueltig'); }
  const a = anfrage.authorization;
  if (typeof a !== 'string' || !a.startsWith('Bearer ')) throw fehler('gw.schein_fehlt');
  const schein = a.slice(7);
  const lauf = SCHEIN.test(schein) ? lage.laeufe.find((l) => l.schein_sha256 === sha(schein)) : undefined;
  if (!lauf) throw fehler('gw.schein_unbekannt');
  if (anfrage.verbindung.art === 'unbekannt') throw fehler('gw.peer_unbekannt');
  if (!peerGleich(lauf.peer, anfrage.verbindung)) throw fehler('gw.peer_falsch');
  const protokoll = PFADE[anfrage.pfad];
  if (!protokoll) throw fehler('gw.pfad_unbekannt');
  const route = lage.einstellungen.routen.find((r) => r.schluessel === lauf.route);
  if (!route || typeof route.klasse !== 'string') throw fehler('gw.route_ohne_klasse');
  // Modellzonen (A127 bis A129): die Entscheidung trifft das Tor, das Gateway liest nur das Ergebnis der Laufpruefung
  const zonen = lage.zonen ?? { an: false };
  if (zonen.an) {
    const z = zonen.routen[lauf.route];
    if (!z || z.zone === null) throw fehler('gw.route_ohne_zone');
    if (z.lauf_frei !== true) throw fehler('gw.zone_gesperrt');
  }
  const kandidaten = reg.routen.get(lauf.route);
  const eintrag = kandidaten && kandidaten.find((m) => m.modell === lauf.modell);
  if (!eintrag) throw fehler('gw.registry_fehlt');
  let wahl = eintrag;
  if (reg.optionen.modellwahl_nach_preis) {
    const gleich = kandidaten.filter((m) => m.stufe === eintrag.stufe);
    wahl = gleich.reduce((best, m) => (m.preis_ein + m.preis_aus < best.preis_ein + best.preis_aus ? m : best), gleich[0]);
  }
  const anb = reg.anbieter.get(wahl.anbieter);
  if (anb.art !== route.art) throw fehler('gw.art_unvertraeglich');
  if (anb.protokoll !== protokoll) throw fehler('gw.protokoll_falsch');
  const koerper = Buffer.from(anfrage.koerper_text, 'utf8');
  if (koerper.length > KOERPER_MAX[protokoll]) throw fehler('gw.koerper_zu_gross');
  let k;
  try { k = jsonLesen(koerper); } catch { throw fehler('gw.json_ungueltig'); }
  if (!istObj(k)) throw fehler('gw.json_ungueltig');
  for (const f of k.keys()) if (!FELDER[protokoll].includes(f)) throw fehler('gw.feld_unbekannt');
  return {
    anbieter: anb.schluessel, einheit: reg.einheit, halter: anb.halter, kosten_mitschreiben: reg.optionen.kosten_mitschreiben,
    kunde: lauf.kunde, modell: wahl.modell, preis_aus: wahl.preis_aus, preis_ein: wahl.preis_ein, protokoll, route: lauf.route,
    sitzung: lauf.sitzung, strom: k.get('stream') === true, waehrung: reg.waehrung, weg: anb.halter === null ? 'direkt' : 'halter',
    pfad: ANBIETER_PFAD[protokoll], ziel: anb.ziel === null ? null : anb.ziel + ANBIETER_PFAD[protokoll],
  };
}

// --- Zaehlung: tokens_aus(protokoll, strom, antwort) (Abschnitt 5) ---

const tokZahl = (v) => (v instanceof Zahl && v.ganz && v.wert >= 0 && v.wert <= U32 ? v.wert : null);
function tokensAus(protokoll, strom, antwort) {
  const ergebnis = (ein, aus) => (ein === null || aus === null || ein > U32 || aus > U32 ? null : { token_aus: aus, token_ein: ein });
  const lesen = (s) => { try { return jsonLesen(s); } catch { return undefined; } };
  if (protokoll === 'chat') {
    let usage;
    if (!strom) { const o = lesen(antwort); usage = istObj(o) ? o.get('usage') : undefined; } else {
      for (const z of antwort.split('\n')) {
        if (!z.startsWith('data: ') || z === 'data: [DONE]') continue;
        const o = lesen(z.slice(6));
        if (istObj(o) && istObj(o.get('usage'))) usage = o.get('usage');
      }
    }
    if (!istObj(usage)) return null;
    return ergebnis(tokZahl(usage.get('prompt_tokens')), tokZahl(usage.get('completion_tokens')));
  }
  const stand = new Map();
  const mischen = (u) => { if (istObj(u)) for (const f of ['input_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens', 'output_tokens']) if (u.has(f)) stand.set(f, u.get(f)); };
  if (!strom) { const o = lesen(antwort); if (istObj(o)) mischen(o.get('usage')); } else {
    for (const z of antwort.split('\n')) {
      if (!z.startsWith('data: ')) continue;
      const o = lesen(z.slice(6));
      if (!istObj(o)) continue;
      if (o.get('type') === 'message_start' && istObj(o.get('message'))) mischen(o.get('message').get('usage'));
      if (o.get('type') === 'message_delta') mischen(o.get('usage'));
    }
  }
  const ein = tokZahl(stand.get('input_tokens'));
  const aus = tokZahl(stand.get('output_tokens'));
  if (ein === null || aus === null) return null;
  const extra = ['cache_creation_input_tokens', 'cache_read_input_tokens'].map((f) => (stand.has(f) ? tokZahl(stand.get(f)) : 0));
  if (extra.includes(null)) return null;
  return ergebnis(ein + extra[0] + extra[1], aus);
}

// --- Kostenzeile (Abschnitt 6): zeile_aufruf(&Aufruf, vorher) ---

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw fehler('zeit_ungueltig');
  return new Date(ms).toISOString();
}

function zeileAufruf(a, vorher) {
  const f = (feld, ok) => { if (!ok) throw fehler(`feld_ungueltig:${feld}`); };
  const ganzIn = (v, max) => Number.isInteger(v) && v >= 0 && v <= max;
  f('route', KENNUNG.test(a.route));
  f('sitzung', KENNUNG.test(a.sitzung));
  f('kunde', a.kunde === null || KENNUNG.test(a.kunde));
  f('anbieter', KENNUNG.test(a.anbieter));
  f('modell', MODELL.test(a.modell));
  f('ergebnis', ERGEBNISSE.includes(a.ergebnis));
  const fehlschlag = a.ergebnis === 'anbieter_fehler';
  f('grund', fehlschlag ? GRUENDE.includes(a.grund) : a.grund === null);
  f('status', a.status === null ? fehlschlag : ganzIn(a.status, 599) && a.status >= 100);
  const mitTokens = a.ergebnis === 'ok';
  f('token_ein', mitTokens ? ganzIn(a.token_ein, U32) : a.token_ein === null);
  f('token_aus', mitTokens ? ganzIn(a.token_aus, U32) : a.token_aus === null);
  f('preis_ein', ganzIn(a.preis_ein, PREIS_MAX));
  f('preis_aus', ganzIn(a.preis_aus, PREIS_MAX));
  f('waehrung', WAEHRUNG.test(a.waehrung));
  f('einheit', ganzIn(a.einheit, 9));
  f('vorher', HEX.test(vorher));
  const zeit = zeitText(a.zeit_ms);
  const kosten = mitTokens ? a.token_ein * a.preis_ein + a.token_aus * a.preis_aus : null;
  const zeile = jcs({
    aktionsname: 'modell.aufruf', anbieter: a.anbieter, einheit: a.einheit, ergebnis: a.ergebnis, fassung: 2, grund: a.grund, kosten,
    kunde: a.kunde, modell: a.modell, route: a.route, sitzung: a.sitzung, status: a.status, token_aus: a.token_aus, token_ein: a.token_ein,
    vorher, waehrung: a.waehrung, zeit,
  });
  return zeile;
}

// Unabhaengige Gegenrechnung der Zeile ohne jcs: feste Feldfolge, nur Werte ohne Escape. Weicht sie ab, bricht das Skript ab.
function zeileVonHand(a, vorher) {
  const s = (v) => (v === null ? 'null' : typeof v === 'number' ? String(v) : `"${v}"`);
  const kosten = a.ergebnis === 'ok' ? a.token_ein * a.preis_ein + a.token_aus * a.preis_aus : null;
  const w = { aktionsname: 'modell.aufruf', anbieter: a.anbieter, einheit: a.einheit, ergebnis: a.ergebnis, fassung: 2, grund: a.grund,
    kosten, kunde: a.kunde, modell: a.modell, route: a.route, sitzung: a.sitzung, status: a.status, token_aus: a.token_aus,
    token_ein: a.token_ein, vorher, waehrung: a.waehrung, zeit: new Date(a.zeit_ms).toISOString() };
  return `{${ZEILE_FELDER.map((k) => `"${k}":${s(w[k])}`).join(',')}}`;
}

// --- Ausgangslagen ---

// Einstellungsdatei der Faelle (Vertrag 2, Fassung 2): eigene Beispieldatei dieses Vertrags, nicht die Vorfuehrdatei.
// Grund: Die Vorfuehrdatei traegt die echten Routen- und Modellnamen des Durchstichs und aendert sich mit ihm; die Faelle
// dieses Vertrags brauchen feste, neutrale Namen (Vertrag 8, Abschnitt 1: echte Namen nur in der Vorfuehrdatei).
const EINST_BASIS = {
  fassung: 2,
  klassen: ['oeffentlich', 'intern', 'kundendaten'],
  vorgabestufe: 'kundendaten',
  routen: [
    { schluessel: 'cloud-beispiel', art: 'cloud', klasse: 'intern', netzziele: [] },
    { schluessel: 'lokal-beispiel', art: 'lokal', klasse: 'kundendaten', ziel: 'http://127.0.0.1:11434/v1', modell: 'lokal-beispiel/modell-a', netzziele: [] },
  ],
  kunden: [{ schluessel: 'k-beispiel', erlaubt: ['lokal-beispiel'], verboten: [] }],
  kundendaten_an_cloud: { vorgabe: 'nie', freigaben: [] },
};
const einstText = (e) => JSON.stringify(e, null, 1) + '\n';
const EINST_OK = { text: einstText(EINST_BASIS), routen: EINST_BASIS.routen };
// Einstellungen mit klasse null an der Cloud-Route: der Lader von Vertrag 2 lehnt die Datei ab (einst.route_ohne_klasse)
const einstNull = { ...EINST_BASIS, routen: EINST_BASIS.routen.map((r) => (r.art === 'cloud' ? { ...r, klasse: null } : r)) };
const EINST_FEHLER = { text: einstText(einstNull), fehler: 'einst.route_ohne_klasse' };
// Einstellungen ohne die Cloud-Route (Admin hat sie nach dem Start entfernt): gueltig, aber die Route der Sitzung fehlt (A42).
const einstOhneCloud = { ...EINST_BASIS, routen: EINST_BASIS.routen.filter((r) => r.art !== 'cloud') };
const EINST_OHNE_CLOUD = { text: einstText(einstOhneCloud), routen: einstOhneCloud.routen };

// Sicht des Tors auf die Modellzonen (A127 bis A129, rust/spec/datentor/zonen/VERTRAG.md): das Ergebnis der Laufpruefung
// (darf_zonen mit einheit null) je Route, das der Kern-Dienst dem Gateway in die Lage gibt. Das Gateway entscheidet nichts selbst.
const ZONEN_LAGEN = {
  aus: { an: false },
  an: { an: true, routen: {
    'cloud-beispiel': { zone: 'z-cloud', lauf_frei: true, grund: null },
    'lokal-beispiel': { zone: 'z-lokal', lauf_frei: true, grund: null } } },
  cloud_ohne_zone: { an: true, routen: {
    'cloud-beispiel': { zone: null, lauf_frei: false, grund: 'route-ohne-zone' },
    'lokal-beispiel': { zone: 'z-lokal', lauf_frei: true, grund: null } } },
  cloud_gesperrt: { an: true, routen: {
    'cloud-beispiel': { zone: 'z-unsicher', lauf_frei: false, grund: 'zone-gesperrt' },
    'lokal-beispiel': { zone: 'z-lokal', lauf_frei: true, grund: null } } },
};
for (const [n, z] of Object.entries(ZONEN_LAGEN)) {
  if (!z.an) continue;
  for (const [r, w] of Object.entries(z.routen)) {
    if (!KENNUNG.test(r)) abbruch(`Zonenlage ${n}: Route ${r} nicht in Kennungsform`);
    if (!(w.zone === null || KENNUNG.test(w.zone))) abbruch(`Zonenlage ${n}: Zone nicht in Kennungsform`);
    if (!(w.grund === null || ZONENGRUND.includes(w.grund))) abbruch(`Zonenlage ${n}: Grund ${w.grund} unbekannt`);
  }
}

const ANB_CLOUD = { schluessel: 'anb-cloud-a', art: 'cloud', protokoll: 'messages', ziel: null, halter: 'anb-cloud-a' };
const ANB_CLOUD_B = { schluessel: 'anb-cloud-b', art: 'cloud', protokoll: 'messages', ziel: null, halter: 'anb-cloud-b' };
const ANB_LOKAL = { schluessel: 'anb-lokal', art: 'lokal', protokoll: 'chat', ziel: 'http://127.0.0.1:11434', halter: null };
const OPT = { kosten_mitschreiben: true, kostenauswertung: false, modellwahl_nach_preis: false };
const regText = (r) => JSON.stringify(r, null, 1) + '\n';
const REG_BSP = {
  fassung: 1, waehrung: 'EUR', einheit: 9, optionen: OPT,
  anbieter: [ANB_CLOUD, ANB_LOKAL],
  routen: [
    { route: 'cloud-beispiel', modelle: [{ modell: 'cloud-beispiel/modell-a', anbieter: 'anb-cloud-a', stufe: 'stufe-1', preis_ein: 3000, preis_aus: 15000 }] },
    { route: 'lokal-beispiel', modelle: [{ modell: 'lokal-beispiel/modell-a', anbieter: 'anb-lokal', stufe: 'stufe-1', preis_ein: 0, preis_aus: 0 }] },
  ],
};
const mitRoute = (r, route, modelle) => ({ ...r, routen: r.routen.map((x) => (x.route === route ? { route, modelle } : x)) });
const REGS = {
  beispiel: REG_BSP,
  // derselbe Stand, der Admin hat die Cloud-Route auf Anbieter B umgestellt (A13)
  anbieter_b: { ...REG_BSP, anbieter: [ANB_CLOUD_B, ANB_LOKAL],
    routen: REG_BSP.routen.map((x) => (x.route === 'cloud-beispiel' ? { route: x.route, modelle: [{ ...x.modelle[0], anbieter: 'anb-cloud-b' }] } : x)) },
  // Anbieter B eingestellt, die Route hat aber keinen Eintrag fuer das Modell der Sitzung: kein Rueckfall auf A
  anbieter_b_ohne_modell: { ...REG_BSP, anbieter: [ANB_CLOUD_B, ANB_LOKAL],
    routen: REG_BSP.routen.map((x) => (x.route === 'cloud-beispiel' ? { route: x.route, modelle: [{ ...x.modelle[0], modell: 'anderes-modell', anbieter: 'anb-cloud-b' }] } : x)) },
  ohne_cloud_route: { ...REG_BSP, routen: REG_BSP.routen.filter((x) => x.route !== 'cloud-beispiel') },
  // die lokale Route zeigt faelschlich auf einen Cloud-Anbieter
  lokal_auf_cloud: mitRoute(REG_BSP, 'lokal-beispiel', [{ modell: 'lokal-beispiel/modell-a', anbieter: 'anb-cloud-a', stufe: 'stufe-1', preis_ein: 0, preis_aus: 0 }]),
  // Stufen fuer A87: M1 teuer und zuerst gelistet, M2 kostenfrei in derselben Stufe, M3 kostenfrei in einer anderen Stufe (nie Kandidat)
  stufen_aus: mitRoute(REG_BSP, 'cloud-beispiel', [
    { modell: 'cloud-beispiel/modell-a', anbieter: 'anb-cloud-a', stufe: 'stufe-1', preis_ein: 3000, preis_aus: 15000 },
    { modell: 'm2-frei', anbieter: 'anb-cloud-a', stufe: 'stufe-1', preis_ein: 0, preis_aus: 0 },
    { modell: 'm3-tief', anbieter: 'anb-cloud-a', stufe: 'stufe-2', preis_ein: 0, preis_aus: 0 },
  ]),
  ungueltig: { ...REG_BSP, fassung: 2 },
};
REGS.stufen_an = { ...REGS.stufen_aus, optionen: { ...OPT, modellwahl_nach_preis: true } };
REGS.kosten_aus = { ...REG_BSP, optionen: { ...OPT, kosten_mitschreiben: false } };
const REG_TEXTE = Object.fromEntries(Object.entries(REGS).map(([k, v]) => [k, regText(v)]));

const UID = 1000;
const WIN_SID = 'S-1-15-2-1-2-3-4-5-6-7';
const LAEUFE = [
  { sitzung: 's-1', route: 'cloud-beispiel', kunde: 'k-beispiel', modell: 'cloud-beispiel/modell-a', schein_sha256: sha(scheinVon('s-1')),
    peer: { art: 'unix', socket: 's-1', uid: UID } },
  { sitzung: 's-2', route: 'lokal-beispiel', kunde: 'k-beispiel', modell: 'lokal-beispiel/modell-a', schein_sha256: sha(scheinVon('s-2')),
    peer: { art: 'unix', socket: 's-2', uid: UID } },
  { sitzung: 's-3', route: 'cloud-beispiel', kunde: null, modell: 'cloud-beispiel/modell-a', schein_sha256: sha(scheinVon('s-3')),
    peer: { art: 'tcp', sid: WIN_SID, job: 'j-3', lauf: 'l-3' } },
  { sitzung: 's-4', route: 'lokal-beispiel', kunde: 'k-beispiel', modell: 'lokal-beispiel/modell-a', schein_sha256: sha(scheinVon('s-4')),
    peer: { art: 'mac', sitzung: 's-4', uid: UID } },
];
for (const l of LAEUFE) if (!SID.test(l.peer.sid ?? WIN_SID)) abbruch('SID der Lage hat falsche Form');

const MSG = JSON.stringify({ model: 'egal', max_tokens: 16, messages: [{ role: 'user', content: 'Hallo' }] });
const CHAT = JSON.stringify({ model: 'egal', messages: [{ role: 'user', content: 'Hallo' }] });
const unix = (s) => ({ art: 'unix', socket: s, uid: UID });
const mac = (s) => ({ art: 'mac', pid: 4242, merkmal: true, sitzung: s, uid: UID });
const P_MSG = '/gw/v1/modelle/v1/messages';
const P_CHAT = '/gw/v1/modelle/chat/completions';
const anf = (schein, verbindung, pfad, koerper, extra = {}) => ({
  authorization: schein === null ? null : `Bearer ${schein}`, verbindung, pfad, koerper_text: koerper, ...extra,
});

const W_CLOUD = { anbieter: 'anb-cloud-a', einheit: 9, halter: 'anb-cloud-a', kosten_mitschreiben: true, kunde: 'k-beispiel', modell: 'cloud-beispiel/modell-a',
  preis_aus: 15000, preis_ein: 3000, protokoll: 'messages', route: 'cloud-beispiel', sitzung: 's-1', strom: false, waehrung: 'EUR', weg: 'halter',
  pfad: '/v1/messages', ziel: null };
const W_LOKAL = { anbieter: 'anb-lokal', einheit: 9, halter: null, kosten_mitschreiben: true, kunde: 'k-beispiel', modell: 'lokal-beispiel/modell-a',
  preis_aus: 0, preis_ein: 0, protokoll: 'chat', route: 'lokal-beispiel', sitzung: 's-2', strom: false, waehrung: 'EUR', weg: 'direkt',
  pfad: '/v1/chat/completions', ziel: 'http://127.0.0.1:11434/v1/chat/completions' };

// name, beschreibung, einstellungen, registry, anfrage, erwartet ({weiterleitung} oder {code}), Zusatz
const S1 = scheinVon('s-1'); const S2 = scheinVon('s-2'); const S3 = scheinVon('s-3'); const S4 = scheinVon('s-4');
const ENTSCHEIDE = [
  ['cloud_gueltig', 'Gueltiger Aufruf der Cloud-Sitzung s-1: Weg ueber den Halter an genau den Anbieter der Route', 'ok', 'beispiel', anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: W_CLOUD }],
  ['lokal_gueltig', 'Gueltiger Aufruf der lokalen Sitzung s-2: direkter Weg an das lokale Ziel auf Loopback, ohne Halter', 'ok', 'beispiel', anf(S2, unix('s-2'), P_CHAT, CHAT), { weiterleitung: W_LOKAL }],
  ['lokal_strom', 'Strom angefragt: strom true, die Huelle setzt stream_options.include_usage', 'ok', 'beispiel',
    anf(S2, unix('s-2'), P_CHAT, JSON.stringify({ model: 'lokal-beispiel/modell-a', stream: true, messages: [] })), { weiterleitung: { ...W_LOKAL, strom: true } }],
  ['modell_der_anfrage_zaehlt_nicht', 'Die Anfrage nennt ein anderes Modell; es gilt das Modell aus Start und Registry', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, JSON.stringify({ model: 'm3-tief', max_tokens: 1, messages: [] })), { weiterleitung: W_CLOUD }],
  ['windows_gueltig', 'Windows: Peer ueber den Plattformdienst mit SID, Job und Lauf des Scheins', 'ok', 'beispiel',
    anf(S3, { art: 'tcp', sid: WIN_SID, job: 'j-3', lauf: 'l-3', im_job: true }, P_MSG, MSG), { weiterleitung: { ...W_CLOUD, kunde: null, sitzung: 's-3' } }],
  // Route aus der Anfrage
  ['route_im_koerper', 'Die Anfrage der Cloud-Sitzung nennt im Koerper die lokale Route: abgelehnt, das Feld steht auf keiner Erlaubnisliste', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, JSON.stringify({ model: 'lokal-beispiel/modell-a', route: 'lokal-beispiel', max_tokens: 1, messages: [] })), { code: 'gw.feld_unbekannt' }],
  ['route_in_kopfzeile', 'Die Anfrage nennt die lokale Route in einer Kopfzeile: wirkungslos, die Huelle reicht keine Kopfzeile ausser Authorization an den Kern', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG, { kopfzeilen: { 'x-awb-route': 'lokal-beispiel', 'x-wb-route': 'lokal-beispiel' } }), { weiterleitung: W_CLOUD }],
  ['route_im_pfad', 'Ein Pfad mit Route ist kein Weg des Gateways', 'ok', 'beispiel',
    anf(S1, unix('s-1'), '/gw/v1/modelle/lokal-beispiel/v1/messages', MSG), { code: 'gw.pfad_unbekannt' }],
  // Route ohne Klasse
  ['route_ohne_klasse', 'Die Route der Sitzung steht nicht mehr in der Einstellungsdatei (A42): Route cloud-beispiel hat keine Klasse', 'ohne_cloud', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.route_ohne_klasse' }],
  ['einstellungen_route_null', 'Die Einstellungsdatei hat eine Route mit klasse null; der Lader von Vertrag 2 lehnt sie ab, das Gateway bedient keine Sitzung', 'fehler', 'beispiel',
    anf(S2, unix('s-2'), P_CHAT, CHAT), { code: 'gw.einstellungen_ungueltig' }],
  // Registry
  ['ohne_registry_eintrag', 'Die Route hat keinen Registry-Eintrag: Abbruch, kein Rueckfall', 'ok', 'ohne_cloud_route', anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.registry_fehlt' }],
  ['registry_ungueltig', 'Die Registry-Datei ist ungueltig (Fassung 2): kein Aufruf', 'ok', 'ungueltig', anf(S2, unix('s-2'), P_CHAT, CHAT), { code: 'gw.registry_ungueltig' }],
  ['lokal_auf_cloud_anbieter', 'Die lokale Route zeigt auf einen Cloud-Anbieter: abgelehnt, Kundendaten der lokalen Route gingen sonst an eine Cloud', 'ok', 'lokal_auf_cloud',
    anf(S2, unix('s-2'), P_CHAT, CHAT), { code: 'gw.art_unvertraeglich' }],
  // Zielwahl: anderer eingestellter Anbieter (A13)
  ['zielwahl_anbieter_b', 'Der Admin stellt die Cloud-Route auf Anbieter B um: derselbe Aufruf geht an B und nie an A', 'ok', 'anbieter_b',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: { ...W_CLOUD, anbieter: 'anb-cloud-b', halter: 'anb-cloud-b' }, nie_halter: 'anb-cloud-a' }],
  ['zielwahl_kein_rueckfall', 'Anbieter B eingestellt, aber ohne Eintrag fuer das Modell der Sitzung: Abbruch statt Rueckfall auf A', 'ok', 'anbieter_b_ohne_modell',
    anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.registry_fehlt' }],
  // Preis und Stufe (A87)
  ['preis_aus_erster', 'Modellwahl nach Preis aus (Vorgabe): das Modell der Sitzung, auch wenn ein guenstigeres derselben Stufe existiert', 'ok', 'stufen_aus',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: W_CLOUD }],
  ['preis_an_gleiche_stufe', 'Modellwahl nach Preis an: das kostenfreie Modell derselben Stufe, nie das der tieferen Stufe', 'ok', 'stufen_an',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: { ...W_CLOUD, modell: 'm2-frei', preis_ein: 0, preis_aus: 0 }, nie_modell: 'm3-tief' }],
  ['kosten_aus', 'Option Kosten mitschreiben aus: die Weiterleitung sagt es der Huelle, es entsteht keine Zeile', 'ok', 'kosten_aus',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: { ...W_CLOUD, kosten_mitschreiben: false } }],
  // Lauf-Schein und Peer
  ['ohne_schein', 'Keine Kopfzeile Authorization', 'ok', 'beispiel', anf(null, unix('s-1'), P_MSG, MSG), { code: 'gw.schein_fehlt' }],
  ['schein_falsche_art', 'Authorization ohne Bearer', 'ok', 'beispiel', { ...anf(S1, unix('s-1'), P_MSG, MSG), authorization: `Basic ${S1}` }, { code: 'gw.schein_fehlt' }],
  ['schein_unbekannt', 'Ein Schein der richtigen Form, den kein Lauf traegt', 'ok', 'beispiel', anf(scheinVon('fremd'), unix('s-1'), P_MSG, MSG), { code: 'gw.schein_unbekannt' }],
  ['schein_falsche_form', 'Ein Anbieterschluessel statt eines Scheins', 'ok', 'beispiel', anf('sk-beispiel-0000', unix('s-1'), P_MSG, MSG), { code: 'gw.schein_unbekannt' }],
  ['schein_beendeter_lauf', 'Der Lauf s-1 ist beendet und abgemeldet; sein Schein gilt nicht mehr', 'ok', 'beispiel', anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.schein_unbekannt' }, { ohne_lauf: 's-1' }],
  ['schein_am_fremden_socket', 'Der Schein von s-2 kommt ueber den Socket von s-1', 'ok', 'beispiel', anf(S2, unix('s-1'), P_CHAT, CHAT), { code: 'gw.peer_falsch' }],
  ['fremde_kennung', 'Der Schein kommt am richtigen Socket, aber von einer anderen Kennung', 'ok', 'beispiel', anf(S1, { art: 'unix', socket: 's-1', uid: 1001 }, P_MSG, MSG), { code: 'gw.peer_falsch' }],
  ['windows_fremder_job', 'Windows: richtige SID, anderer Job', 'ok', 'beispiel', anf(S3, { art: 'tcp', sid: WIN_SID, job: 'j-9', lauf: 'l-3', im_job: true }, P_MSG, MSG), { code: 'gw.peer_falsch' }],
  ['windows_nicht_im_job', 'Windows: Prozess nicht im Job des Laufs', 'ok', 'beispiel', anf(S3, { art: 'tcp', sid: WIN_SID, job: 'j-3', lauf: 'l-3', im_job: false }, P_MSG, MSG), { code: 'gw.peer_falsch' }],
  ['windows_peer_unbekannt', 'Windows: der Plattformdienst antwortet nicht in 500 ms oder der Client ist schon fort', 'ok', 'beispiel', anf(S3, { art: 'unbekannt' }, P_MSG, MSG), { code: 'gw.peer_unbekannt' }],
  ['schein_vor_peer', 'Ohne Schein meldet der Kern schein_fehlt, auch wenn der Peer unbekannt ist', 'ok', 'beispiel', anf(null, { art: 'unbekannt' }, P_MSG, MSG), { code: 'gw.schein_fehlt' }],
  // Koerper
  ['protokoll_falsch', 'Die Cloud-Sitzung ruft die Chat-Route, ihr Anbieter spricht Messages', 'ok', 'beispiel', anf(S1, unix('s-1'), P_CHAT, CHAT), { code: 'gw.protokoll_falsch' }],
  ['koerper_doppelt', 'Ein doppelter Schluessel in einer tiefen Ebene des Koerpers', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, '{"model":"x","max_tokens":1,"messages":[{"role":"user","role":"assistant","content":"x"}]}'), { code: 'gw.json_ungueltig' }],
  ['koerper_kein_objekt', 'Der Koerper ist eine Liste', 'ok', 'beispiel', anf(S1, unix('s-1'), P_MSG, '[]'), { code: 'gw.json_ungueltig' }],
  ['koerper_zu_gross', 'Der Koerper der Chat-Route hat 2 MiB und ein Byte', 'ok', 'beispiel',
    anf(S2, unix('s-2'), P_CHAT, `{"messages":"${'a'.repeat(2 * 1024 * 1024 - 14)}"}`), { code: 'gw.koerper_zu_gross' }],
  ['feld_unbekannt', 'Ein Feld ausserhalb der Erlaubnisliste (Ziel des Anbieters)', 'ok', 'beispiel',
    anf(S2, unix('s-2'), P_CHAT, JSON.stringify({ model: 'x', messages: [], api_base: 'https://fremd.beispiel.invalid' })), { code: 'gw.feld_unbekannt' }],
  // macOS: Peer ueber libproc, Merkmal des Laufs (A143, A145)
  ['mac_gueltig', 'macOS: die Verbindung gehoert einer PID mit dem Merkmal der Sitzung, der Schein passt', 'ok', 'beispiel',
    anf(S4, mac('s-4'), P_CHAT, CHAT), { weiterleitung: { ...W_LOKAL, sitzung: 's-4' } }],
  ['mac_ohne_merkmal', 'macOS: die PID der Verbindung traegt das Merkmal des Laufs nicht (Prozess der Person ausserhalb des Profils)', 'ok', 'beispiel',
    anf(S4, { art: 'mac', pid: 4243, merkmal: false, sitzung: null, uid: UID }, P_CHAT, CHAT), { code: 'gw.peer_falsch' }],
  ['mac_fremde_sitzung', 'macOS: die PID traegt das Merkmal einer anderen Sitzung', 'ok', 'beispiel',
    anf(S4, { art: 'mac', pid: 4244, merkmal: true, sitzung: 's-2', uid: UID }, P_CHAT, CHAT), { code: 'gw.peer_falsch' }],
  ['mac_fremde_kennung', 'macOS: Merkmal und Sitzung passen, die Kennung nicht', 'ok', 'beispiel',
    anf(S4, { art: 'mac', pid: 4245, merkmal: true, sitzung: 's-4', uid: 1001 }, P_CHAT, CHAT), { code: 'gw.peer_falsch' }],
  ['mac_peer_unbekannt', 'macOS: libproc ordnet die Verbindung keiner PID zu, oder der Client ist schon fort', 'ok', 'beispiel',
    anf(S4, { art: 'unbekannt' }, P_CHAT, CHAT), { code: 'gw.peer_unbekannt' }],
  ['mac_schein_am_fremden_port', 'macOS: der Schein von s-2 kommt ueber den Weiterreicher von s-4', 'ok', 'beispiel',
    anf(S2, mac('s-4'), P_CHAT, CHAT), { code: 'gw.peer_falsch' }],
  // Modellzonen (A127 bis A129)
  ['zonen_aus', 'Option Modellzonen aus (Vorgabe): das Gateway fragt keine Zone', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: W_CLOUD }, { zonen: 'aus' }],
  ['zonen_an_frei', 'Option an, die Route hat eine Zone und die Laufpruefung ist frei', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { weiterleitung: W_CLOUD }, { zonen: 'an' }],
  ['route_ohne_zone', 'Option an, die Route steht in keiner Zone (A127): Route cloud-beispiel hat keine Zone', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.route_ohne_zone' }, { zonen: 'cloud_ohne_zone' }],
  ['zone_gesperrt', 'Option an, die Zone der Route ist gesperrt und keine Ausnahme gilt (A128)', 'ok', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.zone_gesperrt' }, { zonen: 'cloud_gesperrt' }],
  ['zone_nach_klasse', 'Die Klasse kommt vor der Zone: eine Route ohne Klasse endet mit route_ohne_klasse, auch mit eingeschalteten Zonen', 'ohne_cloud', 'beispiel',
    anf(S1, unix('s-1'), P_MSG, MSG), { code: 'gw.route_ohne_klasse' }, { zonen: 'cloud_ohne_zone' }],
  ['zone_der_lokalen_route_frei', 'Option an: die lokale Sitzung laeuft weiter, nur die Cloud-Route ist gesperrt', 'ok', 'beispiel',
    anf(S2, unix('s-2'), P_CHAT, CHAT), { weiterleitung: W_LOKAL }, { zonen: 'cloud_gesperrt' }],
];

const EINST_LAGEN = { ok: EINST_OK, fehler: EINST_FEHLER, ohne_cloud: EINST_OHNE_CLOUD };

function baueEntscheide() {
  const faelle = [];
  for (const [name, beschreibung, e, r, anfrage, erw, zusatz = {}] of ENTSCHEIDE) {
    const laeufe = LAEUFE.filter((l) => l.sitzung !== zusatz.ohne_lauf);
    const z = zusatz.zonen ?? 'aus';
    const lage = { einstellungen: EINST_LAGEN[e], registry_text: REG_TEXTE[r], laeufe, zonen: ZONEN_LAGEN[z] };
    let ist;
    try { ist = { weiterleitung: entscheide(lage, anfrage) }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; }
    const soll = { ...erw }; delete soll.nie_halter; delete soll.nie_modell;
    if (jcs(ist) !== jcs(soll)) abbruch(`entscheide ${name}: Modell ${jcs(ist)} gegen Absicht ${jcs(soll)}`);
    if (erw.nie_halter && (ist.weiterleitung.halter === erw.nie_halter || ist.weiterleitung.anbieter === erw.nie_halter)) abbruch(`entscheide ${name}: alter Anbieter erreicht`);
    if (erw.nie_modell && ist.weiterleitung.modell === erw.nie_modell) abbruch(`entscheide ${name}: tiefere Stufe gewaehlt`);
    const aKopie = { ...anfrage };
    if (anfrage.koerper_text.length > 4096) { aKopie.koerper_text = undefined; aKopie.koerper_muster = { vorne: anfrage.koerper_text.slice(0, 13), fuellung: 'a', anzahl: anfrage.koerper_text.length - 15, hinten: '"}' }; }
    faelle.push({ name, beschreibung, einstellungen: e, registry: r, zonen: z, laeufe_ohne: zusatz.ohne_lauf ?? null, anfrage: JSON.parse(JSON.stringify(aKopie)), erwartet: erw });
  }
  // der ausgelagerte Koerper muss sich genau zurueckbauen lassen
  for (const f of faelle) if (f.anfrage.koerper_muster) {
    const m = f.anfrage.koerper_muster; const t = m.vorne + m.fuellung.repeat(m.anzahl) + m.hinten;
    if (Buffer.byteLength(t) !== KOERPER_MAX.chat + 1) abbruch('Koerpermuster ergibt nicht 2 MiB und ein Byte');
  }
  return {
    fassung: 1,
    hinweis: 'Vertrag 7, Abschnitt 2 und 10. einstellungen: Schluessel in lagen.einstellungen (text = Bytes fuer den Lader von Vertrag 2; fehler = dessen erwarteter Code). registry: Schluessel in lagen.registry (Bytes fuer den Lader aus Abschnitt 3). zonen: Schluessel in lagen.zonen (Sicht des Tors, Abschnitt 4a). laeufe_ohne: dieser Lauf ist abgemeldet. kopfzeilen reicht die Huelle nicht an den Kern. koerper_muster: vorne + fuellung * anzahl + hinten.',
    lagen: {
      einstellungen: Object.fromEntries(Object.entries(EINST_LAGEN).map(([k, v]) => [k, v.fehler ? { text: v.text, fehler: v.fehler } : { text: v.text }])),
      registry: REG_TEXTE,
      zonen: ZONEN_LAGEN,
      laeufe: LAEUFE,
      scheine: { 's-1': S1, 's-2': S2, 's-3': S3, 's-4': S4 },
    },
    faelle,
  };
}

// --- Registry-Faelle ---

function baueRegistry() {
  const v = REG_BSP;
  const tief = (o) => JSON.parse(JSON.stringify(o));
  const mit = (f) => { const o = tief(v); f(o); return regText(o); };
  const FAELLE = [
    ['beispiel', 'die Beispiel-Registry', regText(v), null],
    ['zu_gross', 'mehr als 65536 Byte', regText(v).replace('\n}', `,"x":"${'a'.repeat(REG_MAX)}"\n}`), 'reg.zu_gross'],
    ['bom', 'UTF-8 mit BOM', '﻿' + regText(v), 'reg.json_ungueltig'],
    ['doppelt_tief', 'doppelter Schluessel im Objekt eines Anbieters', regText(v).replace('"art": "cloud",', '"art": "cloud", "art": "cloud",'), 'reg.json_ungueltig'],
    ['unbekannt_tief', 'unbekannter Schluessel in einem Modelleintrag', mit((o) => { o.routen[0].modelle[0].klasse = 'intern'; }), 'reg.schluessel_unbekannt'],
    ['schluessel_im_eintrag', 'ein Feld fuer den Anbieterschluessel ist unbekannt (Schluessel nur beim Halter)', mit((o) => { o.anbieter[0].api_key = 'sk-beispiel'; }), 'reg.schluessel_unbekannt'],
    ['halter_fehlt', 'Anbieter ohne Feld halter', mit((o) => { delete o.anbieter[1].halter; }), 'reg.schluessel_fehlt'],
    ['preis_dezimal', 'preis_ein als 3000.0', regText(v).replace('"preis_ein": 3000', '"preis_ein": 3000.0'), 'reg.typ_falsch'],
    ['fassung_2', 'fassung 2', mit((o) => { o.fassung = 2; }), 'reg.fassung_unbekannt'],
    ['route_ohne_modell', 'eine Route mit leerer Modellliste', mit((o) => { o.routen[0].modelle = []; }), 'reg.anzahl'],
    ['kennung_gross', 'Anbieterschluessel mit Grossbuchstaben', mit((o) => { o.anbieter[0].schluessel = 'Anb'; o.routen[0].modelle[0].anbieter = 'Anb'; }), 'reg.kennung_form'],
    ['modell_leerzeichen', 'Modellname mit Leerzeichen', mit((o) => { o.routen[0].modelle[0].modell = 'claude sonnet'; }), 'reg.modell_form'],
    ['art_hybrid', 'art hybrid', mit((o) => { o.anbieter[0].art = 'hybrid'; }), 'reg.art_unbekannt'],
    ['ziel_mit_pfad', 'Ziel mit Pfad', mit((o) => { o.anbieter[1].ziel = 'https://modelle.firma.beispiel.invalid/v1'; }), 'reg.ziel_form'],
    ['ziel_https_lokal', 'lokaler Anbieter ueber TLS auf einem Rechner der Firma ist gueltig', mit((o) => { o.anbieter[1].ziel = 'https://modelle.firma.beispiel.invalid:8443'; }), null],
    ['ziel_http_fremd', 'lokaler Anbieter ohne TLS auf einer fremden Adresse', mit((o) => { o.anbieter[1].ziel = 'http://192.168.1.5:11434'; }), 'reg.ziel_form'],
    ['ziel_port_null', 'Port 0', mit((o) => { o.anbieter[1].ziel = 'http://127.0.0.1:0'; }), 'reg.ziel_form'],
    ['ziel_benutzer', 'Ziel mit Benutzerteil', mit((o) => { o.anbieter[1].ziel = 'https://u:p@modelle.beispiel.invalid'; }), 'reg.ziel_form'],
    ['weg_beide', 'Anbieter mit Ziel und Halter zugleich', mit((o) => { o.anbieter[0].ziel = 'https://modelle-a.beispiel.invalid'; }), 'reg.weg_form'],
    ['weg_keiner', 'lokaler Anbieter ohne Ziel und ohne Halter', mit((o) => { o.anbieter[1].ziel = null; }), 'reg.weg_form'],
    ['lokal_mit_halter', 'lokaler Anbieter mit Schluessel ueber den Halter ist gueltig', mit((o) => { o.anbieter[1].ziel = null; o.anbieter[1].halter = 'anb-lokal'; }), null],
    ['waehrung_klein', 'Waehrung eur', mit((o) => { o.waehrung = 'eur'; }), 'reg.waehrung_form'],
    ['einheit_10', 'einheit 10', mit((o) => { o.einheit = 10; }), 'reg.einheit_grenze'],
    ['preis_zu_hoch', 'preis_aus 1000001', mit((o) => { o.routen[0].modelle[0].preis_aus = 1000001; }), 'reg.preis_grenze'],
    ['modell_doppelt', 'dasselbe Modell zweimal in einer Route', mit((o) => { o.routen[0].modelle.push(tief(o.routen[0].modelle[0])); }), 'reg.kennung_doppelt'],
    ['anbieter_unbekannt', 'Modelleintrag nennt einen fehlenden Anbieter', mit((o) => { o.routen[0].modelle[0].anbieter = 'anb-fehlt'; }), 'reg.anbieter_unbekannt'],
    ['cloud_direkt', 'Cloud-Anbieter direkt mit Ziel statt ueber den Halter', mit((o) => { o.anbieter[0].halter = null; o.anbieter[0].ziel = 'https://modelle-a.beispiel.invalid'; }), 'reg.halter_pflicht'],
    ['reihenfolge', 'unbekannter Schluessel und falsche Fassung: der Schluessel zuerst', mit((o) => { o.fassung = 2; o.x = 1; }), 'reg.schluessel_unbekannt'],
  ];
  const faelle = [];
  for (const [name, beschreibung, text, code] of FAELLE) {
    let ist; try { regLesen(text); ist = null; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`registry ${name}: Modell ${ist} gegen Absicht ${code}`);
    const eintrag = { name, beschreibung };
    if (text.length > 4096) eintrag.text_muster = { ersetze_ende: '\n}', durch: ',"x":"<a mal 65536>"\n}', basis: 'beispiel' };
    else eintrag.text = text;
    eintrag.erwartet = code === null ? { gueltig: true } : { gueltig: false, code };
    faelle.push(eintrag);
  }
  return { fassung: 1, hinweis: 'Vertrag 7, Abschnitt 3. text sind die Bytes (UTF-8) der Datei; text_muster baut den grossen Fall aus der Vorfuehr-Registry.', faelle };
}

// --- Zaehlung ---

function baueZaehlung() {
  const FAELLE = [
    ['chat_json', 'chat', false, '{"id":"x","usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}', { token_aus: 5, token_ein: 12 }],
    ['chat_strom', 'chat', true, 'data: {"choices":[{"delta":{"content":"Ha"}}]}\n\ndata: {"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":7}}\n\ndata: [DONE]\n\n', { token_aus: 7, token_ein: 20 }],
    ['chat_ohne_usage', 'chat', false, '{"id":"x","choices":[]}', null],
    ['chat_dezimal', 'chat', false, '{"usage":{"prompt_tokens":12.0,"completion_tokens":5}}', null],
    ['chat_negativ', 'chat', false, '{"usage":{"prompt_tokens":-1,"completion_tokens":5}}', null],
    ['messages_json_cache', 'messages', false, '{"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":3}}', { token_aus: 3, token_ein: 1110 }],
    ['messages_strom', 'messages', true,
      'event: message_start\ndata: {"type":"message_start","message":{"usage":{"input_tokens":25,"cache_read_input_tokens":5,"output_tokens":1}}}\n\nevent: message_delta\ndata: {"type":"message_delta","usage":{"output_tokens":40}}\n\n',
      { token_aus: 40, token_ein: 30 }],
    ['messages_ohne_output', 'messages', false, '{"usage":{"input_tokens":10}}', null],
    ['messages_ueber_u32', 'messages', false, '{"usage":{"input_tokens":4294967295,"cache_read_input_tokens":1,"output_tokens":1}}', null],
    ['kein_json', 'messages', false, 'Fehlerseite', null],
  ];
  const faelle = [];
  for (const [name, protokoll, strom, antwort, soll] of FAELLE) {
    const ist = tokensAus(protokoll, strom, antwort);
    if (jcs(ist) !== jcs(soll)) abbruch(`zaehlung ${name}: Modell ${jcs(ist)} gegen Absicht ${jcs(soll)}`);
    faelle.push({ name, protokoll, strom, antwort, erwartet: soll });
  }
  return { fassung: 1, hinweis: 'Vertrag 7, Abschnitt 5. erwartet null heisst: ohne Angabe (Ergebnis ohne_angabe, keine Kosten).', faelle };
}

// --- Kostenzeile ---

function baueKostenzeile() {
  const T0 = Date.UTC(2026, 10, 20, 14, 0, 0, 0);
  const BASIS = { zeit_ms: T0, sitzung: 's-1', route: 'cloud-beispiel', kunde: 'k-beispiel', anbieter: 'anb-cloud-a', modell: 'cloud-beispiel/modell-a',
    ergebnis: 'ok', grund: null, status: 200, token_ein: 1200, token_aus: 300, preis_ein: 3000, preis_aus: 15000, waehrung: 'EUR', einheit: 9 };
  const ZEILEN = [
    ['cloud_ok', 'Cloud-Aufruf: 1200 * 3000 + 300 * 15000 = 8100000 (0,0081 EUR)', BASIS, NULL64, null],
    ['lokal_kostenfrei', 'lokaler Aufruf mit Preis 0', { ...BASIS, sitzung: 's-2', route: 'lokal-beispiel', anbieter: 'anb-lokal', modell: 'lokal-beispiel/modell-a', preis_ein: 0, preis_aus: 0 }, NULL64, null],
    ['ohne_kunde', 'Sitzung ohne Kunde: kunde null', { ...BASIS, kunde: null }, NULL64, null],
    ['ohne_angabe', 'Antwort ohne lesbares usage: Tokens und Kosten null, Status 200, Grund null', { ...BASIS, ergebnis: 'ohne_angabe', token_ein: null, token_aus: null }, NULL64, null],
    ['anbieter_fehler', 'Anbieter antwortet mit einem Status ausser 2xx: Status 503, Grund gw.anbieter_fehler', { ...BASIS, ergebnis: 'anbieter_fehler', grund: 'gw.anbieter_fehler', status: 503, token_ein: null, token_aus: null }, NULL64, null],
    ['halter_lehnt_ab', 'Der Halter lehnt ab: Status 502, Grund gw.halter', { ...BASIS, ergebnis: 'anbieter_fehler', grund: 'gw.halter', status: 502, token_ein: null, token_aus: null }, NULL64, null],
    ['frist_ohne_status', 'Die Frist verstreicht, ohne dass ein Status kam: status null, Grund gw.frist', { ...BASIS, ergebnis: 'anbieter_fehler', grund: 'gw.frist', status: null, token_ein: null, token_aus: null }, NULL64, null],
    ['strom_abgebrochen', 'Abgebrochener Strom mit Status 200 ohne lesbare Tokens: ohne_angabe, Grund null', { ...BASIS, ergebnis: 'ohne_angabe', token_ein: null, token_aus: null }, sha('y'), null],
    ['groesste_kosten', 'Hoechstwerte: Kosten 8589934590000000 bleiben unter 2^53', { ...BASIS, token_ein: U32, token_aus: U32, preis_ein: PREIS_MAX, preis_aus: PREIS_MAX }, NULL64, null],
    ['zeit_grenze', 'letzte Millisekunde von 9999', { ...BASIS, zeit_ms: MAX_MS }, NULL64, null],
    ['mit_vorher', 'vorher aus einer vorigen Zeile', BASIS, sha('x'), null],
    ['route_gross', 'Route mit Grossbuchstaben', { ...BASIS, route: 'Cloud' }, NULL64, 'feld_ungueltig:route'],
    ['sitzung_leer', 'Sitzung leer', { ...BASIS, sitzung: '' }, NULL64, 'feld_ungueltig:sitzung'],
    ['kunde_name', 'Kunde als Anzeigename mit Leerzeichen', { ...BASIS, kunde: 'Kunde A' }, NULL64, 'feld_ungueltig:kunde'],
    ['anbieter_url', 'Anbieter als Adresse statt Schluessel', { ...BASIS, anbieter: 'https://x' }, NULL64, 'feld_ungueltig:anbieter'],
    ['modell_anfuehrung', 'Modell mit Anfuehrungszeichen', { ...BASIS, modell: 'a"b' }, NULL64, 'feld_ungueltig:modell'],
    ['ergebnis_fremd', 'Ergebnis ausserhalb der Liste', { ...BASIS, ergebnis: 'gesperrt' }, NULL64, 'feld_ungueltig:ergebnis'],
    ['grund_bei_ok', 'ok mit einem Grund', { ...BASIS, grund: 'gw.halter' }, NULL64, 'feld_ungueltig:grund'],
    ['grund_fehlt', 'anbieter_fehler ohne Grund', { ...BASIS, ergebnis: 'anbieter_fehler', grund: null, status: 503, token_ein: null, token_aus: null }, NULL64, 'feld_ungueltig:grund'],
    ['grund_fremd', 'anbieter_fehler mit einem Grund ausserhalb der Liste', { ...BASIS, ergebnis: 'anbieter_fehler', grund: 'gw.schein_fehlt', status: 503, token_ein: null, token_aus: null }, NULL64, 'feld_ungueltig:grund'],
    ['status_null_bei_ok', 'ok ohne Status: nur anbieter_fehler darf status null tragen', { ...BASIS, status: null }, NULL64, 'feld_ungueltig:status'],
    ['status_zu_klein', 'Status 99', { ...BASIS, status: 99 }, NULL64, 'feld_ungueltig:status'],
    ['status_zu_gross', 'Status 600', { ...BASIS, status: 600 }, NULL64, 'feld_ungueltig:status'],
    ['grund_vor_status', 'falscher Grund und falscher Status: der Grund zuerst', { ...BASIS, grund: 'gw.halter', status: 600 }, NULL64, 'feld_ungueltig:grund'],
    ['ok_ohne_tokens', 'ok ohne Tokens', { ...BASIS, token_ein: null }, NULL64, 'feld_ungueltig:token_ein'],
    ['ohne_angabe_mit_tokens', 'ohne_angabe mit Tokens', { ...BASIS, ergebnis: 'ohne_angabe', token_ein: null, token_aus: 3 }, NULL64, 'feld_ungueltig:token_aus'],
    ['token_ueber_u32', 'token_aus 4294967296', { ...BASIS, token_aus: U32 + 1 }, NULL64, 'feld_ungueltig:token_aus'],
    ['preis_ueber', 'preis_ein 1000001', { ...BASIS, preis_ein: PREIS_MAX + 1 }, NULL64, 'feld_ungueltig:preis_ein'],
    ['preis_aus_negativ', 'preis_aus -1', { ...BASIS, preis_aus: -1 }, NULL64, 'feld_ungueltig:preis_aus'],
    ['waehrung_symbol', 'Waehrung als Zeichen', { ...BASIS, waehrung: 'EU' }, NULL64, 'feld_ungueltig:waehrung'],
    ['einheit_negativ', 'einheit -1', { ...BASIS, einheit: -1 }, NULL64, 'feld_ungueltig:einheit'],
    ['vorher_gross', 'vorher in Grossbuchstaben', BASIS, NULL64.toUpperCase().replace(/0/g, 'A'), 'feld_ungueltig:vorher'],
    ['zeit_negativ', 'zeit vor 1970', { ...BASIS, zeit_ms: -1 }, NULL64, 'zeit_ungueltig'],
    ['reihenfolge', 'falsche Route und falsche Zeit: die Route zuerst', { ...BASIS, route: '', zeit_ms: -1 }, NULL64, 'feld_ungueltig:route'],
  ];
  const zeilen = [];
  for (const [name, beschreibung, aufruf, vorher, code] of ZEILEN) {
    let ist;
    try { ist = { zeile: zeileAufruf(aufruf, vorher) }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; }
    if ((code === null) !== ('zeile' in ist) || (code !== null && ist.code !== code)) abbruch(`kostenzeile ${name}: Modell ${jcs(ist)} gegen Absicht ${code}`);
    const e = { name, beschreibung, aufruf, vorher };
    if (code === null) {
      const hand = zeileVonHand(aufruf, vorher);
      if (hand !== ist.zeile) abbruch(`kostenzeile ${name}: jcs und Handform weichen ab`);
      const o = JSON.parse(ist.zeile);
      if (Object.keys(o).sort().join() !== [...ZEILE_FELDER].sort().join()) abbruch(`kostenzeile ${name}: Felder nicht geschlossen`);
      if (typeof o.kosten === 'number' && !Number.isSafeInteger(o.kosten)) abbruch(`kostenzeile ${name}: Kosten nicht exakt`);
      if (/titel|pfad|prompt|antwort|content/.test(ist.zeile)) abbruch(`kostenzeile ${name}: Inhalt in der Zeile`);
      e.erwartet = { zeile: ist.zeile, sha256: sha(ist.zeile) };
    } else e.erwartet = { code };
    zeilen.push(e);
  }
  // Kette: drei Aufrufe auf einen leeren Abschnitt, dann ein Aufruf nach einer Zeile der Fassung 1g (gemischte Kette)
  const k1 = readFileSync(KETTE_VEK, 'utf8').split('\n').filter(Boolean).map((z) => JSON.parse(z)).find((v) => v.kette === 'k1' && v.vorher === NULL64);
  if (!k1 || sha(k1.zeile) !== k1.hash) abbruch('Vektor k1 fehlt oder ist nicht gleich');
  const kette = (anfang, aufrufe) => {
    let inhalt = anfang; const rueck = [];
    for (const a of aufrufe) {
      const zeilenAlt = inhalt.split('\n').filter((z) => z !== '');
      const vorher = zeilenAlt.length ? sha(zeilenAlt[zeilenAlt.length - 1]) : NULL64;
      const z = zeileAufruf(a, vorher);
      inhalt += z + '\n'; rueck.push(sha(z));
    }
    // eigene Kettenpruefung: jede Zeile verweist auf die vorige
    const zs = inhalt.split('\n').slice(0, -1);
    zs.forEach((z, i) => { if (i > 0 && JSON.parse(z).vorher !== sha(zs[i - 1])) abbruch('Kette gebrochen'); });
    return { inhalt, rueckgaben: rueck, letzte: sha(zs[zs.length - 1]), zeilen: zs.length };
  };
  const drei = [BASIS, { ...BASIS, zeit_ms: T0 + 1000, ergebnis: 'anbieter_fehler', grund: 'gw.anbieter_fehler', status: 429, token_ein: null, token_aus: null },
    { ...BASIS, zeit_ms: T0 + 2000, sitzung: 's-2', route: 'lokal-beispiel', anbieter: 'anb-lokal', modell: 'lokal-beispiel/modell-a', preis_ein: 0, preis_aus: 0 }];
  const a = kette('', drei);
  const b = kette(k1.zeile + '\n', [BASIS]);
  const ketten = [
    { name: 'drei_aufrufe', beschreibung: 'drei Aufrufe nacheinander auf gateway-2026-11.jsonl, leer am Anfang', datei: 'gateway-2026-11.jsonl', anfang: '', aufrufe: drei,
      erwartet: { rueckgaben: a.rueckgaben, inhalt: a.inhalt, pruefe: { bruch: null, letzte: a.letzte, ok: true, zeilen: a.zeilen } } },
    { name: 'nach_fassung_1g', beschreibung: 'der laufende Monat des alten Gateways (Fassung 1g, Vektor k1) wird fortgesetzt; vorher ist der sha256 der Rohbytes der Zeile 1g',
      datei: 'gateway-2026-11.jsonl', anfang: k1.zeile + '\n', aufrufe: [BASIS],
      erwartet: { rueckgaben: b.rueckgaben, inhalt: b.inhalt, pruefe: { bruch: null, letzte: b.letzte, ok: true, zeilen: b.zeilen } } },
  ];
  if (!b.inhalt.split('\n')[1].includes(`"vorher":"${k1.hash}"`)) abbruch('gemischte Kette verweist nicht auf die Zeile 1g');
  return { fassung: 1, hinweis: 'Vertrag 7, Abschnitt 6. aufruf sind die Felder von Aufruf; erwartet.zeile sind die Bytes ohne LF. ketten: pruefe ist die erwartete Ausgabe von kette pruefe (Vertrag 5) ueber die Datei.', zeilen, ketten };
}

// --- schreiben oder vergleichen ---

const dateien = {
  'faelle/entscheide.json': baueEntscheide(),
  'faelle/registry.json': baueRegistry(),
  'faelle/zaehlung.json': baueZaehlung(),
  'faelle/kostenzeile.json': baueKostenzeile(),
};
const ausgabe = Object.fromEntries(Object.entries(dateien).map(([k, v]) => [k, JSON.stringify(v, null, 1) + '\n']));
ausgabe['registry.beispiel.json'] = REG_TEXTE.beispiel;
// die Beispiel-Registry muss der eigene Lader annehmen
regLesen(REG_TEXTE.beispiel);

if (process.argv.includes('--pruefen')) {
  let gleich = true;
  for (const [pfad, inhalt] of Object.entries(ausgabe)) {
    let alt = null; try { alt = readFileSync(join(HIER, pfad), 'utf8'); } catch { /* fehlt */ }
    if (alt !== inhalt) { process.stdout.write(`verschieden: ${pfad}\n`); gleich = false; }
  }
  process.stdout.write(gleich ? `gleich: ${Object.keys(ausgabe).length} Dateien\n` : '');
  process.exit(gleich ? 0 : 1);
}
mkdirSync(AUS, { recursive: true });
for (const [pfad, inhalt] of Object.entries(ausgabe)) writeFileSync(join(HIER, pfad), inhalt);
const n = (d) => (d.faelle ?? d.zeilen).length + (d.ketten ? d.ketten.length : 0);
process.stdout.write(`geschrieben: ${Object.entries(dateien).map(([k, v]) => `${k} ${n(v)}`).join(', ')}, registry.beispiel.json\n`);

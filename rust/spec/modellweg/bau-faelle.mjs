// Erzeugt die sichtbaren Faelle von Vertrag 8 (README.md in diesem Ordner) nach faelle/.
// Bytes der Protokollzeile aus der JCS-Referenz (RFC 8785) und node:crypto. Ein eigenes Referenzmodell in diesem Skript
// (Anfrage pruefen, Antwort durchreichen mit Leck-Sperre, Tokens, Zeile, Aufruf, Schluesseldatei) rechnet jeden Fall nach; weicht es
// von der Absicht im Fall ab, bricht das Skript mit Exit 1 ab und schreibt nichts.
// Aufruf: node rust/spec/modellweg/bau-faelle.mjs (leicht, ohne Abhaengigkeit). --pruefen vergleicht die Dateien Byte fuer Byte mit einem neuen Lauf.
// Alle Schluessel hier sind erfundene Testwerte. Kein Fall spricht ein echtes Ziel an.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs } from '../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const KETTE_VEK = join(HIER, '../../../spec/protokoll/vektoren/kette-vektoren.jsonl');

const NULL64 = '0'.repeat(64);
const KENNUNG = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const MODELL = /^[A-Za-z0-9._:/@+-]{1,128}$/;
const HEX = /^[0-9a-f]{64}$/;
const ZIEL = /^(https:\/\/([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*)|http:\/\/(127\.0\.0\.1|\[::1\]))(?::([1-9][0-9]{0,4}))?((?:\/[A-Za-z0-9._~-]+)*\/v1)$/;
const SCHLUESSEL = /^[\x21-\x7e]{1,4096}$/;
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const U32 = 4294967295;
const KOPF_MAX = 65536;
const KOERPER_MAX = 33554432;
const PARALLEL_MAX = 8;
const TUNNEL_MAX = 32;
const PFAD = '/v1/chat/completions';
const ZEILE_FELDER = ['aktionsname', 'betriebsart', 'ergebnis', 'fassung', 'grund', 'host', 'modell', 'pfad', 'route', 'sitzung', 'status', 'token_aus', 'token_ein', 'vorher', 'zeit'];

// eigene Ablehnungen: Code, Status, fester Text
const ABLEHNUNG = {
  'mw.protokoll_fehlt': [503, 'Protokoll nicht schreibbar'],
  'mw.anfrage_unlesbar': [400, 'Anfrage nicht lesbar'],
  'mw.ausgelastet': [429, 'zu viele gleichzeitige Anfragen'],
  'mw.pfad_unbekannt': [404, 'Weg unbekannt'],
  'mw.methode_falsch': [405, 'nur POST'],
  'mw.chunked_nicht_unterstuetzt': [411, 'Transfer-Encoding nicht unterstuetzt, Content-Length ist Pflicht'],
  'mw.laenge_fehlt': [411, 'Content-Length fehlt'],
  'mw.koerper_zu_gross': [413, 'Koerper zu gross'],
  'mw.json_ungueltig': [400, 'kein gueltiges JSON'],
  'mw.modell_nicht_erlaubt': [403, 'Modell nicht erlaubt'],
  'mw.ziel_nicht_erreichbar': [502, 'Ziel nicht erreichbar'],
  'mw.ziel_frist': [504, 'Ziel antwortet nicht'],
  'mw.leck_gesperrt': [502, 'Antwort des Ziels gesperrt'],
  'mw.zaehlung_fehler': [503, 'Zaehlung nicht schreibbar'],
};
const GRUENDE = {
  weitergereicht: [null],
  abgelehnt: ['mw.anfrage_unlesbar', 'mw.ausgelastet', 'mw.pfad_unbekannt', 'mw.methode_falsch', 'mw.chunked_nicht_unterstuetzt', 'mw.laenge_fehlt', 'mw.koerper_zu_gross', 'mw.json_ungueltig', 'mw.modell_nicht_erlaubt', 'mw.zaehlung_fehler'],
  ziel_fehler: ['mw.ziel_nicht_erreichbar', 'mw.ziel_frist'],
  leck_gesperrt: ['mw.leck_gesperrt'],
};
const fehlerKoerper = (code) => jcs({ error: { code, message: ABLEHNUNG[code][1] } });

const sha = (b) => createHash('sha256').update(typeof b === 'string' ? Buffer.from(b, 'utf8') : b).digest('hex');
class Fehler extends Error {}
const fehler = (code) => new Fehler(code);
const abbruch = (satz) => { process.stderr.write(`bau-faelle: ${satz}\n`); process.exit(1); };
// Gezielte Mutanten des Modells (Abschnitt 14, M-W1 bis M-W7, M-Z0 bis M-Z4; Abschnitt 4, M-F1a bis M-F1c; Abschnitt 9a, M-S1 bis M-S4): nur mit --pruefen, schreibt nie Dateien.
const MUT = process.env.BAU_MUTANT ?? '';
const M = (n) => MUT === n;
if (MUT && !process.argv.includes('--pruefen')) abbruch('BAU_MUTANT nur zusammen mit --pruefen');

// --- strenges JSON-Lesen: UTF-8 ohne BOM, doppelte Schluessel auf jeder Ebene verboten ---

class Zahl { constructor(wert, ganz) { this.wert = wert; this.ganz = ganz; } }
function jsonLesen(bytes) {
  const b = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes, 'utf8');
  if (b.length >= 3 && b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) throw fehler('json');
  let t;
  try { t = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(b); } catch { throw fehler('json'); }
  let i = 0;
  const ws = () => { while (i < t.length && ' \t\n\r'.includes(t[i])) i++; };
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
  const v = wert(); ws();
  if (i !== t.length) throw fehler('json');
  return v;
}
const istObj = (v) => v instanceof Map;

// --- Ziel zerlegen ---

function zielTeile(ziel) {
  const m = ZIEL.exec(ziel);
  if (!m) return null;
  const host = m[2] ?? m[3];
  if (m[2] && m[2].length > 253) return null;
  if (m[4] !== undefined && Number(m[4]) > 65535) return null;
  return { tls: ziel.startsWith('https:'), host, port: m[4] ?? null, praefix: m[5] };
}

// --- Abschnitt 4: eine Anfrage pruefen (roh, HTTP/1.1) ---

function anfrageLesen(roh, voll = false) {
  const b = Buffer.from(roh, 'latin1');
  const ende = b.indexOf('\r\n\r\n');
  if (ende < 0 || ende + 4 > KOPF_MAX) throw fehler('mw.anfrage_unlesbar');
  const kopfText = b.subarray(0, ende).toString('latin1');
  const zeilen = kopfText.split('\r\n');
  const start = /^([A-Z]{1,16}) (\/[\x21-\x7e]*) HTTP\/1\.1$/.exec(zeilen[0]);
  if (!start) throw fehler('mw.anfrage_unlesbar');
  const kopf = [];
  for (const z of zeilen.slice(1)) {
    const k = /^([!#$%&'*+.^_`|~0-9A-Za-z-]+):[ \t]*([\x20-\x7e]*?)[ \t]*$/.exec(z);
    if (!k) throw fehler('mw.anfrage_unlesbar');
    kopf.push([k[1].toLowerCase(), k[2]]);
  }
  const werte = (n) => kopf.filter(([k]) => k === n).map(([, v]) => v);
  const pfad = pfadFeld(start[2], voll);
  const cl = werte('content-length');
  if (cl.length > 1 || (cl.length === 1 && !/^(0|[1-9][0-9]{0,15})$/.test(cl[0]))) { const e = fehler('mw.anfrage_unlesbar'); e.pfad = pfad; throw e; }
  const koerper = b.subarray(ende + 4);
  const laenge = cl.length ? Number(cl[0]) : 0;
  return { methode: start[1], ziel: start[2], pfad, kopf, laenge, hatLaenge: cl.length === 1, koerper, werte };
}

function pruefeAnfrage(lage, roh) {
  if (lage.betriebsart !== 'openai-lokal') abbruch('nur die Betriebsart openai-lokal ist beschrieben');
  if (!lage.protokoll_ok) throw fehler('mw.protokoll_fehlt');
  const a = anfrageLesen(roh, lage.pfad_voll === true);
  // Der Pfad der Zeile steht fest, sobald der Kopf gelesen ist (Abschnitt 8); jede Ablehnung danach traegt ihn.
  const pfad = a.pfad;
  const wirf = (code) => { const e = fehler(code); e.pfad = pfad; throw e; };
  if (lage.laufend >= PARALLEL_MAX) wirf('mw.ausgelastet');
  if (a.ziel !== PFAD) wirf('mw.pfad_unbekannt');
  if (a.methode !== 'POST') wirf('mw.methode_falsch');
  if (a.werte('transfer-encoding').length) wirf('mw.chunked_nicht_unterstuetzt');
  if (!a.hatLaenge) wirf('mw.laenge_fehlt');
  if (a.laenge > KOERPER_MAX) wirf('mw.koerper_zu_gross');
  if (a.koerper.length !== a.laenge) wirf('mw.anfrage_unlesbar');
  let o;
  try { o = jsonLesen(a.koerper); } catch { wirf('mw.json_ungueltig'); }
  if (!istObj(o)) wirf('mw.json_ungueltig');
  if (o.get('model') !== lage.modell) wirf('mw.modell_nicht_erlaubt');
  const z = zielTeile(lage.ziel);
  const accept = a.werte('accept');
  const kopf = [
    ['Host', z.host + (z.port ? `:${z.port}` : '')],
    ['Authorization', `Bearer ${lage.schluessel}`],
    ['Content-Type', 'application/json'],
    ...(accept.length === 1 && ['application/json', 'text/event-stream'].includes(accept[0]) ? [['Accept', accept[0]]] : []),
    ['Content-Length', String(a.laenge)],
    ['Connection', 'close'],
  ];
  const zeilen = [`POST ${z.praefix}/chat/completions HTTP/1.1`, ...kopf.map(([k, v]) => `${k}: ${v}`)];
  return { url: `${lage.ziel}/chat/completions`, an_ziel: zeilen.join('\r\n') + '\r\n\r\n' + a.koerper.toString('latin1') };
}

// --- Abschnitt 5: Antwort des Ziels durchreichen mit Leck-Sperre ---

function antwortDurch(lage, ziel) {
  const key = Buffer.from(lage.schluessel, 'latin1');
  const k1 = key.length - 1;
  const ct = ziel.kopf.filter(([k]) => k.toLowerCase() === 'content-type').map(([, v]) => v);
  const kopf = ct.length === 1 && /^[\x20-\x7e]{1,256}$/.test(ct[0]) ? [['content-type', ct[0]]] : [];
  if (kopf.length && Buffer.from(kopf[0][1], 'latin1').includes(key)) return { art: 'ersetzt', status: 502, kopf: [['content-type', 'application/json']], koerper: fehlerKoerper('mw.leck_gesperrt') };
  let puffer = Buffer.alloc(0); let gesendet = Buffer.alloc(0); let kopfGesendet = false;
  for (const t of ziel.teile) {
    puffer = Buffer.concat([puffer, Buffer.from(t, 'latin1')]);
    if (puffer.includes(key)) {
      return kopfGesendet ? { art: 'abgerissen', status: ziel.status, kopf, gesendet: gesendet.toString('latin1') }
        : { art: 'ersetzt', status: 502, kopf: [['content-type', 'application/json']], koerper: fehlerKoerper('mw.leck_gesperrt') };
    }
    if (puffer.length > k1) {
      gesendet = Buffer.concat([gesendet, puffer.subarray(0, puffer.length - k1)]);
      puffer = puffer.subarray(puffer.length - k1);
      kopfGesendet = true;
    }
  }
  gesendet = Buffer.concat([gesendet, puffer]);
  return { art: 'antwort', status: ziel.status, kopf, koerper: gesendet.toString('latin1') };
}

// Tokens nur aus usage, falls das Ziel sie liefert
const tokZahl = (v) => (v instanceof Zahl && v.ganz && v.wert >= 0 && v.wert <= U32 ? v.wert : null);
function tokensAus(koerper) {
  const lesen = (s) => { try { return jsonLesen(Buffer.from(s, 'latin1')); } catch { return undefined; } };
  let usage;
  const ganz = lesen(koerper);
  if (istObj(ganz)) usage = ganz.get('usage');
  else for (const z of koerper.split('\n')) {
    if (!z.startsWith('data: ') || z.trimEnd() === 'data: [DONE]') continue;
    const o = lesen(z.slice(6).replace(/\r$/, ''));
    if (istObj(o) && istObj(o.get('usage'))) usage = o.get('usage');
  }
  if (!istObj(usage)) return { token_aus: null, token_ein: null };
  const ein = tokZahl(usage.get('prompt_tokens')); const aus = tokZahl(usage.get('completion_tokens'));
  return ein === null || aus === null ? { token_aus: null, token_ein: null } : { token_aus: aus, token_ein: ein };
}

// --- Feld pfad der Zeile (Abschnitt 8, Projektentscheid, 22.11.2026) ---

const PFAD_SEGMENT = /^[A-Za-z0-9._~-]+$/;
const SEGMENT_MAX = 32;
const PFAD_VOLL_MAX = 512;
// Begrenzter Pfad der Anfrage: ohne Abfrage und Fragment, hoechstens zwei Segmente, jedes Segment ausserhalb der
// Zeichenform als "*" (nie gekuerzt, nie dekodiert). Mit voll der ganze Pfad, hoechstens 512 Zeichen.
// M-P1: kuerzt ein zu langes Segment statt "*"; M-P2: die Abfrage bleibt stehen.
function pfadFeld(ziel, voll) {
  const ohne = M('M-P2') ? ziel.split('#')[0] : ziel.split(/[?#]/)[0];
  if (!ohne.startsWith('/')) return '*';
  if (ohne === '/') return '/';
  if (voll && ohne.length > PFAD_VOLL_MAX) return '*';
  const max = voll ? PFAD_VOLL_MAX : SEGMENT_MAX;
  const teile = ohne.slice(1).split('/').map((t) => {
    if (PFAD_SEGMENT.test(t) && t.length <= max) return t;
    if (M('M-P1') && PFAD_SEGMENT.test(t)) return t.slice(0, max);
    return '*';
  });
  return '/' + (voll ? teile : teile.slice(0, 2)).join('/');
}
const PFAD_FELD = /^(\*|\/|(\/(\*|[A-Za-z0-9._~-]+))+)$/;
const pfadFeldOk = (w) => w.length <= PFAD_VOLL_MAX && PFAD_FELD.test(w);
// Im Feld steht der Pfad nur bei einer Ablehnung und bei GET. M-P3: er steht auch bei einem weitergereichten POST.
const pfadInZeile = (pfad, ergebnis, methode) => (pfad === null || pfad === undefined ? null
  : (ergebnis === 'abgelehnt' || methode === 'GET' || M('M-P3') ? pfad : null));

// --- Abschnitt 6: Protokollzeile ---

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw fehler('zeit_ungueltig');
  return new Date(ms).toISOString();
}
function zeileModellweg(a, vorher) {
  const f = (feld, ok) => { if (!ok) throw fehler(`feld_ungueltig:${feld}`); };
  const tok = (v) => (a.ergebnis === 'weitergereicht' ? v === null || (Number.isInteger(v) && v >= 0 && v <= U32) : v === null);
  f('betriebsart', BETRIEBSARTEN.includes(a.betriebsart));
  f('route', KENNUNG.test(a.route));
  f('sitzung', KENNUNG.test(a.sitzung));
  f('modell', MODELL.test(a.modell));
  f('ergebnis', a.ergebnis in GRUENDE);
  f('grund', GRUENDE[a.ergebnis].includes(a.grund));
  f('status', Number.isInteger(a.status) && a.status >= 100 && a.status <= 599);
  f('token_ein', tok(a.token_ein));
  f('token_aus', tok(a.token_aus) && (a.token_ein === null) === (a.token_aus === null));
  // Ein GET kommt in dieser Betriebsart nie durch Stufe 4 oder 5; nur eine Ablehnung traegt einen Pfad.
  f('pfad', a.pfad === null || (pfadFeldOk(a.pfad) && a.ergebnis === 'abgelehnt'));
  f('host', a.host === null);
  f('vorher', HEX.test(vorher));
  const zeit = zeitText(a.zeit_ms);
  return jcs({ aktionsname: 'modellweg.aufruf', betriebsart: a.betriebsart, ergebnis: a.ergebnis, fassung: 2, grund: a.grund, host: a.host, modell: a.modell,
    pfad: a.pfad, route: a.route, sitzung: a.sitzung, status: a.status, token_aus: a.token_aus, token_ein: a.token_ein, vorher, zeit });
}
function zeileVonHand(a, vorher) {
  const s = (v) => (v === null ? 'null' : typeof v === 'number' ? String(v) : `"${v}"`);
  const w = { aktionsname: 'modellweg.aufruf', betriebsart: a.betriebsart, ergebnis: a.ergebnis, fassung: 2, grund: a.grund, host: a.host, modell: a.modell,
    pfad: a.pfad, route: a.route, sitzung: a.sitzung, status: a.status, token_aus: a.token_aus, token_ein: a.token_ein, vorher, zeit: new Date(a.zeit_ms).toISOString() };
  return `{${ZEILE_FELDER.map((k) => `"${k}":${s(w[k])}`).join(',')}}`;
}

// --- Abschnitt 3: Aufruf und Schluesseldatei ---

// Pflicht in der Betriebsart openai-lokal; --betriebsart ist frei (Vorgabe openai-lokal). Eine spaetere Betriebsart legt ihre eigenen Pflichtoptionen fest.
const OPTIONEN = ['--socket', '--socket-rechte', '--ziel', '--modell', '--schluessel-datei', '--sitzung', '--route', '--protokoll'];
const BETRIEBSARTEN = ['openai-lokal'];
const PFAD_ABS = (p, max) => typeof p === 'string' && Buffer.byteLength(p) <= max && /^(\/[^/\u0000-\u001f\u007f-\u009f]+)+$/.test(p)
  && !p.split('/').slice(1).some((s) => s === '.' || s === '..');
// Probelauf mit Marker-Zaehlung (Abschnitt 9, fuer jede Betriebsart): --probelauf, --marker und --marker-zaehlung alle drei oder keiner
const MARKER_OPT = ['--probelauf', '--marker', '--marker-zaehlung'];
// Schalter ohne Wert, fuer jede Betriebsart: --probelauf (Abschnitt 9) und --protokoll-pfad-voll (Abschnitt 8).
const PFAD_VOLL_OPT = '--protokoll-pfad-voll';
const SCHALTER = ['--probelauf', PFAD_VOLL_OPT];
// argv lesen: --probelauf ist ein Schalter ohne Wert, jede andere Option hat genau einen Wert
function argvLesen(argv, erlaubt) {
  const w = new Map();
  for (let i = 0; i < argv.length;) {
    const o = argv[i];
    if (!(erlaubt.includes(o) || o === '--betriebsart' || MARKER_OPT.includes(o) || o === PFAD_VOLL_OPT) || w.has(o)) throw fehler('mw.aufruf');
    if (SCHALTER.includes(o)) { w.set(o, true); i += 1; continue; }
    if (i + 1 >= argv.length) throw fehler('mw.aufruf');
    w.set(o, argv[i + 1]); i += 2;
  }
  return w;
}
function markerPruefen(w) {
  const da = MARKER_OPT.map((o) => w.has(o));
  if (da.every((x) => !x)) return null;
  if (!da.every((x) => x)) throw fehler('mw.probelauf_aufruf');
  const liste = w.get('--marker').split(',');
  if (liste.length < 1 || liste.length > 16 || liste.some((x) => !/^[A-Z0-9-]{1,32}$/.test(x)) || new Set(liste).size !== liste.length
    || !PFAD_ABS(w.get('--marker-zaehlung'), 1024)) throw fehler('mw.probelauf_aufruf');
  return { marker: liste, datei: w.get('--marker-zaehlung') };
}
function zeileProbelauf(a, vorher) {
  const f = (feld, ok) => { if (!ok) throw fehler(`feld_ungueltig:${feld}`); };
  f('programm', KENNUNG.test(a.programm));
  f('sitzung', KENNUNG.test(a.sitzung));
  f('anzahl', Number.isInteger(a.anzahl) && a.anzahl >= 1 && a.anzahl <= 16);
  f('vorher', HEX.test(vorher));
  return jcs({ aktionsname: 'probelauf.zaehlung', anzahl: a.anzahl, ergebnis: 'an', fassung: 2, programm: a.programm, sitzung: a.sitzung, vorher, zeit: zeitText(a.zeit_ms) });
}
function zaehlen(marker, koerper) {
  const b = Buffer.from(koerper, 'latin1'); const aus = {};
  for (const m of marker) { let n = 0; let i = 0; const k = Buffer.from(m, 'latin1'); for (;;) { const j = b.indexOf(k, i); if (j < 0) break; n++; i = j + k.length; } aus[m] = n; }
  return aus;
}
function aufrufLesen(argv) {
  const w = argvLesen(argv, OPTIONEN);
  if (!w.has('--betriebsart')) w.set('--betriebsart', 'openai-lokal');
  if (!BETRIEBSARTEN.includes(w.get('--betriebsart'))) throw fehler('mw.betriebsart_unbekannt');
  markerPruefen(w);
  if ([...w.keys()].filter((k) => !MARKER_OPT.includes(k) && k !== PFAD_VOLL_OPT).length !== OPTIONEN.length + 1) throw fehler('mw.aufruf');
  if (!PFAD_ABS(w.get('--socket'), 107) || w.get('--socket-rechte') !== '0600' || !zielTeile(w.get('--ziel')) || !MODELL.test(w.get('--modell'))
    || !PFAD_ABS(w.get('--schluessel-datei'), 1024) || !KENNUNG.test(w.get('--sitzung')) || !KENNUNG.test(w.get('--route'))
    || !PFAD_ABS(w.get('--protokoll'), 1024)) throw fehler('mw.aufruf');
  return w;
}
function schluesselLesen(d) {
  if (d.art === 'fehlt' || d.art !== 'datei' || !d.eigen || d.modus !== '0600') throw fehler('mw.schluessel_datei');
  const b = Buffer.from(d.inhalt, 'latin1');
  if (b.length > 4097) throw fehler('mw.schluessel_form');
  const t = d.inhalt.endsWith('\n') ? d.inhalt.slice(0, -1) : d.inhalt;
  if (!SCHLUESSEL.test(t)) throw fehler('mw.schluessel_form');
  return t;
}
const START_CODES = { 'mw.aufruf': [64, 'Aufruf ungueltig'], 'mw.betriebsart_unbekannt': [64, 'Betriebsart unbekannt oder nicht gebaut'], 'mw.probelauf_aufruf': [64, 'Probelauf-Optionen ungueltig'], 'mw.protokoll_fehlt': [74, 'Protokoll nicht schreibbar'], 'mw.zaehlung_fehler': [74, 'Zaehldatei nicht schreibbar'],'mw.schluessel_datei': [78, 'Schluesseldatei nicht eigen, nicht 0600 oder keine Datei'],
  'mw.schluessel_form': [78, 'Schluessel hat falsche Form'], 'mw.protokoll_ordner': [78, 'Protokollordner fehlt oder ist kein Ordner'],
  'mw.ziel_unaufloesbar': [69, 'Ziel nicht aufloesbar'], 'mw.socket_belegt': [74, 'Socket belegt oder nicht anlegbar'], 'mw.intern': [70, 'interner Fehler'] };

// --- Ausgangslage ---

const SCHL = 'mw-testschluessel-7f3a9c0e5b2d4618';
const ZIEL_FIRMA = 'https://modelle.firma.beispiel.invalid/v1';
const MOD = 'lokal-beispiel/modell-a';
const LAGE = { betriebsart: 'openai-lokal', ziel: ZIEL_FIRMA, modell: MOD, schluessel: SCHL, laufend: 0, protokoll_ok: true };
const KOERPER = JSON.stringify({ model: MOD, messages: [{ role: 'user', content: 'Hallo' }] });
const KOERPER_STROM = JSON.stringify({ model: MOD, stream: true, messages: [{ role: 'user', content: 'Hallo' }] });
const roh = (methode, pfad, kopf, koerper) => `${methode} ${pfad} HTTP/1.1\r\n${kopf.map(([k, v]) => `${k}: ${v}\r\n`).join('')}\r\n${koerper}`;
const post = (koerper, kopf = []) => roh('POST', PFAD, [['Host', '127.0.0.1:11434'], ['Content-Type', 'application/json'], ...kopf, ['Content-Length', String(Buffer.byteLength(koerper, 'latin1'))]], koerper);
const anZiel = (koerper, accept) => `POST /v1/chat/completions HTTP/1.1\r\nHost: modelle.firma.beispiel.invalid\r\nAuthorization: Bearer ${SCHL}\r\nContent-Type: application/json\r\n${accept ? `Accept: ${accept}\r\n` : ''}Content-Length: ${Buffer.byteLength(koerper, 'latin1')}\r\nConnection: close\r\n\r\n${koerper}`;

const ANFRAGEN = [
  ['gueltig', 'gueltige Anfrage: geht unveraendert mit dem Schluessel aus der Datei an das Ziel', LAGE, post(KOERPER), { an_ziel: anZiel(KOERPER) }],
  ['platzhalter_ersetzt', 'Authorization "Bearer platzhalter" des Harness wird verworfen; hinaus geht nur der Schluessel aus der Datei', LAGE,
    post(KOERPER, [['Authorization', 'Bearer platzhalter']]), { an_ziel: anZiel(KOERPER), nie_im_ziel: 'platzhalter' }],
  ['fremde_koepfe_verworfen', 'Koepfe des Harness (Cookie, X-Forwarded-For, eigener Host) gehen nicht hinaus', LAGE,
    post(KOERPER, [['Cookie', 'a=b'], ['X-Forwarded-For', '10.0.0.1'], ['X-Api-Key', 'platzhalter']]), { an_ziel: anZiel(KOERPER), nie_im_ziel: 'platzhalter' }],
  ['strom', 'gestreamte Anfrage mit Accept text/event-stream: Koerper unveraendert, Accept geht mit', LAGE,
    post(KOERPER_STROM, [['Accept', 'text/event-stream']]), { an_ziel: anZiel(KOERPER_STROM, 'text/event-stream') }],
  ['koerper_unveraendert', 'Koerper mit Leerraum und Escape im Modellnamen geht Byte fuer Byte hinaus', LAGE,
    post('{ "model" : "lokal-beispiel\\/modell-a", "messages":[] }'), { an_ziel: anZiel('{ "model" : "lokal-beispiel\\/modell-a", "messages":[] }') }],
  ['lokal_ollama', 'Ausweichziel ohne TLS auf dem Loopback des Rechners', { ...LAGE, ziel: 'http://127.0.0.1:11434/v1', modell: 'qwen3:8b' },
    post(JSON.stringify({ model: 'qwen3:8b', messages: [] })),
    { an_ziel: `POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nAuthorization: Bearer ${SCHL}\r\nContent-Type: application/json\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{"model":"qwen3:8b","messages":[]}` }],
  ['anderes_modell', 'ein anderes Modell wird abgelehnt, ohne das Ziel zu beruehren', LAGE, post(JSON.stringify({ model: 'lokal-beispiel/modell-b', messages: [] })), { code: 'mw.modell_nicht_erlaubt' }],
  ['modell_fehlt', 'ohne Feld model', LAGE, post(JSON.stringify({ messages: [] })), { code: 'mw.modell_nicht_erlaubt' }],
  ['modell_doppelt', 'model zweimal: erst erlaubt, dann ein anderes (der Leser des Ziels nimmt oft den letzten)', LAGE,
    post(`{"model":"${MOD}","model":"lokal-beispiel/modell-b","messages":[]}`), { code: 'mw.json_ungueltig' }],
  ['modell_gross', 'Modellname mit anderer Schreibweise', LAGE, post(JSON.stringify({ model: MOD.toUpperCase(), messages: [] })), { code: 'mw.modell_nicht_erlaubt' }],
  ['pfad_models', 'GET /v1/models', LAGE, roh('GET', '/v1/models', [['Host', 'x']], ''), { code: 'mw.pfad_unbekannt' }],
  ['pfad_completions', 'POST /v1/completions', LAGE, roh('POST', '/v1/completions', [['Host', 'x'], ['Content-Length', '2']], '{}'), { code: 'mw.pfad_unbekannt' }],
  ['pfad_abfrage', 'Pfad mit Abfrageteil', LAGE, roh('POST', `${PFAD}?x=1`, [['Host', 'x'], ['Content-Length', '2']], '{}'), { code: 'mw.pfad_unbekannt' }],
  ['pfad_umweg', 'Pfad mit Punkt-Segment', LAGE, roh('POST', '/v1/../v1/chat/completions', [['Host', 'x'], ['Content-Length', '2']], '{}'), { code: 'mw.pfad_unbekannt' }],
  ['methode_get', 'GET auf den richtigen Pfad', LAGE, roh('GET', PFAD, [['Host', 'x']], ''), { code: 'mw.methode_falsch' }],
  ['methode_put', 'PUT auf den richtigen Pfad', LAGE, roh('PUT', PFAD, [['Host', 'x'], ['Content-Length', '2']], '{}'), { code: 'mw.methode_falsch' }],
  ['zu_gross', 'Content-Length 33554433', LAGE, roh('POST', PFAD, [['Host', 'x'], ['Content-Length', '33554433']], ''), { code: 'mw.koerper_zu_gross' }],
  ['chunked', 'Transfer-Encoding chunked', LAGE, roh('POST', PFAD, [['Host', 'x'], ['Transfer-Encoding', 'chunked']], '2\r\n{}\r\n0\r\n\r\n'), { code: 'mw.chunked_nicht_unterstuetzt' }],
  ['ohne_laenge', 'POST ohne Content-Length und ohne Koerper', LAGE, roh('POST', PFAD, [['Host', 'x']], ''), { code: 'mw.laenge_fehlt' }],
  ['laenge_doppelt', 'zwei Content-Length', LAGE, roh('POST', PFAD, [['Host', 'x'], ['Content-Length', '2'], ['Content-Length', '2']], '{}'), { code: 'mw.anfrage_unlesbar' }],
  ['laenge_falsch', 'Content-Length groesser als der Koerper', LAGE, roh('POST', PFAD, [['Host', 'x'], ['Content-Length', '9']], '{}'), { code: 'mw.anfrage_unlesbar' }],
  ['http10', 'HTTP/1.0', LAGE, `POST ${PFAD} HTTP/1.0\r\nContent-Length: 2\r\n\r\n{}`, { code: 'mw.anfrage_unlesbar' }],
  ['kein_json', 'Koerper ist kein JSON', LAGE, post('model=x'), { code: 'mw.json_ungueltig' }],
  ['kein_objekt', 'Koerper ist eine Liste', LAGE, post('[]'), { code: 'mw.json_ungueltig' }],
  ['ausgelastet', 'acht Anfragen laufen schon', { ...LAGE, laufend: 8 }, post(KOERPER), { code: 'mw.ausgelastet' }],
  ['protokoll_fehlt', 'das Anhaengen einer Zeile ist vorher gescheitert', { ...LAGE, protokoll_ok: false }, post(KOERPER), { code: 'mw.protokoll_fehlt' }],
  ['reihenfolge_pfad_vor_methode', 'GET auf einen fremden Pfad: der Pfad zaehlt zuerst', LAGE, roh('GET', '/admin', [['Host', 'x']], ''), { code: 'mw.pfad_unbekannt' }],
];

function baueAnfragen() {
  const faelle = [];
  for (const [name, beschreibung, lage, r, erw] of ANFRAGEN) {
    let ist;
    let pfad = null;
    try { ist = pruefeAnfrage(lage, r); ist = { an_ziel: ist.an_ziel }; } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      ist = { code: x.message };
      pfad = x.pfad ?? null;
    }
    const soll = erw.code ? { code: erw.code } : { an_ziel: erw.an_ziel };
    if (jcs(ist) !== jcs(soll)) abbruch(`anfrage ${name}: Modell ${jcs(ist)} gegen Absicht ${jcs(soll)}`);
    if (erw.nie_im_ziel && ist.an_ziel.includes(erw.nie_im_ziel)) abbruch(`anfrage ${name}: Platzhalter geht hinaus`);
    if (ist.an_ziel && ist.an_ziel.split(`Bearer ${SCHL}`).length !== 2) abbruch(`anfrage ${name}: Schluessel nicht genau einmal`);
    const e = erw.code
      ? { ziel_beruehrt: false, status: ABLEHNUNG[erw.code][0], koerper: fehlerKoerper(erw.code),
        zeile: { ergebnis: 'abgelehnt', grund: erw.code, status: ABLEHNUNG[erw.code][0], pfad: pfadInZeile(pfad, 'abgelehnt', null), host: null } }
      : { ziel_beruehrt: true, an_ziel: erw.an_ziel };
    if (erw.code === 'mw.protokoll_fehlt') e.zeile = null;
    faelle.push({ name, beschreibung, lage, anfrage: r, erwartet: e });
  }
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 10 (Betriebsart openai-lokal). anfrage und an_ziel sind die Bytes (latin1) der HTTP-Anfrage. lage.schluessel ist der Inhalt der Schluesseldatei (erfundener Testwert). ziel_beruehrt false heisst: keine Verbindung zum Ziel. zeile nennt die Felder ergebnis, grund, status, pfad und host der Protokollzeile; null heisst keine Zeile.', faelle };
}

// --- Antworten ---

const SSE = ['data: {"id":"c1","choices":[{"delta":{"reasoning":"Ich denke"}}]}\n\n', 'data: {"id":"c1","choices":[{"delta":{"content":"Hallo"}}]}\n\n',
  'data: {"id":"c1","choices":[],"usage":{"prompt_tokens":11,"completion_tokens":4,"total_tokens":15}}\n\n', 'data: [DONE]\n\n'];
const MIT_REASONING = '{"id":"c2","choices":[{"message":{"role":"assistant","content":"Hallo","reasoning":"Der Nutzer gruesst."}}],"usage":{"prompt_tokens":9,"completion_tokens":3}}';
const POLICY = '{"error":{"message":"model lokal-beispiel/modell-b is not allowed for this key","type":"policy_model_blocked","code":403}}';
const ANTWORTEN = [
  ['reasoning_unveraendert', 'Antwort mit Feld reasoning geht Byte fuer Byte durch; Tokens aus usage', { status: 200, kopf: [['Content-Type', 'application/json'], ['Set-Cookie', 's=1']], teile: [MIT_REASONING] },
    { art: 'antwort', status: 200, kopf: [['content-type', 'application/json']], koerper: MIT_REASONING }, { token_aus: 3, token_ein: 9 }, 'weitergereicht'],
  ['strom_unveraendert', 'SSE-Strom mit reasoning geht Byte fuer Byte durch; Tokens aus dem letzten usage', { status: 200, kopf: [['Content-Type', 'text/event-stream']], teile: SSE },
    { art: 'antwort', status: 200, kopf: [['content-type', 'text/event-stream']], koerper: SSE.join('') }, { token_aus: 4, token_ein: 11 }, 'weitergereicht'],
  ['strom_ohne_usage', 'Strom ohne usage: Tokens null', { status: 200, kopf: [['Content-Type', 'text/event-stream']], teile: [SSE[0], SSE[1], SSE[3]] },
    { art: 'antwort', status: 200, kopf: [['content-type', 'text/event-stream']], koerper: SSE[0] + SSE[1] + SSE[3] }, { token_aus: null, token_ein: null }, 'weitergereicht'],
  ['policy_403', '403 policy_model_blocked des Ziels: Status und Koerper unveraendert', { status: 403, kopf: [['Content-Type', 'application/json']], teile: [POLICY] },
    { art: 'antwort', status: 403, kopf: [['content-type', 'application/json']], koerper: POLICY }, { token_aus: null, token_ein: null }, 'weitergereicht'],
  ['fehler_401', '401 des Ziels mit Textkoerper unveraendert', { status: 401, kopf: [['Content-Type', 'text/plain']], teile: ['unauthorized'] },
    { art: 'antwort', status: 401, kopf: [['content-type', 'text/plain']], koerper: 'unauthorized' }, { token_aus: null, token_ein: null }, 'weitergereicht'],
  ['umleitung_nicht_gefolgt', '302 des Ziels: unveraendert zurueck, Location geht nicht mit, kein zweiter Aufruf', { status: 302, kopf: [['Location', 'https://fremd.beispiel.invalid/']], teile: [''] },
    { art: 'antwort', status: 302, kopf: [], koerper: '' }, { token_aus: null, token_ein: null }, 'weitergereicht'],
  ['leck_im_fehlerkoerper', 'das Ziel spiegelt den Schluessel in einem Fehlerkoerper: ersetzt, bevor ein Byte zum Harness geht', { status: 401, kopf: [['Content-Type', 'application/json']], teile: [`{"error":"bad key Bearer ${SCHL}"}`] },
    { art: 'ersetzt', status: 502, kopf: [['content-type', 'application/json']], koerper: fehlerKoerper('mw.leck_gesperrt') }, { token_aus: null, token_ein: null }, 'leck_gesperrt'],
  ['leck_ueber_teilgrenze', 'der Schluessel ist auf zwei Teile verteilt', { status: 200, kopf: [['Content-Type', 'application/json']], teile: [`{"x":"${SCHL.slice(0, 10)}`, `${SCHL.slice(10)}"}`] },
    { art: 'ersetzt', status: 502, kopf: [['content-type', 'application/json']], koerper: fehlerKoerper('mw.leck_gesperrt') }, { token_aus: null, token_ein: null }, 'leck_gesperrt'],
  ['leck_spaet_im_strom', 'der Schluessel kommt erst nach 2 Ereignissen: die Verbindung zum Harness reisst ab', { status: 200, kopf: [['Content-Type', 'text/event-stream']], teile: [SSE[0], SSE[1], `data: {"k":"${SCHL}"}\n\n`] },
    { art: 'abgerissen', status: 200, kopf: [['content-type', 'text/event-stream']], gesendet: (SSE[0] + SSE[1]).slice(0, (SSE[0] + SSE[1]).length - (SCHL.length - 1)) }, { token_aus: null, token_ein: null }, 'leck_gesperrt'],
  ['leck_im_kopf', 'der Schluessel steht im Content-Type des Ziels', { status: 200, kopf: [['Content-Type', `text/plain; x=${SCHL}`]], teile: ['ok'] },
    { art: 'ersetzt', status: 502, kopf: [['content-type', 'application/json']], koerper: fehlerKoerper('mw.leck_gesperrt') }, { token_aus: null, token_ein: null }, 'leck_gesperrt'],
  ['teil_kuerzer_als_schluessel', 'viele kurze Teile ohne Schluessel kommen vollstaendig und in Reihenfolge an', { status: 200, kopf: [['Content-Type', 'text/event-stream']], teile: ['da', 'ta: ', '{}', '\n\n'] },
    { art: 'antwort', status: 200, kopf: [['content-type', 'text/event-stream']], koerper: 'data: {}\n\n' }, { token_aus: null, token_ein: null }, 'weitergereicht'],
];

function baueAntworten() {
  const faelle = [];
  for (const [name, beschreibung, ziel, soll, tokSoll, ergebnis] of ANTWORTEN) {
    const ist = antwortDurch(LAGE, ziel);
    if (jcs(ist) !== jcs(soll)) abbruch(`antwort ${name}: Modell ${jcs(ist)} gegen Absicht ${jcs(soll)}`);
    const tok = ist.art === 'antwort' ? tokensAus(ist.koerper) : { token_aus: null, token_ein: null };
    if (jcs(tok) !== jcs(tokSoll)) abbruch(`antwort ${name}: Tokens ${jcs(tok)} gegen ${jcs(tokSoll)}`);
    for (const teil of [ist.koerper ?? '', ist.gesendet ?? '', ...ist.kopf.map(([, v]) => v)]) if (teil.includes(SCHL)) abbruch(`antwort ${name}: Schluessel geht zum Harness`);
    if (ist.art === 'antwort' && ist.koerper !== ziel.teile.join('')) abbruch(`antwort ${name}: Koerper nicht unveraendert`);
    const ergErw = ist.art === 'antwort' ? 'weitergereicht' : 'leck_gesperrt';
    if (ergErw !== ergebnis) abbruch(`antwort ${name}: Ergebnis ${ergErw}`);
    faelle.push({ name, beschreibung, ziel, erwartet: { an_harness: ist, zeile: { ergebnis, grund: ergebnis === 'leck_gesperrt' ? 'mw.leck_gesperrt' : null, status: ist.status, ...tok, pfad: pfadInZeile(pfadFeld(PFAD, false), ergebnis, 'POST'), host: null } } });
  }
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitte 7 und 11 (Betriebsart openai-lokal). Schluessel der Lage: ' + SCHL + ' (erfunden). ziel.teile sind die Teile des Koerpers in der Reihenfolge des Eintreffens (latin1). an_harness: antwort (Status, Koepfe ausser der Rahmung, Koerper), ersetzt (eigene 502) oder abgerissen (gesendet = Bytes vor dem Abriss).', schluessel: SCHL, faelle };
}

// --- Aufruf und Schluesseldatei ---

function baueAufruf() {
  const gut = ['--socket', '/run/user/1000/wb/s-2/modell.sock', '--socket-rechte', '0600', '--ziel', ZIEL_FIRMA, '--modell', MOD,
    '--schluessel-datei', '/home/beispiel/.config/wb/modellweg.schluessel', '--sitzung', 's-2', '--route', 'lokal-beispiel', '--protokoll', '/home/beispiel/.local/state/wb/protokoll'];
  const setze = (opt, wert) => { const a = [...gut]; a[a.indexOf(opt) + 1] = wert; return a; };
  const ohne = (opt) => { const a = [...gut]; a.splice(a.indexOf(opt), 2); return a; };
  const ARGV = [
    ['gueltig', 'die acht Pflichtoptionen ohne --betriebsart: Vorgabe openai-lokal', gut, null],
    ['betriebsart_openai_lokal', '--betriebsart openai-lokal ausdruecklich', [...gut, '--betriebsart', 'openai-lokal'], null],
    ['betriebsart_vorne', '--betriebsart openai-lokal als erste Option', ['--betriebsart', 'openai-lokal', ...gut], null],
    ['betriebsart_kopf_einsetzen', '--betriebsart kopf-einsetzen mit den Optionen von openai-lokal: wo Betriebsart 2 nicht gebaut ist, unbekannt; wo gebaut, mw.aufruf (erwartet_gebaut)', [...gut, '--betriebsart', 'kopf-einsetzen'], 'mw.betriebsart_unbekannt'],
    ['betriebsart_fremd', '--betriebsart mit unbekanntem Wert', [...gut, '--betriebsart', 'openai'], 'mw.betriebsart_unbekannt'],
    ['betriebsart_doppelt', '--betriebsart zweimal', [...gut, '--betriebsart', 'openai-lokal', '--betriebsart', 'openai-lokal'], 'mw.aufruf'],
    ['betriebsart_vor_form', 'nicht gebaute Betriebsart und fehlendes --ziel: die Betriebsart zaehlt zuerst; wo gebaut, mw.aufruf (erwartet_gebaut)', [...ohne('--ziel'), '--betriebsart', 'kopf-einsetzen'], 'mw.betriebsart_unbekannt'],
    ['pfad_voll', '--protokoll-pfad-voll ist ein Schalter ohne Wert, Vorgabe aus', [...gut, '--protokoll-pfad-voll'], null],
    ['pfad_voll_vorne', '--protokoll-pfad-voll als erste Option', ['--protokoll-pfad-voll', ...gut], null],
    ['pfad_voll_doppelt', '--protokoll-pfad-voll zweimal', [...gut, '--protokoll-pfad-voll', '--protokoll-pfad-voll'], 'mw.aufruf'],
    ['pfad_voll_mit_wert', '--protokoll-pfad-voll mit Wert ja: ja ist keine Option', [...gut, '--protokoll-pfad-voll', 'ja'], 'mw.aufruf'],
    ['leer', 'ohne Argument', [], 'mw.aufruf'],
    ['option_fehlt', 'ohne --schluessel-datei', ohne('--schluessel-datei'), 'mw.aufruf'],
    ['option_doppelt', '--modell zweimal', [...gut, '--modell', MOD], 'mw.aufruf'],
    ['option_unbekannt', '--schluessel statt --schluessel-datei (ein Wert in argv waere ein Leck)', setze('--schluessel-datei', '/x').map((v) => (v === '--schluessel-datei' ? '--schluessel' : v)), 'mw.aufruf'],
    ['gleichheitszeichen', '--route=lokal-beispiel', [...ohne('--route'), '--route=lokal-beispiel', 'x'], 'mw.aufruf'],
    ['rechte_0666', '--socket-rechte 0666', setze('--socket-rechte', '0666'), 'mw.aufruf'],
    ['socket_zu_lang', 'Socketpfad mit 108 Byte', setze('--socket', '/' + 'a'.repeat(107)), 'mw.aufruf'],
    ['socket_relativ', 'relativer Socketpfad', setze('--socket', 'modell.sock'), 'mw.aufruf'],
    ['ziel_http_fremd', 'http an eine fremde Adresse', setze('--ziel', 'http://modelle.firma.beispiel.invalid/v1'), 'mw.aufruf'],
    ['ziel_benutzer', 'Ziel mit Benutzerteil', setze('--ziel', 'https://u:p@modelle.firma.beispiel.invalid/v1'), 'mw.aufruf'],
    ['ziel_abfrage', 'Ziel mit Abfrageteil', setze('--ziel', 'https://modelle.firma.beispiel.invalid/v1?x=1'), 'mw.aufruf'],
    ['ziel_schraegstrich', 'Ziel endet mit /', setze('--ziel', 'https://modelle.firma.beispiel.invalid/v1/'), 'mw.aufruf'],
    ['ziel_port_gross', 'Port 65536', setze('--ziel', 'https://modelle.firma.beispiel.invalid:65536/v1'), 'mw.aufruf'],
    ['ziel_loopback_http', 'http auf 127.0.0.1 ist gueltig', setze('--ziel', 'http://127.0.0.1:11434/v1'), null],
    ['ziel_ohne_v1', 'Basisadresse ohne /v1 am Ende', setze('--ziel', 'https://modelle.firma.beispiel.invalid'), 'mw.aufruf'],
    ['ziel_praefix_v1', 'Basisadresse mit Praefix vor /v1 ist gueltig', setze('--ziel', 'https://modelle.firma.beispiel.invalid/gw/v1'), null],
    ['modell_leerzeichen', 'Modell mit Leerzeichen', setze('--modell', 'qwen 3'), 'mw.aufruf'],
    ['sitzung_gross', 'Sitzung S-2', setze('--sitzung', 'S-2'), 'mw.aufruf'],
    ['protokoll_punkt', 'Protokollordner mit ..', setze('--protokoll', '/home/beispiel/../x'), 'mw.aufruf'],
  ];
  const aufruf = [];
  for (const [name, beschreibung, argv, code] of ARGV) {
    let ist = null; try { aufrufLesen(argv); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`aufruf ${name}: Modell ${ist} gegen ${code}`);
    let ist2 = null; try { aufrufLesen2(argv); } catch (x) { if (!(x instanceof Fehler)) throw x; ist2 = x.message; }
    const erw = (c) => (c === null ? { bis_schritt: 'aufruf_gut' } : { exit: START_CODES[c][0], stderr: `${c}: ${START_CODES[c][1]}\n`, stdout: '' });
    const fall = { name, beschreibung, argv, erwartet: erw(code) };
    if (ist2 !== ist) fall.erwartet_gebaut = erw(ist2);
    aufruf.push(fall);
  }
  const anders = aufruf.filter((f) => f.erwartet_gebaut).map((f) => f.name);
  const GEBAUT_ANDERS = ['betriebsart_kopf_einsetzen', 'betriebsart_vor_form'];
  if (jcs(anders) !== jcs(GEBAUT_ANDERS)) abbruch(`aufruf: mit gebauter Betriebsart 2 anders ${jcs(anders)} gegen ${jcs(GEBAUT_ANDERS)}`);
  const D = { art: 'datei', eigen: true, modus: '0600', inhalt: SCHL + '\n' };
  const DATEIEN = [
    ['gueltig', 'eigene Datei 0600 mit einer Zeile', D, null],
    ['ohne_lf', 'ohne Zeilenende', { ...D, inhalt: SCHL }, null],
    ['modus_0640', 'Modus 0640', { ...D, modus: '0640' }, 'mw.schluessel_datei'],
    ['modus_0400', 'Modus 0400', { ...D, modus: '0400' }, 'mw.schluessel_datei'],
    ['fremd', 'Eigentuemer ist eine andere Kennung', { ...D, eigen: false }, 'mw.schluessel_datei'],
    ['link', 'symbolischer Link (O_NOFOLLOW)', { ...D, art: 'link' }, 'mw.schluessel_datei'],
    ['ordner', 'ein Ordner', { ...D, art: 'ordner' }, 'mw.schluessel_datei'],
    ['fehlt', 'Datei fehlt', { art: 'fehlt' }, 'mw.schluessel_datei'],
    ['leer', 'leere Datei', { ...D, inhalt: '' }, 'mw.schluessel_form'],
    ['zwei_zeilen', 'zwei Zeilen', { ...D, inhalt: `${SCHL}\nzweite\n` }, 'mw.schluessel_form'],
    ['leerzeichen', 'Schluessel mit Leerzeichen', { ...D, inhalt: 'Bearer x\n' }, 'mw.schluessel_form'],
    ['crlf', 'Zeilenende CRLF', { ...D, inhalt: `${SCHL}\r\n` }, 'mw.schluessel_form'],
    ['zu_lang', '4097 Zeichen ohne LF', { ...D, inhalt: 'a'.repeat(4097) }, 'mw.schluessel_form'],
  ];
  const schluessel = [];
  for (const [name, beschreibung, d, code] of DATEIEN) {
    let ist = null; let wert;
    try { wert = schluesselLesen(d); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`schluesseldatei ${name}: Modell ${ist} gegen ${code}`);
    if (code === null && wert !== SCHL) abbruch(`schluesseldatei ${name}: falscher Wert`);
    schluessel.push({ name, beschreibung, datei: d, erwartet: code === null ? { gueltig: true } : { exit: START_CODES[code][0], stderr: `${code}: ${START_CODES[code][1]}\n`, stdout: '' } });
  }
  for (const f of [...aufruf, ...schluessel]) if (f.erwartet.stderr && f.erwartet.stderr.includes(SCHL)) abbruch('Schluessel in stderr');
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitte 4, 5 und 10. aufruf: argv nach dem Programmnamen; bis_schritt aufruf_gut heisst, die Pruefung des Aufrufs besteht (die weiteren Schritte brauchen Dateien). schluesseldatei: art datei, link, ordner oder fehlt; eigen = Eigentuemer ist die eigene Kennung; inhalt latin1.', codes: START_CODES, aufruf, schluesseldatei: schluessel };
}

// --- Protokollzeile ---

function baueZeile() {
  const T0 = Date.UTC(2026, 10, 20, 15, 0, 0, 0);
  const B = { zeit_ms: T0, betriebsart: 'openai-lokal', sitzung: 's-2', route: 'lokal-beispiel', modell: MOD, ergebnis: 'weitergereicht', grund: null, status: 200, token_ein: 9, token_aus: 3, pfad: null, host: null };
  const Z = [
    ['weitergereicht', 'Antwort 200 mit usage', B, NULL64, null],
    ['ohne_tokens', 'Antwort ohne usage', { ...B, token_ein: null, token_aus: null }, NULL64, null],
    ['policy_403', '403 des Ziels unveraendert weitergereicht', { ...B, status: 403, token_ein: null, token_aus: null }, NULL64, null],
    ['abgelehnt_modell', 'eigene Ablehnung: anderes Modell; modell ist das eingestellte, nie das angefragte', { ...B, ergebnis: 'abgelehnt', grund: 'mw.modell_nicht_erlaubt', status: 403, token_ein: null, token_aus: null }, NULL64, null],
    ['ziel_fehler', 'Ziel nicht erreichbar', { ...B, ergebnis: 'ziel_fehler', grund: 'mw.ziel_nicht_erreichbar', status: 502, token_ein: null, token_aus: null }, NULL64, null],
    ['leck', 'Leck gesperrt', { ...B, ergebnis: 'leck_gesperrt', grund: 'mw.leck_gesperrt', status: 502, token_ein: null, token_aus: null }, NULL64, null],
    ['tokens_max', 'Tokens 4294967295', { ...B, token_ein: U32, token_aus: U32 }, sha('x'), null],
    ['pfad_bei_ablehnung', 'eigene Ablehnung: der begrenzte Pfad der Anfrage steht in der Zeile', { ...B, ergebnis: 'abgelehnt', grund: 'mw.pfad_unbekannt', status: 404, token_ein: null, token_aus: null, pfad: '/v1/models' }, NULL64, null],
    ['pfad_stern', 'ein Segment ausserhalb der Zeichenform steht als Stern', { ...B, ergebnis: 'abgelehnt', grund: 'mw.pfad_unbekannt', status: 404, token_ein: null, token_aus: null, pfad: '/*' }, NULL64, null],
    ['pfad_bei_weiter', 'weitergereicht mit Pfad: in dieser Betriebsart traegt nur eine Ablehnung einen Pfad', { ...B, pfad: '/v1/chat' }, NULL64, 'feld_ungueltig:pfad'],
    ['pfad_mit_abfrage', 'ein roher Pfad mit Abfrage im Feld pfad', { ...B, ergebnis: 'abgelehnt', grund: 'mw.pfad_unbekannt', status: 404, token_ein: null, token_aus: null, pfad: '/v1/models?a=b' }, NULL64, 'feld_ungueltig:pfad'],
    ['host_gesetzt', 'host in Betriebsart 1 ist immer null', { ...B, host: 'api.beispiel.invalid' }, NULL64, 'feld_ungueltig:host'],
    ['betriebsart_fremd', 'Betriebsart kopf-einsetzen hat in Fassung 1 keine Zeilenform', { ...B, betriebsart: 'kopf-einsetzen' }, NULL64, 'feld_ungueltig:betriebsart'],
    ['reihenfolge_betriebsart', 'falsche Betriebsart und falsche Route: die Betriebsart zuerst', { ...B, betriebsart: '', route: 'Lokal' }, NULL64, 'feld_ungueltig:betriebsart'],
    ['route_falsch', 'Route mit Grossbuchstaben', { ...B, route: 'Lokal' }, NULL64, 'feld_ungueltig:route'],
    ['sitzung_falsch', 'Sitzung leer', { ...B, sitzung: '' }, NULL64, 'feld_ungueltig:sitzung'],
    ['modell_falsch', 'Modell mit Anfuehrungszeichen (ein angefragtes Modell darf nie hinein)', { ...B, modell: 'a"b' }, NULL64, 'feld_ungueltig:modell'],
    ['ergebnis_falsch', 'Ergebnis ok', { ...B, ergebnis: 'ok' }, NULL64, 'feld_ungueltig:ergebnis'],
    ['grund_bei_weiter', 'weitergereicht mit Grund', { ...B, grund: 'mw.ausgelastet' }, NULL64, 'feld_ungueltig:grund'],
    ['grund_fremd', 'abgelehnt mit Grund eines Zielfehlers', { ...B, ergebnis: 'abgelehnt', grund: 'mw.ziel_frist', status: 504, token_ein: null, token_aus: null }, NULL64, 'feld_ungueltig:grund'],
    ['status_600', 'Status 600', { ...B, status: 600 }, NULL64, 'feld_ungueltig:status'],
    ['tokens_bei_ablehnung', 'abgelehnt mit Tokens', { ...B, ergebnis: 'abgelehnt', grund: 'mw.pfad_unbekannt', status: 404 }, NULL64, 'feld_ungueltig:token_ein'],
    ['tokens_halb', 'nur token_ein', { ...B, token_aus: null }, NULL64, 'feld_ungueltig:token_aus'],
    ['vorher_kurz', 'vorher mit 63 Zeichen', B, NULL64.slice(1), 'feld_ungueltig:vorher'],
    ['zeit_negativ', 'zeit vor 1970', { ...B, zeit_ms: -1 }, NULL64, 'zeit_ungueltig'],
  ];
  const zeilen = [];
  for (const [name, beschreibung, a, vorher, code] of Z) {
    let ist;
    try { ist = { zeile: zeileModellweg(a, vorher) }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; }
    if ((code === null) !== ('zeile' in ist) || (code !== null && ist.code !== code)) abbruch(`zeile ${name}: Modell ${jcs(ist)} gegen ${code}`);
    const e = { name, beschreibung, aufruf: a, vorher };
    if (code === null) {
      if (zeileVonHand(a, vorher) !== ist.zeile) abbruch(`zeile ${name}: jcs und Handform weichen ab`);
      if (/content|reasoning|Hallo|Bearer|mw-testschluessel|[?#]/.test(ist.zeile)) abbruch(`zeile ${name}: Inhalt in der Zeile`);
      const sollPfad = a.pfad === null ? 'null' : JSON.stringify(a.pfad);
      if (!ist.zeile.includes(`"pfad":${sollPfad}`)) abbruch(`zeile ${name}: Feld pfad weicht ab`);
      e.erwartet = { zeile: ist.zeile, sha256: sha(ist.zeile) };
    } else e.erwartet = { code };
    zeilen.push(e);
  }
  const k1 = readFileSync(KETTE_VEK, 'utf8').split('\n').filter(Boolean).map((z) => JSON.parse(z)).find((v) => v.kette === 'k1' && v.vorher === NULL64);
  if (!k1 || sha(k1.zeile) !== k1.hash) abbruch('Vektor k1 fehlt');
  const kette = (anfang, aufrufe) => {
    let inhalt = anfang; const rueck = [];
    for (const a of aufrufe) {
      const alt = inhalt.split('\n').filter(Boolean);
      const z = zeileModellweg(a, alt.length ? sha(alt[alt.length - 1]) : NULL64);
      inhalt += z + '\n'; rueck.push(sha(z));
    }
    const zs = inhalt.split('\n').slice(0, -1);
    zs.forEach((z, i) => { if (i > 0 && JSON.parse(z).vorher !== sha(zs[i - 1])) abbruch('Kette gebrochen'); });
    return { rueckgaben: rueck, inhalt, pruefe: { bruch: null, letzte: sha(zs[zs.length - 1]), ok: true, zeilen: zs.length } };
  };
  const drei = [B, { ...B, zeit_ms: T0 + 1000, ergebnis: 'abgelehnt', grund: 'mw.modell_nicht_erlaubt', status: 403, token_ein: null, token_aus: null }, { ...B, zeit_ms: T0 + 2000, status: 403, token_ein: null, token_aus: null }];
  const ketten = [
    { name: 'drei_aufrufe', beschreibung: 'drei Aufrufe auf einen leeren Abschnitt gateway-2026-11.jsonl', datei: 'gateway-2026-11.jsonl', anfang: '', aufrufe: drei, erwartet: kette('', drei) },
    { name: 'nach_fassung_1g', beschreibung: 'ein laufender Monat mit einer Zeile des alten Gateways (Fassung 1g) wird fortgesetzt', datei: 'gateway-2026-11.jsonl', anfang: k1.zeile + '\n', aufrufe: [B], erwartet: kette(k1.zeile + '\n', [B]) },
  ];
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitte 8 und 11. aufruf sind die Felder von Modellweg; erwartet.zeile die Bytes ohne LF. ketten.erwartet.pruefe ist die erwartete Ausgabe von kette pruefe (Vertrag 5).', zeilen, ketten };
}

// ===================================================================================================================
// --- Betriebsart 2 kopf-einsetzen (Teil C, Abschnitte 11 bis 13): noch nicht gebaut; die Faelle gelten ab dem Bau ---
// ===================================================================================================================

const METHODEN = ['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'];
const HOST_DNS = /^(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
const REGEL_PFAD = /^(\/\*|(\/[A-Za-z0-9._~-]+)+(\/\*)?)$/;
const KOPF_NAME = /^[a-z0-9-]{1,64}$/;
const KOPF_VERBOTEN = ['host', 'connection', 'content-length', 'transfer-encoding', 'te', 'trailer', 'upgrade', 'keep-alive', 'proxy-connection', 'accept-encoding'];
// vorlaeufig (Entscheidung vom 20.11.2026): ausser Authorization wird nichts umgeschrieben; accept-encoding und Cookies gehen unveraendert
const ANMELDEKOEPFE = ['authorization'];
// Jeder Name, den der Halter verbietet (Halter-Vertrag, Abschnitt 4: 31 Namen). Das ist die Liste des HALTERWEGS
// (modell.aufruf und modell.strom, ab Schritt I); dort setzt der Halter Host und Rahmung selbst. Der Weg ist noch
// nicht gebaut; die Liste steht hier benannt, und die Pruefung unten haelt ihre Laenge fest.
const HALTER_VERBOTEN = ['authorization', 'proxy-authorization', 'x-api-key', 'api-key', 'cookie', 'set-cookie', 'host', 'content-length',
  'transfer-encoding', 'connection', 'upgrade', 'te', 'trailer', 'keep-alive', 'proxy-connection', 'forwarded', 'x-forwarded-for',
  'x-forwarded-host', 'accept-encoding', 'sec-websocket-extensions', 'x-http-method-override', 'x-http-method', 'x-method-override',
  'x-original-method', 'x-original-url', 'x-original-uri', 'x-rewrite-url', 'x-forwarded-uri', 'x-forwarded-path', 'x-forwarded-prefix',
  'x-envoy-original-path'];
// Was der DIREKTE Weg zu einem gebundenen Ziel zusaetzlich entfernt (vor Schritt I, Schluessel aus der Datei;
// Entscheid des Betreibers, 25.11.2026): die Liste ohne host, content-length und transfer-encoding, die die Anfrage
// dort braucht, und ohne die acht, die schon oben fallen (authorization, accept-encoding und die sechs Koepfe einer Verbindung).
const DIREKT_BLEIBT = ['host', 'content-length', 'transfer-encoding'];
const HOP = ['connection', 'keep-alive', 'proxy-connection', 'te', 'trailer', 'upgrade'];
const DIREKT_VERBOTEN = HALTER_VERBOTEN.filter((h) => !DIREKT_BLEIBT.includes(h) && h !== 'authorization' && h !== 'accept-encoding' && !HOP.includes(h));
if (HALTER_VERBOTEN.length !== 31) abbruch(`Halterweg-Liste hat ${HALTER_VERBOTEN.length} Namen statt 31`);
if (DIREKT_VERBOTEN.length !== 20) abbruch(`direkte Liste hat ${DIREKT_VERBOTEN.length} Namen statt 20`);
const ZL_MAX = 65536;
const ZL_FORM = {
  oben: ['fassung', 'ca_zulassung', 'ziele'],
  ziel: ['name', 'host', 'port', 'schluessel_datei', 'kopf', 'wert_vorsatz', 'regeln', 'upgrade'],
  regel: ['methode', 'pfad'],
};

// Zielliste lesen (Abschnitt 11.2)
function zieleLesen(text) {
  const b = Buffer.from(text, 'utf8');
  if (b.length > ZL_MAX) throw fehler('zl.zu_gross');
  let o;
  try { o = jsonLesen(b); } catch { throw fehler('zl.json_ungueltig'); }
  if (!istObj(o)) throw fehler('zl.json_ungueltig');
  const ebenen = [[o, ZL_FORM.oben]];
  if (Array.isArray(o.get('ziele'))) for (const z of o.get('ziele')) if (istObj(z)) {
    ebenen.push([z, ZL_FORM.ziel]);
    if (Array.isArray(z.get('regeln'))) for (const r of z.get('regeln')) if (istObj(r)) ebenen.push([r, ZL_FORM.regel]);
  }
  for (const [e, f] of ebenen) for (const k of e.keys()) if (!f.includes(k)) throw fehler('zl.schluessel_unbekannt');
  for (const [e, f] of ebenen) for (const k of f) if (!e.has(k)) throw fehler('zl.schluessel_fehlt');
  const typ = (ok) => { if (!ok) throw fehler('zl.typ_falsch'); };
  const ganzZahl = (v) => v instanceof Zahl && v.ganz && Number.isSafeInteger(v.wert);
  typ(ganzZahl(o.get('fassung')) && typeof o.get('ca_zulassung') === 'string' && Array.isArray(o.get('ziele')));
  for (const z of o.get('ziele')) {
    typ(istObj(z));
    typ(typeof z.get('name') === 'string' && typeof z.get('host') === 'string' && ganzZahl(z.get('port')) && typeof z.get('upgrade') === 'string');
    for (const k of ['schluessel_datei', 'kopf', 'wert_vorsatz']) typ(z.get(k) === null || typeof z.get(k) === 'string');
    typ(Array.isArray(z.get('regeln')));
    for (const r of z.get('regeln')) typ(istObj(r) && typeof r.get('methode') === 'string' && typeof r.get('pfad') === 'string');
  }
  if (o.get('fassung').wert !== 1) throw fehler('zl.fassung_unbekannt');
  const zl = o.get('ziele');
  if (zl.length < 1 || zl.length > 16) throw fehler('zl.anzahl');
  for (const z of zl) if (z.get('regeln').length < 1 || z.get('regeln').length > 32) throw fehler('zl.anzahl');
  for (const z of zl) if (!KENNUNG.test(z.get('name'))) throw fehler('zl.name_form');
  for (const z of zl) if (!HOST_DNS.test(z.get('host')) || /^[0-9.]+$/.test(z.get('host'))) throw fehler('zl.host_form');
  for (const z of zl) if (z.get('port').wert < 1 || z.get('port').wert > 65535) throw fehler('zl.port_grenze');
  for (const z of zl) for (const r of z.get('regeln')) {
    const p = r.get('pfad');
    if (!(r.get('methode') === '*' || METHODEN.includes(r.get('methode'))) || !REGEL_PFAD.test(p) || p.split('/').some((s) => s === '.' || s === '..')) throw fehler('zl.regel_form');
  }
  for (const z of zl) {
    const gebunden = [z.get('kopf'), z.get('wert_vorsatz'), z.get('schluessel_datei')].map((v) => v !== null);
    if (gebunden.some((g) => g !== gebunden[0])) throw fehler('zl.bindung');
    if (gebunden[0]) {
      if (!KOPF_NAME.test(z.get('kopf')) || KOPF_VERBOTEN.includes(z.get('kopf'))) throw fehler('zl.bindung');
      if (!/^[\x20-\x7e]{0,32}$/.test(z.get('wert_vorsatz'))) throw fehler('zl.bindung');
      if (!PFAD_ABS(z.get('schluessel_datei'), 1024)) throw fehler('zl.bindung');
    }
  }
  for (const z of zl) if (!['ablehnen', 'durchreichen'].includes(z.get('upgrade')) || (z.get('kopf') === null && z.get('upgrade') !== 'ablehnen')) throw fehler('zl.upgrade_wert');
  if (!['zugelassen', 'offen', 'abgelehnt'].includes(o.get('ca_zulassung'))) throw fehler('zl.ca_zulassung_wert');
  const namen = zl.map((z) => z.get('name')); const hp = zl.map((z) => `${z.get('host')}:${z.get('port').wert}`);
  if (new Set(namen).size !== namen.length || new Set(hp).size !== hp.length) throw fehler('zl.doppelt');
  const liste = zl.map((z) => ({
    name: z.get('name'), host: z.get('host'), port: z.get('port').wert, schluessel_datei: z.get('schluessel_datei'), kopf: z.get('kopf'),
    wert_vorsatz: z.get('wert_vorsatz'), upgrade: z.get('upgrade'), regeln: z.get('regeln').map((r) => ({ methode: r.get('methode'), pfad: r.get('pfad') })),
  }));
  liste.ca_zulassung = o.get('ca_zulassung');
  return liste;
}

const ABLEHNUNG2 = {
  'mw.protokoll_fehlt': [503, 'Protokoll nicht schreibbar'],
  'mw.anfrage_unlesbar': [400, 'Anfrage nicht lesbar'],
  'mw.ausgelastet': [429, 'zu viele gleichzeitige Anfragen'],
  'mw.nur_connect': [405, 'nur CONNECT'],
  'mw.ziel_nicht_erlaubt': [403, 'Ziel nicht erlaubt'],
  'mw.sni_falsch': [null, ''],
  'mw.host_falsch': [400, 'Host passt nicht zum Tunnel'],
  'mw.pfad_nicht_erlaubt': [403, 'Pfad nicht erlaubt'],
  'mw.methode_falsch': [405, 'Methode nicht erlaubt'],
  'mw.upgrade_abgelehnt': [403, 'Upgrade nicht erlaubt'],
  'mw.anmeldung_an_ungebundenes_ziel': [403, 'Anmeldung an diesem Ziel nicht erlaubt'],
  'mw.koerper_zu_gross': [413, 'Koerper zu gross'],
  'mw.schluessel_fehlt': [503, 'Schluessel nicht lesbar'],
  'mw.ziel_nicht_erreichbar': [502, 'Ziel nicht erreichbar'],
  'mw.ziel_frist': [504, 'Ziel antwortet nicht'],
  'mw.antwort_komprimiert': [502, 'Antwort komprimiert'],
  'mw.leck_gesperrt': [502, 'Antwort des Ziels gesperrt'],
  'mw.zaehlung_fehler': [503, 'Zaehlung nicht schreibbar'],
};
const GRUENDE2 = {
  weitergereicht: [null],
  abgelehnt: ['mw.anfrage_unlesbar', 'mw.ausgelastet', 'mw.nur_connect', 'mw.ziel_nicht_erlaubt', 'mw.sni_falsch', 'mw.host_falsch', 'mw.pfad_nicht_erlaubt',
    'mw.methode_falsch', 'mw.upgrade_abgelehnt', 'mw.anmeldung_an_ungebundenes_ziel', 'mw.koerper_zu_gross', 'mw.zaehlung_fehler'],
  ziel_fehler: ['mw.schluessel_fehlt', 'mw.ziel_nicht_erreichbar', 'mw.ziel_frist', 'mw.antwort_komprimiert'],
  leck_gesperrt: ['mw.leck_gesperrt'],
};
const fehlerKoerper2 = (code) => jcs({ error: { code, message: ABLEHNUNG2[code][1] } });

// Tunnel: CONNECT pruefen (Abschnitt 12.1)
function tunnelPruefen(lage, roh) {
  if (!lage.protokoll_ok) throw fehler('mw.protokoll_fehlt');
  const b = Buffer.from(roh, 'latin1');
  const ende = b.indexOf('\r\n\r\n');
  if (ende < 0 || ende + 4 > KOPF_MAX) throw fehler('mw.anfrage_unlesbar');
  const zeilen = b.subarray(0, ende).toString('latin1').split('\r\n');
  const start = /^([A-Z]{1,16}) ([\x21-\x7e]+) HTTP\/1\.1$/.exec(zeilen[0]);
  if (!start) throw fehler('mw.anfrage_unlesbar');
  for (const z of zeilen.slice(1)) if (!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+:[ \t]*[\x20-\x7e]*$/.test(z)) throw fehler('mw.anfrage_unlesbar');
  if (lage.laufend >= TUNNEL_MAX) throw fehler('mw.ausgelastet');
  if (start[1] !== 'CONNECT') throw fehler('mw.nur_connect');
  const ziel = lage.ziele.find((z) => start[2] === `${z.host}:${z.port}`);
  if (!ziel) throw fehler('mw.ziel_nicht_erlaubt');
  if ((lage.unaufgeloest ?? []).includes(ziel.name)) { const e = fehler('mw.ziel_nicht_erreichbar'); e.ziel = ziel.name; throw e; }
  if (lage.sni !== ziel.host) throw fehler('mw.sni_falsch');
  return ziel;
}

// sicherer Pfad und passende Regel
function pfadSicher(p) {
  return /^\/[\x21-\x7e]*$/.test(p) && !p.includes('//') && !p.includes('\\') && !/%(2e|2f|5c)/i.test(p)
    && !p.split('/').some((s) => s === '.' || s === '..');
}
const regelPasst = (r, pfad) => r.pfad === '/*' || (r.pfad.endsWith('/*') ? pfad.startsWith(r.pfad.slice(0, -1)) : pfad === r.pfad);

// eine Anfrage im Tunnel pruefen und die Bytes an das Ziel bilden (Abschnitt 12.2)
function innenPruefen(lage, ziel, roh) {
  const meldung = { pfad: null, methode: null, pfadregel: null, kopf: null };
  const wirf = (code) => { const e = fehler(code); e.meldung = { ...meldung }; throw e; };
  const b = Buffer.from(roh, 'latin1');
  const ende = b.indexOf('\r\n\r\n');
  if (ende < 0 || ende + 4 > KOPF_MAX) wirf('mw.anfrage_unlesbar');
  const zeilen = b.subarray(0, ende).toString('latin1').split('\r\n');
  const start = /^([A-Z]{1,16}) (\/[\x21-\x7e]*) HTTP\/1\.1$/.exec(zeilen[0]);
  if (!start) wirf('mw.anfrage_unlesbar');
  const kopf = [];
  for (const z of zeilen.slice(1)) {
    const k = /^([!#$%&'*+.^_`|~0-9A-Za-z-]+):[ \t]*([\x20-\x7e]*?)[ \t]*$/.exec(z);
    if (!k) wirf('mw.anfrage_unlesbar');
    kopf.push([k[1], k[2]]);
  }
  // Der Kopf ist gelesen: der Pfad der Zeile steht fest, vor jeder Stufe (Abschnitt 8).
  meldung.pfad = pfadFeld(start[2], lage.pfad_voll === true);
  const werte = (n) => kopf.filter(([k]) => k.toLowerCase() === n).map(([, v]) => v);
  const te = werte('transfer-encoding'); const cl = werte('content-length');
  if (te.length > 1 || (te.length === 1 && te[0].toLowerCase() !== 'chunked') || (te.length && cl.length) || cl.length > 1
    || (cl.length === 1 && !/^(0|[1-9][0-9]{0,15})$/.test(cl[0]))) wirf('mw.anfrage_unlesbar');
  const koerper = b.subarray(ende + 4);
  const host = werte('host');
  if (host.length !== 1 || !(host[0] === ziel.host || host[0] === `${ziel.host}:${ziel.port}`)) wirf('mw.host_falsch');
  const methode = start[1]; const pfad = start[2].split('?')[0];
  if (METHODEN.includes(methode)) meldung.methode = methode;
  if (!pfadSicher(pfad)) wirf('mw.pfad_nicht_erlaubt');
  const passend = ziel.regeln.filter((r) => regelPasst(r, pfad));
  if (!passend.length) wirf('mw.pfad_nicht_erlaubt');
  const mitMethode = passend.filter((r) => r.methode === '*' ? METHODEN.includes(methode) : r.methode === methode);
  if (!mitMethode.length) { meldung.pfadregel = passend[0].pfad; wirf('mw.methode_falsch'); }
  meldung.pfadregel = mitMethode[0].pfad;
  const upgrade = werte('upgrade').length > 0 || werte('connection').some((v) => v.toLowerCase().split(',').map((s) => s.trim()).includes('upgrade'));
  if (upgrade && ziel.upgrade === 'ablehnen') wirf('mw.upgrade_abgelehnt');
  const anmeldung = kopf.some(([k]) => ANMELDEKOEPFE.includes(k.toLowerCase()));
  if (ziel.kopf === null && anmeldung) { meldung.kopf = 'abgewiesen'; wirf('mw.anmeldung_an_ungebundenes_ziel'); }
  if (cl.length === 1 && Number(cl[0]) > KOERPER_MAX) wirf('mw.koerper_zu_gross');
  if (cl.length === 1 && koerper.length !== Number(cl[0])) wirf('mw.anfrage_unlesbar');
  meldung.kopf = ziel.kopf === null ? 'keiner' : 'ersetzt';
  const wsDurch = upgrade && ziel.upgrade === 'durchreichen';
  const verboten = DIREKT_VERBOTEN.filter((h) => h !== 'sec-websocket-extensions' || !M('M-W1'));   // M-W1: die Erweiterung bleibt am gebundenen Ziel
  const weg = new Set([...ANMELDEKOEPFE, ...(ziel.kopf ? [ziel.kopf, 'accept-encoding', ...verboten] : []), ...HOP.filter((h) => !(upgrade && ziel.upgrade === 'durchreichen' && (h === 'upgrade' || h === 'connection')))]);
  const behalten = kopf.filter(([k]) => !weg.has(k.toLowerCase()));
  const schluessel = ziel.kopf === null ? null : lage.schluessel[ziel.name];
  if (ziel.kopf !== null && schluessel === undefined) wirf('mw.schluessel_fehlt');
  const neu = [...behalten, ...(ziel.kopf ? [['accept-encoding', 'identity']] : []), ...(upgrade && ziel.upgrade === 'durchreichen' ? [] : [['connection', 'close']]),
    ...(ziel.kopf && !(M('M-W6') && wsDurch) ? [[ziel.kopf, ziel.wert_vorsatz + schluessel]] : [])];
  const an_ziel = [zeilen[0], ...neu.map(([k, v]) => `${k}: ${v}`)].join('\r\n') + '\r\n\r\n' + koerper.toString('latin1');
  return { an_ziel, meldung };
}

// Rahmen des Dienstes nach der 101 lesen (RFC 6455, Abschnitt 5.2) und die Leck-Sperre auf den Nutzdaten laufen lassen (Abschnitt 13.3, W3 bis W5).
// Die Grenzen der Teile (TCP-Segmente) haben keine Wirkung. Freigegeben wird alles bis zum ersten der zurueckgehaltenen Nutzdaten-Bytes (die letzten k1 der
// Datenrahmen); Rahmenkoepfe und Steuerrahmen dahinter warten mit. Bei Abriss geht nichts Weiteres an den Harness, bei Ende der Verbindung geht alles durch.
const WS_OPCODES = [0x0, 0x1, 0x2, 0x8, 0x9, 0xa];
function wsPruefen(keys, k1, kopf, teile) {
  const s = Buffer.from(teile.join(''), 'latin1');
  const abgerissen = (roh, nutz, grund) => ({ art: 'abgerissen', status: 101, kopf, gesendet: roh.subarray(0, freigabe(roh, nutz)).toString('latin1'), grund });
  const freigabe = (roh, nutz) => { const h = Math.min(k1, nutz.length); return h === 0 ? roh.length : nutz[nutz.length - h]; };
  let roh = Buffer.alloc(0); const nutz = [];   // alle gelesenen Bytes; Stellen der Nutzdaten-Bytes der Datenrahmen darin
  let daten = Buffer.alloc(0);                  // die letzten k1 Nutzdaten-Bytes (Rest der Sperre ueber alle Datenrahmen)
  let offen = false;                            // eine geteilte Nachricht ist offen (FIN=0 kam, die abschliessende Fortsetzung fehlt)
  let i = 0;
  while (i < s.length) {
    if (s.length - i < 2) { roh = Buffer.concat([roh, s.subarray(i)]); break; }
    const b0 = s[i]; const b1 = s[i + 1];
    const opcode = b0 & 0x0f;
    if (!M('M-W5') && (b0 & 0x70)) return abgerissen(roh, nutz, 'mw.antwort_komprimiert');
    if ((b1 & 0x80) || !WS_OPCODES.includes(opcode)) return abgerissen(roh, nutz, 'mw.ziel_nicht_erreichbar');
    let laenge = b1 & 0x7f; let kopfLaenge = 2;
    if (laenge === 126) { if (s.length - i < 4) { roh = Buffer.concat([roh, s.subarray(i)]); break; } laenge = s.readUInt16BE(i + 2); kopfLaenge = 4; }
    else if (laenge === 127) { if (s.length - i < 10) { roh = Buffer.concat([roh, s.subarray(i)]); break; } const l = s.readBigUInt64BE(i + 2); laenge = l > BigInt(KOERPER_MAX) ? KOERPER_MAX + 1 : Number(l); kopfLaenge = 10; }
    if (laenge > KOERPER_MAX) return abgerissen(roh, nutz, 'mw.ziel_nicht_erreichbar');
    // Regelwidrige Rahmen (Nachtrag 2, RFC 6455 Abschnitt 5.4 und 5.5); M-W7: diese Regeln fehlen
    if (!M('M-W7')) {
      if (opcode >= 0x8 && (!(b0 & 0x80) || laenge > 125)) return abgerissen(roh, nutz, 'mw.ziel_nicht_erreichbar');
      if (opcode === 0x0 && !offen) return abgerissen(roh, nutz, 'mw.ziel_nicht_erreichbar');
      if ((opcode === 0x1 || opcode === 0x2) && offen) return abgerissen(roh, nutz, 'mw.ziel_nicht_erreichbar');
    }
    const nutzdaten = s.subarray(i + kopfLaenge, i + kopfLaenge + laenge);
    const steuer = opcode >= 0x8;
    const suche = steuer ? nutzdaten : Buffer.concat([daten, nutzdaten]);
    if (keys.some((k) => suche.includes(k))) return abgerissen(roh, nutz, 'mw.leck_gesperrt');
    const von = roh.length + kopfLaenge;
    roh = Buffer.concat([roh, s.subarray(i, i + kopfLaenge + nutzdaten.length)]);
    if (!steuer) { for (let j = 0; j < nutzdaten.length; j++) nutz.push(von + j); daten = suche.subarray(Math.max(0, suche.length - k1)); }
    else if (M('M-W4')) daten = Buffer.alloc(0);  // M-W4: ein Steuerrahmen setzt die Sperre der Datenrahmen zurueck
    if (!steuer) offen = !(b0 & 0x80);
    i += kopfLaenge + laenge;
  }
  return { art: 'antwort', status: 101, kopf, koerper: roh.toString('latin1'), grund: null };
}

// Antwort des Ziels an den Harness im Tunnel (Abschnitt 12.3)
function antwortDurch2(schluesselListe, ziel) {
  const umstellen = ziel.status === 101 && ziel.upgrade_ziel === 'durchreichen';
  const keys = schluesselListe.map((s) => Buffer.from(s, 'latin1'));
  const k1 = Math.max(...keys.map((k) => k.length)) - 1;
  const ersetzt = (code) => ({ art: 'ersetzt', status: ABLEHNUNG2[code][0], kopf: [['content-type', 'application/json'], ['content-length', String(Buffer.byteLength(fehlerKoerper2(code)))]], koerper: fehlerKoerper2(code), grund: code });
  const enc = ziel.kopf.filter(([k]) => k.toLowerCase() === 'content-encoding').map(([, v]) => v.toLowerCase());
  const komprimiert = enc.some((v) => v !== 'identity');
  if (komprimiert && ziel.gebunden) return ersetzt('mw.antwort_komprimiert');  // fail closed: die Leck-Sperre saehe nichts
  // WebSocket (Abschnitt 13.3, W2): eine ausgehandelte Erweiterung macht die Sperre blind; statt der 101 kommt 502
  if (umstellen && ziel.gebunden && !M('M-W2') && ziel.kopf.some(([k]) => k.toLowerCase() === 'sec-websocket-extensions')) return ersetzt('mw.antwort_komprimiert');
  // ungebundenes Ziel: es hat nie einen Schluessel bekommen; ein komprimierter Koerper geht ungelesen durch, die Koepfe werden geprueft
  const weg = new Set([...HOP.filter((h) => !(umstellen && (h === 'upgrade' || h === 'connection'))), 'transfer-encoding', 'content-length', 'set-cookie', 'proxy-authenticate']);
  const kopf = ziel.kopf.filter(([k]) => !weg.has(k.toLowerCase()));
  if (kopf.some(([, v]) => keys.some((k) => Buffer.from(v, 'latin1').includes(k)))) return ersetzt('mw.leck_gesperrt');
  if (umstellen && !M('M-W3')) return wsPruefen(keys, k1, [...kopf], ziel.teile);  // M-W3: Sperre ueber die rohen Bytes statt ueber die Nutzdaten
  const mitLaenge = ziel.laenge !== null;
  const rahmen = umstellen ? [] : mitLaenge ? [['content-length', String(ziel.laenge)]] : [['transfer-encoding', 'chunked']];
  let puffer = Buffer.alloc(0); let gesendet = Buffer.alloc(0); let kopfGesendet = false;
  for (const t of ziel.teile) {
    puffer = Buffer.concat([puffer, Buffer.from(t, 'latin1')]);
    if (!komprimiert && keys.some((k) => puffer.includes(k))) {
      return kopfGesendet ? { art: 'abgerissen', status: ziel.status, kopf: [...kopf, ...rahmen], gesendet: gesendet.toString('latin1'), grund: 'mw.leck_gesperrt' } : ersetzt('mw.leck_gesperrt');
    }
    if (puffer.length > k1) { gesendet = Buffer.concat([gesendet, puffer.subarray(0, puffer.length - k1)]); puffer = puffer.subarray(puffer.length - k1); kopfGesendet = true; }
  }
  gesendet = Buffer.concat([gesendet, puffer]);
  return { art: 'antwort', status: ziel.status, kopf: [...kopf, ...rahmen], koerper: gesendet.toString('latin1'), grund: null };
}

// Protokollzeile der Betriebsart 2 (Abschnitt 13)
const ZEILE2_FELDER = ['aktionsname', 'betriebsart', 'ergebnis', 'fassung', 'grund', 'host', 'kopf', 'methode', 'pfad', 'pfadregel', 'route', 'sitzung', 'status', 'vorher', 'zeit', 'ziel'];
function zeileKopf(a, vorher) {
  const f = (feld, ok) => { if (!ok) throw fehler(`feld_ungueltig:${feld}`); };
  f('betriebsart', a.betriebsart === 'kopf-einsetzen');
  f('route', KENNUNG.test(a.route));
  f('sitzung', KENNUNG.test(a.sitzung));
  f('ziel', a.ziel === null || KENNUNG.test(a.ziel));
  f('methode', a.methode === null || METHODEN.includes(a.methode));
  f('pfadregel', a.pfadregel === null || (REGEL_PFAD.test(a.pfadregel) && a.ziel !== null));
  f('kopf', [null, 'ersetzt', 'keiner', 'abgewiesen'].includes(a.kopf) && (a.ergebnis !== 'weitergereicht' || ['ersetzt', 'keiner'].includes(a.kopf))
    && (a.kopf !== 'abgewiesen' || a.grund === 'mw.anmeldung_an_ungebundenes_ziel'));
  f('ergebnis', a.ergebnis in GRUENDE2);
  f('grund', GRUENDE2[a.ergebnis].includes(a.grund));
  f('status', a.status === null ? a.grund === 'mw.sni_falsch' : Number.isInteger(a.status) && a.status >= 100 && a.status <= 599);
  f('pfad', a.pfad === null || (pfadFeldOk(a.pfad) && (a.ergebnis === 'abgelehnt' || a.methode === 'GET')));
  // Der Host steht genau dort, wo ziel steht, und kommt aus der geprueften Zielliste.
  f('host', (a.host === null) === (a.ziel === null) && (a.host === null || HOST_DNS.test(a.host)));
  f('vorher', HEX.test(vorher));
  const zeit = zeitText(a.zeit_ms);
  return jcs({ aktionsname: 'modellweg.aufruf', betriebsart: a.betriebsart, ergebnis: a.ergebnis, fassung: 2, grund: a.grund, host: a.host, kopf: a.kopf,
    methode: a.methode, pfad: a.pfad, pfadregel: a.pfadregel, route: a.route, sitzung: a.sitzung, status: a.status, vorher, zeit, ziel: a.ziel });
}
function zeileKopfVonHand(a, vorher) {
  const s = (v) => (v === null ? 'null' : typeof v === 'number' ? String(v) : `"${v}"`);
  const w = { ...a, aktionsname: 'modellweg.aufruf', fassung: 2, vorher, zeit: new Date(a.zeit_ms).toISOString() };
  return `{${ZEILE2_FELDER.map((k) => `"${k}":${s(w[k])}`).join(',')}}`;
}

// Aufruf der Betriebsart 2 (gilt ab dem Bau)
const OPTIONEN2 = ['--socket', '--socket-rechte', '--sitzung', '--route', '--protokoll', '--zielliste', '--ca-schluessel', '--ca-zertifikat'];
function aufrufLesen2(argv) {
  const w = argvLesen(argv, [...OPTIONEN2, ...OPTIONEN]);
  if (!w.has('--betriebsart')) w.set('--betriebsart', 'openai-lokal');
  if (!['openai-lokal', 'kopf-einsetzen'].includes(w.get('--betriebsart'))) throw fehler('mw.betriebsart_unbekannt');
  if (w.get('--betriebsart') === 'openai-lokal') return aufrufLesen(argv);
  markerPruefen(w);
  const ohneMarker = [...w.keys()].filter((k) => !MARKER_OPT.includes(k) && k !== PFAD_VOLL_OPT);
  if (ohneMarker.length !== OPTIONEN2.length + 1 || ohneMarker.some((k) => k !== '--betriebsart' && !OPTIONEN2.includes(k))) throw fehler('mw.aufruf');
  if (!PFAD_ABS(w.get('--socket'), 107) || w.get('--socket-rechte') !== '0600' || !KENNUNG.test(w.get('--sitzung')) || !KENNUNG.test(w.get('--route'))
    || !PFAD_ABS(w.get('--protokoll'), 1024) || !PFAD_ABS(w.get('--zielliste'), 1024) || !PFAD_ABS(w.get('--ca-schluessel'), 1024) || !PFAD_ABS(w.get('--ca-zertifikat'), 1024)) throw fehler('mw.aufruf');
  return w;
}
START_CODES['mw.ziele_ungueltig'] = [78, 'Zielliste ungueltig'];
START_CODES['mw.ca_ungueltig'] = [78, 'CA-Datei ungueltig'];
START_CODES['mw.ca_nicht_zugelassen'] = [78, 'Lokale CA nicht zugelassen'];

// Gegenprobe E5 (Abschnitt 14): Modell der Suche im Einschluss
function einschlussProbe(aufbau, marke) {
  const funde = [];
  for (const [k, v] of Object.entries(aufbau.umgebung)) if (v.includes(marke)) funde.push(`umgebung:${k}`);
  for (const [p, inhalt] of Object.entries(aufbau.dateien)) if (inhalt.includes(marke)) funde.push(`datei:${p}`);
  if (aufbau.dbus_wirt) funde.push('secret-tool');
  if (aufbau.gh_angemeldet) funde.push('gh auth token');
  if (aufbau.speicher_lesbar) funde.push('/proc/<cli-pid>/mem');
  return funde;
}

// --- Ausgangslage der Betriebsart 2 ---

const MARKE = 'wb-marke-echtes-token-7d1c0e';  // im Aussentest eine Marke im Format des echten Tokens; hier bewusst nicht tokenfoermig
const PLATZ = 'wb-platzhalter-0000';  // im Aussentest ein Platzhalter im Format des echten Tokens
const ZL_VORF = {
  fassung: 1,
  ca_zulassung: 'zugelassen',
  ziele: [
    { name: 'github-api', host: 'api.github.com', port: 443, schluessel_datei: '/srv/beispiel/.local/state/wb/modellweg/copilot.schluessel', kopf: 'authorization', wert_vorsatz: 'Bearer ',
      regeln: [{ methode: 'GET', pfad: '/copilot_internal/user' }, { methode: 'GET', pfad: '/copilot/mcp_registry' }], upgrade: 'durchreichen' },
    { name: 'copilot-api', host: 'api.githubcopilot.com', port: 443, schluessel_datei: '/srv/beispiel/.local/state/wb/modellweg/copilot.schluessel', kopf: 'authorization', wert_vorsatz: 'Bearer ',
      regeln: [{ methode: '*', pfad: '/*' }], upgrade: 'durchreichen' },
    { name: 'copilot-plan', host: 'api.individual.githubcopilot.com', port: 443, schluessel_datei: '/srv/beispiel/.local/state/wb/modellweg/copilot.schluessel', kopf: 'authorization', wert_vorsatz: 'Bearer ',
      regeln: [{ methode: '*', pfad: '/*' }], upgrade: 'durchreichen' },
    { name: 'copilot-telemetrie', host: 'telemetry.individual.githubcopilot.com', port: 443, schluessel_datei: null, kopf: null, wert_vorsatz: null,
      regeln: [{ methode: 'POST', pfad: '/telemetry' }], upgrade: 'ablehnen' },
    { name: 'copilot-exp', host: 'exp.individual.githubcopilot.com', port: 443, schluessel_datei: null, kopf: null, wert_vorsatz: null,
      regeln: [{ methode: 'POST', pfad: '/api/v1/assignments' }], upgrade: 'ablehnen' },
  ],
};
const ZL_TEXT = JSON.stringify(ZL_VORF, null, 1) + '\n';
const ZIELE = zieleLesen(ZL_TEXT);
const LAGE2 = { betriebsart: 'kopf-einsetzen', ziele: ZIELE, schluessel: { 'github-api': MARKE, 'copilot-api': MARKE, 'copilot-plan': MARKE }, laufend: 0, protokoll_ok: true };
const zielVon = (name) => ZIELE.find((z) => z.name === name);
// host der Zeile: der Host des zugeordneten Ziels aus der Zielliste, klein und ohne Port (Abschnitt 13.4).
const hostVon = (name) => (name ? zielVon(name).host : null);
// die Felder der Zeile, die eine Anfrage im Tunnel setzt: der Pfad nur bei einer Ablehnung und bei GET.
const zeileM = (ergebnis, zielName, m) => ({ methode: m.methode, pfadregel: m.pfadregel, kopf: m.kopf, pfad: pfadInZeile(m.pfad, ergebnis, m.methode), host: hostVon(zielName) });
const innen = (methode, pfad, host, kopf = [], koerper = '') => `${methode} ${pfad} HTTP/1.1\r\nHost: ${host}\r\n${kopf.map(([k, v]) => `${k}: ${v}\r\n`).join('')}\r\n${koerper}`;
const COPILOT_KOEPFE = [['accept', 'application/json'], ['accept-encoding', 'gzip, br'], ['authorization', `Bearer ${PLATZ}`], ['copilot-harness-id', 'h-1'], ['copilot-integration-id', 'copilot-developer-cli'], ['editor-version', 'copilot/1.0.88'], ['openai-intent', 'conversation-agent'], ['runtime-client-version', '1'], ['user-agent', 'copilot/1.0.88'], ['x-client-machine-id', 'm-1'], ['x-copilot-client-exp-assignment-context', 'c-1'], ['x-copilot-exp-flight-names', 'f-1'], ['x-github-api-version', '2025-05-01'], ['x-initiator', 'user'], ['x-interaction-id', 'i-1']];  // Kopfnamen gemessen (decision), Werte erfunden

function baueKopf() {
  // Zielliste
  const tief = (o) => JSON.parse(JSON.stringify(o));
  const mit = (f) => { const o = tief(ZL_VORF); f(o); return JSON.stringify(o, null, 1) + '\n'; };
  const ZL_FAELLE = [
    ['vorfuehrung', 'die Vorfuehr-Zielliste (Werte von endpoints.* sind Beispiele der Messung, fuer den Firmenplan ungemessen)', ZL_TEXT, null],
    ['zu_gross', 'mehr als 65536 Byte', mit((o) => { o.x = 'a'.repeat(ZL_MAX); }), 'zl.zu_gross'],
    ['doppelt_tief', 'doppelter Schluessel in einer Regel', ZL_TEXT.replace('"methode": "POST",', '"methode": "POST", "methode": "GET",'), 'zl.json_ungueltig'],
    ['unbekannt', 'unbekannter Schluessel im Ziel', mit((o) => { o.ziele[0].schluessel = MARKE; }), 'zl.schluessel_unbekannt'],
    ['upgrade_fehlt', 'Ziel ohne upgrade', mit((o) => { delete o.ziele[1].upgrade; }), 'zl.schluessel_fehlt'],
    ['port_text', 'Port als Text', mit((o) => { o.ziele[0].port = '443'; }), 'zl.typ_falsch'],
    ['fassung_2', 'fassung 2', mit((o) => { o.fassung = 2; }), 'zl.fassung_unbekannt'],
    ['ohne_regel', 'Ziel ohne Regel', mit((o) => { o.ziele[0].regeln = []; }), 'zl.anzahl'],
    ['name_gross', 'Name mit Grossbuchstaben', mit((o) => { o.ziele[0].name = 'GitHub'; }), 'zl.name_form'],
    ['host_ip', 'Host als IP-Adresse', mit((o) => { o.ziele[0].host = '140.82.112.6'; }), 'zl.host_form'],
    ['host_stern', 'Host mit Platzhalter *', mit((o) => { o.ziele[2].host = '*.githubcopilot.com'; }), 'zl.host_form'],
    ['host_gross', 'Host mit Grossbuchstaben', mit((o) => { o.ziele[0].host = 'API.github.com'; }), 'zl.host_form'],
    ['port_null', 'Port 0', mit((o) => { o.ziele[0].port = 0; }), 'zl.port_grenze'],
    ['regel_methode', 'Methode CONNECT in einer Regel', mit((o) => { o.ziele[0].regeln[0].methode = 'CONNECT'; }), 'zl.regel_form'],
    ['regel_punkt', 'Pfad mit ..', mit((o) => { o.ziele[0].regeln[0].pfad = '/copilot_internal/../user/*'; }), 'zl.regel_form'],
    ['regel_stern_mitte', 'Stern mitten im Pfad', mit((o) => { o.ziele[0].regeln[0].pfad = '/copilot_*/user'; }), 'zl.regel_form'],
    ['regel_schraegstrich', 'Teilbaum mit / statt /* am Ende', mit((o) => { o.ziele[0].regeln[0].pfad = '/copilot_internal/'; }), 'zl.regel_form'],
    ['regel_33', '33 Regeln', mit((o) => { o.ziele[1].regeln = Array.from({ length: 33 }, (_, i) => ({ methode: 'GET', pfad: `/p${i}` })); }), 'zl.anzahl'],
    ['bindung_halb', 'kopf gesetzt, schluessel_datei null', mit((o) => { o.ziele[0].schluessel_datei = null; }), 'zl.bindung'],
    ['bindung_host', 'kopf host', mit((o) => { o.ziele[0].kopf = 'host'; }), 'zl.bindung'],
    ['bindung_relativ', 'relative Schluesseldatei', mit((o) => { o.ziele[0].schluessel_datei = 'copilot.schluessel'; }), 'zl.bindung'],
    ['upgrade_wert', 'upgrade ja', mit((o) => { o.ziele[0].upgrade = 'ja'; }), 'zl.upgrade_wert'],
    ['upgrade_ungebunden_durch', 'ein ungebundenes Ziel mit upgrade durchreichen', mit((o) => { o.ziele[3].upgrade = 'durchreichen'; }), 'zl.upgrade_wert'],
    ['upgrade_gebunden_ablehnen', 'ein gebundenes Ziel darf ablehnen', mit((o) => { o.ziele[2].upgrade = 'ablehnen'; }), null],
    ['zulassung_fehlt', 'ohne ca_zulassung', mit((o) => { delete o.ca_zulassung; }), 'zl.schluessel_fehlt'],
    ['zulassung_wert', 'ca_zulassung ja', mit((o) => { o.ca_zulassung = 'ja'; }), 'zl.ca_zulassung_wert'],
    ['zulassung_offen', 'die Firma hat noch nicht geantwortet: Liste gueltig, Start verweigert', mit((o) => { o.ca_zulassung = 'offen'; }), null],
    ['zulassung_abgelehnt', 'die Firma lehnt die lokale CA ab: Liste gueltig, Start verweigert', mit((o) => { o.ca_zulassung = 'abgelehnt'; }), null],
    ['doppelt_host', 'derselbe Host und Port zweimal', mit((o) => { o.ziele[1].host = 'api.github.com'; }), 'zl.doppelt'],
    ['doppelt_name', 'derselbe Name zweimal', mit((o) => { o.ziele[1].name = 'github-api'; }), 'zl.doppelt'],
  ];
  const ziele = [];
  for (const [name, beschreibung, text, code] of ZL_FAELLE) {
    let ist = null; let gelesen = null; try { gelesen = zieleLesen(text); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`zielliste ${name}: Modell ${ist} gegen ${code}`);
    const e = { name, beschreibung };
    if (Buffer.byteLength(text) > 4096) e.text_muster = { basis: 'vorfuehrung', zusatz: 'Schluessel x mit 65536 mal a auf oberster Ebene' }; else e.text = text;
    e.erwartet = code === null
      ? { gueltig: true, ca_erlaubte_namen: [...new Set(gelesen.map((z) => z.host))].sort(), start: gelesen.ca_zulassung === 'zugelassen' ? { bis_schritt: 'zielliste_gut' } : { exit: 78, stderr: `mw.ca_nicht_zugelassen: ${START_CODES['mw.ca_nicht_zugelassen'][1]}\n`, stdout: '' } }
      : { gueltig: false, code, start: { exit: 78, stderr: 'mw.ziele_ungueltig: Zielliste ungueltig\n', stdout: '' } };
    ziele.push(e);
  }
  if (ZL_TEXT.includes(MARKE)) abbruch('Marke in der Zielliste');
  const vorf = ziele.find((z) => z.name === 'vorfuehrung');
  // Die Namen sind eine Liste von Hosts: Vergleich je Eintrag auf Gleichheit, nie als Teil eines Textes
  const vorfNamen = new Set(vorf.erwartet.ca_erlaubte_namen);
  if (vorf.erwartet.ca_erlaubte_namen.length !== ZL_VORF.ziele.length || !['exp.individual.githubcopilot.com', 'telemetry.individual.githubcopilot.com'].every((h) => vorfNamen.has(h))) abbruch('CA-Namen ohne ungebundene Ziele (F3)');

  // Tunnel
  const con = (ziel, kopf = []) => `CONNECT ${ziel} HTTP/1.1\r\nHost: ${ziel}\r\n${kopf.map(([k, v]) => `${k}: ${v}\r\n`).join('')}\r\n`;
  const TUNNEL = [
    ['gueltig', 'CONNECT an ein Ziel der Liste, SNI gleich dem Host', LAGE2, 'api.github.com', con('api.github.com:443'), { ziel: 'github-api' }],
    ['ziel_fremd', 'CONNECT an github.com (nur fuer die Anmeldung ausserhalb)', LAGE2, 'github.com', con('github.com:443'), { code: 'mw.ziel_nicht_erlaubt' }],
    ['ziel_ohne_port', 'CONNECT ohne Port', LAGE2, 'api.github.com', con('api.github.com'), { code: 'mw.ziel_nicht_erlaubt' }],
    ['ziel_port_80', 'CONNECT an Port 80', LAGE2, 'api.github.com', con('api.github.com:80'), { code: 'mw.ziel_nicht_erlaubt' }],
    ['ziel_gross', 'CONNECT mit Grossbuchstaben im Host', LAGE2, 'API.github.com', con('API.github.com:443'), { code: 'mw.ziel_nicht_erlaubt' }],
    ['ziel_ip', 'CONNECT an eine IP-Adresse', LAGE2, '140.82.112.6', con('140.82.112.6:443'), { code: 'mw.ziel_nicht_erlaubt' }],
    ['nur_connect', 'GET in Proxy-Form statt CONNECT', LAGE2, null, 'GET http://api.github.com/user HTTP/1.1\r\nHost: api.github.com\r\n\r\n', { code: 'mw.nur_connect' }],
    ['sni_falsch', 'Tunnel zu api.github.com, im TLS-Gruss SNI eines anderen Hosts', LAGE2, 'evil.beispiel.invalid', con('api.github.com:443'), { code: 'mw.sni_falsch' }],
    ['sni_fehlt', 'Tunnel ohne SNI', LAGE2, null, con('api.github.com:443'), { code: 'mw.sni_falsch' }],
    ['unlesbar', 'kein HTTP', LAGE2, null, 'hallo\r\n\r\n', { code: 'mw.anfrage_unlesbar' }],
    ['letzter_platz', 'einunddreissig Tunnel offen, der zweiunddreissigste kommt noch durch', { ...LAGE2, laufend: 31 }, 'api.github.com', con('api.github.com:443'), { ziel: 'github-api' }],
    ['ausgelastet', 'zweiunddreissig Tunnel offen', { ...LAGE2, laufend: 32 }, 'api.github.com', con('api.github.com:443'), { code: 'mw.ausgelastet' }],
    ['protokoll_fehlt', 'Anhaengen vorher gescheitert', { ...LAGE2, protokoll_ok: false }, 'api.github.com', con('api.github.com:443'), { code: 'mw.protokoll_fehlt' }],
    ['telemetrie_nxdomain', 'ungebundenes Ziel beim Start nicht aufloesbar (gemessen NXDOMAIN): 502 fuer diesen Tunnel, alles andere laeuft', { ...LAGE2, unaufgeloest: ['copilot-telemetrie'] }, 'telemetry.individual.githubcopilot.com', con('telemetry.individual.githubcopilot.com:443'), { code: 'mw.ziel_nicht_erreichbar' }],
    ['andere_trotz_nxdomain', 'derselbe Zustand: ein gebundenes Ziel geht weiter', { ...LAGE2, unaufgeloest: ['copilot-telemetrie'] }, 'api.individual.githubcopilot.com', con('api.individual.githubcopilot.com:443'), { ziel: 'copilot-plan' }],
  ];
  const tunnel = [];
  for (const [name, beschreibung, lage, sni, roh, erw] of TUNNEL) {
    let ist; let zielName = null; try { ist = { ziel: tunnelPruefen({ ...lage, sni }, roh).name }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; zielName = x.ziel ?? null; }
    if (jcs(ist) !== jcs(erw)) abbruch(`tunnel ${name}: Modell ${jcs(ist)} gegen ${jcs(erw)}`);
    const e = erw.code
      ? { ziel_beruehrt: false, antwort: ABLEHNUNG2[erw.code][0] === null ? 'tls_abbruch' : { status: ABLEHNUNG2[erw.code][0], koerper: fehlerKoerper2(erw.code) },
        zeile: erw.code === 'mw.protokoll_fehlt' ? null : { ergebnis: erw.code === 'mw.ziel_nicht_erreichbar' ? 'ziel_fehler' : 'abgelehnt', grund: erw.code, status: ABLEHNUNG2[erw.code][0], ziel: erw.code === 'mw.sni_falsch' ? 'github-api' : zielName, methode: null, pfadregel: null, kopf: null, pfad: null, host: hostVon(erw.code === 'mw.sni_falsch' ? 'github-api' : zielName) } }
      : { antwort: 'HTTP/1.1 200 Connection Established\r\n\r\n', danach: 'TLS-Abschluss mit einem Blattzertifikat fuer den Host, ausgestellt von der Wegwerf-CA', ziel: erw.ziel };
    tunnel.push({ name, beschreibung, lage: { ziele: 'vorfuehrung', laufend: lage.laufend, protokoll_ok: lage.protokoll_ok, unaufgeloest: lage.unaufgeloest ?? [] }, sni, connect: roh, erwartet: e });
  }

  // Anfragen im Tunnel
  const H = 'api.github.com';
  const ANF = [
    ['github_user', 'GET /copilot_internal/user mit den gemessenen Kopfnamen: Platzhalter verworfen, Marke eingesetzt, accept-encoding durch identity ersetzt, alle anderen Koepfe unveraendert', 'github-api', innen('GET', '/copilot_internal/user', H, COPILOT_KOEPFE), { kopf: 'ersetzt', pfadregel: '/copilot_internal/user', methode: 'GET' }],
    ['github_registry', 'GET /copilot/mcp_registry (genaue Regel)', 'github-api', innen('GET', '/copilot/mcp_registry', H, COPILOT_KOEPFE), { kopf: 'ersetzt', pfadregel: '/copilot/mcp_registry', methode: 'GET' }],
    ['github_abfrage', 'Abfrageteil bleibt, die Regel prueft nur den Pfad', 'github-api', innen('GET', '/copilot_internal/user?x=1', H, COPILOT_KOEPFE), { kopf: 'ersetzt', pfadregel: '/copilot_internal/user', methode: 'GET' }],
    ['user_repos', 'GET /user/repos: nicht in der Pfadliste (Mutant 5 macht das gruen)', 'github-api', innen('GET', '/user/repos', H, COPILOT_KOEPFE), { code: 'mw.pfad_nicht_erlaubt' }],
    ['github_post', 'POST /copilot_internal/user: Methode nicht erlaubt', 'github-api', innen('POST', '/copilot_internal/user', H, [...COPILOT_KOEPFE, ['Content-Length', '2']], '{}'), { code: 'mw.methode_falsch', pfadregel: '/copilot_internal/user', methode: 'POST' }],
    ['registry_unterpfad', 'GET /copilot/mcp_registry/x: die genaue Regel deckt keinen Unterpfad', 'github-api', innen('GET', '/copilot/mcp_registry/x', H, COPILOT_KOEPFE), { code: 'mw.pfad_nicht_erlaubt' }],
    ['punkt_segment', 'GET /copilot_internal/../user/repos', 'github-api', innen('GET', '/copilot_internal/../user/repos', H, COPILOT_KOEPFE), { code: 'mw.pfad_nicht_erlaubt' }],
    ['kodiert', 'GET /copilot_internal/%2e%2e/user', 'github-api', innen('GET', '/copilot_internal/%2e%2e/user', H, COPILOT_KOEPFE), { code: 'mw.pfad_nicht_erlaubt' }],
    ['doppelter_strich', 'GET /copilot_internal//user', 'github-api', innen('GET', '//copilot_internal/user', H, COPILOT_KOEPFE), { code: 'mw.pfad_nicht_erlaubt' }],
    ['host_falsch', 'Host im Tunnel nennt ein anderes Ziel der Liste', 'github-api', innen('GET', '/copilot_internal/user', 'api.githubcopilot.com', COPILOT_KOEPFE), { code: 'mw.host_falsch' }],
    ['host_mit_port', 'Host mit :443 ist gueltig', 'github-api', innen('GET', '/copilot_internal/user', 'api.github.com:443', COPILOT_KOEPFE), { kopf: 'ersetzt', pfadregel: '/copilot_internal/user', methode: 'GET' }],
    ['plan_modelle', 'POST /chat/completions am Plan-Host: alle Pfade (bis der echte Lauf sie liefert)', 'copilot-plan',
      innen('POST', '/chat/completions', 'api.individual.githubcopilot.com', [...COPILOT_KOEPFE, ['Content-Type', 'application/json'], ['Content-Length', '2']], '{}'), { kopf: 'ersetzt', pfadregel: '/*', methode: 'POST' }],
    ['plan_chunked', 'chunked im Tunnel geht unveraendert durch, auch der Kopf transfer-encoding (direkter Weg)', 'copilot-plan',
      innen('POST', '/responses', 'api.individual.githubcopilot.com', [...COPILOT_KOEPFE, ['Transfer-Encoding', 'chunked']], '2\r\n{}\r\n0\r\n\r\n'), { kopf: 'ersetzt', pfadregel: '/*', methode: 'POST' }],
    ['upgrade_durchgereicht', 'WebSocket-Upgrade am gebundenen Plan-Host: Upgrade und Connection bleiben, kein connection: close, Schluessel eingesetzt', 'copilot-plan',
      innen('GET', '/responses', 'api.individual.githubcopilot.com', [...COPILOT_KOEPFE, ['Connection', 'Upgrade'], ['Upgrade', 'websocket'], ['Sec-WebSocket-Key', 'dGhlIHNhbXBsZSBub25jZQ=='], ['Sec-WebSocket-Version', '13']]), { kopf: 'ersetzt', pfadregel: '/*', methode: 'GET' }],
    ['upgrade_ungebunden', 'Upgrade am ungebundenen Telemetrie-Ziel: abgelehnt', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['Connection', 'Upgrade'], ['Upgrade', 'websocket'], ['Content-Length', '2']], '[]'), { code: 'mw.upgrade_abgelehnt', pfadregel: '/telemetry', methode: 'POST' }],
    ['telemetrie', 'POST /telemetry ohne Anmeldung an das ungebundene Ziel: kein Schluessel geht hinaus', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['Content-Length', '2']], '[]'), { kopf: 'keiner', pfadregel: '/telemetry', methode: 'POST' }],
    ['telemetrie_mit_anmeldung', 'ungebundenes Ziel mit Authorization: 403 und Zeile mit kopf abgewiesen', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['Authorization', `Bearer ${PLATZ}`], ['Content-Length', '2']], '[]'), { code: 'mw.anmeldung_an_ungebundenes_ziel', pfadregel: '/telemetry', methode: 'POST', kopf: 'abgewiesen' }],
    ['telemetrie_cookie', 'ungebundenes Ziel mit Cookie: Cookie geht unveraendert mit, kein Schluessel (vorlaeufig)', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['Cookie', 'a=b'], ['Content-Length', '2']], '[]'), { kopf: 'keiner', pfadregel: '/telemetry', methode: 'POST' }],
    ['telemetrie_api_key', 'ungebundenes Ziel mit X-Api-Key: geht unveraendert mit, nur Authorization zaehlt als Anmeldung (vorlaeufig)', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['X-Api-Key', 'x'], ['Content-Length', '2']], '[]'), { kopf: 'keiner', pfadregel: '/telemetry', methode: 'POST' }],
    ['exp_assignments', 'POST /api/v1/assignments ohne authorization an das ungebundene exp-Ziel: weitergereicht, accept-encoding unveraendert, kein Schluessel', 'copilot-exp',
      innen('POST', '/api/v1/assignments', 'exp.individual.githubcopilot.com', [['accept-encoding', 'gzip, br'], ['content-type', 'application/json'], ['content-length', '2']], '{}'), { kopf: 'keiner', pfadregel: '/api/v1/assignments', methode: 'POST' }],
    ['exp_mit_authorization', 'authorization an das ungebundene exp-Ziel: 403 und Zeile mit kopf abgewiesen', 'copilot-exp',
      innen('POST', '/api/v1/assignments', 'exp.individual.githubcopilot.com', [['authorization', `Bearer ${PLATZ}`], ['content-length', '2']], '{}'), { code: 'mw.anmeldung_an_ungebundenes_ziel', pfadregel: '/api/v1/assignments', methode: 'POST', kopf: 'abgewiesen' }],
    ['exp_anderer_pfad', 'exp-Ziel: der Pfad gilt genau, /api/v1/assignments/x passt nicht', 'copilot-exp',
      innen('POST', '/api/v1/assignments/x', 'exp.individual.githubcopilot.com', [['content-length', '2']], '{}'), { code: 'mw.pfad_nicht_erlaubt' }],
    ['telemetrie_get', 'GET /telemetry: nur POST erlaubt', 'copilot-telemetrie', innen('GET', '/telemetry', 'telemetry.individual.githubcopilot.com'), { code: 'mw.methode_falsch', pfadregel: '/telemetry', methode: 'GET' }],
    ['methode_fremd', 'unbekannte Methode PROPFIND: methode null in der Zeile', 'copilot-plan', innen('PROPFIND', '/x', 'api.individual.githubcopilot.com', COPILOT_KOEPFE), { code: 'mw.methode_falsch', pfadregel: '/*' }],
    ['zwei_laengen', 'Content-Length und Transfer-Encoding zugleich', 'copilot-plan',
      innen('POST', '/x', 'api.individual.githubcopilot.com', [['Content-Length', '2'], ['Transfer-Encoding', 'chunked']], '{}'), { code: 'mw.anfrage_unlesbar' }],
    ['zu_gross', 'Content-Length 33554433', 'copilot-plan', innen('POST', '/x', 'api.individual.githubcopilot.com', [['Content-Length', '33554433']]), { code: 'mw.koerper_zu_gross', pfadregel: '/*', methode: 'POST' }],
    ['plan_methodenkopf', 'gebundenes Ziel mit X-Http-Method-Override und X-Original-Url: beide Koepfe fallen weg (der Halter verbietet sie)', 'copilot-plan',
      innen('POST', '/chat/completions', 'api.individual.githubcopilot.com', [...COPILOT_KOEPFE, ['X-Http-Method-Override', 'DELETE'], ['X-Original-Url', '/user/repos'], ['Content-Type', 'application/json'], ['Content-Length', '2']], '{}'), { kopf: 'ersetzt', pfadregel: '/*', methode: 'POST' }],
    ['plan_rahmung_bleibt', 'gebundenes Ziel auf dem direkten Weg: Host und Content-Length bleiben, Cookie faellt weg (Entscheid des Betreibers, 25.11.2026)', 'copilot-plan',
      innen('POST', '/chat/completions', 'api.individual.githubcopilot.com', [['Cookie', 'a=b'], ['Content-Type', 'application/json'], ['Content-Length', '2']], '{}'), { kopf: 'ersetzt', pfadregel: '/*', methode: 'POST' }],
    ['telemetrie_methodenkopf', 'ungebundenes Ziel mit X-Http-Method-Override: der Kopf geht unveraendert mit, kein Schluessel', 'copilot-telemetrie',
      innen('POST', '/telemetry', 'telemetry.individual.githubcopilot.com', [['X-Http-Method-Override', 'DELETE'], ['Content-Length', '2']], '[]'), { kopf: 'keiner', pfadregel: '/telemetry', methode: 'POST' }],
  ];
  const anfragen = [];
  for (const [name, beschreibung, zielName, roh, erw] of ANF) {
    const ziel = zielVon(zielName);
    let ist;
    try { const r = innenPruefen(LAGE2, ziel, roh); ist = { an_ziel: r.an_ziel, meldung: r.meldung }; } catch (x) {
      if (!(x instanceof Fehler)) throw x; ist = { code: x.message, meldung: x.meldung };
    }
    const sollCode = erw.code;
    if ((ist.code ?? null) !== (sollCode ?? null)) abbruch(`anfrage2 ${name}: Modell ${ist.code} gegen ${sollCode}`);
    const m = ist.meldung;
    for (const k of ['methode', 'pfadregel', 'kopf']) if (erw[k] !== undefined && m[k] !== erw[k]) abbruch(`anfrage2 ${name}: ${k} ${m[k]} gegen ${erw[k]}`);
    if (ist.an_ziel) {
      if (ist.an_ziel.includes(PLATZ)) abbruch(`anfrage2 ${name}: Platzhalter geht hinaus`);
      const mitMarke = ist.an_ziel.split(MARKE).length - 1;
      if (mitMarke !== (ziel.kopf ? 1 : 0)) abbruch(`anfrage2 ${name}: Marke ${mitMarke}-mal`);
      if (ziel.kopf && (!/\r\naccept-encoding: identity\r\n/.test(ist.an_ziel) || /\r\nAccept-Encoding: /.test(ist.an_ziel))) abbruch(`anfrage2 ${name}: identity nicht gesetzt`);
      if (!ziel.kopf && /accept-encoding: identity/.test(ist.an_ziel)) abbruch(`anfrage2 ${name}: Kodierung am ungebundenen Ziel umgeschrieben`);
      const verbotenRe = new RegExp(`^(${DIREKT_VERBOTEN.join('|')}):`, 'i');
      const rohKopf = roh.slice(0, roh.indexOf('\r\n\r\n')).split('\r\n').slice(1).filter((z) => !/^(authorization|connection|upgrade|keep-alive|te|trailer|proxy-connection):/i.test(z) && !(ziel.kopf && (/^accept-encoding:/i.test(z) || verbotenRe.test(z))) || (/^(connection|upgrade):/i.test(z) && /upgrade/i.test(roh) && ziel.upgrade === 'durchreichen'));
      for (const z of rohKopf) if (!ist.an_ziel.includes('\r\n' + z + '\r\n')) abbruch(`anfrage2 ${name}: Kopf ${z} fehlt`);
      if (!ist.an_ziel.startsWith(roh.split('\r\n')[0] + '\r\n')) abbruch(`anfrage2 ${name}: Anfragezeile veraendert`);
      if (!ist.an_ziel.endsWith('\r\n\r\n' + roh.slice(roh.indexOf('\r\n\r\n') + 4))) abbruch(`anfrage2 ${name}: Koerper veraendert`);
    }
    const e = ist.code
      ? { ziel_beruehrt: false, status: ABLEHNUNG2[ist.code][0], koerper: fehlerKoerper2(ist.code), zeile: { ergebnis: 'abgelehnt', grund: ist.code, status: ABLEHNUNG2[ist.code][0], ziel: zielName, ...zeileM('abgelehnt', zielName, m) } }
      : { ziel_beruehrt: true, an_ziel: ist.an_ziel, zeile: { ergebnis: 'weitergereicht', grund: null, ziel: zielName, ...zeileM('weitergereicht', zielName, m) } };
    anfragen.push({ name, beschreibung, ziel: zielName, anfrage: roh, erwartet: e });
  }

  // Antworten im Tunnel
  const ANT = [
    ['json_unveraendert', 'Antwort mit Koepfen des Dienstes: alle ausser Rahmung und set-cookie gehen mit', { status: 200, gebunden: true, kopf: [['Content-Type', 'application/json'], ['X-GitHub-Request-Id', 'A:B'], ['Set-Cookie', 's=1'], ['Content-Length', '11']], laenge: 11, teile: ['{"login":1}'] }, 'antwort'],
    ['strom_chunked', 'SSE vom Plan-Host ohne Laenge: an den Harness chunked, Bytes gleich', { status: 200, gebunden: true, kopf: [['Content-Type', 'text/event-stream'], ['Transfer-Encoding', 'chunked']], laenge: null, teile: ['data: {"a":1}\n\n', 'data: [DONE]\n\n'] }, 'antwort'],
    ['fehler_401', '401 des Dienstes unveraendert', { status: 401, gebunden: true, kopf: [['Content-Type', 'application/json'], ['Content-Length', '28']], laenge: 28, teile: ['{"message":"Bad credentials"}'.slice(0, 28)] }, 'antwort'],
    ['gzip_gebunden', 'ein gebundenes Ziel antwortet trotz identity komprimiert: verworfen, 502 mw.antwort_komprimiert', { status: 200, gebunden: true, kopf: [['Content-Encoding', 'gzip'], ['Content-Length', '3']], laenge: 3, teile: ['\x1f\x8b\x08'] }, 'ersetzt'],
    ['gzip_ungebunden', 'ein ungebundenes Ziel antwortet komprimiert: ungelesen und unveraendert durch', { status: 200, gebunden: false, kopf: [['Content-Encoding', 'gzip'], ['Content-Length', '3']], laenge: 3, teile: ['\x1f\x8b\x08'] }, 'antwort'],
    ['identity_gebunden', 'ein gebundenes Ziel mit Content-Encoding identity: gelesen und durchgereicht', { status: 200, gebunden: true, kopf: [['Content-Encoding', 'identity'], ['Content-Length', '2']], laenge: 2, teile: ['{}'] }, 'antwort'],
    ['leck_koerper', 'der Dienst spiegelt die Marke im Koerper', { status: 200, gebunden: true, kopf: [['Content-Type', 'application/json']], laenge: null, teile: [`{"token":"${MARKE}"}`] }, 'ersetzt'],
    ['leck_kopf', 'der Dienst spiegelt die Marke in einem Kopf', { status: 200, gebunden: true, kopf: [['X-Echo', `Bearer ${MARKE}`], ['Content-Length', '2']], laenge: 2, teile: ['{}'] }, 'ersetzt'],
    ['websocket_101', 'Umstellung auf WebSocket am gebundenen Ziel: 101 mit Upgrade und Connection, danach die Rahmen Byte fuer Byte', { status: 101, gebunden: true, upgrade_ziel: 'durchreichen', kopf: [['Upgrade', 'websocket'], ['Connection', 'Upgrade'], ['Sec-WebSocket-Accept', 's3pPLMBiTxaQ9kYGzzhZRbK+xOo=']], laenge: null, teile: ['\x81\x05Hallo', '\x81\x02ok'] }, 'antwort'],
    ['websocket_leck', 'die Marke kommt in einem spaeteren WebSocket-Rahmen: Abriss', { status: 101, gebunden: true, upgrade_ziel: 'durchreichen', kopf: [['Upgrade', 'websocket'], ['Connection', 'Upgrade']], laenge: null, teile: ['\x81\x7e\x00\x40' + 'x'.repeat(64), `\x81\x1c${MARKE}`] }, 'abgerissen'],
    ['leck_spaet', 'die Marke kommt spaet im Strom: Abriss', { status: 200, gebunden: true, kopf: [['Content-Type', 'text/event-stream']], laenge: null, teile: ['data: {"a":1}\n\ndata: {"b":2}\n\ndata: {"c":3}\n\n', `data: {"k":"${MARKE}"}\n\n`] }, 'abgerissen'],
  ];
  const antworten = [];
  for (const [name, beschreibung, ziel, art] of ANT) {
    const ist = antwortDurch2([MARKE], ziel);
    if (ist.art !== art) abbruch(`antwort2 ${name}: ${ist.art} gegen ${art}`);
    if (JSON.stringify(ist).includes(MARKE)) abbruch(`antwort2 ${name}: Marke zum Harness`);
    if (ist.art === 'antwort' && ist.koerper !== ziel.teile.join('')) abbruch(`antwort2 ${name}: Koerper veraendert`);
    if (name === 'gzip_ungebunden' && !ist.kopf.some(([k, v]) => k === 'Content-Encoding' && v === 'gzip')) abbruch('gzip: Content-Encoding fehlt');
    if (name === 'gzip_gebunden' && ist.grund !== 'mw.antwort_komprimiert') abbruch('gzip gebunden nicht verworfen');
    if (ist.art === 'antwort' && ist.kopf.some(([k]) => k.toLowerCase() === 'set-cookie')) abbruch(`antwort2 ${name}: set-cookie`);
    const ergebnis = ist.grund === null ? 'weitergereicht' : ist.grund === 'mw.leck_gesperrt' ? 'leck_gesperrt' : 'ziel_fehler';
    antworten.push({ name, beschreibung, ziel_antwort: ziel, erwartet: { an_harness: { art: ist.art, status: ist.status, kopf: ist.kopf, ...(ist.koerper !== undefined ? { koerper: ist.koerper } : { gesendet: ist.gesendet }) }, zeile: { ergebnis, grund: ist.grund, status: ist.status } } });
  }

  // WebSocket (Abschnitt 13.3, W1 bis W6): Anfragen, Antworten mit Rahmen des Dienstes und die Richtung vom Harness zum Dienst
  const WS_KOPF = [['Connection', 'Upgrade'], ['Upgrade', 'websocket'], ['Sec-WebSocket-Key', 'dGhlIHNhbXBsZSBub25jZQ=='], ['Sec-WebSocket-Version', '13']];
  const PLAN = 'api.individual.githubcopilot.com';
  const wsAnf = (extra) => innen('GET', '/responses', PLAN, [...COPILOT_KOEPFE, ...extra, ...WS_KOPF]);
  // Hand gebildete Bytes an das Ziel: Koepfe in der Reihenfolge des Harness, ohne authorization, accept-encoding und jeden
  // Namen der direkten Liste (darunter die Erweiterungen); Host bleibt, dann identity und der Schluessel
  const wsAnZiel = (extraBleibt) => `GET /responses HTTP/1.1\r\nHost: ${PLAN}\r\n${COPILOT_KOEPFE.filter(([k]) => k !== 'authorization' && k !== 'accept-encoding' && !DIREKT_VERBOTEN.includes(k.toLowerCase())).map(([k, v]) => `${k}: ${v}\r\n`).join('')}${extraBleibt}Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\naccept-encoding: identity\r\nauthorization: Bearer ${MARKE}\r\n\r\n`;
  const WSA = [
    ['ws_erweiterung_entfernt', 'Upgrade-Anfrage mit Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits an das gebundene Ziel: geht ohne diesen Kopf hinaus, Schluessel eingesetzt (W1)',
      wsAnf([['Sec-WebSocket-Extensions', 'permessage-deflate; client_max_window_bits']]), wsAnZiel('')],
    ['ws_erweiterung_schreibweisen', 'zwei Erweiterungs-Koepfe in anderer Schreibweise und mit anderem Wert: beide entfernt, jeder Wert (W1)',
      wsAnf([['SEC-WEBSOCKET-EXTENSIONS', 'x-webkit-deflate-frame'], ['sec-websocket-extensions', 'permessage-deflate']]), wsAnZiel('')],
    ['ws_anfrage_tausch', 'Upgrade-Anfrage ohne Erweiterung: der Platzhalter wird gegen die Marke getauscht, Upgrade und Connection bleiben, kein connection: close (Kopf-Tausch in der Upgrade-Anfrage)',
      wsAnf([]), wsAnZiel('')],
  ];
  const wsAnfragen = [];
  for (const [name, beschreibung, anfrage, soll] of WSA) {
    const ziel = zielVon('copilot-plan');
    const r = innenPruefen(LAGE2, ziel, anfrage);
    if (r.an_ziel !== soll) abbruch(`ws anfrage ${name}: Bytes an das Ziel weichen ab:\n${JSON.stringify(r.an_ziel)}\ngegen\n${JSON.stringify(soll)}`);
    if (/sec-websocket-extensions/i.test(r.an_ziel) || r.an_ziel.includes(PLATZ) || r.an_ziel.split(MARKE).length !== 2 || /connection: close/i.test(r.an_ziel)) abbruch(`ws anfrage ${name}: Erweiterung, Platzhalter, Marke oder close`);
    wsAnfragen.push({ name, beschreibung, ziel: 'copilot-plan', anfrage, erwartet: { ziel_beruehrt: true, an_ziel: soll, zeile: { ergebnis: 'weitergereicht', grund: null, ziel: 'copilot-plan', ...zeileM('weitergereicht', 'copilot-plan', r.meldung) } } });
  }

  // Rahmen nach RFC 6455 Abschnitt 5.2, als latin1-Text; form 126 oder 127 erzwingt die lange Laengenform
  const fr = (opcode, nutz, o = {}) => {
    const n = Buffer.byteLength(nutz, 'latin1'); const form = o.form ?? (n < 126 ? 7 : n < 65536 ? 126 : 127);
    const b0 = String.fromCharCode((o.fin === false ? 0 : 0x80) | ((o.rsv ?? 0) << 4) | opcode);
    const l = form === 7 ? String.fromCharCode(n) : form === 126 ? String.fromCharCode(0x7e, n >> 8, n & 255) : '\x7f\x00\x00\x00\x00' + String.fromCharCode((n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255);
    return b0 + l + nutz;
  };
  const K101 = [['Upgrade', 'websocket'], ['Connection', 'Upgrade'], ['Sec-WebSocket-Accept', 's3pPLMBiTxaQ9kYGzzhZRbK+xOo=']];
  const antw101 = (teile, extra = []) => ({ status: 101, gebunden: true, upgrade_ziel: 'durchreichen', kopf: [...K101, ...extra], laenge: null, teile });
  const VOR = fr(1, 'a'.repeat(40));      // ein harmloser Rahmen davor: 40 Nutzdaten-Bytes, die letzten 27 (k1) bleiben zurueck
  const VOR_AUS = VOR.slice(0, 2 + 40 - 27);
  const M1 = MARKE.slice(0, 14); const M2 = MARKE.slice(14);
  const T1 = fr(1, `xx${M1}`, { fin: false }); const T2 = fr(0, M2);
  // [name, beschreibung, ziel_antwort, art, grund, gesendet oder Koerper]; art, grund und die Bytes sind von Hand gesetzt, das Modell rechnet nach
  const WSZ = [
    ['ws_erweiterung_101', 'die 101 nennt Sec-WebSocket-Extensions: permessage-deflate: statt der 101 kommt 502 mw.antwort_komprimiert, nichts vom Strom geht hinaus (W2)',
      antw101([fr(1, 'ok')], [['Sec-WebSocket-Extensions', 'permessage-deflate']]), 'ersetzt', 'mw.antwort_komprimiert', null],
    ['ws_erweiterung_101_schreibweise', 'andere Schreibweise und anderer Wert der Erweiterung in der 101: ebenso 502 (W2)',
      antw101([], [['sec-websocket-extensions', 'x-webkit-deflate-frame']]), 'ersetzt', 'mw.antwort_komprimiert', null],
    ['ws_marke_geteilt', 'die Marke steht in zwei Teilen einer Textnachricht (erster Rahmen FIN=0, Fortfuehrung FIN=1): Abriss mw.leck_gesperrt; vom ersten Rahmen ging nur der Kopf hinaus (W3)',
      antw101([T1, T2]), 'abgerissen', 'mw.leck_gesperrt', T1.slice(0, 2)],
    ['ws_marke_geteilt_ping', 'dasselbe mit einem Ping zwischen den beiden Teilen und einem Schnitt mitten im zweiten Rahmen: Abriss, der Ping wartete hinter den zurueckgehaltenen Bytes (W3)',
      antw101([T1, fr(9, 'p'), T2.slice(0, 5), T2.slice(5)]), 'abgerissen', 'mw.leck_gesperrt', T1.slice(0, 2)],
    ['ws_marke_im_ping', 'die ganze Marke in einem Ping: Abriss; vom Rahmen davor ging nur alles ausser den letzten 27 Nutzdaten-Bytes hinaus (W3)',
      antw101([VOR, fr(9, MARKE)]), 'abgerissen', 'mw.leck_gesperrt', VOR_AUS],
    ['ws_marke_im_close', 'die ganze Marke im Grund eines Close-Rahmens: Abriss (W3, jeder Steuerrahmen fuer sich)',
      antw101([VOR, fr(8, `\x03\xe8${MARKE}`)]), 'abgerissen', 'mw.leck_gesperrt', VOR_AUS],
    ['ws_rsv1', 'ein Rahmen des Dienstes mit gesetztem RSV1: Abriss mw.antwort_komprimiert (W4)',
      antw101([VOR, '\xc1\x02ab']), 'abgerissen', 'mw.antwort_komprimiert', VOR_AUS],
    ['ws_rsv2', 'RSV2: Abriss mw.antwort_komprimiert (W4)', antw101([VOR, '\xa1\x02ab']), 'abgerissen', 'mw.antwort_komprimiert', VOR_AUS],
    ['ws_rsv3', 'RSV3: Abriss mw.antwort_komprimiert (W4)', antw101([VOR, '\x91\x02ab']), 'abgerissen', 'mw.antwort_komprimiert', VOR_AUS],
    ['ws_maske', 'ein Rahmen des Dienstes mit gesetztem Maskenbit: Abriss mw.ziel_nicht_erreichbar (W4)',
      antw101([VOR, '\x81\x82\x01\x02\x03\x04ab']), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_opcode_3', 'unbekannter Opcode 0x3: Abriss mw.ziel_nicht_erreichbar (W4)', antw101([VOR, '\x83\x01a']), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_laenge_zu_gross', 'Laenge 33554433 in der 127-Form: Abriss mw.ziel_nicht_erreichbar, schon beim Kopf (W4)',
      antw101([VOR, '\x82\x7f\x00\x00\x00\x00\x02\x00\x00\x01']), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_ping_fin0', 'ein Ping mit FIN=0: Abriss mw.ziel_nicht_erreichbar (Nachtrag 2)', antw101([VOR, '\x09\x01p']), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_ping_laenge_126', 'ein Ping mit der Laenge 126: Abriss mw.ziel_nicht_erreichbar (Nachtrag 2)', antw101([VOR, fr(9, 'p'.repeat(126))]), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_fortsetzung_ohne_anfang', 'eine Fortsetzung ohne offene geteilte Nachricht: Abriss mw.ziel_nicht_erreichbar (Nachtrag 2)', antw101([VOR, fr(0, 'ab')]), 'abgerissen', 'mw.ziel_nicht_erreichbar', VOR_AUS],
    ['ws_text_in_geteilter', 'ein neuer Textrahmen, waehrend eine geteilte Nachricht offen ist: Abriss mw.ziel_nicht_erreichbar (Nachtrag 2)',
      antw101([fr(1, 'a'.repeat(40), { fin: false }), fr(1, 'cd')]), 'abgerissen', 'mw.ziel_nicht_erreichbar', fr(1, 'a'.repeat(40), { fin: false }).slice(0, 2 + 40 - 27)],
    ['ws_abgeschnitten', 'der Rahmen am Ende der Verbindung ist abgeschnitten (Laenge 10, nur 4 Bytes da): alles geht hinaus, was die Sperre gesehen hat, kein Abriss (Nachtrag 2)',
      antw101([VOR, '\x81\x0aabcd']), 'antwort', null, VOR + '\x81\x0aabcd'],
    ['ws_abgeschnitten_kopf', 'das Ende der Verbindung mitten im Rahmenkopf (nur das erste Byte da): alles geht hinaus, kein Abriss (Nachtrag 2)',
      antw101([VOR, '\x81']), 'antwort', null, VOR + '\x81'],
    ['ws_laengen_126_127', 'Rahmen in der 126- und der 127-Laengenform ohne Marke: unveraendert durch, die Laenge ist richtig gelesen (W3)',
      antw101([fr(1, 'a'.repeat(200)), fr(2, 'b'.repeat(300), { form: 127 })]), 'antwort', null, fr(1, 'a'.repeat(200)) + fr(2, 'b'.repeat(300), { form: 127 })],
    ['ws_praefix_am_ende', 'ein Praefix der Marke am Ende eines Rahmens, danach Ende der Verbindung ohne den Rest: alle Bytes kommen an, nichts bleibt zurueck (W3)',
      antw101([fr(1, `ok${MARKE.slice(0, 27)}`)]), 'antwort', null, fr(1, `ok${MARKE.slice(0, 27)}`)],
    ['ws_praefix_dann_anderes', 'ein Praefix der Marke am Ende des ersten Teils, im zweiten Teil etwas anderes: kein Fund, alle Bytes kommen an (W3)',
      antw101([fr(1, MARKE.slice(0, 20), { fin: false }), fr(0, 'XYZ')]), 'antwort', null, fr(1, MARKE.slice(0, 20), { fin: false }) + fr(0, 'XYZ')],
    ['ws_steuer_dazwischen', 'Ping zwischen zwei Teilen einer Nachricht und ein Close ohne Marke: unveraendert durch (W3)',
      antw101([fr(1, 'ab', { fin: false }), fr(9, 'p'), fr(0, 'cd'), fr(8, '\x03\xe8')]), 'antwort', null, fr(1, 'ab', { fin: false }) + fr(9, 'p') + fr(0, 'cd') + fr(8, '\x03\xe8')],
  ];
  const wsAntworten = [];
  for (const [name, beschreibung, ziel, art, grund, bytes] of WSZ) {
    const ist = antwortDurch2([MARKE], ziel);
    if (ist.art !== art || ist.grund !== grund) abbruch(`ws antwort ${name}: ${ist.art} ${ist.grund} gegen ${art} ${grund}`);
    if (art === 'abgerissen' && ist.gesendet !== bytes) abbruch(`ws antwort ${name}: gesendet ${JSON.stringify(ist.gesendet)} gegen ${JSON.stringify(bytes)}`);
    if (art === 'antwort' && ist.koerper !== bytes) abbruch(`ws antwort ${name}: Koerper ${JSON.stringify(ist.koerper)} gegen ${JSON.stringify(bytes)}`);
    if (art === 'antwort' && ist.koerper !== ziel.teile.join('')) abbruch(`ws antwort ${name}: Strom veraendert`);
    if (JSON.stringify(ist).includes(MARKE)) abbruch(`ws antwort ${name}: Marke zum Harness`);
    if (art === 'ersetzt' && (ist.status !== 502 || ist.kopf.some(([k]) => /^(upgrade|connection)$/i.test(k)))) abbruch(`ws antwort ${name}: 101 geht trotz Erweiterung hinaus`);
    const ergebnis = ist.grund === null ? 'weitergereicht' : ist.grund === 'mw.leck_gesperrt' ? 'leck_gesperrt' : 'ziel_fehler';
    wsAntworten.push({ name, beschreibung, ziel_antwort: ziel, erwartet: { an_harness: { art: ist.art, status: ist.status, kopf: ist.kopf, ...(ist.koerper !== undefined ? { koerper: ist.koerper } : { gesendet: ist.gesendet }) }, zeile: { ergebnis, grund: ist.grund, status: ist.status } } });
  }
  // die zwei Fassungen im Altbestand rechnen mit dem Rahmenmodell dasselbe (die Dateien kopf-antworten.json bleiben gleich)
  for (const n of ['websocket_101', 'websocket_leck']) {
    const a = antworten.find((x) => x.name === n); const z = ANT.find((x) => x[0] === n)[2];
    if (jcs(a.erwartet.an_harness) !== jcs({ art: ANT.find((x) => x[0] === n)[3], status: 101, kopf: z.kopf, ...(n === 'websocket_101' ? { koerper: z.teile.join('') } : { gesendet: '\x81\x7e\x00\x40' + 'x'.repeat(37) }) })) abbruch(`ws Altfall ${n} weicht ab`);
  }

  // Richtung Harness -> Dienst (W6): Byte fuer Byte unveraendert, keine Pruefung der Rahmen
  const maske = (nutz, m) => Buffer.from(nutz, 'latin1').map((c, j) => c ^ m.charCodeAt(j % 4)).toString('latin1');
  const MK = '\x37\xfa\x21\x3d';
  const WSH = [
    ['ws_harness_maskiert', 'ein maskierter Textrahmen des Harness: unveraendert zum Dienst', [`\x81\x85${MK}${maske('Hallo', MK)}`]],
    ['ws_harness_beliebig', 'beliebiger Inhalt in Richtung Dienst, auch RSV1, Opcode 0x3, kein Maskenbit und die Marke: unveraendert, keine Pruefung', [`\xc3\x1c${MARKE}`, '\x81\x82\x00\x00\x00\x00ab']],
  ];
  const wsHarness = WSH.map(([name, beschreibung, teile]) => ({ name, beschreibung, von_harness: teile, erwartet: { an_dienst: teile.join(''), zeile: null } }));
  const websocket = { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 13.3 (WebSocket) und 13.2 (Upgrade-Anfrage). Teile und Rahmen sind latin1-Text. anfragen: Bytes an das Ziel wie in kopf-anfragen.json. antworten: wie kopf-antworten.json; bei abgerissen ist gesendet alles, was vor dem Abriss an den Harness ging. von_harness: Bytes unveraendert zum Dienst, ohne Zeile. Schluessel ist die Marke, der Platzhalter ' + PLATZ + ' (beide erfunden).', marke: MARKE, platzhalter: PLATZ, anfragen: wsAnfragen, antworten: wsAntworten, von_harness: wsHarness };

  // Zeile
  const T0 = Date.UTC(2026, 10, 20, 16, 0, 0, 0);
  const B2 = { zeit_ms: T0, betriebsart: 'kopf-einsetzen', route: 'cloud-beispiel', sitzung: 's-1', ziel: 'github-api', methode: 'GET', pfadregel: '/copilot_internal/user', kopf: 'ersetzt', ergebnis: 'weitergereicht', grund: null, status: 200, pfad: null, host: 'api.github.com' };
  const Z2 = [
    ['erlaubt', 'erlaubte Anfrage, Kopf ersetzt', B2, NULL64, null],
    ['ungebunden', 'Telemetrie ohne Anmeldung', { ...B2, ziel: 'copilot-telemetrie', methode: 'POST', pfadregel: '/telemetry', kopf: 'keiner' }, NULL64, null],
    ['abgewiesen', 'Verstoss: Anmeldung an ungebundenes Ziel, unterscheidbar an kopf und grund; die Ablehnung traegt den Pfad', { ...B2, ziel: 'copilot-telemetrie', host: 'telemetry.individual.githubcopilot.com', methode: 'POST', pfadregel: '/telemetry', kopf: 'abgewiesen', ergebnis: 'abgelehnt', grund: 'mw.anmeldung_an_ungebundenes_ziel', status: 403, pfad: '/telemetry' }, NULL64, null],
    ['pfad_verboten', 'GET /user/repos abgelehnt: pfadregel null, nie der Pfad', { ...B2, pfadregel: null, kopf: null, ergebnis: 'abgelehnt', grund: 'mw.pfad_nicht_erlaubt', status: 403 }, NULL64, null],
    ['ziel_fremd', 'CONNECT an ein fremdes Ziel: ziel und host null', { ...B2, ziel: null, host: null, methode: null, pfadregel: null, kopf: null, ergebnis: 'abgelehnt', grund: 'mw.ziel_nicht_erlaubt', status: 403 }, NULL64, null],
    ['sni', 'TLS-Abbruch: status null', { ...B2, methode: null, pfadregel: null, kopf: null, ergebnis: 'abgelehnt', grund: 'mw.sni_falsch', status: null }, NULL64, null],
    ['leck', 'Leck gesperrt', { ...B2, ergebnis: 'leck_gesperrt', grund: 'mw.leck_gesperrt', status: 502 }, sha('x'), null],
    ['get_mit_pfad', 'GET weitergereicht: der begrenzte Pfad steht in der Zeile', { ...B2, pfad: '/copilot_internal/v2' }, NULL64, null],
    ['post_ohne_pfad', 'POST weitergereicht: pfad null', { ...B2, ziel: 'copilot-telemetrie', host: 'telemetry.individual.githubcopilot.com', methode: 'POST', pfadregel: '/telemetry', kopf: 'keiner', pfad: null }, NULL64, null],
    ['abgelehnt_mit_pfad', 'Ablehnung: der begrenzte Pfad steht in der Zeile, pfadregel bleibt null', { ...B2, pfadregel: null, kopf: null, ergebnis: 'abgelehnt', grund: 'mw.pfad_nicht_erlaubt', status: 403, pfad: '/a/b' }, NULL64, null],
    ['pfad_stern', 'jedes Segment ausserhalb der Zeichenform steht als Stern', { ...B2, ergebnis: 'abgelehnt', grund: 'mw.pfad_nicht_erlaubt', pfadregel: null, kopf: null, status: 403, pfad: '/*' }, NULL64, null],
    ['pfad_voll_drei', 'mit --protokoll-pfad-voll stehen auch drei Segmente in der Zeile', { ...B2, pfad: '/a/b/c' }, NULL64, null],
    ['pfad_bei_post_weiter', 'POST weitergereicht mit Pfad: abgewiesen', { ...B2, methode: 'POST', pfadregel: '/copilot_internal/user', pfad: '/a/b' }, NULL64, 'feld_ungueltig:pfad'],
    ['pfad_mit_abfrage', 'ein roher Pfad mit Abfrage im Feld pfad', { ...B2, ergebnis: 'abgelehnt', grund: 'mw.pfad_nicht_erlaubt', pfadregel: null, kopf: null, status: 403, pfad: '/a/b?x=1' }, NULL64, 'feld_ungueltig:pfad'],
    ['host_fehlt', 'Ziel gesetzt, host null', { ...B2, host: null }, NULL64, 'feld_ungueltig:host'],
    ['host_mit_port', 'host mit Port', { ...B2, host: 'api.github.com:443' }, NULL64, 'feld_ungueltig:host'],
    ['host_ohne_ziel', 'host gesetzt, ziel null', { ...B2, ziel: null, pfadregel: null, methode: null, kopf: null, ergebnis: 'abgelehnt', grund: 'mw.ziel_nicht_erlaubt', status: 403 }, NULL64, 'feld_ungueltig:host'],
    ['betriebsart_falsch', 'Betriebsart openai-lokal mit der Form der Betriebsart 2', { ...B2, betriebsart: 'openai-lokal' }, NULL64, 'feld_ungueltig:betriebsart'],
    ['methode_text', 'Methode ausserhalb der Liste', { ...B2, methode: 'PROPFIND' }, NULL64, 'feld_ungueltig:methode'],
    ['pfadregel_pfad', 'ein echter Pfad mit Abfrage statt einer Regel', { ...B2, pfadregel: '/user/repos?per_page=100' }, NULL64, 'feld_ungueltig:pfadregel'],
    ['kopf_weiter_abgewiesen', 'weitergereicht mit kopf abgewiesen', { ...B2, kopf: 'abgewiesen' }, NULL64, 'feld_ungueltig:kopf'],
    ['status_null_sonst', 'status null ohne TLS-Abbruch', { ...B2, status: null }, NULL64, 'feld_ungueltig:status'],
  ];
  const zeilen = [];
  for (const [name, beschreibung, a, vorher, code] of Z2) {
    let ist; try { ist = { zeile: zeileKopf(a, vorher) }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; }
    if ((code === null) !== ('zeile' in ist) || (code !== null && ist.code !== code)) abbruch(`zeile2 ${name}: Modell ${jcs(ist)} gegen ${code}`);
    const e = { name, beschreibung, aufruf: a, vorher };
    if (code === null) {
      if (zeileKopfVonHand(a, vorher) !== ist.zeile) abbruch(`zeile2 ${name}: Handform weicht ab`);
      if (ist.zeile.includes(MARKE) || ist.zeile.includes(PLATZ) || /Bearer|repos|[?#]/.test(ist.zeile)) abbruch(`zeile2 ${name}: Inhalt in der Zeile`);
      const soll = a.pfad === null ? 'null' : JSON.stringify(a.pfad);
      if (!ist.zeile.includes(`"pfad":${soll}`)) abbruch(`zeile2 ${name}: Feld pfad weicht ab`);
      e.erwartet = { zeile: ist.zeile, sha256: sha(ist.zeile) };
    } else e.erwartet = { code };
    zeilen.push(e);
  }

  // Faelle des Feldes pfad (Abschnitt 8): das Modell rechnet jeden nach
  const LANG32 = 'a'.repeat(32);
  const LANG33 = 'a'.repeat(33);
  const VOLL512 = '/' + 'a'.repeat(PFAD_VOLL_MAX - 1);
  const PF = [
    ['ein_segment', 'ein Segment', '/models', false, '/models'],
    ['zwei_segmente', 'zwei Segmente', '/models/session', false, '/models/session'],
    ['drei_segmente', 'drei Segmente: nur zwei bleiben', '/a/b/c', false, '/a/b'],
    ['fuenf_segmente', 'fuenf Segmente: nur zwei bleiben', '/models/session/a/b/c', false, '/models/session'],
    ['abfrage', 'die Abfrage fehlt', '/models?per_page=100', false, '/models'],
    ['fragment', 'das Fragment fehlt', '/models#teil', false, '/models'],
    ['abfrage_und_fragment', 'Abfrage und Fragment fehlen', '/models?a=b#c', false, '/models'],
    ['abfrage_mit_pfad', 'ein Pfad in der Abfrage kommt nicht in das Feld', '/a/b?x=/c/d', false, '/a/b'],
    ['segment_32', 'Segment mit 32 Zeichen bleibt', '/' + LANG32, false, '/' + LANG32],
    ['segment_33', 'Segment mit 33 Zeichen steht als Stern, nie gekuerzt', '/' + LANG33, false, '/*'],
    ['segment_33_zweites', 'das zweite Segment mit 33 Zeichen steht als Stern', '/a/' + LANG33, false, '/a/*'],
    ['prozent', 'Prozentkodierung steht als Stern, nie dekodiert', '/%2e%2e', false, '/*'],
    ['prozent_tilde', 'Prozentkodierung im zweiten Segment', '/a/%7Eb', false, '/a/*'],
    ['zeichen_ausserhalb', 'Doppelpunkt ausserhalb der Zeichenform', '/a:b/c', false, '/*/c'],
    ['leerzeichen', 'Leerzeichen im zweiten Segment', '/a/b c', false, '/a/*'],
    ['leeres_segment', 'leeres Segment', '/a//b', false, '/a/*'],
    ['schraegstrich_ende', 'Schraegstrich am Ende', '/a/', false, '/a/*'],
    ['stern', 'Anfragezeile mit *', '*', false, '*'],
    ['absolute_uri', 'absolute URI', 'https://api.beispiel.invalid/a/b', false, '*'],
    ['wurzel', 'die Wurzel', '/', false, '/'],
    ['wurzel_mit_abfrage', 'die Wurzel mit Abfrage', '/?a=b', false, '/'],
    ['voll_drei', 'voll: drei Segmente bleiben', '/a/b/c', true, '/a/b/c'],
    ['voll_lang', 'voll: ein Segment ueber 32 Zeichen bleibt', '/' + LANG33, true, '/' + LANG33],
    ['voll_zeichen', 'voll: ein Segment ausserhalb der Zeichenform steht als Stern', '/a/b c/d', true, '/a/*/d'],
    ['voll_abfrage', 'voll: die Abfrage fehlt', '/a/b?x=y', true, '/a/b'],
    ['voll_512', 'voll: 512 Zeichen bleiben', VOLL512, true, VOLL512],
    ['voll_513', 'voll: 513 Zeichen machen das ganze Feld zu einem Stern', VOLL512 + 'a', true, '*'],
  ];
  const pfade = [];
  for (const [name, beschreibung, ziel, voll, erwartet] of PF) {
    const ist = pfadFeld(ziel, voll);
    if (ist !== erwartet) abbruch(`pfad ${name}: Modell ${ist} gegen ${erwartet}`);
    if (!pfadFeldOk(ist)) abbruch(`pfad ${name}: ${ist} ist kein Wert des Feldes`);
    if (/[?#]/.test(ist)) abbruch(`pfad ${name}: Abfrage oder Fragment im Feld`);
    pfade.push({ name, beschreibung, ziel, voll, erwartet });
  }

  const zA = zeilen.find((z) => z.name === 'erlaubt').erwartet.zeile;
  const zV = zeilen.find((z) => z.name === 'abgewiesen').erwartet.zeile;
  if (zA === zV || !zV.includes('"kopf":"abgewiesen"')) abbruch('Verstoss nicht unterscheidbar');

  // Aufruf ab dem Bau
  const g2 = ['--betriebsart', 'kopf-einsetzen', '--socket', '/run/user/1000/wb/s-1/modell.sock', '--socket-rechte', '0600', '--sitzung', 's-1', '--route', 'cloud-beispiel',
    '--protokoll', '/srv/beispiel/.local/state/wb/protokoll', '--zielliste', '/run/user/1000/wb/s-1/zielliste.json', '--ca-schluessel', '/run/user/1000/wb/s-1/ca.key', '--ca-zertifikat', '/run/user/1000/wb/s-1/ca.crt'];
  const AUF2 = [
    ['gueltig', 'Betriebsart 2 mit ihren acht Optionen', g2, null],
    ['mit_pfad_voll', 'Betriebsart 2 mit dem Schalter --protokoll-pfad-voll', [...g2, '--protokoll-pfad-voll'], null],
    ['mit_ziel', '--ziel gehoert zu openai-lokal', [...g2, '--ziel', 'https://x.beispiel.invalid/v1'], 'mw.aufruf'],
    ['ohne_ca_zertifikat', 'ohne --ca-zertifikat', g2.slice(0, -2), 'mw.aufruf'],
    ['alte_option_ca', '--ca statt der zwei CA-Optionen', [...g2.slice(0, -4), '--ca', '/run/user/1000/wb/s-1/ca.pem'], 'mw.aufruf'],
    ['ohne_betriebsart', 'ohne --betriebsart gilt openai-lokal, dann fehlen dessen Optionen', g2.slice(2), 'mw.aufruf'],
    ['ca_relativ', '--ca-zertifikat relativ', [...g2.slice(0, -1), 'ca.crt'], 'mw.aufruf'],
  ];
  const aufruf = [];
  for (const [name, beschreibung, argv, code] of AUF2) {
    let ist = null; try { aufrufLesen2(argv); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`aufruf2 ${name}: Modell ${ist} gegen ${code}`);
    aufruf.push({ name, beschreibung, argv, erwartet: code === null ? { bis_schritt: 'aufruf_gut' } : { exit: START_CODES[code][0], stderr: `${code}: ${START_CODES[code][1]}\n`, stdout: '' } });
  }

  // Gegenprobe E5 und Mutanten
  const BASIS = {
    umgebung: { COPILOT_GITHUB_TOKEN: PLATZ, HTTPS_PROXY: 'http://127.0.0.1:8402', COPILOT_AUTO_UPDATE: 'false', COPILOT_MASCOT_BLINK: '0', HOME: '/srv/agent' },
    dateien: { '/srv/agent/.copilot/settings.json': '{"storeTokenPlaintext":false}', '/etc/ssl/certs/ca-certificates.crt': '<System-Bundle des Rechners>\n<Zertifikat der Wegwerf-CA>' },
    dbus_wirt: false, gh_angemeldet: false, speicher_lesbar: false, zielliste: ZL_TEXT,
  };
  const repos = (zl) => { try { innenPruefen({ ...LAGE2, ziele: zieleLesen(zl) }, zieleLesen(zl)[0], innen('GET', '/user/repos', H, COPILOT_KOEPFE)); return 'gelingt'; } catch { return 'abgelehnt'; } };
  const AUFBAUTEN = [
    ['basis', 'Weg (a) wie vorgesehen', BASIS, 'Weiterreicher und Einbindung', { funde: 0, user_repos: 'abgelehnt' }],
    ['gegenprobe_ohne_einschluss', 'Gegenprobe: Marke direkt in der Umgebung der CLI (wie Messung v1)', { ...BASIS, umgebung: { ...BASIS.umgebung, COPILOT_GITHUB_TOKEN: MARKE } }, 'Gegenprobe', { funde: 1 }],
    ['mutant_1_umgebung', 'Mutant 1: das echte Token in der Umgebung des Einschlusses', { ...BASIS, umgebung: { ...BASIS.umgebung, GH_TOKEN: MARKE } }, 'Einbindung', { funde: 1 }],
    ['mutant_2_config', 'Mutant 2: config.json mit der Marke eingebunden', { ...BASIS, dateien: { ...BASIS.dateien, '/srv/agent/.copilot/config.json': `{"copilot_tokens":{"github.com":"${MARKE}"}}` } }, 'Einbindung', { funde: 1 }],
    ['mutant_3_dbus', 'Mutant 3: D-Bus-Socket des Wirts eingebunden (secret-tool lookup service copilot-cli)', { ...BASIS, dbus_wirt: true }, 'Einbindung', { funde: 1 }],
    ['mutant_4_gh', 'Mutant 4: gh mit Anmeldung erreichbar (gh auth token)', { ...BASIS, gh_angemeldet: true }, 'Einbindung', { funde: 1 }],
    ['mutant_5_pfadliste', 'Mutant 5: Pfadliste fuer api.github.com fehlt (eine Regel * /*)', { ...BASIS, zielliste: JSON.stringify({ ...ZL_VORF, ziele: [{ ...ZL_VORF.ziele[0], regeln: [{ methode: '*', pfad: '/*' }] }, ...ZL_VORF.ziele.slice(1)] }, null, 1) + '\n' }, 'Weiterreicher', { funde: 0, user_repos: 'gelingt' }],
  ];
  const gegenprobe = [];
  for (const [name, beschreibung, aufbau, zuordnung, soll] of AUFBAUTEN) {
    const funde = einschlussProbe(aufbau, MARKE);
    const ist = { funde: funde.length, ...(soll.user_repos ? { user_repos: repos(aufbau.zielliste) } : {}) };
    if (jcs(ist) !== jcs(soll)) abbruch(`gegenprobe ${name}: ${jcs(ist)} gegen ${jcs(soll)}`);
    const rot = name.startsWith('mutant') || name.startsWith('gegenprobe');
    gegenprobe.push({ name, beschreibung, zuordnung, aufbau, erwartet: { ...ist, fundorte: funde, test: rot ? 'rot' : 'gruen' } });
  }
  return {
    'faelle/kopf-ziele.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 12 (Betriebsart kopf-einsetzen, noch nicht gebaut). text sind die Bytes der Zielliste.', faelle: ziele },
    'faelle/kopf-tunnel.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 13.1. sni ist der Name im TLS-Gruss des Harness (null: keiner). lage.ziele ist die Vorfuehr-Zielliste aus kopf-ziele.json.', faelle: tunnel },
    'faelle/kopf-anfragen.json': { fassung: 1, hinweis: `Vertrag 8, Abschnitt 13.2. Anfragen im Tunnel (latin1). Schluessel aller gebundenen Ziele ist die Marke ${MARKE}, der Platzhalter ${PLATZ} (beide erfunden). zeile nennt die Felder der Protokollzeile.`, marke: MARKE, platzhalter: PLATZ, faelle: anfragen },
    'faelle/kopf-websocket.json': websocket,
    'faelle/kopf-antworten.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 13.3. ziel_antwort.laenge ist Content-Length des Ziels oder null (chunked oder bis zum Schliessen).', marke: MARKE, faelle: antworten },
    'faelle/kopf-zeile.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitte 8 und 13.4. zeilen: Felder der Zeile und ihre Bytes. pfade: das Feld pfad fuer eine Anfragezeile, voll ist der Admin-Schalter --protokoll-pfad-voll.', zeilen, pfade },
    'faelle/kopf-aufruf.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 13.5. Gilt, wo Betriebsart 2 gebaut ist (im Durchstich nur Linux); sonst gilt in aufruf.json das Feld erwartet der Faelle betriebsart_kopf_einsetzen und betriebsart_vor_form.', codes: { 'mw.ziele_ungueltig': START_CODES['mw.ziele_ungueltig'], 'mw.ca_nicht_zugelassen': START_CODES['mw.ca_nicht_zugelassen'], 'mw.ca_ungueltig': START_CODES['mw.ca_ungueltig'] }, aufruf },
    'faelle/kopf-gegenprobe.json': { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 14. Modell der Gegenprobe E5 als Ausseentest in der VM: funde zaehlt Treffer der Marke im Einschluss; user_repos ist das Ergebnis von GET /user/repos ueber den Weiterreicher. test rot heisst: dieser Aufbau muss den Ausseentest rot machen.', marke: MARKE, faelle: gegenprobe },
  };
}


// --- Umgebung beim Start (Abschnitt 3 und 4, Stufe 0, F1): Faelle ---

// Stufe 0 vor jedem anderen Schritt, fuer beide Betriebsarten: OpenSSL folgt diesen drei Variablen; schon eine gesetzte, auch leere, beendet den Start.
const UMG_CODE = { 'mw.umgebung_unsicher': [78, 'Umgebung unsicher'] };   // bewusst nicht in START_CODES: aufruf.json schreibt diese Tabelle und bleibt gleich
const UMGEBUNG_LISTE = ['SSL_CERT_FILE', 'SSL_CERT_DIR', 'OPENSSL_CONF'];
function umgebungPruefen(betriebsart, env) {
  const liste = M('M-F1c') ? UMGEBUNG_LISTE.filter((n) => n !== 'OPENSSL_CONF') : UMGEBUNG_LISTE;   // M-F1c: OPENSSL_CONF fehlt in der Liste
  if (M('M-F1a') && betriebsart === 'kopf-einsetzen') return;                                       // M-F1a: Betriebsart 2 prueft nicht
  for (const n of liste) {
    if (!Object.prototype.hasOwnProperty.call(env, n)) continue;
    if (M('M-F1b') && env[n] === '') continue;                                                      // M-F1b: eine leere Variable gilt als nicht gesetzt
    throw fehler('mw.umgebung_unsicher');
  }
}
function baueUmgebung() {
  const WERT = '/srv/beispiel/openssl/x';
  const FALL = [
    // [name, beschreibung, umgebung, argv (optional), Code oder null]
    ['ssl_cert_file', 'SSL_CERT_FILE allein gesetzt', { SSL_CERT_FILE: WERT }, null, 'mw.umgebung_unsicher'],
    ['ssl_cert_dir', 'SSL_CERT_DIR allein gesetzt', { SSL_CERT_DIR: WERT }, null, 'mw.umgebung_unsicher'],
    ['openssl_conf', 'OPENSSL_CONF allein gesetzt', { OPENSSL_CONF: WERT }, null, 'mw.umgebung_unsicher'],
    ['leer_ssl_cert_file', 'SSL_CERT_FILE leer gesetzt gilt als gesetzt', { SSL_CERT_FILE: '' }, null, 'mw.umgebung_unsicher'],
    ['leer_ssl_cert_dir', 'SSL_CERT_DIR leer gesetzt gilt als gesetzt', { SSL_CERT_DIR: '' }, null, 'mw.umgebung_unsicher'],
    ['leer_openssl_conf', 'OPENSSL_CONF leer gesetzt gilt als gesetzt', { OPENSSL_CONF: '' }, null, 'mw.umgebung_unsicher'],
    ['zwei_zugleich', 'zwei der drei zugleich gesetzt: derselbe Fehler, nur eine Zeile', { SSL_CERT_FILE: WERT, OPENSSL_CONF: WERT }, null, 'mw.umgebung_unsicher'],
    ['vor_dem_aufruf', 'die Umgebung wird vor dem Aufruf geprueft: ein ungueltiger Aufruf aendert den Fehler nicht', { SSL_CERT_DIR: WERT }, ['--unbekannt'], 'mw.umgebung_unsicher'],
    ['keine', 'keine der drei gesetzt: der Start geht zu Schritt 1 weiter', {}, null, null],
    ['andere_variablen', 'andere Variablen gesetzt: der Start geht weiter', { PATH: '/usr/bin', LANG: 'C' }, null, null],
    ['aehnlich_files', 'SSL_CERT_FILES (mit S) ist eine andere Variable: der Start geht weiter', { SSL_CERT_FILES: WERT }, null, null],
    ['aehnlich_confx', 'OPENSSL_CONFX ist eine andere Variable: der Start geht weiter', { OPENSSL_CONFX: WERT }, null, null],
  ];
  const faelle = [];
  for (const betriebsart of ['openai-lokal', 'kopf-einsetzen']) {
    for (const [name, beschreibung, umgebung, argv, code] of FALL) {
      let ist = null; try { umgebungPruefen(betriebsart, umgebung); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
      if (ist !== code) abbruch(`umgebung ${betriebsart} ${name}: Modell ${ist} gegen ${code}`);
      const e = code === null ? { bis_schritt: 'umgebung_gut' } : { exit: UMG_CODE[code][0], stderr: `${code}: ${UMG_CODE[code][1]}\n`, stdout: '' };
      faelle.push({ name: `${betriebsart}_${name}`, beschreibung, betriebsart, umgebung, ...(argv ? { argv: ['--betriebsart', betriebsart, ...argv] } : {}), erwartet: e });
    }
  }
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 4 (Stufe 0 vor Schritt 1, F1). umgebung ist die Umgebung des Weiterreichers beim Start (Name: Wert); argv gilt nur, wo es steht. bis_schritt umgebung_gut heisst: Stufe 0 besteht, der Start geht zu Schritt 1 weiter. Der Fehler nennt weder Namen noch Wert der Variablen, nur die feste Zeile.', faelle };
}

// --- Marker-Zaehlung (Abschnitt 9): Faelle ---

function baueMarker() {
  const gut = ['--socket', '/run/user/1000/wb/s-2/modell.sock', '--socket-rechte', '0600', '--ziel', ZIEL_FIRMA, '--modell', MOD,
    '--schluessel-datei', '/srv/beispiel/.config/wb/modellweg.schluessel', '--sitzung', 's-2', '--route', 'lokal-beispiel', '--protokoll', '/srv/beispiel/.local/state/wb/protokoll'];
  const Z = '/run/user/1000/wb/s-2/marker.json';
  const PL = ['--probelauf', '--marker', 'MK-KUNDE,MK-INTERN', '--marker-zaehlung', Z];
  const ARGV = [
    ['ohne_probelauf', 'keine der drei Optionen: keine Zaehlung, keine Datei, keine Startzeile', gut, null],
    ['alle_drei', '--probelauf, --marker und --marker-zaehlung', [...gut, ...PL], null],
    ['schalter_vorn', '--probelauf als erstes Argument', [...PL, ...gut], null],
    ['nur_probelauf', 'nur --probelauf', [...gut, '--probelauf'], 'mw.probelauf_aufruf'],
    ['marker_ohne_probelauf', '--marker und --marker-zaehlung ohne --probelauf', [...gut, '--marker', 'MK-KUNDE', '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['nur_marker', 'nur --marker', [...gut, '--marker', 'MK-KUNDE'], 'mw.probelauf_aufruf'],
    ['probelauf_ohne_datei', '--probelauf und --marker ohne --marker-zaehlung', [...gut, '--probelauf', '--marker', 'MK-KUNDE'], 'mw.probelauf_aufruf'],
    ['probelauf_doppelt', '--probelauf zweimal', [...gut, ...PL, '--probelauf'], 'mw.aufruf'],
    ['probelauf_mit_wert', '--probelauf mit Wert ja: ja ist keine Option', [...gut, '--probelauf', 'ja', '--marker', 'MK-A', '--marker-zaehlung', Z], 'mw.aufruf'],
    ['marker_klein', 'Marker mit Kleinbuchstaben', [...gut, '--probelauf', '--marker', 'mk-kunde', '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['marker_lang', 'Marker mit 33 Zeichen', [...gut, '--probelauf', '--marker', 'A'.repeat(33), '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['marker_leer', 'leerer Marker zwischen zwei Kommas', [...gut, '--probelauf', '--marker', 'MK-A,,MK-B', '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['marker_17', '17 Marker', [...gut, '--probelauf', '--marker', Array.from({ length: 17 }, (_, i) => `MK-${i}`).join(','), '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['marker_doppelt', 'derselbe Marker zweimal', [...gut, '--probelauf', '--marker', 'MK-A,MK-A', '--marker-zaehlung', Z], 'mw.probelauf_aufruf'],
    ['datei_relativ', 'Zaehldatei relativ', [...gut, '--probelauf', '--marker', 'MK-A', '--marker-zaehlung', 'marker.json'], 'mw.probelauf_aufruf'],
    ['vor_vollstaendigkeit', 'nur --probelauf und fehlendes --ziel: der Probelauf-Fehler zuerst', [...gut.filter((v, i, a) => v !== '--ziel' && a[i - 1] !== '--ziel'), '--probelauf'], 'mw.probelauf_aufruf'],
  ];
  const aufruf = [];
  for (const [name, beschreibung, argv, code] of ARGV) {
    let ist = null; try { aufrufLesen(argv); } catch (x) { if (!(x instanceof Fehler)) throw x; ist = x.message; }
    if (ist !== code) abbruch(`marker-aufruf ${name}: Modell ${ist} gegen ${code}`);
    aufruf.push({ name, beschreibung, argv, erwartet: code === null ? { bis_schritt: 'aufruf_gut' } : { exit: START_CODES[code][0], stderr: `${code}: ${START_CODES[code][1]}\n`, stdout: '' } });
  }
  // Folge von Anfragen in einer Sitzung: Zaehlung nur im Koerper an das Ziel, abgelehnte zaehlen nicht, Datei nicht schreibbar = Ablehnung
  const MARKER = ['MK-KUNDE', 'MK-INTERN'];
  const k1 = JSON.stringify({ model: MOD, messages: [{ role: 'user', content: 'Frage zu MK-KUNDE und MK-KUNDE' }] });
  const k2 = JSON.stringify({ model: MOD, messages: [{ role: 'user', content: 'MK-INTERN' }] });
  const kAb = JSON.stringify({ model: 'lokal-beispiel/modell-b', messages: [{ role: 'user', content: 'MK-KUNDE' }] });
  const FOLGE = [
    ['erste', 'zwei Treffer MK-KUNDE im Koerper', post(k1, [['X-Notiz', 'MK-INTERN']]), true],
    ['abgelehnt', 'anderes Modell: abgelehnt, zaehlt nicht', post(kAb), true],
    ['zweite', 'ein Treffer MK-INTERN', post(k2), true],
    ['nicht_schreibbar', 'Zaehldatei nicht schreibbar: die Anfrage geht nicht hinaus', post(k1), false],
    ['danach', 'Datei wieder schreibbar: weiter ab dem letzten guten Stand', post(k2), true],
  ];
  let stand = { anfragen: 0, marker: Object.fromEntries(MARKER.map((m) => [m, 0])), sitzung: 's-2' };
  const anfang = jcs(stand);
  const folge = [];
  for (const [name, beschreibung, roh, schreibbar] of FOLGE) {
    let ergebnis;
    try {
      const r = pruefeAnfrage(LAGE, roh);
      const koerper = r.an_ziel.slice(r.an_ziel.indexOf('\r\n\r\n') + 4);
      if (!schreibbar) ergebnis = { ziel_beruehrt: false, status: ABLEHNUNG['mw.zaehlung_fehler'][0], koerper: fehlerKoerper('mw.zaehlung_fehler'), zeile: { ergebnis: 'abgelehnt', grund: 'mw.zaehlung_fehler' }, datei: jcs(stand) };
      else {
        const z = zaehlen(MARKER, koerper);
        stand = { ...stand, anfragen: stand.anfragen + 1, marker: Object.fromEntries(MARKER.map((m) => [m, stand.marker[m] + z[m]])) };
        ergebnis = { ziel_beruehrt: true, an_ziel: r.an_ziel, datei: jcs(stand) };
      }
    } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      ergebnis = { ziel_beruehrt: false, code: x.message, datei: jcs(stand) };
    }
    folge.push({ name, beschreibung, anfrage: roh, datei_schreibbar: schreibbar, erwartet: ergebnis });
  }
  const ende = JSON.parse(folge[folge.length - 1].erwartet.datei);
  if (ende.anfragen !== 3 || ende.marker['MK-KUNDE'] !== 2 || ende.marker['MK-INTERN'] !== 2) abbruch(`marker-folge: Endstand ${jcs(ende)}`);
  if (folge[1].erwartet.code !== 'mw.modell_nicht_erlaubt' || folge[1].erwartet.datei !== folge[0].erwartet.datei) abbruch('marker-folge: abgelehnte Anfrage hat gezaehlt');
  if (folge[3].erwartet.datei !== folge[2].erwartet.datei) abbruch('marker-folge: Zaehlung trotz Fehler');
  for (const f of folge) if (f.erwartet.datei.includes(SCHL)) abbruch('Schluessel in der Zaehldatei');
  // Startzeile probelauf.zaehlung und Startfolge
  const T0 = Date.UTC(2026, 10, 20, 16, 30, 0, 0);
  const SZ = [
    ['zwei_marker', 'Probelauf mit zwei Markern', { zeit_ms: T0, programm: 'modellweg', sitzung: 's-2', anzahl: 2 }, NULL64, null],
    ['sechzehn', 'Probelauf mit 16 Markern', { zeit_ms: T0, programm: 'modellweg', sitzung: 's-2', anzahl: 16 }, sha('x'), null],
    ['programm_gross', 'Programm mit Grossbuchstaben', { zeit_ms: T0, programm: 'Modellweg', sitzung: 's-2', anzahl: 2 }, NULL64, 'feld_ungueltig:programm'],
    ['sitzung_leer', 'Sitzung leer', { zeit_ms: T0, programm: 'modellweg', sitzung: '', anzahl: 2 }, NULL64, 'feld_ungueltig:sitzung'],
    ['anzahl_0', 'anzahl 0', { zeit_ms: T0, programm: 'modellweg', sitzung: 's-2', anzahl: 0 }, NULL64, 'feld_ungueltig:anzahl'],
    ['anzahl_17', 'anzahl 17', { zeit_ms: T0, programm: 'modellweg', sitzung: 's-2', anzahl: 17 }, NULL64, 'feld_ungueltig:anzahl'],
    ['vorher_falsch', 'vorher mit 63 Zeichen', { zeit_ms: T0, programm: 'modellweg', sitzung: 's-2', anzahl: 2 }, NULL64.slice(1), 'feld_ungueltig:vorher'],
    ['zeit_negativ', 'zeit vor 1970', { zeit_ms: -1, programm: 'modellweg', sitzung: 's-2', anzahl: 2 }, NULL64, 'zeit_ungueltig'],
  ];
  const startzeile = [];
  for (const [name, beschreibung, a, vorher, code] of SZ) {
    let ist; try { ist = { zeile: zeileProbelauf(a, vorher) }; } catch (x) { if (!(x instanceof Fehler)) throw x; ist = { code: x.message }; }
    if ((code === null) !== ('zeile' in ist) || (code !== null && ist.code !== code)) abbruch(`startzeile ${name}: ${jcs(ist)} gegen ${code}`);
    const e = { name, beschreibung, aufruf: a, vorher };
    if (code === null) {
      const hand = `{"aktionsname":"probelauf.zaehlung","anzahl":${a.anzahl},"ergebnis":"an","fassung":2,"programm":"${a.programm}","sitzung":"${a.sitzung}","vorher":"${vorher}","zeit":"${new Date(a.zeit_ms).toISOString()}"}`;
      if (hand !== ist.zeile) abbruch(`startzeile ${name}: Handform weicht ab`);
      if (/MK-/.test(ist.zeile)) abbruch('Marker in der Startzeile');
      e.erwartet = { zeile: ist.zeile, sha256: sha(ist.zeile) };
    } else e.erwartet = { code };
    startzeile.push(e);
  }
  const zeile1 = startzeile[0].erwartet.zeile;
  const startfolge = [
    { name: 'start_gut', beschreibung: 'Probelauf mit zwei Markern, Protokoll und Zaehldatei schreibbar', argv: [...gut, ...PL], protokoll_schreibbar: true, zaehldatei_schreibbar: true, zeit_ms: T0,
      erwartet: { exit: null, stdout: '{"bereit":true}\n', stderr: 'mw.probelauf: Marker werden gezählt\n', protokoll_neu: zeile1 + '\n', zaehldatei: anfang } },
    { name: 'protokoll_nicht_schreibbar', beschreibung: 'Startzeile nicht schreibbar: kein Start', argv: [...gut, ...PL], protokoll_schreibbar: false, zaehldatei_schreibbar: true, zeit_ms: T0,
      erwartet: { exit: 74, stdout: '', stderr: 'mw.protokoll_fehlt: Protokoll nicht schreibbar\n', protokoll_neu: '', socket: 'nicht angelegt' } },
    { name: 'zaehldatei_nicht_schreibbar', beschreibung: 'Zaehldatei nicht schreibbar: kein Start, keine Startzeile', argv: [...gut, ...PL], protokoll_schreibbar: true, zaehldatei_schreibbar: false, zeit_ms: T0,
      erwartet: { exit: 74, stdout: '', stderr: 'mw.zaehlung_fehler: Zaehldatei nicht schreibbar\n', protokoll_neu: '', socket: 'nicht angelegt' } },
    { name: 'ohne_probelauf', beschreibung: 'ohne Probelauf: keine Startzeile, keine Zaehldatei, keine Zeile auf stderr', argv: gut, protokoll_schreibbar: true, zaehldatei_schreibbar: true, zeit_ms: T0,
      erwartet: { exit: null, stdout: '{"bereit":true}\n', stderr: '', protokoll_neu: '', zaehldatei: null } },
  ];
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 9 (fuer jede Betriebsart, nur mit --probelauf). startfolge: Ausgabe des Starts (protokoll_neu = an die Kette angehaengte Bytes, leere Kette am Anfang). folge: Anfragen einer Sitzung in der Lage aus anfragen.json mit --marker MK-KUNDE,MK-INTERN; datei ist der Inhalt der Zaehldatei nach der Anfrage (RFC 8785). anfang ist der Inhalt vor der ersten Anfrage.', marker: MARKER, anfang, aufruf, folge, startzeile, startfolge };
}

// --- Probelauf: Marker-Zaehlung in WebSocket-Rahmen vom Harness zum Dienst (Abschnitt 9, Z1 bis Z3, Nachtrag 17): Faelle ---

// Nur im Probelauf liest der Weiterreicher nach der 101 auch die Rahmen des Harness (RFC 6455 Abschnitt 5.2), entmaskiert die Nutzdaten der Datenrahmen nur fuer die Zaehlung
// und zaehlt jeden Marker ohne Ueberlappung ueber die Folge aller Datenrahmen-Nutzdaten. Ein Rahmen geht erst hinaus, wenn er vollstaendig gelesen, geprueft, gezaehlt und
// die Zaehldatei geschrieben ist. Die Bytes an den Dienst bleiben unveraendert (maskiert, wie vom Harness). Mutanten M-Z0 bis M-Z4 (Abschnitt 14).
const HARNESS_OPCODES = [0x0, 0x1, 0x2, 0x8, 0x9, 0xa];
function wsZaehlenProbelauf(o, teile) {
  const s = Buffer.from(teile.join(''), 'latin1');
  if (!o.probelauf) return { an_ziel: s.toString('latin1'), rahmen_hinaus: null, abriss: null, datei_anfang: null, datei_folge: [], datei_ende: null };
  const nullen = () => Object.fromEntries(o.marker.map((m) => [m, 0]));
  const stand = (marker) => jcs({ anfragen: 1, marker, sitzung: o.sitzung });
  const zaehle = (b) => Object.fromEntries(o.marker.map((m) => { const k = Buffer.from(m, 'latin1'); let n = 0; let p = 0; for (;;) { const j = b.indexOf(k, p); if (j < 0) break; n++; p = j + k.length; } return [m, n]; }));
  const anfang = stand(nullen());
  let i = 0; let hinaus = 0; let offen = false; let nr = 0; let strom = Buffer.alloc(0); let summe = nullen(); let datei = anfang; const folge = [];
  const fertig = (abriss, bis = i) => ({ an_ziel: s.subarray(0, bis).toString('latin1'), rahmen_hinaus: hinaus, abriss, datei_anfang: anfang, datei_folge: folge, datei_ende: datei });
  for (;;) {
    if (s.length - i < 2) break;
    const b0 = s[i]; const b1 = s[i + 1];
    const opcode = b0 & 0x0f; const fin = !!(b0 & 0x80); const maske = !!(b1 & 0x80);
    let laenge = b1 & 0x7f; let kopf = 2;
    if (laenge === 126) { if (s.length - i < 4) break; laenge = s.readUInt16BE(i + 2); kopf = 4; }
    else if (laenge === 127) { if (s.length - i < 10) break; const l = s.readBigUInt64BE(i + 2); laenge = l > BigInt(KOERPER_MAX) ? KOERPER_MAX + 1 : Number(l); kopf = 10; }
    // Z3: ein nicht lesbarer Rahmen geht nicht hinaus; geprueft, sobald der Kopf bis zur Laengenangabe da ist: RSV, Maskenbit, Opcode, Laenge, Regeln aus 13.3
    const unlesbar = (b0 & 0x70) !== 0 || !maske || !HARNESS_OPCODES.includes(opcode) || laenge > KOERPER_MAX
      || (opcode >= 0x8 && (!fin || laenge > 125)) || (opcode === 0x0 && !offen) || ((opcode === 0x1 || opcode === 0x2) && offen);
    if (unlesbar) { if (M('M-Z4')) return fertig(null, s.length); return fertig('mw.anfrage_unlesbar'); }   // M-Z4: ein nicht lesbarer Rahmen geht im Probelauf durch
    const ende = i + kopf + 4 + laenge;
    if (s.length < ende) break;                                                                           // unvollstaendiger Rahmen: nicht gezaehlt, nicht gesendet
    const schluessel = s.subarray(i + kopf, i + kopf + 4); const maskiert = s.subarray(i + kopf + 4, ende);
    const nutz = Buffer.from(maskiert.map((c, j) => c ^ schluessel[j % 4]));
    const datenRahmen = opcode < 0x8;
    if (!datenRahmen && !M('M-Z3')) { i = ende; hinaus++; continue; }                                    // M-Z3: Steuerrahmen zaehlen mit
    nr++;
    if (o.schreibbar_ab && nr >= o.schreibbar_ab) return fertig('mw.zaehlung_fehler');
    if (!M('M-Z0')) {                                                                                     // M-Z0: die Richtung Harness -> Ziel wird nicht gezaehlt
      const quelle = M('M-Z1') ? maskiert : nutz;                                                         // M-Z1: Zaehlung auf den maskierten Bytes
      if (M('M-Z2')) { const z = zaehle(quelle); summe = Object.fromEntries(o.marker.map((m) => [m, summe[m] + z[m]])); datei = stand(summe); }   // M-Z2: je Rahmen statt ueber Rahmengrenzen
      else { strom = Buffer.concat([strom, quelle]); datei = stand(zaehle(strom)); }
    }
    folge.push(datei);
    if (datenRahmen) offen = !fin;
    i = ende; hinaus++;
  }
  return fertig(null);
}

function baueProbelaufWs() {
  const KEYS = ['\x37\xfa\x21\x3d', '\x6b\x12\xc4\x09', '\xa5\x5e\x80\x71', '\x1d\xd3\x4f\xb6'];
  let kz = 0;
  // Rahmen des Harness nach RFC 6455 Abschnitt 5.2, latin1-Text; masked ausser mit ohneMaske; angabe ueberschreibt die Laengenangabe; rsv ist der Wert der drei RSV-Bits (4 = RSV1)
  const cf = (opcode, nutz, o = {}) => {
    const n = Buffer.byteLength(nutz, 'latin1'); const L = o.angabe ?? n; const form = o.form ?? (L < 126 ? 7 : L < 65536 ? 126 : 127);
    const mb = o.ohneMaske ? 0 : 0x80;
    const b0 = (o.fin === false ? 0 : 0x80) | ((o.rsv ?? 0) << 4) | opcode;
    const lb = form === 7 ? [mb | L] : form === 126 ? [mb | 126, L >> 8, L & 255] : [mb | 127, 0, 0, 0, 0, (L >>> 24) & 255, (L >>> 16) & 255, (L >>> 8) & 255, L & 255];
    const key = o.ohneMaske ? '' : KEYS[kz++ % KEYS.length];
    const koerper = o.ohneMaske ? nutz : Buffer.from(nutz, 'latin1').map((c, j) => c ^ key.charCodeAt(j % 4)).toString('latin1');
    if (!o.ohneMaske && /MK-KUNDE|MK-INTERN|AB-AB/.test(koerper)) abbruch(`probelauf-ws: Marker im maskierten Rahmen lesbar: ${JSON.stringify(nutz)}`);
    return String.fromCharCode(b0, ...lb) + key + koerper;
  };
  const MARKER = ['MK-KUNDE', 'MK-INTERN'];
  const VOR = cf(1, 'MK-KUNDE');        // ein gezaehlter Rahmen davor
  const DANACH = cf(1, 'MK-INTERN');    // ein gueltiger Rahmen danach: er darf nach einem Abriss nicht mehr hinausgehen
  const ZEILE_ENDE = { ergebnis: 'weitergereicht', grund: null, status: 101 };
  const ZEILE_ABRISS = (g) => ({ ergebnis: 'abgelehnt', grund: g, status: 101 });
  // [name, beschreibung, teile, optionen, hand]; hand: hinaus (Rahmen an den Dienst), abriss, folge (Zaehlstand nach jedem Datenrahmen, je Marker in der Reihenfolge der Liste), ende; alles von Hand gesetzt
  const FALL = [
    ['ws_marker_textrahmen', 'ein Marker in einem Textrahmen des Harness: gezaehlt, der Rahmen geht unveraendert (maskiert) hinaus (Z1)', [cf(1, 'Frage zu MK-KUNDE')], {}, { hinaus: 1, abriss: null, folge: [[1, 0]], ende: [1, 0] }],
    ['ws_zwei_marker_in_einem_rahmen', 'drei Treffer in einem Rahmen (zwei verschiedene Marker, einer doppelt)', [cf(1, 'MK-KUNDE und MK-INTERN und MK-KUNDE')], {}, { hinaus: 1, abriss: null, folge: [[2, 1]], ende: [2, 1] }],
    ['ws_marker_geteilt_zwei_rahmen', 'ein Marker ueber zwei Rahmen einer Nachricht verteilt (erster Rahmen FIN=0, Fortsetzung FIN=1): zaehlt einmal, erst mit dem zweiten Rahmen (Z1)', [cf(1, 'xx MK-KU', { fin: false }), cf(0, 'NDE yy')], {}, { hinaus: 2, abriss: null, folge: [[0, 0], [1, 0]], ende: [1, 0] }],
    ['ws_marker_ein_und_geteilt', 'PFLICHT: ein Marker in einem Rahmen und ein anderer ueber zwei Rahmen verteilt, beide gezaehlt', [cf(1, 'MK-INTERN'), cf(1, 'a MK-KU', { fin: false }), cf(0, 'NDE')], {}, { hinaus: 3, abriss: null, folge: [[0, 1], [0, 1], [1, 1]], ende: [1, 1] }],
    ['ws_marker_ueber_nachrichten', 'ein Marker ueber die Grenze zweier Nachrichten (beide Rahmen FIN=1): zaehlt einmal (Z1: ueber Rahmen- und Nachrichtengrenzen)', [cf(1, 'MK-KU'), cf(1, 'NDE')], {}, { hinaus: 2, abriss: null, folge: [[0, 0], [1, 0]], ende: [1, 0] }],
    ['ws_marker_binaerrahmen', 'ein Marker in einem Binaerrahmen (Opcode 0x2)', [cf(2, '\x00\x01MK-INTERN\xff')], {}, { hinaus: 1, abriss: null, folge: [[0, 1]], ende: [0, 1] }],
    ['ws_marker_laenge_126', 'ein Marker am Ende eines Rahmens in der 16-Bit-Laengenform (130 Zeichen davor)', [cf(1, 'x'.repeat(130) + 'MK-KUNDE')], {}, { hinaus: 1, abriss: null, folge: [[1, 0]], ende: [1, 0] }],
    ['ws_marker_im_ping', 'ein Marker in einem Ping: nicht gezaehlt, der Ping geht unveraendert hinaus, keine Datei geschrieben (Z1: Steuerrahmen zaehlen nicht)', [cf(1, 'ab'), cf(9, 'MK-KUNDE')], {}, { hinaus: 2, abriss: null, folge: [[0, 0]], ende: [0, 0] }],
    ['ws_marker_im_close', 'ein Marker im Grund eines Close-Rahmens: nicht gezaehlt', [cf(8, '\x03\xe8MK-KUNDE')], {}, { hinaus: 1, abriss: null, folge: [], ende: [0, 0] }],
    ['ws_ping_zwischen_teilen', 'ein Marker ueber zwei Rahmen, dazwischen ein Ping, der denselben Marker traegt: zaehlt einmal; der Ping unterbricht nichts und traegt nichts bei (Z1)', [cf(1, 'MK-KU', { fin: false }), cf(9, 'MK-KUNDE'), cf(0, 'NDE')], {}, { hinaus: 3, abriss: null, folge: [[0, 0], [1, 0]], ende: [1, 0] }],
    ['ws_marker_ohne_ueberlappung', 'ein Marker, der sich mit sich selbst ueberlappen kann (AB-AB): ueber zwei Rahmen verteilt zaehlt von links ohne Ueberlappung, 2 und nicht 3', [cf(1, 'AB-AB-AB', { fin: false }), cf(0, '-AB')], { marker: ['AB-AB'] }, { hinaus: 2, abriss: null, folge: [[1], [2]], ende: [2] }],
    ['ws_teile_byteweise', 'dieselben zwei Rahmen wie in ws_marker_ein_und_geteilt, Byte fuer Byte als einzelne Teile geliefert: dasselbe Ergebnis, die Teilgrenzen haben keine Wirkung', [...[cf(1, 'MK-INTERN'), cf(1, 'a MK-KU', { fin: false }), cf(0, 'NDE')].join('')], {}, { hinaus: 3, abriss: null, folge: [[0, 1], [0, 1], [1, 1]], ende: [1, 1] }],
    ['ws_ohne_probelauf', 'ohne Probelauf: keine Zaehlung, keine Datei; auch ein Rahmen ohne Maskenbit, einer mit RSV1 und ein Opcode 0x3 gehen unveraendert hinaus (W6 bleibt, keine Pruefung dieser Richtung)',
      [cf(1, 'MK-KUNDE'), cf(1, 'MK-INTERN', { ohneMaske: true }), cf(1, 'ab', { rsv: 4 }), cf(3, 'ab')], { probelauf: false }, { hinaus: null, abriss: null, folge: [], ende: null }],
  ];
  const KAPUTT = [
    ['ws_maske_fehlt', 'ein Rahmen des Harness ohne Maskenbit, mit einem Marker: nicht gezaehlt, geht nicht hinaus, Abriss in beide Richtungen (Z3); der Rahmen danach geht auch nicht mehr hinaus', cf(1, 'MK-INTERN', { ohneMaske: true })],
    ['ws_rsv1', 'RSV1 gesetzt: Abriss (Z3)', cf(1, 'ab', { rsv: 4 })],
    ['ws_rsv3', 'RSV3 gesetzt: Abriss (Z3)', cf(1, 'ab', { rsv: 1 })],
    ['ws_opcode_3', 'unbekannter Opcode 0x3: Abriss (Z3)', cf(3, 'ab')],
    ['ws_laenge_zu_gross', 'Laengenangabe 33554433 in der 64-Bit-Form: Abriss schon am Kopf, ohne die Nutzdaten abzuwarten (Z3)', cf(1, 'a', { form: 127, angabe: 33554433 })],
    ['ws_ping_fin0', 'ein Ping mit FIN=0: Abriss (Z3, Regelverstoss wie in 13.3)', cf(9, 'p', { fin: false })],
    ['ws_ping_laenge_126', 'ein Ping mit der Laenge 126: Abriss (Z3)', cf(9, 'p'.repeat(126))],
    ['ws_fortsetzung_ohne_anfang', 'eine Fortsetzung ohne offene geteilte Nachricht: Abriss (Z3)', cf(0, 'ab')],
  ];
  const faelle = [];
  const baue = (name, beschreibung, teile, opt, hand) => {
    const o = { probelauf: opt.probelauf ?? true, marker: opt.marker ?? MARKER, sitzung: 's-1', schreibbar_ab: opt.schreibbar_ab ?? null };
    const ist = wsZaehlenProbelauf(o, teile);
    const wo = `probelauf-ws ${name}`;
    const alle = teile.join('');
    if (ist.rahmen_hinaus !== hand.hinaus) abbruch(`${wo}: Rahmen hinaus ${ist.rahmen_hinaus} gegen ${hand.hinaus}`);
    if (ist.abriss !== hand.abriss) abbruch(`${wo}: Abriss ${ist.abriss} gegen ${hand.abriss}`);
    if (!alle.startsWith(ist.an_ziel)) abbruch(`${wo}: die Bytes an den Dienst sind kein Anfang der Bytes des Harness`);
    if (hand.abriss === null && !hand.abgeschnitten && ist.an_ziel !== alle) abbruch(`${wo}: Bytes veraendert oder verloren`);
    if (hand.abriss !== null && ist.an_ziel.length >= alle.length) abbruch(`${wo}: nach dem Abriss ging alles hinaus`);
    if (o.probelauf) {
      const zahlen = (d) => o.marker.map((m) => JSON.parse(d).marker[m]);
      if (jcs(ist.datei_folge.map(zahlen)) !== jcs(hand.folge)) abbruch(`${wo}: Zaehlstand je Datenrahmen ${jcs(ist.datei_folge.map(zahlen))} gegen ${jcs(hand.folge)}`);
      if (jcs(zahlen(ist.datei_ende)) !== jcs(hand.ende)) abbruch(`${wo}: Endstand ${jcs(zahlen(ist.datei_ende))} gegen ${jcs(hand.ende)}`);
      for (const d of [ist.datei_anfang, ...ist.datei_folge]) if (JSON.parse(d).anfragen !== 1) abbruch(`${wo}: anfragen zaehlt etwas anderes als HTTP-Anfragen`);
    } else if (ist.datei_ende !== null || ist.datei_folge.length) abbruch(`${wo}: Zaehlung ohne Probelauf`);
    else if (ist.an_ziel !== alle) abbruch(`${wo}: ohne Probelauf geaendert`);
    const zeile = ist.abriss ? ZEILE_ABRISS(ist.abriss) : ZEILE_ENDE;
    faelle.push({ name, beschreibung, probelauf: o.probelauf, ...(o.probelauf ? { marker: o.marker } : {}), ...(o.schreibbar_ab ? { zaehldatei_nicht_schreibbar_ab_datenrahmen: o.schreibbar_ab } : {}), von_harness: teile,
      erwartet: { an_ziel: ist.an_ziel, rahmen_hinaus: ist.rahmen_hinaus, abriss: ist.abriss, datei_anfang: ist.datei_anfang, datei_folge: ist.datei_folge, datei_ende: ist.datei_ende, zeile } });
  };
  for (const [name, beschr, teile, opt, hand] of FALL) baue(name, beschr, teile, opt, hand);
  for (const [name, beschr, bad] of KAPUTT) baue(name, beschr, [VOR, bad, DANACH], {}, { hinaus: 1, abriss: 'mw.anfrage_unlesbar', folge: [[1, 0]], ende: [1, 0] });
  baue('ws_text_in_geteilter', 'ein neuer Textrahmen, waehrend eine geteilte Nachricht offen ist: Abriss (Z3); der erste Teil ging hinaus und ist gezaehlt', [VOR, cf(1, 'ab', { fin: false }), cf(1, 'cd'), DANACH], {}, { hinaus: 2, abriss: 'mw.anfrage_unlesbar', folge: [[1, 0], [1, 0]], ende: [1, 0] });
  baue('ws_zaehldatei_nicht_schreibbar', 'die Zaehldatei ist ab dem zweiten Datenrahmen nicht schreibbar: der zweite Rahmen geht nicht hinaus, beide Richtungen schliessen, Zeile mw.zaehlung_fehler; der Stand bleibt beim letzten geschriebenen Wert (Z3)',
    [VOR, cf(1, 'MK-INTERN'), cf(1, 'MK-KUNDE')], { schreibbar_ab: 2 }, { hinaus: 1, abriss: 'mw.zaehlung_fehler', folge: [[1, 0]], ende: [1, 0] });
  baue('ws_zaehldatei_nicht_schreibbar_erster', 'die Zaehldatei ist schon beim ersten Datenrahmen nicht schreibbar: nichts geht hinaus, der Stand bleibt der Anfangsstand (Z3)',
    [VOR, DANACH], { schreibbar_ab: 1 }, { hinaus: 0, abriss: 'mw.zaehlung_fehler', folge: [], ende: [0, 0] });
  baue('ws_steuerrahmen_ohne_schreiben', 'nur Steuerrahmen und eine nicht schreibbare Zaehldatei: kein Datenrahmen, also kein Schreiben und kein Abriss; die Rahmen gehen hinaus (Z2: Schreiben nach jedem Datenrahmen)',
    [cf(9, 'ping'), cf(10, 'pong')], { schreibbar_ab: 1 }, { hinaus: 2, abriss: null, folge: [], ende: [0, 0] });
  baue('ws_abgeschnitten', 'der zweite Rahmen ist beim Ende der Verbindung abgeschnitten: er ist nie vollstaendig, wird nicht gezaehlt und geht nicht hinaus; kein Abriss, Zeile wie beim normalen Ende',
    [VOR, cf(1, 'MK-INTERN').slice(0, -3)], {}, { hinaus: 1, abriss: null, abgeschnitten: true, folge: [[1, 0]], ende: [1, 0] });
  baue('ws_abgeschnitten_kopf', 'die Verbindung endet mitten im Rahmenkopf (nur das erste Byte da): nichts davon geht hinaus', [VOR, '\x81'], {}, { hinaus: 1, abriss: null, abgeschnitten: true, folge: [[1, 0]], ende: [1, 0] });
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 9 (WebSocket im Probelauf, Z1 bis Z3, Nachtrag 17) und 13.3. Nur Betriebsart 2, nur nach einer durchgereichten 101. von_harness sind die Teile (latin1-Text) der Rahmen vom Harness zum Dienst, maskiert nach RFC 6455 Abschnitt 5.3; die Lage davor: die Upgrade-Anfrage ist gezaehlt, also anfragen 1 und alle Marker 0 (datei_anfang). an_ziel sind die Bytes, die zum Dienst gehen: unveraendert, also maskiert, und nur vollstaendig gelesene, geprueft und gezaehlte Rahmen. datei_folge ist der Inhalt der Zaehldatei nach jedem vollstaendigen Datenrahmen (RFC 8785); Steuerrahmen schreiben nichts. abriss: Code, mit dem beide Richtungen schliessen (Ergebnis abgelehnt, status 101); null heisst Ende der Verbindung. Ohne probelauf gibt es weder Zaehlung noch Pruefung. Marker, Schluessel und Platzhalter sind erfundene Testwerte.', sitzung: 's-1', faelle };
}

// --- Suche nach dem Schluessel (Abschnitt 9a, Nachtrag 18): Faelle ---

// Mutanten der Suche: M-S1 Verknuepfungen verfolgt, M-S2 nicht lesbare zaehlen als ohne Fund, M-S3 nur der erste Schluessel, M-S4 Fund ueber eine Lesegrenze zaehlt nicht.
const SU_CODE = { 'mw.suche_aufruf': [64, 'Suchaufruf ungueltig'], 'mw.ziele_ungueltig': [78, 'Zielliste ungueltig'], 'mw.schluessel_fehlt': [78, 'Schluessel nicht lesbar'], ...UMG_CODE };
const SU_BLOCK = 4096;   // nur das Modell des Mutanten M-S4
const SU_QUELLEN = ['--schluessel-datei', '--zielliste'];
function sucheAufruf(argv) {
  if (argv[0] !== 'suche') throw fehler('mw.suche_aufruf');
  const w = new Map(); const ins = [];
  for (let i = 1; i < argv.length; i += 2) {
    const o = argv[i];
    if (!(SU_QUELLEN.includes(o) || o === '--in') || i + 1 >= argv.length) throw fehler('mw.suche_aufruf');
    if (o === '--in') ins.push(argv[i + 1]);
    else { if (w.has(o)) throw fehler('mw.suche_aufruf'); w.set(o, argv[i + 1]); }
  }
  if (w.size !== 1 || ins.length < 1 || ins.length > 16) throw fehler('mw.suche_aufruf');
  for (const p of [...w.values(), ...ins]) if (!PFAD_ABS(p, 1024)) throw fehler('mw.suche_aufruf');
  return { quelle: [...w.keys()][0], datei: [...w.values()][0], ins };
}
function sucheSchluessel(a, baum, zlText) {
  let dateien;
  if (a.quelle === '--schluessel-datei') dateien = [a.datei];
  else {
    let liste; try { liste = zieleLesen(zlText); } catch (x) { if (x instanceof Fehler) throw fehler('mw.ziele_ungueltig'); throw x; }
    dateien = [...new Set(liste.filter((z) => z.schluessel_datei !== null).map((z) => z.schluessel_datei))];
    if (dateien.length === 0) throw fehler('mw.schluessel_fehlt');
  }
  const werte = [];
  for (const d of dateien) {
    const e = baum.get(d);
    let w; try { w = schluesselLesen(e ? { art: e.art, eigen: e.eigen !== false, modus: e.modus ?? '0600', inhalt: e.inhalt ?? '' } : { art: 'fehlt' }); } catch (x) { if (x instanceof Fehler) throw fehler('mw.schluessel_fehlt'); throw x; }
    if (!werte.includes(w)) werte.push(w);
  }
  return M('M-S3') ? werte.slice(0, 1) : werte;   // M-S3: nur der erste Schluessel
}
function sucheZaehlen(werte, inhalt) {
  const b = Buffer.from(inhalt, 'latin1'); let n = 0;
  const bloecke = M('M-S4') ? Array.from({ length: Math.ceil(b.length / SU_BLOCK) }, (_, i) => b.subarray(i * SU_BLOCK, (i + 1) * SU_BLOCK)) : [b];   // M-S4: je Block gezaehlt
  for (const w of werte) for (const blk of bloecke) { const k = Buffer.from(w, 'latin1'); let i = 0; for (;;) { const j = blk.indexOf(k, i); if (j < 0) break; n++; i = j + k.length; } }
  return n;
}
function sucheLaufen(argv, env, baumListe, zlText) {
  umgebungPruefen('openai-lokal', env);
  const a = sucheAufruf(argv);
  const baum = new Map(baumListe.map((e) => [e.pfad, e]));
  const werte = sucheSchluessel(a, baum, zlText);
  const z = { dateien: 0, nicht_lesbar: 0, treffer: 0, uebersprungen: 0 };
  const gesehen = new Set();
  const kinder = (p) => baumListe.filter((e) => e.pfad.slice(0, e.pfad.lastIndexOf('/')) === p).map((e) => e.pfad).sort();
  const gehe = (p, wurzel) => {
    let e = baum.get(p);
    if (e && e.art === 'verknuepfung' && M('M-S1')) e = baum.get(e.ziel);   // M-S1: Verknuepfungen werden verfolgt
    if (wurzel && (!e || !['datei', 'ordner'].includes(e.art))) { z.nicht_lesbar++; return; }
    if (!e || gesehen.has(p)) return;
    gesehen.add(p);
    if (e.art === 'verknuepfung' || e.art === 'fifo') { z.uebersprungen++; return; }
    if (e.lesbar === false) { z.nicht_lesbar++; return; }
    if (e.art === 'datei') { z.dateien++; z.treffer += sucheZaehlen(werte, e.inhalt ?? ''); return; }
    for (const k of kinder(p)) gehe(k, false);
  };
  for (const p of a.ins) gehe(p, true);
  const exit = z.treffer > 0 ? 1 : (z.nicht_lesbar > 0 && !M('M-S2') ? 3 : 0);   // M-S2: nicht lesbare zaehlen als ohne Fund
  return { exit, stdout: jcs(z) + '\n', stderr: '' };
}
function baueSuche() {
  const LAUF = '/srv/beispiel/lauf'; const ARB = `${LAUF}/arbeit`; const PROT = `${LAUF}/protokoll`; const ZAEHL = `${LAUF}/zaehlung`; const AUS = `${LAUF}/ausgabe/sitzung.txt`;
  const SK = `${LAUF}/schluessel`;
  const K1 = 'mw-suchschluessel-4c8e1a7d93b0'; const K2 = 'mw-suchschluessel-b52f60d1e8a7'; const K3 = 'wb-t-wb';
  const KD1 = `${SK}/lokal.schluessel`; const KD2 = `${SK}/copilot-a.schluessel`; const KD3 = `${SK}/copilot-b.schluessel`;
  const IN = ['--in', ARB, '--in', PROT, '--in', ZAEHL, '--in', AUS];
  const d = (pfad, inhalt = '', extra = {}) => ({ pfad, art: 'datei', inhalt, lesbar: true, ...extra });
  const o = (pfad, extra = {}) => ({ pfad, art: 'ordner', lesbar: true, ...extra });
  const v = (pfad, ziel) => ({ pfad, art: 'verknuepfung', ziel });
  const fifo = (pfad) => ({ pfad, art: 'fifo' });
  const schl = (pfad, wert, extra = {}) => d(pfad, `${wert}\n`, { eigen: true, modus: '0600', ...extra });
  const BASIS = [o('/srv'), o('/srv/beispiel'), o(LAUF), o(ARB), o(PROT), o(ZAEHL), o(`${LAUF}/ausgabe`), o(SK),
    d(`${ARB}/notizen.txt`, 'Notizen der Sitzung ohne Geheimnis.\n'), d(`${PROT}/gateway-2026-11.jsonl`, '{"aktionsname":"modellweg.aufruf"}\n'), d(`${ZAEHL}/zaehlung.json`, '{"anfragen":2,"marker":{},"sitzung":"s-1"}'), d(AUS, 'sichtbare Ausgabe\n')];
  const ohneP = (baum, ...pfade) => baum.filter((e) => !pfade.includes(e.pfad));
  const mitP = (...extra) => { const m = new Map(BASIS.map((e) => [e.pfad, e])); for (const e of extra) m.set(e.pfad, e); return [...m.values()]; };
  const lokal = (...ex) => ['suche', '--schluessel-datei', KD1, ...ex];
  const ZL = (ziele) => JSON.stringify({ fassung: 1, ca_zulassung: 'zugelassen', ziele }, null, 1) + '\n';
  const gz = (name, host, datei) => ({ name, host, port: 443, schluessel_datei: datei, kopf: 'authorization', wert_vorsatz: 'Bearer ', regeln: [{ methode: '*', pfad: '/*' }], upgrade: 'durchreichen' });
  const uz = (name, host) => ({ name, host, port: 443, schluessel_datei: null, kopf: null, wert_vorsatz: null, regeln: [{ methode: 'POST', pfad: '/telemetry' }], upgrade: 'ablehnen' });
  const ZLD = `${LAUF}/zielliste.json`;
  const cloud = (...ex) => ['suche', '--zielliste', ZLD, ...ex];
  const ZL_ZWEI_GLEICH = ZL([gz('api-a', 'api.beispiel.test', KD2), gz('api-b', 'plan.beispiel.test', KD2), uz('telemetrie', 'telemetry.beispiel.test')]);
  const ZL_ZWEI = ZL([gz('api-a', 'api.beispiel.test', KD2), gz('api-b', 'plan.beispiel.test', KD3)]);
  const ZL_NUR_UNGEBUNDEN = ZL([uz('telemetrie', 'telemetry.beispiel.test')]);
  const ZL_KAPUTT = '{"fassung":1,"ca_zulassung":"zugelassen","ziele":[]}\n';
  const lang = (n, wert, rest = 'x') => 'a'.repeat(n) + wert + rest;
  const E = (exit, z) => ({ exit, ...z });
  // [name, beschreibung, argv, baum, erwartet, extra]; erwartet: {exit, dateien, nicht_lesbar, treffer, uebersprungen} oder {exit, code}
  const F = [
    ['kein_fund', 'alle vier Wurzeln ohne den Schluessel: gruen', lokal(...IN), mitP(schl(KD1, K1)), E(0, { dateien: 4, nicht_lesbar: 0, treffer: 0, uebersprungen: 0 })],
    ['fund_eine_datei', 'der Schluessel steht einmal im Arbeitsordner, ohne LF am Dateiende (der gesuchte Wert hat kein LF): rot', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/ausgabe.txt`, `Token: ${K1}`)), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['fund_zwei_in_datei', 'zwei Vorkommen in einer Datei zaehlen zweimal', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/doppelt.txt`, `a ${K1} b\n${K1}\n`)), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 2, uebersprungen: 0 })],
    ['fund_in_ausgabedatei', 'der Schluessel steht nur in der sichtbaren Ausgabe, die als Datei unter --in steht: rot', lokal(...IN), mitP(schl(KD1, K1), d(AUS, `sichtbar: ${K1}\n`)), E(1, { dateien: 4, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['fund_in_unterordner', 'tief verschachtelte Datei im Protokollordner: rot', lokal(...IN), mitP(schl(KD1, K1), o(`${PROT}/a`), o(`${PROT}/a/b`), d(`${PROT}/a/b/c.txt`, `${K1}\n`)), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['fund_lesegrenze_4096', 'der Schluessel liegt ueber der Grenze von 4096 Byte (Hinweis: der Bau testet mit einem kleinen Puffer, auch mit 1 bis 7 Byte)', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/grenze.bin`, lang(4090, K1))), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['fund_lesegrenze_65536', 'der Schluessel liegt ueber der Grenze von 65536 Byte, zwei Vorkommen in der Datei', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/grenze2.bin`, lang(65530, K1, K1))), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 2, uebersprungen: 0 })],
    ['ohne_ueberlappung', 'Schluessel wb-t-wb in wb-t-wb-t-wb: ohne Ueberlappung ein Vorkommen, nicht zwei', lokal(...IN), mitP(schl(KD1, K3), d(`${ARB}/ueberlappung.txt`, 'wb-t-wb-t-wb\n')), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['nur_verknuepfung', 'der Schluessel steht nur hinter Verknuepfungen im Arbeitsordner (auf die Schluesseldatei und auf einen Ordner ausserhalb): nicht gelesen, nicht verfolgt, kein Fund', lokal(...IN),
      mitP(schl(KD1, K1), o(`${LAUF}/fremd`), d(`${LAUF}/fremd/kopie.txt`, `${K1}\n`), v(`${ARB}/verweis-schluessel`, KD1), v(`${ARB}/verweis-ordner`, `${LAUF}/fremd`)), E(0, { dateien: 4, nicht_lesbar: 0, treffer: 0, uebersprungen: 2 })],
    ['in_ist_verknuepfung', '--in ist selbst eine Verknuepfung auf einen Ordner mit dem Schluessel: nicht durchsucht, nicht als Fund gezaehlt; als Wurzel zaehlt sie in nicht_lesbar, also offen (Exit 3, offener Punkt des Vertrags)', lokal('--in', `${LAUF}/wurzel-verweis`, '--in', ARB),
      mitP(schl(KD1, K1), o(`${LAUF}/fremd`), d(`${LAUF}/fremd/kopie.txt`, `${K1}\n`), v(`${LAUF}/wurzel-verweis`, `${LAUF}/fremd`)), E(3, { dateien: 1, nicht_lesbar: 1, treffer: 0, uebersprungen: 0 })],
    ['in_fehlt', '--in nennt einen Pfad, den es nicht gibt: nicht durchsucht, also offen', lokal('--in', ARB, '--in', `${LAUF}/gibt-es-nicht`), mitP(schl(KD1, K1)), E(3, { dateien: 1, nicht_lesbar: 1, treffer: 0, uebersprungen: 0 })],
    ['nicht_lesbar_ohne_fund', 'eine Datei ist nicht lesbar, sonst kein Fund: offen (Exit 3)', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/gesperrt.txt`, 'x', { lesbar: false })), E(3, { dateien: 4, nicht_lesbar: 1, treffer: 0, uebersprungen: 0 })],
    ['nicht_lesbar_mit_fund', 'eine Datei ist nicht lesbar und anderswo steht der Schluessel: rot (Exit 1)', lokal(...IN), mitP(schl(KD1, K1), d(`${ARB}/gesperrt.txt`, 'x', { lesbar: false }), d(`${PROT}/leck.txt`, K1)), E(1, { dateien: 5, nicht_lesbar: 1, treffer: 1, uebersprungen: 0 })],
    ['nicht_lesbarer_ordner', 'ein Ordner ist nicht lesbar: die Suche steigt nicht ab, offen', lokal(...IN), mitP(schl(KD1, K1), o(`${ARB}/zu`, { lesbar: false }), d(`${ARB}/zu/innen.txt`, K1)), E(3, { dateien: 4, nicht_lesbar: 1, treffer: 0, uebersprungen: 0 })],
    ['fifo', 'eine FIFO wird nicht geoeffnet und zaehlt in uebersprungen, nicht in nicht_lesbar', lokal(...IN), mitP(schl(KD1, K1), fifo(`${ARB}/rohr`)), E(0, { dateien: 4, nicht_lesbar: 0, treffer: 0, uebersprungen: 1 })],
    ['verschachtelte_in', '--in nennt dieselben Dateien mehrfach (Ordner, Unterordner und die Datei selbst): jede Datei einmal gelesen und gezaehlt', lokal('--in', ARB, '--in', `${ARB}/unter`, '--in', `${ARB}/unter/fund.txt`),
      mitP(schl(KD1, K1), o(`${ARB}/unter`), d(`${ARB}/unter/fund.txt`, K1)), E(1, { dateien: 2, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['schluesseldatei_unter_in', 'die Schluesseldatei selbst liegt unter einem --in: sie zaehlt als Fund, rot (keine Ausnahme per Inode, Abschnitt 9a S4)', lokal('--in', SK), mitP(schl(KD1, K1)), E(1, { dateien: 1, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 })],
    ['zielliste_gleiche_datei', 'Zielliste mit zwei gebundenen Zielen auf dieselbe Schluesseldatei und einem ungebundenen Ziel: ein Schluessel, ein Fund zaehlt einmal', cloud(...IN),
      mitP(schl(KD2, K2), d(ZLD, ZL_ZWEI_GLEICH), d(`${ARB}/leck.txt`, `${K2}\n`)), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 }), { zielliste: ZL_ZWEI_GLEICH }],
    ['zielliste_zwei_schluessel_fund_zweiter', 'Zielliste mit zwei verschiedenen Schluesseln: nur der zweite steht im Arbeitsordner, rot', cloud(...IN),
      mitP(schl(KD2, K2), schl(KD3, `${K3}-z`), d(`${ARB}/leck.txt`, `${K3}-z`)), E(1, { dateien: 5, nicht_lesbar: 0, treffer: 1, uebersprungen: 0 }), { zielliste: ZL_ZWEI }],
    ['zielliste_zwei_schluessel_beide', 'beide Schluessel stehen je einmal: treffer ist die Summe, 2', cloud(...IN),
      mitP(schl(KD2, K2), schl(KD3, K1), d(`${ARB}/a.txt`, K2), d(`${PROT}/b.txt`, K1)), E(1, { dateien: 6, nicht_lesbar: 0, treffer: 2, uebersprungen: 0 }), { zielliste: ZL_ZWEI }],
    ['zielliste_kein_fund', 'Zielliste mit zwei Schluesseln, keiner steht irgendwo: gruen', cloud(...IN), mitP(schl(KD2, K2), schl(KD3, K1)), E(0, { dateien: 4, nicht_lesbar: 0, treffer: 0, uebersprungen: 0 }), { zielliste: ZL_ZWEI }],
    ['platzhalter_gho', 'ein Platzhalter im gho_-Format steht im Arbeitsordner (zu kurz fuer ein Token, ausdruecklich nicht der Wert): kein Fund', cloud(...IN),
      mitP(schl(KD2, K2), schl(KD3, K1), d(`${ARB}/umgebung.txt`, 'COPILOT_GITHUB_TOKEN=gho_platzhalter-0000\n')), E(0, { dateien: 5, nicht_lesbar: 0, treffer: 0, uebersprungen: 0 }), { zielliste: ZL_ZWEI }],
    ['aufruf_ohne_in', 'ohne --in', lokal(), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_ohne_quelle', 'ohne Schluesselquelle', ['suche', ...IN], mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_beide_quellen', 'beide Schluesselquellen', ['suche', '--schluessel-datei', KD1, '--zielliste', ZLD, ...IN], mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }, { zielliste: ZL_ZWEI }],
    ['aufruf_quelle_doppelt', '--schluessel-datei zweimal', ['suche', '--schluessel-datei', KD1, '--schluessel-datei', KD1, ...IN], mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_relativer_in', '--in als relativer Pfad', lokal('--in', 'arbeit'), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_relative_quelle', '--schluessel-datei als relativer Pfad', ['suche', '--schluessel-datei', 'lokal.schluessel', ...IN], mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_punkt_segment', '--in mit Segment ..', lokal('--in', `${ARB}/../arbeit`), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_17_in', '17 mal --in', lokal(...Array.from({ length: 17 }, (_, i) => ['--in', `${ARB}/w${i}`]).flat()), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_16_in', '16 mal --in ist gueltig (Wurzeln, die es nicht gibt, zaehlen in nicht_lesbar)', lokal(...Array.from({ length: 16 }, (_, i) => ['--in', `${ARB}/w${i}`]).flat()), mitP(schl(KD1, K1)), E(3, { dateien: 0, nicht_lesbar: 16, treffer: 0, uebersprungen: 0 })],
    ['aufruf_gleichheitszeichen', '--in=pfad statt zwei Argumente', lokal(`--in=${ARB}`, 'x'), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['aufruf_unbekannte_option', 'eine unbekannte Option', lokal(...IN, '--schluessel', K1), mitP(schl(KD1, K1)), { exit: 64, code: 'mw.suche_aufruf' }],
    ['schluessel_modus_0644', 'die Schluesseldatei hat Modus 0644: keine Suche', lokal(...IN), mitP(schl(KD1, K1, { modus: '0644' }), d(`${ARB}/leck.txt`, K1)), { exit: 78, code: 'mw.schluessel_fehlt' }],
    ['schluessel_datei_fehlt', 'die Schluesseldatei gibt es nicht', lokal(...IN), ohneP(mitP(), KD1), { exit: 78, code: 'mw.schluessel_fehlt' }],
    ['schluessel_verknuepfung', 'der Pfad der Schluesseldatei ist eine Verknuepfung (O_NOFOLLOW)', lokal(...IN), mitP(schl(`${SK}/echt.schluessel`, K1), v(KD1, `${SK}/echt.schluessel`)), { exit: 78, code: 'mw.schluessel_fehlt' }],
    ['schluessel_zwei_zeilen', 'die Schluesseldatei hat zwei Zeilen', lokal(...IN), mitP(d(KD1, `${K1}\n${K2}\n`, { eigen: true, modus: '0600' }), d(`${ARB}/leck.txt`, K1)), { exit: 78, code: 'mw.schluessel_fehlt' }],
    ['zielliste_zweite_datei_schlecht', 'Zielliste mit zwei Schluesseln, der zweite hat Modus 0644: keine Suche, obwohl der erste im Arbeitsordner steht', cloud(...IN),
      mitP(schl(KD2, K2), schl(KD3, K1, { modus: '0644' }), d(`${ARB}/leck.txt`, K2)), { exit: 78, code: 'mw.schluessel_fehlt' }, { zielliste: ZL_ZWEI }],
    ['zielliste_ungueltig', 'die Zielliste ist ungueltig (keine Ziele): Abschnitt 12', cloud(...IN), mitP(schl(KD2, K2)), { exit: 78, code: 'mw.ziele_ungueltig' }, { zielliste: ZL_KAPUTT }],
    ['zielliste_ohne_gebundenes_ziel', 'gueltige Zielliste ohne gebundenes Ziel: nichts zu suchen, kein Gruen', cloud(...IN), mitP(schl(KD2, K2)), { exit: 78, code: 'mw.schluessel_fehlt' }, { zielliste: ZL_NUR_UNGEBUNDEN }],
    ['umgebung_ssl_cert_file', 'Stufe 0 gilt auch hier: SSL_CERT_FILE gesetzt, der Aufruf ist sonst gueltig', lokal(...IN), mitP(schl(KD1, K1)), { exit: 78, code: 'mw.umgebung_unsicher' }, { umgebung: { SSL_CERT_FILE: '/srv/beispiel/openssl/x' } }],
    ['umgebung_vor_aufruf', 'Stufe 0 vor dem Aufruf: ungueltiger Aufruf und OPENSSL_CONF leer gesetzt ergeben mw.umgebung_unsicher', ['suche'], mitP(schl(KD1, K1)), { exit: 78, code: 'mw.umgebung_unsicher' }, { umgebung: { OPENSSL_CONF: '' } }],
    ['umgebung_andere', 'andere Variablen stoeren nicht', lokal(...IN), mitP(schl(KD1, K1)), E(0, { dateien: 4, nicht_lesbar: 0, treffer: 0, uebersprungen: 0 }), { umgebung: { PATH: '/usr/bin', SSL_CERT_FILES: '/srv/beispiel/x' } }],
  ];
  const faelle = [];
  for (const [name, beschreibung, argv, baum, erw, extra = {}] of F) {
    const umgebung = extra.umgebung ?? {};
    let ist; try { ist = sucheLaufen(argv, umgebung, baum, extra.zielliste ?? ''); } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      ist = { exit: SU_CODE[x.message][0], stdout: '', stderr: `${x.message}: ${SU_CODE[x.message][1]}\n` };
    }
    const soll = erw.code ? { exit: erw.exit, stdout: '', stderr: `${erw.code}: ${SU_CODE[erw.code][1]}\n` } : { exit: erw.exit, stdout: jcs({ dateien: erw.dateien, nicht_lesbar: erw.nicht_lesbar, treffer: erw.treffer, uebersprungen: erw.uebersprungen }) + '\n', stderr: '' };
    if (ist.exit !== soll.exit || ist.stdout !== soll.stdout || ist.stderr !== soll.stderr) abbruch(`suche ${name}: Modell ${JSON.stringify(ist)} gegen ${JSON.stringify(soll)}`);
    faelle.push({ name, beschreibung, argv, umgebung, baum, ...(extra.zielliste ? { zielliste: { pfad: ZLD, inhalt: extra.zielliste } } : {}), erwartet: soll });
  }
  return { fassung: 1, hinweis: 'Vertrag 8, Abschnitt 9a (Suche nach dem Schluessel, Nachtrag 18). argv ist der Aufruf des Programms ab dem Unterbefehl. baum ist der Dateibaum der Probe als Liste: pfad, art (datei, ordner, verknuepfung, fifo), inhalt (latin1-Text, nur datei), lesbar (false: nicht lesbar), ziel (nur verknuepfung); eigen und modus gelten fuer Schluesseldateien (Vorgabe: eigene Datei, 0600). Ordner stehen als eigene Eintraege. zielliste ist die Datei der Route Copilot (pfad und inhalt). erwartet: exit, stdout (eine Zeile RFC 8785 mit dateien, nicht_lesbar, treffer, uebersprungen, oder leer) und stderr (leer oder die feste Zeile mit Code). Die Schluessel sind erfundene Testwerte und nicht tokenfoermig; der Platzhalter im gho_-Format ist bewusst kuerzer als ein Token. Hinweis fuer den Bau: Der Fall fund_lesegrenze_4096 und die Grenzen bei kleinem Puffer gehoeren in dessen Tests.', faelle };
}

// --- schreiben oder vergleichen ---

const dateien = {
  'faelle/anfragen.json': baueAnfragen(),
  'faelle/antworten.json': baueAntworten(),
  'faelle/aufruf.json': baueAufruf(),
  'faelle/zeile.json': baueZeile(),
  'faelle/probelauf.json': baueMarker(),
  'faelle/umgebung.json': baueUmgebung(),
  'faelle/suche.json': baueSuche(),
  ...baueKopf(),
  'faelle/probelauf-websocket.json': baueProbelaufWs(),
};
const ausgabe = Object.fromEntries(Object.entries(dateien).map(([k, v]) => [k, JSON.stringify(v, null, 1) + '\n']));
if (process.argv.includes('--pruefen')) {
  let gleich = true;
  for (const [pfad, inhalt] of Object.entries(ausgabe)) {
    let alt = null; try { alt = readFileSync(join(HIER, pfad), 'utf8'); } catch { /* fehlt */ }
    if (alt !== inhalt) { process.stdout.write(`verschieden: ${pfad}\n`); gleich = false; }
  }
  process.stdout.write(gleich ? `gleich: ${Object.keys(ausgabe).length} Dateien\n` : '');
  process.exit(gleich ? 0 : 1);
}
mkdirSync(join(HIER, 'faelle'), { recursive: true });
for (const [pfad, inhalt] of Object.entries(ausgabe)) writeFileSync(join(HIER, pfad), inhalt);
const n = (d) => ['faelle', 'zeilen', 'ketten', 'aufruf', 'schluesseldatei', 'anfragen', 'antworten', 'von_harness'].reduce((s, k) => s + (d[k] ? d[k].length : 0), 0);
process.stdout.write(`geschrieben: ${Object.entries(dateien).map(([k, v]) => `${k} ${n(v)}`).join(', ')}\n`);

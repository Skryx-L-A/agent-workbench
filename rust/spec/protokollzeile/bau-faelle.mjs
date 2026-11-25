// Erzeugt die sichtbaren Faelle von Vertrag 5 (README.md in diesem Ordner) nach faelle/.
// Bytes aus der JCS-Referenz (RFC 8785) und node:crypto; Vektoren aus deren Vektorordner.
// Ein kleiner eigener Pruefer rechnet jeden Prueffall nach; weicht er von der Absicht ab, bricht das Skript ab.
// Aufruf: node rust/spec/protokollzeile/bau-faelle.mjs (leicht, ohne Abhaengigkeit). Gleicher Stand ergibt gleiche Bytes.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { jcs, RFC_BEISPIELE } from '../../../spec/protokoll/jcs.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const VEK = join(HIER, '../../../spec/protokoll/vektoren');
const AUS = join(HIER, 'faelle');

const NULL64 = '0'.repeat(64);
const SCHLUESSEL = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const ZEIT = /^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$/;
const HEX = /^[0-9a-f]{64}$/;
const QUELLEN = ['gateway', 'stelle', 'brain', 'halter', 'ordnermarke'];
const MAX_MS = Date.UTC(9999, 11, 31, 23, 59, 59, 999);
const ZEILE_MAX = 4194304;
const GESCHLOSSEN = ['aktionsname', 'anzahl', 'ergebnis', 'fassung', 'grund', 'klasse', 'kunde', 'route', 'sitzung', 'vorher', 'zeit'];

const sha = (b) => createHash('sha256').update(typeof b === 'string' ? Buffer.from(b, 'utf8') : b).digest('hex');
class Fehler extends Error {}
const fehler = (code) => new Fehler(code);

// --- Bibliothek (Abschnitt 2) ---

function zeitText(ms) {
  if (!Number.isInteger(ms) || ms < 0 || ms > MAX_MS) throw fehler('zeit_ungueltig');
  return new Date(ms).toISOString();
}

function abschnitt(quelle, ms) {
  if (!QUELLEN.includes(quelle)) throw fehler('feld_ungueltig:quelle');
  return `${quelle}-${zeitText(ms).slice(0, 7)}.jsonl`;
}

function zeileGesperrt(e, vorher) {
  const pflicht = (feld) => { if (typeof e[feld] !== 'string' || !SCHLUESSEL.test(e[feld])) throw fehler(`feld_ungueltig:${feld}`); };
  const kann = (feld) => { if (e[feld] !== null && (typeof e[feld] !== 'string' || !SCHLUESSEL.test(e[feld]))) throw fehler(`feld_ungueltig:${feld}`); };
  pflicht('route'); kann('klasse'); kann('kunde'); pflicht('sitzung'); pflicht('grund');
  if (!Number.isInteger(e.anzahl) || e.anzahl < 1 || e.anzahl > 4294967295) throw fehler('feld_ungueltig:anzahl');
  if (typeof vorher !== 'string' || !HEX.test(vorher)) throw fehler('feld_ungueltig:vorher');
  const zeit = zeitText(e.zeit_ms);
  return jcs({
    aktionsname: 'ausgabe.gesperrt', anzahl: e.anzahl, ergebnis: 'gesperrt', fassung: 2, grund: e.grund,
    klasse: e.klasse, kunde: e.kunde, route: e.route, sitzung: e.sitzung, vorher, zeit,
  });
}

// anhaengen auf einen Dateiinhalt (Text oder null fuer "fehlt"); gibt { hash, inhalt } oder wirft.
function anhaengen(inhalt, e) {
  const probe = zeileGesperrt(e, NULL64); // Felder zuerst pruefen, wie in Abschnitt 2
  void probe;
  const alt = inhalt ?? '';
  if (alt.length > 0 && !alt.endsWith('\n')) throw fehler('letzte_zeile_unvollstaendig');
  const teile = alt.split('\n');
  const letzte = alt.length === 0 ? null : teile[teile.length - 2];
  const vorher = letzte === null ? NULL64 : sha(letzte);
  const zeile = zeileGesperrt(e, vorher);
  return { hash: sha(zeile), inhalt: `${alt}${zeile}\n` };
}

// --- Pruefer (Abschnitt 3), unabhaengig vom Rust-Bau ---

function familie(name) {
  if (/^gateway-([0-9]{4}-(?:0[1-9]|1[0-2]))\.jsonl$/.test(name)) return '1g';
  if (/^audit\.jsonl$/.test(name) || /^audit\.([0-9]{4}-(?:0[1-9]|1[0-2]))(?:\.([0-9]+))?\.jsonl$/.test(name)) return '1s';
  return null;
}

const EINSAM = /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/;
function einsamesSurrogat(w) {
  if (typeof w === 'string') return EINSAM.test(w);
  if (Array.isArray(w)) return w.some(einsamesSurrogat);
  if (w !== null && typeof w === 'object') return Object.entries(w).some(([k, v]) => EINSAM.test(k) || einsamesSurrogat(v));
  return false;
}

function geschlossen(w) {
  const keys = Object.keys(w).sort();
  if (keys.length !== GESCHLOSSEN.length || keys.some((k, i) => k !== GESCHLOSSEN[i])) return false;
  const schl = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
  return w.aktionsname === 'ausgabe.gesperrt' && w.ergebnis === 'gesperrt' && w.fassung === 2
    && Number.isInteger(w.anzahl) && w.anzahl >= 1 && w.anzahl <= 4294967295
    && schl(w.grund) && schl(w.route) && schl(w.sitzung)
    && (w.klasse === null || schl(w.klasse)) && (w.kunde === null || schl(w.kunde))
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}

const has = (o, k) => Object.prototype.hasOwnProperty.call(o, k);

// Die Zeile kanal.abgewiesen (Abschnitt 6).
const GRUENDE_PK = ['pk.hallo_fehlt', 'pk.hallo_falsch', 'pk.messung_gescheitert', 'pk.prozess_nicht_offen', 'pk.job_unbekannt', 'pk.prozess_beendet', 'pk.startzeit_weicht_ab', 'pk.pidversion_weicht_ab', 'pk.eingeschraenkt', 'pk.sid_nicht_erlaubt', 'pk.appcontainer_falsch', 'pk.uid_nicht_erlaubt', 'pk.gid_falsch'];
const GRUENDE_SUCH = ['such.dienst_stumm', 'such.lauf_unbekannt', 'such.sitzung_fremd', 'such.kennung_fremd', 'such.kennung_unmessbar'];
const grundAbgOk = (kanal, grund) => GRUENDE_PK.includes(grund) || (kanal === 'such' && GRUENDE_SUCH.includes(grund));
const ABG_FELDER = ['aktionsname', 'ergebnis', 'fassung', 'grund', 'kanal', 'route', 'sitzung', 'vorher', 'zeit'];
function geschlossenAbg(w) {
  const keys = Object.keys(w).sort();
  if (keys.length !== ABG_FELDER.length || keys.some((k, i) => k !== ABG_FELDER[i])) return false;
  const schl = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
  return w.aktionsname === 'kanal.abgewiesen' && w.ergebnis === 'abgewiesen' && w.fassung === 2
    && grundAbgOk(w.kanal, w.grund) && schl(w.kanal) && (w.route === null || schl(w.route)) && (w.sitzung === null || schl(w.sitzung))
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
// Die Zeile probelauf.zaehlung (Abschnitt 6): acht Felder, keine Marker.
const PROGRAMME = ['suche', 'modellweg'];
const PROBE_FELDER = ['aktionsname', 'anzahl', 'ergebnis', 'fassung', 'programm', 'sitzung', 'vorher', 'zeit'];
function geschlossenProbe(w) {
  const keys = Object.keys(w).sort();
  if (keys.length !== PROBE_FELDER.length || keys.some((k, i) => k !== PROBE_FELDER[i])) return false;
  return w.aktionsname === 'probelauf.zaehlung' && w.ergebnis === 'an' && w.fassung === 2
    && Number.isInteger(w.anzahl) && w.anzahl >= 1 && w.anzahl <= 16
    && PROGRAMME.includes(w.programm) && typeof w.sitzung === 'string' && SCHLUESSEL.test(w.sitzung)
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
function zeileProbelauf(e, vorher) {
  const reihe = [['programm', PROGRAMME.includes(e.programm)], ['sitzung', typeof e.sitzung === 'string' && SCHLUESSEL.test(e.sitzung)], ['anzahl', Number.isInteger(e.anzahl) && e.anzahl >= 1 && e.anzahl <= 16], ['vorher', typeof vorher === 'string' && HEX.test(vorher)]];
  const falsch = reihe.find(([, ok]) => !ok);
  if (falsch) throw fehler(`feld_ungueltig:${falsch[0]}`);
  return jcs({ aktionsname: 'probelauf.zaehlung', anzahl: e.anzahl, ergebnis: 'an', fassung: 2, programm: e.programm, sitzung: e.sitzung, vorher, zeit: zeitText(e.zeit_ms) });
}
// Die Zeile modellweg.aufruf (Abschnitt 6.7): Vertrag 8 legt sie fest, hier steht nur, was der Pruefer verlangt.
const MW_1 = ['aktionsname', 'betriebsart', 'ergebnis', 'fassung', 'grund', 'host', 'modell', 'pfad', 'route', 'sitzung', 'status', 'token_aus', 'token_ein', 'vorher', 'zeit'];
const MW_2 = ['aktionsname', 'betriebsart', 'ergebnis', 'fassung', 'grund', 'host', 'kopf', 'methode', 'pfad', 'pfadregel', 'route', 'sitzung', 'status', 'vorher', 'zeit', 'ziel'];
const MW_ERGEBNIS = ['weitergereicht', 'abgelehnt', 'ziel_fehler', 'leck_gesperrt'];
const MW_METHODEN = ['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'];
const MW_PFAD = /^(\*|\/|(\/(\*|[A-Za-z0-9._~-]+))+)$/;
const MW_HOST = /^(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
const MW_REGEL = /^(\/\*|(\/[A-Za-z0-9._~-]+)+(\/\*)?)$/;
function geschlossenMw(w) {
  if (!['openai-lokal', 'kopf-einsetzen'].includes(w.betriebsart)) return false;
  const neu = has(w, 'pfad') || has(w, 'host');
  const soll = (w.betriebsart === 'openai-lokal' ? MW_1 : MW_2).filter((f) => neu || !['pfad', 'host'].includes(f));
  const keys = Object.keys(w).sort();
  if (keys.length !== soll.length || keys.some((k, i) => k !== soll[i])) return false;
  const schl = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
  const text = (v, re) => typeof v === 'string' && re.test(v);
  const u32 = (v) => v === null || (Number.isInteger(v) && v >= 0 && v <= 4294967295);
  const eigen = w.betriebsart === 'openai-lokal'
    ? typeof w.modell === 'string' && w.modell.length >= 1 && w.modell.length <= 128 && u32(w.token_ein) && u32(w.token_aus) && (!neu || w.host === null)
    : (w.ziel === null || schl(w.ziel)) && (w.methode === null || MW_METHODEN.includes(w.methode))
      && (w.pfadregel === null || text(w.pfadregel, MW_REGEL)) && [null, 'ersetzt', 'keiner', 'abgewiesen', 'halter'].includes(w.kopf);
  const status = w.status === null ? w.betriebsart === 'kopf-einsetzen' : Number.isInteger(w.status) && w.status >= 100 && w.status <= 599;
  return w.aktionsname === 'modellweg.aufruf' && w.fassung === 2 && MW_ERGEBNIS.includes(w.ergebnis) && eigen && status
    && (w.grund === null || schl(w.grund)) && schl(w.route) && schl(w.sitzung)
    && (!neu || ((w.pfad === null || (text(w.pfad, MW_PFAD) && w.pfad.length <= 512)) && (w.host === null || text(w.host, MW_HOST))))
    && text(w.vorher, HEX) && text(w.zeit, ZEIT);
}
// Die Zeilen person.geloescht und person.auskunft (Vertrag 9, Abschnitt 8): sieben Felder je Zeile.
const PS = /^ps_[0-9a-f]{32}$/;
const handelndOk = (w, nullOk) => (w.handelnd === null ? nullOk : typeof w.handelnd === 'string' && PS.test(w.handelnd));
function geschlossenGeloescht(w) {
  const keys = Object.keys(w).sort();
  if (keys.join() !== ['aktionsname', 'ergebnis', 'fassung', 'handelnd', 'loeschmarke', 'vorher', 'zeit'].join()) return false;
  const marke = (w.ergebnis === 'geloescht' && typeof w.loeschmarke === 'string' && HEX.test(w.loeschmarke)) || (w.ergebnis === 'unbekannt' && w.loeschmarke === null);
  return w.aktionsname === 'person.geloescht' && marke && w.fassung === 2 && handelndOk(w, true)
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
function geschlossenAuskunft(w) {
  const keys = Object.keys(w).sort();
  if (keys.join() !== ['aktionsname', 'ergebnis', 'fassung', 'handelnd', 'vorher', 'zeilen', 'zeit'].join()) return false;
  const zahl = Number.isInteger(w.zeilen) && w.zeilen >= 0 && w.zeilen <= 4294967295;
  const zeilenOk = (w.ergebnis === 'geliefert' && zahl) || (w.ergebnis === 'unbekannt' && w.zeilen === 0);
  return w.aktionsname === 'person.auskunft' && zeilenOk && w.fassung === 2 && handelndOk(w, false)
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
// Die vier Zeilenarten der Entscheidungsstelle (Abschnitt 6.8): Vertrag 6 legt sie fest, hier steht nur,
// was der Pruefer verlangt.
const ST_SCHREIB = ['aktionsname', 'anfrage', 'digest', 'entscheidend', 'ergebnis', 'fassung', 'person', 'server', 'sitzung', 'vorher', 'werkzeug', 'zeit', 'zeit_anfrage', 'zeit_entscheidung'];
const ST_SITZUNG = ['aktionsname', 'ergebnis', 'fassung', 'grund', 'harness', 'kunde', 'route', 'sitzung', 'vorher', 'zeit'];
const ST_VORGANG = ['aktionsname', 'anfrage', 'aufrufer', 'ergebnis', 'fassung', 'grund', 'vorgang', 'vorher', 'zeit'];
const ST_ERGEBNIS = ['eingeloest', 'abgelehnt', 'abgelaufen', 'ohne_freigabe'];
const ST_AUFRUFER = ['kern', 'hub', 'mensch', 'agent', 'takt', 'unbekannt'];
const ST_VORGAENGE = ['mcp.anfragen', 'mcp.entscheiden', 'mcp.einloesen', 'mcp.offene', 'mcp.stand', 'ablauf.pruefen'];
const ST_SITZUNGSARTEN = ['sitzung.gestartet', 'sitzung.abgelehnt'];
const ST_ANFRAGE = /^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$/;
const ST_WERKZEUG = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
// Mutanten des Modells (nur zur Gegenprobe, Umgebungsvariable STELLE_MUTANT): S1 nimmt ein zusaetzliches Feld an,
// S2 prueft die Kopplung von entscheidend und zeit_entscheidung nicht, S3 laesst jede Art der Fassung 2 durch
// (kein Register). Ohne die Variable bleibt die Ausgabe gleich.
const ST_MUTANT = process.env.STELLE_MUTANT ?? '';
const felderGenau = (w, soll) => {
  if (ST_MUTANT === 'S1') return soll.every((f) => has(w, f));
  const keys = Object.keys(w).sort();
  const s = [...soll].sort();
  return keys.length === s.length && keys.every((k, i) => k === s[i]);
};
function geschlossenSchreib(w) {
  if (!felderGenau(w, ST_SCHREIB)) return false;
  const ps = (v) => typeof v === 'string' && PS.test(v);
  const zeit = (v) => typeof v === 'string' && ZEIT.test(v);
  if (!(w.entscheidend === null || ps(w.entscheidend))) return false;
  if (!(w.zeit_entscheidung === null || zeit(w.zeit_entscheidung))) return false;
  const ent = w.entscheidend !== null;
  if (ST_MUTANT !== 'S2' && ent !== (w.zeit_entscheidung !== null)) return false;
  // eingeloest und abgelehnt nennen den Entscheidenden, ohne_freigabe nennt keinen, abgelaufen beides.
  const kopplung = ['eingeloest', 'abgelehnt'].includes(w.ergebnis) ? ent : (w.ergebnis === 'ohne_freigabe' ? !ent : w.ergebnis === 'abgelaufen');
  return w.aktionsname === 'mcp.schreibaufruf' && w.fassung === 2 && ST_ERGEBNIS.includes(w.ergebnis) && kopplung
    && typeof w.anfrage === 'string' && ST_ANFRAGE.test(w.anfrage)
    && typeof w.digest === 'string' && HEX.test(w.digest) && ps(w.person)
    && typeof w.server === 'string' && SCHLUESSEL.test(w.server)
    && typeof w.sitzung === 'string' && SCHLUESSEL.test(w.sitzung)
    && typeof w.werkzeug === 'string' && ST_WERKZEUG.test(w.werkzeug)
    && zeit(w.zeit_anfrage) && typeof w.vorher === 'string' && HEX.test(w.vorher) && zeit(w.zeit);
}
function geschlossenSitzung(w) {
  if (!felderGenau(w, ST_SITZUNG) || !ST_SITZUNGSARTEN.includes(w.aktionsname)) return false;
  const gestartet = w.aktionsname === 'sitzung.gestartet';
  const grund = w.grund === null ? gestartet : (!gestartet && typeof w.grund === 'string' && SCHLUESSEL.test(w.grund));
  return w.ergebnis === (gestartet ? 'gestartet' : 'abgelehnt') && grund && w.fassung === 2
    && ['harness', 'kunde', 'route', 'sitzung'].every((k) => schlNull(w[k]))
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
function geschlossenVorgang(w) {
  if (!felderGenau(w, ST_VORGANG)) return false;
  return w.aktionsname === 'vorgang.abgelehnt' && w.ergebnis === 'abgelehnt' && w.fassung === 2
    && (w.anfrage === null || (typeof w.anfrage === 'string' && ST_ANFRAGE.test(w.anfrage)))
    && ST_AUFRUFER.includes(w.aufrufer) && typeof w.grund === 'string' && SCHLUESSEL.test(w.grund)
    && ST_VORGAENGE.includes(w.vorgang)
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
const stelleForm = (w) => (w.aktionsname === 'mcp.schreibaufruf' ? geschlossenSchreib(w)
  : (ST_SITZUNGSARTEN.includes(w.aktionsname) ? geschlossenSitzung(w)
    : (w.aktionsname === 'vorgang.abgelehnt' ? geschlossenVorgang(w) : true)));
const ST_ARTEN = ['mcp.schreibaufruf', 'sitzung.gestartet', 'sitzung.abgelehnt', 'vorgang.abgelehnt'];
// Register (Abschnitt 3): Eine Zeile der Fassung 2 mit einem Aktionsnamen, den Vertrag 5 nicht nennt, ist form_falsch.
// Gruppe "angemeldet": Vertrag 5 nennt die Art, ihre Form prueft noch kein Arm. Gruppe "fremd": die Form steht in einem anderen Vertrag.
const REG_ANGEMELDET = ['regel.geaendert', 'regel.abgelehnt', 'suche.rueckfall'];
const REG_FREMD = ['hersteller.zugang', 'hersteller.zugriff', 'pflicht.aenderung', 'protokoll.einsicht', 'vorgang.gedrosselt',
  'modell.aufruf', 'fall.freitext', 'mcp.schein_abgelehnt', 'halter.leck_gesperrt', 'halter.pfad_verboten', 'wissen.geschrieben', 'wissen.schreiben.abgelehnt',
  'klasse.erlaubt', 'lauf.melden', 'sitzung.abgemeldet'];
const REG_MIT_FORM = ['ausgabe.gesperrt', 'kanal.abgewiesen', 'probelauf.zaehlung', 'ordnermarke.geloest', 'ordnermarke.gesenkt',
  'person.geloescht', 'person.auskunft', 'modellweg.aufruf', ...ST_ARTEN];
const imRegister = (art) => ST_MUTANT === 'S3' || REG_MIT_FORM.includes(art) || REG_ANGEMELDET.includes(art) || REG_FREMD.includes(art);
// Die Zeilen ordnermarke.geloest und ordnermarke.gesenkt (Abschnitt 6.4): zehn Felder je Zeile, marke ist eine zufaellige Kennung.
// Mutanten des Modells (nur zur Gegenprobe, Umgebungsvariable ORDNERMARKE_MUTANT): O1 nimmt ein zusaetzliches Feld an,
// O2 prueft die Form von marke nicht, O3 nimmt geloest mit klasse_neu an. Ohne die Variable bleibt die Ausgabe gleich.
const MUTANT = process.env.ORDNERMARKE_MUTANT ?? '';
const ORD_ARTEN = ['ordnermarke.geloest', 'ordnermarke.gesenkt'];
const ORD_FELDER = ['aktionsname', 'ergebnis', 'fassung', 'klasse_alt', 'klasse_neu', 'kunde_alt', 'kunde_neu', 'marke', 'vorher', 'zeit'];
const MARKE = /^[0-9a-f]{32}$/;
const schlNull = (v) => v === null || (typeof v === 'string' && SCHLUESSEL.test(v));
// Der erste falsche Wert in der Reihenfolge des Schreibers: erst die Form je Feld, dann die Kopplung; null, wenn alles passt.
function ordFehler(w) {
  if (!ORD_ARTEN.includes(w.aktionsname)) return 'aktionsname';
  if (typeof w.marke !== 'string' || (MUTANT !== 'O2' && !MARKE.test(w.marke))) return 'marke';
  for (const f of ['klasse_alt', 'klasse_neu', 'kunde_alt', 'kunde_neu']) if (!schlNull(w[f])) return f;
  if (w.aktionsname === 'ordnermarke.geloest') {
    if (w.klasse_alt === null && w.kunde_alt === null) return 'klasse_alt';
    if (w.klasse_neu !== null && MUTANT !== 'O3') return 'klasse_neu';
    if (w.kunde_neu !== null) return 'kunde_neu';
  } else {
    if (w.klasse_neu === null || w.klasse_neu === w.klasse_alt) return 'klasse_neu';
    if (w.kunde_neu !== w.kunde_alt) return 'kunde_neu';
  }
  return null;
}
function geschlossenOrd(w) {
  const keys = Object.keys(w).sort();
  const formOk = MUTANT === 'O1' ? ORD_FELDER.every((k) => keys.includes(k)) : keys.length === ORD_FELDER.length && keys.every((k, i) => k === ORD_FELDER[i]);
  return formOk && w.ergebnis === 'ausgefuehrt' && w.fassung === 2 && ordFehler(w) === null
    && typeof w.vorher === 'string' && HEX.test(w.vorher) && typeof w.zeit === 'string' && ZEIT.test(w.zeit);
}
function zeileOrdnermarke(e, vorher) {
  const f = ordFehler(e);
  if (f !== null) throw fehler(`feld_ungueltig:${f}`);
  if (typeof vorher !== 'string' || !HEX.test(vorher)) throw fehler('feld_ungueltig:vorher');
  return jcs({ aktionsname: e.aktionsname, ergebnis: 'ausgefuehrt', fassung: 2, klasse_alt: e.klasse_alt, klasse_neu: e.klasse_neu, kunde_alt: e.kunde_alt, kunde_neu: e.kunde_neu, marke: e.marke, vorher, zeit: zeitText(e.zeit_ms) });
}
function zeileAbgewiesen(e, vorher) {
  const schl = (v) => typeof v === 'string' && SCHLUESSEL.test(v);
  const reihe = [['grund', grundAbgOk(e.kanal, e.grund)], ['kanal', schl(e.kanal)], ['route', e.route === null || schl(e.route)], ['sitzung', e.sitzung === null || schl(e.sitzung)], ['vorher', typeof vorher === 'string' && HEX.test(vorher)]];
  const falsch = reihe.find(([, ok]) => !ok);
  if (falsch) throw fehler(`feld_ungueltig:${falsch[0]}`);
  return jcs({ aktionsname: 'kanal.abgewiesen', ergebnis: 'abgewiesen', fassung: 2, grund: e.grund, kanal: e.kanal, route: e.route, sitzung: e.sitzung, vorher, zeit: zeitText(e.zeit_ms) });
}

function bytesVon(d) {
  return d.inhalt_b64 !== undefined ? Buffer.from(d.inhalt_b64, 'base64') : Buffer.from(d.inhalt, 'utf8');
}

function sekunden(zeit) {
  if (typeof zeit !== 'string' || !ZEIT.test(zeit)) return null;
  const ms = Date.parse(zeit);
  if (!Number.isFinite(ms) || ms < 0 || new Date(ms).toISOString() !== zeit) return null;
  return ms / 1000;
}

// nach(datei, werte, index): wird nach jeder Datei gerufen; gibt null oder { datei, zeile, grund } zurueck (Abschnitt 5.4).
// werte ist null fuer eine Datei ohne Zeile, sonst { erste_vorher, von, bis, letzte }.
function pruefe(dateien, { vorher = NULL64, letzte = null, nach = null } = {}) {
  let prev = vorher;
  let n = 0;
  let letzteDatei = dateien.length ? dateien[dateien.length - 1].name : null;
  let letzteNr = null;
  const rot = (datei, zeile, grund) => ({ bruch: { datei, grund, zeile }, letzte: null, ok: false, zeilen: n });
  for (const [index, d] of dateien.entries()) {
    if (d.fehlt) return rot(d.name, null, 'datei_nicht_lesbar');
    const buf = bytesVon(d);
    const werte = { erste_vorher: prev, von: null, bis: null, letzte: null };
    let inDatei = 0;
    if (buf.length === 0) {
      const b = nach?.(d, null, index);
      if (b) return rot(b.datei, b.zeile, b.grund);
      continue;
    }
    const stuecke = [];
    let start = 0;
    for (let i = 0; i < buf.length; i += 1) if (buf[i] === 0x0a) { stuecke.push(buf.subarray(start, i)); start = i + 1; }
    const unvollstaendig = start < buf.length;
    if (unvollstaendig) stuecke.push(buf.subarray(start));
    for (let i = 0; i < stuecke.length; i += 1) {
      const nr = i + 1;
      const roh = stuecke[i];
      if (unvollstaendig && i === stuecke.length - 1) return rot(d.name, nr, 'letzte_zeile_unvollstaendig');
      if (roh.length === 0 || roh.length > ZEILE_MAX) return rot(d.name, nr, 'zeile_unlesbar');
      let text;
      let w;
      try { text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(roh); w = JSON.parse(text); } catch { return rot(d.name, nr, 'zeile_unlesbar'); }
      if (w === null || typeof w !== 'object' || Array.isArray(w) || einsamesSurrogat(w)) return rot(d.name, nr, 'zeile_unlesbar');
      let fas;
      if (has(w, 'fassung')) {
        if (w.fassung !== 2) return rot(d.name, nr, 'form_falsch');
        fas = '2';
      } else {
        fas = familie(d.name);
        if (fas === null) return rot(d.name, nr, 'fassung_fehlt');
      }
      if (has(w, 'kettenbruch')) return rot(d.name, nr, typeof w.kettenbruch === 'string' ? `angekuendigt:${w.kettenbruch}` : 'form_falsch');
      if (!has(w, 'vorher')) return rot(d.name, nr, 'vorher_fehlt');
      if (w.vorher !== prev) return rot(d.name, nr, 'kette_gebrochen');
      if (fas === '2') {
        if (jcs(w) !== text) return rot(d.name, nr, 'nicht_kanonisch');
        if (typeof w.zeit !== 'string' || !ZEIT.test(w.zeit) || typeof w.aktionsname !== 'string' || typeof w.ergebnis !== 'string') return rot(d.name, nr, 'form_falsch');
      }
      if (w.aktionsname === 'ausgabe.gesperrt' && (fas !== '2' || !geschlossen(w))) return rot(d.name, nr, 'form_falsch');
      if (w.aktionsname === 'kanal.abgewiesen' && (fas !== '2' || !geschlossenAbg(w))) return rot(d.name, nr, 'form_falsch');
      if (w.aktionsname === 'probelauf.zaehlung' && (fas !== '2' || !geschlossenProbe(w))) return rot(d.name, nr, 'form_falsch');
      if (ORD_ARTEN.includes(w.aktionsname) && (fas !== '2' || !geschlossenOrd(w))) return rot(d.name, nr, 'form_falsch');
      if (w.aktionsname === 'person.geloescht' && (fas !== '2' || !geschlossenGeloescht(w))) return rot(d.name, nr, 'form_falsch');
      if (w.aktionsname === 'person.auskunft' && (fas !== '2' || !geschlossenAuskunft(w))) return rot(d.name, nr, 'form_falsch');
      if (w.aktionsname === 'modellweg.aufruf' && (fas !== '2' || !geschlossenMw(w))) return rot(d.name, nr, 'form_falsch');
      if (ST_ARTEN.includes(w.aktionsname) && (fas !== '2' || !stelleForm(w))) return rot(d.name, nr, 'form_falsch');
      if (fas === '2' && !imRegister(w.aktionsname)) return rot(d.name, nr, 'form_falsch');
      prev = sha(roh);
      n += 1;
      inDatei += 1;
      if (inDatei === 1) werte.von = sekunden(w.zeit);
      werte.bis = sekunden(w.zeit);
      werte.letzte = prev;
      letzteDatei = d.name;
      letzteNr = nr;
    }
    const b = nach?.(d, werte, index);
    if (b) return rot(b.datei, b.zeile, b.grund);
  }
  if (letzte !== null && (n === 0 || prev !== letzte)) return rot(letzteDatei, letzteNr, 'letzte_falsch');
  return { bruch: null, letzte: n > 0 ? prev : null, ok: true, zeilen: n };
}

// Aufruf wie der Befehl: gibt { exit, stdout }.
function befehl(argv, dateien) {
  const fehlerAufruf = { exit: 64, stdout: '' };
  if (argv[0] !== 'pruefe') return fehlerAufruf;
  const opt = { vorher: NULL64, letzte: null };
  const pfade = [];
  for (let i = 1; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--vorher' || a === '--letzte') {
      const v = argv[i + 1];
      if (v === undefined || !HEX.test(v)) return fehlerAufruf;
      opt[a.slice(2)] = v;
      i += 1;
    } else if (a.startsWith('--')) {
      return fehlerAufruf;
    } else {
      pfade.push(a);
    }
  }
  if (pfade.length === 0) return fehlerAufruf;
  const gewaehlt = pfade.map((p) => {
    const d = dateien.find((x) => `@${x.name}` === p);
    if (!d) throw new Error(`Aufruf nennt ${p}, das keine Datei des Falls ist`);
    return d;
  });
  const erg = pruefe(gewaehlt, opt);
  return { exit: erg.ok ? 0 : 1, stdout: `${jcs(erg)}\n` };
}

// --- Monate, Anker und Wechsel (Abschnitt 5), unabhaengig vom Rust-Bau ---
// Ein Ordner ist hier ein Objekt { dateiname: inhalt als Text }. Die Modelle arbeiten auf einer Kopie im Speicher.

const monatMuster = (quelle) => new RegExp(`^${quelle}-([0-9]{4}-(?:0[1-9]|1[0-2]))\\.jsonl$`);
const zeilenVon = (text) => (text.length === 0 ? [] : text.slice(0, -1).split('\n'));
const ankerName = (quelle) => `${quelle}.anker.jsonl`;

function versuch(text) {
  try {
    const w = JSON.parse(text);
    return w !== null && typeof w === 'object' && !Array.isArray(w) ? w : null;
  } catch { return null; }
}

// Werte des Ankers aus einer Monatsdatei (Text, endet auf LF, nicht leer); null fuer unlesbar.
function ankerWerte(name, quelle, inhalt) {
  const z = zeilenVon(inhalt);
  const lesbar = (t) => (Buffer.byteLength(t) <= ZEILE_MAX ? versuch(t) : null);
  const erste = lesbar(z[0]);
  const letzteW = lesbar(z[z.length - 1]);
  return {
    abschnitt: name,
    bis: letzteW ? sekunden(letzteW.zeit) : null,
    fassung: 2,
    letzte: sha(z[z.length - 1]),
    monat: name.match(monatMuster(quelle))[1],
    von: erste ? sekunden(erste.zeit) : null,
    vorher_erste: erste && typeof erste.vorher === 'string' ? erste.vorher : null,
  };
}

function leseAnker(text) {
  if (text.length > 0 && !text.endsWith('\n')) throw fehler('letzte_zeile_unvollstaendig');
  return zeilenVon(text).map((l) => {
    const w = versuch(l);
    if (w === null || typeof w.abschnitt !== 'string') throw fehler('anker_falsch');
    return w;
  });
}

// Prueft die fertigen Zeilen eines Aufrufs so, wie es der Pruefer tut (Schritte von Abschnitt 3, als stuende jede in einer Datei brain-<Monat>.jsonl).
function pruefeFertig(zeilen, erwartet) {
  let prev = erwartet;
  for (const text of zeilen) {
    const r = pruefe([{ name: BRAIN, inhalt: `${text}\n` }], { vorher: prev });
    if (!r.ok) throw fehler(`zeile_ungueltig:${r.bruch.grund}`);
    prev = sha(text);
  }
}

// Der Schreiber auf einem Ordner im Speicher. baue(vorher) gibt die fertigen Zeilen; erwartet ist null oder ein sha256.
function schreibeMonat(ordner, quelle, ms, baue, erwartet) {
  const name = abschnitt(quelle, ms);
  const anker = ankerName(quelle);
  if (!(anker in ordner)) ordner[anker] = ''; // die Ankerdatei dient der Sperre und entsteht leer
  const eintraege = leseAnker(ordner[anker]);
  const muster = monatMuster(quelle);
  const dateien = Object.keys(ordner).filter((k) => muster.test(k)).sort();
  if (dateien.some((d) => d > name) || eintraege.some((a) => a.abschnitt === name)) throw fehler('zeit_rueckwaerts');
  const vorg = dateien.filter((d) => d < name && ordner[d].length > 0).pop() ?? null;
  const letzteVon = (d) => {
    if (!ordner[d].endsWith('\n')) throw fehler('letzte_zeile_unvollstaendig');
    const z = zeilenVon(ordner[d]);
    return z[z.length - 1];
  };
  const vorhanden = name in ordner;
  let vorher;
  let ankerNeu = null;
  if (vorhanden && ordner[name].length > 0) {
    if (vorg !== null && !eintraege.some((a) => a.abschnitt === vorg)) throw fehler('abschnitt_vorhanden');
    vorher = sha(letzteVon(name));
  } else if (vorg === null) {
    vorher = NULL64;
  } else {
    const hash = sha(letzteVon(vorg));
    const alt = eintraege.filter((a) => a.abschnitt === vorg);
    if (alt.length > 1 || (alt.length === 1 && alt[0].letzte !== hash)) throw fehler('anker_falsch');
    if (alt.length === 0) ankerNeu = jcs(ankerWerte(vorg, quelle, ordner[vorg]));
    vorher = hash;
  }
  if (erwartet !== null && erwartet !== vorher) throw fehler('vorher_abweichend');
  const zeilen = baue(vorher);
  if (ankerNeu !== null) ordner[anker] += `${ankerNeu}\n`;
  ordner[name] = (ordner[name] ?? '') + zeilen.map((z) => `${z}\n`).join('');
  return sha(zeilen[zeilen.length - 1]);
}

function gesperrtIn(ordner, quelle, e) {
  zeileGesperrt({ ...e, zeit_ms: T0 }, NULL64); // Felder vor Quelle vor Zeit
  abschnitt(quelle, e.zeit_ms);
  return schreibeMonat(ordner, quelle, e.zeit_ms, (vorher) => [zeileGesperrt(e, vorher)], null);
}

function zeileAllgemein(objekt, ms, vorher) {
  const o = { ...objekt, fassung: 2, vorher, zeit: zeitText(ms) };
  const z = jcs(o);
  if (o.aktionsname === 'ausgabe.gesperrt' && !geschlossen(o)) throw fehler('feld_ungueltig:zeile');
  if (o.aktionsname === 'kanal.abgewiesen' && !geschlossenAbg(o)) throw fehler('feld_ungueltig:zeile');
  if (o.aktionsname === 'probelauf.zaehlung' && !geschlossenProbe(o)) throw fehler('feld_ungueltig:zeile');
  if (ORD_ARTEN.includes(o.aktionsname) && !geschlossenOrd(o)) throw fehler('feld_ungueltig:zeile');
  if (o.aktionsname === 'person.geloescht' && !geschlossenGeloescht(o)) throw fehler('feld_ungueltig:zeile');
  if (o.aktionsname === 'person.auskunft' && !geschlossenAuskunft(o)) throw fehler('feld_ungueltig:zeile');
  if (ST_ARTEN.includes(o.aktionsname) && !stelleForm(o)) throw fehler('feld_ungueltig:zeile');
  if (!imRegister(o.aktionsname)) throw fehler('feld_ungueltig:zeile');
  if (Buffer.byteLength(z) > ZEILE_MAX) throw fehler('feld_ungueltig:zeile');
  return z;
}

function abgewiesenIn(ordner, quelle, e) {
  zeileAbgewiesen({ ...e, zeit_ms: T0 }, NULL64); // Felder vor Quelle vor Zeit
  const objekt = { aktionsname: 'kanal.abgewiesen', ergebnis: 'abgewiesen', grund: e.grund, kanal: e.kanal, route: e.route, sitzung: e.sitzung };
  return zeileIn(ordner, quelle, e.zeit_ms, objekt);
}

function probelaufIn(ordner, quelle, e) {
  zeileProbelauf({ ...e, zeit_ms: T0 }, NULL64); // Felder vor Quelle vor Zeit
  const objekt = { aktionsname: 'probelauf.zaehlung', ergebnis: 'an', programm: e.programm, sitzung: e.sitzung, anzahl: e.anzahl };
  return zeileIn(ordner, quelle, e.zeit_ms, objekt);
}

function ordnermarkeIn(ordner, quelle, e) {
  zeileOrdnermarke({ ...e, zeit_ms: T0 }, NULL64); // Felder vor Quelle vor Zeit
  const objekt = { aktionsname: e.aktionsname, ergebnis: 'ausgefuehrt', klasse_alt: e.klasse_alt, klasse_neu: e.klasse_neu, kunde_alt: e.kunde_alt, kunde_neu: e.kunde_neu, marke: e.marke };
  return zeileIn(ordner, quelle, e.zeit_ms, objekt);
}

function zeileIn(ordner, quelle, ms, objekt) {
  if (objekt === null || typeof objekt !== 'object' || Array.isArray(objekt)) throw fehler('feld_ungueltig:zeile');
  if (typeof objekt.aktionsname !== 'string') throw fehler('feld_ungueltig:aktionsname');
  if (typeof objekt.ergebnis !== 'string') throw fehler('feld_ungueltig:ergebnis');
  for (const f of ['fassung', 'vorher', 'zeit']) if (has(objekt, f)) throw fehler(`feld_ungueltig:${f}`);
  abschnitt(quelle, ms);
  zeileAllgemein(objekt, ms, NULL64);
  return schreibeMonat(ordner, quelle, ms, (vorher) => [zeileAllgemein(objekt, ms, vorher)], null);
}

function zeilenIn(ordner, quelle, erwartet, zeilen) {
  if (!Array.isArray(zeilen) || zeilen.length === 0) throw fehler('feld_ungueltig:zeilen');
  if (typeof erwartet !== 'string' || !HEX.test(erwartet)) throw fehler('feld_ungueltig:vorher');
  abschnitt(quelle, 0);
  pruefeFertig(zeilen, erwartet);
  const monate = zeilen.map((z) => {
    const ms = Date.parse(JSON.parse(z).zeit);
    return { ms, name: abschnitt(quelle, ms) };
  });
  if (monate.some((m) => m.name !== monate[0].name)) throw fehler('feld_ungueltig:zeit');
  return schreibeMonat(ordner, quelle, monate[0].ms, () => zeilen, erwartet);
}

// Die Pruefung eines Ordners (Abschnitt 5.4). dateien: Liste { name, inhalt | inhalt_b64 | fehlt }; ordnerName fuer den Fall, dass der Ordner fehlt.
function pruefeQuelle(dateien, ordnerName, quelle, { vorher = NULL64, letzte = null } = {}) {
  if (dateien === null) return { bruch: { datei: ordnerName, grund: 'datei_nicht_lesbar', zeile: null }, letzte: null, ok: false, zeilen: 0 };
  const rot = (datei, zeile, grund) => ({ bruch: { datei, grund, zeile }, letzte: null, ok: false, zeilen: 0 });
  const muster = monatMuster(quelle);
  const vorhanden = dateien.filter((d) => !d.fehlt);
  const monate = vorhanden.filter((d) => muster.test(d.name)).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const aname = ankerName(quelle);
  const anker = new Map();
  const adatei = vorhanden.find((d) => d.name === aname);
  if (adatei) {
    const buf = bytesVon(adatei);
    const stuecke = [];
    let start = 0;
    for (let i = 0; i < buf.length; i += 1) if (buf[i] === 0x0a) { stuecke.push(buf.subarray(start, i)); start = i + 1; }
    const unvollstaendig = start < buf.length;
    if (unvollstaendig) stuecke.push(buf.subarray(start));
    for (let i = 0; i < stuecke.length; i += 1) {
      const nr = i + 1;
      const roh = stuecke[i];
      if (unvollstaendig && i === stuecke.length - 1) return rot(aname, nr, 'letzte_zeile_unvollstaendig');
      if (roh.length === 0 || roh.length > ZEILE_MAX) return rot(aname, nr, 'zeile_unlesbar');
      let text;
      let w;
      try { text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(roh); w = JSON.parse(text); } catch { return rot(aname, nr, 'zeile_unlesbar'); }
      if (w === null || typeof w !== 'object' || Array.isArray(w) || einsamesSurrogat(w)) return rot(aname, nr, 'zeile_unlesbar');
      if (typeof w.abschnitt !== 'string' || !muster.test(w.abschnitt)) return rot(aname, nr, 'anker_falsch');
      if (anker.has(w.abschnitt)) return rot(aname, nr, 'anker_doppelt');
      if (!monate.some((m) => m.name === w.abschnitt)) return rot(aname, nr, 'anker_ohne_datei');
      anker.set(w.abschnitt, { nr, w, text });
    }
  }
  const nach = (d, werte, index) => {
    const a = anker.get(d.name);
    const schlecht = { datei: aname, zeile: a?.nr ?? null, grund: 'anker_falsch' };
    if (werte === null) return a ? schlecht : null;
    if (!a) return index < monate.length - 1 ? { datei: d.name, zeile: null, grund: 'anker_fehlt' } : null;
    const w = a.w;
    if (has(w, 'fassung') && (w.fassung !== 2 || jcs(w) !== a.text)) return schlecht;
    const soll = { abschnitt: d.name, bis: werte.bis, letzte: werte.letzte, monat: d.name.match(muster)[1], von: werte.von, vorher_erste: werte.erste_vorher };
    const rest = Object.keys(w).filter((k) => k !== 'fassung').sort();
    const sollKeys = Object.keys(soll).sort();
    if (rest.length !== sollKeys.length || rest.some((k, i) => k !== sollKeys[i])) return schlecht;
    return sollKeys.every((k) => w[k] === soll[k]) ? null : schlecht;
  };
  return pruefe(monate, { vorher, letzte, nach });
}

// Aufruf wie der Befehl `kette pruefe-quelle [--vorher H] [--letzte H] <ordner> <quelle>`; @@ ist der Ordner.
function befehlQuelle(argv, dateien, ordnerName) {
  const fehlerAufruf = { exit: 64, stdout: '' };
  if (argv[0] !== 'pruefe-quelle') return fehlerAufruf;
  const opt = { vorher: NULL64, letzte: null };
  const gesehen = new Set();
  const pos = [];
  for (let i = 1; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--vorher' || a === '--letzte') {
      const v = argv[i + 1];
      if (v === undefined || !HEX.test(v) || gesehen.has(a)) return fehlerAufruf;
      gesehen.add(a);
      opt[a.slice(2)] = v;
      i += 1;
    } else if (a.startsWith('--')) {
      return fehlerAufruf;
    } else {
      pos.push(a);
    }
  }
  if (pos.length !== 2 || !QUELLEN.includes(pos[1])) return fehlerAufruf;
  const erg = pruefeQuelle(pos[0] === '@@' ? dateien : null, pos[0] === '@@' ? ordnerName : pos[0].replace(/^@/, ''), pos[1], opt);
  return { exit: erg.ok ? 0 : 1, stdout: `${jcs(erg)}\n` };
}

// --- Daten ---

const lies = (n) => JSON.parse(readFileSync(join(VEK, n), 'utf8'));
const jcsVek = lies('jcs-vektoren.json');
const fassungVek = lies('fassung-vektoren.json');
const stelleVek = lies('stelle-vektoren.json');
const ketteVek = readFileSync(join(VEK, 'kette-vektoren.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));

const T0 = Date.UTC(2026, 10, 20, 13, 0, 0, 0);
const E1 = { zeit_ms: T0, route: 'cloud-beispiel', sitzung: 's-0001', grund: 'kundendaten-an-cloud', klasse: 'kundendaten', kunde: 'k-beispiel', anzahl: 1 };
const E2 = { zeit_ms: T0 + 1500, route: 'cloud-beispiel', sitzung: 's-0001', grund: 'klasse-ueber-route', klasse: 'vertraulich', kunde: null, anzahl: 3 };
const E3 = { zeit_ms: T0 + 61001, route: 'cloud-beispiel', sitzung: 's-0002', grund: 'einheit-ohne-klasse', klasse: null, kunde: null, anzahl: 2 };
const L1 = zeileGesperrt(E1, NULL64);
const L2 = zeileGesperrt(E2, sha(L1));
const L3 = zeileGesperrt(E3, sha(L2));
const BRAIN = 'brain-2026-11.jsonl';
const datei = (name, zeilen, ende = '\n') => ({ name, inhalt: zeilen.length ? zeilen.join('\n') + ende : '' });

// --- kanonisch ---

const kanonischFaelle = [
  ...jcsVek.faelle.map((f) => ({ fall: f.fall, quelle: 'jcs-vektoren.json', eingabe_text: f.eingabe_text, erwartet: f.rfc8785 })),
  ...fassungVek.faelle.map((f) => ({ fall: f.fall, quelle: 'fassung-vektoren.json', eingabe_text: f.eingabe_text, erwartet: f.fassung2 })),
  { fall: 'Sortierung nach UTF-16-Codeeinheiten (RFC 8785, 3.2.3)', quelle: 'jcs.mjs RFC_BEISPIELE', eingabe_text: RFC_BEISPIELE.sortierung.eingabe },
  { fall: 'Zahlen aus RFC 8785, Anhang B', quelle: 'jcs.mjs RFC_BEISPIELE', eingabe_text: '[-0,1e21,5e-324,0.000001,1e-7,9007199254740992]' },
  { fall: 'Zahlen: 5.0 wird 5, 1E2 wird 100, 1e20 bleibt ausgeschrieben, 0.1 bleibt', quelle: 'Ergaenzung', eingabe_text: '{"a":5.0,"b":1E2,"c":1e20,"d":0.1,"e":-1.5e-10,"f":123456789012345678901}' },
  { fall: 'Leerraum und Verschachtelung', quelle: 'Ergaenzung', eingabe_text: '{ "z" : [ 1 , { "b" : null , "a" : true } ] , "a" : "x" }' },
].map((f) => {
  const wert = jcs(JSON.parse(f.eingabe_text));
  if (f.erwartet !== undefined && f.erwartet !== wert) throw new Error(`kanonisch weicht ab: ${f.fall}`);
  return { ...f, erwartet: wert };
});

// --- zeile, zeit_text, abschnitt ---

const mit = (basis, aend) => ({ ...basis, ...aend });
const zeileFaelle = [];
const zeileFall = (fall, e, vorher) => {
  try {
    const z = zeileGesperrt(e, vorher);
    zeileFaelle.push({ fall, eingabe: e, vorher, erwartet: { zeile: z, sha256: sha(z) } });
  } catch (x) {
    if (!(x instanceof Fehler)) throw x;
    zeileFaelle.push({ fall, eingabe: e, vorher, erwartet: { fehler: x.message } });
  }
};
zeileFall('erste Zeile einer Kette, mit Kunde', E1, NULL64);
zeileFall('zweite Zeile, ohne Kunde', E2, sha(L1));
zeileFall('Klasse und Kunde null (einheit-ohne-klasse)', E3, sha(L2));
zeileFall('anzahl am oberen Rand', mit(E1, { anzahl: 4294967295 }), NULL64);
zeileFall('Schluessel mit 64 Zeichen und Punkt, Strich, Unterstrich', mit(E1, { route: `a.b-c_${'d'.repeat(58)}` }), NULL64);
zeileFall('Route mit Grossbuchstaben', mit(E1, { route: 'Cloud' }), NULL64);
zeileFall('Klasse leer', mit(E1, { klasse: '' }), NULL64);
zeileFall('Kunde mit Leerzeichen', mit(E1, { kunde: 'k beispiel' }), NULL64);
zeileFall('Sitzung mit 65 Zeichen', mit(E1, { sitzung: 's'.repeat(65) }), NULL64);
zeileFall('Grund als Satz', mit(E1, { grund: 'Kundendaten an Cloud' }), NULL64);
zeileFall('Schluessel beginnt mit Strich', mit(E1, { route: '-cloud' }), NULL64);
zeileFall('anzahl 0', mit(E1, { anzahl: 0 }), NULL64);
zeileFall('vorher keine 64 hex', E1, 'abc');
zeileFall('vorher mit Grossbuchstaben', E1, 'A'.repeat(64));
zeileFall('Zeit vor 1970', mit(E1, { zeit_ms: -1 }), NULL64);
zeileFall('Zeit nach 9999', mit(E1, { zeit_ms: MAX_MS + 1 }), NULL64);
zeileFall('zwei Fehler: der erste in der Reihenfolge zaehlt (route vor anzahl)', mit(E1, { route: 'X', anzahl: 0 }), NULL64);
zeileFall('zwei Fehler: Felder vor Zeit', mit(E1, { grund: '', zeit_ms: -1 }), NULL64);

const zeitFaelle = [0, MAX_MS, Date.UTC(2028, 1, 29, 12, 34, 56, 7), T0].map((ms) => ({ ms, erwartet: { text: zeitText(ms) } }));
zeitFaelle.push({ ms: -1, erwartet: { fehler: 'zeit_ungueltig' } }, { ms: MAX_MS + 1, erwartet: { fehler: 'zeit_ungueltig' } });

const abschnittFaelle = [
  ['brain', Date.UTC(2026, 10, 30, 23, 59, 59, 999)],
  ['brain', Date.UTC(2026, 11, 1, 0, 0, 0, 0)],
  ['stelle', Date.UTC(2027, 0, 15)],
  ['gateway', T0],
  ['halter', 0],
  ['audit', T0],
  ['Brain', T0],
  ['brain', -1],
].map(([quelle, ms]) => {
  try { return { quelle, ms, erwartet: { name: abschnitt(quelle, ms) } }; } catch (x) { return { quelle, ms, erwartet: { fehler: x.message } }; }
});

// --- anhaengen ---

const anhaengenFaelle = [];
const anhFall = (fall, anfang, aufrufe) => {
  let inhalt = anfang;
  const schritte = aufrufe.map((e) => {
    try {
      const r = anhaengen(inhalt, e);
      inhalt = r.inhalt;
      return { eingabe: e, erwartet: { sha256: r.hash } };
    } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      return { eingabe: e, erwartet: { fehler: x.message } };
    }
  });
  anhaengenFaelle.push({ fall, datei: BRAIN, anfang, aufrufe: schritte, ende: inhalt, rechte_neu_unix: anfang === null ? '0600' : null });
};
anhFall('neue Datei: erste Zeile mit 64 Nullen, Rechte 0600', null, [E1]);
anhFall('leere Datei wie neue', '', [E1]);
anhFall('drei Aufrufe nacheinander ergeben L1, L2, L3', null, [E1, E2, E3]);
anhFall('fortsetzen nach vorhandener Zeile', `${L1}\n`, [E2]);
anhFall('letzte Zeile ohne LF: nichts geschrieben', L1, [E2]);
anhFall('falsches Feld: nichts geschrieben, der naechste Aufruf gelingt', `${L1}\n`, [mit(E2, { route: 'X' }), E2]);
anhFall('unlesbare letzte Zeile: der Schreiber hasht nur Rohbytes', 'kaputt\n', [E1]);
const zweiSchreiber = {
  fall: 'zwei_schreiber',
  beschreibung: 'Zwei Schreiber mit je eigenem Datei-Handle (zwei Prozesse oder zwei Threads mit eigenem open) haengen gleichzeitig je 50 Zeilen an dieselbe neue Datei. Schreiber a nutzt sitzung s-a, Schreiber b sitzung s-b; Aufruf i (1 bis 50) hat anzahl i und zeit_ms basis + i. Die Reihenfolge der Zeilen ist offen.',
  datei: BRAIN,
  basis: { ...E1, zeit_ms: T0 },
  je_schreiber: 50,
  erwartet: { pruefe: { exit: 0, zeilen: 100 }, je_sitzung_anzahl: Array.from({ length: 50 }, (_, i) => i + 1) },
};

// --- pruefe ---

const pruefeFaelle = [];
const pFall = (fall, dateien, aufruf, absicht, ziel = pruefeFaelle) => {
  const erg = befehl(aufruf, dateien);
  if (erg.exit !== absicht.exit) throw new Error(`pruefe ${fall}: Exit ${erg.exit}, Absicht ${absicht.exit}: ${erg.stdout}`);
  if (absicht.exit !== 64) {
    const ist = JSON.parse(erg.stdout);
    for (const [k, v] of Object.entries(absicht)) {
      if (k === 'exit') continue;
      if (JSON.stringify(ist[k]) !== JSON.stringify(v)) throw new Error(`pruefe ${fall}: ${k} ist ${JSON.stringify(ist[k])}, Absicht ${JSON.stringify(v)}`);
    }
  }
  ziel.push({ fall, dateien, aufruf, erwartet: erg });
};
const ok = (zeilen) => ({ exit: 0, ok: true, zeilen, bruch: null });
const br = (datei, zeile, grund, zeilen) => ({ exit: 1, ok: false, zeilen, bruch: { datei, grund, zeile }, letzte: null });

pFall('drei gesperrt-Zeilen: gruen', [datei(BRAIN, [L1, L2, L3])], ['pruefe', `@${BRAIN}`], { ...ok(3), letzte: sha(L3) });
pFall('Klasse in Zeile 2 von Hand geaendert: rot in Zeile 3', [datei(BRAIN, [L1, L2.replace('"klasse":"vertraulich"', '"klasse":"intern"'), L3])], ['pruefe', `@${BRAIN}`], br(BRAIN, 3, 'kette_gebrochen', 2));
pFall('Leerzeichen in Zeile 2 eingefuegt: nicht kanonisch', [datei(BRAIN, [L1, L2.replace('"anzahl":3', '"anzahl": 3'), L3])], ['pruefe', `@${BRAIN}`], br(BRAIN, 2, 'nicht_kanonisch', 1));
pFall('Zeile 2 geloescht: rot in der neuen Zeile 2', [datei(BRAIN, [L1, L3])], ['pruefe', `@${BRAIN}`], br(BRAIN, 2, 'kette_gebrochen', 1));
const L2titel = jcs({ ...JSON.parse(L2), titel: 'geheim' });
pFall('zusaetzliches Feld titel, sonst richtig verkettet: Form falsch', [datei(BRAIN, [L1, L2titel])], ['pruefe', `@${BRAIN}`], br(BRAIN, 2, 'form_falsch', 1));
const L3geaendert = L3.replace('"anzahl":2', '"anzahl":9');
pFall('letzte Zeile geaendert, ohne --letzte: bleibt unentdeckt (gruen)', [datei(BRAIN, [L1, L2, L3geaendert])], ['pruefe', `@${BRAIN}`], { ...ok(3), letzte: sha(L3geaendert) });
pFall('letzte Zeile geaendert, mit --letzte: rot', [datei(BRAIN, [L1, L2, L3geaendert])], ['pruefe', '--letzte', sha(L3), `@${BRAIN}`], br(BRAIN, 3, 'letzte_falsch', 3));
pFall('--letzte stimmt: gruen', [datei(BRAIN, [L1, L2, L3])], ['pruefe', '--letzte', sha(L3), `@${BRAIN}`], { ...ok(3), letzte: sha(L3) });
pFall('letzte Zeile ohne LF', [datei(BRAIN, [L1, L2], '')], ['pruefe', `@${BRAIN}`], br(BRAIN, 2, 'letzte_zeile_unvollstaendig', 1));
const ohneFassung = jcs({ aktionsname: 'test.zeile', ergebnis: 'ok', vorher: NULL64, zeit: zeitText(T0) });
pFall('Zeile ohne fassung in brain-Datei', [datei(BRAIN, [ohneFassung])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'fassung_fehlt', 0));
pFall('dieselbe Zeile in einer gateway-Datei gilt als 1g: gruen', [datei('gateway-2026-11.jsonl', [ohneFassung])], ['pruefe', '@gateway-2026-11.jsonl'], { ...ok(1), letzte: sha(ohneFassung) });
const fassung3 = jcs({ ...JSON.parse(L1), fassung: 3 });
pFall('fassung 3', [datei(BRAIN, [fassung3])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
pFall('fassung 2.0: besteht den Fassungsschritt, ist aber nicht kanonisch', [datei(BRAIN, [L1.replace('"fassung":2', '"fassung":2.0')])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'nicht_kanonisch', 0));
const zeitOhneMs = jcs({ ...JSON.parse(L1), zeit: '2026-11-20T13:00:00Z' });
pFall('Zeit ohne Millisekunden', [datei(BRAIN, [zeitOhneMs])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
const routeGross = jcs({ ...JSON.parse(L1), route: 'Cloud' });
pFall('Route mit Grossbuchstaben in der Zeile', [datei(BRAIN, [routeGross])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
const anzahlNull = jcs({ ...JSON.parse(L1), anzahl: 0 });
pFall('anzahl 0 in der Zeile', [datei(BRAIN, [anzahlNull])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
pFall('doppelter Schluessel klasse', [datei(BRAIN, [L1.replace('"klasse":"kundendaten"', '"klasse":"intern","klasse":"kundendaten"')])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'nicht_kanonisch', 0));
const roh1 = Buffer.from(`${L1}\n`, 'utf8');
const kaputt = Buffer.concat([roh1.subarray(0, 20), Buffer.from([0xff]), roh1.subarray(21)]);
pFall('kein UTF-8', [{ name: BRAIN, inhalt_b64: kaputt.toString('base64') }], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'zeile_unlesbar', 0));
pFall('BOM vor der ersten Zeile', [{ name: BRAIN, inhalt_b64: Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), roh1]).toString('base64') }], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'zeile_unlesbar', 0));
pFall('leere Zeile in der Mitte', [{ name: BRAIN, inhalt: `${L1}\n\n${L2}\n` }], ['pruefe', `@${BRAIN}`], br(BRAIN, 2, 'zeile_unlesbar', 1));
pFall('Zeile ist ein Feld, kein Objekt', [{ name: BRAIN, inhalt: '[1,2]\n' }], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'zeile_unlesbar', 0));

// Die Zeile modellweg.aufruf (Abschnitt 6.7): Vertrag 8 baut sie, hier steht nur die Form, die der Pruefer verlangt.
const GATE = 'gateway-2026-11.jsonl';
const MW_BASIS_1 = { aktionsname: 'modellweg.aufruf', betriebsart: 'openai-lokal', ergebnis: 'abgelehnt', fassung: 2, grund: 'mw.pfad_unbekannt', host: null,
  modell: 'lokal-beispiel/modell-a', pfad: '/v1/models', route: 'lokal-beispiel', sitzung: 's-2', status: 404, token_aus: null, token_ein: null, vorher: NULL64, zeit: zeitText(T0) };
const MWZ1 = jcs(MW_BASIS_1);
const MWZ2 = jcs({ aktionsname: 'modellweg.aufruf', betriebsart: 'kopf-einsetzen', ergebnis: 'weitergereicht', fassung: 2, grund: null, host: 'api.beispiel.invalid',
  kopf: 'ersetzt', methode: 'GET', pfad: '/models/session', pfadregel: '/models/*', route: 'cloud-beispiel', sitzung: 's-1', status: 200, vorher: sha(MWZ1), zeit: zeitText(T0 + 1000), ziel: 'ziel-1' });
pFall('zwei modellweg-Zeilen beider Betriebsarten: gruen', [datei(GATE, [MWZ1, MWZ2])], ['pruefe', `@${GATE}`], { ...ok(2), letzte: sha(MWZ2) });
const MW_ALT = { ...MW_BASIS_1 };
delete MW_ALT.pfad; delete MW_ALT.host;
const MWALT = jcs(MW_ALT);
pFall('aeltere Zeile ohne pfad und host: gruen', [datei(GATE, [MWALT])], ['pruefe', `@${GATE}`], { ...ok(1), letzte: sha(MWALT) });
const MW_HALB = { ...MW_BASIS_1 };
delete MW_HALB.host;
pFall('nur pfad, host fehlt: Form falsch', [datei(GATE, [jcs(MW_HALB)])], ['pruefe', `@${GATE}`], br(GATE, 1, 'form_falsch', 0));
pFall('pfad mit Abfrage: Form falsch', [datei(GATE, [jcs({ ...MW_BASIS_1, pfad: '/v1/models?a=b' })])], ['pruefe', `@${GATE}`], br(GATE, 1, 'form_falsch', 0));
pFall('zusaetzliches Feld in der modellweg-Zeile: Form falsch', [datei(GATE, [jcs({ ...MW_BASIS_1, titel: 'geheim' })])], ['pruefe', `@${GATE}`], br(GATE, 1, 'form_falsch', 0));
const MW_BASIS_2 = { aktionsname: 'modellweg.aufruf', betriebsart: 'kopf-einsetzen', ergebnis: 'abgelehnt', fassung: 2, grund: 'mw.ziel_nicht_erlaubt', host: null,
  kopf: null, methode: null, pfad: null, pfadregel: null, route: 'cloud-beispiel', sitzung: 's-1', status: 403, vorher: NULL64, zeit: zeitText(T0), ziel: null };
pFall('host mit Port: Form falsch', [datei(GATE, [jcs({ ...MW_BASIS_2, host: 'api.beispiel.invalid:443' })])], ['pruefe', `@${GATE}`], br(GATE, 1, 'form_falsch', 0));
// kopf: halter (Nachtrag 25.11.2026) steht im Register; ein Wert ausserhalb des Registers bleibt abgewiesen.
const MW_HALTER = jcs({ ...JSON.parse(MWZ2), kopf: 'halter', vorher: NULL64 });
pFall('kopf halter auf dem Halterweg: gruen', [datei(GATE, [MW_HALTER])], ['pruefe', `@${GATE}`], { ...ok(1), letzte: sha(MW_HALTER) });
pFall('kopf ausserhalb des Registers: Form falsch', [datei(GATE, [jcs({ ...JSON.parse(MWZ2), kopf: 'halters', vorher: NULL64 })])], ['pruefe', `@${GATE}`], br(GATE, 1, 'form_falsch', 0));
// Register (Abschnitt 3): fail closed fuer die Fassung 2, kein Register fuer die Fassungen 1g und 1s.
{
  const frei = (art, zusatz = {}) => jcs({ aktionsname: art, ergebnis: 'ok', fassung: 2, vorher: NULL64, zeit: zeitText(T0), ...zusatz });
  const unbekannt = frei('mcp.schreibaufrufe', { vorschau: 'Inhalt der Notiz' });
  pFall('Fassung 2 mit einer Art ausserhalb des Registers: Form falsch', [datei(BRAIN, [unbekannt])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
  pFall('Fassung 2 mit einem Tippfehler in einer bekannten Art: Form falsch', [datei(BRAIN, [frei('ausgabe.gesperrte')])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'form_falsch', 0));
  const ohneF = (() => { const w = JSON.parse(unbekannt); delete w.fassung; return jcs(w); })();
  pFall('dieselbe Zeile ohne fassung in einer gateway-Datei (1g): nur Kettenglied, gruen', [datei(GATE, [ohneF])], ['pruefe', `@${GATE}`], { ...ok(1), letzte: sha(ohneF) });
  pFall('dieselbe Zeile ohne fassung in einer brain-Datei: fassung fehlt', [datei(BRAIN, [ohneF])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'fassung_fehlt', 0));
  // Je eine Zeile der angemeldeten Arten: Vertrag 5 nennt sie, ihre Form prueft noch kein Arm.
  for (const art of REG_ANGEMELDET) {
    const z = frei(art, { feld: 'wert' });
    pFall(`angemeldete Art ${art} mit freier Form: gruen`, [datei(BRAIN, [z])], ['pruefe', `@${BRAIN}`], { ...ok(1), letzte: sha(z) });
  }
  // Je eine Zeile der Arten, deren Form ein anderer Vertrag festlegt.
  for (const art of ['modell.aufruf', 'wissen.geschrieben', 'protokoll.einsicht']) {
    const z = frei(art);
    pFall(`fremde Art ${art} aus dem Register: gruen`, [datei(BRAIN, [z])], ['pruefe', `@${BRAIN}`], { ...ok(1), letzte: sha(z) });
  }
}
pFall('leere Datei: gruen mit null Zeilen', [datei(BRAIN, [])], ['pruefe', `@${BRAIN}`], { ...ok(0), letzte: null });
pFall('Datei fehlt', [{ name: BRAIN, fehlt: true }], ['pruefe', `@${BRAIN}`], br(BRAIN, null, 'datei_nicht_lesbar', 0));
pFall('zwei Dateien hintereinander als eine Kette', [datei('brain-2026-10.jsonl', [L1]), datei(BRAIN, [L2, L3])], ['pruefe', '@brain-2026-10.jsonl', `@${BRAIN}`], { ...ok(3), letzte: sha(L3) });
pFall('Datei beginnt mitten in der Kette, ohne --vorher', [datei(BRAIN, [L2, L3])], ['pruefe', `@${BRAIN}`], br(BRAIN, 1, 'kette_gebrochen', 0));
pFall('Datei beginnt mitten in der Kette, mit --vorher', [datei(BRAIN, [L2, L3])], ['pruefe', '--vorher', sha(L1), `@${BRAIN}`], { ...ok(2), letzte: sha(L3) });

const k1 = ketteVek.filter((v) => v.art === 'glied' && v.kette === 'k1');
const G10 = 'gateway-2026-10.jsonl';
pFall('kette-vektoren k1, Zeilen 1 bis 4 (Fassung 1g): gruen', [datei(G10, k1.slice(0, 4).map((v) => v.zeile))], ['pruefe', `@${G10}`], { ...ok(4), letzte: k1[3].hash });
pFall('kette-vektoren k1, alle fuenf: rot am angekuendigten Bruch', [datei(G10, k1.map((v) => v.zeile))], ['pruefe', `@${G10}`], br(G10, 5, 'angekuendigt:letzte_zeile_unvollstaendig', 4));
const k2 = ketteVek.find((v) => v.art === 'glied' && v.kette === 'k2');
pFall('kette-vektoren k2: Bruch ohne vorher', [datei(G10, [k2.zeile])], ['pruefe', `@${G10}`], br(G10, 1, 'angekuendigt:letzte_zeile_unlesbar', 0));
const gm = fassungVek.gemischte_kette.zeilen;
pFall('gemischte Kette 1g -> 2 aus fassung-vektoren.json: gruen', [datei('gateway-2026-11.jsonl', gm.map((z) => z.zeile))], ['pruefe', '@gateway-2026-11.jsonl'], { ...ok(gm.length), letzte: sha(gm[gm.length - 1].zeile) });
const sm = stelleVek.gemischte_kette.zeilen;
const smAlt = sm.filter((z) => z.datei === 'audit.jsonl');
const smNeu = sm.filter((z) => z.datei === 'stelle-2026-12.jsonl');
const smStart = JSON.parse(smAlt[0].zeile).vorher;
pFall('gemischte Kette 1s -> 2 aus stelle-vektoren.json ueber zwei Dateien: gruen', [datei('audit.jsonl', smAlt.map((z) => z.zeile)), datei('stelle-2026-12.jsonl', smNeu.map((z) => z.zeile))], ['pruefe', '--vorher', smStart, '@audit.jsonl', '@stelle-2026-12.jsonl'], { ...ok(sm.length), letzte: sm[sm.length - 1].hash });
pFall('Zeile 1s in einer stelle-Datei: fassung fehlt', [datei('stelle-2026-12.jsonl', smAlt.map((z) => z.zeile))], ['pruefe', '--vorher', smStart, '@stelle-2026-12.jsonl'], br('stelle-2026-12.jsonl', 1, 'fassung_fehlt', 0));

const eine = [datei(BRAIN, [L1])];
pFall('Aufruf ohne Unterbefehl', eine, [], { exit: 64 });
pFall('unbekannter Unterbefehl', eine, ['zeige', `@${BRAIN}`], { exit: 64 });
pFall('keine Datei', eine, ['pruefe'], { exit: 64 });
pFall('unbekannter Schalter', eine, ['pruefe', '--alles', `@${BRAIN}`], { exit: 64 });
pFall('--vorher zu kurz', eine, ['pruefe', '--vorher', 'abc', `@${BRAIN}`], { exit: 64 });
pFall('--vorher in Grossbuchstaben', eine, ['pruefe', '--vorher', 'A'.repeat(64), `@${BRAIN}`], { exit: 64 });
pFall('--letzte ohne Wert', eine, ['pruefe', '--letzte'], { exit: 64 });

// --- Fälle: Monate und Anker (Abschnitt 5) ---

const NOV = Date.UTC(2026, 10, 15, 12, 0, 0, 0);
const DEZ = Date.UTC(2026, 11, 1, 0, 0, 0, 0);
const JAN = Date.UTC(2027, 0, 2, 8, 30, 0, 0);
const ev = (ms, n, extra = {}) => ({ ...E1, zeit_ms: ms, anzahl: n, ...extra });
// Zeilen einer Kette ab vorher; gibt { zeilen, letzte }.
function kettenStueck(ereignisse, vorher = NULL64) {
  const zeilen = [];
  let v = vorher;
  for (const e of ereignisse) { const z = zeileGesperrt(e, v); zeilen.push(z); v = sha(z); }
  return { zeilen, letzte: v };
}
const text = (zeilen) => zeilen.map((z) => `${z}\n`).join('');
const monat = (q, ms) => abschnitt(q, ms);
const kopie = (o) => ({ ...o });

const novZ = kettenStueck([ev(NOV, 1), ev(NOV + 1000, 2)]);
const NOVD = monat('brain', NOV);
const DEZD = monat('brain', DEZ);
const OKTD = monat('brain', Date.UTC(2026, 9, 20));
const A = 'brain.anker.jsonl';

const monateFaelle = [];
// aufrufe: { art: 'gesperrt'|'zeile'|'zeilen', quelle, ... }; die Spalte "absicht" ist ein Feld je Aufruf (Code oder 'ok').
function mFall(fall, anfang, aufrufe, quellenGeprueft = ['brain']) {
  const ordner = kopie(anfang);
  const schritte = aufrufe.map(({ absicht, ...a }) => {
    let erg;
    try {
      let hash;
      if (a.art === 'gesperrt') hash = gesperrtIn(ordner, a.quelle, a.eingabe);
      else if (a.art === 'zeile') hash = zeileIn(ordner, a.quelle, a.zeit_ms, a.objekt);
      else if (a.art === 'abgewiesen') hash = abgewiesenIn(ordner, a.quelle, a.eingabe);
      else if (a.art === 'probelauf') hash = probelaufIn(ordner, a.quelle, a.eingabe);
      else if (a.art === 'ordnermarke') hash = ordnermarkeIn(ordner, a.quelle, a.eingabe);
      else hash = zeilenIn(ordner, a.quelle, a.erwartet_vorher, a.zeilen);
      erg = { sha256: hash };
    } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      erg = { fehler: x.message };
    }
    const ist = erg.fehler ?? 'ok';
    if (ist !== absicht) throw new Error(`monate ${fall}: ${ist}, Absicht ${absicht}`);
    return { ...a, erwartet: erg };
  });
  const pruefungen = {};
  for (const q of quellenGeprueft) {
    const dateien = Object.entries(ordner).map(([name, inhalt]) => ({ name, inhalt }));
    pruefungen[q] = befehlQuelle(['pruefe-quelle', '@@', q], dateien, 'ordner');
  }
  monateFaelle.push({ fall, anfang, aufrufe: schritte, ende: ordner, pruefe_quelle: pruefungen });
  return ordner;
}
const g = (ev1, absicht = 'ok', quelle = 'brain') => ({ art: 'gesperrt', quelle, eingabe: ev1, absicht });

// 1. Wechsel November/Dezember: der Anker des Novembers entsteht genau einmal, die erste Dezemberzeile traegt den sha256 der letzten Novemberzeile.
{
  const ende = mFall('Wechsel November nach Dezember mit Anker', { [NOVD]: text(novZ.zeilen) }, [g(ev(DEZ, 3)), g(ev(DEZ + 5, 4))]);
  const soll = jcs({ abschnitt: NOVD, bis: (NOV + 1000) / 1000, fassung: 2, letzte: novZ.letzte, monat: '2026-11', von: NOV / 1000, vorher_erste: NULL64 });
  if (ende[A] !== `${soll}\n`) throw new Error('Anker des Novembers weicht von der Absicht ab');
  const dez = zeilenVon(ende[DEZD]);
  if (dez.length !== 2 || JSON.parse(dez[0]).vorher !== novZ.letzte) throw new Error('erste Dezemberzeile traegt nicht die letzte Novemberzeile');
}
// 2. Zwei Wechsel hintereinander, auch ueber das Jahr; leere Ordner-Anfaenge.
{
  const ende = mFall('zwei Wechsel Oktober, November, Dezember', { [OKTD]: text(kettenStueck([ev(Date.UTC(2026, 9, 20), 1)]).zeilen) }, [g(ev(NOV, 2)), g(ev(NOV + 1, 3)), g(ev(DEZ + 1, 4))]);
  if (zeilenVon(ende[A]).length !== 2) throw new Error('zwei Wechsel brauchen zwei Anker');
}
mFall('Wechsel ueber den Jahreswechsel', { [DEZD]: text(kettenStueck([ev(DEZ + 99, 1)]).zeilen) }, [g(ev(JAN, 2))]);
mFall('leere Novemberdatei zwischen Oktober und Dezember: der Vorgaenger ist der Oktober', { [OKTD]: text(kettenStueck([ev(Date.UTC(2026, 9, 20), 1)]).zeilen), [NOVD]: '' }, [g(ev(DEZ + 7, 2))]);
mFall('stiller Monat: Oktober, dann Dezember (kein November)', { [OKTD]: text(kettenStueck([ev(Date.UTC(2026, 9, 20), 1)]).zeilen) }, [g(ev(DEZ + 7, 2))]);
mFall('leerer Ordner: erste Zeile mit 64 Nullen, die Ankerdatei entsteht leer', {}, [g(ev(NOV, 1))]);
// 3. Die Datei des neuen Monats gibt es schon, der Vormonat ist nicht geschlossen: Fehler, nichts geschrieben.
{
  const dezFremd = kettenStueck([ev(DEZ + 1, 9)], novZ.letzte);
  mFall('neue Monatsdatei existiert schon, Vormonat ohne Anker', { [NOVD]: text(novZ.zeilen), [DEZD]: text(dezFremd.zeilen), [A]: '' }, [g(ev(DEZ + 5, 3), 'abschnitt_vorhanden')]);
  const ank = jcs(ankerWerte(NOVD, 'brain', text(novZ.zeilen)));
  mFall('laufender Dezember mit Anker des Novembers: fortgesetzt (Neustart)', { [NOVD]: text(novZ.zeilen), [DEZD]: text(dezFremd.zeilen), [A]: `${ank}\n` }, [g(ev(DEZ + 5, 3))]);
  mFall('Anker geschrieben, Dezember noch nicht angelegt: kein zweiter Anker', { [NOVD]: text(novZ.zeilen), [A]: `${ank}\n` }, [g(ev(DEZ, 3))]);
  mFall('leere Dezemberdatei nach dem Anker: die Kette laeuft vom November weiter', { [NOVD]: text(novZ.zeilen), [DEZD]: '', [A]: `${ank}\n` }, [g(ev(DEZ, 3))]);
  mFall('Anker des Novembers stimmt nicht mit der Datei: Fehler, nichts geschrieben', { [NOVD]: text(novZ.zeilen), [A]: `${ank.replace(novZ.letzte, '1'.repeat(64))}\n` }, [g(ev(DEZ, 3), 'anker_falsch')]);
  mFall('zwei Anker fuer denselben Monat', { [NOVD]: text(novZ.zeilen), [A]: `${ank}\n${ank}\n` }, [g(ev(DEZ, 3), 'anker_falsch')]);
  mFall('Ankerdatei mit unlesbarer Zeile', { [NOVD]: text(novZ.zeilen), [A]: 'kaputt\n' }, [g(ev(NOV + 5, 3), 'anker_falsch')]);
  mFall('Ankerdatei ohne LF am Ende', { [NOVD]: text(novZ.zeilen), [A]: ank }, [g(ev(NOV + 5, 3), 'letzte_zeile_unvollstaendig')]);
  // 4. Zeit rueckwaerts ueber eine Monatsgrenze.
  const dezZ = kettenStueck([ev(DEZ, 3)], novZ.letzte);
  const ende = mFall('Zeit rueckwaerts: November nach dem Wechsel (Anker da, Dezember da)', { [NOVD]: text(novZ.zeilen), [DEZD]: text(dezZ.zeilen), [A]: `${ank}\n` }, [g(ev(NOV + 500, 4), 'zeit_rueckwaerts'), g(ev(DEZ + 9, 5))]);
  if (zeilenVon(ende[DEZD]).length !== 2) throw new Error('nach dem Fehler muss der Dezember weiterlaufen');
  mFall('Zeit rueckwaerts: Oktober, den es nicht gibt, nach dem Wechsel', { [NOVD]: text(novZ.zeilen), [DEZD]: text(dezZ.zeilen), [A]: `${ank}\n` }, [g(ev(Date.UTC(2026, 9, 31), 4), 'zeit_rueckwaerts')]);
  mFall('Zeit rueckwaerts: Monat mit Anker, aber ohne Folgedatei', { [NOVD]: text(novZ.zeilen), [A]: `${ank}\n` }, [g(ev(NOV + 3, 4), 'zeit_rueckwaerts')]);
  mFall('Zeit innerhalb des Monats rueckwaerts ist erlaubt', { [NOVD]: text(novZ.zeilen) }, [g(ev(NOV - 1000, 3))]);
}
// 5. Unvollstaendige letzte Zeile des Vormonats: nichts geschrieben.
mFall('Wechsel: letzte Zeile des Novembers ohne LF', { [NOVD]: text(novZ.zeilen).slice(0, -1) }, [g(ev(DEZ, 3), 'letzte_zeile_unvollstaendig')]);
mFall('Fortsetzen: letzte Zeile des Novembers ohne LF', { [NOVD]: text(novZ.zeilen).slice(0, -1) }, [g(ev(NOV + 5, 3), 'letzte_zeile_unvollstaendig')]);
// 6. Quellen und Felder: Reihenfolge Felder, Quelle, Zeit.
mFall('zwei Quellen im selben Ordner, je eine Kette', {}, [g(ev(NOV, 1)), g(ev(NOV, 1), 'ok', 'stelle'), g(ev(DEZ, 2)), g(ev(DEZ, 2), 'ok', 'stelle')], ['brain', 'stelle', 'halter']);
mFall('falsche Quelle', {}, [g(ev(NOV, 1), 'feld_ungueltig:quelle', 'audit')]);
mFall('Feld vor Quelle: route falsch, Quelle falsch', {}, [g(ev(NOV, 1, { route: 'X' }), 'feld_ungueltig:route', 'audit')]);
mFall('Quelle vor Zeit: Quelle falsch, Zeit falsch', {}, [g(ev(-1, 1), 'feld_ungueltig:quelle', 'audit')]);
mFall('Zeit ausserhalb', {}, [g(ev(MAX_MS + 1, 1), 'zeit_ungueltig')]);
// 7. Allgemeines Anhaengen: ein Objekt, fassung und vorher setzt der Schreiber.
{
  // Die allgemeine Zeile traegt eine Art des Registers (Abschnitt 3). Die Schluessel stehen hier absichtlich
  // unsortiert; kanonisch sortiert sie. Die Art suche.rueckfall ist angemeldet, ihre Form prueft noch kein Arm.
  const o = { grund: 'keine_verbindung', aktionsname: 'suche.rueckfall', sitzung: 's-1', ergebnis: 'lexikalisch', route: 'cloud-beispiel' };
  const z = (extra, absicht = 'ok', ms = NOV) => ({ art: 'zeile', quelle: 'brain', zeit_ms: ms, objekt: extra, absicht });
  const ende = mFall('allgemeine Zeile: Wechsel mit Anker, Schluessel sortiert, fassung 2', { [NOVD]: text(novZ.zeilen) }, [z(o), z({ ...o, grund: 'frist' }, 'ok', DEZ)]);
  if (!zeilenVon(ende[NOVD])[2].startsWith('{"aktionsname":"suche.rueckfall","ergebnis":"lexikalisch","fassung":2,"grund":"keine_verbindung","route":"cloud-beispiel","sitzung":"s-1","vorher":')) throw new Error('allgemeine Zeile weicht ab');
  // Register, fail closed: dieselbe freie Form mit einem Namen ausserhalb des Registers schreibt der Schreiber nicht.
  mFall('allgemeine Zeile: Art ausserhalb des Registers', {}, [z({ aktionsname: 'test.zeile', ergebnis: 'ok', menge: 3, rest: { b: 1, a: [1.0, 'x'] } }, 'feld_ungueltig:zeile'), z({ aktionsname: 'mcp.schreibaufrufe', ergebnis: 'ok', vorschau: 'Inhalt' }, 'feld_ungueltig:zeile')]);
  mFall('allgemeine Zeile: Objekt ist keine Map', {}, [z([1], 'feld_ungueltig:zeile'), z(null, 'feld_ungueltig:zeile'), z('x', 'feld_ungueltig:zeile')]);
  mFall('allgemeine Zeile: aktionsname und ergebnis sind Text', {}, [z({ ergebnis: 'ok' }, 'feld_ungueltig:aktionsname'), z({ aktionsname: 1, ergebnis: 'ok' }, 'feld_ungueltig:aktionsname'), z({ aktionsname: 'a.b' }, 'feld_ungueltig:ergebnis'), z({ aktionsname: 'a.b', ergebnis: null }, 'feld_ungueltig:ergebnis')]);
  mFall('allgemeine Zeile: fassung, vorher und zeit setzt der Schreiber', {}, [z({ ...o, fassung: 2 }, 'feld_ungueltig:fassung'), z({ ...o, vorher: NULL64 }, 'feld_ungueltig:vorher'), z({ ...o, zeit: '2026-11-15T12:00:00.000Z' }, 'feld_ungueltig:zeit'), z({ ...o, vorher: 1, fassung: 3 }, 'feld_ungueltig:fassung')]);
  mFall('allgemeine Zeile: falsche Quelle und Zeit', {}, [{ ...z(o, 'feld_ungueltig:quelle'), quelle: 'audit' }, z(o, 'zeit_ungueltig', -1), { ...z(o, 'feld_ungueltig:quelle', -1), quelle: 'audit' }]);
  mFall('allgemeine Zeile: ausgabe.gesperrt nur in der geschlossenen Form', {}, [z({ aktionsname: 'ausgabe.gesperrt', ergebnis: 'gesperrt', titel: 'x' }, 'feld_ungueltig:zeile'), z((() => { const w = JSON.parse(L1); delete w.fassung; delete w.vorher; delete w.zeit; return w; })(), 'ok')]);
  // Die Grenze von 4194304 Bytes je Zeile prueft der Bau mit einem Test; eine Zeile dieser Laenge steht nicht in den Fallen (zu gross).
}
// 8. Mehrere fertig verkettete Zeilen in einem Aufruf mit erwartetem sha256 der letzten Zeile.
{
  const zl = (erwartet, zeilen, absicht = 'ok', quelle = 'brain') => ({ art: 'zeilen', quelle, erwartet_vorher: erwartet, zeilen, absicht });
  const stueck = kettenStueck([ev(NOV + 10, 3), ev(NOV + 20, 4), ev(NOV + 30, 5)], novZ.letzte);
  const ende = mFall('mehrere Zeilen in einem Aufruf an eine Kette', { [NOVD]: text(novZ.zeilen) }, [zl(novZ.letzte, stueck.zeilen)]);
  if (ende[NOVD] !== text([...novZ.zeilen, ...stueck.zeilen])) throw new Error('mehrere Zeilen: Inhalt weicht ab');
  mFall('mehrere Zeilen: erwarteter sha256 weicht ab, nichts geschrieben', { [NOVD]: text(novZ.zeilen) }, [zl(sha('anderes'), kettenStueck([ev(NOV + 10, 3)], sha('anderes')).zeilen, 'vorher_abweichend'), zl(novZ.letzte, stueck.zeilen)]);
  mFall('mehrere Zeilen in eine neue Kette (64 Nullen)', {}, [zl(NULL64, kettenStueck([ev(NOV, 1), ev(NOV + 1, 2)]).zeilen)]);
  mFall('mehrere Zeilen: 64 Nullen erwartet, Datei hat Zeilen', { [NOVD]: text(novZ.zeilen) }, [zl(NULL64, kettenStueck([ev(NOV + 10, 3)]).zeilen, 'vorher_abweichend')]);
  const dez = kettenStueck([ev(DEZ, 3), ev(DEZ + 5, 4)], novZ.letzte);
  const e2 = mFall('mehrere Zeilen im neuen Monat: Wechsel mit Anker, erwartet ist die letzte Zeile des Vormonats', { [NOVD]: text(novZ.zeilen) }, [zl(novZ.letzte, dez.zeilen)]);
  if (zeilenVon(e2[A]).length !== 1) throw new Error('Wechsel mit mehreren Zeilen: ein Anker');
  mFall('mehrere Zeilen im neuen Monat: erwartet weicht ab, kein Anker', { [NOVD]: text(novZ.zeilen) }, [zl(sha('x'), kettenStueck([ev(DEZ, 3)], sha('x')).zeilen, 'vorher_abweichend')]);
  const gemischt = kettenStueck([ev(NOV + 40, 3), ev(DEZ, 4)], novZ.letzte);
  mFall('mehrere Zeilen ueber zwei Monate: Fehler, nichts geschrieben', { [NOVD]: text(novZ.zeilen) }, [zl(novZ.letzte, gemischt.zeilen, 'feld_ungueltig:zeit')]);
  const kaputt = kettenStueck([ev(NOV + 10, 3), ev(NOV + 20, 4)], novZ.letzte).zeilen;
  mFall('mehrere Zeilen: Pruefung jeder Zeile wie im Pruefer', { [NOVD]: text(novZ.zeilen) }, [
    zl(novZ.letzte, [], 'feld_ungueltig:zeilen'),
    zl('abc', kaputt, 'feld_ungueltig:vorher'),
    zl(novZ.letzte, kaputt.map((x) => x.replace('"anzahl":3', '"anzahl": 3')), 'zeile_ungueltig:nicht_kanonisch'),
    zl(novZ.letzte, [kaputt[1], kaputt[0]], 'zeile_ungueltig:kette_gebrochen'),
    zl(novZ.letzte, [kaputt[0], kaputt[0]], 'zeile_ungueltig:kette_gebrochen'),
    zl(novZ.letzte, [jcs({ ...JSON.parse(kaputt[0]), fassung: 3 })], 'zeile_ungueltig:form_falsch'),
    zl(novZ.letzte, [(() => { const o = JSON.parse(kaputt[0]); delete o.fassung; return jcs(o); })()], 'zeile_ungueltig:fassung_fehlt'),
    zl(novZ.letzte, [jcs({ ...JSON.parse(kaputt[0]), titel: 'x', aktionsname: 'ausgabe.gesperrt' })], 'zeile_ungueltig:form_falsch'),
    zl(novZ.letzte, ['kaputt'], 'zeile_ungueltig:zeile_unlesbar'),
    zl(novZ.letzte, kaputt, 'feld_ungueltig:quelle', 'audit'),
    zl(novZ.letzte, kaputt),
  ]);
}
const zweiSchreiberGrenze = {
  fall: 'zwei_schreiber_monatsgrenze',
  beschreibung: 'Der Ordner enthaelt brain-2026-11.jsonl mit den zwei Zeilen von L1N, L2N. Zwei Schreiber mit je eigenem Handle (zwei Prozesse oder zwei Threads mit eigenem open) beginnen gleichzeitig und haengen je 50 Zeilen an, alle mit Zeit im Dezember (Schreiber a sitzung s-a, b sitzung s-b; Aufruf i von 1 bis 50 hat anzahl i und zeit_ms basis + i). Der erste, der die Sperre bekommt, wechselt den Monat; der zweite findet den Wechsel fertig vor. Das Ganze laeuft in 10 Runden mit je neuem Ordner.',
  anfang: { [NOVD]: text(novZ.zeilen) },
  basis: { ...E1, zeit_ms: DEZ },
  je_schreiber: 50,
  runden: 10,
  erwartet: { anker_zeilen: 1, anker: jcs(ankerWerte(NOVD, 'brain', text(novZ.zeilen))), pruefe_quelle: { exit: 0, zeilen: 102 }, je_sitzung_anzahl: Array.from({ length: 50 }, (_, i) => i + 1) },
};

// --- Prueffaelle ueber einen Ordner (Abschnitt 5.4) ---

const quelleFaelle = [];
const abweichungen = [];
const qDatei = (name, zeilen, ende = '\n') => datei(name, zeilen, ende);
function qFall(fall, dateien, aufruf, absicht) {
  const erg = befehlQuelle(aufruf, dateien.filter((d) => !d.fehlt || true), 'ordner');
  if (erg.exit !== absicht.exit) abweichungen.push(`pruefe-quelle ${fall}: Exit ${erg.exit}, Absicht ${absicht.exit}: ${erg.stdout}`);
  else if (absicht.exit !== 64) {
    const ist = JSON.parse(erg.stdout);
    for (const [k, v] of Object.entries(absicht)) {
      if (k === 'exit') continue;
      if (JSON.stringify(ist[k]) !== JSON.stringify(v)) abweichungen.push(`pruefe-quelle ${fall}: ${k} ist ${JSON.stringify(ist[k])}, Absicht ${JSON.stringify(v)}`);
    }
  }
  quelleFaelle.push({ fall, dateien, aufruf, erwartet: erg });
}
{
  const oktZ = kettenStueck([ev(Date.UTC(2026, 9, 20), 1), ev(Date.UTC(2026, 9, 21), 2)]);
  const novZ2 = kettenStueck([ev(NOV, 3), ev(NOV + 1000, 4)], oktZ.letzte);
  const dezZ2 = kettenStueck([ev(DEZ, 5), ev(DEZ + 1000, 6)], novZ2.letzte);
  const aOkt = jcs(ankerWerte(OKTD, 'brain', text(oktZ.zeilen)));
  const aNov = jcs(ankerWerte(NOVD, 'brain', text(novZ2.zeilen)));
  const q = ['pruefe-quelle', '@@', 'brain'];
  const alle = () => [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov])];
  const gruen = { exit: 0, ok: true, zeilen: 6, bruch: null, letzte: dezZ2.letzte };
  qFall('drei Monate mit zwei Ankern: gruen', alle(), q, gruen);
  qFall('Reihenfolge der Dateien im Ordner ist gleich', alle().reverse(), q, gruen);
  qFall('nur der laufende Monat, keine Ankerdatei: gruen', [datei(DEZD, kettenStueck([ev(DEZ, 5)]).zeilen)], q, { exit: 0, ok: true, zeilen: 1 });
  qFall('laufender Monat mit eigenem Anker (Wechsel unterbrochen): gruen', [datei(NOVD, novZ2.zeilen.slice(0, 0).concat(kettenStueck([ev(NOV, 3)]).zeilen)), datei(A, [jcs(ankerWerte(NOVD, 'brain', text(kettenStueck([ev(NOV, 3)]).zeilen)))])], q, { exit: 0, ok: true, zeilen: 1 });
  qFall('leerer Ordner: gruen mit null Zeilen', [], q, { exit: 0, ok: true, zeilen: 0, letzte: null });
  qFall('der Ordner fehlt', [], ['pruefe-quelle', '@weg', 'brain'], { exit: 1, ok: false, zeilen: 0, bruch: { datei: 'weg', grund: 'datei_nicht_lesbar', zeile: null } });
  qFall('stiller Monat (Oktober, Dezember): gruen', [datei(OKTD, oktZ.zeilen), datei(DEZD, kettenStueck([ev(DEZ, 5)], oktZ.letzte).zeilen), datei(A, [aOkt])], q, { exit: 0, ok: true, zeilen: 3 });
  qFall('Anker geaendert: letzte', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt.replace(oktZ.letzte, '2'.repeat(64)), aNov])], q, br(A, 1, 'anker_falsch', 2));
  for (const f of ['bis', 'von', 'vorher_erste', 'monat', 'abschnitt']) {
    const w = JSON.parse(aNov);
    w[f] = f === 'bis' || f === 'von' ? w[f] + 1 : f === 'monat' ? '2026-10' : f === 'abschnitt' ? DEZD : '3'.repeat(64);
    qFall(`Anker des Novembers: Feld ${f} geaendert`, [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, jcs(w)])], q, f === 'abschnitt' ? br(NOVD, null, 'anker_fehlt', 4) : br(A, 2, 'anker_falsch', 4));
  }
  const nullBis = JSON.parse(aNov); nullBis.von = null;
  qFall('Anker des Novembers: von null statt Zahl', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, jcs(nullBis)])], q, br(A, 2, 'anker_falsch', 4));
  const zusatz = { ...JSON.parse(aNov), titel: 'x' };
  qFall('Anker mit zusaetzlichem Feld', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, jcs(zusatz)])], q, br(A, 2, 'anker_falsch', 4));
  const ohneFeld = JSON.parse(aNov); delete ohneFeld.von;
  qFall('Anker ohne Feld von', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, jcs(ohneFeld)])], q, br(A, 2, 'anker_falsch', 4));
  qFall('Anker mit fassung 3', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, jcs({ ...JSON.parse(aNov), fassung: 3 })])], q, br(A, 2, 'anker_falsch', 4));
  qFall('Anker mit fassung 2 und Leerzeichen: nicht kanonisch', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov.replace('"monat"', ' "monat"')])], q, br(A, 2, 'anker_falsch', 4));
  const alt = JSON.parse(aNov); delete alt.fassung;
  qFall('Anker ohne fassung (Form 1g, Leerzeichen erlaubt): gilt', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, JSON.stringify(alt, null, 0).replace(',"', ', "')])], q, gruen);
  qFall('Monat November geloescht, Anker bleibt', [datei(OKTD, oktZ.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov])], q, br(A, 2, 'anker_ohne_datei', 0));
  qFall('Monat Oktober (zuerst) geloescht, Anker bleibt', [datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov])], q, br(A, 1, 'anker_ohne_datei', 0));
  qFall('Anker ohne Datei: geloeschter Monat und kein Anker (die Kette zeigt es)', [datei(OKTD, oktZ.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt])], q, br(DEZD, 1, 'kette_gebrochen', 2));
  qFall('Monat November geloescht samt Anker: die Kette zeigt es', [datei(OKTD, oktZ.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt])], q, br(DEZD, 1, 'kette_gebrochen', 2));
  qFall('geschlossener Monat ohne Anker', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt])], q, br(NOVD, null, 'anker_fehlt', 4));
  qFall('geschlossener Monat, die Ankerdatei fehlt ganz', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen)], q, br(OKTD, null, 'anker_fehlt', 2));
  qFall('zwei Anker fuer einen Monat', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(A, [aOkt, aOkt])], q, br(A, 2, 'anker_doppelt', 0));
  qFall('Anker nennt eine Datei einer anderen Quelle', [datei(OKTD, oktZ.zeilen), datei(A, [aOkt.replace('brain-', 'stelle-')])], q, br(A, 1, 'anker_falsch', 0));
  qFall('Anker nennt keinen Monatsnamen', [datei(OKTD, oktZ.zeilen), datei(A, [aOkt.replace(OKTD, 'brain-2026-13.jsonl')])], q, br(A, 1, 'anker_falsch', 0));
  qFall('Anker ohne Text abschnitt', [datei(OKTD, oktZ.zeilen), datei(A, [jcs({ ...JSON.parse(aOkt), abschnitt: 5 })])], q, br(A, 1, 'anker_falsch', 0));
  qFall('Anker auf eine leere Monatsdatei', [datei(OKTD, []), datei(NOVD, kettenStueck([ev(NOV, 3)]).zeilen), datei(A, [aOkt])], q, br(A, 1, 'anker_falsch', 0));
  qFall('leere Monatsdatei ohne Anker zwischen zwei Monaten: gruen', [datei(OKTD, oktZ.zeilen), datei(NOVD, []), datei(DEZD, kettenStueck([ev(DEZ, 3)], oktZ.letzte).zeilen), datei(A, [aOkt])], q, { exit: 0, ok: true, zeilen: 3 });
  // Aenderungen in einem geschlossenen Monat.
  const oktAend = (zeilen, i) => zeilen.map((z, k) => (k === i ? z.replace('"anzahl":2', '"anzahl":7').replace('"anzahl":1', '"anzahl":8') : z));
  qFall('geschlossener Monat: Zeile 1 geaendert, einfach: die Kette bricht in Zeile 2', [datei(OKTD, oktAend(oktZ.zeilen, 0)), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov])], q, br(OKTD, 2, 'kette_gebrochen', 1));
  qFall('geschlossener Monat: letzte Zeile geaendert, einfach: rot ueber den Anker', [datei(OKTD, oktAend(oktZ.zeilen, 1)), datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aOkt, aNov])], q, br(A, 1, 'anker_falsch', 2));
  // Neu verkettet: der Angreifer rechnet alle `vorher` danach neu aus; nur der Anker verraet es.
  const neuVerkettet = (aend, ab) => {
    const evs = [[ev(Date.UTC(2026, 9, 20), 1), ev(Date.UTC(2026, 9, 21), 2)], [ev(NOV, 3), ev(NOV + 1000, 4)], [ev(DEZ, 5), ev(DEZ + 1000, 6)]];
    aend(evs);
    let v = NULL64;
    return evs.map((m) => { const k = kettenStueck(m, v); v = k.letzte; return k.zeilen; });
  };
  const nv1 = neuVerkettet((e) => { e[0][0] = ev(Date.UTC(2026, 9, 20), 99); });
  qFall('geschlossener Monat: Zeile 1 geaendert und alles danach neu verkettet: rot ueber den Anker', [datei(OKTD, nv1[0]), datei(NOVD, nv1[1]), datei(DEZD, nv1[2]), datei(A, [aOkt, aNov])], q, br(A, 1, 'anker_falsch', 2));
  const nv2 = neuVerkettet((e) => { e[0][1] = ev(Date.UTC(2026, 9, 21), 99); });
  qFall('geschlossener Monat: letzte Zeile geaendert und alles danach neu verkettet: rot ueber den Anker', [datei(OKTD, nv2[0]), datei(NOVD, nv2[1]), datei(DEZD, nv2[2]), datei(A, [aOkt, aNov])], q, br(A, 1, 'anker_falsch', 2));
  const nv3 = neuVerkettet((e) => { e[1][1] = ev(NOV + 1000, 99); });
  qFall('zweiter geschlossener Monat neu verkettet: der erste Anker stimmt, der zweite nicht', [datei(OKTD, nv3[0]), datei(NOVD, nv3[1]), datei(DEZD, nv3[2]), datei(A, [aOkt, aNov])], q, br(A, 2, 'anker_falsch', 4));
  const nv4 = neuVerkettet((e) => { e[2][1] = ev(DEZ + 1000, 99); });
  qFall('laufender Monat neu verkettet ohne Anker: bleibt unentdeckt (gruen)', [datei(OKTD, nv4[0]), datei(NOVD, nv4[1]), datei(DEZD, nv4[2]), datei(A, [aOkt, aNov])], q, { exit: 0, ok: true, zeilen: 6 });
  qFall('laufender Monat letzte Zeile geaendert, mit --letzte: rot', [datei(OKTD, oktZ.zeilen), datei(NOVD, novZ2.zeilen), datei(DEZD, [dezZ2.zeilen[0], dezZ2.zeilen[1].replace('"anzahl":6', '"anzahl":9')]), datei(A, [aOkt, aNov])], ['pruefe-quelle', '--letzte', dezZ2.letzte, '@@', 'brain'], br(DEZD, 2, 'letzte_falsch', 6));
  qFall('--letzte stimmt', alle(), ['pruefe-quelle', '--letzte', dezZ2.letzte, '@@', 'brain'], gruen);
  qFall('--vorher fuer einen Ordner, der mitten in der Kette beginnt', [datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aNov])], ['pruefe-quelle', '--vorher', oktZ.letzte, '@@', 'brain'], { exit: 0, ok: true, zeilen: 4, letzte: dezZ2.letzte });
  qFall('ohne --vorher beginnt dieser Ordner mit einem Bruch', [datei(NOVD, novZ2.zeilen), datei(DEZD, dezZ2.zeilen), datei(A, [aNov])], q, br(NOVD, 1, 'kette_gebrochen', 0));
  qFall('Zeile im Monat geaendert, Pruefer meldet die Kette zuerst', [datei(NOVD, [novZ2.zeilen[0].replace('"anzahl":3', '"anzahl":9'), novZ2.zeilen[1]]), datei(DEZD, dezZ2.zeilen)], ['pruefe-quelle', '--vorher', oktZ.letzte, '@@', 'brain'], br(NOVD, 2, 'kette_gebrochen', 1));
  qFall('Monatsdatei beginnt mit einem Bruch im zweiten Monat', [datei(OKTD, oktZ.zeilen), datei(NOVD, kettenStueck([ev(NOV, 3)], NULL64).zeilen), datei(A, [aOkt])], q, br(NOVD, 1, 'kette_gebrochen', 2));
  // Ankerdatei selbst.
  qFall('Ankerdatei: unlesbare Zeile', [datei(OKTD, oktZ.zeilen), datei(A, ['kaputt'])], q, br(A, 1, 'zeile_unlesbar', 0));
  qFall('Ankerdatei: leere Zeile', [datei(OKTD, oktZ.zeilen), { name: A, inhalt: `${aOkt}\n\n` }], q, br(A, 2, 'zeile_unlesbar', 0));
  qFall('Ankerdatei: Zeile ist ein Feld, kein Objekt', [datei(OKTD, oktZ.zeilen), { name: A, inhalt: '[1]\n' }], q, br(A, 1, 'zeile_unlesbar', 0));
  qFall('Ankerdatei: letzte Zeile ohne LF', [datei(OKTD, oktZ.zeilen), datei(A, [aOkt], '')], q, br(A, 1, 'letzte_zeile_unvollstaendig', 0));
  qFall('Ankerdatei leer: wie keine Anker', [datei(OKTD, oktZ.zeilen), datei(A, [])], q, { exit: 0, ok: true, zeilen: 2 });
  // Namen im Ordner.
  qFall('Dateien anderer Quellen und fremde Namen zaehlen nicht', [datei(NOVD, novZ2.zeilen.slice(0, 0).concat(kettenStueck([ev(NOV, 3)]).zeilen)), datei('stelle-2026-11.jsonl', ['kaputt']), datei('brain-2026-11.jsonl.bak', ['kaputt']), datei('brain-2026-13.jsonl', ['kaputt']), datei('brain.jsonl', ['kaputt']), datei('stelle.anker.jsonl', ['kaputt'])], q, { exit: 0, ok: true, zeilen: 1 });
  qFall('Quelle stelle im selben Ordner', [datei(NOVD, ['kaputt']), datei('stelle-2026-11.jsonl', kettenStueck([ev(NOV, 3)]).zeilen)], ['pruefe-quelle', '@@', 'stelle'], { exit: 0, ok: true, zeilen: 1 });
  qFall('Monatsdatei fehlt in der Liste (fehlt: true) zaehlt nicht', [{ name: NOVD, fehlt: true }], q, { exit: 0, ok: true, zeilen: 0 });
  // Altes Gateway (Fassung 1g) und die Kette k1: ein Anker in Form 1g (kein fassung), Oktober und November.
  {
    const gateway1 = k1.slice(0, 2).map((v) => v.zeile);
    const gateway2 = k1.slice(2, 4).map((v) => v.zeile);
    const ankerAlt = ketteVek.find((v) => v.art === 'anker').anker;
    qFall('Gateway: Oktober (1g) mit altem Anker, November (1g) laufend', [datei(G10, gateway1), datei('gateway-2026-11.jsonl', gateway2), datei('gateway.anker.jsonl', [ankerAlt])], ['pruefe-quelle', '@@', 'gateway'], { exit: 0, ok: true, zeilen: 4, letzte: k1[3].hash });
    qFall('Gateway: der alte Anker passt nicht zur Datei', [datei(G10, gateway1), datei('gateway-2026-11.jsonl', gateway2), datei('gateway.anker.jsonl', [ankerAlt.replace('1793491198.5', '1793491198.6')])], ['pruefe-quelle', '@@', 'gateway'], br('gateway.anker.jsonl', 1, 'anker_falsch', 2));
  }
  // Aufrufe.
  const ein = [datei(NOVD, kettenStueck([ev(NOV, 3)]).zeilen)];
  qFall('Aufruf ohne Quelle', ein, ['pruefe-quelle', '@@'], { exit: 64 });
  qFall('Aufruf ohne Ordner und Quelle', ein, ['pruefe-quelle'], { exit: 64 });
  qFall('Aufruf mit drei Angaben', ein, ['pruefe-quelle', '@@', 'brain', 'stelle'], { exit: 64 });
  qFall('Aufruf mit unbekannter Quelle audit', ein, ['pruefe-quelle', '@@', 'audit'], { exit: 64 });
  qFall('Aufruf mit Quelle in Grossbuchstaben', ein, ['pruefe-quelle', '@@', 'Brain'], { exit: 64 });
  qFall('Aufruf mit unbekanntem Schalter', ein, ['pruefe-quelle', '--alles', '@@', 'brain'], { exit: 64 });
  qFall('Aufruf mit --vorher zu kurz', ein, ['pruefe-quelle', '--vorher', 'abc', '@@', 'brain'], { exit: 64 });
  qFall('Aufruf mit --vorher doppelt', ein, ['pruefe-quelle', '--vorher', NULL64, '--vorher', NULL64, '@@', 'brain'], { exit: 64 });
  qFall('Aufruf mit --letzte ohne Wert', ein, ['pruefe-quelle', '@@', 'brain', '--letzte'], { exit: 64 });
}

// --- Fälle: kanal.abgewiesen (Abschnitt 6) ---

const abgZeile = [];
const abgPruefe = [];
const abgAnhaengen = [];
const A1 = { zeit_ms: T0, grund: 'pk.sid_nicht_erlaubt', kanal: 'such', route: 'cloud-beispiel', sitzung: 's-1' };
const abgFall = (fall, e, vorher, absichtFehler = null) => {
  let erg;
  try {
    const z = zeileAbgewiesen(e, vorher);
    erg = { zeile: z, sha256: sha(z) };
  } catch (x) {
    if (!(x instanceof Fehler)) throw x;
    erg = { fehler: x.message };
  }
  if ((erg.fehler ?? null) !== absichtFehler) throw new Error(`abgewiesen ${fall}: ${erg.fehler ?? 'ok'}, Absicht ${absichtFehler ?? 'ok'}`);
  abgZeile.push({ fall, eingabe: e, vorher, erwartet: erg });
};
for (const grund of GRUENDE_PK) abgFall(`Grund ${grund}`, { ...A1, grund }, NULL64);
for (const grund of GRUENDE_SUCH) abgFall(`Grund ${grund} bei Kanal such`, { ...A1, grund, kanal: 'such' }, NULL64);
abgFall('pk.sid_nicht_erlaubt bei Kanal stelle', { ...A1, grund: 'pk.sid_nicht_erlaubt', kanal: 'stelle' }, NULL64);
abgFall('pk.hallo_fehlt bei Kanal weiterreicher', { ...A1, grund: 'pk.hallo_fehlt', kanal: 'weiterreicher' }, NULL64);
abgFall('such.sitzung_fremd bei Kanal stelle: abgelehnt', { ...A1, grund: 'such.sitzung_fremd', kanal: 'stelle' }, NULL64, 'feld_ungueltig:grund');
abgFall('such.dienst_stumm bei Kanal weiterreicher: abgelehnt', { ...A1, grund: 'such.dienst_stumm', kanal: 'weiterreicher' }, NULL64, 'feld_ungueltig:grund');
abgFall('such.kennung_fremd bei Kanal stelle: abgelehnt', { ...A1, grund: 'such.kennung_fremd', kanal: 'stelle' }, NULL64, 'feld_ungueltig:grund');
abgFall('such.hallo_fehlt bei Kanal such (nicht in der Liste): abgelehnt', { ...A1, grund: 'such.hallo_fehlt', kanal: 'such' }, NULL64, 'feld_ungueltig:grund');
abgFall('stelle.dienst_stumm bei Kanal stelle (stelle hat keine eigenen Gruende): abgelehnt', { ...A1, grund: 'stelle.dienst_stumm', kanal: 'stelle' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund mit Kanal als Vorsilbe bei einem Kanal ohne Gruende (x.y): abgelehnt', { ...A1, grund: 'x.y', kanal: 'x' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund such.* und Kanal ungueltig: der Grund zuerst', { ...A1, grund: 'such.sitzung_fremd', kanal: 'X' }, NULL64, 'feld_ungueltig:grund');
abgFall('Route und Sitzung null', { ...A1, route: null, sitzung: null }, NULL64);
abgFall('nur die Route null, Kanal weiterreicher', { ...A1, route: null, kanal: 'weiterreicher' }, sha('x'));
abgFall('nur die Sitzung null, Kanal stelle', { ...A1, sitzung: null, kanal: 'stelle' }, sha('x'));
abgFall('Schluessel mit 64 Zeichen', { ...A1, kanal: `a.b-c_${'d'.repeat(58)}`, sitzung: 's'.repeat(64) }, NULL64);
abgFall('Grund nicht aus der Liste', { ...A1, grund: 'pk.fremd' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund der frueheren Liste peer.sid_fremd', { ...A1, grund: 'peer.sid_fremd' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund leer', { ...A1, grund: '' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund mit Grossbuchstaben', { ...A1, grund: 'PK.sid_nicht_erlaubt' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund der anderen Zeilenart', { ...A1, grund: 'kundendaten-an-cloud' }, NULL64, 'feld_ungueltig:grund');
abgFall('Grund null', { ...A1, grund: null }, NULL64, 'feld_ungueltig:grund');
abgFall('Kanal mit Grossbuchstaben', { ...A1, kanal: 'Such' }, NULL64, 'feld_ungueltig:kanal');
abgFall('Kanal leer', { ...A1, kanal: '' }, NULL64, 'feld_ungueltig:kanal');
abgFall('Kanal null', { ...A1, kanal: null }, NULL64, 'feld_ungueltig:kanal');
abgFall('Route mit Grossbuchstaben', { ...A1, route: 'Cloud' }, NULL64, 'feld_ungueltig:route');
abgFall('Route leer', { ...A1, route: '' }, NULL64, 'feld_ungueltig:route');
abgFall('Sitzung mit Leerzeichen', { ...A1, sitzung: 's 1' }, NULL64, 'feld_ungueltig:sitzung');
abgFall('Sitzung mit 65 Zeichen', { ...A1, sitzung: 's'.repeat(65) }, NULL64, 'feld_ungueltig:sitzung');
abgFall('vorher keine 64 hex', A1, 'abc', 'feld_ungueltig:vorher');
abgFall('Zeit vor 1970', { ...A1, zeit_ms: -1 }, NULL64, 'zeit_ungueltig');
abgFall('zwei Fehler: Grund vor Kanal', { ...A1, grund: 'x', kanal: 'X' }, NULL64, 'feld_ungueltig:grund');
abgFall('zwei Fehler: Kanal vor Route', { ...A1, kanal: 'X', route: 'X' }, NULL64, 'feld_ungueltig:kanal');
abgFall('zwei Fehler: Route vor Sitzung', { ...A1, route: 'X', sitzung: 'X' }, NULL64, 'feld_ungueltig:route');
abgFall('zwei Fehler: Sitzung vor vorher', { ...A1, sitzung: 'X' }, 'abc', 'feld_ungueltig:sitzung');
abgFall('zwei Fehler: Felder vor Zeit', { ...A1, kanal: 'X', zeit_ms: -1 }, NULL64, 'feld_ungueltig:kanal');

{
  const e1 = { ...A1 };
  const e2 = { ...A1, zeit_ms: T0 + 1000, grund: 'pk.hallo_falsch', route: null, sitzung: null };
  const e3 = { ...A1, zeit_ms: T0 + 2000, grund: 'pk.gid_falsch', kanal: 'stelle' };
  const a1 = zeileAbgewiesen(e1, NULL64);
  const a2 = zeileAbgewiesen(e2, sha(a1));
  const a3 = zeileAbgewiesen(e3, sha(a2));
  const g2 = zeileGesperrt(E1, sha(a1));
  const a3g = zeileAbgewiesen(e3, sha(g2));
  const ok = (zeilen, letzte) => ({ exit: 0, ok: true, zeilen, bruch: null, letzte });
  const aFall = (fall, dateien, aufruf, absicht) => pFall(fall, dateien, aufruf, absicht, abgPruefe);
  const P = ['pruefe', `@${BRAIN}`];
  aFall('drei abgewiesen-Zeilen: gruen', [datei(BRAIN, [a1, a2, a3])], P, ok(3, sha(a3)));
  aFall('abgewiesen und gesperrt gemischt in einer Kette: gruen', [datei(BRAIN, [a1, g2, a3g])], P, ok(3, sha(a3g)));
  aFall('Grund in Zeile 2 von Hand geaendert: rot in Zeile 3', [datei(BRAIN, [a1, a2.replace('pk.hallo_falsch', 'pk.job_unbekannt'), a3])], P, br(BRAIN, 3, 'kette_gebrochen', 2));
  aFall('Leerzeichen eingefuegt: nicht kanonisch', [datei(BRAIN, [a1, a2.replace('"kanal":"such"', '"kanal": "such"')])], P, br(BRAIN, 2, 'nicht_kanonisch', 1));
  const mitW = (z, f) => { const o = JSON.parse(z); f(o); return jcs(o); };
  aFall('zusaetzliches Feld sid, sonst richtig: form_falsch', [datei(BRAIN, [a1, mitW(a2, (o) => { o.sid = 'S-1-5'; })])], P, br(BRAIN, 2, 'form_falsch', 1));
  aFall('zusaetzliches Feld konto: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.konto = 'x'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  for (const f of ['grund', 'kanal', 'route', 'sitzung']) aFall(`Feld ${f} fehlt: form_falsch`, [datei(BRAIN, [mitW(a1, (o) => { delete o[f]; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Grund nicht aus der Liste: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.grund = 'pk.fremd'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Grund kein Text: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.grund = 5; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  const sz = (grund, kanal) => zeileAbgewiesen({ ...A1, grund: 'pk.hallo_fehlt', kanal }, NULL64).replace('pk.hallo_fehlt', grund);
  aFall('such.sitzung_fremd bei Kanal such: gruen', [datei(BRAIN, [sz('such.sitzung_fremd', 'such')])], P, ok(1, sha(sz('such.sitzung_fremd', 'such'))));
  aFall('pk.sid_nicht_erlaubt bei Kanal stelle: gruen', [datei(BRAIN, [sz('pk.sid_nicht_erlaubt', 'stelle')])], P, ok(1, sha(sz('pk.sid_nicht_erlaubt', 'stelle'))));
  aFall('such.sitzung_fremd bei Kanal stelle: form_falsch', [datei(BRAIN, [sz('such.sitzung_fremd', 'stelle')])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('such.hallo_fehlt bei Kanal such: form_falsch', [datei(BRAIN, [sz('such.hallo_fehlt', 'such')])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Grund der frueheren Liste peer.sid_fremd: form_falsch', [datei(BRAIN, [sz('peer.sid_fremd', 'such')])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Kanal mit Grossbuchstaben: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.kanal = 'Such'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Route kein Text und nicht null: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.route = 5; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Sitzung mit Leerzeichen: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.sitzung = 's 1'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Ergebnis gesperrt statt abgewiesen: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.ergebnis = 'gesperrt'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('Zeit ohne Millisekunden: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.zeit = '2026-11-20T13:00:00Z'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('fassung 3: form_falsch', [datei(BRAIN, [mitW(a1, (o) => { o.fassung = 3; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  aFall('ohne fassung in brain-Datei: fassung fehlt', [datei(BRAIN, [mitW(a1, (o) => { delete o.fassung; })])], P, br(BRAIN, 1, 'fassung_fehlt', 0));
  aFall('ohne fassung in gateway-Datei (1g): form_falsch, die Zeile ist nur in Fassung 2 gueltig', [datei('gateway-2026-11.jsonl', [mitW(a1, (o) => { delete o.fassung; })])], ['pruefe', '@gateway-2026-11.jsonl'], br('gateway-2026-11.jsonl', 1, 'form_falsch', 0));
  aFall('--letzte mit abgewiesen-Zeilen: rot bei geaenderter letzter Zeile', [datei(BRAIN, [a1, a2, a3.replace('"kanal":"stelle"', '"kanal":"such"')])], ['pruefe', '--letzte', sha(a3), `@${BRAIN}`], br(BRAIN, 3, 'letzte_falsch', 3));

  // Anhaengen.
  const ag = (eingabe, absicht = 'ok', quelle = 'brain') => ({ art: 'abgewiesen', quelle, eingabe, absicht });
  const nachAbg = (n = 1) => { for (let i = 0; i < n; i += 1) abgAnhaengen.push(monateFaelle.pop()); };
  mFall('abgewiesen: neue Datei, erste Zeile mit 64 Nullen', {}, [ag(e1)]);
  nachAbg();
  {
    const ende = mFall('abgewiesen und gesperrt hintereinander, dann der Wechsel in den Dezember mit Anker', { [NOVD]: text(novZ.zeilen) }, [ag({ ...e1, zeit_ms: NOV + 5000 }), g(ev(NOV + 6000, 4)), ag({ ...e3, zeit_ms: DEZ }), ag({ ...e2, zeit_ms: DEZ + 1 })]);
    nachAbg();
    if (zeilenVon(ende[A]).length !== 1 || zeilenVon(ende[DEZD]).length !== 2) throw new Error('abgewiesen: Wechsel');
  }
  mFall('abgewiesen: Grund und Kanal gehoeren zusammen', {}, [ag({ ...e1, grund: 'such.sitzung_fremd', kanal: 'such' }), ag({ ...e1, grund: 'pk.sid_nicht_erlaubt', kanal: 'stelle' }), ag({ ...e1, grund: 'such.sitzung_fremd', kanal: 'stelle' }, 'feld_ungueltig:grund'), ag({ ...e1, grund: 'pk.fremd' }, 'feld_ungueltig:grund')]);
  nachAbg();
  mFall('abgewiesen: Felder in der Reihenfolge Grund, Kanal, Route, Sitzung', {}, [
    ag({ ...e1, grund: 'x', kanal: 'X' }, 'feld_ungueltig:grund'),
    ag({ ...e1, kanal: 'X', route: 'X' }, 'feld_ungueltig:kanal'),
    ag({ ...e1, route: 'X', sitzung: 'X' }, 'feld_ungueltig:route'),
    ag({ ...e1, sitzung: 'X' }, 'feld_ungueltig:sitzung'),
  ]);
  nachAbg();
  mFall('abgewiesen: Felder vor Quelle vor Zeit', {}, [ag({ ...e1, grund: 'x' }, 'feld_ungueltig:grund', 'audit'), ag(e1, 'feld_ungueltig:quelle', 'audit'), ag({ ...e1, zeit_ms: -1 }, 'feld_ungueltig:quelle', 'audit'), ag({ ...e1, zeit_ms: -1 }, 'zeit_ungueltig')]);
  nachAbg();
  mFall('abgewiesen: Zeit rueckwaerts nach dem Wechsel', { [NOVD]: text(novZ.zeilen) }, [ag({ ...e1, zeit_ms: DEZ }), ag({ ...e1, zeit_ms: NOV + 9 }, 'zeit_rueckwaerts')]);
  nachAbg();
  mFall('abgewiesen: Kanal null ist kein Schluessel', {}, [ag({ ...e1, kanal: null }, 'feld_ungueltig:kanal')]);
  nachAbg();
  // Das allgemeine Anhaengen lehnt eine abgewiesen-Zeile ausserhalb der Form ab.
  const zlA = (objekt, absicht) => ({ art: 'zeile', quelle: 'brain', zeit_ms: NOV, objekt, absicht });
  const basis = { aktionsname: 'kanal.abgewiesen', ergebnis: 'abgewiesen', grund: 'pk.sid_nicht_erlaubt', kanal: 'such', route: null, sitzung: null };
  mFall('allgemeines Anhaengen: kanal.abgewiesen nur in der geschlossenen Form', {}, [zlA({ ...basis, sid: 'x' }, 'feld_ungueltig:zeile'), zlA({ ...basis, grund: 'pk.fremd' }, 'feld_ungueltig:zeile'), zlA({ ...basis, ergebnis: 'gesperrt' }, 'feld_ungueltig:zeile'), zlA((() => { const o = { ...basis }; delete o.route; return o; })(), 'feld_ungueltig:zeile'), zlA(basis, 'ok')]);
  nachAbg();
  const zeilenKette = kettenStueck([ev(NOV + 10, 3)], novZ.letzte).zeilen;
  const abgKette = zeileAbgewiesen({ ...e1, zeit_ms: NOV + 20 }, sha(zeilenKette[0]));
  const abgFalsch = jcs({ ...JSON.parse(abgKette), sid: 'x' });
  mFall('mehrere Zeilen mit einer abgewiesen-Zeile: gepruefte Form', { [NOVD]: text(novZ.zeilen) }, [
    { art: 'zeilen', quelle: 'brain', erwartet_vorher: novZ.letzte, zeilen: [zeilenKette[0], abgFalsch], absicht: 'zeile_ungueltig:form_falsch' },
    { art: 'zeilen', quelle: 'brain', erwartet_vorher: novZ.letzte, zeilen: [zeilenKette[0], abgKette], absicht: 'ok' },
  ]);
  nachAbg();
}

// --- Fälle: probelauf.zaehlung (Abschnitt 6) ---

const probeZeile = [];
const probePruefe = [];
const probeAnhaengen = [];
const P1 = { zeit_ms: T0, programm: 'suche', sitzung: 's-1', anzahl: 3 };
const probeFall = (fall, e, vorher, absichtFehler = null) => {
  let erg;
  try {
    const z = zeileProbelauf(e, vorher);
    erg = { zeile: z, sha256: sha(z) };
  } catch (x) {
    if (!(x instanceof Fehler)) throw x;
    erg = { fehler: x.message };
  }
  if ((erg.fehler ?? null) !== absichtFehler) throw new Error(`probelauf ${fall}: ${erg.fehler ?? 'ok'}, Absicht ${absichtFehler ?? 'ok'}`);
  probeZeile.push({ fall, eingabe: e, vorher, erwartet: erg });
};
probeFall('Programm suche, anzahl 3', P1, NULL64);
probeFall('Programm modellweg, anzahl 1 (untere Grenze)', { ...P1, programm: 'modellweg', anzahl: 1 }, sha('x'));
probeFall('anzahl 16 (obere Grenze)', { ...P1, anzahl: 16 }, sha('x'));
probeFall('Sitzung mit 64 Zeichen', { ...P1, sitzung: `a.b-c_${'d'.repeat(58)}` }, NULL64);
probeFall('Programm nicht aus der Liste', { ...P1, programm: 'stelle' }, NULL64, 'feld_ungueltig:programm');
probeFall('Programm mit Grossbuchstaben', { ...P1, programm: 'Suche' }, NULL64, 'feld_ungueltig:programm');
probeFall('Programm leer', { ...P1, programm: '' }, NULL64, 'feld_ungueltig:programm');
probeFall('Programm null', { ...P1, programm: null }, NULL64, 'feld_ungueltig:programm');
probeFall('Sitzung mit Grossbuchstaben', { ...P1, sitzung: 'S-1' }, NULL64, 'feld_ungueltig:sitzung');
probeFall('Sitzung leer', { ...P1, sitzung: '' }, NULL64, 'feld_ungueltig:sitzung');
probeFall('Sitzung null (hier kein null erlaubt)', { ...P1, sitzung: null }, NULL64, 'feld_ungueltig:sitzung');
probeFall('Sitzung mit 65 Zeichen', { ...P1, sitzung: 's'.repeat(65) }, NULL64, 'feld_ungueltig:sitzung');
probeFall('anzahl 0', { ...P1, anzahl: 0 }, NULL64, 'feld_ungueltig:anzahl');
probeFall('anzahl 17', { ...P1, anzahl: 17 }, NULL64, 'feld_ungueltig:anzahl');
probeFall('vorher keine 64 hex', P1, 'abc', 'feld_ungueltig:vorher');
probeFall('Zeit vor 1970', { ...P1, zeit_ms: -1 }, NULL64, 'zeit_ungueltig');
probeFall('zwei Fehler: Programm vor Sitzung', { ...P1, programm: 'x', sitzung: 'X' }, NULL64, 'feld_ungueltig:programm');
probeFall('zwei Fehler: Sitzung vor anzahl', { ...P1, sitzung: 'X', anzahl: 0 }, NULL64, 'feld_ungueltig:sitzung');
probeFall('zwei Fehler: anzahl vor vorher', { ...P1, anzahl: 0 }, 'abc', 'feld_ungueltig:anzahl');
probeFall('zwei Fehler: Felder vor Zeit', { ...P1, anzahl: 0, zeit_ms: -1 }, NULL64, 'feld_ungueltig:anzahl');

{
  const p1 = { ...P1 };
  const p2 = { ...P1, zeit_ms: T0 + 1000, programm: 'modellweg', anzahl: 16 };
  const p3 = { ...P1, zeit_ms: T0 + 2000, sitzung: 's-2', anzahl: 1 };
  const z1 = zeileProbelauf(p1, NULL64);
  const z2 = zeileProbelauf(p2, sha(z1));
  const z3 = zeileProbelauf(p3, sha(z2));
  const ok = (zeilen, letzte) => ({ exit: 0, ok: true, zeilen, bruch: null, letzte });
  const pF = (fall, dateien, aufruf, absicht) => pFall(fall, dateien, aufruf, absicht, probePruefe);
  const P = ['pruefe', `@${BRAIN}`];
  const mitW = (z, f) => { const o = JSON.parse(z); f(o); return jcs(o); };
  pF('drei probelauf-Zeilen: gruen', [datei(BRAIN, [z1, z2, z3])], P, ok(3, sha(z3)));
  const g2 = zeileGesperrt(E1, sha(z1));
  const a3 = zeileAbgewiesen(A1, sha(g2));
  const z4 = zeileProbelauf(p3, sha(a3));
  pF('probelauf, gesperrt und abgewiesen gemischt in einer Kette: gruen', [datei(BRAIN, [z1, g2, a3, z4])], P, ok(4, sha(z4)));
  pF('anzahl in Zeile 2 von Hand geaendert: rot in Zeile 3', [datei(BRAIN, [z1, z2.replace('"anzahl":16', '"anzahl":15'), z3])], P, br(BRAIN, 3, 'kette_gebrochen', 2));
  pF('Leerzeichen eingefuegt: nicht kanonisch', [datei(BRAIN, [z1, z2.replace('"anzahl":16', '"anzahl": 16')])], P, br(BRAIN, 2, 'nicht_kanonisch', 1));
  pF('zusaetzliches Feld marker: form_falsch', [datei(BRAIN, [z1, mitW(z2, (o) => { o.marker = 'm-1'; })])], P, br(BRAIN, 2, 'form_falsch', 1));
  pF('zusaetzliches Feld marker_anzahl: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.marker_anzahl = 2; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  for (const f of ['anzahl', 'programm', 'sitzung', 'ergebnis']) pF(`Feld ${f} fehlt: form_falsch`, [datei(BRAIN, [mitW(z1, (o) => { delete o[f]; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('anzahl 0: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.anzahl = 0; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('anzahl 17: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.anzahl = 17; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('anzahl 2.5: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.anzahl = 2.5; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('anzahl als Text: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.anzahl = '3'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Programm nicht aus der Liste: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.programm = 'stelle'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Programm kein Text: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.programm = 5; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Sitzung null: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.sitzung = null; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Sitzung mit Leerzeichen: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.sitzung = 's 1'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Ergebnis ok statt an: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.ergebnis = 'ok'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('Zeit ohne Millisekunden: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.zeit = '2026-11-20T13:00:00Z'; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('fassung 3: form_falsch', [datei(BRAIN, [mitW(z1, (o) => { o.fassung = 3; })])], P, br(BRAIN, 1, 'form_falsch', 0));
  pF('ohne fassung in brain-Datei: fassung fehlt', [datei(BRAIN, [mitW(z1, (o) => { delete o.fassung; })])], P, br(BRAIN, 1, 'fassung_fehlt', 0));
  pF('ohne fassung in gateway-Datei (1g): form_falsch', [datei('gateway-2026-11.jsonl', [mitW(z1, (o) => { delete o.fassung; })])], ['pruefe', '@gateway-2026-11.jsonl'], br('gateway-2026-11.jsonl', 1, 'form_falsch', 0));

  const pr = (eingabe, absicht = 'ok', quelle = 'brain') => ({ art: 'probelauf', quelle, eingabe, absicht });
  const nachProbe = () => probeAnhaengen.push(monateFaelle.pop());
  mFall('probelauf: neue Datei, erste Zeile mit 64 Nullen', {}, [pr(p1)]);
  nachProbe();
  {
    const ende = mFall('probelauf und gesperrt hintereinander, dann der Wechsel in den Dezember mit Anker', { [NOVD]: text(novZ.zeilen) }, [pr({ ...p1, zeit_ms: NOV + 5000 }), g(ev(NOV + 6000, 4)), pr({ ...p2, zeit_ms: DEZ }), pr({ ...p3, zeit_ms: DEZ + 1 })]);
    nachProbe();
    if (zeilenVon(ende[A]).length !== 1 || zeilenVon(ende[DEZD]).length !== 2) throw new Error('probelauf: Wechsel');
  }
  mFall('probelauf: Felder in der Reihenfolge Programm, Sitzung, anzahl', {}, [
    pr({ ...p1, programm: 'x', sitzung: 'X' }, 'feld_ungueltig:programm'),
    pr({ ...p1, sitzung: 'X', anzahl: 0 }, 'feld_ungueltig:sitzung'),
    pr({ ...p1, anzahl: 17 }, 'feld_ungueltig:anzahl'),
  ]);
  nachProbe();
  mFall('probelauf: Felder vor Quelle vor Zeit', {}, [pr({ ...p1, anzahl: 0 }, 'feld_ungueltig:anzahl', 'audit'), pr(p1, 'feld_ungueltig:quelle', 'audit'), pr({ ...p1, zeit_ms: -1 }, 'feld_ungueltig:quelle', 'audit'), pr({ ...p1, zeit_ms: -1 }, 'zeit_ungueltig')]);
  nachProbe();
  mFall('probelauf: Zeit rueckwaerts nach dem Wechsel', { [NOVD]: text(novZ.zeilen) }, [pr({ ...p1, zeit_ms: DEZ }), pr({ ...p1, zeit_ms: NOV + 9 }, 'zeit_rueckwaerts')]);
  nachProbe();
  const zlP = (objekt, absicht) => ({ art: 'zeile', quelle: 'brain', zeit_ms: NOV, objekt, absicht });
  const basis = { aktionsname: 'probelauf.zaehlung', ergebnis: 'an', programm: 'suche', sitzung: 's-1', anzahl: 3 };
  mFall('allgemeines Anhaengen: probelauf.zaehlung nur in der geschlossenen Form', {}, [zlP({ ...basis, marker: 'm' }, 'feld_ungueltig:zeile'), zlP({ ...basis, anzahl: 17 }, 'feld_ungueltig:zeile'), zlP({ ...basis, programm: 'x' }, 'feld_ungueltig:zeile'), zlP({ ...basis, ergebnis: 'ok' }, 'feld_ungueltig:zeile'), zlP((() => { const o = { ...basis }; delete o.sitzung; return o; })(), 'feld_ungueltig:zeile'), zlP(basis, 'ok')]);
  nachProbe();
  const zk = kettenStueck([ev(NOV + 10, 3)], novZ.letzte).zeilen;
  const pk = zeileProbelauf({ ...p1, zeit_ms: NOV + 20 }, sha(zk[0]));
  mFall('mehrere Zeilen mit einer probelauf-Zeile: gepruefte Form', { [NOVD]: text(novZ.zeilen) }, [
    { art: 'zeilen', quelle: 'brain', erwartet_vorher: novZ.letzte, zeilen: [zk[0], jcs({ ...JSON.parse(pk), marker: 'm' })], absicht: 'zeile_ungueltig:form_falsch' },
    { art: 'zeilen', quelle: 'brain', erwartet_vorher: novZ.letzte, zeilen: [zk[0], pk], absicht: 'ok' },
  ]);
  nachProbe();
}

// --- Fälle: ordnermarke.geloest und ordnermarke.gesenkt (Abschnitt 6.4) ---

const ordZeile = [];
const ordPruefe = [];
const ordAnhaengen = [];
{
  const MK = 'a1b2c3d4e5f60718293a4b5c6d7e8f90';
  const GE = { zeit_ms: T0, aktionsname: 'ordnermarke.geloest', marke: MK, klasse_alt: 'kundendaten', klasse_neu: null, kunde_alt: 'k-beispiel', kunde_neu: null };
  const SE = { zeit_ms: T0, aktionsname: 'ordnermarke.gesenkt', marke: MK, klasse_alt: 'kundendaten', klasse_neu: 'intern', kunde_alt: 'k-beispiel', kunde_neu: 'k-beispiel' };
  const ordFall = (fall, e, vorher, absichtFehler = null) => {
    let erg;
    try {
      const z = zeileOrdnermarke(e, vorher);
      erg = { zeile: z, sha256: sha(z) };
    } catch (x) {
      if (!(x instanceof Fehler)) throw x;
      erg = { fehler: x.message };
    }
    if ((erg.fehler ?? null) !== absichtFehler) throw new Error(`ordnermarke ${fall}: ${erg.fehler ?? 'ok'}, Absicht ${absichtFehler ?? 'ok'}`);
    ordZeile.push({ fall, eingabe: e, vorher, erwartet: erg });
  };
  ordFall('geloest: Kunde und Klasse der alten Marke', GE, NULL64);
  ordFall('geloest: nur die Klasse bekannt (kunde_alt null)', { ...GE, kunde_alt: null }, sha('x'));
  ordFall('geloest: nur der Kunde bekannt (klasse_alt null)', { ...GE, klasse_alt: null }, NULL64);
  ordFall('gesenkt: kundendaten nach intern, Kunde bleibt', SE, NULL64);
  ordFall('gesenkt: Marke ohne Kunde (kunde_alt und kunde_neu null)', { ...SE, kunde_alt: null, kunde_neu: null }, NULL64);
  ordFall('gesenkt: klasse_alt null, klasse_neu gesetzt', { ...SE, klasse_alt: null }, NULL64);
  ordFall('marke aus 32 Nullen (Grenze der Form)', { ...GE, marke: '0'.repeat(32) }, NULL64);
  ordFall('Kennung mit 64 Zeichen in klasse_alt (obere Grenze)', { ...GE, klasse_alt: `a.b-c_${'d'.repeat(58)}` }, NULL64);
  ordFall('Aktionsname unbekannt', { ...GE, aktionsname: 'ordnermarke.entfernt' }, NULL64, 'feld_ungueltig:aktionsname');
  ordFall('Aktionsname ausgabe.gesperrt', { ...GE, aktionsname: 'ausgabe.gesperrt' }, NULL64, 'feld_ungueltig:aktionsname');
  ordFall('Aktionsname null', { ...GE, aktionsname: null }, NULL64, 'feld_ungueltig:aktionsname');
  ordFall('marke mit Grossbuchstaben', { ...GE, marke: MK.toUpperCase() }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke mit 31 Zeichen', { ...GE, marke: MK.slice(1) }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke mit 33 Zeichen', { ...GE, marke: `${MK}0` }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke leer', { ...GE, marke: '' }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke null', { ...GE, marke: null }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke als Pfad', { ...GE, marke: '/srv/beispiel/projekt' }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke als sha256 (64 hex, etwa ueber einen Pfad)', { ...GE, marke: sha('/srv/beispiel/projekt') }, NULL64, 'feld_ungueltig:marke');
  ordFall('marke mit g (kein hex)', { ...GE, marke: `g${MK.slice(1)}` }, NULL64, 'feld_ungueltig:marke');
  ordFall('klasse_alt mit Grossbuchstaben', { ...GE, klasse_alt: 'Kundendaten' }, NULL64, 'feld_ungueltig:klasse_alt');
  ordFall('klasse_alt mit Pfad', { ...GE, klasse_alt: '/srv/beispiel' }, NULL64, 'feld_ungueltig:klasse_alt');
  ordFall('klasse_alt leer', { ...GE, klasse_alt: '' }, NULL64, 'feld_ungueltig:klasse_alt');
  ordFall('klasse_neu mit Grossbuchstaben (gesenkt)', { ...SE, klasse_neu: 'Intern' }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('kunde_alt mit 65 Zeichen', { ...GE, kunde_alt: 'k'.repeat(65) }, NULL64, 'feld_ungueltig:kunde_alt');
  ordFall('kunde_neu mit Leerzeichen (gesenkt)', { ...SE, kunde_neu: 'k beispiel' }, NULL64, 'feld_ungueltig:kunde_neu');
  ordFall('geloest: kunde_alt und klasse_alt beide null', { ...GE, kunde_alt: null, klasse_alt: null }, NULL64, 'feld_ungueltig:klasse_alt');
  ordFall('geloest mit klasse_neu', { ...GE, klasse_neu: 'intern' }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('geloest mit kunde_neu', { ...GE, kunde_neu: 'k-beispiel' }, NULL64, 'feld_ungueltig:kunde_neu');
  ordFall('gesenkt: klasse_neu null', { ...SE, klasse_neu: null }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('gesenkt: klasse_neu gleich klasse_alt', { ...SE, klasse_neu: 'kundendaten' }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('gesenkt: klasse_alt und klasse_neu beide null', { ...SE, klasse_alt: null, klasse_neu: null }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('gesenkt mit anderem kunde_neu', { ...SE, kunde_neu: 'k-anderer' }, NULL64, 'feld_ungueltig:kunde_neu');
  ordFall('gesenkt: kunde_neu null bei kunde_alt gesetzt', { ...SE, kunde_neu: null }, NULL64, 'feld_ungueltig:kunde_neu');
  ordFall('gesenkt: kunde_alt null, kunde_neu gesetzt', { ...SE, kunde_alt: null }, NULL64, 'feld_ungueltig:kunde_neu');
  ordFall('vorher keine 64 hex', GE, 'abc', 'feld_ungueltig:vorher');
  ordFall('Zeit vor 1970', { ...GE, zeit_ms: -1 }, NULL64, 'zeit_ungueltig');
  ordFall('zwei Fehler: Aktionsname vor marke', { ...GE, aktionsname: 'x', marke: 'X' }, NULL64, 'feld_ungueltig:aktionsname');
  ordFall('zwei Fehler: marke vor klasse_alt', { ...GE, marke: 'X', klasse_alt: 'X' }, NULL64, 'feld_ungueltig:marke');
  ordFall('zwei Fehler: Form vor Kopplung (klasse_alt Form, klasse_neu Kopplung)', { ...GE, klasse_alt: 'X', klasse_neu: 'intern' }, NULL64, 'feld_ungueltig:klasse_alt');
  ordFall('zwei Fehler: Kopplung klasse_neu vor kunde_neu', { ...GE, klasse_neu: 'intern', kunde_neu: 'k-x' }, NULL64, 'feld_ungueltig:klasse_neu');
  ordFall('zwei Fehler: Felder vor vorher', { ...GE, marke: 'X' }, 'abc', 'feld_ungueltig:marke');
  ordFall('zwei Fehler: vorher vor Zeit', GE, 'abc', 'feld_ungueltig:vorher');
  ordFall('zwei Fehler: Felder vor Zeit', { ...GE, marke: 'X', zeit_ms: -1 }, NULL64, 'feld_ungueltig:marke');

  const o1 = { ...GE };
  const o2 = { ...SE, zeit_ms: T0 + 1000, marke: 'f'.repeat(32) };
  const o3 = { ...GE, zeit_ms: T0 + 2000, kunde_alt: null, marke: '0123456789abcdef0123456789abcdef' };
  const z1 = zeileOrdnermarke(o1, NULL64);
  const z2 = zeileOrdnermarke(o2, sha(z1));
  const z3 = zeileOrdnermarke(o3, sha(z2));
  const z2n = zeileOrdnermarke(o2, NULL64); // gesenkt als erste Zeile einer Datei
  const ok = (zeilen, letzte) => ({ exit: 0, ok: true, zeilen, bruch: null, letzte });
  const oF = (fall, dateien, aufruf, absicht) => pFall(fall, dateien, aufruf, absicht, ordPruefe);
  const P = ['pruefe', `@${BRAIN}`];
  const mitW = (z, f) => { const o = JSON.parse(z); f(o); return jcs(o); };
  const rot1 = (fall, f, grund = 'form_falsch', z = z1, d = BRAIN) => oF(fall, [datei(d, [mitW(z, f)])], ['pruefe', `@${d}`], br(d, 1, grund, 0));
  oF('drei Zeilen geloest, gesenkt, geloest: gruen', [datei(BRAIN, [z1, z2, z3])], P, ok(3, sha(z3)));
  const g2 = zeileGesperrt(E1, sha(z1));
  const z4 = zeileOrdnermarke(o2, sha(g2));
  oF('ordnermarke und gesperrt gemischt in einer Kette: gruen', [datei(BRAIN, [z1, g2, z4])], P, ok(3, sha(z4)));
  oF('Datei ordnermarke-2026-11.jsonl: gruen', [datei('ordnermarke-2026-11.jsonl', [z1, z2])], ['pruefe', '@ordnermarke-2026-11.jsonl'], ok(2, sha(z2)));
  oF('klasse_neu in Zeile 2 von Hand geaendert: rot in Zeile 3', [datei(BRAIN, [z1, z2.replace('"klasse_neu":"intern"', '"klasse_neu":"oeffentlich"'), z3])], P, br(BRAIN, 3, 'kette_gebrochen', 2));
  oF('Leerzeichen eingefuegt: nicht kanonisch', [datei(BRAIN, [z1, z2.replace('"klasse_neu":"intern"', '"klasse_neu": "intern"')])], P, br(BRAIN, 2, 'nicht_kanonisch', 1));
  oF('letzte Zeile geaendert: ohne --letzte unentdeckt, mit --letzte rot', [datei(BRAIN, [z1, z2.replace('"klasse_neu":"intern"', '"klasse_neu":"oeffentlich"')])], ['pruefe', '--letzte', sha(z2), `@${BRAIN}`], br(BRAIN, 2, 'letzte_falsch', 2));
  rot1('zusaetzliches Feld ordner: form_falsch', (o) => { o.ordner = '/srv/beispiel/projekt'; });
  rot1('zusaetzliches Feld ordner_hash: form_falsch', (o) => { o.ordner_hash = sha('/srv/beispiel/projekt'); });
  rot1('zusaetzliches Feld titel: form_falsch', (o) => { o.titel = 't'; });
  rot1('zusaetzliches Feld ordner in Zeile 2: rot in Zeile 2', (o) => { o.ordner = '/srv/beispiel/projekt'; }, 'form_falsch', z1);
  for (const f of ['klasse_alt', 'klasse_neu', 'kunde_alt', 'kunde_neu', 'marke', 'ergebnis']) rot1(`Feld ${f} fehlt: form_falsch`, (o) => { delete o[f]; });
  rot1('marke mit Grossbuchstaben: form_falsch', (o) => { o.marke = MK.toUpperCase(); });
  rot1('marke mit 31 Zeichen: form_falsch', (o) => { o.marke = MK.slice(1); });
  rot1('marke mit 33 Zeichen: form_falsch', (o) => { o.marke = `${MK}0`; });
  rot1('marke als sha256 (64 hex): form_falsch', (o) => { o.marke = sha('/srv/beispiel/projekt'); });
  rot1('marke als Pfad: form_falsch', (o) => { o.marke = '/srv/beispiel/projekt'; });
  rot1('marke null: form_falsch', (o) => { o.marke = null; });
  rot1('marke als Zahl: form_falsch', (o) => { o.marke = 5; });
  rot1('klasse_alt mit Grossbuchstaben: form_falsch', (o) => { o.klasse_alt = 'Kundendaten'; });
  rot1('kunde_alt als Zahl: form_falsch', (o) => { o.kunde_alt = 5; });
  rot1('geloest mit klasse_neu: form_falsch', (o) => { o.klasse_neu = 'intern'; });
  rot1('geloest mit kunde_neu: form_falsch', (o) => { o.kunde_neu = 'k-beispiel'; });
  rot1('geloest mit kunde_alt und klasse_alt null: form_falsch', (o) => { o.kunde_alt = null; o.klasse_alt = null; });
  rot1('gesenkt mit klasse_neu gleich klasse_alt: form_falsch', (o) => { o.klasse_neu = o.klasse_alt; }, 'form_falsch', z2n);
  rot1('gesenkt mit klasse_neu null: form_falsch', (o) => { o.klasse_neu = null; }, 'form_falsch', z2n);
  rot1('gesenkt mit anderem kunde_neu: form_falsch', (o) => { o.kunde_neu = 'k-anderer'; }, 'form_falsch', z2n);
  rot1('gesenkt mit kunde_neu null: form_falsch', (o) => { o.kunde_neu = null; }, 'form_falsch', z2n);
  rot1('Ergebnis geloest statt ausgefuehrt: form_falsch', (o) => { o.ergebnis = 'geloest'; });
  rot1('Ergebnis unbekannt: form_falsch', (o) => { o.ergebnis = 'unbekannt'; });
  rot1('Ergebnis null: form_falsch', (o) => { o.ergebnis = null; });
  rot1('Zeit ohne Millisekunden: form_falsch', (o) => { o.zeit = '2026-11-20T13:00:00Z'; });
  rot1('fassung 3: form_falsch', (o) => { o.fassung = 3; });
  rot1('ohne fassung in brain-Datei: fassung fehlt', (o) => { delete o.fassung; }, 'fassung_fehlt');
  rot1('ohne fassung in gateway-Datei (1g): form_falsch', (o) => { delete o.fassung; }, 'form_falsch', z1, 'gateway-2026-11.jsonl');

  const om = (eingabe, absicht = 'ok', quelle = 'ordnermarke') => ({ art: 'ordnermarke', quelle, eingabe, absicht });
  const nachOrd = () => ordAnhaengen.push(monateFaelle.pop());
  const OMQ = ['ordnermarke'];
  const NOVO = monat('ordnermarke', NOV);
  const DEZO = monat('ordnermarke', DEZ);
  const AO = 'ordnermarke.anker.jsonl';
  mFall('ordnermarke: neue Datei, erste Zeile mit 64 Nullen', {}, [om({ ...o1, zeit_ms: NOV })], OMQ);
  nachOrd();
  {
    const n1 = zeileOrdnermarke({ ...o1, zeit_ms: NOV }, NULL64);
    const ende = mFall('ordnermarke: geloest und gesenkt, dann der Wechsel in den Dezember mit Anker', { [NOVO]: `${n1}\n` }, [om({ ...o2, zeit_ms: NOV + 5000 }), om({ ...o3, zeit_ms: DEZ }), om({ ...o2, zeit_ms: DEZ + 1 })], OMQ);
    nachOrd();
    if (zeilenVon(ende[AO]).length !== 1 || zeilenVon(ende[DEZO]).length !== 2) throw new Error('ordnermarke: Wechsel');
  }
  mFall('ordnermarke: Felder in der Reihenfolge Aktionsname, marke, Formen, Kopplung', {}, [
    om({ ...o1, aktionsname: 'x', marke: 'X' }, 'feld_ungueltig:aktionsname'),
    om({ ...o1, marke: 'X', klasse_alt: 'X' }, 'feld_ungueltig:marke'),
    om({ ...o1, klasse_alt: 'X', klasse_neu: 'intern' }, 'feld_ungueltig:klasse_alt'),
    om({ ...o1, klasse_neu: 'intern', kunde_neu: 'k-x' }, 'feld_ungueltig:klasse_neu'),
    om({ ...o2, kunde_neu: 'k-x' }, 'feld_ungueltig:kunde_neu'),
  ], OMQ);
  nachOrd();
  mFall('ordnermarke: Felder vor Quelle vor Zeit', {}, [om({ ...o1, marke: 'X' }, 'feld_ungueltig:marke', 'audit'), om(o1, 'feld_ungueltig:quelle', 'audit'), om({ ...o1, zeit_ms: -1 }, 'feld_ungueltig:quelle', 'audit'), om({ ...o1, zeit_ms: -1 }, 'zeit_ungueltig')], OMQ);
  nachOrd();
  mFall('ordnermarke: Zeit rueckwaerts nach dem Wechsel', { [NOVO]: `${zeileOrdnermarke({ ...o1, zeit_ms: NOV }, NULL64)}\n` }, [om({ ...o2, zeit_ms: DEZ }), om({ ...o2, zeit_ms: NOV + 9 }, 'zeit_rueckwaerts')], OMQ);
  nachOrd();
  const zlO = (objekt, absicht) => ({ art: 'zeile', quelle: 'ordnermarke', zeit_ms: NOV, objekt, absicht });
  const bG = { aktionsname: 'ordnermarke.geloest', ergebnis: 'ausgefuehrt', marke: MK, klasse_alt: 'kundendaten', klasse_neu: null, kunde_alt: 'k-beispiel', kunde_neu: null };
  const bS = { aktionsname: 'ordnermarke.gesenkt', ergebnis: 'ausgefuehrt', marke: MK, klasse_alt: 'kundendaten', klasse_neu: 'intern', kunde_alt: 'k-beispiel', kunde_neu: 'k-beispiel' };
  mFall('allgemeines Anhaengen: ordnermarke.* nur in der geschlossenen Form', {}, [
    zlO({ ...bG, ordner: '/srv/beispiel/projekt' }, 'feld_ungueltig:zeile'),
    zlO({ ...bG, ordner_hash: sha('/srv/beispiel/projekt') }, 'feld_ungueltig:zeile'),
    zlO({ ...bG, marke: MK.toUpperCase() }, 'feld_ungueltig:zeile'),
    zlO({ ...bG, marke: MK.slice(1) }, 'feld_ungueltig:zeile'),
    zlO({ ...bG, klasse_neu: 'intern' }, 'feld_ungueltig:zeile'),
    zlO({ ...bS, klasse_neu: 'kundendaten' }, 'feld_ungueltig:zeile'),
    zlO({ ...bS, kunde_neu: 'k-anderer' }, 'feld_ungueltig:zeile'),
    zlO({ ...bG, ergebnis: 'unbekannt' }, 'feld_ungueltig:zeile'),
    zlO((() => { const o = { ...bG }; delete o.kunde_neu; return o; })(), 'feld_ungueltig:zeile'),
    zlO(bG, 'ok'),
    zlO(bS, 'ok'),
  ], OMQ);
  nachOrd();
  const n1 = zeileOrdnermarke({ ...o1, zeit_ms: NOV }, NULL64);
  const pk = zeileOrdnermarke({ ...o2, zeit_ms: NOV + 20 }, sha(n1));
  mFall('mehrere Zeilen mit einer ordnermarke-Zeile: gepruefte Form', { [NOVO]: `${n1}\n` }, [
    { art: 'zeilen', quelle: 'ordnermarke', erwartet_vorher: sha(n1), zeilen: [jcs({ ...JSON.parse(pk), ordner: '/srv/beispiel/projekt' })], absicht: 'zeile_ungueltig:form_falsch' },
    { art: 'zeilen', quelle: 'ordnermarke', erwartet_vorher: sha(n1), zeilen: [pk], absicht: 'ok' },
  ], OMQ);
  nachOrd();
}

// --- Fälle: person.geloescht und person.auskunft (Vertrag 9, Abschnitt 8) ---

const persPruefe = [];
const persAnhaengen = [];
{
  const PSA = `ps_${'a'.repeat(32)}`;
  const PSB = `ps_${'b'.repeat(32)}`;
  const gl = (ergebnis, handelnd, marke, ms = T0) => ({ aktionsname: 'person.geloescht', ergebnis, handelnd, loeschmarke: marke });
  const au = (ergebnis, handelnd, zeilen) => ({ aktionsname: 'person.auskunft', ergebnis, handelnd, zeilen });
  const bau = (objekt, vorher = NULL64, ms = T0) => zeileAllgemein(objekt, ms, vorher);
  const aFall = (fall, dateien, absicht) => pFall(fall, dateien, ['pruefe', `@${BRAIN}`], absicht, persPruefe);
  const gruen = (zeilen) => ({ exit: 0, ok: true, zeilen: zeilen.length, bruch: null, letzte: sha(zeilen[zeilen.length - 1]) });
  const rot = (zeile, grund) => br(BRAIN, zeile, grund, zeile - 1);
  const einzeln = (fall, objekt, grund) => {
    const z = jcs({ ...objekt, fassung: 2, vorher: NULL64, zeit: zeitText(T0) });
    aFall(fall, [datei(BRAIN, [z])], grund === null ? gruen([z]) : rot(1, grund));
  };
  // gueltig
  einzeln('geloescht, geloescht, Pseudonym handelnd', gl('geloescht', PSA, 'c'.repeat(64)), null);
  einzeln('geloescht, Selbstloeschung: handelnd null', gl('geloescht', null, 'c'.repeat(64)), null);
  einzeln('geloescht, unbekannt, Marke null', gl('unbekannt', PSA, null), null);
  einzeln('geloescht, unbekannt, Selbstloeschung null', gl('unbekannt', null, null), null);
  einzeln('auskunft, geliefert, 3 Zeilen', au('geliefert', PSA, 3), null);
  einzeln('auskunft, geliefert, 0 Zeilen', au('geliefert', PSA, 0), null);
  einzeln('auskunft, geliefert, obere Grenze 4294967295', au('geliefert', PSA, 4294967295), null);
  einzeln('auskunft, unbekannt, 0 Zeilen', au('unbekannt', PSB, 0), null);
  const l1 = bau(gl('geloescht', PSA, 'c'.repeat(64)));
  const l2 = bau(au('geliefert', PSB, 2), sha(l1), T0 + 1000);
  const l3 = bau(gl('unbekannt', null, null), sha(l2), T0 + 2000);
  aFall('drei Zeilen der Personenarten in einer Kette: gruen', [datei(BRAIN, [l1, l2, l3])], gruen([l1, l2, l3]));
  // ungueltig: geloescht
  einzeln('geloescht: ergebnis nicht aus der Liste', gl('geliefert', PSA, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: Marke fehlt bei geloescht (null)', gl('geloescht', PSA, null), 'form_falsch');
  einzeln('geloescht: Marke gesetzt bei unbekannt', gl('unbekannt', PSA, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: Marke mit Grossbuchstaben', gl('geloescht', PSA, 'C'.repeat(64)), 'form_falsch');
  einzeln('geloescht: Marke zu kurz', gl('geloescht', PSA, 'c'.repeat(63)), 'form_falsch');
  einzeln('geloescht: Marke keine Text', gl('geloescht', PSA, 5), 'form_falsch');
  einzeln('geloescht: handelnd kein Pseudonym (Kennung)', gl('geloescht', 'anna', 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: handelnd ps_ mit 31 Ziffern', gl('geloescht', `ps_${'a'.repeat(31)}`, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: handelnd ps_ mit 33 Ziffern', gl('geloescht', `ps_${'a'.repeat(33)}`, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: handelnd ps_ mit Grossbuchstaben', gl('geloescht', `ps_${'A'.repeat(32)}`, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: handelnd ft_ statt ps_', gl('geloescht', `ft_${'a'.repeat(32)}`, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: handelnd keine Text', gl('geloescht', 5, 'c'.repeat(64)), 'form_falsch');
  einzeln('geloescht: zusaetzliches Feld kennung', { ...gl('geloescht', PSA, 'c'.repeat(64)), kennung: 'anna' }, 'form_falsch');
  einzeln('geloescht: zusaetzliches Feld pseudonym', { ...gl('geloescht', PSA, 'c'.repeat(64)), pseudonym: PSB }, 'form_falsch');
  for (const f of ['ergebnis', 'handelnd', 'loeschmarke']) {
    const o = gl('geloescht', PSA, 'c'.repeat(64)); delete o[f];
    einzeln(`geloescht: Feld ${f} fehlt`, o, 'form_falsch');
  }
  // ungueltig: auskunft
  einzeln('auskunft: ergebnis nicht aus der Liste', au('geloescht', PSA, 1), 'form_falsch');
  einzeln('auskunft: handelnd null (nicht erlaubt)', au('geliefert', null, 1), 'form_falsch');
  einzeln('auskunft: handelnd kein Pseudonym', au('geliefert', 'anna', 1), 'form_falsch');
  einzeln('auskunft: unbekannt mit 1 Zeile', au('unbekannt', PSA, 1), 'form_falsch');
  einzeln('auskunft: zeilen 4294967296', au('geliefert', PSA, 4294967296), 'form_falsch');
  einzeln('auskunft: zeilen -1', au('geliefert', PSA, -1), 'form_falsch');
  einzeln('auskunft: zeilen 1.5', au('geliefert', PSA, 1.5), 'form_falsch');
  einzeln('auskunft: zeilen als Text', au('geliefert', PSA, '3'), 'form_falsch');
  einzeln('auskunft: zeilen null', au('geliefert', PSA, null), 'form_falsch');
  einzeln('auskunft: zusaetzliches Feld kennung', { ...au('geliefert', PSA, 1), kennung: 'anna' }, 'form_falsch');
  einzeln('auskunft: zusaetzliches Feld loeschmarke', { ...au('geliefert', PSA, 1), loeschmarke: null }, 'form_falsch');
  for (const f of ['ergebnis', 'handelnd', 'zeilen']) {
    const o = au('geliefert', PSA, 1); delete o[f];
    einzeln(`auskunft: Feld ${f} fehlt`, o, 'form_falsch');
  }
  // Fassung
  const gz = jcs({ ...gl('geloescht', PSA, 'c'.repeat(64)), fassung: 2, vorher: NULL64, zeit: zeitText(T0) });
  const ohneF = (() => { const o = JSON.parse(gz); delete o.fassung; return jcs(o); })();
  aFall('geloescht ohne fassung in brain-Datei: fassung fehlt', [datei(BRAIN, [ohneF])], rot(1, 'fassung_fehlt'));
  pFall('geloescht ohne fassung in gateway-Datei (1g): form_falsch', [datei('gateway-2026-11.jsonl', [ohneF])], ['pruefe', '@gateway-2026-11.jsonl'], br('gateway-2026-11.jsonl', 1, 'form_falsch', 0), persPruefe);
  aFall('geloescht mit fassung 3: form_falsch', [datei(BRAIN, [jcs({ ...JSON.parse(gz), fassung: 3 })])], rot(1, 'form_falsch'));
  // allgemeines Anhaengen
  const zl = (objekt, absicht) => ({ art: 'zeile', quelle: 'brain', zeit_ms: NOV, objekt, absicht });
  mFall('allgemeines Anhaengen: person.geloescht nur in der geschlossenen Form', {}, [zl({ ...gl('geloescht', PSA, 'c'.repeat(64)), ergebnis: 'geloescht', kennung: 'anna', ...{} }, 'feld_ungueltig:zeile'), zl(gl('geloescht', 'anna', 'c'.repeat(64)), 'feld_ungueltig:zeile'), zl({ ...gl('unbekannt', PSA, null) }, 'ok')]);
  persAnhaengen.push(monateFaelle.pop());
  mFall('allgemeines Anhaengen: person.auskunft nur in der geschlossenen Form', {}, [zl(au('unbekannt', PSA, 2), 'feld_ungueltig:zeile'), zl(au('geliefert', null, 2), 'feld_ungueltig:zeile'), zl(au('geliefert', PSA, 2), 'ok')]);
  persAnhaengen.push(monateFaelle.pop());
}

// --- Fälle: die vier Zeilenarten der Entscheidungsstelle (Abschnitt 6.8, Vertrag 6 Abschnitte 7.1 bis 7.3) ---

const stellePruefe = [];
const stelleAnhaengen = [];
{
  const SD = 'stelle-2026-11.jsonl';
  const PSP = `ps_${'1'.repeat(32)}`;
  const PSE = `ps_${'a'.repeat(32)}`;
  const DIG = 'd'.repeat(64);
  const sch = (ueber = {}) => ({ aktionsname: 'mcp.schreibaufruf', anfrage: 'mf-1795179540000-1', digest: DIG, entscheidend: PSE, ergebnis: 'eingeloest', person: PSP, server: 'tickets', sitzung: 's-1', werkzeug: 'update', zeit_anfrage: zeitText(T0 - 7000), zeit_entscheidung: zeitText(T0 - 3000), ...ueber });
  const sg = (ueber = {}) => ({ aktionsname: 'sitzung.gestartet', ergebnis: 'gestartet', grund: null, harness: 'claude', kunde: 'k-beispiel', route: 'cloud-beispiel', sitzung: 's-1', ...ueber });
  const sa = (ueber = {}) => ({ aktionsname: 'sitzung.abgelehnt', ergebnis: 'abgelehnt', grund: 'start.route_unbekannt', harness: 'claude', kunde: 'k-beispiel', route: 'cloud-fremd', sitzung: 's-1', ...ueber });
  const va = (ueber = {}) => ({ aktionsname: 'vorgang.abgelehnt', anfrage: 'mf-1795179540000-1', aufrufer: 'hub', ergebnis: 'abgelehnt', grund: 'stelle.bereits_eingeloest', vorgang: 'mcp.einloesen', ...ueber });
  const ohne = (objekt, feld) => { const o = { ...objekt }; delete o[feld]; return o; };
  const sFall = (fall, dateien, absicht) => pFall(fall, dateien, ['pruefe', `@${SD}`], absicht, stellePruefe);
  const gruen = (zeilen) => ({ exit: 0, ok: true, zeilen: zeilen.length, bruch: null, letzte: sha(zeilen[zeilen.length - 1]) });
  const rot = (zeile, grund) => br(SD, zeile, grund, zeile - 1);
  const einzeln = (fall, objekt, grund) => {
    const z = jcs({ ...objekt, fassung: 2, vorher: NULL64, zeit: zeitText(T0) });
    sFall(fall, [datei(SD, [z])], grund === null ? gruen([z]) : rot(1, grund));
  };
  // mcp.schreibaufruf: gueltig, Zusatzfeld, fehlendes Feld
  einzeln('schreibaufruf eingeloest mit Entscheidendem: gueltig', sch(), null);
  einzeln('schreibaufruf mit Zusatzfeld vorschau: Form falsch', { ...sch(), vorschau: 'Inhalt der Notiz' }, 'form_falsch');
  einzeln('schreibaufruf ohne Feld digest: Form falsch', ohne(sch(), 'digest'), 'form_falsch');
  // weitere Formen und Kopplungen von mcp.schreibaufruf
  einzeln('schreibaufruf mit Zusatzfeld argumente: Form falsch', { ...sch(), argumente: { titel: 'Notiz' } }, 'form_falsch');
  einzeln('schreibaufruf mit Zusatzfeld pfad: Form falsch', { ...sch(), pfad: '/srv/beispiel/notizen' }, 'form_falsch');
  einzeln('schreibaufruf ohne Feld person: Form falsch', ohne(sch(), 'person'), 'form_falsch');
  einzeln('schreibaufruf ohne Feld zeit_entscheidung: Form falsch', ohne(sch(), 'zeit_entscheidung'), 'form_falsch');
  einzeln('schreibaufruf mit person als Name: Form falsch', sch({ person: 'anna' }), 'form_falsch');
  einzeln('schreibaufruf mit entscheidend als Name: Form falsch', sch({ entscheidend: 'anna' }), 'form_falsch');
  einzeln('schreibaufruf abgelehnt mit Entscheidendem: gueltig', sch({ ergebnis: 'abgelehnt' }), null);
  einzeln('schreibaufruf abgelehnt ohne Entscheidenden: Form falsch', sch({ ergebnis: 'abgelehnt', entscheidend: null, zeit_entscheidung: null }), 'form_falsch');
  einzeln('schreibaufruf ohne_freigabe ohne Entscheidenden: gueltig', sch({ ergebnis: 'ohne_freigabe', entscheidend: null, zeit_entscheidung: null }), null);
  einzeln('schreibaufruf ohne_freigabe mit Entscheidendem: Form falsch', sch({ ergebnis: 'ohne_freigabe' }), 'form_falsch');
  einzeln('schreibaufruf abgelaufen nach Erteilung: gueltig', sch({ ergebnis: 'abgelaufen' }), null);
  einzeln('schreibaufruf abgelaufen ohne Entscheidung: gueltig', sch({ ergebnis: 'abgelaufen', entscheidend: null, zeit_entscheidung: null }), null);
  einzeln('schreibaufruf mit Entscheidendem ohne Zeit der Entscheidung: Form falsch', sch({ zeit_entscheidung: null }), 'form_falsch');
  einzeln('schreibaufruf mit Zeit der Entscheidung ohne Entscheidenden: Form falsch', sch({ entscheidend: null }), 'form_falsch');
  einzeln('schreibaufruf mit ergebnis ausserhalb der Liste: Form falsch', sch({ ergebnis: 'erteilt' }), 'form_falsch');
  einzeln('schreibaufruf mit Anfragekennung ohne Praefix: Form falsch', sch({ anfrage: '1795179540000-1' }), 'form_falsch');
  einzeln('schreibaufruf mit Anfragekennung null: Form falsch', sch({ anfrage: null }), 'form_falsch');
  einzeln('schreibaufruf mit digest in Grossbuchstaben: Form falsch', sch({ digest: 'D'.repeat(64) }), 'form_falsch');
  einzeln('schreibaufruf mit werkzeug als Pfad: Form falsch', sch({ werkzeug: '/srv/beispiel/werkzeug' }), 'form_falsch');
  einzeln('schreibaufruf mit werkzeug in Grossbuchstaben: gueltig', sch({ werkzeug: 'Update' }), null);
  einzeln('schreibaufruf mit zeit_anfrage ohne Millisekunden: Form falsch', sch({ zeit_anfrage: '2026-11-20T13:00:01Z' }), 'form_falsch');
  // sitzung.gestartet: gueltig, Zusatzfeld, fehlendes Feld
  einzeln('sitzung.gestartet mit allen vier Werten: gueltig', sg(), null);
  einzeln('sitzung.gestartet mit Zusatzfeld vorschau: Form falsch', { ...sg(), vorschau: 'erste Antwort' }, 'form_falsch');
  einzeln('sitzung.gestartet ohne Feld route: Form falsch', ohne(sg(), 'route'), 'form_falsch');
  einzeln('sitzung.gestartet mit Grund: Form falsch', sg({ grund: 'start.route_unbekannt' }), 'form_falsch');
  einzeln('sitzung.gestartet mit allen vier Werten null: gueltig', sg({ harness: null, kunde: null, route: null, sitzung: null }), null);
  einzeln('sitzung.gestartet mit Kunde als Anzeigename: Form falsch', sg({ kunde: 'Beispiel GmbH' }), 'form_falsch');
  // sitzung.abgelehnt: gueltig, Zusatzfeld, fehlendes Feld
  einzeln('sitzung.abgelehnt mit Code der Ablehnung: gueltig', sa(), null);
  einzeln('sitzung.abgelehnt mit Zusatzfeld argumente: Form falsch', { ...sa(), argumente: { route: 'cloud-fremd' } }, 'form_falsch');
  einzeln('sitzung.abgelehnt ohne Feld grund: Form falsch', ohne(sa(), 'grund'), 'form_falsch');
  einzeln('sitzung.abgelehnt mit Grund null: Form falsch', sa({ grund: null }), 'form_falsch');
  einzeln('sitzung.abgelehnt mit stelle-Code und vier Werten null: gueltig', sa({ grund: 'stelle.sitzung_vergeben', harness: null, kunde: null, route: null, sitzung: null }), null);
  einzeln('sitzung.abgelehnt mit ergebnis gestartet: Form falsch', sa({ ergebnis: 'gestartet' }), 'form_falsch');
  einzeln('sitzung.abgelehnt mit Zusatzfeld aufrufer: Form falsch', { ...sa(), aufrufer: 'agent' }, 'form_falsch');
  // vorgang.abgelehnt: gueltig, Zusatzfeld, fehlendes Feld
  einzeln('vorgang.abgelehnt mit Anfragekennung: gueltig', va(), null);
  einzeln('vorgang.abgelehnt mit Zusatzfeld argumente: Form falsch', { ...va(), argumente: { id: 7 } }, 'form_falsch');
  einzeln('vorgang.abgelehnt ohne Feld aufrufer: Form falsch', ohne(va(), 'aufrufer'), 'form_falsch');
  einzeln('vorgang.abgelehnt mit Zusatzfeld vorschau: Form falsch', { ...va(), vorschau: 'Inhalt' }, 'form_falsch');
  einzeln('vorgang.abgelehnt mit Anfragekennung null: gueltig', va({ anfrage: null }), null);
  einzeln('vorgang.abgelehnt mit unbekanntem Aufrufer: gueltig', va({ aufrufer: 'unbekannt' }), null);
  einzeln('vorgang.abgelehnt mit Aufrufer als Name: Form falsch', va({ aufrufer: 'anna' }), 'form_falsch');
  einzeln('vorgang.abgelehnt mit Vorgang sitzung.starten: Form falsch', va({ vorgang: 'sitzung.starten' }), 'form_falsch');
  einzeln('vorgang.abgelehnt mit Vorgang ablauf.pruefen: gueltig', va({ vorgang: 'ablauf.pruefen', anfrage: null }), null);
  einzeln('vorgang.abgelehnt mit Grund null: Form falsch', va({ grund: null }), 'form_falsch');
  // eine Kette aus allen vier Arten
  const z1 = zeileAllgemein(sg(), T0, NULL64);
  const z2 = zeileAllgemein(va(), T0 + 1000, sha(z1));
  const z3 = zeileAllgemein(sch(), T0 + 2000, sha(z2));
  const z4 = zeileAllgemein(sa(), T0 + 3000, sha(z3));
  sFall('Kette aus allen vier Zeilenarten der Stelle: gruen', [datei(SD, [z1, z2, z3, z4])], gruen([z1, z2, z3, z4]));
  // Fassung
  const ohneF = (() => { const o = JSON.parse(zeileAllgemein(sg(), T0, NULL64)); delete o.fassung; return jcs(o); })();
  sFall('sitzung.gestartet ohne fassung in einer stelle-Datei: fassung fehlt', [datei(SD, [ohneF])], rot(1, 'fassung_fehlt'));
  pFall('sitzung.gestartet ohne fassung in einer gateway-Datei (1g): Form falsch', [datei('gateway-2026-11.jsonl', [ohneF])], ['pruefe', '@gateway-2026-11.jsonl'], br('gateway-2026-11.jsonl', 1, 'form_falsch', 0), stellePruefe);
  const schZ = jcs({ ...sch(), fassung: 2, vorher: NULL64, zeit: zeitText(T0) });
  sFall('schreibaufruf mit fassung 3: Form falsch', [datei(SD, [jcs({ ...JSON.parse(schZ), fassung: 3 })])], rot(1, 'form_falsch'));
  // allgemeines Anhaengen: dieselbe Form, Quelle stelle
  const zl = (objekt, absicht) => ({ art: 'zeile', quelle: 'stelle', zeit_ms: NOV, objekt, absicht });
  mFall('allgemeines Anhaengen: mcp.schreibaufruf nur in der geschlossenen Form', {}, [zl({ ...sch(), vorschau: 'Inhalt der Notiz' }, 'feld_ungueltig:zeile'), zl(ohne(sch(), 'digest'), 'feld_ungueltig:zeile'), zl(sch(), 'ok')], ['stelle']);
  stelleAnhaengen.push(monateFaelle.pop());
  mFall('allgemeines Anhaengen: die drei Zeilenarten der Sitzung und des Vorgangs', {}, [zl({ ...va(), argumente: { id: 7 } }, 'feld_ungueltig:zeile'), zl(sg({ grund: 'start.route_unbekannt' }), 'feld_ungueltig:zeile'), zl(sg(), 'ok'), zl(sa(), 'ok'), zl(va(), 'ok')], ['stelle']);
  stelleAnhaengen.push(monateFaelle.pop());
}

// --- Fälle: letzte Zeile einer Art (Abschnitt 5.7) ---

const letzteFaelle = [];
function letzteZeile(dateien, ordnerName, quelle, filter, vorher) {
  if (!QUELLEN.includes(quelle)) return { fehler: 'feld_ungueltig:quelle' };
  if (!HEX.test(vorher)) return { fehler: 'feld_ungueltig:vorher' };
  const r = pruefeQuelle(dateien, ordnerName, quelle, { vorher });
  if (!r.ok) return { fehler: `kette_bruch:${r.bruch.grund}`, datei: r.bruch.datei, zeile: r.bruch.zeile };
  const muster = monatMuster(quelle);
  const monate = (dateien ?? []).filter((d) => !d.fehlt && muster.test(d.name)).sort((a, b) => (a.name < b.name ? -1 : 1));
  let gefunden = null;
  for (const d of monate) {
    for (const roh of zeilenVon(bytesVon(d).toString('utf8'))) {
      const w = JSON.parse(roh);
      if (Object.entries(filter).every(([k, v]) => has(w, k) && JSON.stringify(w[k]) === JSON.stringify(v))) gefunden = { zeile: w, sha256: sha(roh) };
    }
  }
  return gefunden ?? { keine: true };
}
{
  const regel = (ebene, n) => ({ aktionsname: 'regel.geaendert', ergebnis: 'ok', ebene, inhalt_sha256: sha(`regel${n}`) });
  const gesp = (n, ms) => ({ ...E1, zeit_ms: ms, anzahl: n });
  const OKT = Date.UTC(2026, 9, 20);
  // Zeilen einer Kette aus Objekten (allgemein) und Gesperrt-Ereignissen: [ms, objekt | { gesperrt: e }]
  const kette = (eintraege, vorher = NULL64) => {
    const zeilen = [];
    let v = vorher;
    for (const [ms, o] of eintraege) {
      const z = o.gesperrt ? zeileGesperrt(o.gesperrt, v) : zeileAllgemein(o, ms, v);
      zeilen.push(z);
      v = sha(z);
    }
    return { zeilen, letzte: v };
  };
  const okt = [[OKT, regel('firma', 1)], [OKT + 1000, regel('person', 2)], [OKT + 2000, { gesperrt: gesp(7, OKT + 2000) }]];
  const nov = [[NOV, regel('firma', 3)], [NOV + 1000, { gesperrt: gesp(8, NOV + 1000) }], [NOV + 2000, regel('person', 4)]];
  const dez = [[DEZ, { gesperrt: gesp(9, DEZ) }], [DEZ + 1000, { gesperrt: { ...gesp(10, DEZ + 1000), kunde: null } }]];
  const kOkt = kette(okt);
  const kNov = kette(nov, kOkt.letzte);
  const kDez = kette(dez, kNov.letzte);
  const aOkt = jcs(ankerWerte(OKTD, 'brain', text(kOkt.zeilen)));
  const aNov = jcs(ankerWerte(NOVD, 'brain', text(kNov.zeilen)));
  const alle = () => [datei(OKTD, kOkt.zeilen), datei(NOVD, kNov.zeilen), datei(DEZD, kDez.zeilen), datei(A, [aOkt, aNov])];
  const lFall = (fall, dateien, filter, absicht, opt = {}) => {
    const quelle = opt.quelle ?? 'brain';
    const vorher = opt.vorher ?? NULL64;
    const ordner = opt.ordner ?? '@@';
    const erg = letzteZeile(ordner === '@@' ? dateien : null, ordner.replace(/^@/, ''), quelle, filter, vorher);
    const ist = erg.keine ? 'keine' : erg.fehler ?? 'zeile';
    if (ist !== absicht) throw new Error(`letzte-zeile ${fall}: ${ist}, Absicht ${absicht}`);
    letzteFaelle.push({ fall, dateien, ordner, quelle, vorher, filter, erwartet: erg });
  };
  const sucheRegel = { aktionsname: 'regel.geaendert', ebene: 'firma' };
  lFall('letzte regel.geaendert der Ebene firma: im November, ueber den Dezember zurueck', alle(), sucheRegel, 'zeile');
  const e1 = letzteZeile(alle(), 'o', 'brain', sucheRegel, NULL64);
  if (JSON.stringify(e1.zeile.inhalt_sha256) !== JSON.stringify(sha('regel3'))) throw new Error('falsche Regelzeile');
  lFall('letzte regel.geaendert der Ebene person: die letzte Zeile des Novembers', alle(), { aktionsname: 'regel.geaendert', ebene: 'person' }, 'zeile');
  lFall('Treffer nur im Oktober, ueber zwei Monate zurueck', alle(), { aktionsname: 'regel.geaendert', ebene: 'firma', inhalt_sha256: sha('regel1') }, 'zeile');
  lFall('kein Treffer: Ebene abteilung', alle(), { aktionsname: 'regel.geaendert', ebene: 'abteilung' }, 'keine');
  lFall('kein Treffer: das Feld kommt in keiner Zeile vor', alle(), { gibt_es_nicht: 'x' }, 'keine');
  lFall('Filter mit Zahl: die letzte gesperrt-Zeile mit anzahl 8', alle(), { aktionsname: 'ausgabe.gesperrt', anzahl: 8 }, 'zeile');
  lFall('Filter mit null: die letzte Zeile mit kunde null', alle(), { kunde: null }, 'zeile');
  lFall('Filter mit zwei Bedingungen, eine weicht ab: faellt auf die fruehere Zeile zurueck', alle(), { aktionsname: 'ausgabe.gesperrt', kunde: 'k-beispiel' }, 'zeile');
  lFall('leerer Filter: die letzte Zeile der Kette', alle(), {}, 'zeile');
  lFall('leerer Ordner: keine', [], sucheRegel, 'keine');
  lFall('nur die Ankerdatei, keine Monatsdatei: der Anker nennt eine fehlende Datei', [datei(A, [aOkt])], sucheRegel, 'kette_bruch:anker_ohne_datei');
  // Bruch: nichts wird geliefert, auch wenn eine Zeile passt.
  lFall('Kette gebrochen im Oktober (Zeile 2 geaendert): Fehler', [datei(OKTD, [kOkt.zeilen[0], kOkt.zeilen[1].replace(sha('regel2'), sha('regelX')), kOkt.zeilen[2]]), datei(NOVD, kNov.zeilen), datei(DEZD, kDez.zeilen), datei(A, [aOkt, aNov])], sucheRegel, 'kette_bruch:kette_gebrochen');
  lFall('Anker passt nicht (Oktober neu verkettet): Fehler', (() => {
    const o2 = kette([[OKT, regel('firma', 99)], okt[1], okt[2]]);
    const n2 = kette(nov, o2.letzte);
    const d2 = kette(dez, n2.letzte);
    return [datei(OKTD, o2.zeilen), datei(NOVD, n2.zeilen), datei(DEZD, d2.zeilen), datei(A, [aOkt, aNov])];
  })(), sucheRegel, 'kette_bruch:anker_falsch');
  lFall('Bruch nach dem Treffer (letzte Zeile des Dezembers kaputt): Fehler, nicht der Treffer', [datei(OKTD, kOkt.zeilen), datei(NOVD, kNov.zeilen), datei(DEZD, [kDez.zeilen[0], 'kaputt']), datei(A, [aOkt, aNov])], sucheRegel, 'kette_bruch:zeile_unlesbar');
  lFall('geschlossener Monat ohne Anker: Fehler', [datei(OKTD, kOkt.zeilen), datei(NOVD, kNov.zeilen), datei(DEZD, kDez.zeilen), datei(A, [aOkt])], sucheRegel, 'kette_bruch:anker_fehlt');
  lFall('letzte Zeile ohne LF: Fehler', [datei(NOVD, kNov.zeilen, '')], sucheRegel, 'kette_bruch:letzte_zeile_unvollstaendig', { vorher: kOkt.letzte });
  lFall('Ordner fehlt: Fehler mit dem Namen des Ordners', [], sucheRegel, 'kette_bruch:datei_nicht_lesbar', { ordner: '@weg' });
  // vorher
  lFall('Ordner beginnt mitten in der Kette: mit --vorher gueltig', [datei(NOVD, kNov.zeilen), datei(DEZD, kDez.zeilen), datei(A, [aNov])], sucheRegel, 'zeile', { vorher: kOkt.letzte });
  lFall('Ordner beginnt mitten in der Kette: ohne vorher ein Bruch', [datei(NOVD, kNov.zeilen), datei(DEZD, kDez.zeilen), datei(A, [aNov])], sucheRegel, 'kette_bruch:kette_gebrochen');
  lFall('Quelle falsch', alle(), sucheRegel, 'feld_ungueltig:quelle', { quelle: 'audit' });
  lFall('vorher falsch', alle(), sucheRegel, 'feld_ungueltig:vorher', { vorher: 'abc' });
  lFall('Quelle vor vorher', alle(), sucheRegel, 'feld_ungueltig:quelle', { quelle: 'audit', vorher: 'abc' });
  lFall('andere Quelle im selben Ordner: stelle hat keine Zeile', alle(), sucheRegel, 'keine', { quelle: 'stelle' });
  // altes Gateway (Fassung 1g)
  {
    const gateway = k1.slice(0, 4).map((v) => v.zeile);
    lFall('Gateway (Fassung 1g): die letzte Zeile sitzung.abgemeldet', [datei('gateway-2026-10.jsonl', gateway.slice(0, 2)), datei('gateway-2026-11.jsonl', gateway.slice(2)), datei('gateway.anker.jsonl', [ketteVek.find((v) => v.art === 'anker').anker])], { aktionsname: 'sitzung.abgemeldet' }, 'zeile', { quelle: 'gateway' });
  }
}

if (abweichungen.length) throw new Error(abweichungen.join('\n'));

// --- schreiben ---

// Selbstprobe: was anhaengen schreibt, prueft gruen.
const drei = anhaengenFaelle.find((f) => f.fall.startsWith('drei Aufrufe'));
if (drei.ende !== `${L1}\n${L2}\n${L3}\n`) throw new Error('anhaengen ergibt nicht L1, L2, L3');
if (!pruefe([{ name: BRAIN, inhalt: drei.ende }]).ok) throw new Error('Selbstprobe: geschriebene Kette prueft nicht gruen');

const KOPF = 'Vertrag 5 (rust/spec/protokollzeile/README.md). Erzeugt von bau-faelle.mjs; nicht von Hand aendern.';
mkdirSync(AUS, { recursive: true });
const schreibe = (name, inhalt) => writeFileSync(join(AUS, name), `${JSON.stringify({ quelle: KOPF, ...inhalt }, null, 2)}\n`);
schreibe('kanonisch.json', { faelle: kanonischFaelle });
schreibe('zeile.json', { zeile: zeileFaelle, zeit_text: zeitFaelle, abschnitt: abschnittFaelle });
schreibe('anhaengen.json', { faelle: anhaengenFaelle, zwei_schreiber: zweiSchreiber });
schreibe('pruefe.json', { faelle: pruefeFaelle });
schreibe('monate.json', { faelle: monateFaelle, zwei_schreiber_monatsgrenze: zweiSchreiberGrenze });
schreibe('pruefe-quelle.json', { faelle: quelleFaelle });
schreibe('abgewiesen.json', { zeile: abgZeile, pruefe: abgPruefe, anhaengen: abgAnhaengen });
schreibe('probelauf.json', { zeile: probeZeile, pruefe: probePruefe, anhaengen: probeAnhaengen });
schreibe('ordnermarke.json', { zeile: ordZeile, pruefe: ordPruefe, anhaengen: ordAnhaengen });
schreibe('personenzeilen.json', { pruefe: persPruefe, anhaengen: persAnhaengen });
schreibe('stellezeilen.json', { pruefe: stellePruefe, anhaengen: stelleAnhaengen });
schreibe('letzte-zeile.json', { faelle: letzteFaelle });
process.stdout.write(`kanonisch ${kanonischFaelle.length}, zeile ${zeileFaelle.length}, zeit_text ${zeitFaelle.length}, abschnitt ${abschnittFaelle.length}, anhaengen ${anhaengenFaelle.length}+1, pruefe ${pruefeFaelle.length}, monate ${monateFaelle.length}+1, pruefe-quelle ${quelleFaelle.length}, abgewiesen zeile ${abgZeile.length} pruefe ${abgPruefe.length} anhaengen ${abgAnhaengen.length}, probelauf zeile ${probeZeile.length} pruefe ${probePruefe.length} anhaengen ${probeAnhaengen.length}, ordnermarke zeile ${ordZeile.length} pruefe ${ordPruefe.length} anhaengen ${ordAnhaengen.length}, personenzeilen pruefe ${persPruefe.length} anhaengen ${persAnhaengen.length}, stellezeilen pruefe ${stellePruefe.length} anhaengen ${stelleAnhaengen.length}, letzte-zeile ${letzteFaelle.length}\n`);

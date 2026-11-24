import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { MobileAuthError, MobileAuthenticator } from './auth';
import { ALLOWED_SPECIAL_KEYS, MAX_MOBILE_KEY_SEQUENCE, MOBILE_API_PREFIX } from '../../../mobile-shared/vertrag';
import type { SignedRequestEnvelope } from '../../../mobile-shared/vertrag';
import { canonicalPath } from '../../../mobile-shared/kanonisch';
import { RIGHTS_DECISION_NOTE_MAX } from '../../../mobile-shared/vertrag';
import { aenderungSha256, RECHTEANTRAG_AKTION } from './rechteantrag-beleg';

type PaneExpected = {
  aktion: 'mobil-senden' | 'mobil-taste';
  maschine: string;
  socket: string;
  pane: string;
};

/**
 * DER GERAETEBELEG AN DER PRUEFSTELLE (14.10.2026, Entscheidung des Nutzers „Annahme am
 * Handy und Guard angleichen: ja"). `wb-freigabe erteilen --mobilbeleg` und
 * `wb-decide --mobilbeleg` lassen den signierten Umschlag der Entscheidung hier selbst
 * pruefen und verbrauchen, statt dem Kern zu glauben. Gebunden wird an das, was das
 * Geraet unterschrieben hat: Methode, Pfad (er traegt die Kennung des Eintrags, wie
 * adapters/approvals.ts sie bildet) und den Entscheidungsbody. Die Pruefstelle nennt
 * ihre Erwartung selbst; passt sie nicht, wird nichts verbraucht.
 */
type GuardExpected = { aktion: 'guard-freigabe'; schluessel: string };
type AntragExpected = { aktion: 'antrag-entscheiden'; antrag: string; inhalt_sha256: string; entscheidung: 'approve' | 'reject' };
/**
 * Rechteantrag einer Welt (18.10.2026): gebunden an `POST /agents/actions` und einen Body
 * `rights_request.decide` mit genau dieser Welt, dieser Antragskennung, dieser Entscheidung
 * und der gespeicherten Aenderung (SHA-256 ihrer kanonischen JSON-Form). Der Kern nennt die
 * Erwartung aus seinem frischen Stand; die Bemerkung gibt das Programm so zurueck, wie das
 * Geraet sie unterschrieben hat.
 */
type RechteExpected = { aktion: typeof RECHTEANTRAG_AKTION; welt_id: string; antrag: string; changes_sha256: string; entscheidung: 'approve' | 'reject' };
type Expected = PaneExpected | GuardExpected | AntragExpected | RechteExpected;

type CliInput = {
  umschlag: SignedRequestEnvelope;
  body_b64: string;
  erwartet: Expected;
};

function failure(code: string): never {
  process.stdout.write(`${JSON.stringify({ ok: false, code })}\n`);
  process.exitCode = 1;
  throw new Error('CLI_FAILURE_REPORTED');
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseInput(value: unknown): CliInput {
  if (!isObject(value) || !exactKeys(value, ['body_b64', 'erwartet', 'umschlag']) || !isObject(value.umschlag)
    || typeof value.body_b64 !== 'string' || !isObject(value.erwartet)) failure('bad_request');
  const expected = value.erwartet;
  if (expected.aktion === 'guard-freigabe') {
    if (!exactKeys(expected, ['aktion', 'schluessel']) || typeof expected.schluessel !== 'string'
      || !/^[a-f0-9]{64}$/.test(expected.schluessel)) failure('bad_request');
    return { umschlag: value.umschlag as unknown as SignedRequestEnvelope, body_b64: value.body_b64, erwartet: expected as GuardExpected };
  }
  if (expected.aktion === 'antrag-entscheiden') {
    if (!exactKeys(expected, ['aktion', 'antrag', 'entscheidung', 'inhalt_sha256']) || typeof expected.antrag !== 'string' || !expected.antrag
      || typeof expected.inhalt_sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(expected.inhalt_sha256)
      || (expected.entscheidung !== 'approve' && expected.entscheidung !== 'reject')) failure('bad_request');
    return { umschlag: value.umschlag as unknown as SignedRequestEnvelope, body_b64: value.body_b64, erwartet: expected as AntragExpected };
  }
  if (expected.aktion === RECHTEANTRAG_AKTION) {
    if (!exactKeys(expected, ['aktion', 'antrag', 'changes_sha256', 'entscheidung', 'welt_id'])
      || typeof expected.welt_id !== 'string' || !expected.welt_id || typeof expected.antrag !== 'string' || !expected.antrag
      || typeof expected.changes_sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(expected.changes_sha256)
      || (expected.entscheidung !== 'approve' && expected.entscheidung !== 'reject')) failure('bad_request');
    return { umschlag: value.umschlag as unknown as SignedRequestEnvelope, body_b64: value.body_b64, erwartet: expected as RechteExpected };
  }
  if (!exactKeys(expected, ['aktion', 'maschine', 'pane', 'socket']) || (expected.aktion !== 'mobil-senden' && expected.aktion !== 'mobil-taste')
    || typeof expected.maschine !== 'string' || !expected.maschine || typeof expected.socket !== 'string' || !expected.socket
    || typeof expected.pane !== 'string' || !/^%(0|[1-9][0-9]*)$/.test(expected.pane)) failure('bad_request');
  return { umschlag: value.umschlag as unknown as SignedRequestEnvelope, body_b64: value.body_b64, erwartet: expected as Expected };
}

function strictBase64(value: string): Buffer | undefined {
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) return undefined;
  const decoded = Buffer.from(value, 'base64');
  return decoded.toString('base64') === value ? decoded : undefined;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value).sort();
  return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
}

/**
 * Dieselbe Kennung wie adapters/approvals.ts `id()`: Art und SHA-256 des Schluessels bzw.
 * eines Antrags (Pfad, NUL, SHA-256 der Dateibytes; Review 14.10.2026, F3: ein Antrag, der
 * nach der Anzeige am Handy umgeschrieben wird, traegt eine andere Kennung).
 */
function eintragsKennung(art: 'guard' | 'request', wert: string): string {
  return `${art}:${createHash('sha256').update(wert).digest('hex')}`;
}

/** Bindet Methode, Pfad und Body einer mobilen Entscheidung an die Erwartung der Pruefstelle. */
function bindDecision(envelope: SignedRequestEnvelope, body: Buffer, expected: GuardExpected | AntragExpected): void {
  const pfad = expected.aktion === 'guard-freigabe'
    ? `${MOBILE_API_PREFIX}/approvals/guards/${encodeURIComponent(eintragsKennung('guard', expected.schluessel))}/decision`
    : `${MOBILE_API_PREFIX}/approvals/requests/${encodeURIComponent(eintragsKennung('request', `${expected.antrag}\0${expected.inhalt_sha256}`))}/decision`;
  let gebunden = false;
  try {
    gebunden = typeof envelope.method === 'string' && envelope.method.toUpperCase() === 'POST'
      && typeof envelope.path === 'string' && canonicalPath(envelope.path) === canonicalPath(pfad)
      && Array.isArray(envelope.query) && envelope.query.length === 0;
  } catch {
    gebunden = false;
  }
  if (!gebunden) failure('stale_target');
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString('utf8'));
  } catch {
    failure('bad_request');
  }
  if (!isObject(parsed) || !exactKeys(parsed, ['decision', 'reason']) || typeof parsed.reason !== 'string') failure('bad_request');
  const verlangt = expected.aktion === 'guard-freigabe' ? 'approve' : expected.entscheidung;
  if (parsed.decision !== verlangt) failure('stale_target');
}

/** Steuerzeichen (C0, DEL, C1) und Zeilen-/Absatztrenner: eine Bemerkung bleibt eine Zeile. */
const STEUERZEICHEN = /[\u0000-\u001f\u007f-\u009f\u2028\u2029]/;

/** Bindet eine Rechteantrags-Entscheidung; liefert die unterschriebene Bemerkung oder null. */
function bindRightsDecision(envelope: SignedRequestEnvelope, body: Buffer, expected: RechteExpected): { note: string | null } {
  let gebunden = false;
  try {
    gebunden = typeof envelope.method === 'string' && envelope.method.toUpperCase() === 'POST'
      && typeof envelope.path === 'string' && canonicalPath(envelope.path) === `${MOBILE_API_PREFIX}/agents/actions`
      && Array.isArray(envelope.query) && envelope.query.length === 0;
  } catch {
    gebunden = false;
  }
  if (!gebunden) failure('stale_target');
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString('utf8'));
  } catch {
    failure('bad_request');
  }
  if (!isObject(parsed)) failure('bad_request');
  const felder = ['changes', 'decision', 'rights_request_id', 'type', 'world_id'];
  if (!exactKeys(parsed, 'note' in parsed ? [...felder.slice(0, 2), 'note', ...felder.slice(2)] : felder)
    || parsed.type !== 'rights_request.decide' || !isObject(parsed.changes)) failure('bad_request');
  const note = parsed.note;
  if (note !== undefined && (typeof note !== 'string' || !note.trim() || [...note].length > RIGHTS_DECISION_NOTE_MAX
    || STEUERZEICHEN.test(note))) failure('bad_request');
  if (parsed.world_id !== expected.welt_id || parsed.rights_request_id !== expected.antrag
    || parsed.decision !== expected.entscheidung || aenderungSha256(parsed.changes) !== expected.changes_sha256) failure('stale_target');
  return { note: typeof note === 'string' ? note : null };
}

function bindBody(body: Buffer, expected: PaneExpected): { text_b64?: string; tasten?: string[] } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString('utf8'));
  } catch {
    failure('bad_request');
  }
  if (!isObject(parsed) || parsed.aktion !== expected.aktion || parsed.maschine !== expected.maschine
    || parsed.socket !== expected.socket || parsed.pane !== expected.pane) failure('stale_target');
  if (expected.aktion === 'mobil-senden') {
    if (!exactKeys(parsed, ['aktion', 'maschine', 'pane', 'socket', 'text_b64']) || typeof parsed.text_b64 !== 'string') failure('bad_request');
    const text = strictBase64(parsed.text_b64);
    if (!text || text.length === 0) failure('bad_request');
    return { text_b64: parsed.text_b64 };
  }
  // Die Obergrenze gehoert HIERHER und nicht nur in den Aufrufer (Sicherheits-
  // Review 2026-09-21): das Pruefprogramm ist die massgebliche Pruefung, und
  // ohne sie bestaetigte es eine beliebig lange Tastenfolge -- gemessen 500
  // Tasten in einem `mobil-taste`. Dieselbe Grenze wie Vertrag und `wb-pane-write`.
  if (!exactKeys(parsed, ['aktion', 'maschine', 'pane', 'socket', 'tasten']) || !Array.isArray(parsed.tasten)
    || parsed.tasten.length === 0 || parsed.tasten.length > MAX_MOBILE_KEY_SEQUENCE
    || !parsed.tasten.every((key) => typeof key === 'string' && ALLOWED_SPECIAL_KEYS.includes(key as never))) failure('bad_request');
  return { tasten: [...parsed.tasten] as string[] };
}

export function main(): number {
  try {
    const raw = readFileSync(0, 'utf8');
    const input = parseInput(JSON.parse(raw));
    const body = strictBase64(input.body_b64);
    if (!body) failure('bad_request');
    const erwartet = input.erwartet;
    const output = erwartet.aktion === 'guard-freigabe' || erwartet.aktion === 'antrag-entscheiden'
      ? (bindDecision(input.umschlag, body, erwartet), {})
      : erwartet.aktion === RECHTEANTRAG_AKTION ? bindRightsDecision(input.umschlag, body, erwartet) : bindBody(body, erwartet);
    const authenticator = new MobileAuthenticator();
    const request = authenticator.verifyAndReserve(input.umschlag, { audience: input.umschlag.audience, body });
    try {
      authenticator.consume(request, input.umschlag.audience);
    } catch (error) {
      authenticator.fail(request, input.umschlag.audience);
      throw error;
    }
    process.stdout.write(`${JSON.stringify({ ok: true, geraet: request.device_id, request_id: request.request_id, aktion: input.erwartet.aktion, ...output })}\n`);
    return 0;
  } catch (error) {
    if (error instanceof MobileAuthError) {
      process.stdout.write(`${JSON.stringify({ ok: false, code: error.code })}\n`);
      return 1;
    }
    if (error instanceof Error && error.message === 'CLI_FAILURE_REPORTED') return 1;
    process.stdout.write(`${JSON.stringify({ ok: false, code: 'bad_request' })}\n`);
    return 1;
  }
}

if (require.main === module) process.exitCode = main();

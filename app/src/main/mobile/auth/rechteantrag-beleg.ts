// DER GERAETEBELEG FUER EINEN RECHTEANTRAG: ein Rechteantrag laesst sich auch vom Handy
// entscheiden, unter den folgenden Bedingungen. Der Kern entscheidet einen
// Rechteantrag ueber `welt:rechte_antrag_entscheiden` als `mensch` mit M2 (welten.ts).
// Fuer eine Entscheidung vom Handy gibt es M2 erst, nachdem das feste Pruefprogramm
// `wb-mobil-pruefen` (Heimat laut Benutzerdatenbank, nie $HOME) den signierten Umschlag
// selbst geprueft, an Welt, Antragskennung, gespeicherte Aenderung und Entscheidung
// gebunden und verbraucht hat. Scheitert das, wird nicht entschieden; es gibt keinen
// Rueckfall auf eine andere Messung.
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { userInfo } from 'node:os';
import { join } from 'node:path';
import type { MobilNachweis } from '../../herkunft';

export const RECHTEANTRAG_AKTION = 'rechteantrag-entscheiden';

export interface RechteantragErwartung {
  readonly welt_id: string;
  readonly antrag: string;
  /** SHA-256 der kanonischen JSON-Form der gespeicherten Aenderung (`WeltRechteAntrag.changes`). */
  readonly changes_sha256: string;
  readonly entscheidung: 'approve' | 'reject';
}

export type RechteantragPruefung =
  | { readonly ok: true; readonly geraet: string; readonly request_id: string; readonly note: string | null }
  | { readonly ok: false; readonly code: string };

/** JSON mit sortierten Schluesseln: Handy und Pruefstelle hashen dieselbe Aenderung gleich. */
export function kanonischesJson(wert: unknown): string {
  if (Array.isArray(wert)) return `[${wert.map(kanonischesJson).join(',')}]`;
  if (wert && typeof wert === 'object') {
    const eintraege = Object.entries(wert as Record<string, unknown>).filter(([, v]) => v !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${eintraege.map(([k, v]) => `${JSON.stringify(k)}:${kanonischesJson(v)}`).join(',')}}`;
  }
  return JSON.stringify(wert ?? null);
}

export function aenderungSha256(changes: unknown): string {
  return createHash('sha256').update(kanonischesJson(changes), 'utf8').digest('hex');
}

/** Die Heimat laut Benutzerdatenbank, wie `wb-decide` (WB_MOBIL_HEIM); nie aus $HOME. */
export function mobilHeim(): string {
  return userInfo().homedir;
}

const CODE = /^[a-z_]{1,40}$/;

/**
 * Laesst `<heim>/.local/bin/wb-mobil-pruefen` den Nachweis gegen die Kopplungsliste in
 * `<heim>` pruefen und verbrauchen. Feste Umgebung wie bei `wb-decide --mobilbeleg`.
 */
export function rechteantragBelegPruefen(heim: string, nachweis: MobilNachweis, erwartet: RechteantragErwartung,
  fristMs = 15_000): Promise<RechteantragPruefung> {
  const pruefer = join(heim, '.local', 'bin', 'wb-mobil-pruefen');
  const eingabe = JSON.stringify({ umschlag: nachweis.umschlag, body_b64: nachweis.body.toString('base64'),
    erwartet: { aktion: RECHTEANTRAG_AKTION, ...erwartet } });
  return new Promise((resolve) => {
    let fertig = false;
    const ende = (ergebnis: RechteantragPruefung): void => {
      if (fertig) return;
      fertig = true;
      clearTimeout(uhr);
      resolve(ergebnis);
    };
    let kind: ReturnType<typeof spawn>;
    try {
      kind = spawn(pruefer, [], { env: { HOME: heim, TMPDIR: '/tmp', PATH: '/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin' },
        stdio: ['pipe', 'pipe', 'ignore'] });
    } catch {
      resolve({ ok: false, code: 'capability_unavailable' });
      return;
    }
    const uhr = setTimeout(() => {
      kind.kill('SIGKILL');
      ende({ ok: false, code: 'internal_error' });
    }, fristMs);
    let aus = '';
    kind.stdout?.setEncoding('utf8');
    kind.stdout?.on('data', (teil: string) => { if (aus.length < 65_536) aus += teil; });
    kind.on('error', (fehler: NodeJS.ErrnoException) => ende({ ok: false, code: fehler.code === 'ENOENT' || fehler.code === 'EACCES' ? 'capability_unavailable' : 'internal_error' }));
    kind.on('close', (status) => {
      let antwort: Record<string, unknown> | null = null;
      try {
        const zeile = aus.trim().split('\n').at(-1) ?? '';
        const wert = JSON.parse(zeile) as unknown;
        antwort = wert && typeof wert === 'object' && !Array.isArray(wert) ? wert as Record<string, unknown> : null;
      } catch {
        antwort = null;
      }
      if (status === 0 && antwort?.ok === true && antwort.aktion === RECHTEANTRAG_AKTION
        && typeof antwort.geraet === 'string' && antwort.geraet && typeof antwort.request_id === 'string') {
        ende({ ok: true, geraet: antwort.geraet, request_id: antwort.request_id, note: typeof antwort.note === 'string' ? antwort.note : null });
        return;
      }
      const code = antwort?.ok === false && typeof antwort.code === 'string' && CODE.test(antwort.code) ? antwort.code : 'internal_error';
      ende({ ok: false, code });
    });
    kind.stdin?.on('error', () => { /* Ein frueh beendetes Pruefprogramm meldet sich ueber close. */ });
    kind.stdin?.end(eingabe);
  });
}

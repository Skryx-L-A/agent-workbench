// Der Pruefknopf der Maschinen-Seite.
//
// Bis heute stand die letzte Zeile von ssh im Fenster, und mit der Attrappe der
// Suiten sogar der ganze Aufruf: „not reachable: kein Weg zu -o BatchMode=yes -o
// ConnectTimeout=6 … true“. Das ist eine Kommandozeile, kein Satz fuer Menschen,
// und eine Fehlerzeile von ssh kann Pfade mit dem Nutzernamen tragen
// („Identity file /Users/<name>/.ssh/… not accessible“). Darum ordnet der Kern
// die Ausgabe einem FALL zu, und die Oberflaechen zeigen nur den Satz zu diesem
// Fall (`satz.maschinePruefen.<fall>` in einstellungen/texte.ts). Die rohe
// Ausgabe geht in das Protokoll des Kerns, nie ueber den Kanal.
import { spawn } from 'node:child_process';

export type PruefFall =
  | 'erreichbar'
  | 'keineAntwort'
  | 'name'
  | 'anmeldung'
  | 'hostschluessel'
  | 'verweigert'
  | 'keinSsh'
  | 'sonst';

/** Die Faelle, fuer die es einen Satz gibt -- alle ausser „erreichbar“. */
export const FEHLER_FAELLE: readonly PruefFall[] = [
  'keineAntwort', 'name', 'anmeldung', 'hostschluessel', 'verweigert', 'keinSsh', 'sonst',
];

/** Was ueber den Kanal geht: ein Wahrheitswert und eine Fallkennung, kein Text von ssh. */
export interface PruefAntwort {
  ok: boolean;
  fall: PruefFall;
}

/** Was nur der Kern sieht: dazu die rohe Ausgabe fuer sein Protokoll. */
export interface PruefErgebnis extends PruefAntwort {
  roh: string;
}

/**
 * Ordnet einen ssh-Lauf einem Fall zu. Gelesen wird die ganze Fehlerausgabe von
 * OpenSSH, nicht nur die letzte Zeile. Die Reihenfolge zaehlt: Ein geaenderter
 * Hostschluessel bringt eine lange Warnung, die auch andere Woerter enthaelt.
 *
 * `code === null` heisst: der Prozess wurde beendet, hier von der eigenen Frist.
 */
export function pruefFall(code: number | null, fehlerausgabe: string, startFehler?: { code?: string } | null): PruefFall {
  if (startFehler) return startFehler.code === 'ENOENT' ? 'keinSsh' : 'sonst';
  if (code === 0) return 'erreichbar';
  if (code === null) return 'keineAntwort';
  const s = fehlerausgabe.toLowerCase();
  if (/host key verification failed|remote host identification has changed|host key for .* has changed|no \S+ host key is known/.test(s)) return 'hostschluessel';
  if (/permission denied|too many authentication failures|no supported authentication methods|authentication failed/.test(s)) return 'anmeldung';
  if (/could not resolve hostname|name or service not known|nodename nor servname|temporary failure in name resolution|no address associated with hostname/.test(s)) return 'name';
  if (/connection refused/.test(s)) return 'verweigert';
  if (/timed out|no route to host|network is unreachable|host is down/.test(s)) return 'keineAntwort';
  return 'sonst';
}

/**
 * `ssh <alias> true` mit Zeitlimit, GENAU EIN Handgriff. Kein Netzzugriff von
 * selbst: dieser Weg entsteht nur, wenn jemand auf den Knopf drueckt.
 * BatchMode verhindert, dass eine Passwortfrage das Fenster stehen laesst,
 * denn eine Frage, die niemand sieht, ist ein Haenger.
 */
export function maschinePruefen(name: string, zeitMs: number): Promise<PruefErgebnis> {
  return new Promise((fertig) => {
    let fehler = '';
    let vorbei = false;
    const ende = (e: PruefErgebnis): void => {
      if (vorbei) return;
      vorbei = true;
      clearTimeout(zeit);
      fertig(e);
    };
    const p = spawn('ssh', [
      '-o', 'BatchMode=yes',
      '-o', `ConnectTimeout=${Math.max(1, Math.round(zeitMs / 1000))}`,
      name, 'true',
    ], { stdio: ['ignore', 'ignore', 'pipe'] });
    const zeit = setTimeout(() => p.kill('SIGKILL'), zeitMs + 2000);
    p.stderr?.on('data', (b: Buffer) => { fehler += b.toString(); });
    p.on('error', (e: NodeJS.ErrnoException) => {
      ende({ ok: false, fall: pruefFall(null, '', e), roh: String(e) });
    });
    p.on('close', (code) => {
      const fall = pruefFall(code, fehler);
      const kurz = fehler.trim().split('\n').pop() ?? '';
      ende({ ok: fall === 'erreichbar', fall, roh: kurz || `ssh endete mit ${code}` });
    });
  });
}

/** Nur Ok und Fall -- das, was eine Oberflaeche bekommen darf. */
export function antwortFuerOberflaeche(e: PruefErgebnis): PruefAntwort {
  return { ok: e.ok, fall: e.fall };
}

// Wann der RECHNER zuletzt gestartet ist (nicht das Programm) -- die eine
// Auskunft, an der `lebensspur.ts` einen Neustart von einem bloss geschlossenen
// Fenster unterscheidet (10.11.).
//
// Gemessen wird sie ueber das Betriebssystem, nicht ueber `os.uptime()`: unter
// macOS zaehlt Node dort die Zeit OHNE Schlaf, und ein Mac, der neun Stunden
// schlief, ergaebe einen Startzeitpunkt, der um neun Stunden zu spaet liegt --
// ein Fenster, das vor dem Schlaf geschlossen wurde, galte dann als vor einem
// Neustart geschlossen.
//
//   macOS  `sysctl -n kern.boottime` -> `{ sec = 1794427960, usec = 0 } Thu ...`
//          (Sekunden seit 1970; bleibt ueber Schlaf und Aufwachen gleich)
//   Linux  `btime` in /proc/stat      -> `btime 1794427960`
//   sonst  null
//
// `null` heisst „nicht lesbar", und der Aufrufer behandelt es als „kein
// Neustart bewiesen": lieber keine Fortsetzen-Meldung als eine falsche.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

/** Aus der Ausgabe von `sysctl -n kern.boottime` die Sekunden -- sonst null. */
export function bootzeitAusSysctl(text: string): number | null {
  const m = /\bsec\s*=\s*(\d{9,11})\b/.exec(text);
  return m ? Number(m[1]) * 1000 : null;
}

/** Aus dem Inhalt von /proc/stat die Zeile `btime` -- sonst null. */
export function bootzeitAusProcStat(text: string): number | null {
  const m = /^btime\s+(\d{9,11})\s*$/m.exec(text);
  return m ? Number(m[1]) * 1000 : null;
}

/**
 * Der Startzeitpunkt des Rechners in Millisekunden seit 1970, oder null, wenn er
 * sich nicht lesen laesst. Wirft nie.
 */
export function rechnerStart(plattform: NodeJS.Platform = process.platform): number | null {
  try {
    if (plattform === 'darwin') {
      const aus = execFileSync('/usr/sbin/sysctl', ['-n', 'kern.boottime'], {
        encoding: 'utf8',
        timeout: 3000,
        stdio: ['ignore', 'pipe', 'ignore'],
      });
      return bootzeitAusSysctl(aus);
    }
    if (plattform === 'linux') return bootzeitAusProcStat(readFileSync('/proc/stat', 'utf8'));
  } catch {
    // Nicht lesbar ist eine Auskunft, kein Fehler -- siehe oben.
  }
  return null;
}

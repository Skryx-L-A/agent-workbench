// TESTFENSTER NEHMEN WEDER FOKUS NOCH SICHT.
//
// Anlass: kopflose Suiten (test-fenster-spiegel-groesse, test-app-groessensprung,
// test-app-tab-ueberholt, test-app-tab-zweifenster) legten ein Fenster vor die
// Fenster des Menschen, obwohl das Hauptfenster mit show:false entsteht. Sie rufen
// `ctl fenster maximieren`, und `maximize()` zeigt unter macOS ein verstecktes
// Fenster. Die Regel: Tests duerfen laufen, solange sie dem Menschen am Mac nicht
// die Sicht auf seine Fenster nehmen.
//
// Die EINE Stelle dafuer ist hier, nicht je Fenster: unter `WB_TESTUMGEBUNG=1`
// (run-all, Testheimat; ein Live-Start setzt sie nie) behandelt
// `testfensterZaehmen` jedes BrowserWindow, sobald es entsteht
// ('browser-window-created' kommt synchron aus dem Konstruktor, vor jedem
// show): Deckkraft 0, klickdurchlaessig, `show()` wird `showInactive()`,
// `focus()`/`moveTop()` tun nichts, Deckkraft und Klickdurchlass lassen sich
// nicht zuruecksetzen. Dazu bleibt das Dock versteckt (die App ist "Zubehoer"
// und wird nicht aktiv), `app.dock.show()` und `app.focus()` tun nichts.
//
// Warum Deckkraft 0 und nicht neben die Bildschirme: `maximize()` legt das
// Fenster auf seinen Bildschirm zurueck. Rahmen, Inhaltsgroesse, Zoom und
// `capturePage()` haengen nicht an der Deckkraft; die Suiten messen genau
// diese und bleiben deshalb, was sie waren. Der Fokuswaechter
// (shell/tests/fokus-waechter.py) zaehlt ein Fenster mit Deckkraft 0 nicht als
// sichtbar -- so wie ein Mensch es nicht sieht.
//
// DAS HID-TOR IM STARTWEG (auch ein Direktlauf ohne run-all brachte ein Fenster
// in die Sicht des Menschen). Jeder Weg, auf dem
// ein Fenster der Testumgebung auf den Bildschirm kommt -- show(),
// showInactive(), maximize(), setFullScreen(true) --, fragt vorher den
// HID-Leerlauf: unter 600 s endet der Kern sofort mit "WARTET: Mensch am
// Rechner" und Exit 77, ohne das Fenster zu zeigen. Danach misst ein Begleiter
// jede Sekunde weiter und beendet den Kern bei jeder Eingabe (Leerlauf < 3 s)
// ebenso. Dieselben Schwellen wie shell/tests/lib-mensch-leerlauf.sh, dieselben
// Attrappen der Messung (WB_HID_LEERLAUF_S, WB_HID_LEERLAUF_DATEI).
//
// Typen nur als `import type`: das Modul laeuft in test-app-testfenster.sh
// ohne Electron gegen Attrappen.
import type { App, BrowserWindow } from 'electron';
import { execFileSync } from 'node:child_process';
import { closeSync, constants as fsKonstanten, fstatSync, openSync, readFileSync, readSync } from 'node:fs';
import { userInfo } from 'node:os';
import { join } from 'node:path';

/** Ob dieser Prozess ein Kern der Testumgebung ist. */
export function istTestumgebung(env: NodeJS.ProcessEnv): boolean {
  return env.WB_TESTUMGEBUNG === '1';
}

/** Die feste Erkennungszeile der Testmarke (wie shell/tests/testmaschine_marke.py und shell/wb-ausrollen). */
export const TESTMARKE_ZEILE = 'wb-testumgebung: Testheimat, Ausrollen dieses Stands erlaubt';

/**
 * Ob diese Maschine eine Testmaschine ist (26.10.2026, Entscheidung Hauptagent; dieselbe Regel wie
 * shell/tests/testmaschine_marke.py): `<heim>/.wb-testumgebung` ist eine regulaere Datei (kein Link) mit genau
 * einem Link, gehoert `eigentuemer` (root), ist weder gruppen- noch weltschreibbar und traegt als erste Zeile
 * TESTMARKE_ZEILE. `heim` ist die Heimat laut Benutzerdatenbank (os.userInfo), nie $HOME; nur root legt die
 * Marke an. Die Parameter sind die Testnaht im Repo-Stand, keine Umgebungsvariable.
 */
export function testmaschineMarkiert(heim: string = userInfo().homedir, eigentuemer = 0): boolean {
  let fd: number;
  try {
    fd = openSync(join(heim, '.wb-testumgebung'), fsKonstanten.O_RDONLY | fsKonstanten.O_NOFOLLOW | fsKonstanten.O_NONBLOCK);
  } catch {
    return false;
  }
  try {
    const s = fstatSync(fd);
    if (!s.isFile() || s.nlink !== 1 || s.uid !== eigentuemer || (s.mode & 0o022) !== 0) return false;
    const puffer = Buffer.alloc(4096);
    const n = readSync(fd, puffer, 0, puffer.length, 0);
    return puffer.subarray(0, n).toString('utf8').split('\n')[0] === TESTMARKE_ZEILE;
  } catch {
    return false;
  } finally {
    closeSync(fd);
  }
}

/**
 * DIE PLATTFORMREGEL (26.10.2026, Entscheidung Hauptagent), dieselbe wie mensch_tor_noetig in
 * shell/tests/lib-mensch-leerlauf.sh: auf einer Testmaschine (`testmaschine`, root-eigene Testmarke) nie,
 * dort sitzt niemand; sonst auf macOS immer, auf Linux nur mit grafischer Sitzung (DISPLAY oder
 * WAYLAND_DISPLAY). Ohne sie (ssh) hat niemand eine Sicht. Die Zaehmung der Fenster gilt unabhaengig davon.
 */
export function torNoetig(env: NodeJS.ProcessEnv, plattform: string, testmaschine: boolean = testmaschineMarkiert()): boolean {
  if (testmaschine) return false;
  return plattform === 'darwin' || !!env.DISPLAY || !!env.WAYLAND_DISPLAY;
}

/**
 * Ein Direktlauf soll als WARTET enden, nicht als Fehlschlag (26.10.2026): eine Suite, die
 * shell/tests/lib-testwerkzeuge.sh einliest, hat einen USR1-Trap (Exit 77) und nennt ihre PID in
 * WB_SUITE_WARTET_PID. Ist sie ein Vorfahr dieses Kerns (dazwischen liegt oft der Node-Shim
 * node_modules/.bin/electron), bekommt sie USR1. Gibt zurueck, ob gesendet wurde.
 */
export function wartetAnSuite(
  env: NodeJS.ProcessEnv,
  start: number,
  eltern: (pid: number) => number | null = elternVon,
  senden: (pid: number) => void = (pid) => { process.kill(pid, 'SIGUSR1'); },
): boolean {
  const ziel = Number(env.WB_SUITE_WARTET_PID);
  if (!Number.isInteger(ziel) || ziel <= 1) return false;
  let p: number | null = start;
  for (let n = 0; p !== null && p > 1 && n < 12; n += 1) {
    if (p === ziel) {
      try { senden(ziel); return true; } catch { return false; }
    }
    p = eltern(p);
  }
  return false;
}

function elternVon(pid: number): number | null {
  try {
    const n = Number(execFileSync('ps', ['-o', 'ppid=', '-p', String(pid)], { encoding: 'utf8', timeout: 2000 }).trim());
    return Number.isInteger(n) ? n : null;
  } catch { return null; }
}

/** Sekunden ohne Eingabe; null heisst: nicht messbar (dann gilt der Mensch als da). */
export type Leerlauf = () => number | null;

/** Mindestleerlauf vor einem sichtbaren Weg und Schwelle fuer "Eingabe waehrend des Laufs". */
export const TOR_MIN_S = 600;
export const EINGABE_S = 3;

/** Die Messung: Attrappe aus der Umgebung, sonst die echte (powerMonitor, von main.ts gereicht). */
export function leerlaufMessung(env: NodeJS.ProcessEnv, echt: Leerlauf): Leerlauf {
  return () => {
    let roh: string | undefined;
    if (env.WB_HID_LEERLAUF_DATEI) {
      try { roh = readFileSync(env.WB_HID_LEERLAUF_DATEI, 'utf8').trim(); } catch { return null; }
    } else if (env.WB_HID_LEERLAUF_S !== undefined) {
      roh = env.WB_HID_LEERLAUF_S;
    } else {
      try { return echt(); } catch { return null; }
    }
    return /^\d+$/.test(roh) ? Number(roh) : null;
  };
}

export interface Tor {
  /** Vor jedem sichtbaren Weg: true nur, wenn niemand am Rechner ist; sonst endet der Kern (Exit 77). */
  vorSichtbar(wofuer: string): boolean;
}

/**
 * Das Tor fuer einen Kern der Testumgebung. `beenden` ist app.exit (im Test eine Attrappe);
 * `melde` schreibt die WARTET-Zeile (stderr). `takt` startet den Begleiter (setInterval).
 */
export function torBauen(
  messen: Leerlauf,
  beenden: (code: number) => void,
  melde: (zeile: string) => void,
  takt: (f: () => void) => void,
): Tor {
  let begleiter = false;
  return {
    vorSichtbar(wofuer) {
      const s = messen();
      if (s === null || s < TOR_MIN_S) {
        melde(`WARTET: Mensch am Rechner -- ${s === null ? 'HID-Leerlauf nicht messbar' : `letzte Eingabe vor ${s} s`}, `
          + `gebraucht ${TOR_MIN_S / 60} min ohne Eingabe; ${wofuer} unterbleibt, der Test-Kern endet (Exit 77)`);
        beenden(77);
        return false;
      }
      if (begleiter) return true;
      begleiter = true;
      takt(() => {
        const jetzt = messen();
        if (jetzt === null || jetzt < EINGABE_S) {
          melde('WARTET: Mensch am Rechner -- Eingabe waehrend des Laufs, der Test-Kern endet (Exit 77)');
          beenden(77);
        }
      });
      return true;
    },
  };
}

const GEZAEHMT = Symbol.for('awb.testfenster.gezaehmt');

/** Ein Fenster der Testumgebung: unsichtbar, klickdurchlaessig, ohne Weg nach vorn, jeder Weg auf den Schirm hinter dem Tor. */
export function fensterZaehmen(w: BrowserWindow, tor?: Tor): void {
  const roh = w as unknown as Record<string | symbol, unknown>;
  if (roh[GEZAEHMT]) return;
  roh[GEZAEHMT] = true;
  w.setOpacity(0);
  w.setIgnoreMouseEvents(true);
  const showInactive = w.showInactive.bind(w);
  const maximize = w.maximize.bind(w);
  const setFullScreen = w.setFullScreen.bind(w);
  // Kein Werfen bei geschlossenem Tor: ein Fehler mitten im Start liefe in den uncaughtException-Riegel
  // (Exit 1 statt 77); der Weg unterbleibt einfach, und der Kern endet ueber beenden(77).
  const frei = (wofuer: string): boolean => !tor || tor.vorSichtbar(wofuer);
  roh.show = () => { if (frei('show()')) showInactive(); };
  roh.showInactive = () => { if (frei('showInactive()')) showInactive(); };
  roh.maximize = () => { if (frei('maximize()')) maximize(); };
  roh.setFullScreen = (an: boolean) => { if (!an || frei('setFullScreen(true)')) setFullScreen(an); };
  roh.focus = () => undefined;
  roh.moveTop = () => undefined;
  roh.setOpacity = () => undefined;
  roh.setIgnoreMouseEvents = () => undefined;
}

/**
 * In der Testumgebung: Dock versteckt, `app.focus()` stumm, jedes neue
 * BrowserWindow gezaehmt und hinter dem HID-Tor. Ausserhalb der Testumgebung
 * geschieht nichts. Muss vor dem ersten Fenster laufen. Gibt zurueck, ob
 * gezaehmt wird. `echtLeerlauf` ist powerMonitor.getSystemIdleTime (erst nach
 * app.whenReady gueltig; sichtbare Wege gibt es erst danach).
 */
export function testfensterZaehmen(
  app: App, env: NodeJS.ProcessEnv, plattform: string, echtLeerlauf: Leerlauf = () => null,
  testmaschine: () => boolean = testmaschineMarkiert,
): boolean {
  if (!istTestumgebung(env)) return false;
  if (plattform === 'darwin' && app.dock) {
    app.dock.hide();
    (app.dock as unknown as Record<string, unknown>).show = () => Promise.resolve();
  }
  (app as unknown as Record<string, unknown>).focus = () => undefined;
  const tor = !torNoetig(env, plattform, testmaschine()) ? undefined : torBauen(
    leerlaufMessung(env, echtLeerlauf),
    (code) => { wartetAnSuite(env, process.ppid); app.exit(code); },
    (zeile) => { process.stderr.write(`${zeile}\n`); process.stdout.write(`${zeile}\n`); },
    (f) => { setInterval(f, 1000).unref?.(); },
  );
  app.on('browser-window-created', (_e, w) => fensterZaehmen(w, tor));
  return true;
}

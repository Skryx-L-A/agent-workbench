// DER MESSSTAND DER INSTALLIERTEN CLAUDE-VERSION (Auftrag mobil-messausloeser, 18.10.2026).
//
// Der Handy-Adapter (dialogs/claude.ts) beantwortet Claude-Dialoge nur fuer
// gemessene Versionen (`CLAUDE_MEASURED_VERSIONS`); jede andere ist fail-closed.
// Claude aktualisiert sich selbst, und bisher merkte niemand das Update, bis
// der Mensch am Handy nicht antworten konnte. Dieser Ausloeser meldet es.
//
// Wann: beim Start des Kerns und bei jeder Aenderung im Ordner
// `~/.local/share/claude/versions/` oder am Eintrag `claude` in `~/.local/bin/`
// (fs.watch, ereignisgesteuert; kein Zeitgeber, kein Polling). Dazu bei jeder
// Aenderung im Gruppenordner `leitung` von wb-gruppe: steht eine Meldung noch aus
// (etwa weil `wb-gruppe auto leitung` nach dem Kernstart lief), holt das den Versuch
// nach. Ohne offene Meldung loest der Gruppenordner nichts aus. `leitung` liegt in
// `gruppen/`: beobachtet werden `gruppen/` (nur das Ereignis `leitung`) und `leitung/`
// selbst, sobald er existiert. Fehlt `leitung/` beim Start, entsteht er spaeter und wird
// dann beobachtet; `gruppen/` dagegen muss beim Start da sein: fehlt es, wird es nicht
// beobachtet (Protokollzeile) und auch nicht erneut versucht, `leitung` bleibt bis zum
// naechsten Kernstart unbeobachtet. Eine Schreibung IN `leitung/` kommt am Elternordner
// nicht an (gemessen 19.10.2026, fs.watch macOS). `leitung/` wird nur neu eingehaengt,
// wenn der Beobachter fehlt oder der Ordner eine neue Inode hat. Die installierte
// Version ist das Ziel des Symlinks `~/.local/bin/claude` (Herleitung wie bei
// `mobilLaufendeVersion` in main.ts: `.../versions/<version>`).
//
// LINUX (peer, gemessen 18.10.2026): `~/.local/bin/claude` ist dort ein Symlink auf einen
// mise-Shim (`.../mise/shims/claude` -> `/usr/bin/mise`), kein Versionsziel. Die Versionen
// liegen unter `~/.local/share/mise/installs/claude/<v>/claude`, daneben `latest -> ./<v>`
// sowie `2` und `2.1` als Verweise. Ohne Versionsziel gilt deshalb dieser Ordner: die
// Version, auf die `latest` zeigt, sonst die numerisch hoechste Versionsmappe mit `claude`
// darin. `mobilVersionMessen` (main.ts) kennt fuer die LAUFENDE Version nur den Pfad
// `.../versions/<v>`; ein mise-Pfad faellt dort auf `<datei> --version` zurueck.
//
// Was: Ist die Version nicht in der Messliste, geht GENAU EINMAL je Version und
// Maschine eine Meldung an die Gruppe `leitung`, adressiert an die Pflege der Mobile-App.
// "Einmal" steht in einer Merkdatei (atomar geschrieben); sie wird erst NACH
// einer geglueckten Zustellung beschrieben, eine gescheiterte Zustellung wird
// beim naechsten Ereignis oder Start erneut versucht.
//
// Fail-closed und still: Zeigt der Symlink ins Leere oder fehlt er, gibt es
// keine Meldung und keinen Absturz. Jeder Fehler endet in einer Protokollzeile.
// Pfade, Messliste, Absender und Beobachter sind eine Naht (`MessstandNaht`);
// der Test in app/test/mobile/stand/claude-messstand.test.ts ersetzt alle vier.
import { execFile } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, readlinkSync, renameSync, statSync, watch, writeFileSync } from 'node:fs';
import { basename, dirname, isAbsolute, join, resolve } from 'node:path';

/** Die Version aus einem Versionsnamen; dieselbe Form wie `mobilVersionMessen` in main.ts. */
const VERSION = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

export interface MessstandNaht {
  /** Der Symlink `~/.local/bin/claude`. */
  linkPfad: string;
  /** Der Ordner `~/.local/share/claude/versions`. */
  versionenOrdner: string;
  /** Linux/mise: der Ordner `~/.local/share/mise/installs/claude`; leer, wo es ihn nicht gibt. */
  miseOrdner: string;
  /** Die Merkdatei der gemeldeten Versionen (dieser Maschine). */
  merkdatei: string;
  /** Der Gruppenordner `leitung` von wb-gruppe; eine Aenderung dort versucht eine offene Meldung erneut. Fehlt er beim Start, wird er beobachtet, sobald er in `gruppen/` entsteht. */
  gruppenOrdner?: string;
  /** Die gemessenen Versionen (`CLAUDE_MEASURED_VERSIONS`). */
  gemessen: readonly string[];
  /** Name dieser Maschine fuer den Meldetext. */
  maschine: string;
  /** Stellt den Text der Gruppe `leitung` zu; `true` nur bei geglueckter Zustellung. */
  senden: (text: string) => Promise<boolean>;
  /** Beobachtet einen Ordner; `bei` bekommt den Dateinamen (leer, wenn unbekannt). Rueckgabe: Beobachter beenden. */
  beobachten: (ordner: string, bei: (name: string) => void) => () => void;
  /** Eine Protokollzeile. */
  protokoll: (zeile: string) => void;
}

/** Versionsnamen numerisch ordnen (2.1.99 < 2.1.302); eine Vorabversion liegt unter ihrer Ausgabe. */
function versionenVergleichen(a: string, b: string): number {
  const teile = (v: string): number[] => v.split(/[-+]/)[0].split('.').map(Number);
  const [x, y] = [teile(a), teile(b)];
  for (let i = 0; i < 3; i += 1) if (x[i] !== y[i]) return x[i] - y[i];
  return Number(!/[-+]/.test(a)) - Number(!/[-+]/.test(b)) || (a < b ? -1 : a > b ? 1 : 0);
}

/**
 * Linux/mise: die Version, auf die `latest` zeigt, sonst die numerisch hoechste
 * Versionsmappe, die eine Datei `claude` enthaelt. Leer, wenn es keine gibt.
 */
function miseVersion(ordner: string): string {
  let namen: string[];
  try {
    namen = readdirSync(ordner);
  } catch {
    return '';
  }
  const vorhanden = namen.filter((n) => VERSION.test(n) && existsSync(join(ordner, n, 'claude')));
  if (!vorhanden.length) return '';
  try {
    const latest = basename(readlinkSync(join(ordner, 'latest')));
    if (vorhanden.includes(latest)) return latest;
  } catch {
    // kein oder ins Leere zeigendes `latest`: die hoechste Mappe entscheidet
  }
  return vorhanden.sort(versionenVergleichen)[vorhanden.length - 1];
}

/**
 * Die installierte Version: das Ziel des Symlinks, wenn es eine Datei mit einem
 * Versionsnamen ist (macOS). Sonst, wo ein mise-Ordner angegeben ist (Linux), dessen
 * Version. Leer bei fehlendem Link, Nicht-Link, ins Leere zeigendem Link oder einem
 * Ziel ohne Versionsnamen und ohne mise-Version.
 */
export function installierteClaudeVersion(linkPfad: string, miseOrdner = ''): string {
  return versionVonLink(linkPfad) || (miseOrdner ? miseVersion(miseOrdner) : '');
}

function versionVonLink(linkPfad: string): string {
  let ziel: string;
  try {
    ziel = readlinkSync(linkPfad);
  } catch {
    return '';
  }
  const voll = isAbsolute(ziel) ? ziel : resolve(dirname(linkPfad), ziel);
  const name = basename(voll);
  if (!VERSION.test(name) || !existsSync(voll)) return '';
  return name;
}

/**
 * Die Version aus dem Pfad der laufenden Programmdatei: macOS `…/claude/versions/<v>`,
 * Linux mit mise `…/mise/installs/claude/<v>/claude` (gemessen auf peer, 18.10.).
 * Leer, wenn der Pfad keine Version traegt.
 */
export function claudeVersionAusPfad(pfad: string): string {
  const treffer = /\/(?:versions|mise\/installs\/claude)\/(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)(?:\/claude)?$/.exec(pfad);
  return treffer ? treffer[1] : '';
}

/** Der Text der Meldung an die Pflege der Mobile-App. */
export function messstandText(version: string, maschine: string): string {
  return `Mobile-App: Claude ${version} auf ${maschine} installiert und nicht gemessen; `
    + `Handy-Dialoge fuer Sitzungen mit ${version} gesperrt. `
    + 'Messen und Merge-Antrag (Regel: docs/MOBILE-APP-BETRIEB.md, Abschnitt "Nach jedem Claude-Update").';
}

/** Die Merkdatei lesen; fehlt sie oder ist sie unlesbar, gilt nichts als gemeldet. */
function gemeldetLesen(merkdatei: string): Record<string, string> {
  try {
    const roh: unknown = JSON.parse(readFileSync(merkdatei, 'utf8'));
    const gemeldet = (roh as { gemeldet?: unknown } | null)?.gemeldet;
    if (!gemeldet || typeof gemeldet !== 'object' || Array.isArray(gemeldet)) return {};
    return Object.fromEntries(Object.entries(gemeldet as Record<string, unknown>).filter(([, v]) => typeof v === 'string')) as Record<string, string>;
  } catch {
    return {};
  }
}

/** Atomar: in eine Nachbardatei schreiben, dann umbenennen. */
function gemeldetSchreiben(merkdatei: string, gemeldet: Record<string, string>): void {
  mkdirSync(dirname(merkdatei), { recursive: true, mode: 0o700 });
  const tmp = `${merkdatei}.${process.pid}.tmp`;
  writeFileSync(tmp, `${JSON.stringify({ schema: 1, gemeldet }, null, 2)}\n`, { mode: 0o600 });
  renameSync(tmp, merkdatei);
}

export class ClaudeMessstand {
  private laufend: Promise<void> | null = null;
  private nochmal = false;
  private beobachter: Array<() => void> = [];
  private leitungEnde: (() => void) | null = null;
  /** Geraet und Inode des Ordners, an dem `leitungEnde` haengt; wechselt sie, ist `leitung` neu entstanden. */
  private leitungKennung = '';
  private beendet = false;

  constructor(private readonly naht: MessstandNaht) {}

  /** Einmal sofort pruefen und ab jetzt bei jeder Aenderung am Ordner oder am Link. */
  start(): Promise<void> {
    if (this.beobachter.length || this.beendet) return this.laufend ?? Promise.resolve();
    const linkName = basename(this.naht.linkPfad);
    this.beobachter.push(this.naht.beobachten(this.naht.versionenOrdner, () => { void this.pruefen(); }));
    if (this.naht.miseOrdner) this.beobachter.push(this.naht.beobachten(this.naht.miseOrdner, () => { void this.pruefen(); }));
    if (this.naht.gruppenOrdner) this.gruppeBeobachten(this.naht.gruppenOrdner);
    this.beobachter.push(this.naht.beobachten(dirname(this.naht.linkPfad), (name) => {
      if (name === linkName || name === '') void this.pruefen();
    }));
    return this.pruefen();
  }

  stop(): void {
    this.beendet = true;
    for (const ende of this.beobachter.splice(0)) {
      try { ende(); } catch { /* ein Beobachter, der sich nicht beenden laesst, haelt nichts an */ }
    }
    try { this.leitungEnde?.(); } catch { /* wie oben */ }
    this.leitungEnde = null;
    this.leitungKennung = '';
  }

  /** `gruppen/` fuer das Entstehen von `leitung` (nur dieser Name), `leitung/` selbst, sobald er existiert. */
  private gruppeBeobachten(gruppe: string): void {
    const name = basename(gruppe);
    this.beobachter.push(this.naht.beobachten(dirname(gruppe), (eintrag) => {
      if (eintrag !== name && eintrag !== '') return;
      this.leitungBeobachten(gruppe);
      void this.pruefen();
    }));
    this.leitungBeobachten(gruppe);
  }

  /**
   * Beobachtet `leitung/`, sobald er existiert. Haengt neu ein nur, wenn kein Beobachter da ist oder
   * `leitung/` eine neue Inode hat (Ordner ersetzt); sonst bleibt der Beobachter, damit zwischen
   * close und watch keine Ereignisse verloren gehen. Verschwindet `leitung/`, wird der Beobachter beendet.
   */
  private leitungBeobachten(gruppe: string): void {
    if (this.beendet) return;
    let kennung = '';
    try {
      const st = statSync(gruppe);
      if (st.isDirectory()) kennung = `${st.dev}:${st.ino}`;
    } catch { /* fehlt noch: kein Beobachter */ }
    if (this.leitungEnde && kennung && kennung === this.leitungKennung) return;
    try { this.leitungEnde?.(); } catch { /* wie in stop() */ }
    this.leitungEnde = kennung ? this.naht.beobachten(gruppe, () => { void this.pruefen(); }) : null;
    this.leitungKennung = kennung;
  }

  /**
   * Eine Pruefung, nie zwei zugleich. Ereignisse, die waehrend eines Laufs kommen,
   * fassen sich zu einem weiteren Lauf zusammen; ein Lauf, der an der Zustellung
   * scheiterte, wiederholt sich im selben Atemzug nur fuer eine ANDERE Version.
   */
  pruefen(): Promise<void> {
    if (this.laufend) {
      this.nochmal = true;
      return this.laufend;
    }
    const lauf = (async () => {
      try {
        do {
          this.nochmal = false;
          const { fehlgeschlagen } = await this.einmal();
          if (fehlgeschlagen && installierteClaudeVersion(this.naht.linkPfad, this.naht.miseOrdner) === fehlgeschlagen) this.nochmal = false;
        } while (this.nochmal && !this.beendet);
      } catch (fehler) {
        this.naht.protokoll(`Messstand: Pruefung fehlgeschlagen -- ${(fehler as Error).message}`);
      } finally {
        this.laufend = null;
      }
    })();
    this.laufend = lauf;
    return lauf;
  }

  /** Ein Durchgang. `fehlgeschlagen` nennt die Version, deren Zustellung scheiterte. */
  private async einmal(): Promise<{ fehlgeschlagen: string }> {
    const version = installierteClaudeVersion(this.naht.linkPfad, this.naht.miseOrdner);
    if (!version || this.naht.gemessen.includes(version)) return { fehlgeschlagen: '' };
    if (version in gemeldetLesen(this.naht.merkdatei)) return { fehlgeschlagen: '' };
    let zugestellt = false;
    try {
      zugestellt = await this.naht.senden(messstandText(version, this.naht.maschine));
    } catch (fehler) {
      this.naht.protokoll(`Messstand: Meldung fuer Claude ${version} nicht zugestellt -- ${(fehler as Error).message}`);
      return { fehlgeschlagen: version };
    }
    if (!zugestellt) {
      this.naht.protokoll(`Messstand: Meldung fuer Claude ${version} nicht zugestellt; naechster Versuch beim naechsten Ereignis oder Start.`);
      return { fehlgeschlagen: version };
    }
    try {
      gemeldetSchreiben(this.naht.merkdatei, { ...gemeldetLesen(this.naht.merkdatei), [version]: new Date().toISOString() });
    } catch (fehler) {
      this.naht.protokoll(`Messstand: Merkdatei nicht geschrieben -- ${(fehler as Error).message}`);
    }
    this.naht.protokoll(`Messstand: Claude ${version} ist nicht gemessen; Meldung an leitung zugestellt.`);
    return { fehlgeschlagen: '' };
  }
}

/** Der echte Beobachter: fs.watch auf den Ordner, ohne den Prozess am Leben zu halten. */
export function ordnerBeobachten(protokoll: (zeile: string) => void): MessstandNaht['beobachten'] {
  return (ordner, bei) => {
    try {
      const w = watch(ordner, { persistent: false }, (_art, name) => bei(name ? String(name) : ''));
      w.on('error', (fehler) => protokoll(`Messstand: Beobachter an ${ordner} gestoppt -- ${fehler.message}`));
      return () => w.close();
    } catch (fehler) {
      protokoll(`Messstand: ${ordner} wird nicht beobachtet -- ${(fehler as Error).message}`);
      return () => undefined;
    }
  };
}

/**
 * Der echte Absender: `wb-gruppe meldung leitung <text>` als Kindprozess mit Frist.
 * Der Kern hat keinen Pane und ist kein Mensch; `wb-gruppe` erkennt ihn als
 * Systemabsender `werkbank`, wenn er sich dazu meldet (WB_KERN_ABSENDER) und
 * `wb-mensch` ihn als geprueften Kern bestaetigt (WB_MENSCH_QUELLE, WB_APP_PID).
 * Exit 7 heisst: gespeichert, aber nicht jedes Mitglied erreicht -- die
 * Nachlieferung gehoert `wb-gruppe`, ein zweiter Versuch hier wuerde doppeln.
 */
export function leitungMeldungSenden(opt: {
  bin: string; home: string; pid: number; tmuxSocket?: string; fristMs?: number; protokoll: (zeile: string) => void;
}): (text: string) => Promise<boolean> {
  return (text) => new Promise((fertig) => {
    execFile(opt.bin, ['meldung', 'leitung', text], {
      env: meldungsUmgebung(opt.home, opt.pid, opt.tmuxSocket ?? ''), timeout: opt.fristMs ?? 120_000,
      killSignal: 'SIGKILL', maxBuffer: 256 * 1024,
    }, (fehler, _out, err) => {
      const code = fehler ? (fehler as { code?: unknown }).code : 0;
      if (code === 0 || code === 7) { fertig(true); return; }
      const grund = String(err ?? '').trim().split('\n').pop() ?? '';
      opt.protokoll(`Messstand: wb-gruppe meldung endete mit ${String(code)}${grund ? ` -- ${grund.slice(0, 200)}` : ''}`);
      fertig(false);
    });
  });
}

/**
 * Feste Umgebung fuer `wb-gruppe meldung` (Review 18.10.): nichts aus der Kernumgebung erben,
 * nur Heimat, ein fester PATH, die Sprache und der eigene tmux-Server, dazu die drei Angaben
 * des Systemabsenders.
 */
export function meldungsUmgebung(home: string, pid: number, tmuxSocket = '', lang = process.env.LANG ?? ''): NodeJS.ProcessEnv {
  return {
    HOME: home,
    PATH: `${join(home, '.local', 'bin')}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
    TMPDIR: '/tmp',
    PYTHONUTF8: '1',
    ...(lang ? { LANG: lang } : {}),
    ...(tmuxSocket ? { WB_TMUX_SOCKET: tmuxSocket } : {}),
    WB_MENSCH_QUELLE: 'oberflaeche', WB_APP_PID: String(pid), WB_KERN_ABSENDER: 'werkbank',
  };
}

/** Der feste Ort von `wb-gruppe` in ~/.local/bin; kein Rueckfall auf einen Namen im PATH (Review 18.10.). */
export function wbGruppePfad(home: string): string {
  return join(home, '.local', 'bin', 'wb-gruppe');
}

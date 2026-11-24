// WER FRAGT, UND WOFUER.
//
// An jeder Freigabe ist sofort zu sehen, ob ein Orchestrator oder ein Worker fragt,
// welcher es ist (der Name, wie die Werkbank ihn anzeigt) und wofuer die Freigabe
// ist. Die Kurzansicht nennt die Absicht hinter dem Befehl, die genauere Ansicht den
// genauen Befehl.
//
// Diese Datei bestimmt das EINMAL, an der Quelle (freigaben.ts), und alle Anzeigen
// lesen es nur: Electron- und Mac-Leiste, beide Blaetter, die Mitteilung, der Push,
// die Mobil-API. Der Befehl selbst bleibt unberuehrt -- er ist der Wortlaut, ueber
// den die Freigabe ihren Schluessel bildet (pane+cwd+command).
//
// DIE ABSICHT WIRD NIE ERFUNDEN. Reihenfolge:
//   1. die Beschreibung, die der Harness mitliefert (Claude Code: `tool_input.description`,
//      im Marker als `description`) -- der Agent sagt selbst, was er vorhat;
//   2. eine Ableitung aus Werkzeug und Befehl fuer bekannte Programme, ohne Pfade;
//   3. sonst „Befehl ausführen“ plus Werkzeugname.

export type FreigabeRolle = 'orchestrator' | 'worker' | 'unbekannt';
export type AbsichtQuelle = 'harness' | 'abgeleitet' | 'unklar';

export interface Absicht {
  text: string;
  quelle: AbsichtQuelle;
}

/** Was eine Anzeige zum Kurztext braucht (GuardBlockEntry und RequestEntry tragen es). */
export interface FreigabeKopf {
  rolle: FreigabeRolle;
  anzeigename: string;
  absicht: string;
  absichtQuelle?: AbsichtQuelle;
}

const HOECHSTENS = 160;

function kuerzen(text: string, max = HOECHSTENS): string {
  return text.length <= max ? text : `${text.slice(0, max - 1).trimEnd()}…`;
}

/** Das Wort vor dem Namen: „Worker wz-x“, „Orchestrator claude-workbench“, „Unbekannter Pane %3“. */
export function rollenWort(rolle: FreigabeRolle): string {
  return rolle === 'orchestrator' ? 'Orchestrator' : rolle === 'worker' ? 'Worker' : 'Unbekannter';
}

export function rollenName(k: Pick<FreigabeKopf, 'rolle' | 'anzeigename'>): string {
  return `${rollenWort(k.rolle)} ${k.anzeigename}`.trim();
}

/** Die Kurzansicht in einer Zeile: „Worker wz-x: Änderungen pushen (git push)“. */
export function freigabeKurztext(k: FreigabeKopf): string {
  return `${rollenName(k)}: ${k.absicht}`;
}

/** Bis zum ersten Satzende, sonst alles; fuer die Mitteilung (ein Satz). */
function ersterSatz(text: string): string {
  const m = /^(.+?[.!?])(\s|$)/.exec(text.trim());
  return (m ? m[1] : text.trim()).replace(/[.!?]$/, '');
}

/**
 * Die Mitteilung am Rechner (Mac/Linux): Rolle, Name und Absicht in EINEM Satz.
 * Eine Beschreibung des Harness ist Agententext; Pfade darin werden nicht
 * ausgeschrieben. Eine Ableitung enthaelt keine (absichtBestimmen).
 */
export function mitteilungsText(k: FreigabeKopf): string {
  const satz = kuerzen(ohnePfade(ersterSatz(k.absicht)), 120);
  return `${rollenName(k)} wartet auf eine Freigabe: ${satz}.`;
}

/**
 * Der Push auf das Telefon: Rolle und Name, OHNE Absicht (Entscheidung des
 * Hauptagenten 26.10.: MOBILE-APP-SPEC.md:196 -- kein Befehl, kein Pfad auf dem
 * Sperrbildschirm; die Absicht stammt aus Agententext und kann beides enthalten).
 * Die Absicht steht nach dem Entsperren in der App (Mobil-API).
 */
export function pushText(k: Pick<FreigabeKopf, 'rolle' | 'anzeigename'>): string {
  return `${rollenName({ rolle: k.rolle, anzeigename: ohnePfade(k.anzeigename) })} wartet auf eine Freigabe.`;
}

/** Pfadartige Woerter (mit `/` oder `~`) werden zu „…“. */
export function ohnePfade(text: string): string {
  return text.split(/(\s+)/).map((w) => (/[/~]/.test(w) && w.trim() ? '…' : w)).join('');
}

// --- Ableitung ---------------------------------------------------------------

/** Trennt an `&&`, `||`, `;`, `|` und Zeilenenden ausserhalb von Anfuehrungszeichen. */
function schritte(befehl: string): string[] {
  const raus: string[] = [];
  let akt = '';
  let q: string | null = null;
  for (let i = 0; i < befehl.length; i++) {
    const c = befehl[i];
    if (q) {
      akt += c;
      if (c === q) q = null;
      continue;
    }
    if (c === '"' || c === "'") { q = c; akt += c; continue; }
    if (c === ';' || c === '\n' || c === '|' || (c === '&' && befehl[i + 1] === '&')) {
      if (akt.trim()) raus.push(akt.trim());
      akt = '';
      if ((c === '&' || c === '|') && befehl[i + 1] === c) i++;
      continue;
    }
    akt += c;
  }
  if (akt.trim()) raus.push(akt.trim());
  return raus;
}

/** Woerter eines Schritts, Anfuehrungszeichen entfernt. */
function woerter(schritt: string): string[] {
  const raus: string[] = [];
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(schritt))) raus.push(m[1] ?? m[2] ?? m[3]);
  return raus;
}

const UEBERGEHEN = new Set(['cd', 'pushd', 'popd', 'export', 'unset', 'set', 'true', ':', 'source', '.']);
const HUELLEN = new Set(['env', 'nohup', 'time', 'command', 'exec', 'nice', 'caffeinate']);

function basis(wort: string): string {
  const teil = wort.split('/').pop() ?? wort;
  return teil;
}

const SKRIPTSPRACHEN = new Set(['python', 'python3', 'node', 'bash', 'sh', 'zsh', 'ruby', 'perl', 'deno', 'bun', 'tsx', 'osascript']);

/** Die Absicht eines einzelnen Schritts, oder null, wenn er nichts Bekanntes ist. */
function schrittAbsicht(w: string[]): { text: string; sudo: boolean } | null {
  let sudo = false;
  let i = 0;
  // Umgebungszuweisungen, Huellen und sudo vorne weg.
  for (;;) {
    const x = w[i];
    if (x === undefined) return null;
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(x)) { i++; continue; }
    if (x === 'sudo') { sudo = true; i++; while (w[i]?.startsWith('-')) i++; continue; }
    if (HUELLEN.has(basis(x))) { i++; while (w[i]?.startsWith('-')) i++; continue; }
    if (basis(x) === 'timeout') { i += 2; continue; }
    break;
  }
  const prog = basis(w[i]);
  const rest = w.slice(i + 1);
  const sub = rest.find((x) => !x.startsWith('-')) ?? '';
  const hat = (...f: string[]) => rest.some((x) => f.includes(x));
  const text = programmAbsicht(prog, sub, rest, hat);
  return text ? { text, sudo } : null;
}

function programmAbsicht(prog: string, sub: string, rest: string[], hat: (...f: string[]) => boolean): string | null {
  // Die Werkbank-Tafel startet den eigentlichen Lauf hinter `--`.
  if (prog === 'wb-tafel') {
    const ab = rest.indexOf('--');
    if (sub === 'nimm' && ab >= 0) {
      const innen = schrittAbsicht(rest.slice(ab + 1));
      return innen ? `Über die Tafel: ${innen.text}` : 'Lauf über die Tafel starten (wb-tafel)';
    }
    return `Belegungstafel bedienen (wb-tafel ${sub})`.replace(' )', ')');
  }
  switch (prog) {
    case 'git': {
      // `git -C <ordner> push`: Schalter mit Wert vor dem Unterbefehl ueberspringen.
      let j = 0;
      while (rest[j]?.startsWith('-')) j += rest[j] === '-C' || rest[j] === '-c' ? 2 : 1;
      const gsub = rest[j] ?? '';
      const g = (t: string) => `${t} (git ${gsub})`;
      switch (gsub) {
        case 'push': return g(hat('--force', '-f', '--force-with-lease') ? 'Änderungen erzwungen pushen' : 'Änderungen pushen');
        case 'commit': return g('Änderungen committen');
        case 'add': return g('Änderungen vormerken');
        case 'reset': return g(hat('--hard') ? 'Arbeitsstand hart zurücksetzen' : 'Vormerkungen zurücksetzen');
        case 'checkout': case 'switch': return g('Zweig oder Dateien auschecken');
        case 'restore': return g('Dateien wiederherstellen');
        case 'merge': return g('Zweige zusammenführen');
        case 'rebase': return g('Zweig neu aufsetzen');
        case 'clean': return g('Unversionierte Dateien löschen');
        case 'branch': return g(hat('-D', '-d', '--delete') ? 'Zweig löschen' : 'Zweige verwalten');
        case 'tag': return g('Markierung setzen');
        case 'stash': return g('Änderungen beiseitelegen');
        case 'clone': return g('Repository klonen');
        case 'pull': case 'fetch': return g('Änderungen holen');
        case 'worktree': return g('Worktree verwalten');
        case 'rm': return g('Dateien aus Git löschen');
        case 'mv': return g('Dateien in Git verschieben');
        case 'cherry-pick': return g('Commit übernehmen');
        case 'revert': return g('Commit rückgängig machen');
        case 'config': return g('Git-Einstellung ändern');
        case 'gc': case 'prune': return g('Git-Ablage aufräumen');
        default: return gsub ? g('Git-Befehl ausführen') : 'Git-Befehl ausführen (git)';
      }
    }
    case 'rm': case 'rmdir': case 'unlink': case 'trash': case 'shred': return `Dateien löschen (${prog})`;
    case 'mv': return 'Dateien verschieben oder umbenennen (mv)';
    case 'cp': case 'ditto': return `Dateien kopieren (${prog})`;
    case 'chmod': case 'chown': case 'chgrp': case 'chflags': case 'xattr': return `Dateirechte ändern (${prog})`;
    case 'ln': return 'Verknüpfung anlegen (ln)';
    case 'mkdir': return 'Ordner anlegen (mkdir)';
    case 'touch': return 'Datei anlegen (touch)';
    case 'tee': return 'In eine Datei schreiben (tee)';
    case 'dd': return 'Rohdaten schreiben (dd)';
    case 'truncate': return 'Datei kürzen (truncate)';
    case 'sed': return hat('-i') || rest.some((x) => x.startsWith('-i')) ? 'Dateien bearbeiten (sed -i)' : null;
    case 'find': return hat('-delete') || (hat('-exec', '-execdir') && rest.some((x) => basis(x) === 'rm'))
      ? 'Dateien suchen und löschen (find)' : null;
    case 'npm': case 'pnpm': case 'yarn': {
      const p = (t: string) => `${t} (${prog} ${sub})`.replace(' )', ')');
      if (['install', 'i', 'ci', 'add'].includes(sub) || (prog === 'yarn' && !sub)) return p('Pakete installieren');
      if (['uninstall', 'remove', 'rm', 'un'].includes(sub)) return p('Pakete entfernen');
      if (['update', 'upgrade', 'up'].includes(sub)) return p('Pakete aktualisieren');
      if (sub === 'publish') return p('Paket veröffentlichen');
      if (sub === 'test') return p('Tests ausführen');
      if (sub === 'run' || sub === 'run-script') {
        const name = rest.filter((x) => !x.startsWith('-'))[1] ?? '';
        return name ? `Skript ${ohnePfade(name)} ausführen (${prog} run)` : p('Skript ausführen');
      }
      return p('Paketbefehl ausführen');
    }
    case 'npx': case 'pnpx': case 'bunx': return sub ? `Paket ${basis(sub)} ausführen (${prog})` : null;
    case 'pip': case 'pip3': case 'pipx': case 'uv':
      if (sub === 'install' || (prog === 'uv' && rest.includes('install'))) return `Python-Pakete installieren (${prog})`;
      if (sub === 'uninstall' || sub === 'remove') return `Python-Pakete entfernen (${prog})`;
      return null;
    case 'brew':
      if (sub === 'install' || sub === 'reinstall') return 'Programme installieren (brew)';
      if (sub === 'upgrade' || sub === 'update') return 'Programme aktualisieren (brew)';
      if (sub === 'uninstall' || sub === 'remove') return 'Programme entfernen (brew)';
      if (sub === 'services') return 'Hintergrunddienst steuern (brew services)';
      return null;
    case 'apt': case 'apt-get': case 'dnf': case 'yum': case 'pacman': case 'zypper':
      return `Systempakete verwalten (${prog})`;
    case 'curl': case 'wget':
      return hat('-X', '--request', '-d', '--data', '--data-binary', '--data-raw', '-F', '--form', '-T', '--upload-file')
        ? `Daten ins Netz senden (${prog})` : `Daten aus dem Netz laden (${prog})`;
    case 'ssh': case 'mosh': return `Auf einer anderen Maschine arbeiten (${prog})`;
    case 'scp': case 'rsync': case 'sftp': return `Dateien übertragen (${prog})`;
    case 'kill': case 'pkill': case 'killall': return `Prozesse beenden (${prog})`;
    case 'launchctl': case 'systemctl': case 'service': return `Systemdienst steuern (${prog})`;
    case 'docker': case 'podman': case 'kubectl': return `Container steuern (${prog})`;
    case 'tmux': return `tmux steuern (tmux${sub ? ` ${sub}` : ''})`;
    case 'screencapture': return 'Bildschirmfoto aufnehmen (screencapture)';
    case 'security': return 'Schlüsselbund ansprechen (security)';
    case 'defaults': return sub === 'write' || sub === 'delete' ? 'Systemeinstellung ändern (defaults)' : null;
    case 'crontab': return 'Zeitplan ändern (crontab)';
    case 'open': return 'Datei oder Programm öffnen (open)';
    case 'make': return 'Bauen (make)';
    case 'xcodebuild': return 'Projekt bauen (xcodebuild)';
    case 'swift': case 'cargo': case 'go':
      if (sub === 'build') return `Projekt bauen (${prog} build)`;
      if (sub === 'test') return `Tests ausführen (${prog} test)`;
      if (sub === 'run') return `Programm starten (${prog} run)`;
      return null;
    case 'gh':
      if (sub === 'pr') return 'Pull Request bearbeiten (gh pr)';
      if (sub === 'release') return 'Release bearbeiten (gh release)';
      if (sub === 'repo') return 'Repository bearbeiten (gh repo)';
      return sub ? `GitHub ansprechen (gh ${sub})` : null;
    case 'diskutil': case 'hdiutil': return `Datenträger bearbeiten (${prog})`;
    case 'reboot': case 'shutdown': case 'halt': return `Rechner neu starten oder ausschalten (${prog})`;
    case 'tccutil': return 'Datenschutzrechte zurücksetzen (tccutil)';
    default: break;
  }
  if (SKRIPTSPRACHEN.has(prog)) {
    if (prog === 'osascript') return 'AppleScript ausführen (osascript)';
    if (hat('-c', '-e', '--eval')) return `Eingebetteten Code ausführen (${prog})`;
    const skript = rest.find((x) => !x.startsWith('-'));
    return skript ? `Skript ${basis(skript)} ausführen (${prog})` : null;
  }
  // Ein Skript per Pfad (./x.sh, shell/tests/y.py, ~/bin/z).
  if (/\.(sh|py|mjs|js|ts|rb|pl)$/.test(prog) || /^wb-/.test(prog)) {
    return /^wb-/.test(prog) ? `Werkbank-Werkzeug ${prog} ausführen` : `Skript ${prog} ausführen`;
  }
  return null;
}

/**
 * Die Absicht hinter einer Freigabe. `beschreibung` ist die Angabe des Harness
 * (leer, wenn keine kam), `werkzeug` der Name seines Werkzeugs (Bash, wenn der
 * Bash-Guard fragt).
 */
export function absichtBestimmen(beschreibung: string, befehl: string, werkzeug = 'Bash'): Absicht {
  const erste = (beschreibung ?? '').split('\n').map((z) => z.trim()).find((z) => z) ?? '';
  // Pfade in der Beschreibung werden zu „…“: die Kurzansicht steht ohne Pfad in einer Zeile
  // (renderer.ts, Regelbruch 8 vom 05.09.2026); der genaue Befehl steht in der Detailansicht.
  if (erste) return { text: kuerzen(ohnePfade(erste.replace(/\s+/g, ' '))), quelle: 'harness' };
  const unklar = (prog = ''): Absicht => ({ text: `Befehl ausführen (${werkzeug}${prog ? `: ${prog}` : ''})`, quelle: 'unklar' });
  const teile = schritte(befehl ?? '');
  const bedeutsam = teile.filter((s) => {
    const w = woerter(s);
    return w.length > 0 && !UEBERGEHEN.has(w[0]);
  });
  if (!bedeutsam.length) return unklar();
  const erster = woerter(bedeutsam[0]);
  // Ersetzungen, Unterschalen und Here-Docs vorne: nicht ohne Raten lesbar.
  if (/^[$(`<{]/.test(erster[0] ?? '')) return unklar();
  const a = schrittAbsicht(erster);
  if (!a) {
    const prog = basis(erster.find((x) => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(x) && x !== 'sudo') ?? '');
    return unklar(/^[A-Za-z0-9._+-]+$/.test(prog) ? prog : '');
  }
  const weitere = bedeutsam.length - 1;
  const text = (a.sudo ? `Mit Administratorrechten: ${a.text}` : a.text)
    + (weitere === 1 ? ', danach ein weiterer Befehl' : weitere > 1 ? `, danach ${weitere} weitere Befehle` : '');
  return { text: kuerzen(text), quelle: 'abgeleitet' };
}

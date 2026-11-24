// Der Electron-Profilordner (userData) eines Test-Kerns liegt in der Testheimat.
//
// Befund 24.10.2026: Ein kopfloser Kern aus einer Testsuite (WB_TESTUMGEBUNG=1, eigenes HOME aus mktemp)
// startete seine Hilfsprozesse mit --user-data-dir=~/Library/Application Support/agent-workbench -- dem
// Profil der LIVE-App. Electron bestimmt userData unter macOS ueber die System-API (Application Support
// der angemeldeten Person), nicht ueber $HOME; ein Test-HOME schuetzte also alle anderen Pfade des Kerns
// (die ueber homedir() laufen), nur dieses Profil nicht. Jede App-Suite am Mac oeffnete damit die
// LevelDB, den Speicher und die Preferences der laufenden Live-App.
//
// Regel: Ist der Kern in der Testumgebung (WB_TESTUMGEBUNG=1) ODER zeigt $HOME nicht auf die echte Heimat
// der Kennung (os.userInfo liest die Nutzerdatenbank, nicht $HOME), liegt userData unter
// <HOME>/.config/agent-workbench/electron-profil. Ein Live-Kern setzt keins von beiden und behaelt den
// Electron-Vorgabeort.
import { realpathSync } from 'node:fs';
import { homedir, userInfo } from 'node:os';
import { join, resolve } from 'node:path';

// Ein HOME mit Schraegstrich am Ende oder ueber einen Symlink ist dieselbe Heimat; roh verglichen schickte es
// einen Live-Kern ins Testprofil. Gibt es den Pfad nicht, gilt die aufgeloeste Schreibweise.
function normal(p: string): string {
  try {
    return realpathSync(p);
  } catch {
    return resolve(p);
  }
}

export function echteHeimat(): string {
  try {
    return userInfo().homedir;
  } catch {
    return '';
  }
}

export function testProfilPfad(
  env: NodeJS.ProcessEnv,
  heim: string = homedir(),
  echt: string = echteHeimat(),
): string | null {
  const test = env.WB_TESTUMGEBUNG === '1' || (echt !== '' && normal(heim) !== normal(echt));
  return test ? join(heim, '.config', 'agent-workbench', 'electron-profil') : null;
}

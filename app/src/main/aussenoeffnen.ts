import { harnessUpdateAutomatikErlaubt } from './harnessupdate';

// DER EINE WEG NACH DRAUSSEN.
//
// Anlass: Der Mensch am Mac bekam wiederholt die Apple-Meldung, eine .png koenne nicht
// geoeffnet werden. Die Mac-Suite test-mac-ordner.sh klickt eine absichtlich
// kaputte PNG (PNG-Kopf plus Nullbyte) in der Testinstanz an; der Kern gab sie
// an `shell.openPath`, und der echte Mac startete Vorschau auf dem Schirm des
// Menschen. Dasselbe galt fuer jeden anderen Weg nach draussen.
//
// Jeder Aufruf von `shell.openPath`, `shell.showItemInFolder` und
// `shell.openExternal` im Kern geht deshalb hierher. Im Betrieb ruft die Funktion
// das System, genau wie vorher. In einer Testinstanz ruft sie es NIE; sie merkt
// sich die Absicht, und die Tests lesen sie ueber den Steuerkanal (`awb-ctl
// aussen`) bzw. in der Antwort von `awb:chat-pfad-oeffnen` (Mac: `ui.pfad.zuletzt.extern`).
//
// WAS EINE TESTINSTANZ IST, entscheidet keine neue Naht: derselbe Massstab wie
// beim Harness-Update (`harnessUpdateAutomatikErlaubt`) -- privates HOME oder ein
// Testmerkmal in der Umgebung. Die Socket-Variablen zaehlen ausdruecklich nicht,
// weil Kernstart.swift sie jedem Produktivkern mitgibt.

export type AussenArt = 'openPath' | 'showItemInFolder' | 'openExternal';

export interface AussenSystem {
  openPath(pfad: string): Promise<string>;
  showItemInFolder(pfad: string): void;
  openExternal(url: string): Promise<void>;
}

export interface AussenAbsicht {
  art: AussenArt;
  ziel: string;
  /** true nur, wenn das System wirklich gerufen wurde -- in einer Testinstanz nie. */
  ausgefuehrt: boolean;
}

const MAX_ABSICHTEN = 200;
const absichten: AussenAbsicht[] = [];

export function istTestinstanz(env: NodeJS.ProcessEnv = process.env): boolean {
  // WB_TESTUMGEBUNG=1 ist die Testmarke der Werkzeuge (lib-testumgebung.sh); ein Live-Kern setzt sie nie.
  return env.WB_TESTUMGEBUNG === '1' || !harnessUpdateAutomatikErlaubt(env);
}

/**
 * Oeffnet `ziel` ueber das System des Menschen -- ausser in einer Testinstanz,
 * dort wird nur die Absicht gemerkt. Gibt die Absicht zurueck, damit der
 * Aufrufer sie weitermelden kann.
 */
export async function aussenOeffnen(
  system: AussenSystem,
  art: AussenArt,
  ziel: string,
  env: NodeJS.ProcessEnv = process.env,
): Promise<AussenAbsicht> {
  const test = istTestinstanz(env);
  const absicht: AussenAbsicht = { art, ziel, ausgefuehrt: !test };
  absichten.push(absicht);
  if (absichten.length > MAX_ABSICHTEN) absichten.shift();
  if (test) return absicht;
  if (art === 'openPath') await system.openPath(ziel);
  else if (art === 'showItemInFolder') system.showItemInFolder(ziel);
  else await system.openExternal(ziel);
  return absicht;
}

/** Die gemerkten Absichten, aelteste zuerst (fuer den Steuerkanal und die Tests). */
export function aussenAbsichten(): AussenAbsicht[] {
  return absichten.map((a) => ({ ...a }));
}

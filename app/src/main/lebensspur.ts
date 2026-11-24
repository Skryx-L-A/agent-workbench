// Welche Sitzungen liefen noch, als dieses Programm zuletzt hingesehen hat.
//
// WARUM ES DIESE DATEI GIBT (10./11.08., gemessen). Um 23:44 riss eine
// Kernel-Panik den Rechner weg -- mit ihm den tmux-Server und dieses Programm.
// Die Zustandsdateien der vier hiesigen Sitzungen lagen unveraendert auf der
// Platte, `leseSessions` hat sie auch gelesen und richtig als 'stopped'
// gefuehrt. Nur sah alice sie nicht: `sichtbare()` blendet beendete
// Sitzungen aus (A12, `showStopped` steht in seiner ui.json auf false), und
// uebrig blieb genau eine -- ProjectX auf peer, deren tmux den Absturz
// ueberlebt hatte. Der Fortsetzen-Knopf haengt an derselben Liste, also war
// auch er weg.
//
// A12 IST NICHT FALSCH, ES IST ZU GROB. Der Filter behandelt jede beendete
// Sitzung gleich, und das sind zwei sehr verschiedene Dinge:
//
//   BEENDET     Jemand hat sie geschlossen. Das Programm hat dabei zugesehen.
//               Sie soll verschwinden -- genau dafuer ist A12 da.
//   VERLOREN    Sie lief noch, als dieses Programm zuletzt hinsah, und jetzt
//               ist sie weg, ohne dass wir das Ende gesehen haetten. Das ist
//               eine Nachricht und kein Aufraeumen.
//
// Es ist dieselbe Unterscheidung, die auf der Sitzungsebene schon einmal Geld
// gekostet hat (sessions.ts, 'unreachable' NEBEN 'stopped'): was man nicht
// gesehen hat, ist etwas anderes als das, was es nicht mehr gibt. Hier eine
// Ebene weiter -- ein Ende, das niemand gesehen hat, ist etwas anderes als
// eines, das jemand herbeigefuehrt hat.
//
// WIE ES GEMESSEN WIRD, ohne den Absturz selbst erkennen zu muessen: Dieses
// Programm sieht in jedem Takt, welche Sitzungen leben, und schreibt genau
// diese Menge hierher. Beim naechsten Start steht in der Datei, was beim
// LETZTEN Blick lief. Eine Sitzung, die dort steht, heute aber nicht mehr
// laeuft und in diesem Lauf auch nie lief, ist verloren. Ein Absturz braucht
// dafuer keinen eigenen Nachweis: er ist genau der Fall, in dem das Programm
// den letzten Stand nicht mehr fortschreiben konnte.
//
// WAS DER PREIS DAVON IST, ausdruecklich: Wer das Fenster schliesst und DANACH
// im Terminal seine Sitzungen beendet, findet sie beim naechsten Start als
// verloren gemeldet. Das ist die richtige Seite zum Irren -- die Meldung
// lautet "lief noch, als ich zuletzt hinsah", und das stimmt dann ja. Sie geht
// von selbst wieder weg, sobald die Sitzung wieder laeuft oder ihre
// Zustandsdatei geloescht ist (Sitzungsmenue).
//
// ZWEI GRENZEN DAZU, beide aus der Durchsicht vom 11.08. -- ohne sie war der
// obige Satz nach jedem gewoehnlichen Neustart falsch, und zwar fuer ALLE
// Sitzungen auf einmal statt fuer eine:
//
//   1. EIN SAUBERES ENDE LEERT DIE SPUR NICHT MEHR, SIE ENTSCHEIDET DER
//      RECHNERSTART (10.11.). Bis dahin loeschte `sauberBeendet` die Spur, und
//      der Grund war richtig: wer sein Fenster ordentlich zumacht, hat kein Ende
//      verpasst, er hat aufgehoert hinzusehen. Falsch war die Folgerung fuer
//      einen Neustart des Rechners. macOS beendet das Programm dabei ORDENTLICH,
//      waehrend der tmux-Server noch lebt (gemessen 10.11.: die Herzschlaege der
//      Kontextwaechter liefen bis kurz vor dem Neustart, die Spur war vorher leer);
//      danach war jede hiesige Sitzung weg und keine trug einen Fortsetzen-Knopf.
//      Jetzt schreibt ein sauberes Ende die Spur um -- dieselben Eintraege, dazu
//      `ende: "sauber"` und `endeZeit` --, und der naechste Start vergleicht mit
//      dem Startzeitpunkt des Rechners (rechnerstart.ts):
//
//        Rechnerstart NACH dem sauberen Ende   Es gab einen Neustart dazwischen.
//                                              Jede hiesige Sitzung der Spur,
//                                              die nicht mehr laeuft, wird
//                                              angeboten -- als „nach Neustart
//                                              fortsetzen" (`nachNeustart`), nicht
//                                              als verloren: niemand hat etwas
//                                              verpasst, der Rechner wurde
//                                              absichtlich neu gestartet.
//        Rechnerstart VOR dem sauberen Ende    Nur das Fenster war zu. Wie bisher:
//                                              nichts, die Spur gilt als leer.
//        Rechnerstart nicht lesbar             Wie bisher: nichts. Der Grund geht
//                                              ins Protokoll, es gibt keine
//                                              Meldung auf Verdacht.
//
//      Eine Sitzung, die LEBT, wird nie angeboten: `sessions.ts` traegt `verloren`
//      nur an 'stopped'. Eine Spur OHNE sauberes Ende (Absturz, harter Abschuss)
//      wird wie bisher gelesen -- `nachNeustart` entsteht nur aus dieser einen
//      Entscheidung und bleibt danach an der Sitzung, bis sie laeuft oder ihre
//      Zustandsdatei weg ist. Ferne Sitzungen sagt der Rechnerstart dieses Macs
//      nichts; sie fallen bei einem sauberen Ende weiter aus der Spur.
//      DIE KEHRSEITE, ausdruecklich: Stirbt der Rechner, WAEHREND das Fenster
//      zu ist, meldet niemand einen Verlust. Die Spur spricht nur fuer die
//      Zeit, in der dieses Programm hingesehen hat, und behauptet fuer die
//      uebrige nichts. Ein Neustart OHNE geschlossenes Fenster (harter Abschuss
//      durch das System) hinterlaesst gar kein sauberes Ende und bleibt der
//      Absturzfall.
//   2. EINE ALTE SPUR VERFAELLT (`SPUR_VERFALL_MS`, sieben Tage). Was ausser
//      dem sauberen Ende noch bleibt -- ein hartes Abschiessen des Fensters,
//      ein Stromausfall bei geschlossenem Fenster, eine lange Pause --, ist
//      nach einer Woche keine Auskunft mehr ueber heute. Der Zeitpunkt steht
//      als `stand` in der Datei (bei einem sauberen Ende gleich `endeZeit`); ein
//      Lauf schreibt ihn in jedem Takt fort, in dem sich etwas aendert. Das gilt
//      auch fuer `nachNeustart`: nach einer Woche ohne Fortsetzen verfaellt das
//      Angebot.
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { rechnerStart } from './rechnerstart';

/** Eine Sitzung, so wie diese Datei sie braucht -- mehr wird nicht gelesen. */
export interface SpurSitzung {
  id: string;
  /**
   * Auf WELCHER Maschine sie liegt. Sie steht hier, weil "konnten wir
   * nachsehen?" je Maschine beantwortet wird und nicht fuer alle zusammen
   * (siehe `spurDurchgang`).
   */
  machine: string;
  /** Wurde ein lebender tmux-Pane fuer sie GESEHEN? */
  alive: boolean;
}

/** Ein Eintrag der Spur: dieselbe Sitzung, nur ohne die Frage nach dem Pane. */
export interface SpurEintrag {
  id: string;
  machine: string;
  /**
   * Der Rechner wurde neu gestartet, waehrend sie in der Spur stand (10.11.):
   * sie wird als „nach Neustart fortsetzen" gezeigt, nicht als verloren. Fehlt
   * das Feld, ist es ein gewoehnlicher Eintrag.
   */
  nachNeustart?: boolean;
}

/**
 * Wie lange eine nicht fortgeschriebene Spur ueberhaupt noch etwas aussagt.
 * Sieben Tage: lang genug, dass ein Absturz am Freitag am Montag noch gemeldet
 * wird, kurz genug, dass eine Datei aus einer vergessenen Woche nicht als
 * Nachricht ueber heute auftaucht.
 */
export const SPUR_VERFALL_MS = 7 * 24 * 60 * 60 * 1000;

/**
 * Der reine Kern, ohne Datei: aus dem letzten Stand, dem, was dieser Lauf
 * schon lebend gesehen hat, und dem aktuellen Durchgang folgt beides -- was
 * als verloren gilt und was als naechster Stand auf die Platte gehoert.
 *
 * Ausdruecklich getrennt von der Klasse darunter, damit ein Test beides ohne
 * Dateisystem messen kann: die Regel ist die Aussage, die Datei nur ihr
 * Gedaechtnis.
 *
 * `einsehbar` WIRD JE MASCHINE GEFRAGT (Durchsicht 11.08.), nicht einmal fuer
 * alle. Der Grund ist derselbe Satz, der eine Ebene hoeher schon zweimal steht
 * (sessions.ts, Kopf und `unreachable`): was man nicht sehen konnte, ist nicht
 * dasselbe wie das, was es nicht mehr gibt. Vorher hing die ganze Frage am
 * hiesigen tmux-Befund, und das ging in beide Richtungen schief:
 *
 *   Der erste Durchgang laeuft SYNCHRON beim Start, da kann keine SSH-Antwort
 *   da sein (remote.ts). Ferne Sitzungen fehlen also in `jetzt`, und wer sie
 *   dann aus der Spur streicht, kann einen fernen Verlust nie mehr melden.
 *
 *   Andersherum sammelt eine bloss stille Maschine ihre Sitzungen als
 *   verloren ein, obwohl niemand nachgesehen hat -- dieselbe Falschaussage
 *   ueber fremden Zustand, gegen die es 'unreachable' ueberhaupt gibt.
 *
 * Was von einer nicht einsehbaren Maschine kommt, wird deshalb WEDER beurteilt
 * NOCH gestrichen: es bleibt unveraendert in der Spur stehen, bis die Maschine
 * wieder antwortet.
 */
export function spurDurchgang(
  vorlauf: SpurEintrag[],
  schonGesehen: Iterable<string>,
  jetzt: SpurSitzung[],
  einsehbar: (machine: string) => boolean,
): { verlorene: string[]; nachNeustart: string[]; naechsterStand: SpurEintrag[] } {
  const beurteilbar = jetzt.filter((s) => einsehbar(s.machine));
  const bekannt = new Set(beurteilbar.map((s) => s.id));
  const lebend = beurteilbar.filter((s) => s.alive).map((s) => ({ id: s.id, machine: s.machine }));
  const gesehen = new Set([...schonGesehen, ...lebend.map((s) => s.id)]);
  // Verloren bleibt verloren, bis die Sitzung wieder laeuft oder ihre
  // Zustandsdatei verschwindet -- sonst waere die Nachricht nach einem
  // einzigen Takt wieder fort, und gerade nach einem Absturz sitzt niemand
  // im selben Moment davor.
  const offen = vorlauf.filter((e) => einsehbar(e.machine) && bekannt.has(e.id) && !gesehen.has(e.id));
  // Und was gerade nicht zu beurteilen war, reist unveraendert mit -- siehe
  // oben. Es taucht in `verlorene` NICHT auf: eine Luecke ist keine Nachricht.
  const unklar = vorlauf.filter((e) => !einsehbar(e.machine));
  // Auf die Platte geht, was JETZT lebt, plus die noch offenen Verluste. Ohne
  // die zweite Haelfte waere die Nachricht nach dem ersten Start nach dem
  // Absturz verbraucht: dort lebt keine der verlorenen Sitzungen mehr, der
  // Stand waere leer, und ein zweiter Start wuesste von nichts mehr.
  const stand = new Map<string, SpurEintrag>();
  for (const e of [...lebend, ...offen, ...unklar]) stand.set(e.id, e);
  // `nachNeustart` ist DIESELBE Menge `offen`, nur in der anderen Sprache: wer
  // nach einem Neustart angeboten wird, ist nicht „ohne sichtbares Ende
  // verschwunden", und die Meldung dafuer waere falsch.
  return {
    verlorene: offen.filter((e) => !e.nachNeustart).map((e) => e.id),
    nachNeustart: offen.filter((e) => e.nachNeustart).map((e) => e.id),
    naechsterStand: [...stand.values()].sort((a, b) => a.id.localeCompare(b.id)),
  };
}

/** Eine Eintragszeile der Datei lesen -- nachsichtig, wie bisher. */
function eintraege(roh: unknown): SpurEintrag[] {
  if (!Array.isArray(roh)) return [];
  return roh
    .map((e): SpurEintrag => {
      if (typeof e === 'string') return { id: e, machine: '' };
      const x = e as SpurEintrag | null;
      const eintrag: SpurEintrag = { id: String(x?.id ?? ''), machine: String(x?.machine ?? '') };
      if (x?.nachNeustart === true) eintrag.nachNeustart = true;
      return eintrag;
    })
    .filter((e) => !!e.id);
}

/**
 * Was beim Start aus der Datei auf der Platte wird (10.11.) -- der reine Kern,
 * ohne Dateisystem und ohne die Uhr des Rechners, damit ein Test jeden Fall mit
 * eingespeister Zeit messen kann. Die Begruendung steht im Kopf der Datei,
 * Grenze 1 und 2.
 *
 * `startzeit` wird NUR gefragt, wenn die Datei ein sauberes Ende traegt: nur
 * dann haengt die Entscheidung daran.
 */
export function spurAmStart(
  roh: { sitzungen?: unknown; stand?: unknown; ende?: unknown; endeZeit?: unknown },
  jetzt: number,
  startzeit: () => number | null,
  hiesig: (machine: string) => boolean,
): { vorlauf: SpurEintrag[]; neustart: boolean; sauber: boolean; grund: string } {
  const sauber = roh.ende === 'sauber';
  const alle = eintraege(roh.sitzungen);
  // Zu alt ist so gut wie nicht da (siehe SPUR_VERFALL_MS oben). Eine Datei
  // ohne `stand` stammt aus einer Fassung vor dieser Grenze und wird wie
  // eine frische behandelt -- verfallen kann nur, was einen Zeitpunkt hat.
  const stand = typeof roh.stand === 'string' ? Date.parse(roh.stand) : NaN;
  if (Number.isFinite(stand) && jetzt - stand > SPUR_VERFALL_MS) {
    return { vorlauf: [], neustart: false, sauber, grund: 'Spur verfallen' };
  }
  if (!sauber) return { vorlauf: alle, neustart: false, sauber, grund: '' };
  // Ein sauberes Ende. Was schon als „nach Neustart" angeboten wurde und noch
  // nicht fortgesetzt ist, bleibt in jedem Fall stehen: das Angebot verfaellt
  // nicht dadurch, dass jemand das Fenster zumacht, bevor er darauf gedrueckt hat.
  const offenes = alle.filter((e) => e.nachNeustart && hiesig(e.machine));
  const endeRoh = typeof roh.endeZeit === 'string' ? roh.endeZeit : roh.stand;
  const ende = typeof endeRoh === 'string' ? Date.parse(endeRoh) : NaN;
  if (!Number.isFinite(ende)) {
    return { vorlauf: offenes, neustart: false, sauber, grund: 'Zeitpunkt des sauberen Endes nicht lesbar' };
  }
  const start = startzeit();
  if (start === null || !Number.isFinite(start)) {
    return { vorlauf: offenes, neustart: false, sauber, grund: 'Startzeit des Rechners nicht lesbar' };
  }
  if (start > ende) {
    const angebot = alle.filter((e) => hiesig(e.machine)).map((e): SpurEintrag => ({ ...e, nachNeustart: true }));
    return { vorlauf: angebot, neustart: true, sauber, grund: '' };
  }
  return { vorlauf: offenes, neustart: false, sauber, grund: '' };
}

/** Was `LebensSpur` von seiner Umgebung braucht -- eingespeist, damit Tests es steuern. */
export interface SpurUmgebung {
  /** Startzeitpunkt des Rechners in ms seit 1970, null wenn nicht lesbar. */
  startzeit?: () => number | null;
  /** Die Uhr in ms seit 1970. */
  jetzt?: () => number;
  /** Der Name DIESER Maschine; leer oder gleich = hiesig. */
  hiesigeMaschine?: string;
  /** Eine Zeile fuer das Protokoll (stderr der App). */
  protokoll?: (zeile: string) => void;
}

/**
 * Das Gedaechtnis dazu. Es liegt im eigenen Verzeichnis des Programms und NICHT
 * bei den Zustandsdateien der Sessions -- die gehoeren den wb-Werkzeugen, und
 * zwei Schreiber auf derselben Datei sind ein Fehler, den dieses Haus schon
 * einmal bezahlt hat (siehe uistate.ts).
 */
export class LebensSpur {
  private readonly datei: string;
  private readonly stateDir: string;
  private readonly jetzt: () => number;
  /** Was beim letzten Blick des VORIGEN Laufs lief. */
  private vorlauf: SpurEintrag[] = [];
  /** Was dieser Lauf schon einmal lebend gesehen hat. */
  private readonly gesehen = new Set<string>();
  /** Der zuletzt geschriebene Stand -- damit nicht in jedem Takt geschrieben wird. */
  private geschrieben = '';
  /**
   * Dieser Start hat aus einem sauberen Ende UND einem Rechnerstart danach auf
   * einen Neustart geschlossen. Nur dann meldet main.ts einmal den Satz „nach
   * dem Neustart lassen sich n Sitzungen fortsetzen"; eine Spur, die das
   * Merkmal aus einem frueheren Start mitbringt, ist kein neuer Anlass.
   */
  readonly neustartErkannt: boolean = false;

  constructor(stateDir: string, umgebung: SpurUmgebung = {}) {
    this.stateDir = stateDir;
    this.datei = join(stateDir, 'lebensspur.json');
    this.jetzt = umgebung.jetzt ?? Date.now;
    const hiesig = (m: string): boolean => !m || !umgebung.hiesigeMaschine || m === umgebung.hiesigeMaschine;
    const protokoll = umgebung.protokoll ?? (() => {});
    try {
      const roh = JSON.parse(readFileSync(this.datei, 'utf8')) as Parameters<typeof spurAmStart>[0];
      if (Array.isArray(roh.sitzungen)) {
        const urteil = spurAmStart(roh, this.jetzt(), umgebung.startzeit ?? rechnerStart, hiesig);
        this.vorlauf = urteil.vorlauf;
        this.neustartErkannt = urteil.neustart;
        if (urteil.neustart) {
          protokoll(`Lebensspur: Neustart des Rechners nach dem sauberen Ende erkannt, ${urteil.vorlauf.length} Sitzung(en) zur Fortsetzung vorgemerkt.`);
        } else if (urteil.grund && urteil.grund !== 'Spur verfallen') {
          protokoll(`Lebensspur: keine Fortsetzen-Meldung -- ${urteil.grund}.`);
        }
        // Ein laufendes Programm hat kein „sauberes Ende": der erste Takt
        // schreibt die Datei ohne dieses Kennzeichen neu, auch wenn sich an der
        // Liste nichts geaendert hat.
        this.geschrieben = urteil.sauber ? '' : JSON.stringify(this.vorlauf);
      }
    } catch {
      // Ein erster Start ohne alles muss funktionieren -- dann ist nichts
      // verloren, weil noch nie jemand hingesehen hat.
    }
  }

  /**
   * Die KANDIDATEN: Sitzungen, die beim letzten Blick liefen und in diesem Lauf
   * noch nie. Ob daraus eine Meldung wird, entscheidet die Liste selbst -- den
   * Merker traegt nur, wer dort auch wirklich 'stopped' ist (sessions.ts). Was
   * hier von einer stillen Maschine mitreist, faellt dort also von allein
   * heraus, weil ihre Sitzungen 'unreachable' heissen und nicht 'stopped'.
   * Dazu gehoeren auch die `nachNeustart`-Sitzungen: sie bleiben in der Liste und
   * tragen den Fortsetzen-Knopf.
   */
  verlorene(): string[] {
    return this.vorlauf.filter((e) => !this.gesehen.has(e.id)).map((e) => e.id);
  }

  /** Davon die Sitzungen, die wegen eines Rechnerneustarts angeboten werden. */
  nachNeustart(): string[] {
    return this.vorlauf.filter((e) => e.nachNeustart && !this.gesehen.has(e.id)).map((e) => e.id);
  }

  /**
   * Ein Durchgang. `einsehbar` ist die Bedingung, unter der die Beobachtung
   * ueberhaupt etwas wert ist: war tmux nicht auszufuehren oder unzerlegbar,
   * ist `alive` fuer JEDE Sitzung false, und ein Fortschreiben wuerde genau die
   * Auskunft loeschen, fuer die es diese Datei gibt. Sie wird JE MASCHINE
   * gestellt (Durchsicht 11.08.) -- warum, steht bei `spurDurchgang`.
   * Geliefert werden die Sitzungen, die OHNE sichtbares Ende weg sind; die
   * `nachNeustart`-Angebote stehen bei `nachNeustart()`.
   */
  durchgang(jetzt: SpurSitzung[], einsehbar: (machine: string) => boolean): string[] {
    const { verlorene, naechsterStand } = spurDurchgang(this.vorlauf, this.gesehen, jetzt, einsehbar);
    for (const s of jetzt) if (s.alive && einsehbar(s.machine)) this.gesehen.add(s.id);
    this.vorlauf = naechsterStand;
    const stand = JSON.stringify(naechsterStand);
    if (stand !== this.geschrieben) {
      this.geschrieben = stand;
      this.speichern(naechsterStand);
    }
    return verlorene;
  }

  /**
   * Das Fenster geht ordentlich zu: ab hier sieht niemand mehr hin. Die Spur
   * bleibt stehen, mit `ende: "sauber"` und dem Zeitpunkt -- ob daraus beim
   * naechsten Start etwas wird, entscheidet der Rechnerstart (Kopf, Punkt 1).
   * Synchron, weil auf dem Beenden-Weg keine Zeit fuer Asynchrones bleibt
   * (derselbe Grund wie bei `zustandZurueckSync` in main.ts).
   */
  sauberBeendet(): void {
    this.geschrieben = JSON.stringify(this.vorlauf);
    this.speichern(this.vorlauf, true);
  }

  private speichern(sitzungen: SpurEintrag[], sauber = false): void {
    try {
      mkdirSync(this.stateDir, { recursive: true });
      const tmp = `${this.datei}.${process.pid}.tmp`;
      const zeit = new Date(this.jetzt()).toISOString();
      const inhalt = sauber
        ? { stand: zeit, ende: 'sauber', endeZeit: zeit, sitzungen }
        : { stand: zeit, sitzungen };
      writeFileSync(tmp, JSON.stringify(inhalt, null, 2));
      renameSync(tmp, this.datei);
    } catch {
      // Ein nicht schreibbarer Zustand darf die Oberflaeche nicht anhalten.
    }
  }
}

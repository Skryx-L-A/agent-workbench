// „WELT BRAUCHT DICH“: WANN EIN RECHTEANTRAG ODER EINE FRAGE EINER WELT ALS PUSH HINAUSGEHT.
//
// Rechteantraege gehen als Push-Meldung hinaus und erscheinen auf dem Handy. Ohne diesen Weg
// erreicht ein Antrag des Hauptagenten den Menschen nur als unmarkierte Direktnachricht im
// Postfach der Welt, eine Frage der Welt gar nicht (nur der Tab „Agents“).
//
// Diese Datei sagt, WAS offen ist: aus den Welten des Kerns (welten.ts) die Antraege und Fragen,
// die auf den MENSCHEN warten. Sie liest nur Kennung, Art, Stand und den Namen der Welt -- nie
// den Text einer Frage, nie die Rechte oder den Grund eines Antrags. Das Senden, Entprellen und
// Erledigen macht MobilePushService.weltAnliegen().

import type { WeltAnliegenRef } from '../../../mobile-shared/vertrag';

/** Der Mensch der Welt (`WELT_MENSCH` in welten.ts); ein Antrag an ihn hat `an === 'mensch'`. */
const MENSCH = 'mensch';

/** Das, was der Kern aus einer Welt braucht -- ein schmaler Ausschnitt von `Welt` (welten.ts). */
export interface WeltAusschnitt {
  readonly id: string;
  readonly name: string;
  /** Die Ablage liegt auf einer anderen Maschine: dann meldet deren Kern, nicht dieser. */
  readonly fern: boolean;
  /** Die Welt ist ohne Lesefehler gelesen (`konsistent`, keine `fehler`); nur dann zaehlt „nicht mehr offen“ als erledigt. */
  readonly konsistent: boolean;
  readonly fehler: readonly string[];
  /** Nur fuer nicht-ferne Welten von Belang: der Pfad der Ablage auf dieser Maschine; der Dienst merkt ihn sich (F4). */
  readonly ablage?: string;
  readonly fragen: readonly { readonly id: string; readonly stand: string; readonly gestellt: string }[];
  readonly rechteantraege: readonly {
    readonly id: string; readonly an: string; readonly stand: string; readonly abgelaufen: boolean; readonly zeit: string;
  }[];
}

export interface WeltAnliegen extends WeltAnliegenRef {
  /** Stabil je Antrag bzw. Frage: `<welt>/<art>/<id>`. */
  readonly schluessel: string;
  /** Wann gestellt (ms); null, wenn die Zeit nicht lesbar ist. Der Dienst meldet nur Frisches (siehe WELT_FRISCH_MS). */
  readonly gestellt: number | null;
  /** Der Pfad der Ablage auf dieser Maschine (nur nicht-ferne Welten); der Dienst merkt ihn sich am Merkeintrag (F4). */
  readonly ablage?: string;
}

export interface WeltAnliegenStand {
  /** Alles, was jetzt auf den Menschen wartet. */
  readonly offen: readonly WeltAnliegen[];
  /** Die Welten, die jetzt sicher gelesen sind: nur fuer sie darf ein verschwundener Eintrag als erledigt gelten. */
  readonly gelesen: ReadonlySet<string>;
  /** Jede Welt des Durchgangs, auch die fern gelesene: fehlt eine hier, fehlt sie ueberhaupt (F4). */
  readonly welten: ReadonlySet<string>;
  /** Vollstaendiger, erfolgreicher Durchgang: jede lokale Welt gelesen, kein Lesefehler (F4). Eine leere Liste zaehlt als vollstaendig; die Ablagepruefung bleibt die zweite Schranke. */
  readonly vollstaendig: boolean;
}

const zeitMs = (iso: string): number | null => {
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : null;
};

export function anliegenSchluessel(weltId: string, art: WeltAnliegenRef['art'], id: string): string {
  return `${weltId}/${art}/${id}`;
}

/**
 * Offen ist ein Rechteantrag, wenn er an den Menschen geht (`an`), noch den Stand `offen` hat und nicht
 * abgelaufen ist (ein Abgelaufener ist nicht mehr entscheidbar). Eine Frage ist offen mit Stand `offen`;
 * `ask_question` stellt nur der Hauptagent, und jede geht an den Menschen. Ein `agent-antrag` (Teamleiter an den
 * Hauptagenten) und ein Rechteantrag eines Teammitglieds (an den Hauptagenten) gehen nie an ihn und zaehlen nicht.
 */
export function weltAnliegenAus(welten: readonly WeltAusschnitt[]): WeltAnliegenStand {
  const offen: WeltAnliegen[] = [];
  const gelesen = new Set<string>();
  const gesehen = new Set<string>();
  let vollstaendig = true;
  for (const w of welten) {
    gesehen.add(w.id);
    if (w.fern) continue;
    if (w.konsistent && w.fehler.length === 0) gelesen.add(w.id);
    else vollstaendig = false;
    for (const f of w.fragen) {
      if (f.stand === 'offen') {
        offen.push({ welt_id: w.id, welt_name: w.name, art: 'frage', anliegen_id: f.id, schluessel: anliegenSchluessel(w.id, 'frage', f.id), gestellt: zeitMs(f.gestellt), ablage: w.ablage });
      }
    }
    for (const x of w.rechteantraege) {
      if (x.an === MENSCH && x.stand === 'offen' && !x.abgelaufen) {
        offen.push({ welt_id: w.id, welt_name: w.name, art: 'rechteantrag', anliegen_id: x.id, schluessel: anliegenSchluessel(w.id, 'rechteantrag', x.id), gestellt: zeitMs(x.zeit), ablage: w.ablage });
      }
    }
  }
  return { offen, gelesen, welten: gesehen, vollstaendig };
}

/**
 * Der Name der Welt ist vom Menschen gewaehlt, aber Freitext: ohne Steuerzeichen und Zeilenumbrueche, gekuerzt.
 * Der Sperrbildschirm zeigt nur ihn, die Art und den Anlass -- nie den Inhalt des Antrags (dort koennen Pfade,
 * Muster fuer Bash-Rechte oder Bezeichner von Zugaengen stehen).
 */
export function weltNameKurz(name: string): string {
  // Hoechstens 40 Zeichen, gezaehlt in Graphemen (was der Mensch als ein Zeichen sieht),
  // nie in Bytes oder UTF-16-Einheiten; ein Emoji oder ein Buchstabe mit Akzent wird nie zerschnitten.
  // Alle unsichtbaren Formatzeichen (Unicode-Kategorie Cf: Richtungszeichen, U+061C, U+00AD, U+180E, U+2060-U+2064,
  // U+FEFF, U+FFF9-U+FFFB ...) fallen weg (Review 18.10.). Es bleiben nur die Verbinder U+200C/U+200D und die
  // Tag-Zeichen U+E0020-U+E007F, die zu Emoji, Schriften und Flaggen gehoeren.
  const sauber = name.replace(/[\u0000-\u001f\u007f-\u009f\u2028\u2029]+/g, ' ')
    .replace(/(?![\u200c\u200d\u{e0020}-\u{e007f}])\p{Cf}/gu, '').replace(/\s+/g, ' ').trim();
  const zeichen = [...GRAPHEME.segment(sauber)].map((teil) => teil.segment);
  return zeichen.length > WELTNAME_MAX ? `${zeichen.slice(0, WELTNAME_MAX - 1).join('').trimEnd()}…` : sauber;
}

/** Hoechstlaenge des Weltnamens auf dem Sperrbildschirm, in Graphemen. */
export const WELTNAME_MAX = 40;
const GRAPHEME = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

export function weltAnliegenText(a: WeltAnliegenRef): string {
  const art = a.art === 'rechteantrag' ? 'Rechteantrag' : 'Frage';
  const name = weltNameKurz(a.welt_name);
  return name ? `Welt „${name}“: ${art} wartet auf dich.` : `Eine Welt: ${art} wartet auf dich.`;
}

/**
 * Deep Link in die PWA (Hash-Route, router.ts). Die Frage hat ihre Route (`#/agents/<welt>/frage/<id>`); fuer den
 * Rechteantrag legt die Handy-Spur `#/agents/<welt>/antrag/<id>` an -- bis dahin faellt der Link auf die Welt zurueck.
 */
export function weltAnliegenLink(a: WeltAnliegenRef): string {
  const teil = a.art === 'rechteantrag' ? 'antrag' : 'frage';
  return `/#/agents/${encodeURIComponent(a.welt_id)}/${teil}/${encodeURIComponent(a.anliegen_id)}`;
}

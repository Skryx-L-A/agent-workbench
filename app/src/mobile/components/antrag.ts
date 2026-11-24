/**
 * EIN RECHTEANTRAG (Push-Art `welt.braucht-dich`: Push-Meldung und Handy-Anzeige).
 *
 * Ein Agent beantragt Aenderungen an seinen eigenen Rechten beim Menschen oder
 * beim Hauptagenten (`kind: rechte-antrag`, `main/welten.ts`). Das Handy entscheidet
 * Rechteantraege von Welten mit Geraetebeleg: die Ansicht bietet Freigeben und Ablehnen an, wenn der Antrag entscheidbar ist
 * (`rechteantragEntscheidbar`). Die Handlung ist `rights_request.decide` und
 * traegt `changes` unveraendert zurueck; eine Bemerkung gibt es nicht
 * (kein Freitext, nur der Entscheidungsknopf). Sonst bleibt die
 * Ansicht zum Lesen. Der Inhalt (Rechte, Muster, Grund) steht nur hier in der
 * App, nie in einer Mitteilung, einem Log oder einem Titel: Titel der Ebene ist
 * immer „Rechteantrag“.
 */
import type { AgentAction, ApiError, Decision } from '../../mobile-shared/vertrag';
import type { RechteAntragKurz, RechteKurz, WeltKurz } from './agents-daten';
import { RECHTE_NAMEN, rechteantragEntscheidbar } from './agents-daten';
import { agentFigur, agentName } from './agents';
import { abfrage } from './blatt';
import { ersetzen, h } from './dom';
import { FEHLER_IN_WORTEN } from './stand';
import { abschnitt, hinweis, leerzustand, maschinenName, marke, punkt, seite, type Ansicht, type Umgebung } from './teile';
import { marke as zeitmarke, relativ, uhr } from './zeit';

type Farbe = 'neutral' | 'will' | 'aus' | 'fertig';

/**
 * Das Ablaufdatum liegt meist Tage voraus; `zeit.marke` kennt nur Heute,
 * Gestern und Vergangenes und nennte jede Zukunft „Heute“.
 */
function frist(t: number, jetzt = Date.now()): string {
  const morgen = new Date(jetzt).setHours(24, 0, 0, 0);
  if (t < morgen) return zeitmarke(t, jetzt);
  return `${new Intl.DateTimeFormat('de-DE', { weekday: 'short', day: 'numeric', month: 'long' }).format(t)}, ${uhr(t)}`;
}

/** Der Stand eines Antrags in Worten und Farbe. */
export function antragStand(w: WeltKurz, r: RechteAntragKurz): { text: string; farbe: Farbe } {
  if (r.stand === 'offen' && r.abgelaufen) return { text: 'Abgelaufen', farbe: 'aus' };
  if (r.stand === 'offen') return r.an === 'mensch' ? { text: 'Wartet auf Dich', farbe: 'will' } : { text: `Wartet auf ${agentName(w, r.an)}`, farbe: 'neutral' };
  if (r.stand === 'zurueckgezogen') return { text: 'Zurückgezogen', farbe: 'aus' };
  if (r.entscheidung === 'freigegeben') return { text: 'Freigegeben', farbe: 'fertig' };
  if (r.entscheidung === 'abgelehnt') return { text: 'Abgelehnt', farbe: 'aus' };
  return { text: r.entscheidung ? `Entschieden: ${r.entscheidung}` : 'Entschieden', farbe: 'neutral' };
}

function empfaenger(w: WeltKurz, an: string): string {
  return an === 'mensch' ? 'Dich' : agentName(w, an);
}

/** Ein Rechtewert: Listen Zeile fuer Zeile, Web als Satzteil, Fehlendes ausdruecklich. */
function rechteWert(r: RechteKurz, k: keyof RechteKurz): HTMLElement {
  if (k === 'web') {
    const web = r.web;
    return h('dd', null, web === undefined ? 'nicht vermerkt' : web ? 'erlaubt' : 'nicht erlaubt');
  }
  const l = r[k];
  if (l === undefined) return h('dd', null, 'nicht vermerkt');
  if (!l.length) return h('dd', null, 'keine');
  return h('dd', { class: k === 'skills' ? null : 'mono' }, ...l.map((x) => h('div', { class: 'rechte-eintrag' }, x)));
}

function rechteKarte(titel: string, r: RechteKurz, arten: (keyof RechteKurz)[], klasse: string): HTMLElement[] {
  return [abschnitt(titel),
    h('dl', { class: `karte eigenschaften ${klasse}` },
      ...(arten.length
        ? arten.map((k) => {
          // Listen (ein Eintrag je Zeile) stehen linksbuendig unter der Beschriftung; Kurzes wie „Web: erlaubt“ bleibt rechts.
          const liste = k !== 'web' && (r[k]?.length ?? 0) > 0;
          return h('div', { 'data-art': k, class: liste ? 'mehrzeilig' : null }, h('dt', null, RECHTE_NAMEN[k]), rechteWert(r, k));
        })
        : [h('div', null, h('dt', null, 'Rechte'), h('dd', null, 'keine Angaben'))]))];
}

/** Die Rechtearten, die der Antrag aendert, in der Reihenfolge der Werkbank. */
function geaenderteArten(r: RechteAntragKurz): (keyof RechteKurz)[] {
  return (Object.keys(RECHTE_NAMEN) as (keyof RechteKurz)[]).filter((k) => r.changes[k] !== undefined);
}

/** Hoechstens so viele Eintraege je Rechteart nennt die Bestaetigung; die Seite zeigt alle. */
const BESTAETIGUNG_EINTRAEGE = 6;

/** „Bash-Muster „a“, „b“; Web erlaubt“: was der Antrag aendert, als Satzteil. */
function aenderungInWorten(r: RechteAntragKurz): string {
  const teile = geaenderteArten(r).map((k) => {
    if (k === 'web') return `Web ${r.changes.web ? 'erlaubt' : 'nicht erlaubt'}`;
    const l = r.changes[k] ?? [];
    if (!l.length) return `${RECHTE_NAMEN[k]} keine`;
    const mehr = l.length - BESTAETIGUNG_EINTRAEGE;
    return `${RECHTE_NAMEN[k]} ${l.slice(0, BESTAETIGUNG_EINTRAEGE).map((x) => `„${x}“`).join(', ')}${mehr > 0 ? ` und ${mehr} weitere` : ''}`;
  });
  return teile.length ? teile.join('; ') : 'keine Angaben';
}

function bestaetigungsText(w: WeltKurz, r: RechteAntragKurz, d: Decision): string {
  const wer = agentName(w, r.agent || r.von);
  return d === 'approve'
    ? `Danach gilt für ${wer}: ${aenderungInWorten(r)}.`
    : `Die Rechte von ${wer} bleiben, wie sie sind. Abgelehnt wird: ${aenderungInWorten(r)}.`;
}

/** Die Fehler einer Entscheidung in Worten; die Servermeldungen sind englisch und bleiben aussen vor. */
function entscheidungsFehler(f: ApiError): string {
  switch (f.code) {
    case 'stale_target': return 'Der Antrag hat sich geändert oder ist schon entschieden. Lade die Ansicht neu.';
    case 'capability_unavailable': return 'Diesen Antrag kann das Handy gerade nicht entscheiden. Entscheide ihn am Mac.';
    case 'bad_request': return 'Der Mac hat die Entscheidung nicht angenommen. Entscheide den Antrag am Mac.';
    case 'device_not_paired':
    case 'device_revoked': return FEHLER_IN_WORTEN.device_not_paired ?? f.message;
    default: return 'Die Entscheidung ist nicht angekommen. Prüfe die Verbindung und versuche es noch einmal.';
  }
}

/** `rights_request.decide` aus dem Vertrag; `changes` geht unveraendert aus GET /agents zurueck. */
type RechteEntscheidung = Extract<AgentAction, { type: 'rights_request.decide' }>;

/** Was die Ansicht nach einer Entscheidung zeigt, bis der neue Stand da ist und darueber hinaus. */
type Ausgang = { ok: true; text: string } | { ok: false; text: string; neuLaden: boolean };

function antragInhalt(w: WeltKurz, r: RechteAntragKurz): (HTMLElement | null)[] {
  const st = antragStand(w, r);
  const agent = w.agenten.find((a) => a.id === r.agent) ?? w.agenten.find((a) => a.id === r.von);
  // Gezeigt werden genau die Rechtearten, die der Antrag aendert; „Bisher“ zum Vergleich daneben.
  const arten = geaenderteArten(r);
  return [
    h('section', { class: 'karte antrag-kopf', 'aria-label': `Rechteantrag von ${agentName(w, r.von)}` },
      h('div', { class: 'frage-kopf' },
        // Eigener Platz: auf dem iPad steht die Kachel derselben Figur zugleich links in der Liste.
        agent ? agentFigur(agent, 40, 'antrag') : punkt(st.farbe === 'will' ? 'will' : 'neutral'),
        h('div', { class: 'frage-wer' },
          h('span', { class: 'frage-name' }, agent?.name ?? r.agent),
          h('span', { class: 'frage-meta' }, `${w.name}, ${relativ(r.zeit)}`),
        ),
      ),
      marke(st.text, st.farbe),
      h('p', { class: 'frage-text antrag-grund' }, r.grund || 'Ohne Begründung.'),
    ),
    ...rechteKarte('Beantragt', r.changes, arten, 'antrag-beantragt'),
    ...rechteKarte('Bisher', r.vorher, arten, 'antrag-vorher'),
    abschnitt('Antrag'),
    h('dl', { class: 'karte eigenschaften antrag-daten' },
      h('div', null, h('dt', null, 'Von'), h('dd', null, agentName(w, r.von))),
      r.agent && r.agent !== r.von ? h('div', null, h('dt', null, 'Für'), h('dd', null, agentName(w, r.agent))) : null,
      h('div', null, h('dt', null, 'An'), h('dd', null, empfaenger(w, r.an))),
      h('div', null, h('dt', null, 'Gestellt'), h('dd', null, r.zeit ? zeitmarke(r.zeit) : 'unbekannt')),
      h('div', null, h('dt', null, 'Läuft ab'), h('dd', null, r.abgelaufen ? 'abgelaufen' : r.laeuft_ab ? frist(r.laeuft_ab) : 'unbekannt')),
      h('div', null, h('dt', null, 'Stand'), h('dd', null, st.text)),
      r.entscheidung ? h('div', null, h('dt', null, 'Entscheidung'), h('dd', null, r.entscheidung)) : null,
      r.bemerkung ? h('div', null, h('dt', null, 'Bemerkung'), h('dd', null, r.bemerkung)) : null,
    ),
  ];
}

/** Der ruhige Satz unter einem Antrag, der hier nicht entschieden werden kann. */
function nurLesen(w: WeltKurz, r: RechteAntragKurz): HTMLElement {
  if (r.stand !== 'offen') return hinweis('haken', `${antragStand(w, r).text}. Der Antrag ist erledigt.`);
  if (r.abgelaufen) return hinweis('schloss', 'Der Antrag ist abgelaufen und lässt sich nicht mehr entscheiden.');
  if (w.fern) return hinweis('schloss', `Der Antrag liegt auf ${maschinenName(w.maschine)}. Entscheiden geht nur dort.`);
  return hinweis('schloss', r.an === 'mensch'
    ? 'Entscheiden geht am Mac. Hier steht der Antrag nur zum Lesen.'
    : `Der Antrag wartet auf ${agentName(w, r.an)}. Entscheiden geht am Mac.`);
}

export function antragAnsicht(u: Umgebung, weltId: string, antragId: string): Ansicht {
  const stand = u.stand;
  const welt = (): WeltKurz | undefined => (weltId === '-'
    ? stand.agents?.welten.find((x) => x.rechteantraege.some((r) => r.id === antragId))
    : stand.agents?.welten.find((x) => x.id === weltId));
  const s = seite(u, { titel: 'Rechteantrag', zurueck: weltId === '-' ? 'Agents' : welt()?.name ?? 'Welt', klasse: 'detail-ansicht antrag-ansicht' });
  const koerper = h('div', { class: 'detail-koerper' });
  s.inhalt.append(koerper);
  // Waehrend eine Entscheidung unterwegs ist, sind beide Knoepfe gesperrt.
  let laeuft: Decision | null = null;
  let ausgang: Ausgang | null = null;

  const entscheiden = async (d: Decision): Promise<void> => {
    if (laeuft) return;
    const w = welt();
    const r = w?.rechteantraege.find((x) => x.id === antragId);
    if (!w || !r || !r.changes_roh || !rechteantragEntscheidbar(w, r)) return;
    // Geschickt wird genau die Aenderung, die die Bestaetigung zeigt.
    const changes = r.changes_roh;
    laeuft = d;
    aktualisieren();
    const ja = await abfrage({
      titel: d === 'approve' ? 'Rechteantrag freigeben?' : 'Rechteantrag ablehnen?',
      text: bestaetigungsText(w, r, d),
      ja: d === 'approve' ? 'Freigeben' : 'Ablehnen',
      jaArt: d === 'approve' ? 'normal' : 'zerstoerend',
    });
    if (ja === null) { laeuft = null; aktualisieren(); return; }
    ausgang = null;
    const aktion: RechteEntscheidung = { type: 'rights_request.decide', world_id: w.id, rights_request_id: r.id, decision: d, changes };
    const e = await stand.agentAktion(aktion);
    laeuft = null;
    if (e.ok) {
      const res = e.data.result;
      const meldung = res !== null && typeof res === 'object' && !Array.isArray(res) && typeof res.meldung === 'string' ? res.meldung.trim() : '';
      ausgang = { ok: true, text: meldung || (d === 'approve' ? 'Freigegeben. Die Rechte gelten ab dem nächsten Zug.' : 'Abgelehnt. Die Rechte bleiben, wie sie sind.') };
    } else {
      ausgang = { ok: false, text: entscheidungsFehler(e.fehler), neuLaden: e.fehler.code === 'stale_target' };
    }
    aktualisieren();
  };

  const neuLaden = async (): Promise<void> => {
    ausgang = null;
    aktualisieren();
    await stand.abgleichen();
  };

  /** Freigeben und Ablehnen, oder nach einer Entscheidung ihr Ausgang. */
  const entscheidungsTeil = (w: WeltKurz, r: RechteAntragKurz): (HTMLElement | null)[] => {
    if (ausgang?.ok) return [hinweis('haken', ausgang.text)];
    const fehler = ausgang && !ausgang.ok
      ? h('div', { class: 'entscheidung-fehler antrag-fehler', role: 'alert' }, ausgang.text,
        ausgang.neuLaden ? h('button', { class: 'text-knopf antrag-neu-laden', type: 'button', onclick: () => { void neuLaden(); } }, 'Neu laden') : null)
      : null;
    if (!rechteantragEntscheidbar(w, r)) return [nurLesen(w, r), fehler];
    const gesperrt = laeuft !== null || !stand.darfSchreiben();
    const knopf = (d: Decision, text: string, art: string): HTMLButtonElement => h('button', {
      class: `knopf ${art}`, type: 'button', 'data-entscheidung': d, disabled: gesperrt, 'aria-busy': laeuft === d ? 'true' : null,
      onclick: () => { void entscheiden(d); },
    }, text);
    return [
      h('div', { class: 'entscheidung-knoepfe antrag-knoepfe' }, knopf('reject', 'Ablehnen', 'getoent'), knopf('approve', 'Freigeben', 'primaer')),
      r.an !== 'mensch' ? h('p', { class: 'entscheidung-notiz antrag-notiz' }, `Der Antrag ist an ${agentName(w, r.an)} gerichtet. Du kannst ihn trotzdem selbst entscheiden.`) : null,
      fehler,
    ];
  };

  const aktualisieren = (): void => {
    if (!stand.agents) { ersetzen(koerper, leerzustand('Noch nichts geladen', 'Der Antrag erscheint, sobald Mac oder peer antwortet.')); return; }
    const w = welt();
    const r = w?.rechteantraege.find((x) => x.id === antragId);
    if (!w || !r) {
      ersetzen(koerper, leerzustand('Antrag nicht gefunden', 'Er wurde vielleicht schon entschieden oder zurückgezogen. Offene Anträge stehen unter Agents.'));
      return;
    }
    ersetzen(koerper, ...antragInhalt(w, r), ...entscheidungsTeil(w, r));
  };
  aktualisieren();
  return { el: s.el, aktualisieren };
}

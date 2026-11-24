/**
 * DER AGENTS-STAND FUER DIE ANZEIGE.
 *
 * `AgentsData` ist im Vertrag bewusst ein offenes `JsonObject`: der Kern
 * liefert die Weltenform aus `main/welten.ts` (`WeltenNutzlast`). Die PWA
 * liest davon nur die Felder, die sie zeigt, und prueft jedes einzeln. Was
 * fehlt oder falsch getypt ist, faellt weg statt eine Ansicht zu sprengen.
 * Die Feldnamen bleiben die des Kerns, damit Paket 7 nichts uebersetzen muss.
 */
import type { AgentsData, JsonObject, JsonValue } from '../../mobile-shared/vertrag';

export interface AgentNachricht {
  id: string;
  von: string;
  text: string;
  zeit: number;
  von_mensch: boolean;
}

export interface AgentKurz {
  id: string;
  name: string;
  stufe: string;
  team: string | null;
  spezialgebiet: string;
  figur: { art: string; rolle: string };
  modell: string;
  maschine: string;
  zustand: string;
  zustand_text: string;
  figur_zustand: string;
  ticket: string | null;
  postfach_offen: number;
  einzelchat: AgentNachricht[];
}

export interface TicketKurz {
  id: string;
  titel: string;
  ziel: string;
  stand: string;
  kind: string;
  prioritaet: number;
  prioritaet_text: string;
  ampel: string;
  bearbeiter: string | null;
  /** Einzelne Agenten, dazu ein adressiertes Team als `team:<name>` wie im Adressfeld des Kerns. */
  adressaten: string[];
  geaendert: number;
  fertig_punkte: { text: string; erledigt: boolean }[];
  verlauf: { zeit: number; ereignis: string; von: string; text: string }[];
  ergebnis: { text: string; von: string; zeit: number } | null;
  /**
   * Wer das Ticket abgenommen hat, wann, mit welchem Grund und welcher
   * Bemerkung (`welten.ts`, Feld `abnahme` aus dem `approval` des Kerns). Der
   * Grund fehlt bei einer Rueckgabe: der Kern schreibt `reason_code` nur, wenn
   * tatsaechlich abgenommen wurde.
   */
  abnahme: { von: string; zeit: number; grund: string | null; bemerkung: string | null } | null;
}

export interface FrageKurz {
  id: string;
  text: string;
  optionen: string[];
  empfehlung: string | null;
  ticket: string | null;
  stand: string;
  von: string;
  gestellt: number;
}

/** Rechte, wie sie in einem Rechteantrag stehen (`RohRechteAenderung` in `main/welten.ts`). Ein fehlendes Feld bleibt unveraendert. */
export interface RechteKurz {
  tools?: string[];
  bash?: string[];
  skills?: string[];
  web?: boolean;
}

/**
 * Ein Rechteantrag (`WeltRechteAntrag` in `main/welten.ts`, `kind: rechte-antrag`).
 * `an` ist `mensch` oder der Hauptagent; `abgelaufen` heisst offen, aber nicht
 * mehr entscheidbar. Entscheiden geht am Mac und, wo der Kern es anbietet,
 * auch hier (`rights_request.decide`, `rechteantragEntscheidbar`).
 */
export interface RechteAntragKurz {
  id: string;
  von: string;
  an: string;
  agent: string;
  changes: RechteKurz;
  /**
   * `changes` genau so, wie `GET /agents` es geliefert hat. Die Entscheidung
   * schickt dieses Objekt unveraendert zurueck; der Server vergleicht es mit
   * der gespeicherten Aenderung. Null, wenn der Kern kein Objekt liefert.
   */
  changes_roh: JsonObject | null;
  vorher: RechteKurz;
  grund: string;
  stand: string;
  abgelaufen: boolean;
  laeuft_ab: number;
  zeit: number;
  entscheidung: string | null;
  bemerkung: string | null;
}

/**
 * Ein Team der Welt (`WeltTeam` in `main/welten.ts`): Leiter ist der erste
 * Teamleiter, Mitglieder die uebrigen. Der Kern adressiert mit `team:<name>`
 * Leiter und Mitglieder zusammen (`adressenAufloesen`).
 */
export interface TeamKurz {
  name: string;
  leiter: string | null;
  mitglieder: string[];
}

export interface WeltKurz {
  id: string;
  name: string;
  maschine: string;
  fern: boolean;
  /** Ob der Kern Rechteantraege dieser Welt vom Handy entscheiden laesst (`WeltRechteAntrag`, Kernfeld). */
  rechteantrag_entscheidbar: boolean;
  verbindung: { ok: boolean; seit: number | null; text: string };
  hauptagent: string | null;
  agenten: AgentKurz[];
  teams: TeamKurz[];
  tickets: TicketKurz[];
  fragen: FrageKurz[];
  rechteantraege: RechteAntragKurz[];
  kanal: AgentNachricht[];
  zaehler: { brauchen_dich: number; laufen: number; tickets_offen: number };
  /** Definition of Done der Welt; der Kern verlangt sie vor jeder Abnahme bestaetigt. */
  dod: string[];
}

export interface AgentsAnsicht {
  welten: WeltKurz[];
  /** Ob Agents-Aktionen schon als Mensch gelten; sonst handelt die App als `cli-operator`. */
  mensch_absender: boolean;
}

type Obj = { readonly [k: string]: JsonValue };

function obj(v: JsonValue | undefined): Obj | null {
  return v !== null && typeof v === 'object' && !Array.isArray(v) ? (v as Obj) : null;
}
function str(v: JsonValue | undefined, sonst = ''): string {
  return typeof v === 'string' ? v : sonst;
}
function strOderNull(v: JsonValue | undefined): string | null {
  return typeof v === 'string' ? v : null;
}
function zahl(v: JsonValue | undefined, sonst = 0): number {
  return typeof v === 'number' && Number.isFinite(v) ? v : sonst;
}
function liste(v: JsonValue | undefined): JsonValue[] {
  return Array.isArray(v) ? v : [];
}
function strListe(v: JsonValue | undefined): string[] {
  return liste(v).filter((x): x is string => typeof x === 'string');
}
/** Zeiten kommen aus dem Kern als ISO-Text; Zahlen werden als Millisekunden genommen. */
function zeit(v: JsonValue | undefined): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'string') {
    const t = Date.parse(v);
    return Number.isFinite(t) ? t : 0;
  }
  return 0;
}

function nachricht(v: JsonValue): AgentNachricht | null {
  const o = obj(v);
  if (!o) return null;
  // Einzelchat-Eintraege tragen die Nachricht verschachtelt (`ChatEintrag`).
  const n = obj(o.nachricht) ?? o;
  const id = str(n.id);
  if (!id) return null;
  return { id, von: str(n.von), text: str(n.text), zeit: zeit(n.zeit), von_mensch: n.von_mensch === true };
}

function agent(v: JsonValue): AgentKurz | null {
  const o = obj(v);
  if (!o || !str(o.id)) return null;
  const f = obj(o.figur);
  return {
    id: str(o.id),
    name: str(o.name, str(o.id)),
    stufe: str(o.stufe, 'mitglied'),
    team: strOderNull(o.team),
    spezialgebiet: str(o.spezialgebiet),
    figur: { art: str(f?.art, 'roboter'), rolle: str(f?.rolle, str(o.id)) },
    modell: str(o.modell),
    maschine: str(o.maschine),
    zustand: str(o.zustand),
    zustand_text: str(o.zustand_text),
    figur_zustand: str(o.figur_zustand),
    ticket: strOderNull(o.ticket),
    postfach_offen: zahl(o.postfach_offen),
    einzelchat: liste(o.einzelchat).map(nachricht).filter((n): n is AgentNachricht => n !== null),
  };
}

function ticket(v: JsonValue): TicketKurz | null {
  const o = obj(v);
  if (!o || !str(o.id)) return null;
  const erg = obj(o.ergebnis);
  const abn = obj(o.abnahme);
  return {
    id: str(o.id),
    titel: str(o.titel),
    ziel: str(o.ziel),
    stand: str(o.stand),
    kind: str(o.kind),
    prioritaet: zahl(o.prioritaet, 2),
    prioritaet_text: str(o.prioritaet_text),
    ampel: str(o.ampel),
    bearbeiter: strOderNull(o.bearbeiter),
    adressaten: [...strListe(o.adressaten), ...(str(o.team) ? [`team:${str(o.team)}`] : [])],
    geaendert: zeit(o.geaendert),
    fertig_punkte: liste(o.fertig_punkte).map(obj).filter((p): p is Obj => p !== null)
      .map((p) => ({ text: str(p.text), erledigt: p.erledigt === true })),
    verlauf: liste(o.verlauf).map(obj).filter((p): p is Obj => p !== null)
      .map((p) => ({ zeit: zeit(p.zeit), ereignis: str(p.ereignis), von: str(p.von), text: str(p.text) })),
    ergebnis: erg ? { text: str(erg.text), von: str(erg.von), zeit: zeit(erg.zeit) } : null,
    abnahme: abn ? { von: str(abn.von), zeit: zeit(abn.zeit), grund: strOderNull(abn.grund), bemerkung: strOderNull(abn.bemerkung) } : null,
  };
}

function frage(v: JsonValue): FrageKurz | null {
  const o = obj(v);
  if (!o || !str(o.id)) return null;
  return {
    id: str(o.id),
    text: str(o.text),
    optionen: strListe(o.optionen),
    empfehlung: strOderNull(o.empfehlung),
    ticket: strOderNull(o.ticket),
    stand: str(o.stand),
    von: str(o.von),
    gestellt: zeit(o.gestellt),
  };
}

function rechte(v: JsonValue | undefined): RechteKurz {
  const o = obj(v);
  const r: RechteKurz = {};
  if (!o) return r;
  for (const k of ['tools', 'bash', 'skills'] as const) if (Array.isArray(o[k])) r[k] = strListe(o[k]);
  if (typeof o.web === 'boolean') r.web = o.web;
  return r;
}

function rechteAntrag(v: JsonValue): RechteAntragKurz | null {
  const o = obj(v);
  if (!o || !str(o.id)) return null;
  return {
    id: str(o.id),
    von: str(o.von),
    an: str(o.an),
    agent: str(o.agent, str(o.von)),
    changes: rechte(o.changes),
    changes_roh: obj(o.changes) as JsonObject | null,
    vorher: rechte(o.vorher),
    grund: str(o.grund),
    stand: str(o.stand),
    abgelaufen: o.abgelaufen === true,
    laeuft_ab: zeit(o.laeuft_ab),
    zeit: zeit(o.zeit),
    entscheidung: strOderNull(o.entscheidung),
    bemerkung: strOderNull(o.bemerkung),
  };
}

function team(v: JsonValue): TeamKurz | null {
  const o = obj(v);
  if (!o || !str(o.name)) return null;
  return { name: str(o.name), leiter: strOderNull(o.leiter), mitglieder: strListe(o.mitglieder) };
}

/**
 * Die Teams aus dem Feld `team` der Agenten, nach `teamsBilden` in
 * `main/welten.ts`: ohne Hauptagent, nach Name, Leiter ist der erste
 * Teamleiter. Gilt nur, wenn der Stand selbst keine `teams` liefert.
 */
export function teamsAusAgenten(agenten: AgentKurz[]): TeamKurz[] {
  const im = agenten.filter((a) => a.stufe !== 'hauptagent' && a.team);
  const nachName = (a: AgentKurz, b: AgentKurz): number => a.name.localeCompare(b.name, 'de');
  return [...new Set(im.map((a) => a.team as string))].sort((a, b) => a.localeCompare(b, 'de')).map((name) => {
    const alle = im.filter((a) => a.team === name);
    const leiter = alle.filter((a) => a.stufe === 'teamleiter').sort(nachName)[0] ?? null;
    return { name, leiter: leiter?.id ?? null, mitglieder: alle.filter((a) => a !== leiter).sort(nachName).map((a) => a.id) };
  });
}

/** Wen `team:<name>` erreicht: Leiter und Mitglieder, ohne Doppelte. */
export function teamEmpfaenger(t: TeamKurz): string[] {
  return [...new Set([...(t.leiter ? [t.leiter] : []), ...t.mitglieder])];
}

function welt(v: JsonValue): WeltKurz | null {
  const o = obj(v);
  if (!o || !str(o.id)) return null;
  const vb = obj(o.verbindung);
  const z = obj(o.zaehler);
  const agenten = liste(o.agenten).map(agent).filter((a): a is AgentKurz => a !== null);
  return {
    id: str(o.id),
    name: str(o.name, str(o.id)),
    maschine: str(o.maschine),
    fern: o.fern === true,
    rechteantrag_entscheidbar: o.rechteantrag_entscheidbar === true,
    verbindung: { ok: vb?.ok !== false, seit: vb?.seit ? zeit(vb.seit) : null, text: str(vb?.text) },
    hauptagent: strOderNull(o.hauptagent),
    agenten,
    teams: Array.isArray(o.teams) ? o.teams.map(team).filter((t): t is TeamKurz => t !== null) : teamsAusAgenten(agenten),
    tickets: liste(o.tickets).map(ticket).filter((t): t is TicketKurz => t !== null),
    fragen: liste(o.fragen).map(frage).filter((f): f is FrageKurz => f !== null),
    rechteantraege: liste(o.rechteantraege).map(rechteAntrag).filter((r): r is RechteAntragKurz => r !== null),
    kanal: liste(o.kanal).map(nachricht).filter((n): n is AgentNachricht => n !== null),
    dod: strListe(o.dod),
    zaehler: {
      brauchen_dich: zahl(z?.brauchen_dich),
      laufen: zahl(z?.laufen),
      tickets_offen: zahl(z?.tickets_offen),
    },
  };
}

export function agentsLesen(data: AgentsData): AgentsAnsicht {
  const nutzlast = obj(data.welten) ?? data;
  return {
    welten: liste(nutzlast.welten).map(welt).filter((w): w is WeltKurz => w !== null),
    mensch_absender: data.mensch_absender === true,
  };
}

/** Offene Fragen einer Welt, die auf den Menschen warten. */
export function offeneFragen(w: WeltKurz): FrageKurz[] {
  return w.fragen.filter((f) => f.stand === 'offen');
}

/** Rechteantraege einer Welt, die auf den Menschen warten: an ihn gerichtet, offen und noch entscheidbar. */
export function offeneRechteantraege(w: WeltKurz): RechteAntragKurz[] {
  return w.rechteantraege.filter((r) => r.an === 'mensch' && r.stand === 'offen' && !r.abgelaufen);
}

/**
 * Ob dieser Antrag vom Handy entschieden werden kann: offen, nicht abgelaufen,
 * eine Welt dieser Maschine, und der Kern bietet es an. Die einzige Sperre ist
 * das Kernfeld `rechteantrag_entscheidbar`; die App fuehrt keine eigene.
 */
export function rechteantragEntscheidbar(w: WeltKurz, r: RechteAntragKurz): boolean {
  return r.stand === 'offen' && !r.abgelaufen && !w.fern && w.rechteantrag_entscheidbar && r.changes_roh !== null;
}

/** Die Rechtearten eines Antrags in Worten der Werkbank. */
export const RECHTE_NAMEN: Readonly<Record<keyof RechteKurz, string>> = {
  tools: 'Werkzeuge', bash: 'Bash-Muster', skills: 'Skills', web: 'Web',
};

/** Welche Rechtearten ein Antrag aendert, kurz fuer eine Kachel: „Bash-Muster, Web“. */
export function rechteKurztext(r: RechteKurz): string {
  const arten = (Object.keys(RECHTE_NAMEN) as (keyof RechteKurz)[]).filter((k) => r[k] !== undefined);
  return arten.length ? arten.map((k) => RECHTE_NAMEN[k]).join(', ') : 'keine Angaben';
}

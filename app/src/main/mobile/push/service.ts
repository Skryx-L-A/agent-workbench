import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { request as httpRequest } from 'node:http';
import { request as httpsRequest } from 'node:https';
import type { MobileRouteHandlers } from '../http/server';
import { MobileAuthError } from '../auth/auth';
import type { MobileEventBroker } from '../events/ereignisse';
import type { JsonObject, NotificationEventType } from '../../../mobile-shared/vertrag';
import { PushJournal, dedupeKey, notificationText, notificationLink, sourceFor } from './journal';
import type { WeltOffenRecord } from './journal';
import { weltAnliegenLink, weltAnliegenText } from './welt';
import type { WeltAnliegenStand } from './welt';
import { workerFertigEinstufen } from './dringend';
import type { WorkerFertigFelder, WorkerFertigGrund } from './dringend';
import { encryptWebPushPayload, loadVapidKeyMaterial, vapidAuthorization, vapidPublicKey } from './webpush';
import type { VapidKeyMaterial, WebPushSubscription } from './webpush';

/**
 * Entscheidung decision 2026-10-09: nur Dringendes wird als Push verschickt
 * (Freigabe-Anfrage, Fehler, Frage an ihn). `worker.done` steht nur im Journal,
 * also in der App (Tab „Meldungen"), mit Zustellung `in_app` -- ausser die feste
 * Regel in dringend.ts stuft es hoch (Wartewunsch, Frage an alice, Spur steht).
 */
export const PUSH_TYPES: ReadonlySet<NotificationEventType> = new Set<NotificationEventType>([
  'command.blocked', 'worker.stalled', 'session.lost', 'orchestrator.waiting', 'welt.braucht-dich',
]);

/** Wie oft ein Durchgang eine fehlgeschlagene Zustellung von „Welt braucht dich“ wiederholt (je Antrag bzw. Frage). */
export const WELT_VERSUCHE = 3;

/**
 * Nur Frisches wird gemeldet: ein Antrag oder eine Frage, die beim ersten Sehen schon laenger als 24 Stunden offen ist
 * (der erste Lauf nach dem Ausrollen, eine App, die tagelang aus war), steht laengst in der Ansicht „Agents“ und wuerde
 * als Schwall alter Meldungen kommen. Sie wird still vermerkt (ohne Journaleintrag) und erst bei der Erledigung vergessen.
 */
export const WELT_FRISCH_MS = 24 * 60 * 60 * 1000;

/** Was ein Durchgang von `weltAnliegen` getan hat: neu gemeldet, als erledigt vermerkt. */
export interface WeltAnliegenErgebnis { readonly gemeldet: number; readonly gesendet: number; readonly erledigt: number; }

export interface PushTrigger {
  /** Persistente Event-ID aus dem vorhandenen Ereignisstrom, nicht aus Chatinhalt erzeugt. */
  readonly event_id: string;
  readonly type: NotificationEventType;
  readonly authoritative_machine: string;
  readonly target_id: string;
  readonly state_fingerprint: string;
  readonly created_at?: number;
  /** Nur bei `worker.done`: die gezaehlten Felder fuer dringend.ts. Fehlen sie, zaehlt nur der Wartewunsch. */
  readonly worker?: WorkerFertigFelder;
  /**
   * Der Kurztext des Kerns statt des neutralen (26.10.2026, task): bei
   * `command.blocked` Rolle und Name („Worker wz-x wartet auf eine Freigabe.“).
   * `notificationText` laesst keinen Pfad durch (MOBILE-APP-SPEC.md:196).
   */
  readonly text?: string;
}

/** Wartewunsch „Bescheid geben“: bis zum ersten ausgeloesten Push, hoechstens 12 Stunden (Entscheidung decision 2026-10-09). */
export const WARTEWUNSCH_MS = 12 * 60 * 60 * 1000;

export interface PushDelivery {
  readonly event_id: string;
  readonly sent: number;
  readonly suppressed: boolean;
}

export interface MobilePushServiceOptions {
  readonly machine: string;
  readonly stateDirectory: string;
  /**
   * Vom Betreiber gesetzter 0600-Laufzeitpfad, nie ein Repositorypfad. Leer
   * (Paket 10): kein Web Push, das Meldungsjournal laeuft trotzdem -- die
   * Meldungen erscheinen in der App, nur nicht auf dem Sperrbildschirm.
   */
  readonly vapidKeyPath: string;
  /**
   * Ob ein Geraet Pushes bekommen darf (pushwelt-Review F1, 18.10.2026): im Kern
   * `MobileAuthenticator.geraetZulaessig` -- gekoppelt, nicht widerrufen, gueltig signiert.
   * Pflicht: ein Abo eines widerrufenen oder eingeschleusten Geraets bekommt keinen Push.
   */
  readonly geraetZulaessig: (deviceId: string) => boolean;
  readonly dedupeTtlMs?: number;
  readonly journalMaximum?: number;
  /** Nur die isolierte lokale Push-Attrappe darf HTTP verwenden. */
  readonly allowInsecureEndpointForTests?: boolean;
  readonly now?: () => number;
  /** Der vorhandene Paket-4-Ring informiert beide PWA-Origin-Clients ueber Journalaenderungen. */
  readonly events?: Pick<MobileEventBroker, 'publish'>;
}

interface HttpResult { readonly status: number; }
type PushTransport = (endpoint: URL, headers: Readonly<Record<string, string>>, body: Buffer) => Promise<HttpResult>;

function validateSubscription(value: unknown): WebPushSubscription {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('push subscription is malformed');
  const record = value as Record<string, unknown>;
  const keys = record.keys;
  if (typeof record.endpoint !== 'string' || typeof keys !== 'object' || keys === null || Array.isArray(keys)
    || typeof (keys as Record<string, unknown>).p256dh !== 'string' || typeof (keys as Record<string, unknown>).auth !== 'string') {
    throw new Error('push subscription is malformed');
  }
  return { endpoint: record.endpoint, keys: { p256dh: (keys as Record<string, string>).p256dh, auth: (keys as Record<string, string>).auth } };
}

function defaultTransport(endpoint: URL, headers: Readonly<Record<string, string>>, body: Buffer): Promise<HttpResult> {
  return new Promise((resolve, reject) => {
    const send = endpoint.protocol === 'https:' ? httpsRequest : httpRequest;
    const request = send(endpoint, { method: 'POST', headers }, (response) => {
      response.resume();
      response.once('end', () => resolve({ status: response.statusCode ?? 0 }));
    });
    request.once('error', reject);
    request.end(body);
  });
}

/**
 * Die Abbildung in den Kern wird im Integrationspaket an bestehende Waechter
 * angeschlossen. Diese kleine Schnittstelle verhindert einen zweiten Beobachter
 * und verlangt fuer jedes Ereignis die bereits bekannte Autoritaet und ID.
 */
export interface MobilePushTriggers {
  orchestratorWaiting(trigger: Omit<PushTrigger, 'type'>): Promise<PushDelivery>;
  workerDone(trigger: Omit<PushTrigger, 'type'>): Promise<PushDelivery>;
  commandBlocked(trigger: Omit<PushTrigger, 'type'>): Promise<PushDelivery>;
  sessionLost(trigger: Omit<PushTrigger, 'type'>): Promise<PushDelivery>;
  workerStalled(trigger: Omit<PushTrigger, 'type'>): Promise<PushDelivery>;
}

export class MobilePushService {
  readonly journal: PushJournal;
  private readonly material: VapidKeyMaterial | null;
  private readonly ttl: number;
  private readonly now: () => number;
  private weltKette: Promise<void> = Promise.resolve();

  constructor(private readonly options: MobilePushServiceOptions, private readonly transport: PushTransport = defaultTransport) {
    this.journal = new PushJournal(options.stateDirectory, options.journalMaximum);
    this.material = options.vapidKeyPath ? loadVapidKeyMaterial(options.vapidKeyPath) : null;
    this.ttl = options.dedupeTtlMs ?? 24 * 60 * 60 * 1000;
    this.now = options.now ?? Date.now;
    if (!Number.isSafeInteger(this.ttl) || this.ttl < 1) throw new Error('dedupe TTL must be positive');
  }

  /** Der oeffentliche VAPID-Schluessel fuer `GET /bootstrap`; null ohne Schluesseldatei. */
  publicKey(): string | null {
    return this.material ? vapidPublicKey(this.material) : null;
  }

  handlers(): Pick<MobileRouteHandlers, 'PUT /push/subscription' | 'DELETE /push/subscription/:id' | 'GET /notifications'> {
    return {
      'PUT /push/subscription': (request) => {
        if (!this.material) throw new MobileAuthError('capability_unavailable', 'web push has no VAPID key on this host');
        const subscription = validateSubscription((request.body as { subscription?: unknown } | undefined)?.subscription);
        this.assertEndpoint(subscription.endpoint);
        const saved = this.journal.saveSubscription(request.device?.device_id ?? '', subscription, this.now());
        return { data: { subscription_id: saved.id } as JsonObject };
      },
      'DELETE /push/subscription/:id': (request) => {
        this.journal.removeSubscription(request.device?.device_id ?? '', request.params.id);
        return { data: undefined, status: 204 };
      },
      'GET /notifications': () => ({ data: { notifications: this.journal.list(), cursor: null } as unknown as JsonObject }),
    };
  }

  triggers(): MobilePushTriggers {
    return {
      orchestratorWaiting: (trigger) => this.publish({ ...trigger, type: 'orchestrator.waiting' }),
      workerDone: (trigger) => this.publish({ ...trigger, type: 'worker.done' }),
      commandBlocked: (trigger) => this.publish({ ...trigger, type: 'command.blocked' }),
      sessionLost: (trigger) => this.publish({ ...trigger, type: 'session.lost' }),
      workerStalled: (trigger) => this.publish({ ...trigger, type: 'worker.stalled' }),
    };
  }

  /**
   * „WELT BRAUCHT DICH“ (Entscheidung decision 2026-10-14): ein Rechteantrag oder eine Frage einer Welt wartet auf
   * den Menschen. Aufgerufen mit jedem geaenderten Stand der Welten (`weltAnliegenAus`); der Durchgang ist die
   * eine Stelle, die entscheidet:
   *   * NEU offen und noch nicht gemerkt -> genau EINE Meldung (Journal + Web Push), der Merkeintrag steht VOR dem
   *     Senden: ein Absturz mittendrin verliert hoechstens den Push, nie verdoppelt er ihn;
   *   * schon gemerkt -> nichts, auch nach einem Neustart (der Merkeintrag steht in push-state.json);
   *   * nicht mehr offen UND die Welt ist sicher gelesen -> der Journaleintrag bekommt `erledigt_at`, die App erfaehrt
   *     es ueber `notification.changed`. Einen Web Push zum Zuruecknehmen gibt es nicht (iOS verlangt zu jedem Push
   *     eine sichtbare Meldung); die App schliesst die Meldung mit dem Tag der Event-ID selbst.
   * Nutzlast: nur Welt-Name, Art, Kennung zum Oeffnen -- nie Text, Rechte oder Grund (siehe welt.ts).
   * Aufrufe laufen nacheinander (eine Kette), ein zweiter Durchgang sieht den Merkeintrag des ersten.
   */
  weltAnliegen(stand: WeltAnliegenStand): Promise<WeltAnliegenErgebnis> {
    const lauf = this.weltKette.then(() => this.weltAnliegenDurchgang(stand));
    this.weltKette = lauf.then(() => undefined, () => undefined);
    return lauf;
  }

  private async weltAnliegenDurchgang(stand: WeltAnliegenStand): Promise<WeltAnliegenErgebnis> {
    const jetzt = this.now();
    const bekannt = this.journal.weltOffen();
    const offen = new Set(stand.offen.map((a) => a.schluessel));
    let gemeldet = 0;
    let gesendet = 0;
    let erledigt = 0;
    // Erledigt zuerst: so steht ein Antrag, der kurz hintereinander erscheint und verschwindet, in der richtigen Reihenfolge.
    for (const [schluessel, eintrag] of Object.entries(bekannt)) {
      if (offen.has(schluessel) || !stand.gelesen.has(eintrag.welt_id)) continue;
      const aktualisiert = this.journal.erledigtVermerken(eintrag.event_id, jetzt);
      this.journal.weltOffenSetzen(schluessel, null);
      if (aktualisiert) {
        this.options.events?.publish('notification.changed', [aktualisiert.id], aktualisiert.source);
        erledigt += 1;
      }
    }
    // F4 (pushwelt-Review 18.10.2026): Fehlt eine Welt in einem vollstaendigen, erfolgreichen Durchgang
    // (alle lokalen Welten gelesen, kein Lesefehler) und gibt es ihre abgemerkte Ablage nicht mehr, ist
    // die Welt geloescht: ihre Merkeintraege werden erledigt und entfernt. Eine Welt, die der Durchgang
    // noch sieht -- auch als fern -- wird nie angefasst; ebensowenig Eintraege ohne abgemerkten Pfad
    // (aeltere Bestaende) und solche, deren Ablage noch liegt (die Welt kann wieder auftauchen).
    if (stand.vollstaendig) {
      for (const [schluessel, eintrag] of Object.entries(bekannt)) {
        if (offen.has(schluessel) || stand.gelesen.has(eintrag.welt_id)) continue;
        if (stand.welten.has(eintrag.welt_id) || eintrag.ablage === undefined || existsSync(eintrag.ablage)) continue;
        const aktualisiert = this.journal.erledigtVermerken(eintrag.event_id, jetzt);
        this.journal.weltOffenSetzen(schluessel, null);
        if (aktualisiert) {
          this.options.events?.publish('notification.changed', [aktualisiert.id], aktualisiert.source);
          erledigt += 1;
        }
      }
    }
    for (const anliegen of stand.offen) {
      const alt = bekannt[anliegen.schluessel];
      const eventId = alt?.event_id ?? weltEventId(this.options.machine, anliegen.schluessel);
      // F3 (pushwelt-Review 18.10.2026): Taucht eine erledigte Kennung wieder offen auf, nimmt der Dienst
      // den Vermerk zurueck -- ohne zweiten Push; die erneute Erledigung vermerkt der Durchgang spaeter
      // wieder. Bei einer erneuten Zustellung ersetzt der neue Journaleintrag den alten ohnehin.
      const zurueck = this.journal.erledigtZuruecknehmen(eventId);
      if (zurueck) this.options.events?.publish('notification.changed', [zurueck.id], zurueck.source);
      if (alt && (alt.fertig || alt.versuche >= WELT_VERSUCHE)) {
        // Aeltere Merkeintraege kennen den Pfad der Ablage noch nicht (vor dem 18.10.2026): einmal
        // nachtragen, damit F4 eine spaeter geloeschte Welt erkennt; sonst wird hier nicht geschrieben.
        if (alt.ablage === undefined && anliegen.ablage !== undefined) {
          this.journal.weltOffenSetzen(anliegen.schluessel, { ...alt, ablage: anliegen.ablage });
        }
        continue;
      }
      const eintrag: WeltOffenRecord = alt ?? { welt_id: anliegen.welt_id, art: anliegen.art, anliegen_id: anliegen.anliegen_id, event_id: eventId, seit: jetzt, versuche: 0, fertig: false, ablage: anliegen.ablage };
      if (!alt && (anliegen.gestellt === null || jetzt - anliegen.gestellt > WELT_FRISCH_MS)) {
        this.journal.weltOffenSetzen(anliegen.schluessel, { ...eintrag, fertig: true });
        continue;
      }
      // Ein Journaleintrag mit dieser Kennung (nicht fehlgeschlagen) heisst: gemeldet. So verdoppelt auch ein verlorener Merkeintrag nichts.
      if (!alt && this.journal.hasEvent(eventId)) {
        this.journal.weltOffenSetzen(anliegen.schluessel, { ...eintrag, fertig: true });
        continue;
      }
      this.journal.weltOffenSetzen(anliegen.schluessel, { ...eintrag, versuche: eintrag.versuche + 1 });
      const text = weltAnliegenText(anliegen);
      const link = weltAnliegenLink(anliegen);
      const welt = { welt_id: anliegen.welt_id, welt_name: anliegen.welt_name, art: anliegen.art, anliegen_id: anliegen.anliegen_id };
      const grundlage = { id: eventId, type: 'welt.braucht-dich' as const, source: 'authoritative' as const, machine: this.options.machine,
        target_id: anliegen.anliegen_id, created_at: jetzt, text, link, welt };
      gemeldet += 1;
      const material = this.material;
      if (!material) {
        // Ohne VAPID-Schluessel gibt es keinen Pushdienst: die Meldung steht im Journal, ausdruecklich als nicht zugestellt.
        this.recordNotification({ ...grundlage, delivery: 'suppressed' });
        this.journal.weltOffenSetzen(anliegen.schluessel, { ...eintrag, versuche: eintrag.versuche + 1, fertig: true });
        continue;
      }
      const payload = Buffer.from(JSON.stringify({ event_id: eventId, type: grundlage.type, machine: grundlage.machine,
        target_id: grundlage.target_id, created_at: jetzt, text, link, welt_id: anliegen.welt_id, art: anliegen.art }), 'utf8');
      const ergebnis = await this.zustellen(material, payload, jetzt);
      const geklappt = ergebnis.sent > 0;
      this.recordNotification({ ...grundlage, delivery: ergebnis.failed ? 'failed' : 'sent' });
      this.journal.weltOffenSetzen(anliegen.schluessel, { ...eintrag, versuche: eintrag.versuche + 1, fertig: geklappt || !ergebnis.failed });
      if (geklappt) gesendet += 1;
    }
    return { gemeldet, gesendet, erledigt };
  }

  async publish(trigger: PushTrigger): Promise<PushDelivery> {
    const createdAt = trigger.created_at ?? this.now();
    const source = sourceFor(trigger.authoritative_machine, this.options.machine);
    const key = dedupeKey(trigger.type, trigger.authoritative_machine, trigger.target_id, trigger.state_fingerprint);
    if (this.journal.hasEvent(trigger.event_id)) return { event_id: trigger.event_id, sent: 0, suppressed: true };
    // Spiegel werden protokolliert, aber erzeugen nie eine sichtbare Pushmeldung.
    if (source === 'mirror') {
      this.recordNotification({ id: trigger.event_id, type: trigger.type, source, machine: trigger.authoritative_machine,
        target_id: trigger.target_id, created_at: createdAt, text: notificationText(trigger.type, trigger.text),
        link: notificationLink(trigger.type, trigger.target_id, trigger.event_id), delivery: 'suppressed' });
      return { event_id: trigger.event_id, sent: 0, suppressed: true };
    }
    if (!this.journal.reserveDedupe(key, createdAt + this.ttl, createdAt)) return { event_id: trigger.event_id, sent: 0, suppressed: true };
    // Die eine Entscheidungsstelle fuer `worker.done` (dringend.ts); die vier
    // dringenden Arten fragt sie nie.
    const stufe = trigger.type === 'worker.done'
      ? workerFertigEinstufen(this.journal.wartewunschBis(trigger.target_id, createdAt) !== null, trigger.worker ?? null)
      : null;
    const grund: { grund?: WorkerFertigGrund } = stufe ? { grund: stufe.grund } : {};
    if (!PUSH_TYPES.has(trigger.type) && stufe?.zustellung !== 'push') {
      this.recordNotification({ id: trigger.event_id, type: trigger.type, source, machine: trigger.authoritative_machine,
        target_id: trigger.target_id, created_at: createdAt, text: notificationText(trigger.type, trigger.text),
        link: notificationLink(trigger.type, trigger.target_id, trigger.event_id), delivery: 'in_app', ...grund });
      return { event_id: trigger.event_id, sent: 0, suppressed: true };
    }
    const material = this.material;
    if (!material) {
      // Ohne VAPID-Schluessel gibt es keinen Pushdienst: die Meldung steht im
      // Journal (Tab „Meldungen"), ausdruecklich als nicht zugestellt.
      this.journal.releaseDedupe(key);
      this.recordNotification({ id: trigger.event_id, type: trigger.type, source, machine: trigger.authoritative_machine,
        target_id: trigger.target_id, created_at: createdAt, text: notificationText(trigger.type, trigger.text),
        link: notificationLink(trigger.type, trigger.target_id, trigger.event_id), delivery: 'suppressed', ...grund });
      return { event_id: trigger.event_id, sent: 0, suppressed: true };
    }
    const payload = Buffer.from(JSON.stringify({ event_id: trigger.event_id, type: trigger.type, machine: trigger.authoritative_machine,
      target_id: trigger.target_id, created_at: createdAt, text: notificationText(trigger.type, trigger.text),
      link: notificationLink(trigger.type, trigger.target_id, trigger.event_id) }), 'utf8');
    const { sent, failed } = await this.zustellen(material, payload, createdAt);
    // Der Ledger-Eintrag gilt nur nach mindestens einer erfolgreichen
    // Uebergabe an den Pushdienst; ein Ausfall darf die erneute Zustellung
    // beim naechsten beobachteten Zustand nicht dauerhaft sperren.
    if (sent === 0) this.journal.releaseDedupe(key);
    // Der Wartewunsch gilt bis zum ersten ausgeloesten Push dieser Sitzung; ein
    // Ausfall des Pushdienstes verbraucht ihn nicht.
    if (stufe && sent > 0) this.journal.wartewunschSetzen(trigger.target_id, null, createdAt);
    this.recordNotification({ id: trigger.event_id, type: trigger.type, source, machine: trigger.authoritative_machine,
      target_id: trigger.target_id, created_at: createdAt, text: notificationText(trigger.type, trigger.text),
      link: notificationLink(trigger.type, trigger.target_id, trigger.event_id), delivery: failed ? 'failed' : 'sent', ...grund });
    return { event_id: trigger.event_id, sent, suppressed: false };
  }

  /**
   * Wartewunsch „Bescheid geben“ setzen oder zuruecknehmen. Zurueck kommt, bis
   * wann er gilt (null = keiner). Kein Geheimnis, kein Pfad: nur die globale
   * Sitzungskennung und ein Zeitpunkt.
   */
  wartewunschSetzen(sessionId: string, an: boolean): number | null {
    const jetzt = this.now();
    const bis = an ? jetzt + WARTEWUNSCH_MS : null;
    this.journal.wartewunschSetzen(sessionId, bis, jetzt);
    return bis;
  }

  wartewunschBis(sessionId: string): number | null {
    return this.journal.wartewunschBis(sessionId, this.now());
  }

  /** Alle geltenden Wartewuensche (Sitzung -> bis), fuer den Takt des Kerns mit einem Lesezugriff. */
  wartewuensche(): Readonly<Record<string, number>> {
    return this.journal.wartewuensche(this.now());
  }

  /** Beim Widerruf eines Geraets: seine Abos entfernen (F1). */
  geraetEntfernen(deviceId: string): number {
    return this.journal.removeDeviceSubscriptions(deviceId);
  }

  clearState(type: NotificationEventType, authoritativeMachine: string, targetId: string, stateFingerprint: string): void {
    this.journal.clearState(type, authoritativeMachine, targetId, stateFingerprint);
  }

  /** Alle Abos der Reihe nach bedienen; ein Ausfall eines Abos haelt die anderen nicht auf. */
  private async zustellen(material: VapidKeyMaterial, payload: Buffer, createdAt: number): Promise<{ sent: number; failed: boolean }> {
    let sent = 0;
    let failed = false;
    for (const subscription of this.journal.subscriptions()) {
      // F1: nur Abos gueltiger Geraete; die Pruefung scheitert geschlossen.
      let zulaessig = false;
      try { zulaessig = this.options.geraetZulaessig(subscription.device_id); } catch { zulaessig = false; }
      if (!zulaessig) continue;
      try {
        const encrypted = encryptWebPushPayload(payload, subscription);
        const endpoint = new URL(subscription.endpoint);
        const response = await this.transport(endpoint, {
          Authorization: vapidAuthorization(material, subscription.endpoint, createdAt),
          'Content-Encoding': encrypted.contentEncoding,
          TTL: '60',
          'Content-Length': String(encrypted.body.length),
        }, encrypted.body);
        if (response.status >= 200 && response.status < 300) sent += 1;
        else {
          failed = true;
          // Der Pushdienst kennt das Abo nicht mehr: es kommt nie wieder an.
          if (response.status === 404 || response.status === 410) this.journal.removeSubscription(subscription.device_id, subscription.id);
        }
      } catch {
        failed = true;
      }
    }
    return { sent, failed };
  }

  private assertEndpoint(value: string): void {
    let endpoint: URL;
    try { endpoint = new URL(value); } catch { throw new Error('push endpoint is invalid'); }
    if (endpoint.protocol !== 'https:' && !(this.options.allowInsecureEndpointForTests && endpoint.protocol === 'http:' && endpoint.hostname === '127.0.0.1')) {
      throw new Error('push endpoint must use HTTPS');
    }
  }

  private recordNotification(notification: Parameters<PushJournal['append']>[0]): void {
    this.journal.append(notification);
    this.options.events?.publish('notification.changed', [notification.id], notification.source);
  }
}

/**
 * Die Journalkennung eines Antrags bzw. einer Frage: stabil aus Maschine und Schluessel, nie aus Inhalt.
 * Bekannte Grenze (pushwelt-Review F4, bleibt so): nach `wb-welt umziehen` meldet der Kern der neuen
 * Maschine dasselbe Anliegen innerhalb von 24 Stunden noch einmal, denn die Maschine geht in den Hash
 * ein; der Merkeintrag des alten Kerns bleibt dort bestehen.
 */
function weltEventId(machine: string, schluessel: string): string {
  return `welt-${createHash('sha256').update(`${machine}\0${schluessel}`).digest('hex').slice(0, 24)}`;
}

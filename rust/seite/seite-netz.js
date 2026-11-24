// Netz der Seite (Vertrag `seite` SE6, Vertrag `bruecke` Abschnitt 5): nur `GET /strom`, `POST /eingabe` und `POST /starten`
// desselben Ursprungs, das Token nur im Kopf `Authorization`, nie in der Adresse.

// Das Token steht im Fragment als `#t=<64 Kleinbuchstaben-Hex>`; sonst gibt es keines.
export function tokenAusFragment(hash) {
  const r = /^#t=([0-9a-f]{64})$/.exec(hash || '');
  return r ? r[1] : '';
}

// Schickt die Ereignisse der Ansicht der Reihe nach: das naechste `POST /eingabe` erst nach der Antwort auf das vorige (SE1 Punkt 11).
export class Sender {
  constructor(token) {
    this.token = token;
    this.verbindung = null;
    this.kette = Promise.resolve();
  }

  sende(ereignis) {
    const verbindung = this.verbindung;
    if (!verbindung) return;
    const body = JSON.stringify(ereignis);
    this.kette = this.kette.then(() => fetch('/eingabe', {
      method: 'POST',
      headers: { 'Authorization': 'Bearer ' + this.token, 'Content-Type': 'application/json', 'X-Verbindung': verbindung },
      body,
    }).then(() => undefined, () => undefined));
  }
}

// `POST /starten` mit der Start-Struktur. Liefert { status, antwort } oder { status: 0 } ohne Verbindung.
export async function starten(token, start) {
  let r;
  try {
    r = await fetch('/starten', {
      method: 'POST',
      headers: { 'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json' },
      body: JSON.stringify(start),
    });
  } catch {
    return { status: 0, antwort: null };
  }
  let antwort = null;
  try {
    antwort = await r.json();
  } catch {
    antwort = null;
  }
  return { status: r.status, antwort };
}

// Liest den Ereignisstrom: Rahmen `[n: u32 big endian][art: u8][nutzlast]` (Vertrag `dienst` Abschnitt 5), in beliebigen Stuecken.
// `beiOffen({ verbindung, deflate })` nach den Koepfen, `beiRahmen(art, nutzlast)` je Rahmen, `beiEnde()` genau einmal am Schluss.
// Liefert eine Funktion, die den Strom abbricht.
export function oeffneStrom(token, { beiOffen, beiRahmen, beiEnde }) {
  const abbruch = new AbortController();
  let beendet = false;
  const ende = () => {
    if (beendet) return;
    beendet = true;
    beiEnde();
  };
  (async () => {
    let antwort;
    try {
      antwort = await fetch('/strom', { headers: { 'Authorization': 'Bearer ' + token }, signal: abbruch.signal });
    } catch {
      ende();
      return;
    }
    if (!antwort.ok || !antwort.body) {
      ende();
      return;
    }
    beiOffen({ verbindung: antwort.headers.get('X-Verbindung'), deflate: antwort.headers.get('X-Deflate') === '1' });
    const leser = antwort.body.getReader();
    let puffer = new Uint8Array(0);
    for (;;) {
      let r;
      try {
        r = await leser.read();
      } catch {
        break;
      }
      if (r.done || beendet) break;
      const neu = new Uint8Array(puffer.length + r.value.length);
      neu.set(puffer);
      neu.set(r.value, puffer.length);
      puffer = neu;
      let p = 0;
      while (puffer.length - p >= 4) {
        const n = puffer[p] * 0x1000000 + (puffer[p + 1] << 16) + (puffer[p + 2] << 8) + puffer[p + 3];
        if (n < 1 || puffer.length - p < 4 + n) break;
        beiRahmen(puffer[p + 4], puffer.slice(p + 5, p + 4 + n));
        p += 4 + n;
        if (beendet) break;
      }
      puffer = puffer.slice(p);
      if (beendet) break;
    }
    ende();
  })();
  return () => {
    abbruch.abort();
    ende();
  };
}

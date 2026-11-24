// Startformular (Vertrag `seite` SE6 Punkt 3, Vertrag `dienst` DI12 Punkt 10): die Wahlfelder fuellt die Nachricht `Startwahl`;
// das Absenden ergibt die Start-Struktur der Fassung 1, „ohne Kunde“ als `null`.

export const OHNE_KUNDE = 'ohne Kunde';

// Setzt die Optionen eines Auswahlfelds. `eintraege`: Liste von { wert, text }. Die bisherige Wahl bleibt, wenn es sie noch gibt;
// sonst gilt `vorgabe` (ein Wert) oder der erste Eintrag.
export function setzeOptionen(select, eintraege, vorgabe = null) {
  const bisher = select.options.length ? select.value : null;
  const doc = select.ownerDocument;
  select.textContent = '';
  for (const { wert, text } of eintraege) {
    const o = doc.createElement('option');
    o.value = wert;
    o.textContent = text;
    select.append(o);
  }
  const werte = eintraege.map((e) => e.wert);
  if (bisher !== null && werte.includes(bisher)) select.value = bisher;
  else if (vorgabe !== null && werte.includes(vorgabe)) select.value = vorgabe;
}

const text = (x) => typeof x === 'string' && x !== '';

// Das Modell, das ohne Angabe der Route gilt (SE6 Punkt 3, SE7 Punkt 2).
export const MODELL_AUTO = 'auto';

// Die Eintraege der Wahlfelder aus einer Nachricht `Startwahl`, oder null, wenn sie nicht die Form der Fassung 1 hat.
// `modelle` ordnet jeder Route ihr Modell zu (der Text der Route oder `null`).
export function eintraegeAusStartwahl(n) {
  if (!n || n.fassung !== 1 || !Array.isArray(n.harness) || !Array.isArray(n.routen) || !Array.isArray(n.kunden)) return null;
  const harness = n.harness.filter((h) => h && text(h.name)).map((h) => ({ wert: h.name, text: h.name }));
  const routen = n.routen.filter((r) => r && text(r.schluessel)).map((r) => ({ wert: r.schluessel, text: r.schluessel }));
  const modelle = new Map(n.routen.filter((r) => r && text(r.schluessel)).map((r) => [r.schluessel, text(r.modell) ? r.modell : null]));
  // „ohne Kunde“ steht zuletzt, auch wenn `null` in der Liste weiter vorn steht oder fehlt.
  const kunden = n.kunden.filter(text).map((k) => ({ wert: k, text: k }));
  kunden.push({ wert: '', text: OHNE_KUNDE });
  return { harness, routen, kunden, modelle };
}

// Der einzige Eintrag des Felds `Modell` fuer die gewaehlte Route: ihr Modell, sonst `auto`.
export function modellEintraege(modelle, route) {
  const m = modelle.get(route);
  const wert = typeof m === 'string' && m !== '' ? m : MODELL_AUTO;
  return [{ wert, text: wert }];
}

// Die Start-Struktur aus den Werten des Formulars (SE6 Punkt 3).
export function startStruktur(werte) {
  return {
    fassung: 1,
    harness: werte.harness,
    modell: werte.modell,
    route: werte.route,
    kunde: werte.kunde === '' ? null : werte.kunde,
    arbeitsordner: werte.arbeitsordner,
  };
}

// Test der Startwahl (Vertrag `seite` SE6 Punkt 3, SE7 Punkt 2): Eintraege der Wahlfelder aus der Nachricht, Modell je Route, `auto` ohne Modell.
// Ohne DOM; Exit ueber process.exitCode.
import { eintraegeAusStartwahl, modellEintraege, MODELL_AUTO } from '../seite-start.js';

let rot = 0;
const pruefe = (ok, text) => {
  if (ok) return;
  rot++;
  console.log('ROT start: ' + text);
};
const gleich = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const n = {
  art: 'Startwahl', fassung: 1,
  harness: [{ name: 'claude', art: 'claude', routenart: 'cloud' }, { name: 'pi', art: 'pi', routenart: 'lokal' }],
  routen: [
    { schluessel: 'cloud', art: 'cloud', klasse: 'extern', modell: 'modell-a' },
    { schluessel: 'offen', art: 'cloud', klasse: 'extern', modell: null },
    { schluessel: 'lokal', art: 'lokal', klasse: 'intern' },
    { schluessel: 'leer', art: 'lokal', klasse: 'intern', modell: '' },
    { schluessel: 'falsch', art: 'lokal', klasse: 'intern', modell: 7 },
  ],
  kunden: ['k-eins', null, 'k-zwei'],
};
const e = eintraegeAusStartwahl(n);
pruefe(e !== null, 'Nachricht der Fassung 1 nicht gelesen');
pruefe(gleich(e.harness.map((h) => h.wert), ['claude', 'pi']), 'Harness');
pruefe(gleich(e.routen.map((r) => r.wert), ['cloud', 'offen', 'lokal', 'leer', 'falsch']), 'Routen');
pruefe(gleich(e.kunden.map((k) => k.text), ['k-eins', 'k-zwei', 'ohne Kunde']), 'ohne Kunde nicht zuletzt: ' + JSON.stringify(e.kunden));
pruefe(e.kunden[e.kunden.length - 1].wert === '', '`ohne Kunde` hat nicht den Wert ""');
const ohneNull = eintraegeAusStartwahl({ ...n, kunden: ['k-eins'] });
pruefe(gleich(ohneNull.kunden.map((k) => k.text), ['k-eins', 'ohne Kunde']), 'ohne Kunde fehlt, wenn `null` fehlt');
const wert = (r) => modellEintraege(e.modelle, r);
pruefe(gleich(wert('cloud'), [{ wert: 'modell-a', text: 'modell-a' }]), 'Route mit Modell');
for (const r of ['offen', 'lokal', 'leer', 'falsch', 'unbekannt']) pruefe(gleich(wert(r), [{ wert: MODELL_AUTO, text: MODELL_AUTO }]), `Route ${r} ohne Modell ergibt nicht auto`);
pruefe(MODELL_AUTO === 'auto', 'MODELL_AUTO');
pruefe(eintraegeAusStartwahl({ ...n, fassung: 2 }) === null, 'Fassung 2 angenommen');
pruefe(eintraegeAusStartwahl(null) === null, 'null angenommen');

if (rot) process.exitCode = 1;
else console.log('start: gruen');

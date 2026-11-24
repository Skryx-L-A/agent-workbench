// Tastatur des Terminals (Vertrag `seite`, SE1 und SE2 Punkt 4): ein `keydown` wird zu einem Ereignis der Ansicht, zu einem
// Fokuswechsel oder zu nichts. Die Funktion uebersetzt nie in Bytes; das tut der Kern (`VT` R1). Ohne DOM, damit sie fuer sich pruefbar ist.

// Benannte Tasten mit den Namen von `VT` R1.
export const TASTEN = {
  Enter: 'Enter', Tab: 'Tab', Escape: 'Escape', Backspace: 'Ruecktaste', Delete: 'Entf',
  ArrowUp: 'Hoch', ArrowDown: 'Runter', ArrowLeft: 'Links', ArrowRight: 'Rechts',
  Home: 'Pos1', End: 'Ende', PageUp: 'BildAuf', PageDown: 'BildAb',
};

// Umschalter immer in der Reihenfolge umschalt, strg, alt (SE1 Punkt 7).
export function umschalterVon(e) {
  const u = [];
  if (e.shiftKey) u.push('umschalt');
  if (e.ctrlKey) u.push('strg');
  if (e.altKey) u.push('alt');
  return u;
}

// Liefert eines von:
//   null                                  nichts schicken, nichts verhindern (dem Browser bleiben, tote Taste, Komposition)
//   { fokus: 1 } oder { fokus: -1 }       F6 oder Umschalt+F6: das Terminal verlassen
//   { art: 'Taste', taste, umschalter }   eine benannte Taste oder Strg/Alt mit Buchstabe
//   { art: 'Text', text }                 ein druckbares Zeichen (auch AltGr und Wahltaste)
export function deuteTaste(e) {
  if (e.isComposing || e.key === 'Dead' || e.key === 'Process' || e.keyCode === 229) return null;
  if (e.key === 'F6') return { fokus: e.shiftKey ? -1 : 1 };
  if (e.metaKey) return null;
  const u = umschalterVon(e);
  if (Object.prototype.hasOwnProperty.call(TASTEN, e.key)) return { art: 'Taste', taste: TASTEN[e.key], umschalter: u };
  if ([...e.key].length !== 1) return null;
  const buchstabe = /^[a-zA-Z]$/.test(e.key);
  if (e.ctrlKey && !e.altKey) {
    // Strg+V und Strg+Umschalt+Buchstabe gehoeren dem Browser (SE1 Punkt 8), Strg mit anderen Zeichen ebenso.
    if (!buchstabe || e.shiftKey || e.key.toLowerCase() === 'v') return null;
    return { art: 'Taste', taste: e.key.toLowerCase(), umschalter: ['strg'] };
  }
  // Alt mit einem unveraenderten Buchstaben ist eine Taste; was die Belegung mit Alt oder AltGr liefert, ist Text (SE1 Punkt 2 und 6).
  if (e.altKey && /^[a-z]$/.test(e.key)) return { art: 'Taste', taste: e.key, umschalter: u.filter((x) => x !== 'umschalt') };
  return { art: 'Text', text: e.key };
}

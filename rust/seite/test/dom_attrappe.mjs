// Eine kleine Attrappe des DOM fuer die Tests unter Node (kein Browser, keine npm-Pakete). Sie kann nur, was `zeileZuHtml` braucht:
// Fragment, Element (`span`), Textknoten. `innerHTML`, `outerHTML` und `insertAdjacentHTML` werfen, damit Text nie als HTML zusammengesetzt wird.

class Textknoten {
  constructor(data) {
    this.nodeType = 3;
    this.data = String(data);
  }
}

class Stil {
  constructor() {
    this.eigenschaften = new Map();
  }

  setProperty(name, wert) {
    this.eigenschaften.set(name, wert);
  }
}

class Element {
  constructor(tag) {
    this.nodeType = 1;
    this.tagName = tag.toUpperCase();
    this.className = '';
    this.style = new Stil();
    this.childNodes = [];
  }

  appendChild(k) {
    this.childNodes.push(k);
    return k;
  }

  set innerHTML(_) {
    throw new Error('innerHTML ist nicht erlaubt');
  }

  set outerHTML(_) {
    throw new Error('outerHTML ist nicht erlaubt');
  }

  insertAdjacentHTML() {
    throw new Error('insertAdjacentHTML ist nicht erlaubt');
  }
}

class Fragment {
  constructor() {
    this.nodeType = 11;
    this.childNodes = [];
  }

  appendChild(k) {
    this.childNodes.push(k);
    return k;
  }
}

export const dokument = {
  createElement: (tag) => new Element(tag),
  createTextNode: (text) => new Textknoten(text),
  createDocumentFragment: () => new Fragment(),
};

// Der Inhalt als einfache Daten: Textknoten als Text, Element als { tag, klassen, stil, kinder }.
export function alsDaten(knoten) {
  if (knoten.nodeType === 3) return knoten.data;
  const kinder = knoten.childNodes.map(alsDaten);
  if (knoten.nodeType === 11) return kinder;
  return {
    tag: knoten.tagName,
    klassen: knoten.className ? knoten.className.split(' ') : [],
    stil: Object.fromEntries(knoten.style.eigenschaften),
    kinder,
  };
}

// Browser-Helfer des Laeufers `drehbuch`: bedient die Seite der Bruecke mit einer VORHANDENEN Playwright-Installation.
// Kein Download, keine Installation. Aufruf: node browser.mjs <pfad zu playwright-core> <profilordner>
// Protokoll: je Zeile auf stdin ein JSON-Objekt {id, op, ...}, je Zeile auf stdout {id, ok, wert|fehler}.
// Der Browser laeuft kopflos mit einem eigenen Profilordner (nie dem Profil eines Nutzers).
import { createRequire } from "node:module";
import readline from "node:readline";

const [, , pw, profil] = process.argv;
const require = createRequire(import.meta.url);
const { chromium } = require(pw);

let kontext = null;
let seite = null;

const sel = (testid) => `[data-testid="${testid}"]`;

async function op(m) {
  switch (m.op) {
    case "oeffnen": {
      kontext = await chromium.launchPersistentContext(profil, { headless: true, args: ["--no-first-run", "--disable-extensions"] });
      seite = kontext.pages()[0] || (await kontext.newPage());
      await seite.goto(m.url, { waitUntil: "load", timeout: 15000 });
      return true;
    }
    case "fuellen":
      await seite.fill(sel(m.testid), m.wert, { timeout: 5000 });
      return true;
    case "waehlen": {
      // Ein Wert, den die Seite nicht anbietet, wird als Option eingefuegt (eine gefaelschte Eingabe des Clients).
      await seite.$eval(sel(m.testid), (s, w) => {
        if (![...s.options].some((o) => o.value === w)) s.add(new Option(w, w));
      }, m.wert);
      await seite.selectOption(sel(m.testid), m.wert, { timeout: 5000 });
      return true;
    }
    case "warteoption":
      // Wartet, bis das select die Option mit dem Wert anbietet. Ohne Erfolg binnen der Frist ein fester Fehlertext ohne Seiteninhalt.
      try {
        await seite.waitForFunction(([q, w]) => { const e = document.querySelector(q); return !!e && e.tagName === "SELECT" && [...e.options].some((o) => o.value === w); },
          [sel(m.testid), m.wert], { timeout: m.frist_ms });
      } catch (e) {
        throw new Error("Option nicht angeboten");
      }
      return true;
    case "klicken":
      await seite.click(sel(m.testid), { timeout: 5000 });
      return true;
    case "vorhanden":
      return (await seite.$(sel(m.testid))) !== null;
    case "sichtbar":
      return await seite.isVisible(sel(m.testid));
    case "text":
      return await seite.textContent(sel(m.testid), { timeout: 5000 });
    case "attribut":
      return await seite.getAttribute(sel(m.testid), m.name, { timeout: 5000 });
    case "zeilen": {
      // alle Zeilen des Rasters: data-testid terminal-zeile-<n>, nach n sortiert
      return await seite.$$eval(`[data-testid^="${m.praefix}"]`, (els, p) =>
        els.map((e) => [parseInt(e.getAttribute("data-testid").slice(p.length), 10), e.textContent])
          .sort((a, b) => a[0] - b[0]).map((x) => x[1]), m.praefix);
    }
    case "liste":
      return await seite.$$eval(`[data-testid^="${m.praefix}"]`, (els, p) =>
        els.map((e) => ({ id: e.getAttribute("data-testid").slice(p.length), zustand: e.getAttribute("data-zustand"), code: e.getAttribute("data-code") })), m.praefix);
    case "schliessen":
      if (kontext) await kontext.close();
      return true;
    default:
      throw new Error("unbekannte Operation " + m.op);
  }
}

const rl = readline.createInterface({ input: process.stdin });
let kette = Promise.resolve();
rl.on("line", (zeile) => {
  kette = kette.then(async () => {
    let m;
    try {
      m = JSON.parse(zeile);
      const wert = await op(m);
      process.stdout.write(JSON.stringify({ id: m.id, ok: true, wert }) + "\n");
    } catch (e) {
      process.stdout.write(JSON.stringify({ id: m ? m.id : null, ok: false, fehler: String(e.message || e).split("\n")[0] }) + "\n");
    }
    if (m && m.op === "schliessen") process.exit(0);
  });
});
rl.on("close", async () => {
  try { if (kontext) await kontext.close(); } catch (e) { /* schon zu */ }
  process.exit(0);
});

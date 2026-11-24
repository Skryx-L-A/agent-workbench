// js-wert.ts -- ein Wert als JavaScript-Literal fuer Code, den der Hauptprozess
// per `webContents.executeJavaScript` in den Renderer gibt.
//
// `JSON.stringify` allein liefert ein gueltiges Literal, laesst aber `<`, `>`,
// `/` und die Zeilentrenner U+2028/U+2029 roh stehen. Wird der Code einmal in
// ein `<script>` eingebettet oder von einer aelteren Engine gelesen, beendet
// `</script>` den Block oder U+2028 die Zeichenkette. Diese Funktion schreibt
// die Zeichen als `\uXXXX`-Escape; der Wert bleibt derselbe (CodeQL
// js/bad-code-sanitization, Empfehlung der Regel).

const UNSICHER: Record<string, string> = {
  '<': '\\u003C',
  '>': '\\u003E',
  '/': '\\u002F',
  '\b': '\\b',
  '\f': '\\f',
  '\n': '\\n',
  '\r': '\\r',
  '\t': '\\t',
  '\0': '\\0',
  '\u2028': '\\u2028',
  '\u2029': '\\u2029',
};

function unsichereZeichenMaskieren(text: string): string {
  return text.replace(/[<>/\b\f\n\r\t\0\u2028\u2029]/g, (z) => UNSICHER[z]);
}

// Ein JSON-faehiger Wert als sicheres JavaScript-Literal; `undefined` wird zu `undefined`.
export function jsWert(wert: unknown): string {
  const json = JSON.stringify(wert);
  if (json === undefined) return 'undefined';
  return unsichereZeichenMaskieren(json);
}

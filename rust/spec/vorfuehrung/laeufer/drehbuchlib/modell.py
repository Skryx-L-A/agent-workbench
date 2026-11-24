"""Modell-Attrappe auf 127.0.0.1, die jedes empfangene Byte mitschreibt (Muster: die Modell-Attrappe des Vertrags `traeger`).

Anthropic-Messages mit und ohne Strom (Claude Code streamt). Die Antwort richtet sich nach der letzten Nutzereingabe,
die der Laeufer tippt:
  `#T<n> BEFEHL <befehl>`  Werkzeugaufruf Bash mit `<befehl>; echo "RC=$?"`, danach der Text `FERTIG T<n>`
  `#T<n> ZAEHLEN`          keine Werkzeuge, nur der Text `FERTIG T<n>` (die gezaehlte Anfrage des knappen Cloud-Laufs)
  `#T<n> BUENDEL <skript>`  wie BEFEHL, mit einem Buendel von Befehlen (Zeilen `<name>=<zahl> RC=<n>`)
  `#T<n> SUCHE <frage>`    Werkzeugaufruf des Such-Werkzeugs mit `query`, danach eine Antwort aus dem Werkzeugergebnis
                           und `FERTIG T<n>`
Alles andere (Titel- und Nebenanfragen ohne Werkzeuge): ein kurzer fester Text. Nichts stammt aus der Umgebung.
Der Mitschnitt haelt die rohen Bytes aller Anfragen in der Reihenfolge ihres Eintreffens.
"""
from __future__ import annotations

import json
import re
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TURN = re.compile(r"^\s*#T(\d+)\s+(?:[A-Z]{2}-[0-9A-Z]{3}\s+)?(BEFEHL|SUCHE|BUENDEL|ZAEHLEN)(?:\s+(.*))?$", re.S)


class Mitschnitt:
    def __init__(self, pfad=None):
        self.puffer = bytearray()
        self.lock = threading.Lock()
        self.anfragen = []  # (offset_ende, geparstes JSON oder None)
        self.pfad = pfad

    def schreibe(self, b):
        with self.lock:
            self.puffer += b
            if self.pfad:
                with open(self.pfad, "ab") as f:
                    f.write(b)

    def laenge(self):
        with self.lock:
            return len(self.puffer)

    def ab(self, offset):
        with self.lock:
            return bytes(self.puffer[offset:])

    def merke(self, json_obj):
        with self.lock:
            self.anfragen.append((len(self.puffer), json_obj))

    def tool_ergebnis(self, turn):
        """Text des Werkzeugergebnisses, das der Agent im Zug `turn` ans Modell zurueckgab: (text, is_error) oder None.
        Kennt beide Schnittstellen: Anthropic (`tool_result`) und OpenAI (`role: tool`)."""
        gesucht = "toolu_T%s" % turn
        with self.lock:
            kopie = list(self.anfragen)
        for _, d in reversed(kopie):
            for m in (d or {}).get("messages", []) if isinstance(d, dict) else []:
                if m.get("role") == "tool" and m.get("tool_call_id") == gesucht:
                    return _als_text(m.get("content")), False
                c = m.get("content")
                for b in c if isinstance(c, list) else []:
                    if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") == gesucht:
                        return _als_text(b.get("content")), bool(b.get("is_error"))
        return None


def _als_text(inh):
    if isinstance(inh, list):
        return "".join(x.get("text", "") for x in inh if isinstance(x, dict))
    return inh or ""


class _Tee:
    def __init__(self, f, mitschnitt):
        self.f, self.m = f, mitschnitt

    def readline(self, n=-1):
        b = self.f.readline(n)
        self.m.schreibe(b)
        return b

    def read(self, n=-1):
        b = self.f.read(n)
        self.m.schreibe(b)
        return b

    def __getattr__(self, name):
        return getattr(self.f, name)


def _text_der_nutzernachricht(m):
    c = m.get("content")
    bloecke = [{"type": "text", "text": c}] if isinstance(c, str) else (c or [])
    return [b.get("text", "") for b in bloecke if isinstance(b, dict) and b.get("type") == "text"
            and not b.get("text", "").lstrip().startswith("<system-reminder>")]


def plan(d, mcp_werkzeug):
    """Liste von Bloecken ("text", s) oder ("tool_use", id, name, eingabe) und stop_reason. Beide Schnittstellen."""
    if not d.get("tools"):
        return [("text", "Aufnahme")], "end_turn"
    nachrichten = d.get("messages") or []
    idx, turn = None, None
    for i, m in enumerate(nachrichten):
        if m.get("role") != "user":
            continue
        for t in _text_der_nutzernachricht(m):
            g = TURN.match(t)
            if g:
                idx, turn = i, g
    if turn is None:
        return [("text", "bereit")], "end_turn"
    n, art, arg = turn.group(1), turn.group(2), (turn.group(3) or "").strip()
    if art == "ZAEHLEN":  # die gezaehlte Anfrage: kein Werkzeug, nur das Ende-Token
        return [("text", "FERTIG T%s" % n)], "end_turn"
    if art == "BUENDEL":  # ein Buendel Shell-Befehle, ein Werkzeugaufruf wie bei BEFEHL
        art = "BEFEHL"
    ergebnisse = []
    for m in nachrichten[idx + 1:]:
        if m.get("role") == "tool":
            ergebnisse.append(_als_text(m.get("content")))
        c = m.get("content")
        for b in c if isinstance(c, list) else []:
            if isinstance(b, dict) and b.get("type") == "tool_result":
                ergebnisse.append(_als_text(b.get("content")))
    tid = "toolu_T%s" % n
    if not ergebnisse:
        if art == "BEFEHL":
            return [("tool_use", tid, "Bash", {"command": arg + '; echo "RC=$?"', "description": "Befehl des Drehbuchs"})], "tool_use"
        return [("tool_use", tid, mcp_werkzeug, {"query": arg})], "tool_use"
    text = "FERTIG T%s" % n
    if art == "SUCHE":
        try:
            a = json.loads(ergebnisse[-1])
            teile = ["%s: %s" % (h.get("title", ""), h.get("text", "")) for h in a.get("hits", [])]
            text = "Antwort: %s | gesperrt: %s %s" % (" || ".join(teile), a.get("gesperrt"), text)
        except ValueError:
            text = "Antwort: %s %s" % (ergebnisse[-1][:200], text)
    return [("text", text)], "end_turn"


def starte(mitschnitt, mcp_werkzeug, bind=("127.0.0.1", 0), schluessel=None):
    """`schluessel`: verlangt `Authorization: Bearer <schluessel>` (Gateway-Attrappe); ohne Schluessel Antwort 401."""
    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def setup(self):
            super().setup()
            self.rfile = _Tee(self.rfile, mitschnitt)

        def log_message(self, *a):
            pass

        def _json(self, obj, code=200):
            b = json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("content-type", "application/json")
            self.send_header("Content-Length", str(len(b)))
            self.end_headers()
            self.wfile.write(b)

        def _chunk(self, s):
            b = s.encode()
            self.wfile.write(("%x\r\n" % len(b)).encode() + b + b"\r\n")
            self.wfile.flush()

        def _erlaubt(self):
            if schluessel is not None and self.headers.get("Authorization") != "Bearer " + schluessel:
                self._json({"error": "unauthorized"}, 401)
                return False
            srv.autorisiert += 1  # Zaehler am Ziel: Anfragen, die mit dem richtigen Schluessel ankamen
            return True

        def do_GET(self):
            if not self._erlaubt():
                return
            if self.path.split("?")[0].endswith("/models"):
                return self._json({"object": "list", "data": [{"id": "attrappe", "object": "model"}]})
            self._json({"error": "nicht da"}, 404)

        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            roh = self.rfile.read(n) if n else b""
            try:
                d = json.loads(roh.decode("utf-8", "replace"))
            except ValueError:
                d = None
            mitschnitt.merke(d)
            if not self._erlaubt():
                return
            p = self.path.split("?")[0]
            try:
                if p.endswith("/messages/count_tokens"):
                    return self._json({"input_tokens": 10})
                if p.endswith("/v1/messages") and isinstance(d, dict):
                    return self.messages(d)
                if p.endswith("/chat/completions") and isinstance(d, dict):
                    return self.chat(d)
            except (BrokenPipeError, ConnectionResetError):
                return
            self._json({"error": "nicht da"}, 404)

        def messages(self, d):
            bloecke, stop = plan(d, mcp_werkzeug)
            modell = d.get("model") or "attrappe"
            msg = {"id": "msg_attrappe", "type": "message", "role": "assistant", "model": modell, "content": [],
                   "stop_reason": None, "stop_sequence": None,
                   "usage": {"input_tokens": 12, "output_tokens": 1, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}}
            if not d.get("stream"):
                msg.update(content=[{"type": "text", "text": b[1]} if b[0] == "text" else
                                    {"type": "tool_use", "id": b[1], "name": b[2], "input": b[3]} for b in bloecke],
                           stop_reason=stop)
                return self._json(msg)
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("cache-control", "no-cache")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()

            def ev(typ, daten):
                self._chunk("event: %s\ndata: %s\n\n" % (typ, json.dumps(dict(daten, type=typ), ensure_ascii=False)))
            ev("message_start", {"message": msg})
            for i, b in enumerate(bloecke):
                if b[0] == "text":
                    ev("content_block_start", {"index": i, "content_block": {"type": "text", "text": ""}})
                    ev("content_block_delta", {"index": i, "delta": {"type": "text_delta", "text": b[1]}})
                else:
                    ev("content_block_start", {"index": i, "content_block": {"type": "tool_use", "id": b[1], "name": b[2], "input": {}}})
                    ev("content_block_delta", {"index": i, "delta": {"type": "input_json_delta", "partial_json": json.dumps(b[3])}})
                ev("content_block_stop", {"index": i})
            ev("message_delta", {"delta": {"stop_reason": stop, "stop_sequence": None}, "usage": {"output_tokens": 20}})
            ev("message_stop", {})
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()


        def chat(self, d):
            """OpenAI-kompatibel (chat/completions) mit Werkzeugaufrufen, mit und ohne Strom."""
            bloecke, stop = plan(d, mcp_werkzeug)
            modell = d.get("model") or "attrappe"
            text = "".join(b[1] for b in bloecke if b[0] == "text")
            aufrufe = [{"id": b[1], "type": "function", "function": {"name": b[2], "arguments": json.dumps(b[3])}}
                       for b in bloecke if b[0] == "tool_use"]
            fin = "tool_calls" if aufrufe else "stop"
            if not d.get("stream"):
                nachricht = {"role": "assistant", "content": text or None}
                if aufrufe:
                    nachricht["tool_calls"] = aufrufe
                return self._json({"id": "c1", "object": "chat.completion", "created": 0, "model": modell,
                                   "choices": [{"index": 0, "message": nachricht, "finish_reason": fin}],
                                   "usage": {"prompt_tokens": 5, "completion_tokens": 5, "total_tokens": 10}})
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("cache-control", "no-cache")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()

            def ch(delta, f=None):
                o = {"id": "c1", "object": "chat.completion.chunk", "created": 0, "model": modell,
                     "choices": [{"index": 0, "delta": delta, "finish_reason": f}]}
                self._chunk("data: %s\n\n" % json.dumps(o, ensure_ascii=False))
            ch({"role": "assistant", "content": ""})
            if text:
                ch({"content": text})
            for i, a in enumerate(aufrufe):
                ch({"tool_calls": [{"index": i, "id": a["id"], "type": "function",
                                    "function": {"name": a["function"]["name"], "arguments": a["function"]["arguments"]}}]})
            ch({}, fin)
            self._chunk("data: [DONE]\n\n")
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()

    srv = ThreadingHTTPServer(bind, H)
    srv.daemon_threads = True
    srv.autorisiert = 0
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv

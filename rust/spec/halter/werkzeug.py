"""Hilfen für bau-faelle.py und pruefen.py: strenges JSON-Lesen, RFC 8785 und Ed25519 (RFC 8032).

Nur Standardbibliothek. Ed25519 ist die Referenz aus RFC 8032 Abschnitt 6 (langsam, nur für die Fälle).
Kanonisch werden nur Texte, Ganzzahlen, Wahrheitswerte, null, Listen und Objekte; eine Gleitkommazahl
gilt hier als „erst mit dem Bau prüfbar“ (die Fälle enthalten keine).
"""
import hashlib
import json

GANZ_GRENZE = 2**53 - 1


# ---------- strenges Lesen ----------

def streng_lesen(roh):
    """Liest eine Zeile ohne LF. Wirft ValueError bei BOM, UTF-8-Fehler, doppeltem Schlüssel, Nicht-Objekt."""
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    text = roh.decode("utf-8")

    def paare(liste):
        obj = {}
        for k, v in liste:
            if k in obj:
                raise ValueError("doppelter Schlüssel")
            obj[k] = v
        return obj

    def keine_konstante(name):
        raise ValueError(f"keine JSON-Zahl: {name}")

    obj = json.loads(text, object_pairs_hook=paare, parse_constant=keine_konstante)
    if not isinstance(obj, dict):
        raise ValueError("kein Objekt")
    return obj


# ---------- RFC 8785 ----------

class NichtKanonisierbar(ValueError):
    pass


def _text(s):
    aus = ['"']
    for z in s:
        o = ord(z)
        if z == '"':
            aus.append('\\"')
        elif z == "\\":
            aus.append("\\\\")
        elif z == "\b":
            aus.append("\\b")
        elif z == "\f":
            aus.append("\\f")
        elif z == "\n":
            aus.append("\\n")
        elif z == "\r":
            aus.append("\\r")
        elif z == "\t":
            aus.append("\\t")
        elif o < 0x20:
            aus.append(f"\\u{o:04x}")
        else:
            aus.append(z)
    aus.append('"')
    return "".join(aus)


def _utf16(s):
    return s.encode("utf-16-be")


def kanonisch(wert):
    """RFC 8785 als str. Wirft NichtKanonisierbar für Gleitkommazahlen und Ganzzahlen außerhalb ±(2^53-1)."""
    if wert is None:
        return "null"
    if wert is True:
        return "true"
    if wert is False:
        return "false"
    if isinstance(wert, int):
        if abs(wert) > GANZ_GRENZE:
            raise NichtKanonisierbar("Ganzzahl außerhalb von ±(2^53-1)")
        return str(wert)
    if isinstance(wert, float):
        # Eine ganzzahlige Gleitkommazahl im sicheren Bereich schreibt RFC 8785 als Ganzzahl (1.0 -> 1).
        if wert.is_integer() and abs(wert) <= GANZ_GRENZE:
            return str(int(wert))
        raise NichtKanonisierbar("Gleitkommazahl (erst mit dem Bau prüfbar)")
    if isinstance(wert, str):
        return _text(wert)
    if isinstance(wert, list):
        return "[" + ",".join(kanonisch(x) for x in wert) + "]"
    if isinstance(wert, dict):
        teile = [_text(k) + ":" + kanonisch(wert[k]) for k in sorted(wert, key=_utf16)]
        return "{" + ",".join(teile) + "}"
    raise NichtKanonisierbar(f"Typ {type(wert).__name__}")


def kanonisch_bytes(wert):
    return kanonisch(wert).encode("utf-8")


def sha256_hex(daten):
    return hashlib.sha256(daten).hexdigest()


def digest(server, werkzeug, argumente, person, vorschau_sha256):
    """digest nach entwurf-mcp-freigabe.md Abschnitt 2."""
    return sha256_hex(kanonisch_bytes({
        "argumente": argumente, "person": person, "server": server,
        "vorschau_sha256": vorschau_sha256, "werkzeug": werkzeug,
    }))


# ---------- Ed25519 (RFC 8032, Abschnitt 6) ----------

_p = 2**255 - 19
_q = 2**252 + 27742317777372353535851937790883648493


def _inv(x):
    return pow(x, _p - 2, _p)


_d = -121665 * _inv(121666) % _p
_wurzel_m1 = pow(2, (_p - 1) // 4, _p)


def _sha512_modq(s):
    return int.from_bytes(hashlib.sha512(s).digest(), "little") % _q


def _add(P, Q):
    A, B = (P[1] - P[0]) * (Q[1] - Q[0]) % _p, (P[1] + P[0]) * (Q[1] + Q[0]) % _p
    C, D = 2 * P[3] * Q[3] * _d % _p, 2 * P[2] * Q[2] % _p
    E, F, G, H = B - A, D - C, D + C, B + A
    return (E * F, G * H, F * G, E * H)


def _mul(s, P):
    Q = (0, 1, 1, 0)
    while s > 0:
        if s & 1:
            Q = _add(Q, P)
        P = _add(P, P)
        s >>= 1
    return Q


def _gleich(P, Q):
    return (P[0] * Q[2] - Q[0] * P[2]) % _p == 0 and (P[1] * Q[2] - Q[1] * P[2]) % _p == 0


def _x(y, vorzeichen):
    if y >= _p:
        return None
    x2 = (y * y - 1) * _inv(_d * y * y + 1)
    if x2 == 0:
        return None if vorzeichen else 0
    x = pow(x2, (_p + 3) // 8, _p)
    if (x * x - x2) % _p != 0:
        x = x * _wurzel_m1 % _p
    if (x * x - x2) % _p != 0:
        return None
    if (x & 1) != vorzeichen:
        x = _p - x
    return x


_gy = 4 * _inv(5) % _p
_gx = _x(_gy, 0)
_G = (_gx, _gy, 1, _gx * _gy % _p)


def _packen(P):
    zi = _inv(P[2])
    x, y = P[0] * zi % _p, P[1] * zi % _p
    return int.to_bytes(y | ((x & 1) << 255), 32, "little")


def _entpacken(s):
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    vz = y >> 255
    y &= (1 << 255) - 1
    x = _x(y, vz)
    return None if x is None else (x, y, 1, x * y % _p)


def _erweitern(geheim):
    h = hashlib.sha512(geheim).digest()
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8
    a |= 1 << 254
    return a, h[32:]


def oeffentlich(geheim):
    a, _ = _erweitern(geheim)
    return _packen(_mul(a, _G))


def signieren(geheim, nachricht):
    a, praefix = _erweitern(geheim)
    A = _packen(_mul(a, _G))
    r = _sha512_modq(praefix + nachricht)
    Rs = _packen(_mul(r, _G))
    h = _sha512_modq(Rs + A + nachricht)
    return Rs + int.to_bytes((r + h * a) % _q, 32, "little")


def pruefen_signatur(oeff, nachricht, signatur):
    if len(oeff) != 32 or len(signatur) != 64:
        return False
    A = _entpacken(oeff)
    R = _entpacken(signatur[:32])
    if A is None or R is None:
        return False
    s = int.from_bytes(signatur[32:], "little")
    if s >= _q:
        return False
    h = _sha512_modq(signatur[:32] + oeff + nachricht)
    return _gleich(_mul(s, _G), _add(R, _mul(h, A)))


def selbsttest():
    """RFC 8032, Abschnitt 7.1, Test 1 und RFC 8785-Kleinfall. Wirft AssertionError."""
    geheim = bytes.fromhex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
    assert oeffentlich(geheim).hex() == "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    sig = signieren(geheim, b"")
    assert sig.hex() == ("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bac"
                         "c61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
    assert pruefen_signatur(oeffentlich(geheim), b"", sig)
    assert not pruefen_signatur(oeffentlich(geheim), b"x", sig)
    # Schlüsselordnung nach UTF-16: U+10000 (D800 DC00) vor U+E000.
    assert kanonisch({"": 1, "\U00010000": 2, "a": "\u0001\n"}) == '{"a":"\\u0001\\n","\U00010000":2,"":1}'

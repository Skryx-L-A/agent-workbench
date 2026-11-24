#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle von Fassung 2 des Vertrags peerkanal: faelle/<name>.json und erwartet/<name>.json.

Aufruf: python3 -I rust/spec/peerkanal/fassung-2/bau-faelle.py
Die Fälle von Fassung 1 (rust/spec/peerkanal/faelle, erwartet) baut rust/spec/peerkanal/bau-faelle.py; dieses Programm
fasst sie nicht an. Es löscht nichts: Liegt in faelle/ oder erwartet/ eine Datei, die es nicht schreibt, endet es mit
Exit 1 und nennt sie.
Alle Kennungen sind Beispielwerte (zehnstellige Zahlen ab 10000). Der Außenläufer ersetzt sie über `konten` durch die
Werte der Testmaschine.
"""
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent

UID = 1000000101          # die Person
UID_HALTER = 1000000103   # Systemkonto des Servers (wb-halter)
GID = 1000000201          # Hauptgruppe der Person
GID_FREMD = 1000000202    # eine Gruppe, die keine Rollengruppe ist
G_HUB = 1000000211        # Rollengruppe wb-hub
G_BRUECKE = 1000000212    # Rollengruppe wb-bruecke
ORDNER = "/k-beispiel/lauf/pk"
ORDNER_ROLLE = "/k-beispiel/run/wb-halter/hub"
LINUX = ["linux"]
MAC = ["mac"]

FAELLE = {}


def fall(name, beschreibung, art, ort, konten, server, client, aufbau, handlung, erwartet):
    FAELLE[name] = (
        {"fassung": 2, "beschreibung": beschreibung, "art": art, "ort": ort, "konten": konten, "server": server,
         "client": client, "aufbau": aufbau, "handlung": handlung},
        erwartet,
    )


def erw(seite, code=None, ereignisse=(), peer=None, gemessen=()):
    return {"fassung": 2, "seite": seite, "ergebnis": "ok" if code is None else "fehler", "code": code,
            "ereignisse": list(ereignisse), "peer": peer, "gemessen": list(gemessen)}


def peer_linux(uid, gid, gruppen, cgroup="=client", programm="=client"):
    return {"system": "linux", "sid": None, "appcontainer_sid": None, "uid": uid, "gid": gid,
            "pid": "=client", "startzeit": "=client", "im_job": None, "programm": programm, "signatur": None,
            "eingeschraenkt": None, "sandbox": None, "gruppen": gruppen, "cgroup": cgroup}


def peer_mac(uid, gid):
    return {"system": "macos", "sid": None, "appcontainer_sid": None, "uid": uid, "gid": gid,
            "pid": "=client", "startzeit": "=client", "im_job": None, "programm": "=client", "signatur": "=client",
            "eingeschraenkt": None, "sandbox": False, "gruppen": None, "cgroup": None}


def ab(code, grund, peer):
    return {"ereignis": "abgewiesen", "kanal": "k-beispiel", "code": code, "grund": grund, "peer": peer}


def sv(erlaubt, modus="0600", ordner=ORDNER, **unix_mehr):
    unix = {"ordner": ordner, "modus": modus}
    unix.update(unix_mehr)
    return {"name": "k-beispiel", "erlaubt": erlaubt, "instanzen": 4, "unix": unix}


def cv(uid=UID, ordner=ORDNER):
    return {"name": "k-beispiel", "erwartet": {"uid": uid}, "frist_ms": 2000, "unix_ordner": ordner}


K = {"uid_person": UID, "gid_person": GID, "ordner": ORDNER}
KH = dict(K, g_hub=G_HUB)
VU = "pk.vorgabe_ungueltig"
KNE = "pk.kennung_nicht_erlaubt"
SETGID_HUB = "Rollenprogramm des Tests: root:g_hub, Modus 2755, auf einem Dateisystem ohne nosuid; Client startet es ohne no_new_privs"
GRUPPEN_PERSON = "Testkonto uid_person: Hauptgruppe gid_person, Ergänzungsgruppen genau [gid_person], Mitglied keiner Rollengruppe"

# --- Vorgaben: schon heute mit dem Schema prüfbar -------------------------------------------------------------------
fall("v2-gueltig-gruppe", "gültige Vorgabe mit Erlaubnis {uid, gruppe}", "vorgabe", LINUX, KH,
     sv([{"uid": UID, "gruppe": G_HUB}]), cv(), [], ["Vorgabe prüfen"], erw("vorgabe"))
fall("v2-gid-und-gruppe", "Erlaubnis mit gid und gruppe zugleich", "vorgabe", LINUX, KH,
     sv([{"uid": UID, "gid": GID, "gruppe": G_HUB}]), None, [], ["Server anlegen"], erw("vorgabe", VU))
fall("v2-gruppe-ohne-uid", "Erlaubnis nur mit gruppe, ohne uid", "vorgabe", LINUX, KH,
     sv([{"gruppe": G_HUB}]), None, [], ["Server anlegen"], erw("vorgabe", VU))
fall("v2-ordner-gruppe-modus", "ordner_gruppe mit Socket-Modus 0600", "vorgabe", LINUX,
     {"uid_halter": UID_HALTER, "uid_person": UID, "g_hub": G_HUB, "ordner": ORDNER_ROLLE},
     sv([{"uid": UID, "gruppe": G_HUB}], ordner=ORDNER_ROLLE, ordner_gruppe=G_HUB), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v2-gueltig-ordner-gruppe", "gültige Vorgabe: Ordner 0750 mit Rollengruppe, Socket 0666", "vorgabe", LINUX,
     {"uid_halter": UID_HALTER, "uid_person": UID, "g_hub": G_HUB, "ordner": ORDNER_ROLLE},
     sv([{"uid": UID, "gruppe": G_HUB}], modus="0666", ordner=ORDNER_ROLLE, ordner_gruppe=G_HUB), None, [],
     ["Vorgabe prüfen"], erw("vorgabe"))

# --- Anlegen ---------------------------------------------------------------------------------------------------------
fall("a-gruppe-unter-mac", "Erlaubnis {uid, gruppe} unter macOS", "aussentest", MAC, KH,
     sv([{"uid": UID, "gruppe": G_HUB}]), None, ["Ordner 0700 der eigenen Kennung"], ["Server::anlegen"],
     erw("anlegen", VU, gemessen=["Rückgabe pk.vorgabe_ungueltig", "kein Eintrag am Socketpfad"]))
fall("a-ordner-gruppe-linux", "Server unter eigenem Konto, Ordner 0750 mit der Rollengruppe", "aussentest", LINUX,
     {"uid_halter": UID_HALTER, "uid_person": UID, "g_hub": G_HUB, "ordner": ORDNER_ROLLE},
     sv([{"uid": UID, "gruppe": G_HUB}], modus="0666", ordner=ORDNER_ROLLE, ordner_gruppe=G_HUB), None,
     ["Server läuft unter uid_halter, Mitglied keiner Rollengruppe",
      "ordner gehört uid_halter:g_hub mit Modus 0750 (wie systemd-tmpfiles ihn anlegt)"],
     ["Server::anlegen", "ein Prozess unter uid_person ohne g_hub ruft connect", "derselbe mit g_hub als gid"],
     erw("anlegen",
         gemessen=["Rückgabe ok, Socket mit Modus 0666 vor listen",
                   "ohne g_hub: connect scheitert mit EACCES, kein Ereignis (Annahme L-2)",
                   "mit g_hub: Verbindung kommt beim Server an"]))
fall("a-ordner-gruppe-falsch-linux", "Ordner 0750 mit einer anderen Gruppe als ordner_gruppe", "aussentest", LINUX,
     {"uid_halter": UID_HALTER, "uid_person": UID, "g_hub": G_HUB, "g_bruecke": G_BRUECKE, "ordner": ORDNER_ROLLE},
     sv([{"uid": UID, "gruppe": G_HUB}], modus="0666", ordner=ORDNER_ROLLE, ordner_gruppe=G_HUB), None,
     ["Server läuft unter uid_halter", "ordner gehört uid_halter:g_bruecke mit Modus 0750"],
     ["Server::anlegen", "danach derselbe Aufruf mit ordner uid_halter:g_hub und Modus 0700"],
     erw("anlegen", "pk.ordner_rechte",
         gemessen=["beide Aufrufe: pk.ordner_rechte", "Modus, Eigentümer und Gruppe des Ordners unverändert"]))

# --- Annehmen --------------------------------------------------------------------------------------------------------
fall("a-rollengruppe-linux", "Client mit der Rollengruppe als gid (setgid-Rollenprogramm)", "aussentest", LINUX, KH,
     sv([{"uid": UID, "gruppe": G_HUB}]), cv(), [GRUPPEN_PERSON, SETGID_HUB],
     ["das Rollenprogramm verbindet 100-mal"],
     erw("annehmen", peer=peer_linux(UID, G_HUB, [GID]),
         gemessen=["100 von 100 angenommen", "peer.gid gleich g_hub, peer.gruppen gleich [gid_person] (Annahme L-1)",
                   "peer.cgroup gleich der Zeile 0:: aus /proc/<pid>/cgroup beim Client"]))
fall("n-gid-fremd-unix", "Client mit fremder Gruppe als gid, Rollengruppe weder gid noch in gruppen", "aussentest",
     LINUX, dict(KH, gid_fremd=GID_FREMD), sv([{"uid": UID, "gruppe": G_HUB}]), cv(),
     ["Testkonto uid_person: Ergänzungsgruppen genau [gid_person, gid_fremd]", "Client läuft mit gid gid_fremd (newgrp)"],
     ["Client verbindet"],
     erw("annehmen", KNE, [ab(KNE, "pk.gruppe_fehlt", peer_linux(UID, GID_FREMD, [GID, GID_FREMD]))],
         gemessen=["Ereignis mit grund pk.gruppe_fehlt", "Client: pk.abgewiesen", "Server liest außer dem Hallo kein Byte"]))
fall("n-rollengruppe-nur-ergaenzend-linux", "Rollengruppe nur in gruppen, nicht als gid", "aussentest", LINUX, KH,
     sv([{"uid": UID, "gruppe": G_HUB}]), cv(),
     ["Testkonto uid_person: Hauptgruppe gid_person, Ergänzungsgruppen genau [gid_person, g_hub] (wie SupplementaryGroups= einer System-Einheit)"],
     ["Client verbindet"],
     erw("annehmen", peer=peer_linux(UID, GID, [GID, G_HUB]),
         gemessen=["angenommen bis zum Haken: g_hub steht in peer.gruppen, peer.gid ist gid_person",
                   "ob das für eine Rolle reicht, entscheidet der Aufrufer im Haken (Grundsatz 8)"]))
fall("n-mehrere-rollengruppen", "Client trägt zwei Rollengruppen; der Haken des Aufrufers lehnt ab", "aussentest",
     LINUX, dict(KH, g_bruecke=G_BRUECKE),
     sv([{"uid": UID, "gruppe": G_HUB}, {"uid": UID, "gruppe": G_BRUECKE}]), cv(),
     ["Testkonto uid_person: Ergänzungsgruppen genau [gid_person, g_bruecke]", SETGID_HUB,
      "Haken des Aufrufers wie im Halter: genau eine Rollengruppe in gid und gruppen, sonst ablehnen mit aufrufer:rollen-mehrdeutig"],
     ["das Rollenprogramm verbindet"],
     erw("annehmen", "pk.aufrufer_abgewiesen",
         [ab("pk.aufrufer_abgewiesen", "aufrufer:rollen-mehrdeutig", peer_linux(UID, G_HUB, [GID, G_BRUECKE]))],
         gemessen=["die Liste lässt den Peer durch (beide Erlaubnisse treffen); erst der Haken lehnt ab",
                   "Ereignis mit peer.gid g_hub und g_bruecke in peer.gruppen",
                   "Client: pk.abgewiesen, 0 Byte vom Server, auch kein Hallo"]))
fall("n-gruppen-null-linux", "SO_PEERGROUPS scheitert, die Rollengruppe ist nicht die gid: fail closed", "aussentest",
     LINUX, KH, sv([{"uid": UID, "gruppe": G_HUB}]), cv(),
     [GRUPPEN_PERSON, "Testnaht des Läufers: getsockopt(SO_PEERGROUPS) am Server endet mit ENOPROTOOPT"],
     ["Client verbindet ohne Rollenprogramm"],
     erw("annehmen", KNE, [ab(KNE, "pk.gruppen_unlesbar", peer_linux(UID, GID, None))],
         gemessen=["Ereignis mit grund pk.gruppen_unlesbar und peer.gruppen null (nicht [])", "Client: pk.abgewiesen"]))
fall("n-gruppen-null-gid-trifft-linux", "SO_PEERGROUPS scheitert, die Rollengruppe ist die gid", "aussentest", LINUX,
     KH, sv([{"uid": UID, "gruppe": G_HUB}]), cv(),
     [GRUPPEN_PERSON, SETGID_HUB, "Testnaht des Läufers: getsockopt(SO_PEERGROUPS) am Server endet mit ENOPROTOOPT"],
     ["das Rollenprogramm verbindet"],
     erw("annehmen", peer=peer_linux(UID, G_HUB, None),
         gemessen=["angenommen, peer.gruppen null: für gruppe gleich gid braucht die Prüfung gruppen nicht"]))
fall("n-cgroup-unlesbar", "die Cgroup des Clients ist nicht lesbar", "aussentest", LINUX, K, sv([{"uid": UID}]), cv(),
     ["Testnaht des Läufers: das Lesen von /proc/<pid>/cgroup am Server endet mit EACCES"], ["Client verbindet"],
     erw("annehmen", peer=peer_linux(UID, GID, [GID], cgroup=None),
         gemessen=["angenommen, peer.cgroup null, kein Ereignis und kein Abbruch",
                   "Gegenprobe: dieselbe Erlaubnis mit gruppe statt ohne ändert daran nichts, cgroup gehört zu keiner Erlaubnis"]))
fall("n-cgroup-linux", "Cgroup des Clients wird gemessen", "aussentest", LINUX, K, sv([{"uid": UID}]), cv(),
     ["Client läuft in einer eigenen Cgroup (transiente Einheit des Läufers)"], ["Client verbindet"],
     erw("annehmen", peer=peer_linux(UID, GID, [GID]),
         gemessen=["peer.cgroup gleich dem Pfad aus der Zeile 0:: von /proc/<pid>/cgroup beim Client",
                   "die Bibliothek prüft nach dem Lesen an der pidfd, dass der Prozess noch lebt"]))
fall("n-erlaubt-mac-fassung2", "macOS in Fassung 2: gruppen und cgroup sind null", "aussentest", MAC, K,
     sv([{"uid": UID}]), cv(), [], ["Client verbindet"],
     erw("annehmen", peer=peer_mac(UID, GID),
         gemessen=["angenommen, peer.gruppen null und peer.cgroup null", "übrige Felder wie in n-erlaubt-unix"]))


def main():
    fremd = []
    for ordner in ("faelle", "erwartet"):
        ziel = HIER / ordner
        ziel.mkdir(exist_ok=True)
        fremd += [str(p.relative_to(HIER)) for p in sorted(ziel.iterdir()) if p.stem not in FAELLE]
    if fremd:
        print("Dateien, die dieses Programm nicht schreibt (bitte selbst prüfen und entfernen):", *fremd, sep="\n  ")
        return 1
    for name, (f, e) in sorted(FAELLE.items()):
        for ordner, obj in (("faelle", f), ("erwartet", e)):
            text = json.dumps(obj, ensure_ascii=False, indent=1) + "\n"
            (HIER / ordner / f"{name}.json").write_text(text, encoding="utf-8")
    print(f"{len(FAELLE)} Fälle geschrieben")
    return 0


if __name__ == "__main__":
    sys.exit(main())

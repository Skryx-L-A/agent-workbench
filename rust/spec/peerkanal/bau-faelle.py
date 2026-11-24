#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags peerkanal: faelle/<name>.json und erwartet/<name>.json.

Aufruf: python3 -I rust/spec/peerkanal/bau-faelle.py
Alle Kennungen sind Beispielwerte (zehnstellige Zahlen ab 10000). Der Außenläufer ersetzt sie über `konten` durch die
Werte der Testmaschine. Echte SIDs, uids oder Namen stehen nie in den Fällen.
"""
import json
import shutil
from pathlib import Path

HIER = Path(__file__).resolve().parent

PERSON = "S-1-5-21-1000000001-1000000002-1000000003-1001"
PERSON2 = "S-1-5-21-1000000001-1000000002-1000000003-1002"
FREMD = "S-1-5-21-1000000001-1000000002-1000000003-1003"
DIENST = "S-1-5-80-1000000011-1000000012-1000000013-1000000014-1000000015"
CONT_A = "S-1-15-2-1000000021-1000000022-1000000023-1000000024-1000000025-1000000026-1000000027"
CONT_B = "S-1-15-2-1000000031-1000000032-1000000033-1000000034-1000000035-1000000036-1000000037"
UID = 1000000101
UID_FREMD = 1000000102
GID = 1000000201
ORDNER = "/k-beispiel/lauf/pk"
WIN = ["windows"]
UNIX = ["linux", "mac"]
ALLE = ["windows", "linux", "mac"]

FAELLE = {}


def fall(name, beschreibung, art, ort, konten, server, client, aufbau, handlung, erwartet):
    FAELLE[name] = (
        {"beschreibung": beschreibung, "art": art, "ort": ort, "konten": konten, "server": server, "client": client,
         "aufbau": aufbau, "handlung": handlung},
        erwartet,
    )


def erw(seite, code=None, ereignisse=(), peer=None, gemessen=()):
    return {"seite": seite, "ergebnis": "ok" if code is None else "fehler", "code": code,
            "ereignisse": list(ereignisse), "peer": peer, "gemessen": list(gemessen)}


def peer_win(sid, cont=None, pid="=client", start="=client", im_job=None, programm="=client", eingeschraenkt=False):
    return {"system": "windows", "sid": sid, "appcontainer_sid": cont, "uid": None, "gid": None,
            "pid": pid, "startzeit": start, "im_job": im_job, "programm": programm, "signatur": None,
            "eingeschraenkt": eingeschraenkt, "sandbox": None}


def peer_unix(system, uid, gid, pid="=client", start="=client", programm="=client", signatur=None, sandbox=None):
    # signatur und sandbox gibt es nur unter macOS; bei system "=system" setzt der Außenläufer sie für macOS ein.
    return {"system": system, "sid": None, "appcontainer_sid": None, "uid": uid, "gid": gid,
            "pid": pid, "startzeit": start, "im_job": None, "programm": programm, "signatur": signatur,
            "eingeschraenkt": None, "sandbox": sandbox}


def ab(code, grund, peer, kanal="k-beispiel"):
    return {"ereignis": "abgewiesen", "kanal": kanal, "code": code, "grund": grund, "peer": peer}


def sv_win(erlaubt, **mehr):
    v = {"name": "k-beispiel", "erlaubt": erlaubt, "instanzen": 4}
    v.update(mehr)
    return v


def sv_unix(erlaubt, modus="0600", **mehr):
    v = {"name": "k-beispiel", "erlaubt": erlaubt, "instanzen": 4, "unix": {"ordner": ORDNER, "modus": modus}}
    v.update(mehr)
    return v


def cv_win(sid, frist=2000):
    return {"name": "k-beispiel", "erwartet": {"sid": sid}, "frist_ms": frist}


def cv_unix(uid, frist=2000):
    return {"name": "k-beispiel", "erwartet": {"uid": uid}, "frist_ms": frist, "unix_ordner": ORDNER}


KW = {"person": PERSON}
KU = {"uid_person": UID, "gid_person": GID, "ordner": ORDNER}
VU = "pk.vorgabe_ungueltig"

# --- Vorgaben: schon heute mit dem Schema prüfbar -------------------------------------------------------------------
fall("v-name-form", "Kanalname mit Großbuchstaben", "vorgabe", ALLE, KW,
     dict(sv_win([{"sid": PERSON, "appcontainer_sid": None}]), name="K-Beispiel"), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-erlaubt-leer", "keine erlaubte Kennung", "vorgabe", ALLE, {}, sv_win([]), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-instanzen-null", "null Instanzen", "vorgabe", ALLE, KW,
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], instanzen=0), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-instanzen-65", "mehr als 64 Instanzen", "vorgabe", ALLE, KW,
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], instanzen=65), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-sid-everyone", "Everyone (S-1-1-0) als erlaubte Kennung", "vorgabe", WIN, {"everyone": "S-1-1-0"},
     sv_win([{"sid": "S-1-1-0", "appcontainer_sid": None}]), None, [], ["Server anlegen"], erw("vorgabe", VU))
fall("v-sid-authenticated-users", "Authenticated Users (S-1-5-11) als erlaubte Kennung", "vorgabe", WIN,
     {"au": "S-1-5-11"}, sv_win([{"sid": "S-1-5-11", "appcontainer_sid": None}]), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-alle-pakete", "ALL APPLICATION PACKAGES (S-1-15-2-1) als AppContainer-SID", "vorgabe", WIN,
     {"person": PERSON, "ac": "S-1-15-2-1"}, sv_win([{"sid": PERSON, "appcontainer_sid": "S-1-15-2-1"}]), None, [],
     ["Server anlegen"], erw("vorgabe", VU))
fall("v-gemischt", "SID und uid in derselben Liste", "vorgabe", ALLE, {"person": PERSON, "uid_person": UID},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}, {"uid": UID}]), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-unix-ohne-ordner", "uid-Liste ohne Unix-Teil", "vorgabe", UNIX, KU,
     {"name": "k-beispiel", "erlaubt": [{"uid": UID}], "instanzen": 1}, None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-windows-mit-unix-teil", "SID-Liste mit Unix-Teil", "vorgabe", WIN, KW,
     dict(sv_win([{"sid": PERSON, "appcontainer_sid": None}]), unix={"ordner": ORDNER, "modus": "0600"}), None, [],
     ["Server anlegen"], erw("vorgabe", VU))
fall("v-modus-0644", "Socket-Modus 0644", "vorgabe", UNIX, KU, sv_unix([{"uid": UID}], modus="0644"), None, [],
     ["Server anlegen"], erw("vorgabe", VU))
fall("v-ordner-relativ", "Socket-Ordner als relativer Pfad", "vorgabe", UNIX, KU,
     dict(sv_unix([{"uid": UID}]), unix={"ordner": "k-beispiel/pk", "modus": "0600"}), None, [], ["Server anlegen"],
     erw("vorgabe", VU))
fall("v-ordner-punktpunkt", "Socket-Ordner mit ..", "vorgabe", UNIX, KU,
     dict(sv_unix([{"uid": UID}]), unix={"ordner": "/k-beispiel/../pk", "modus": "0600"}), None, [],
     ["Server anlegen"], erw("vorgabe", VU))
fall("v-job-unter-unix", "Job-Prüfung unter Unix verlangt", "vorgabe", UNIX, KU,
     sv_unix([{"uid": UID}], im_job_pruefen=True), None, [], ["Server anlegen"], erw("vorgabe", VU))
fall("v-client-frist-null", "Client mit Frist 0 ms", "vorgabe", ALLE, {"dienst": DIENST}, None, cv_win(DIENST, 0), [],
     ["Client verbinden"], erw("vorgabe", VU))
fall("v-client-unix-ohne-ordner", "Client erwartet uid, nennt aber keinen Ordner", "vorgabe", UNIX, KU, None,
     {"name": "k-beispiel", "erwartet": {"uid": UID}, "frist_ms": 2000}, [], ["Client verbinden"], erw("vorgabe", VU))
fall("v-gueltig-windows", "gültige Vorgabe Windows mit Person und Container", "vorgabe", WIN,
     {"person": PERSON, "container_a": CONT_A},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}, {"sid": PERSON, "appcontainer_sid": CONT_A}],
            prozess_pflicht=True, im_job_pruefen=True, hallo_frist_ms=2000), None, [], ["Vorgabe prüfen"],
     erw("vorgabe"))
fall("v-gueltig-unix", "gültige Vorgabe Unix mit uid und gid", "vorgabe", UNIX, KU,
     sv_unix([{"uid": UID, "gid": GID}], modus="0660"), cv_unix(UID), [], ["Vorgabe prüfen"], erw("vorgabe"))

# --- Anlegen ---------------------------------------------------------------------------------------------------------
fall("a-vorbeleger-windows", "ein Prozess der Person belegt den Pipe-Namen vor dem Dienst", "aussentest", WIN,
     {"person": PERSON, "dienst": DIENST}, sv_win([{"sid": PERSON, "appcontainer_sid": None}]), None,
     ["Der Prozess der Person legt \\\\.\\pipe\\k-beispiel an und hält ihn offen."],
     ["Der Dienst ruft Server::anlegen."],
     erw("anlegen", "pk.name_belegt",
         gemessen=["Rückgabe von anlegen", "OS-Fehler 5 (ERROR_ACCESS_DENIED) aus CreateNamedPipeW mit FILE_FLAG_FIRST_PIPE_INSTANCE",
                   "Die Pipe des Vorbelegers besteht unverändert weiter; der Dienst legt keine Instanz ohne FILE_FLAG_FIRST_PIPE_INSTANCE an"]))
fall("a-vorbeleger-unix", "am Socketpfad liegt schon ein Socket (auch ein verwaister)", "aussentest", UNIX, KU,
     sv_unix([{"uid": UID}]), None,
     ["Ordner 0700 der eigenen Kennung", "Ein Socket k-beispiel.sock liegt dort; niemand hört an ihm."],
     ["Server::anlegen"],
     erw("anlegen", "pk.name_belegt",
         gemessen=["Rückgabe von anlegen", "Inode, Modus und mtime des vorhandenen Eintrags vorher und nachher gleich"]))
fall("a-vorbeleger-datei-unix", "am Socketpfad liegt eine Datei oder ein Link", "aussentest", UNIX, KU,
     sv_unix([{"uid": UID}]), None,
     ["Ordner 0700", "k-beispiel.sock ist ein symbolischer Link auf eine Datei außerhalb des Ordners"],
     ["Server::anlegen"],
     erw("anlegen", "pk.name_belegt", gemessen=["Rückgabe von anlegen", "Link und Ziel unverändert"]))
fall("a-ordner-0755", "Socket-Ordner mit Modus 0755", "aussentest", UNIX, KU, sv_unix([{"uid": UID}]), None,
     ["Ordner der eigenen Kennung mit Modus 0755"], ["Server::anlegen"],
     erw("anlegen", "pk.ordner_rechte", gemessen=["Rückgabe von anlegen", "Ordner danach weiter 0755, kein Socket darin"]))
fall("a-ordner-link", "Socket-Ordner ist ein symbolischer Link", "aussentest", UNIX, KU, sv_unix([{"uid": UID}]), None,
     ["Pfad des Ordners ist ein Link auf einen Ordner 0700"], ["Server::anlegen"],
     erw("anlegen", "pk.ordner_rechte", gemessen=["Rückgabe von anlegen", "kein Socket im Ziel des Links"]))
fall("a-ordner-fremd", "Socket-Ordner gehört einer anderen Kennung", "aussentest", UNIX,
     dict(KU, uid_fremd=UID_FREMD), sv_unix([{"uid": UID}]), None,
     ["Ordner 0700, Besitzer uid_fremd, für den Server betretbar über eine ACL oder als root angelegt"],
     ["Server::anlegen als uid_person"],
     erw("anlegen", "pk.ordner_rechte", gemessen=["Rückgabe von anlegen"]))
fall("a-ordner-fehlt", "Socket-Ordner fehlt", "aussentest", UNIX, KU, sv_unix([{"uid": UID}]), None,
     ["Der Ordner existiert nicht"], ["Server::anlegen"],
     erw("anlegen", "pk.ordner_rechte", gemessen=["Rückgabe von anlegen", "Ordner danach weiter nicht vorhanden"]))
fall("a-pfad-zu-lang-mac", "Socketpfad mit 104 Byte am Mac", "aussentest", ["mac"], KU, sv_unix([{"uid": UID}]), None,
     ["Ordner 0700, dessen Pfad mit /k-beispiel.sock genau 104 Byte ergibt"], ["Server::anlegen"],
     erw("anlegen", "pk.pfad_zu_lang", gemessen=["Rückgabe von anlegen", "kein Eintrag im Ordner"]))
fall("a-pfad-grenze-linux", "Socketpfad mit 107 Byte unter Linux", "aussentest", ["linux"], KU, sv_unix([{"uid": UID}]),
     None, ["Ordner 0700, dessen Pfad mit /k-beispiel.sock genau 107 Byte ergibt"], ["Server::anlegen"],
     erw("anlegen", gemessen=["Rückgabe von anlegen", "Socket vorhanden"]))
fall("a-dacl-windows", "die Pipe trägt nur die erlaubten SIDs", "aussentest", WIN,
     {"person": PERSON, "dienst": DIENST, "container_a": CONT_A},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}, {"sid": PERSON, "appcontainer_sid": CONT_A}]), None,
     ["Dienst unter dienst"], ["Server::anlegen", "Ein Prüfprogramm liest die Sicherheitsbeschreibung der Instanz (GetSecurityInfo)"],
     erw("anlegen", gemessen=[
         "Besitzer = dienst",
         "DACL geschützt (SE_DACL_PROTECTED), genau diese ACEs: dienst 0x0012019F; person und container_a je 0x0012019B (ohne FILE_CREATE_PIPE_INSTANCE 0x4)",
         "SACL mit genau dem Label S:(ML;;NW;;;LW), weil die Liste eine Package-SID trägt",
         "keine ACE für S-1-1-0, S-1-5-11, S-1-5-32-545, S-1-5-32-544, S-1-5-4, S-1-5-2, S-1-5-7, S-1-15-2-1, S-1-15-2-2",
         "PIPE_REJECT_REMOTE_CLIENTS gesetzt (Beleg über den Fall n-remote-windows)"]))
fall("a-name-bleibt-belegt-windows", "zwischen zwei Verbindungen kann niemand den Namen übernehmen", "aussentest", WIN,
     {"person": PERSON, "dienst": DIENST}, sv_win([{"sid": PERSON, "appcontainer_sid": None}], instanzen=1), None,
     ["Dienst unter dienst mit einer Instanz", "Ein Prozess der Person versucht in einer Schleife CreateNamedPipeW auf denselben Namen"],
     ["100 Verbindungen der Person nacheinander, jede verbindet, sendet, schließt"],
     erw("anlegen", gemessen=["100 von 100 Verbindungen bestätigen den Dienst (Besitzer = dienst)",
                              "jeder Versuch des Vorbelegers scheitert (Fehler 5 oder 231)"]))
fall("a-modus-unix", "Socket mit Modus 0600 im Ordner 0700", "aussentest", UNIX, KU, sv_unix([{"uid": UID}]), None,
     ["Ordner 0700 der eigenen Kennung"], ["Server::anlegen", "lstat auf den Socket vor dem ersten annehmen"],
     erw("anlegen", gemessen=["Socket vom Typ S_IFSOCK, Modus 0600, Besitzer uid_person", "Ordner weiter 0700"]))
fall("a-zweimal-anlegen", "zweiter Server desselben Prozesses auf denselben Namen", "aussentest", ALLE,
     dict(KW, **KU), None, None, ["Server A läuft auf k-beispiel"], ["derselbe Prozess legt Server B mit derselben Vorgabe an"],
     erw("anlegen", "pk.name_belegt", gemessen=["Rückgabe für B", "Server A nimmt weiter an"]))

# --- Annehmen --------------------------------------------------------------------------------------------------------
fall("n-erlaubt-windows", "die erlaubte Person verbindet", "aussentest", WIN, {"person": PERSON, "dienst": DIENST},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=False), cv_win(DIENST),
     ["Dienst unter dienst"], ["Die Person verbindet 100-mal nacheinander mit verbinden"],
     erw("annehmen", peer=peer_win(PERSON, None, start=None, programm=None),
         gemessen=["100 von 100 Annahmen mit sid = person und appcontainer_sid null",
                   "pid gleich der Client-PID (GetCurrentProcessId im Client) in 100 von 100",
                   "startzeit und programm null, weil ein Dienst unter NT SERVICE den Prozess der Person nicht öffnen darf"]))
fall("n-erlaubt-gleiches-konto-windows", "Server und Client unter derselben Person, Prozess öffnbar", "aussentest", WIN,
     KW, sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=True), cv_win(PERSON), [],
     ["Die Person verbindet"],
     erw("annehmen", peer=peer_win(PERSON), gemessen=["pid und startzeit gleich den Werten des Clients (GetProcessTimes)"]))
fall("n-container-erlaubt-windows", "Agent im erlaubten AppContainer und im Job des Laufs", "aussentest", WIN,
     {"person": PERSON, "container_a": CONT_A},
     sv_win([{"sid": PERSON, "appcontainer_sid": CONT_A}], prozess_pflicht=True, im_job_pruefen=True), cv_win(PERSON),
     ["Server unter person mit dem Job des Laufs", "Agent in container_a, im Job"], ["Der Agent verbindet"],
     erw("annehmen", peer=peer_win(PERSON, CONT_A, im_job=True),
         gemessen=["sid = person, appcontainer_sid = container_a, im_job true"]))
fall("n-container-ohne-eintrag-windows", "Agent im Container, erlaubt ist nur die Person ohne Container", "aussentest",
     WIN, {"person": PERSON, "container_a": CONT_A},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=True), cv_win(PERSON),
     ["Server unter person; die DACL lässt container_a nicht zu, darum für diesen Fall eine Test-DACL, die container_a zulässt (nur im Testaufbau)"],
     ["Der Agent in container_a verbindet"],
     erw("annehmen", "pk.kennung_nicht_erlaubt", ereignisse=[ab("pk.kennung_nicht_erlaubt", "pk.appcontainer_falsch", peer_win(PERSON, CONT_A))],
         gemessen=["Ereignis mit sid = person und appcontainer_sid = container_a", "Client: pk.abgewiesen"]))
fall("n-anderer-container-windows", "Agent in Container B, erlaubt ist Container A", "aussentest", WIN,
     {"person": PERSON, "container_a": CONT_A, "container_b": CONT_B},
     sv_win([{"sid": PERSON, "appcontainer_sid": CONT_A}]), cv_win(PERSON),
     ["Server unter person, DACL mit container_a"], ["Ein Agent in container_b verbindet"],
     erw("verbinden", "pk.zugriff_verweigert",
         gemessen=["CreateFileW im Client scheitert mit Fehler 5", "kein Ereignis am Server: die Verbindung kommt nicht an"]))
fall("n-job-falsch-windows", "Agent im erlaubten Container, aber außerhalb des Jobs des Laufs", "aussentest", WIN,
     {"person": PERSON, "container_a": CONT_A},
     sv_win([{"sid": PERSON, "appcontainer_sid": CONT_A}], prozess_pflicht=True, im_job_pruefen=True), cv_win(PERSON),
     ["Server mit dem Job des Laufs", "Agent in container_a in einem anderen Job"], ["Der Agent verbindet"],
     erw("annehmen", peer=peer_win(PERSON, CONT_A, im_job=False),
         gemessen=["Die Bibliothek nimmt an und meldet im_job false; die Entscheidung trifft der Aufrufer (Modellweg)"]))
fall("n-fremd-dacl-windows", "ein Konto außerhalb der DACL", "aussentest", WIN,
     {"person": PERSON, "fremd": FREMD, "dienst": DIENST}, sv_win([{"sid": PERSON, "appcontainer_sid": None}]),
     cv_win(DIENST), ["Dienst unter dienst"], ["Das Konto fremd verbindet"],
     erw("verbinden", "pk.zugriff_verweigert", gemessen=["Fehler 5 im Client", "kein Ereignis am Server"]))
fall("n-fremd-unix", "fremde uid an einem Socket mit Modus 0666", "aussentest", UNIX,
     dict(KU, uid_fremd=UID_FREMD, gid_fremd=GID), sv_unix([{"uid": UID}], modus="0666"), cv_unix(UID),
     ["Ordner 0700; der Testaufbau bindet nur den Socket für uid_fremd ein (Einschluss mit eigener Kennung)"],
     ["uid_fremd verbindet"],
     erw("annehmen", "pk.kennung_nicht_erlaubt",
         ereignisse=[ab("pk.kennung_nicht_erlaubt", "pk.uid_nicht_erlaubt", peer_unix("=system", UID_FREMD, GID))],
         gemessen=["Ereignis mit uid = uid_fremd", "Client: pk.abgewiesen", "Server liest außer dem Hallo kein Byte"]))
fall("n-gid-falsch-unix", "richtige uid, falsche gid", "aussentest", UNIX, dict(KU, gid_fremd=GID + 1),
     sv_unix([{"uid": UID, "gid": GID}]), cv_unix(UID), ["Client läuft mit primärer gid gid_fremd"], ["Client verbindet"],
     erw("annehmen", "pk.kennung_nicht_erlaubt",
         ereignisse=[ab("pk.kennung_nicht_erlaubt", "pk.gid_falsch", peer_unix("=system", UID, GID + 1))], gemessen=["Ereignis"]))
fall("n-erlaubt-unix", "erlaubte uid verbindet", "aussentest", UNIX, KU, sv_unix([{"uid": UID}]), cv_unix(UID), [],
     ["Client verbindet 100-mal"],
     erw("annehmen", peer=peer_unix("=system", UID, GID),
         gemessen=["100 von 100 mit uid, gid, pid wie beim Client", "startzeit gleich dem Wert aus /proc/<pid>/stat Feld 22 (Linux) bzw. proc_pidinfo (macOS)"]))
fall("n-remote-windows", "Client von einem anderen Rechner", "aussentest", WIN, {"person": PERSON, "dienst": DIENST},
     sv_win([{"sid": PERSON, "appcontainer_sid": None}]), cv_win(DIENST),
     ["Dienst in der VM", "SMB vom Wirt erreichbar, Konto person auf beiden Seiten bekannt (nur im Testaufbau)"],
     ["Der Wirt öffnet \\\\<vm>\\pipe\\k-beispiel"],
     erw("verbinden", "pk.zugriff_verweigert",
         gemessen=["Der entfernte Client wird abgewiesen (PIPE_REJECT_REMOTE_CLIENTS)", "kein Ereignis am Server"]))
fall("n-hallo-fehlt", "Client verbindet und sendet nichts", "aussentest", ALLE, dict(KW, **KU), None, None,
     ["Server mit hallo_frist_ms 500 und erlaubter Kennung des Clients"], ["Ein roher Client verbindet und wartet 2 s"],
     erw("annehmen", "pk.hallo_ungueltig", ereignisse=[ab("pk.hallo_ungueltig", "pk.hallo_fehlt", None)],
         gemessen=["Ereignis nach 500 ms (Spielraum bis 700 ms)", "Verbindung danach vom Server geschlossen",
                   "peer null unter Windows, weil vor dem ersten Lesen nicht impersoniert wird; unter Unix gemessen"]))
fall("n-hallo-falsch", "Client sendet andere Bytes als das Hallo", "aussentest", ALLE, dict(KW, **KU), None, None,
     ["Server mit erlaubter Kennung des Clients"], ["Ein roher Client sendet GET / und LF"],
     erw("annehmen", "pk.hallo_ungueltig", ereignisse=[ab("pk.hallo_ungueltig", "pk.hallo_falsch", None)],
         gemessen=["Ereignis", "die gesendeten Bytes stehen nicht im Ereignis"]))
fall("n-pid-wiederverwendung-linux", "der Client endet, seine PID gehört danach einem anderen Prozess", "aussentest",
     ["linux"], KU, sv_unix([{"uid": UID}], prozess_pflicht=True), None,
     ["Eigener PID-Namensraum (unshare --user --pid --fork), damit die PID schnell wiederkommt",
      "Der Server hört, ruft annehmen aber erst auf Zeichen des Läufers"],
     ["Client verbindet, sendet das Hallo, reicht den Socket an ein Kind weiter und endet",
      "Der Läufer startet Prozesse, bis einer die alte PID hat", "Dann ruft der Server annehmen"],
     erw("annehmen", "pk.prozess_gewechselt",
         ereignisse=[ab("pk.prozess_gewechselt", "pk.prozess_beendet", peer_unix("linux", UID, GID, pid="=client", start="=neuer_prozess"))],
         gemessen=["Ereignis pk.prozess_gewechselt", "keine Verbindung an den Aufrufer"]))
fall("n-pid-wiederverwendung-windows", "der Client endet vor der Messung, die PID kommt wieder", "aussentest", WIN, KW,
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=True), None,
     ["Server unter person, prozess_pflicht", "Testnaht des Läufers: annehmen wartet nach ConnectNamedPipe auf ein Zeichen"],
     ["Client verbindet, sendet das Hallo, vererbt den Pipe-Handle an ein Kind und endet",
      "Der Läufer startet Prozesse, bis einer die alte PID hat (höchstens 60 s, sonst Fall nicht ausführbar)",
      "Der Server misst"],
     erw("annehmen", "pk.prozess_gewechselt",
         ereignisse=[ab("pk.prozess_gewechselt", "pk.startzeit_weicht_ab", peer_win(PERSON, None, pid="=client", start="=neuer_prozess"))],
         gemessen=["Ereignis pk.prozess_gewechselt", "startzeit des neuen Prozesses liegt nach dem Verbindungszeitpunkt"]))
fall("n-client-tot-mac", "der Client endet vor der Messung", "aussentest", ["mac"], KU,
     sv_unix([{"uid": UID}], prozess_pflicht=True), None, ["annehmen erst auf Zeichen des Läufers"],
     ["Client verbindet, sendet das Hallo, reicht den Socket an ein Kind weiter und endet", "annehmen"],
     erw("annehmen", "pk.prozess_gewechselt",
         ereignisse=[ab("pk.prozess_gewechselt", "pk.pidversion_weicht_ab", peer_unix("macos", UID, GID, pid="=client", start=None, programm=None))],
         gemessen=["Ereignis pk.prozess_gewechselt (pidversion aus LOCAL_PEERTOKEN passt zu keinem lebenden Prozess)"]))
fall("n-instanzen-grenze", "mehr Clients als Instanzen", "aussentest", ALLE, dict(KW, **KU), None, None,
     ["Server mit instanzen 2", "zwei Clients halten je eine Verbindung"], ["ein dritter Client verbindet mit frist_ms 500"],
     erw("verbinden", "pk.nicht_erreichbar",
         gemessen=["dritter Client nach 500 ms (bis 700 ms) mit pk.nicht_erreichbar", "nach dem Ende eines der zwei verbindet ein vierter Client"]))

fall("n-eingeschraenkt-windows", "Prozess der Person mit eingeschränktem Token (Restricting SIDs)", "aussentest", WIN,
     KW, sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=True), cv_win(PERSON),
     ["Server unter person", "Client mit CreateRestrictedToken (Restricting SIDs: person, Logon-SID) gestartet"],
     ["Der eingeschränkte Client verbindet"],
     erw("annehmen", "pk.kennung_nicht_erlaubt",
         ereignisse=[ab("pk.kennung_nicht_erlaubt", "pk.eingeschraenkt", peer_win(PERSON, eingeschraenkt=True))],
         gemessen=["Ereignis mit eingeschraenkt true", "Client: pk.abgewiesen"]))
fall("n-sandbox-mac", "Client unter Seatbelt mit derselben uid", "aussentest", ["mac"], KU, sv_unix([{"uid": UID}]),
     cv_unix(UID), ["Ordner 0700", "Client unter sandbox-exec mit einem Profil, das nur diesen Socket erlaubt"],
     ["Client verbindet"],
     erw("annehmen", peer=peer_unix("macos", UID, GID, signatur="=client", sandbox=True),
         gemessen=["angenommen: die Bibliothek trennt nicht nach Sandbox, sie meldet sie",
                   "sandbox true, signatur gleich der Signatur des Client-Programms (kennung, team_id, gueltig)"]))
fall("n-programm-linux", "Bildpfad des Clients über die pidfd, auch ohne Lesbarkeit", "aussentest", ["linux"], KU,
     sv_unix([{"uid": UID}]), cv_unix(UID), ["zwei Clients desselben Programms, der zweite setzt PR_SET_DUMPABLE 0"],
     ["beide verbinden"],
     erw("annehmen", peer=peer_unix("linux", UID, GID),
         gemessen=["erster Client: programm gleich readlink /proc/<pid>/exe beim Client",
                   "zweiter Client: angenommen, programm null (/proc/<pid>/exe gehört root)"]))
fall("c-zugriffsmaske-windows", "Client im AppContainer öffnet mit der Maske der Bibliothek", "aussentest", WIN,
     {"person": PERSON, "container_a": CONT_A}, sv_win([{"sid": PERSON, "appcontainer_sid": CONT_A}]), cv_win(PERSON),
     ["Server unter person mit Package-SID container_a", "zwei Clients in container_a"],
     ["Client 1 über verbinden (GENERIC_READ | FILE_WRITE_DATA)", "Client 2 roh mit GENERIC_READ | GENERIC_WRITE"],
     erw("verbinden", peer=peer_win(PERSON, None, pid="=server", start=None, programm=None),
         gemessen=["Client 1 verbindet und schreibt", "Client 2 scheitert mit Fehler 5 (GENERIC_WRITE enthält FILE_CREATE_PIPE_INSTANCE)",
                   "ohne das Label LW scheitert auch Client 1 mit Fehler 5 (Gegenprobe im Testaufbau)"]))

fall("n-haken-lehnt-ab", "der Haken des Aufrufers lehnt einen erlaubten Peer ab", "aussentest", ALLE, dict(KW, **KU),
     None, None, ["Server, dessen Liste den Client erlaubt; der Haken lehnt mit aufrufer:sitzung-fremd ab",
                  "Mitschnitt der Bytes am Client (Testnaht des Läufers)"],
     ["Client verbindet"],
     erw("annehmen", "pk.aufrufer_abgewiesen", ereignisse=[ab("pk.aufrufer_abgewiesen", "aufrufer:sitzung-fremd", None)],
         gemessen=["Ereignis mit grund aufrufer:sitzung-fremd und dem gemessenen Peer (im Fall nicht festgelegt, darum null)",
                   "Client: pk.abgewiesen, 0 Byte vom Server empfangen, auch kein Hallo",
                   "Gegenprobe: derselbe Aufbau mit bestaetigen() liefert das Hallo und die Verbindung"]))
fall("n-haken-bindung-windows", "die Prozessbindung gilt während des Hakens", "aussentest", WIN, KW,
     sv_win([{"sid": PERSON, "appcontainer_sid": None}], prozess_pflicht=True), cv_win(PERSON),
     ["Server unter person; der Haken wartet auf ein Zeichen des Läufers"],
     ["Client verbindet und endet, während der Haken wartet",
      "Der Läufer startet 60 s lang Prozesse und vergleicht ihre pid mit prozess().pid",
      "Der Haken fragt einen Stand-in des Plattformdienstes mit pid und startzeit"],
     erw("annehmen", peer=peer_win(PERSON),
         gemessen=["kein neuer Prozess bekommt die pid, solange Vorlaeufig lebt",
                   "der Stand-in findet den Prozess mit derselben startzeit (beendet, aber nicht wiederverwendet)",
                   "nach ablehnen oder bestaetigen ist das Handle zu"]))

# --- Verbinden (Gegenrichtung) ---------------------------------------------------------------------------------------
fall("c-dienst-am-besitzer-windows", "Client bestätigt einen Dienst unter NT SERVICE am Besitzer der Instanz", "aussentest",
     WIN, {"person": PERSON, "dienst": DIENST}, sv_win([{"sid": PERSON, "appcontainer_sid": None}]), cv_win(DIENST),
     ["Dienst unter dienst ohne Leserecht der Person auf seinen Prozess"], ["Die Person verbindet"],
     erw("verbinden", peer=peer_win(DIENST, None, pid="=server", start=None, programm=None),
         gemessen=["Besitzer der Instanz = dienst (GetSecurityInfo, OWNER)", "OpenProcess auf die Server-PID wird nicht verlangt"]))
fall("c-falscher-server-windows", "die Person belegt den Namen, der Client erwartet den Dienst", "aussentest", WIN,
     {"person": PERSON, "dienst": DIENST}, None, cv_win(DIENST),
     ["Ein Prozess der Person hält \\\\.\\pipe\\k-beispiel und schreibt jedes empfangene Byte mit"], ["Client verbindet"],
     erw("verbinden", "pk.falscher_server",
         gemessen=["Rückgabe pk.falscher_server", "der Vorbeleger hat 0 Byte empfangen, auch kein Hallo"]))
fall("c-falscher-server-unix", "am Socket hört eine andere uid", "aussentest", UNIX, dict(KU, uid_fremd=UID_FREMD), None,
     cv_unix(UID), ["Ein Prozess unter uid_fremd hört am Socket und schreibt jedes Byte mit"], ["Client verbindet"],
     erw("verbinden", "pk.falscher_server", gemessen=["Rückgabe pk.falscher_server", "0 Byte beim Hörer"]))
fall("c-abgewiesen", "der Server weist den Client ab", "aussentest", ALLE, dict(KW, **KU), None, None,
     ["Server, dessen Liste den Client nicht enthält, das Betriebssystem lässt ihn aber durch (wie n-fremd-unix, n-container-ohne-eintrag-windows)"],
     ["Client verbindet"],
     erw("verbinden", "pk.abgewiesen", gemessen=["Client: pk.abgewiesen nach dem Hallo, ohne Nutzdaten"]))
fall("c-nicht-erreichbar", "kein Server unter dem Namen", "aussentest", ALLE, dict(KW, **KU), None, None, [],
     ["Client verbindet mit frist_ms 300"],
     erw("verbinden", "pk.nicht_erreichbar", gemessen=["Rückgabe spätestens nach 500 ms"]))
fall("c-ordner-fremd-unix", "der Socket-Ordner des Clients gehört einer anderen uid als erwartet", "aussentest", UNIX,
     dict(KU, uid_fremd=UID_FREMD), None, cv_unix(UID),
     ["unix_ordner gehört uid_fremd, darin hört ein Prozess unter uid_fremd"], ["Client verbindet"],
     erw("verbinden", "pk.falscher_server", gemessen=["0 Byte beim Hörer"]))
fall("d-daten-erst-nach-pruefung", "Nutzdaten fließen erst nach beiden Prüfungen", "aussentest", ALLE, dict(KW, **KU),
     None, None, ["Server und Client erlaubt", "Mitschnitt der Bytes auf beiden Seiten (Testnaht des Läufers)"],
     ["Client schreibt 5 Byte Nutzdaten direkt nach verbinden"],
     erw("verbinden", gemessen=["Byte 1 bis 4 vom Client: pk1 und LF; Byte 1 bis 4 vom Server: pk1 und LF",
                                "die Nutzdaten kommen erst nach der Antwort des Servers an"]))


def main():
    for ordner in ("faelle", "erwartet"):
        ziel = HIER / ordner
        if ziel.exists():
            shutil.rmtree(ziel)
        ziel.mkdir()
    for name, (f, e) in sorted(FAELLE.items()):
        for ordner, obj in (("faelle", f), ("erwartet", e)):
            # "=system" steht für das System des Laufs (linux oder macos); der Außenläufer setzt es ein.
            text = json.dumps(obj, ensure_ascii=False, indent=1) + "\n"
            (HIER / ordner / f"{name}.json").write_text(text, encoding="utf-8")
    print(f"{len(FAELLE)} Fälle geschrieben")


if __name__ == "__main__":
    main()

#!/usr/bin/python3 -I
"""Reference of the `dienst` contract (the contract text). Standard library only.

  dienst-ref <startdatei.json>          run the service (a real process: Unix socket, PTY, one child)
  dienst-ref --kind <name> [argumente]   fixed child programs of the tests (section 7 of the contract text)
  dienst-ref --gegenproben               print the mutant names, one per line

The reference puts the other contracts together: `zk_ref.Kern` is the state core, `ra_ref.Verbindung` is the extract of each
viewer, and `Traeger` below is a small stand-in for the carrier (`alacritty_terminal`): it understands only what the fixed child
programs write (plain text, CR/LF, SGR, a few cursor and erase sequences, three private modes, OSC 0/2/52). The contract of the
carrier (`traeger`) is checked on its own; here the stand-in is compared with the Rust reference of that contract on every
child output of the case sets (stichprobe.py).
REF_GEGENPROBE=<name> switches on one mutant from the catalogue of the contract text.
"""
from __future__ import annotations

import base64
import codecs
import collections
import copy
import errno
import hashlib
import fcntl
import json
import os
import pwd
import stat as stat_mod
import re
import selectors
import shutil
import signal
import socket
import stat
import struct
import sys
import termios
import time

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)
import ra_ref  # noqa: E402
import zk_ref  # noqa: E402

MUT = os.environ.get("REF_GEGENPROBE", "")

MUTANTEN = [
    # Nachtrag 26.11.2026: DI-L1, DI-L3, DI-L5, DI-L11
    "worker_orchestrator_ungeprueft", "worker_fremder_kunde", "agenten_vom_raumkern",
    "umbenennen_ohne_pruefung", "loeschen_laufend", "loeschen_ohne_bestaetigung", "loeschen_kennung_wieder",
    "fortsetzen_laufend", "fortsetzen_ohne_verweis", "wiederbeleben_ohne_absturz",
    "liste_ohne_ordner", "projekt_ganzer_pfad", "bestand_vergessen", "bestand_laeuft_bleibt",
    "konto_aus_ereignis", "lokal_ohne_peerpruefung", "lokal_ohne_starten",
    # Nachtrag 27.11.2026: Loeschen with the conversation, Firmenvorgabe at the start
    "loeschen_gespraech_bleibt", "loeschen_folgt_verweis", "loeschen_gespraeche_ganz", "loeschen_fortgesetzt_gespraech",
    "vorgabe_start_ungeprueft", "vorgabe_neu_geladen",
    # Nachtrag 27.11.2026: the list of events of a connection (DI-E5)
    "ereignisliste_fehlt", "ereignisliste_nicht_zuerst", "ereignisliste_wiederholt", "ereignisliste_mit_abgelehnter",
    "ereignisliste_ohne_angenommene", "ereignisliste_ohne_lauf_arten", "ereignisliste_je_dienst", "ereignisliste_unsortiert",
    "freigabe_ueber_bruecke", "freigabe_grund_falsch", "freigabe_lokal_ohne_stelle",
    # Nachtrag 27.11.2026: a continuation that lauf ends before any child leaves the old session free (DI-L3 point 2)
    "fortgesetzt_bleibt_ohne_kind",
    # Nachtrag 27.11.2026, DI12 Punkt 15: a conversation under lauf.gespraeche takes its number
    "gespraeche_vergabe_ungeprueft",
    # Nachtrag 27.11.2026, DI-L3 point 2: a session of version 1 is nicht_fortsetzbar
    "fortsetzen_fassung1_ok",
    # DI1 Startdatei
    "start_unbekannter_schluessel_ok", "start_groesse_1001_ok", "start_ruecklauf_ohne_grenze", "start_mehrere_sitzungen_ok",
    "start_relativer_pfad_ok", "start_datei_ohne_grenze", "start_socketname_pfad_ok", "start_traegerfeld_ok",
    # DI2 Rechte
    "rechte_modus_nicht_geprueft", "rechte_0750_ok", "rechte_ordner_anlegen", "rechte_link_folgen",
    "rechte_socket_ersetzen", "rechte_socket_modus_umask", "rechte_besitzer_nicht_geprueft", "rechte_tcp_hoerer",
    "rechte_socket_bleibt",
    # DI3 Protokoll
    "proto_laenge_ohne_grenze", "proto_laenge_null_ok", "proto_unbekannte_art_ignoriert", "proto_json_fehler_bleibt",
    "proto_t_vom_client", "proto_betrachter_vom_client", "proto_traegerereignis_ok", "proto_verbinden_ok",
    "proto_pruef_ohne_modus", "proto_fehler_ohne_trennen", "proto_ablehnung_verschluckt", "proto_ablehnung_an_alle",
    "proto_abgeschnitten_absturz",
    # DI4 Betrachter und Rueckstau
    "betr_name_wiederverwenden", "betr_trennen_nicht_gemeldet", "betr_deflate_ignoriert", "betr_nr_eins",
    "stau_nicht_gemeldet", "stau_ohne_kopf", "stau_stumm_ignoriert", "stau_alles_blockiert", "stau_warteschlange_verwerfen",
    "stau_bericht_nur_wachsen", "stau_ohne_leerbericht",
    # DI5 Fehler im Kern
    "kernfehler_ohne_neubau", "kernfehler_kein_abo", "kernfehler_leerer_schirm", "kernfehler_betrachter_verloren",
    # DI6 Uhr
    "uhr_ignoriert", "uhr_rueckwaerts_ok", "uhr_ohne_takt", "uhr_echtzeit", "monoton_ohne_takt",
    # DI7 Kind
    "kind_code_plus_eins", "kind_signal_roh", "kind_env_erweitert", "kind_cwd_nicht_gesetzt", "pty_vor_groesse",
    "groesse_pty_nicht", "eingabe_cr_zu_lf", "kind_bleibt_am_leben",
    # DI4, Nachtrag 21.11.2026: die Ereignisse des Traegers fuer eine neue Groesse gehen nach den Nachrichten des Schritts an den Kern
    "groesse_ereignis_sofort",
    # DI1 / DI8, Nachtrag 20.11.: kleinste Groesse 2 Spalten (decision of 20.11.2026)
    "start_eine_spalte_ok", "groesse_eine_spalte_durchlassen",
    # DI1 / DI5, Nachtrag 20.11.: Startdatei warteschlange_max
    "start_wmax_ohne_pruefmodus_ok", "start_wmax_ueber_grenze_ok",
    # DI5, Nachtrag 20.11.: harte Obergrenze der Warteschlange (Entscheidung vom 19.11.2026 F4)
    "stau_grenze_fehlt", "stau_grenze_ein_byte_frueh", "stau_grenze_ein_byte_spaet", "stau_grenze_nur_pruefmodus",
    "stau_grenze_vorgabe_klein", "stau_grenze_ohne_trennen", "stau_grenze_kein_protokoll", "stau_grenze_alle_trennen",
    "stau_neuverbinden_ohne_abzug",
    # DI11, Nachtrag 20.11.: Cursor (Zug 0c, decision of 19.11.2026 F2)
    "anfang_cursor_fehlt", "anfang_cursor_unsichtbar", "start_cursorfeld_ok", "kernfehler_cursor_anfang", "kernfehler_cursor_fehlt",
    "traeger_cursor_fehlt", "traeger_cursor_nur_zeilen", "traeger_cursor_sichtbar_fehlt", "traeger_cursor_umbruch_sofort",
    "traeger_groesse_zeilen_oben",
    # Entscheidung vom 20.11.2026: Neubau aus dem Stand des Traegers (Zeilen und Cursor zum selben Zeitpunkt); Protokollzeile ohne Inhalte der Sitzung
    "kernfehler_cursor_aus_zustand", "stau_grenze_protokoll_mit_inhalt",
    # DI12, Nachtrag 20.11.: ein Dienst, mehrere Sitzungen (Entscheidung vom 20.11.2026 (a))
    "lauf_starten_ohne_lauf_ablehnung", "lauf_unbekannter_schluessel_ok", "lauf_programm_relativ_ok", "lauf_eine_spalte_ok",
    "lauf_kind_id_kollision_ok", "lauf_groesse_ohne_kind_ok",
    "starten_form_nicht_objekt_ok", "starten_gross_ok", "starten_grenze_frueh", "starten_fassung_nicht_geprueft",
    "starten_fassung_bool_ok", "starten_sitzung_ok", "starten_fremdfeld_ok",
    "starten_datei_modus_offen", "starten_ordner_modus_offen", "starten_argv_zusatz", "starten_env_erweitert",
    "starten_groesse_vom_kind", "starten_cwd_ordner", "starten_ordner_ersetzen", "starten_datei_ohne_sitzung",
    "sitzung_id_wieder", "sitzung_neunte_ok", "sitzung_achte_abgelehnt", "sitzung_beendete_zaehlen", "sitzung_nr_null",
    "sitzung_nr_zu_gross_ok", "starten_fehler_trotzdem_gestartet", "starten_nr_verbraucht",
    "gestartet_an_alle", "gestartet_vor_liste",
    "liste_nicht_nach_aenderung", "liste_ohne_code", "liste_felder_leer", "liste_nur_sender", "liste_beim_verbinden_fehlt",
    "beenden_ohne_signal", "beenden_kein_sigkill", "beenden_entfernt_sitzung", "beenden_unbekannt_still", "beenden_beendete_ok",
    "eingabe_an_alle_sitzungen", "abo_immer_erste_sitzung", "lauf_raumwechsel_ok",
    "ende_kinder_bleiben", "ende_ordner_bleiben", "lauf_kernfehler_ok",
    # Nachtrag vom 20.11.2026: lauf.einstellungen (Pfad der Einstellungsdatei, nur aus der Startdatei)
    "lauf_einstellungen_nicht_pflicht", "lauf_einstellungen_relativ_ok", "starten_einst_nicht_gelesen",
    # Nachtrag vom 20.11.2026: Ablehnung mit code und text, Nachricht Startwahl
    "ablehnung_ohne_code", "ablehnung_ohne_text", "startwahl_fehlt", "startwahl_nicht_beim_verbinden", "startwahl_nicht_nach_aenderung",
    "startwahl_nur_sender", "startwahl_mit_adresse", "startwahl_ohne_ohne_kunde", "startwahl_harness_falsch", "startwahl_trotz_unlesbar",
    "startwahl_routen_sortiert",
    # Nachtrag vom 20.11.2026: Pruefung der Start-Struktur nach den Stufen von Vertrag 1 (Crate `start`), `modell` der Route in der Startwahl
    "stufe_04_marker_ok", "stufe_05_unbekannt_ok", "stufe_06_fehlt_ok", "stufe_07_typ_ok",
    "stufe_09_kennung_ok", "stufe_09_kennung_laenge_ok", "stufe_09_kennung_gross_ok",
    "stufe_10_modell_ok", "stufe_10_modell_laenge_ok", "stufe_10_modell_leer_ok", "stufe_10_modell_zeichen_ok",
    "stufe_11_ordner_ok", "stufe_11_ordner_relativ_ok", "stufe_11_ordner_laenge_ok", "stufe_11_ordner_punkt_ok", "stufe_11_ordner_c1_ok",
    "stufe_11_ordner_doppel_ok", "stufe_11_ordner_ende_ok",
    "stufe_13_harness_ok", "stufe_13_copilot_unbekannt", "stufe_14_route_ok", "stufe_15_modell_ok", "stufe_15_cloud_ignoriert",
    "stufe_16_kunde_ok", "stufe_16_null_unbekannt", "stufe_17_art_ok", "stufe_17_copilot_lokal",
    "stufen_hoechste_zuerst", "stufen_14_bis_17_umgekehrt", "stufen_14_bis_17_fehlen", "einst_vor_form",
    "startwahl_ohne_modell", "startwahl_modell_leer", "startwahl_modell_ignoriert", "startwahl_modell_fassung1_erfunden", "startwahl_ohne_copilot",
    "start_grund_code", "start_code_ohne_praefix", "start_text_form",
    # Nachtrag vom 21.11.2026: das Sitzungsende raeumt auf (DI12 Punkt 10)
    "ende_nur_bei_normalem_ende", "ende_sitzung_ohne_loeschen", "ende_loeschen_folgt_link", "ende_signal_ohne_leiter",
    "ende_signal_ohne_startzeit", "ende_signal_nach_loeschen", "ende_signal_link_folgen", "ende_signal_ohne_zeilenende",
    "ende_dienstende_ohne_signal", "ende_signal_keine", "starten_datei_name",
    # Nachtrag vom 21.11.2026: keine Kennung, deren Ordner unter lauf.sitzungen noch besteht (DI12 Punkt 13)
    "sitzungen_pruefung_fehlt", "sitzungen_stat_statt_lstat", "sitzungen_nur_ordner", "sitzungen_fehler_gilt_frei",
    "sitzungen_socket_ordner_geprueft", "sitzungen_nummer_max_nicht_beim_ueberspringen", "sitzungen_relativ_ok",
    "sitzungen_naechste_beim_ueberspringen",
    # Nachtrag vom 21.11.2026 (Nachtrag 1): lauf.marker_datei, einmal beim Start gelesen, der Dienst schreibt den Marker in jede start.json (DI12 Punkt 14)
    "marker_ordner_ungeprueft", "marker_ordner_nofollow_fehlt", "marker_datei_nofollow_fehlt", "marker_modus_ungeprueft", "marker_groesse_ungeprueft",
    "marker_fehlt_ist_fehler", "marker_je_start_neu_gelesen", "marker_nie_gesetzt", "marker_nur_erste_sitzung", "marker_ohne_datei",
    "marker_anfrage_ersetzt", "marker_form_uebersprungen", "marker_siebzehn_ok", "marker_komma_ok",
    # Nachtrag 2: der Zaehlordner kommt aus lauf.marker_ordner (Startdatei), aus der Marker-Datei nur die Liste
    "marker_ordner_aus_datei", "marker_ordner_fehlt_uebergangen",
    # eine FIFO an der Stelle der Marker-Datei darf den Start nicht anhalten (O_NONBLOCK)
    "marker_nonblock_fehlt",
    # Nachtrag 3 (Entscheidung vom 21.11.2026): jeder Fehlerfall prueft nur seine eigene Form
    "marker_liste_kein_array_ok", "marker_liste_leer_ok",
    # Nachtrag vom 21.11.2026: keine Kennung, deren Zielliste unter lauf.ziellisten noch liegt (DI12 Punkt 15)
    "ziellisten_pruefung_fehlt", "ziellisten_stat_statt_lstat", "ziellisten_falscher_name", "ziellisten_fehler_gilt_frei",
    "ziellisten_nur_regulaer", "ziellisten_ohne_pfadpruefung",
    # Nachtrag 2 (Entscheidung vom 21.11.2026): jede Datei der Nummer belegt sie (Sitzungsordner, Socket-Ordner lauf.laufzeit, Zielliste, die zwei Zaehldateien in lauf.marker_ordner)
    "laufzeit_nicht_geprueft", "zaehldatei_nur_suche", "zaehldatei_ohne_marker_ordner", "eintrag_fehlt_ist_frei", "laufzeit_ohne_pfadpruefung",
]

TASTE_START = b"g"
KLEINSTE_SPALTEN = 2          # Entscheidung vom 20.11.2026: die eine Stelle, an der die kleinste Spaltenzahl steht (Startdatei, Ereignisse, Traeger-Modell)
WARTESCHLANGE_MAX = 33554432  # Entscheidung vom 19.11.2026 F4: harte Obergrenze je Betrachter (Byte der Warteschlange)
MAX_HINEIN = 1048576
MAX_START = 1048576
SOCKET_PFAD_MAX = 100

A_EREIGNIS, A_UHR, A_STUMM, A_ZUSTAND, A_KERNFEHLER = 0x01, 0x10, 0x11, 0x12, 0x13
A_RASTER, A_NACHRICHT, A_ABLEHNUNG, A_FEHLER = 0x05, 0x06, 0x07, 0x08
ERLAUBT = {"Abo", "Groesse", "Text", "Taste", "Einfuegen", "Antwort", "Freigabe", "RouteWaehlen", "EinstellungSetzen", "RaumWechsel",
           "OrdnerLesen", "InhaltSuchen"}        # OrdnerLesen, InhaltSuchen: Nachtrag 26.11.2026 (L9)
EIGENE_LAUF = ("Starten", "Beenden", "Umbenennen", "Loeschen", "Fortsetzen", "Wiederbeleben")
# Nachtrag 27.11.2026 (DI-E5): the catalogue of all modes in the order of the contract (DI3, then the events of the service in the run)
KATALOG = ("Abo", "Groesse", "Text", "Taste", "Einfuegen", "Antwort", "Freigabe", "RouteWaehlen", "EinstellungSetzen", "RaumWechsel",
           "OrdnerLesen", "InhaltSuchen") + EIGENE_LAUF
L9_GRENZE_PFAD, L9_GRENZE_GLIEDER, L9_GRENZE_MUSTER = 4096, 64, 256
TOP_SCHLUESSEL = {"socket", "betrachter", "kind", "groesse", "ruecklauf", "deflate", "pruefmodus", "anfang", "warteschlange_max", "lauf",
                  "lokal"}       # lokal: Nachtrag 26.11.2026, DI-L11
LAUF_SCHLUESSEL = {"programm", "env", "groesse", "einstellungen"}      # einstellungen: addendum of 20.11.2026, Pflicht
START_MAX = 4096              # Nachtrag vom 20.11.2026: die Start-Struktur hat hoechstens 4096 Byte (kanonische Form, vor dem Einsetzen von `sitzung`)
SITZUNGEN_MAX = 8             # gleichzeitig laufende Sitzungen (decision of 20.11.2026 (a))
NUMMER_MAX = 255              # die Sitzungsnummer im Rasterrahmen ist ein Byte (RA): die Sitzung aus `kind` hat 0, die Sitzungen von Starten teilen sich 1 bis 255
                              # (Nachtrag P25, Abschnitt 19: die Rasternummer ist von der Kennung getrennt und wird neu vergeben; im Pruefmodus lauf.rahmen_max)
KENNUNG_MAX = 999_999_999     # Nachtrag P25: die meisten Kennungen s-<n> je Lauf (neun Ziffern, ^s-[0-9]{1,9}$); eine Kennung wird nie wiederverwendet
KILL_FRIST = 1.0              # Beenden und Dienstende: SIGHUP, bis zu 1 s, SIGKILL
START_EREIGNIS = {"art", "start"}
START_WORKER = {"orchestrator", "rolle"}       # Nachtrag 26.11.2026, DI-L1: the keys of Starten beside `start`
FELD_VERBOTEN = ("konto", "wer", "person", "uid", "sid")     # Nachtrag 26.11.2026, DI-L11 point 7: never an account from the view
KENNUNG = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
BESTAND_MAX = 4194304          # Nachtrag 26.11.2026, DI-L5: the largest Bestand file
HALLO = b"pk1\n"               # peerkanal, Fassung 1: the hello of the local way (DI-L11)
HALLO_FRIST = 2.0
LOKAL_INSTANZEN = 8
# addendum of 20.11.2026: the harness table of version 1 (contract 1) and the fixed texts of the refusals of Starten
HARNESS_TABELLE = [{"name": "claude", "art": "cloud", "routenart": "cloud"}, {"name": "copilot", "art": "cloud", "routenart": "cloud"},
                   {"name": "pi", "art": "lokal", "routenart": "lokal"},
                   {"name": "opencode", "art": "lokal", "routenart": "lokal"}]       # Nachtrag 27.11.2026 (Projektentscheid): opencode, the third harness, local
HARNESS_ART = {h["name"]: h["routenart"] for h in HARNESS_TABELLE}
TEXTE = {"start_form": "Start hat falsche Form", "einst_unlesbar": "Einstellungsdatei nicht lesbar", "zu_viele_sitzungen": "Zu viele Sitzungen",
         "nummern_erschoepft": "Keine Sitzungsnummer frei", "start_ordner": "Ordner der Sitzung nicht anlegbar", "programm_nicht_startbar": "Programm nicht startbar",
         # Nachtrag 26.11.2026 (DI-L1, DI-L3)
         "orchestrator_unbekannt": "Orchestrator unbekannt", "orchestrator_beendet": "Orchestrator ist beendet",
         "orchestrator_ist_worker": "Orchestrator ist selbst ein Worker", "orchestrator_fremder_kunde": "Kunde weicht vom Orchestrator ab",
         "fortsetzen_form": "Fortsetzen hat falsche Form", "wiederbeleben_form": "Wiederbeleben hat falsche Form",
         "sitzung_unbekannt": "Sitzung unbekannt", "sitzung_laeuft": "Sitzung läuft", "schon_fortgesetzt": "Sitzung ist schon fortgesetzt",
         "nicht_fortsetzbar": "Sitzung lässt sich nicht fortsetzen", "nicht_abgestuerzt": "Sitzung ist nicht abgestürzt"}
FILTER_ENV = ("CPATH", "LIBRARY_PATH", "MANPATH", "SDKROOT", "__CF_USER_TEXT_ENCODING")


KEIN = object()
OSC_ENDE = re.compile("[\x07\x1b]")


def _konstante(_):
    raise ValueError("NaN oder Infinity")


def tiefe(o):
    d, stack = 0, [(o, 1)]
    while stack:
        x, n = stack.pop()
        d = max(d, n)
        if isinstance(x, dict):
            stack += [(v, n + 1) for v in x.values()]
        elif isinstance(x, list):
            stack += [(v, n + 1) for v in x]
    return d


def m(name):
    return MUT == name


class StartFehler(Exception):
    def __init__(self, code, text):
        Exception.__init__(self, text)
        self.code = code
        self.text = text


def ganz(x):
    return isinstance(x, int) and not isinstance(x, bool)


def env_ok(env):
    return isinstance(env, dict) and len(env) <= 256 and all(isinstance(a, str) and isinstance(v, str) and a and "=" not in a and "\x00" not in a + v
                                                             for a, v in env.items())


def spalten_ok(sp):
    """The one place of the smallest column count (decision of 20.11.2026)."""
    return sp >= KLEINSTE_SPALTEN


# ---------------------------------------------------------------------------------------------- Startdatei (DI1)

def start_lesen(pfad):
    try:
        st = os.stat(pfad)
        if st.st_size > MAX_START and not m("start_datei_ohne_grenze"):
            raise StartFehler(2, "Startdatei ist groesser als 1048576 Byte")
        with open(pfad, "rb") as f:
            roh = f.read()
        obj = json.loads(roh.decode("utf-8"), parse_constant=_konstante)
        if tiefe(obj) > 128:
            raise StartFehler(2, "Startdatei ist tiefer als 128 Ebenen verschachtelt")
    except StartFehler:
        raise
    except (OSError, ValueError, RecursionError) as e:
        raise StartFehler(2, "Startdatei nicht lesbar: %s" % type(e).__name__)
    return start_pruefen(obj)


def start_pruefen(o):
    def bad(t):
        raise StartFehler(2, "Startdatei ungueltig: " + t)

    if not isinstance(o, dict):
        bad("kein Objekt")
    if not m("start_unbekannter_schluessel_ok"):
        for k in o:
            if k not in TOP_SCHLUESSEL:
                bad("unbekannter Schluessel " + k)
    lauf_da = "lauf" in o
    for k in ("socket", "betrachter", "anfang") + (() if lauf_da else ("kind", "groesse")):
        if k not in o:
            bad("Feld fehlt: " + k)
    if lauf_da and ("kind" in o) != ("groesse" in o) and not (m("lauf_groesse_ohne_kind_ok") and "groesse" in o):
        bad("kind und groesse gehoeren zusammen (beide oder keines, keines nur mit lauf)")      # DI12
    lauf = None
    if lauf_da:
        L = o["lauf"]
        if not isinstance(L, dict):
            bad("lauf")
        fremd = set(L) - LAUF_SCHLUESSEL - {"nummer_max", "rahmen_max", "sitzungen", "ziellisten", "laufzeit", "marker_datei", "marker_ordner",
                                            "bestand", "stelle", "gespraeche", "firmenvorgabe", "vorgabe_helfer"}
        pflicht = LAUF_SCHLUESSEL - ({"einstellungen"} if m("lauf_einstellungen_nicht_pflicht") else set())
        if (fremd and not m("lauf_unbekannter_schluessel_ok")) or not pflicht <= set(L):
            bad("lauf")
        if "einstellungen" in L:
            es = L["einstellungen"]
            if not (isinstance(es, str) and 0 < len(es.encode()) <= 4096 and "\x00" not in es
                    and (es.startswith("/") or m("lauf_einstellungen_relativ_ok"))):
                bad("lauf.einstellungen muss ein absoluter Pfad sein")
        if "sitzungen" in L:       # Nachtrag vom 21.11.2026: wie lauf.einstellungen (DI12 Punkt 13)
            sz = L["sitzungen"]
            if not (isinstance(sz, str) and 0 < len(sz.encode()) <= 4096 and "\x00" not in sz
                    and (sz.startswith("/") or m("sitzungen_relativ_ok"))):
                bad("lauf.sitzungen muss ein absoluter Pfad sein")
        if "bestand" in L:         # Nachtrag 26.11.2026, DI-L5 point 3: like lauf.einstellungen
            bs = L["bestand"]
            if not (isinstance(bs, str) and 0 < len(bs.encode()) <= 4096 and "\x00" not in bs and bs.startswith("/")):
                bad("lauf.bestand muss ein absoluter Pfad sein")
        for k_ in ("gespraeche", "firmenvorgabe", "vorgabe_helfer"):      # Nachtrag 27.11.2026: like lauf.bestand
            if k_ in L:
                v_ = L[k_]
                if not (isinstance(v_, str) and 0 < len(v_.encode()) <= 4096 and "\x00" not in v_ and v_.startswith("/")):
                    bad("lauf.%s muss ein absoluter Pfad sein" % k_)
        if "stelle" in L:          # Nachtrag 26.11.2026, DI-L6 point 2: the form only (the reference has no channel to a decision point)
            st_ = L["stelle"]
            # Nachtrag 27.11.2026, Kanal zur Stelle point 2: the only form carries `erwartet` (Unix: ordner and uid)
            er_ = st_.get("erwartet") if isinstance(st_, dict) else None
            if not (isinstance(st_, dict) and set(st_) <= {"ordner", "erwartet", "nachweis"} and isinstance(st_.get("ordner"), str)
                    and st_["ordner"].startswith("/") and isinstance(er_, dict) and set(er_) == {"uid"}
                    and isinstance(er_["uid"], int) and not isinstance(er_["uid"], bool) and er_["uid"] >= 0):
                bad("lauf.stelle")
        global MARKER_DIENST, MARKER_PFAD
        MARKER_DIENST, MARKER_PFAD = None, None
        if "laufzeit" in L:        # Nachtrag 2 (Entscheidung vom 21.11.2026): wie lauf.sitzungen
            lz_ = L["laufzeit"]
            if not m("laufzeit_ohne_pfadpruefung") and not (isinstance(lz_, str) and 0 < len(lz_.encode()) <= 4096 and "\x00" not in lz_ and lz_.startswith("/")):
                bad("lauf.laufzeit muss ein absoluter Pfad sein")
        if "ziellisten" in L:      # Nachtrag vom 21.11.2026: wie lauf.sitzungen
            zl_ = L["ziellisten"]
            if not m("ziellisten_ohne_pfadpruefung") and not (isinstance(zl_, str) and 0 < len(zl_.encode()) <= 4096 and "\x00" not in zl_ and zl_.startswith("/")):
                bad("lauf.ziellisten muss ein absoluter Pfad sein")
        if "marker_ordner" in L:   # Nachtrag 2: wie lauf.einstellungen; der Dienst legt den Ordner nicht an und prueft ihn nicht
            mo = L["marker_ordner"]
            if not (isinstance(mo, str) and 0 < len(mo.encode()) <= 4096 and "\x00" not in mo and mo.startswith("/")):
                bad("lauf.marker_ordner muss ein absoluter Pfad sein")
        if "marker_datei" in L:    # Nachtrag vom 21.11.2026: wie lauf.einstellungen; die Datei wird genau einmal gelesen, hier (vor dem Socket)
            mp = L["marker_datei"]
            if not (isinstance(mp, str) and 0 < len(mp.encode()) <= 4096 and "\x00" not in mp and mp.startswith("/")):
                bad("lauf.marker_datei muss ein absoluter Pfad sein")
            MARKER_PFAD = mp
            MARKER_DIENST = marker_datei_lesen(mp, L.get("marker_ordner"))
        if "nummer_max" in L:      # nur zum Pruefen, wie warteschlange_max: senkt die Zahl der Kennungen (Sitzungen von Starten insgesamt; P25: bis KENNUNG_MAX)
            nm = L["nummer_max"]
            if not (ganz(nm) and 1 <= nm <= KENNUNG_MAX and o.get("pruefmodus") is True):
                bad("lauf.nummer_max")
        if "rahmen_max" in L:      # Nachtrag P25: nur zum Pruefen, senkt die Zahl der Rasternummern der Sitzungen von Starten (Vorgabe NUMMER_MAX)
            rm = L["rahmen_max"]
            if not (ganz(rm) and 1 <= rm <= NUMMER_MAX and o.get("pruefmodus") is True):
                bad("lauf.rahmen_max")
        pg = L["programm"]
        if not (isinstance(pg, str) and 0 < len(pg.encode()) <= 4096 and "\x00" not in pg):
            bad("lauf.programm")
        if not pg.startswith("/") and not m("lauf_programm_relativ_ok"):
            bad("lauf.programm muss ein absoluter Pfad sein")
        if not env_ok(L["env"]):
            bad("lauf.env")
        lg = L["groesse"]
        if not (isinstance(lg, list) and len(lg) == 2 and all(ganz(x) and 1 <= x <= 1000 for x in lg)):
            bad("lauf.groesse")
        if not spalten_ok(lg[0]) and not m("lauf_eine_spalte_ok"):
            bad("lauf.groesse hat weniger als %d Spalten" % KLEINSTE_SPALTEN)
        lauf = {"programm": pg, "env": dict(L["env"]), "groesse": list(lg), "einstellungen": L.get("einstellungen"), **({"nummer_max": L["nummer_max"]} if "nummer_max" in L else {}),
                **({"rahmen_max": L["rahmen_max"]} if "rahmen_max" in L else {}),
                **({"sitzungen": L["sitzungen"]} if "sitzungen" in L else {}),
                **({"ziellisten": L["ziellisten"]} if "ziellisten" in L else {}),
                **({"laufzeit": L["laufzeit"]} if "laufzeit" in L else {}),
                **({"marker_datei": L["marker_datei"]} if "marker_datei" in L else {}),
                **({"marker_ordner": L["marker_ordner"]} if "marker_ordner" in L else {}),
                **({"bestand": L["bestand"]} if "bestand" in L else {}),
                **({"stelle": L["stelle"]} if "stelle" in L else {}),
                **{k_: L[k_] for k_ in ("gespraeche", "firmenvorgabe", "vorgabe_helfer") if k_ in L}}
    s = o["socket"]
    if not (isinstance(s, dict) and set(s) <= {"ordner", "name"} and isinstance(s.get("ordner"), str)):
        bad("socket")
    name = s.get("name", "dienst.sock")
    if not isinstance(name, str) or name in ("", ".", "..") or "\x00" in name or len(name.encode()) > 64:
        bad("socket.name")
    if ("/" in name or name.startswith("..")) and not m("start_socketname_pfad_ok"):
        bad("socket.name")
    ordner = s["ordner"]
    if not ordner.startswith("/") or "\x00" in ordner or ordner == "/":
        bad("socket.ordner muss ein absoluter Pfad sein")
    pfad = ordner.rstrip("/") + "/" + name
    if len(pfad.encode()) > SOCKET_PFAD_MAX:
        bad("Socket-Pfad ist laenger als %d Byte" % SOCKET_PFAD_MAX)
    lokal = None
    if "lokal" in o:               # Nachtrag 26.11.2026, DI-L11 point 1
        lo = o["lokal"]
        if not (isinstance(lo, dict) and set(lo) == {"kanal"} and isinstance(lo["kanal"], str) and KENNUNG.fullmatch(lo["kanal"])):
            bad("lokal")
        lp = ordner.rstrip("/") + "/" + lo["kanal"] + ".sock"
        if len(lp.encode()) > SOCKET_PFAD_MAX or lo["kanal"] + ".sock" == name:
            bad("lokal.kanal")
        lokal = {"kanal": lo["kanal"], "pfad": lp}
    b = o["betrachter"]
    if not (isinstance(b, dict) and set(b) == {"person", "raum", "ansicht"} and all(isinstance(b[k], str) for k in b)
            and b["ansicht"] in ("fenster", "browser")):
        bad("betrachter")
    argv = env = cwd = g = None
    if "kind" in o:
        k = o["kind"]
        if not (isinstance(k, dict) and set(k) <= {"argv", "env", "cwd"} and "argv" in k and "env" in k):
            bad("kind")
        argv = k["argv"]
        if not (isinstance(argv, list) and 1 <= len(argv) <= 256 and all(isinstance(x, str) and 0 < len(x.encode()) <= 4096 and "\x00" not in x for x in argv)):
            bad("kind.argv")
        if not argv[0].startswith("/") and not m("start_relativer_pfad_ok"):
            bad("kind.argv[0] muss ein absoluter Pfad sein")
        env = k["env"]
        if not env_ok(env):
            bad("kind.env")
        cwd = k.get("cwd", "/")
        if not (isinstance(cwd, str) and cwd.startswith("/") and "\x00" not in cwd):
            bad("kind.cwd")
    if "groesse" in o:
        g = o["groesse"]
        grenze = 1001 if m("start_groesse_1001_ok") else 1000
        if not (isinstance(g, list) and len(g) == 2 and all(ganz(x) and 1 <= x <= grenze for x in g)):
            bad("groesse")
        if not spalten_ok(g[0]) and not m("start_eine_spalte_ok"):
            bad("groesse hat weniger als %d Spalten" % KLEINSTE_SPALTEN)
    rl = o.get("ruecklauf", 2000)
    if not (ganz(rl) and 0 <= rl and (rl <= 100000 or m("start_ruecklauf_ohne_grenze"))):
        bad("ruecklauf")
    de = o.get("deflate", True)
    pm = o.get("pruefmodus", False)
    if not isinstance(de, bool) or not isinstance(pm, bool):
        bad("deflate oder pruefmodus")
    wmax = WARTESCHLANGE_MAX
    if "warteschlange_max" in o:
        w = o["warteschlange_max"]
        if not ganz(w) or w < 1 or (w > WARTESCHLANGE_MAX and not m("start_wmax_ueber_grenze_ok")):
            bad("warteschlange_max")
        if not pm and not m("start_wmax_ohne_pruefmodus_ok"):
            bad("warteschlange_max nur im Pruefmodus")
        wmax = w
    a = o["anfang"]
    if not isinstance(a, dict):
        bad("anfang")
    sz = a.get("sitzungen")
    s0 = None
    if lauf_da and "kind" not in o:
        if sz not in (None, []):
            bad("anfang.sitzungen muss ohne kind fehlen oder leer sein")      # DI12: ohne kind startet der Dienst mit null Sitzungen
    else:
        if not (isinstance(sz, list) and len(sz) >= 1 and all(isinstance(x, dict) for x in sz)):
            bad("anfang.sitzungen")
        if len(sz) != 1 and not m("start_mehrere_sitzungen_ok"):
            bad("anfang.sitzungen muss genau eine Sitzung nennen")
        s0 = sz[0]
        if not isinstance(s0.get("id"), str):
            bad("Sitzung ohne id")
        if lauf_da and re.fullmatch(r"s-[0-9]+", s0["id"]) and not m("lauf_kind_id_kollision_ok"):
            bad("anfang.sitzungen[0].id gehoert mit lauf den Sitzungen von Starten (s-<n>)")
        if not m("start_traegerfeld_ok"):
            for f in ("zeilen", "stand", "bracketed_paste", "invers") + (() if m("start_cursorfeld_ok") else ("cursor",)):
                if f in s0:
                    bad("anfang.sitzungen[0]." + f + " gehoert dem Traeger")
        if "groesse" in s0 and s0["groesse"] != g:
            bad("anfang.sitzungen[0].groesse widerspricht groesse")
    if b["person"] not in (a.get("personen") or {}):
        bad("betrachter.person fehlt in anfang.personen")
    try:
        zk_ref.Kern(copy.deepcopy(a), "")
    except (KeyError, TypeError, ValueError, AttributeError) as e:
        bad("anfang: %r" % e)
    return {"ordner": ordner, "name": name, "pfad": pfad, "betrachter": b, "argv": argv, "env": env, "cwd": cwd,
            "groesse": list(g) if g else None, "ruecklauf": rl, "deflate": de, "pruefmodus": pm, "anfang": a, "sid": s0["id"] if s0 else None,
            "wmax": wmax, "lauf": lauf, "lokal": lokal,
            **({"wmax_gesetzt": True} if "warteschlange_max" in o else {})}


# ---------------------------------------------------------------------------------------------- Rechte (DI2)

def rechte_pruefen(cfg):
    ordner = cfg["ordner"]
    try:
        st = os.lstat(ordner)
    except FileNotFoundError:
        if m("rechte_ordner_anlegen"):
            os.mkdir(ordner, 0o700)
            return
        raise StartFehler(3, "Ordner des Sockets fehlt")
    except OSError as e:
        raise StartFehler(3, "Ordner des Sockets nicht lesbar: %s" % e.strerror)
    if stat.S_ISLNK(st.st_mode):
        if not m("rechte_link_folgen"):
            raise StartFehler(3, "Ordner des Sockets ist ein symbolischer Link")
        st = os.stat(ordner)
    if not stat.S_ISDIR(st.st_mode):
        raise StartFehler(3, "Ordner des Sockets ist kein Ordner")
    if st.st_uid != os.geteuid() and not m("rechte_besitzer_nicht_geprueft"):
        raise StartFehler(3, "Ordner des Sockets gehoert einem anderen Benutzer")
    modus = stat.S_IMODE(st.st_mode)
    if m("rechte_modus_nicht_geprueft"):
        pass
    elif m("rechte_0750_ok"):
        if modus & 0o007:
            raise StartFehler(3, "Ordner des Sockets hat Modus %04o statt 0700" % modus)
    elif modus != 0o700:
        raise StartFehler(3, "Ordner des Sockets hat Modus %04o statt 0700" % modus)
    try:
        os.lstat(cfg["pfad"])
        if not m("rechte_socket_ersetzen"):
            raise StartFehler(3, "Socket-Pfad besteht schon")
        os.unlink(cfg["pfad"])
    except FileNotFoundError:
        pass
    if cfg.get("lokal"):           # Nachtrag 26.11.2026, DI-L11 point 3: the path of the local way must not exist (pk.name_belegt)
        try:
            os.lstat(cfg["lokal"]["pfad"])
            raise StartFehler(3, "Pfad des lokalen Wegs besteht schon")
        except FileNotFoundError:
            pass
    L = cfg.get("lauf") or {}
    if L.get("bestand"):           # Nachtrag 26.11.2026, DI-L5 point 3: the folder of the Bestand like DI2 points 1 to 3
        eltern = os.path.dirname(L["bestand"]) or "/"
        try:
            st = os.lstat(eltern)
        except OSError:
            raise StartFehler(3, "Ordner des Bestands fehlt")
        if stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode) or st.st_uid != os.geteuid() or stat.S_IMODE(st.st_mode) != 0o700:
            raise StartFehler(3, "Ordner des Bestands hat falsche Rechte")


# ---------------------------------------------------------------------------------------------- Traeger (Stand-in)

class Traeger:
    """Screen model for the fixed children: width-1 characters, no reflow."""

    def __init__(self, sp, ze):
        if not spalten_ok(sp) and not m("groesse_eine_spalte_durchlassen"):
            raise ValueError("weniger als %d Spalten" % KLEINSTE_SPALTEN)    # no silent clamping (Entscheidung vom 20.11.2026)
        self.sp, self.ze = sp, ze
        self.zeilen = [self.leer() for _ in range(ze)]
        self.r = self.c = 0
        self.sichtbar = True                  # DECTCEM (Nachtrag 19.11., TR2)
        self.cursor_gemeldet = [0, 0, True]   # the start cursor counts as reported (TR2)
        self.warte = False
        self.stil = (0, 0, 0)
        self.stand = 0
        self.gemeldet = [""] * ze
        self.dec = codecs.getincrementaldecoder("utf-8")("replace")
        self.zst = "g"
        self.buf = ""
        self.bracketed_paste = False
        self.invers = False

    def leer(self):
        return [(0, 0, 0, " ")] * self.sp

    def sgr_zeile(self, i):
        laeufe = [(f, b, fl, ch) for f, b, fl, ch in self.zeilen[i]]
        return ra_ref.zeile_schreiben(ra_ref.normalisiere(laeufe))

    def cursor(self):
        return [self.r, self.c, self.sichtbar]

    def ausgabe(self, ev):
        """TR2 (Nachtrag 19.11.): one Ausgabe when a line or the cursor differs from the last report; it always carries the cursor."""
        neu = {}
        for i in range(self.ze):
            s = self.sgr_zeile(i)
            if s != self.gemeldet[i]:
                neu[str(i)] = s
                self.gemeldet[i] = s
        cur = self.cursor()
        cursor_neu = cur != self.cursor_gemeldet and not m("traeger_cursor_nur_zeilen")
        if neu or cursor_neu:
            self.stand += 1
            e = {"art": "Ausgabe", "stand": self.stand, "zeilen": neu}
            if not m("traeger_cursor_fehlt"):
                e["cursor"] = cur
            ev.append(e)
            self.cursor_gemeldet = list(cur)

    def bytes_(self, b):
        ev = []
        text = self.dec.decode(b)
        i, n = 0, len(text)
        while i < n:
            if self.zst == "osc":     # fast path: the body of an OSC string up to its end (a 100 kB clipboard string is one slice)
                mo = OSC_ENDE.search(text, i)
                j = mo.start() if mo else n
                self.buf += text[i:j]
                i = j
                if i >= n:
                    break
            self.zeichen(text[i], ev)
            i += 1
        self.ausgabe(ev)
        return ev

    def vor_ereignis(self, ev):
        self.ausgabe(ev)

    def scroll(self):
        self.zeilen.pop(0)
        self.zeilen.append([(0, self.stil[1], 0, " ")] * self.sp)

    def lf(self):
        if self.r == self.ze - 1:
            self.scroll()
        else:
            self.r += 1

    def zeichen(self, ch, ev):
        z = self.zst
        o = ord(ch)
        if z == "g":
            if ch == "\x1b":
                self.zst = "esc"
            elif ch == "\r":
                self.c = 0
                self.warte = False
            elif ch == "\n":
                self.lf()       # a line feed leaves the pending wrap standing (measured against tr-ref at 2 columns, Nachtrag 20.11.)
            elif ch == "\x08":
                self.c = max(0, self.c - 1)
                self.warte = False
            elif o < 0x20 or o == 0x7F:
                pass
            else:
                if 0x80 <= o <= 0x9F:
                    ch = "�"
                if self.warte:
                    self.c = 0
                    self.lf()
                    self.warte = False
                self.zeilen[self.r][self.c] = (self.stil[0], self.stil[1], self.stil[2], ch)
                if self.c == self.sp - 1:
                    if m("traeger_cursor_umbruch_sofort"):
                        self.c = 0
                        self.lf()
                    else:
                        self.warte = True
                else:
                    self.c += 1
        elif z == "esc":
            if ch == "[":
                self.zst, self.buf = "csi", ""
            elif ch == "]":
                self.zst, self.buf = "osc", ""
            else:
                self.zst = "g"
        elif z == "csi":
            if ch in "0123456789;?<=>!$ '":
                self.buf += ch
            elif "@" <= ch <= "~":
                self.zst = "g"
                self.csi(self.buf, ch, ev)
            else:
                self.zst = "g"
        elif z == "osc":
            if ch == "\x07":
                self.zst = "g"
                self.osc(self.buf, ev)
            elif ch == "\x1b":
                self.zst = "oscesc"
            else:
                self.buf += ch
        elif z == "oscesc":
            if ch == "\\":
                self.zst = "g"
                self.osc(self.buf, ev)
            else:
                self.zst = "esc"
                self.zeichen(ch, ev)

    def csi(self, buf, final, ev):
        if final == "m" and not buf.startswith(("?", "<", "=", ">")):
            try:
                self.stil = ra_ref.wende_sgr(ra_ref.parameter(buf), self.stil)
            except ra_ref.Fehler:
                pass
        elif final in "Hf":
            ps = [int(x) if x.isdigit() else 0 for x in buf.split(";")] if buf else []
            r = (ps[0] if len(ps) > 0 and ps[0] else 1) - 1
            c = (ps[1] if len(ps) > 1 and ps[1] else 1) - 1
            self.r, self.c, self.warte = min(r, self.ze - 1), min(c, self.sp - 1), False
        elif final == "J" and buf == "2":
            self.zeilen = [[(0, self.stil[1], 0, " ")] * self.sp for _ in range(self.ze)]
        elif final == "K" and buf in ("", "0"):
            if self.warte:      # erase to end of line with the wrap pending erases nothing (measured against tr-ref, Nachtrag 20.11.)
                return
            for i in range(self.c, self.sp):
                self.zeilen[self.r][i] = (0, self.stil[1], 0, " ")
        elif final in "hl" and buf.startswith("?") and buf[1:].replace(";", "").isdigit():
            an = final == "h"
            for p in buf[1:].split(";"):
                n = int(p) if p else 0
                if n == 5:
                    self.vor_ereignis(ev)
                    self.invers = an
                    self.stand += 1
                    ev.append({"art": "Modus", "invers": an, "stand": self.stand})
                elif n == 2004:
                    self.vor_ereignis(ev)
                    self.bracketed_paste = an
                    ev.append({"art": "Modus", "bracketed_paste": an})
                elif n == 2026:
                    self.vor_ereignis(ev)
                    ev.append({"art": "SyncBeginn" if an else "SyncEnde"})
                elif n == 25 and not m("traeger_cursor_sichtbar_fehlt"):
                    self.sichtbar = an

    def osc(self, buf, ev):
        teile = buf.split(";")
        if teile[0] in ("0", "2") and len(teile) >= 2:
            self.vor_ereignis(ev)
            ev.append({"art": "Titel", "text": ";".join(teile[1:])})
        elif teile[0] == "52" and len(teile) == 3:
            self.vor_ereignis(ev)
            if teile[2] == "?":
                ev.append({"art": "Zwischenablage", "lesen": True})
            else:
                ev.append({"art": "Zwischenablage", "daten_b64": teile[2]})

    def groesse(self, sp, ze):
        if not spalten_ok(sp) and not m("groesse_eine_spalte_durchlassen"):
            raise ValueError("weniger als %d Spalten" % KLEINSTE_SPALTEN)
        ev = []
        if ze < self.ze and self.r >= ze and not m("traeger_groesse_zeilen_oben"):
            ueber = self.r - ze + 1
            self.zeilen = self.zeilen[ueber:]
            self.r -= ueber
        self.zeilen = (self.zeilen + [None] * ze)[:ze]
        self.zeilen = [z if z is not None else [(0, 0, 0, " ")] * sp for z in self.zeilen]
        self.zeilen = [(z + [(0, 0, 0, " ")] * sp)[:sp] for z in self.zeilen]
        self.sp, self.ze = sp, ze
        self.r, self.c = min(self.r, ze - 1), min(self.c, sp - 1)
        self.warte = False
        # TR3 (Nachtrag 19.11.): the core clamps its cursor to the new size; the carrier clamps what it reported and compares with that
        cg = self.cursor_gemeldet
        self.cursor_gemeldet = [min(cg[0], ze - 1), min(cg[1], sp - 1), cg[2]]
        self.gemeldet = (self.gemeldet + [""] * ze)[:ze]
        self.ausgabe(ev)
        return ev


# ---------------------------------------------------------------------------------------------- Rahmen

def rahmen(art, nutzlast):
    return struct.pack(">IB", 1 + len(nutzlast), art) + nutzlast


def jbytes(o):
    return json.dumps(o, ensure_ascii=False, sort_keys=True).encode("utf-8")


class Client:
    def __init__(self, sock, name, nr, deflate):
        self.sock, self.name, self.nr = sock, name, nr
        self.rbuf = bytearray()
        self.queue = collections.deque()
        self.qlen = 0
        self.ver = ra_ref.Verbindung(deflate and not m("betr_deflate_ignoriert"))
        self.abo = None
        self.abos = {}                  # P25 (with lauf): session id -> [spalten, zeilen] of every Abo this viewer holds (the service's own record)
        self.stumm = False
        self.schliessen = False
        self.gesperrt = False
        self.tot = False
        self.zuletzt = 0
        self.wer = None                 # DI-L11 point 4: the account of the partner, only on the local way


def proz_stand(pid):
    """Fields of /proc/<pid>/stat (Linux): state, group, session, start time; None where not readable."""
    if not sys.platform.startswith("linux"):
        return None
    try:
        with open("/proc/%d/stat" % pid, "rb") as h:
            text = h.read(4096).decode("latin-1")
        rest = text[text.rindex(")") + 1:].split()
        return {"zustand": rest[0], "gruppe": int(rest[2]), "sitzung": int(rest[3]), "start": int(rest[19])}
    except (OSError, ValueError, IndexError):
        return None


def pid_lesen(ordner, name):
    """The pid of a pid file: a regular file (no link), at most 11 bytes, exactly decimal digits and one line feed, 2 to 2^31-1."""
    pfad = os.path.join(ordner, name)
    folgen = m("ende_signal_link_folgen")
    try:
        st = os.lstat(pfad)
        if not (stat.S_ISREG(st.st_mode) or (folgen and stat.S_ISLNK(st.st_mode))):
            return None
        fd = os.open(pfad, os.O_RDONLY | (0 if folgen else os.O_NOFOLLOW))
        try:
            roh = os.read(fd, 12)
        finally:
            os.close(fd)
    except OSError:
        return None
    if len(roh) > 11:
        return None
    if m("ende_signal_ohne_zeilenende"):
        try:
            pid = int(roh.strip())
        except ValueError:
            return None
    else:
        if not re.fullmatch(rb"[0-9]{1,10}\n", roh):
            return None
        pid = int(roh[:-1])
    return pid if 2 <= pid <= 2147483647 else None


class Sitzung:
    """One set of the service (DI12): carrier, core, child and PTY. The single mode has exactly one set (number 0);
    the base set (`sid` None, no carrier) holds the core of the room in the multi-session mode."""

    def __init__(self, sid, nr, groesse, meta=None, eintrag=None):
        self.sid, self.nr = sid, nr
        self.traeger = Traeger(*groesse) if groesse else None
        self.kern = None
        self.kern_zaehler = 0
        self.kind_ende = None
        self.pty_aus = bytearray()
        self.pid = None
        self.master = None
        self.zustand = "laeuft"
        self.meta = meta or {"harness": "", "modell": "", "route": "", "kunde": ""}
        self.eintrag = eintrag          # the entry of `anfang.sitzungen` this core is built from (a started session)
        self.ordner = None              # the folder `<ordner>/<sid>` the service made for Starten
        self.kill_ab = None             # monotonic deadline of SIGKILL after Beenden
        self.beendet_von_uns = False
        self.kind_pid = None            # pid and start time of the child at the start of the session (for the pid protection at the end)
        self.kind_start = None
        self.beendet_als = None         # P25: the place of this session in the order of the ends of the run (1 for the first end); None while it runs
        # Nachtrag 26.11.2026
        self.name = ""                  # DI-L3 Umbenennen, DI-L5
        self.arbeitsordner = ""         # DI-L5 `ordner`
        self.start = None               # the start structure of the request (DI-L3 Fortsetzen, DI-L5 Bestand)
        self.orchestrator = None        # DI-L1
        self.rolle = None
        self.fortsetzung_von = None     # DI-L3
        self.fortgesetzt_als = None
        self.ende_art = None            # "beenden", "selbst" or "neustart" (DI-L5 point 2)
        self.grund = None

    def abgestuerzt(self):
        """DI-L5 point 2: ended by itself with a code other than 0, or still running at the end of the last run of the service."""
        if self.zustand != "beendet":
            return False
        if m("wiederbeleben_ohne_absturz"):
            return True
        return self.ende_art == "neustart" or (self.ende_art == "selbst" and self.kind_ende not in (0, None))


def projekt_von(ordner):
    """DI-L5 point 1: the display name is the last part of the working folder; "" for "" and "/"."""
    if m("projekt_ganzer_pfad"):
        return ordner
    teil = ordner.rstrip("/").rsplit("/", 1)[-1] if ordner else ""
    return teil


def name_ok(n):
    """DI-L3 point 1: 1 to 64 characters, no control character, no white space at the start or the end."""
    if m("umbenennen_ohne_pruefung"):
        return isinstance(n, str)
    return (isinstance(n, str) and 1 <= len(n) <= 64 and not any(ord(c) < 0x20 or 0x7f <= ord(c) <= 0x9f for c in n)
            and n == n.strip())


def begruendung_form(o, ablehnen):
    """zustandskern R-L6 point 2: the form of `entscheidung` and `begruendung`; None when the form holds, else the reason."""
    if "begruendung" in o and not isinstance(o["begruendung"], str):
        return "ereignis_ungueltig"
    if "begruendung" in o and not ablehnen:
        return "begruendung_nur_bei_ablehnen"
    if "begruendung" in o:
        b = o["begruendung"]
        if not b or len(b) > 500 or any(ord(c) < 0x20 or 0x7f <= ord(c) <= 0x9f for c in b):
            return "begruendung_ungueltig"
    return None


def rahmennummer(n, rmax, vergeben, frei):
    """P25: n itself when it is at most rmax and never handed out, else the smallest number never handed out, else the free number
    whose session ended first (`frei`: (place of the end, number)). None: every number from 1 to rmax is held. 0 is the session of `kind`."""
    if 1 <= n <= rmax and n not in vergeben:
        return n
    for x in range(1, rmax + 1):
        if x not in vergeben:
            return x
    frei = sorted((e, x) for e, x in frei if 1 <= x <= rmax)
    return frei[0][1] if frei else None


def abo_nachfuehren(abos, sid, e):
    """P25: the service's record of the Abos of a viewer (as the build's `abo_nachfuehren`): Abo an with a size, Abo aus, Groesse."""
    art, an = e.get("art"), e.get("an", True)
    groesse = [e["spalten"], e["zeilen"]] if ganz(e.get("spalten")) and ganz(e.get("zeilen")) else None
    if art == "Abo" and an is True:
        if groesse is not None:
            abos[sid] = groesse
    elif art == "Abo" and an is False:
        abos.pop(sid, None)
    elif art == "Groesse" and sid in abos and groesse is not None:
        abos[sid] = groesse


def kanon(o):
    """The canonical form of a start structure (DI12): compact, keys sorted, UTF-8, no \\u escapes for non-ASCII."""
    return json.dumps(o, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


# ---------- Pruefstelle `start` (addendum of 20.11.2026)
# The stages of `start::lesen_anfrage` (1 to 13) and `start::pruefen` (14 to 17) of contract 1 that depend only on the start structure and the
# settings file; the codes and the fixed texts are the ones of the table of contract 1. The stages that need the file system, markers or the enclosure
# belong to `Lauf starten` (steps 4a to 8 of the call), not to this place (contract text, DI12 point 4).
STUFEN = [
    (1, "zu_gross", "Start zu gross"), (2, "json_ungueltig", "kein gueltiges JSON"), (3, "sitzung_nicht_erlaubt", "Sitzung vergibt der Dienst"),
    (4, "marker_nicht_erlaubt", "Marker setzt der Dienst"), (5, "schluessel_unbekannt", "unbekannter Schluessel"), (6, "schluessel_fehlt", "Pflichtfeld fehlt"),
    (7, "typ_falsch", "falscher Typ"), (8, "fassung_unbekannt", "Fassung unbekannt"), (9, "kennung_form", "Kennung hat falsche Form"),
    (10, "modell_form", "Modell hat falsche Form"), (11, "ordner_ungueltig", "Arbeitsordner ungueltig"), (12, "marker_form", "Marker haben falsche Form"),
    (13, "harness_unbekannt", "Harness unbekannt"), (14, "route_unbekannt", "Route unbekannt"), (15, "modell_route", "Modell passt nicht zur Route"),
    (16, "kunde_unbekannt", "Kunde unbekannt"), (17, "harness_route_unvertraeglich", "Harness passt nicht zur Route"),
]
STUFE_VON = {code: (nr, text) for nr, code, text in STUFEN}
ANFRAGE_SCHLUESSEL = ("fassung", "harness", "modell", "route", "kunde", "arbeitsordner")
KENNUNG = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
KENNUNG_GROSS = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")
KENNUNG_LANG = re.compile(r"[a-z0-9][a-z0-9._-]{0,200}")
MODELL = re.compile(r"[A-Za-z0-9._:/@+-]{1,128}")
JSON_TIEFE = 127


class Ganz:
    """A JSON number without fraction and exponent (the text of the number in `t`); it is no text and no float."""

    def __init__(self, t):
        self.t = t


def _int_hook(t):
    return Ganz(t)


def _paare(liste):
    o = {}
    for k, v in liste:
        if k in o:
            raise ValueError("doppelter Schluessel")
        o[k] = v
    return o


def streng_lesen(roh):
    """The strict JSON reader of contracts 1 and 2 on bytes: UTF-8 without a byte order mark, no duplicate key, no NaN, nothing after the value,
    at most 127 levels. Raises ValueError."""
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    text = roh.decode("utf-8")
    tiefe, im_text, maskiert = 0, False, False
    for z in text:
        if im_text:
            if maskiert:
                maskiert = False
            elif z == "\\":
                maskiert = True
            elif z == '"':
                im_text = False
        elif z == '"':
            im_text = True
        elif z in "[{":
            tiefe += 1
            if tiefe > JSON_TIEFE:
                raise ValueError("Verschachtelung")
        elif z in "]}":
            tiefe -= 1
    return json.loads(text, object_pairs_hook=_paare, parse_int=_int_hook, parse_constant=_konstante)


def kennung_ok(t):
    if m("stufe_09_kennung_gross_ok"):
        return KENNUNG_GROSS.fullmatch(t) is not None
    if m("stufe_09_kennung_laenge_ok"):
        return KENNUNG_LANG.fullmatch(t) is not None
    return KENNUNG.fullmatch(t) is not None


def modell_ok(t):
    if m("stufe_10_modell_ok"):
        return True
    if m("stufe_10_modell_laenge_ok"):
        return re.fullmatch(r"[A-Za-z0-9._:/@+-]{1,300}", t) is not None
    if m("stufe_10_modell_leer_ok"):
        return re.fullmatch(r"[A-Za-z0-9._:/@+-]{0,128}", t) is not None
    if m("stufe_10_modell_zeichen_ok"):
        return re.fullmatch(r"[A-Za-z0-9._:/@+ -]{1,128}", t) is not None
    return MODELL.fullmatch(t) is not None


MARKER_DIENST = None       # the markers read from lauf.marker_datei at the start (addendum of 21.11.2026); None: no markers
MARKER_PFAD = None


def marker_ok(w):
    """The content of the marker file: exactly `liste` (1 to 16 texts of 1 to 32 characters of A-Z, 0-9, `-`, no duplicate). The folder of the count is not part of it
    (Nachtrag 2): it comes from `lauf.marker_ordner` of the start file."""
    if m("marker_form_uebersprungen"):
        return isinstance(w, dict)
    erlaubt = {"liste", "ordner"} if m("marker_ordner_aus_datei") else {"liste"}
    if not isinstance(w, dict) or not ({"liste"} <= set(w) <= erlaubt):
        return False
    liste = w["liste"]
    if not isinstance(liste, list):
        if not m("marker_liste_kein_array_ok"):
            return False
        liste = [str(liste)]
    if not ((0 if m("marker_liste_leer_ok") else 1) <= len(liste) <= (17 if m("marker_siebzehn_ok") else 16)):
        return False
    zeichen = r"[A-Z0-9,-]{1,32}" if m("marker_komma_ok") else r"[A-Z0-9-]{1,32}"
    if not all(isinstance(x, str) and re.fullmatch(zeichen, x) is not None for x in liste):
        return False
    return len(set(liste)) == len(liste)


def marker_datei_lesen(pfad, marker_ordner=None):
    """lauf.marker_datei (addendum of 21.11.2026): read once at the start. A missing file (or a missing folder) is the normal case: no markers. Otherwise the folder is
    checked first (`O_DIRECTORY | O_NOFOLLOW`, own owner, mode exactly 0700), the file is opened relative to it (`O_NOFOLLOW`) and checked with `fstat` (a regular file,
    own owner, mode exactly 0600); at most 4096 bytes; UTF-8 and JSON with the form of `marker_ok`. Every other result is exit code 2 with the line
    `lauf.marker_datei ungueltig: <fixed reason>`; the line holds neither the path nor the content."""
    def schlecht(grund):
        raise StartFehler(2, "lauf.marker_datei ungueltig: " + grund)

    eltern, name = os.path.split(pfad)
    try:
        dfd = os.open(eltern or "/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | (0 if m("marker_ordner_nofollow_fehlt") else os.O_NOFOLLOW))
    except FileNotFoundError:
        return None
    except OSError:
        schlecht("ordner")
    try:
        st = os.fstat(dfd)
        if not m("marker_ordner_ungeprueft"):
            if st.st_uid != os.geteuid():
                schlecht("ordner_eigentuemer")
            if stat.S_IMODE(st.st_mode) != 0o700:
                schlecht("ordner_modus")
        try:
            fd = os.open(name, os.O_RDONLY | os.O_CLOEXEC | (0 if m("marker_datei_nofollow_fehlt") else os.O_NOFOLLOW) | (0 if m("marker_nonblock_fehlt") else os.O_NONBLOCK), dir_fd=dfd)
        except FileNotFoundError:
            if m("marker_fehlt_ist_fehler"):
                schlecht("fehlt")
            return None
        except OSError as e:
            schlecht("datei_verknuepfung" if e.errno == errno.ELOOP else "datei_unlesbar")
        try:
            sf = os.fstat(fd)
            if not stat.S_ISREG(sf.st_mode):
                schlecht("datei_art")
            if sf.st_uid != os.geteuid():
                schlecht("datei_eigentuemer")
            if stat.S_IMODE(sf.st_mode) != 0o600 and not m("marker_modus_ungeprueft"):
                schlecht("datei_modus")
            if sf.st_size > 4096 and not m("marker_groesse_ungeprueft"):
                schlecht("zu_gross")
            roh = b""
            while True:
                stueck = os.read(fd, 65536)
                if not stueck:
                    break
                roh += stueck
                if len(roh) > 4096 and not m("marker_groesse_ungeprueft"):
                    schlecht("zu_gross")
        finally:
            os.close(fd)
    finally:
        os.close(dfd)
    try:
        w = json.loads(roh.decode("utf-8"))
    except UnicodeDecodeError:
        schlecht("kein_utf8")
    except ValueError:
        schlecht("kein_json")
    if not marker_ok(w):
        schlecht("form")
    # Nachtrag 2: the folder of the count comes from the start file; with a valid list a missing or malformed `lauf.marker_ordner` is exit code 2
    ordner = w.get("ordner") if m("marker_ordner_aus_datei") and "ordner" in w else marker_ordner
    if ordner is None:
        if m("marker_ordner_fehlt_uebergangen"):
            return None
        raise StartFehler(2, "lauf.marker_ordner fehlt, die Marker-Datei braucht ihn")
    if not ordner_ok(ordner) and not m("marker_form_uebersprungen"):
        raise StartFehler(2, "lauf.marker_ordner ungueltig")
    return {"liste": w["liste"], "ordner": ordner}


def ordner_ok(t):
    """An absolute, normalized path of at most 1024 bytes: no `//`, no trailing `/`, no `.` or `..` segment, no control character (U+0000 to U+001F, U+007F to U+009F)."""
    if m("stufe_11_ordner_ok"):
        return True
    if len(t.encode("utf-8")) > (2048 if m("stufe_11_ordner_laenge_ok") else 1024):
        return False
    if not t.startswith("/") and not m("stufe_11_ordner_relativ_ok"):
        return False
    if t.endswith("/") and not m("stufe_11_ordner_ende_ok"):
        return False
    if "//" in t and not m("stufe_11_ordner_doppel_ok"):
        return False
    if not m("stufe_11_ordner_punkt_ok") and any(sg in (".", "..") for sg in t.split("/")[1:]):
        return False
    grenze = 0x7f if m("stufe_11_ordner_c1_ok") else 0xa0
    if any(ord(z) <= 0x1f or 0x7f <= ord(z) < grenze for z in t):
        return False
    return True


def start_lesen_roh(roh):
    """Stages 1 to 13 of contract 1 on the bytes of a request. Returns (code, felder): the code of the lowest violated stage or None, and the fields."""
    if len(roh) > (START_MAX - 1 if m("starten_grenze_frueh") else START_MAX) and not m("starten_gross_ok"):
        return "zu_gross", None
    try:
        o = streng_lesen(roh)
    except (ValueError, UnicodeDecodeError, RecursionError):
        return "json_ungueltig", None
    if not isinstance(o, dict):
        if m("starten_form_nicht_objekt_ok"):
            o = {}
        else:
            return "json_ungueltig", None
    v = set()
    # contract `start`, version 2 (Nachtrag 26.11.2026): two more keys, `effort` and `name`, each a text or null. The reference checks only
    # their presence and type (stages 5 to 7); the forms of stages 11a, 11b and 13a are the crate's (the cases of the service use null)
    f2 = isinstance(o.get("fassung"), Ganz) and o["fassung"].t == "2"
    schluessel = ANFRAGE_SCHLUESSEL + (("effort", "name") if f2 else ())
    if "sitzung" in o and not m("starten_sitzung_ok"):
        v.add(3)
    if "marker" in o and not m("stufe_04_marker_ok") and not (m("marker_anfrage_ersetzt") and MARKER_DIENST is not None):
        v.add(4)
    if any(k not in schluessel for k in o) and not m("stufe_05_unbekannt_ok") and not (m("starten_sitzung_ok") and set(o) - set(schluessel) == {"sitzung"}):
        v.add(5)

    def pflicht(name):
        if name not in o:
            if not m("stufe_06_fehlt_ok"):
                v.add(6)
            return KEIN
        return o[name]

    def text(name):
        w = pflicht(name)
        if w is KEIN:
            return None
        if not isinstance(w, str):
            if not m("stufe_07_typ_ok"):
                v.add(7)
            return None
        return w

    def kennung(name):
        t = text(name)
        if t is not None and not m("stufe_09_kennung_ok") and not kennung_ok(t):
            v.add(9)
        return t if t is not None else ""

    f = pflicht("fassung")
    if f is not KEIN:
        if isinstance(f, Ganz):
            if f.t not in ("1", "2") and not m("starten_fassung_nicht_geprueft"):
                v.add(8)
        elif f is True and m("starten_fassung_bool_ok"):
            pass
        elif not m("stufe_07_typ_ok"):
            v.add(7)
    if f2:
        for name in ("effort", "name"):
            w = pflicht(name)
            if w is not KEIN and w is not None and not isinstance(w, str) and not m("stufe_07_typ_ok"):
                v.add(7)
    harness = kennung("harness")
    route = kennung("route")
    k = pflicht("kunde")
    kunde = None
    if k is not KEIN and k is not None:
        if not isinstance(k, str):
            if not m("stufe_07_typ_ok"):
                v.add(7)
        else:
            kunde = k
            if not m("stufe_09_kennung_ok") and not kennung_ok(k):
                v.add(9)
    mo = text("modell")
    if mo is not None and not modell_ok(mo):
        v.add(10)
    ao = text("arbeitsordner")
    if ao is not None and not ordner_ok(ao):
        v.add(11)
    if harness not in HARNESS_ART and not m("stufe_13_harness_ok") or (harness == "copilot" and m("stufe_13_copilot_unbekannt")):
        v.add(13)
    if v:
        nr = max(v) if m("stufen_hoechste_zuerst") else min(v)
        return next(c for n, c, _ in STUFEN if n == nr), None
    return None, {"harness": harness, "modell": mo or "", "route": route, "kunde": kunde, "arbeitsordner": ao or ""}


def stufen_pruefen(felder, einst):
    """Stages 14 to 17 of contract 1 against the settings file. Returns the code of the first violated stage or None."""
    if m("stufen_14_bis_17_fehlen"):
        return None
    route = next((r for r in einst["routen"] if r["schluessel"] == felder["route"]), None)

    def s14():
        return "route_unbekannt" if route is None and not m("stufe_14_route_ok") else None

    def s15():
        if route is None or m("stufe_15_modell_ok"):
            return None
        if route["modell"] is not None and route["modell"] != felder["modell"] and not (m("stufe_15_cloud_ignoriert") and route["art"] == "cloud"):
            return "modell_route"
        return None

    def s16():
        if m("stufe_16_kunde_ok"):
            return None
        if felder["kunde"] is None:
            return "kunde_unbekannt" if m("stufe_16_null_unbekannt") else None
        return None if felder["kunde"] in einst["kunden"] else "kunde_unbekannt"

    def s17():
        if route is None or m("stufe_17_art_ok"):
            return None
        art = HARNESS_ART[felder["harness"]]
        if felder["harness"] == "copilot" and m("stufe_17_copilot_lokal"):
            art = "lokal"
        return "harness_route_unvertraeglich" if art != route["art"] else None
    for s in ((s17, s16, s15, s14) if m("stufen_14_bis_17_umgekehrt") else (s14, s15, s16, s17)):
        code = s()
        if code:
            return code
    return None


def einst_aus_objekt(o):
    """The settings file as a JSON value: a JSON object of `fassung` 1 or 2 that holds `routen` (list of objects with the texts `schluessel`, `art`, `klasse`; `modell` is an
    optional text of `fassung` 2) and `kunden` (list of objects with the text `schluessel`). Returns the pieces for the checks and the Startwahl or None. The checks of
    contract 2 (`einst.*`) are the crate's: the cases use only files that the crate reads, and files that the crate refuses are in the cases only where the reference refuses them too."""
    if not isinstance(o, dict):
        return None
    fassung = o.get("fassung")
    if not (ganz(fassung) and fassung in (1, 2)):
        return None
    routen, kunden = o.get("routen"), o.get("kunden")
    if not (isinstance(routen, list) and isinstance(kunden, list)):
        return None
    if not all(isinstance(r, dict) and all(isinstance(r.get(k), str) for k in ("schluessel", "art", "klasse")) for r in routen):
        return None
    if not all(isinstance(k, dict) and isinstance(k.get("schluessel"), str) for k in kunden):
        return None
    rr = []
    for r in routen:
        if "modell" in r and not (fassung == 2 and isinstance(r["modell"], str)):
            return None
        rr.append({"schluessel": r["schluessel"], "art": r["art"], "klasse": r["klasse"], "modell": r.get("modell"), "roh": r})
    return {"fassung": fassung, "routen": rr, "kunden": [k["schluessel"] for k in kunden]}


def start_pruefung(start, einst=None):
    """The reference of `start::lesen_anfrage` (and with `einst` of `start::pruefen`) for the structure of a `Starten`: the structure travels as a JSON value, the
    crate reads its canonical bytes. Returns (code, felder)."""
    try:
        roh = kanon(zahlen_klein(start))
    except (UnicodeEncodeError, ValueError):
        return "json_ungueltig", None
    code, felder = start_lesen_roh(roh)
    if code or einst is None:
        return code, felder
    return stufen_pruefen(felder, einst), felder


def zahlen_klein(w):
    """An integer outside the range of `u64` and `i64` is a floating point number for the Rust reader (serde_json): the class stays, the text differs."""
    if isinstance(w, dict):
        return {k: zahlen_klein(x) for k, x in w.items()}
    if isinstance(w, list):
        return [zahlen_klein(x) for x in w]
    if ganz(w) and not -2**63 <= w < 2**64:
        return float(w)
    return w


class Dienst:
    def __init__(self, cfg):
        self.cfg = cfg
        self.sel = selectors.DefaultSelector()
        self.clients = {}
        self.folge = 0
        self.t0 = time.monotonic()
        self.t_ein = 0
        self.t_zuletzt = 0
        self.ende = False
        self.lauf = cfg["lauf"]
        self.sitz = {}              # sid -> Sitzung, in the order of their start
        self.basis = None
        self.naechste = 0           # numbers of Starten handed out so far (s-<n>, never used again)
        self.vergeben = set()       # P25: raster numbers handed out in this run
        self.enden = 0              # P25: ends of sessions so far (the order of the ends)
        self.s0 = None
        if cfg["sid"] is not None:
            self.s0 = Sitzung(cfg["sid"], 0, cfg["groesse"])
            self.sitz[self.s0.sid] = self.s0
        if self.lauf is not None:
            self.basis = Sitzung(None, 0, None)
        self.arbeit = collections.deque()
        self.sender = None
        self.startwahl_letzt = None
        self.fehler_naechster = False
        self.neu_nach_riss = False
        self.riss_nr = 0
        # Nachtrag 26.11.2026
        self.agenten_letzt = []         # DI-L1 point 4: the list of workers last sent to everyone
        self.frei_geloescht = []        # DI-L3 point 5: (place of the end, number) of deleted sessions; their raster numbers are free
        self.fortsetzung = None         # DI-L3: the session that Fortsetzen or Wiederbeleben continues, during `starten`
        self.lokal_hoerer = None        # DI-L11
        self.lokal_warten = {}          # socket -> [bytes of the hello, deadline, uid]

    # ---------- time (DI6)
    def jetzt(self):
        if self.cfg["pruefmodus"] and not m("uhr_echtzeit"):
            return self.t_ein
        t = int((time.monotonic() - self.t0) * 1000)
        self.t_zuletzt = max(self.t_zuletzt, t)
        return self.t_zuletzt

    def kerne(self):
        return ([self.basis] if self.basis is not None else []) + list(self.sitz.values())

    def kernbau(self):
        return sum(s.kern_zaehler for s in self.kerne())

    # ---------- core
    def kern_bauen(self, s, neu=False):
        a = copy.deepcopy(self.cfg["anfang"])
        if s is self.basis:
            a["sitzungen"] = []
        else:
            if s.eintrag is not None:
                a["sitzungen"] = [copy.deepcopy(s.eintrag)]
            e0 = a["sitzungen"][0]
            e0["groesse"] = [s.traeger.sp, s.traeger.ze]
            # DI11 (decision of 19.11.2026 F2): the start cursor [0, 0, true] always goes along; at the rebuild the cursor last reported by the carrier
            if not m("anfang_cursor_fehlt"):
                e0["cursor"] = [0, 0, False] if m("anfang_cursor_unsichtbar") else [0, 0, True]
            if neu and not m("kernfehler_leerer_schirm"):
                e0["zeilen"] = list(s.traeger.gemeldet)
                e0["stand"] = s.traeger.stand
                e0["bracketed_paste"] = s.traeger.bracketed_paste
                e0["invers"] = s.traeger.invers
                if m("kernfehler_cursor_fehlt"):
                    e0.pop("cursor", None)
                elif m("kernfehler_cursor_aus_zustand"):
                    alt = s.kern.zustand()["sitzungen"][0].get("cursor")      # the held cursor of the old core (DEC 2026 waits for the lines)
                    e0["cursor"] = list(alt if alt else s.traeger.cursor_gemeldet)
                elif not m("kernfehler_cursor_anfang"):
                    e0["cursor"] = list(s.traeger.cursor_gemeldet)
        s.kern = zk_ref.Kern(a, "")
        s.kern_zaehler += 1
        s.kern.idx = -1

    def schritt(self, e, quelle, s):
        """One step of the core of `s`; the effects are handed on at once (breadth first for the carrier reports)."""
        k = s.kern
        k.zeit_bis(e["t"])
        k.idx += 1
        try:
            k.ereignis(e)
        except zk_ref.Abgelehnt as a:
            k.ablehnen(a.grund)
        aus = k.aus
        k.aus = {"nachrichten": {}, "pty": [], "groessen": [], "ablehnungen": [], "protokoll": [], "an_stelle": [], "abfragen": []}
        return aus

    def frist(self):
        fr = [min(s.kern.timer)[0] for s in self.kerne() if s.kern.timer]
        return min(fr) if fr else None

    def ereignis(self, e, quelle=None, s=None):
        s = s if s is not None else self.s0
        self.wirkungen(self.schritt(e, quelle, s), quelle, s)
        while self.arbeit:
            e2, q2, s2 = self.arbeit.popleft()
            self.wirkungen(self.schritt(e2, q2, s2), q2, s2)

    def ereignis_alle(self, e, quelle=None):
        """Verbinden, Trennen and Takt go to the core of every set, the base set first (DI12)."""
        for s in self.kerne():
            if e["art"] == "Trennen" and e["betrachter"] not in s.kern.betrachter:
                continue
            self.ereignis(dict(e), quelle, s)

    def wirkungen(self, aus, quelle, s):
        groessen, pty = aus["groessen"], aus["pty"]
        ziele = [x for x in self.sitz.values()] if (m("eingabe_an_alle_sitzungen") and pty) else [s]
        schritte = [("g", groessen), ("p", pty)]
        if m("pty_vor_groesse"):
            for w in pty:
                self.pty_schreiben(s, bytes.fromhex(w["hex"]))
            pty = []
            schritte = [("g", groessen), ("p", pty)]
        spaeter = []        # DI4 (Nachtrag 21.11.2026): the carrier events of a new size make the next step, after the messages of this one
        for art, liste in schritte:
            for w in liste:
                if art == "g":
                    if m("groesse_pty_nicht"):
                        ev = s.traeger.groesse(w["spalten"], w["zeilen"])
                    elif m("pty_vor_groesse"):
                        self.pty_flush(s)
                        time.sleep(0.15)
                        self.pty_groesse(s, w["spalten"], w["zeilen"])
                        ev = s.traeger.groesse(w["spalten"], w["zeilen"])
                    else:
                        self.pty_groesse(s, w["spalten"], w["zeilen"])
                        ev = s.traeger.groesse(w["spalten"], w["zeilen"])
                    for x in ev:
                        if m("groesse_ereignis_sofort"):
                            self.traeger_ereignis(x, s)
                        else:
                            spaeter.append(x)
                else:
                    b = bytes.fromhex(w["hex"])
                    if m("eingabe_cr_zu_lf"):
                        b = b.replace(b"\r", b"\n")
                    for z in ziele:
                        self.pty_schreiben(z, b)
        namen = sorted(aus["nachrichten"], key=lambda n: int(n[1:]) if n[1:].isdigit() else 0)
        for n in namen:
            c = self.clients.get(n)
            if c is None or c.tot or c.schliessen:
                continue
            for msg in aus["nachrichten"][n]:
                if self.lauf is not None:
                    art = msg.get("art")
                    if art == "Sitzungsliste":
                        # DI12: the list is the service's own; the base core's place in the order is kept, the cores of the sessions give none
                        if s is not self.basis or m("liste_beim_verbinden_fehlt"):
                            continue
                        msg = self.liste_msg()
                    elif art == "Agenten" and s is self.basis and not m("agenten_vom_raumkern"):
                        msg = {"t": self.jetzt(), "art": "Agenten", "agenten": self.agenten_liste()}     # DI-L1 point 3
                    elif s is not self.basis and art in ("Einstellungen", "Routen", "Anfragen", "Agenten", "Leeren"):
                        continue
                self.nachricht_an(c, msg, s)
        for ab in aus["ablehnungen"]:
            if m("proto_ablehnung_verschluckt") or quelle is None:
                continue
            ziele2 = list(self.clients.values()) if m("proto_ablehnung_an_alle") else [quelle]
            for c in ziele2:
                if not c.tot and not c.schliessen:
                    self.senden(c, rahmen(A_ABLEHNUNG, jbytes({"grund": ab["grund"]})))
        for x in spaeter:
            self.traeger_ereignis(x, s)

    def nachricht_an(self, c, msg, s):
        art = msg.get("art")
        if art in ("Abzug", "Rasteraenderung"):
            if s is not None and s.nr is None:
                return              # P25: a session without a raster number sends no frame (its number belongs to another session now)
            nr = s.nr
            if m("betr_nr_eins"):
                nr = 1
            if m("sitzung_nr_null") and self.lauf is not None:
                nr = 0
            try:
                roh = c.ver.nimm(nr, msg)
            except ra_ref.Fehler as f:
                self.fehler(c, "raster_" + f.code)
                return
            if roh is not None:
                self.senden(c, rahmen(A_RASTER, c.ver.verpacken(roh)))
        else:
            self.senden(c, rahmen(A_NACHRICHT, jbytes(msg)))
        self.melden(c)

    # ---------- carrier events -> core
    def traeger_ereignis(self, x, s):
        e = {"t": self.jetzt(), "sitzung": s.sid}
        e.update(x)
        self.ereignis(e, None, s)

    # ---------- the list of sessions (DI12)
    def liste_msg(self):
        aus = []
        for s in self.sitz.values():
            if m("beenden_entfernt_sitzung") and s.beendet_von_uns and s.zustand == "beendet":
                continue
            x = {"id": s.sid, "zustand": s.zustand, "nr": s.nr}     # P25: nr is the raster number, null once a later session took it
            for k in ("harness", "modell", "route", "kunde"):
                x[k] = "" if m("liste_felder_leer") else s.meta[k]
            if s.zustand == "beendet" and not m("liste_ohne_code") and s.kind_ende is not None:
                x["code"] = s.kind_ende
            # Nachtrag 26.11.2026, DI-L5 point 1
            x["name"] = s.name
            if not m("liste_ohne_ordner"):
                x["ordner"] = s.arbeitsordner
                x["projekt"] = projekt_von(s.arbeitsordner)
            if s.zustand == "beendet":
                x["abgestuerzt"] = s.abgestuerzt()
            if s.grund is not None:
                x["grund"] = s.grund
            if s.fortsetzung_von is not None:
                x["fortsetzung_von"] = s.fortsetzung_von
            if s.fortgesetzt_als is not None:
                x["fortgesetzt_als"] = s.fortgesetzt_als
            aus.append(x)
        return {"t": self.jetzt(), "art": "Sitzungsliste", "sitzungen": aus}

    def agenten_liste(self):
        """DI-L1 point 3: the workers in the order of the start."""
        aus = []
        for s in self.sitz.values():
            if s.orchestrator is None:
                continue
            x = {"worker": s.sid, "sitzung": s.orchestrator, "rolle": s.rolle, "modell": s.meta["modell"], "zustand": s.zustand, "nr": s.nr}
            if s.zustand == "beendet" and s.kind_ende is not None:
                x["code"] = s.kind_ende
            aus.append(x)
        return aus

    def liste_und_agenten(self):
        """After a change: the list to everyone, then the workers when their list changed (DI-L1 point 4), then the Bestand (DI-L5)."""
        self.liste_senden()
        neu = self.agenten_liste()
        if neu != self.agenten_letzt:
            self.agenten_letzt = neu
            for c in sorted(self.clients.values(), key=lambda x: x.nr):
                if not c.tot and not c.schliessen:
                    self.nachricht_an(c, {"t": self.jetzt(), "art": "Agenten", "agenten": neu}, None)
        self.bestand_schreiben()

    def bestand_schreiben(self):
        """DI-L5 point 3: a new file next to the Bestand, exclusive, 0600, fsync, rename, fsync of the folder."""
        pfad = (self.lauf or {}).get("bestand")
        if not pfad or m("bestand_vergessen"):
            return
        eintraege = []
        for s in self.sitz.values():
            if s.eintrag is None or s.start is None:
                continue
            x = {"id": s.sid, "harness": s.meta["harness"], "modell": s.meta["modell"], "route": s.meta["route"], "kunde": s.meta["kunde"],
                 "name": s.name, "ordner": s.arbeitsordner, "projekt": projekt_von(s.arbeitsordner), "zustand": s.zustand, "start": s.start}
            for k, v in (("code", s.kind_ende), ("grund", s.grund), ("orchestrator", s.orchestrator), ("rolle", s.rolle),
                         ("fortsetzung_von", s.fortsetzung_von), ("fortgesetzt_als", s.fortgesetzt_als)):
                if v is not None:
                    x[k] = v
            if s.zustand == "beendet":
                x["abgestuerzt"] = s.abgestuerzt()
            eintraege.append(x)
        roh = json.dumps({"fassung": 1, "gestartet": self.naechste, "sitzungen": eintraege}, ensure_ascii=False, sort_keys=True).encode("utf-8")
        neu = "%s.neu.%d" % (pfad, os.getpid())
        try:
            fd = os.open(neu, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                os.write(fd, roh)
                os.fsync(fd)
            finally:
                os.close(fd)
            os.rename(neu, pfad)
            dfd = os.open(os.path.dirname(pfad) or "/", os.O_RDONLY)
            try:
                os.fsync(dfd)
            finally:
                os.close(dfd)
        except OSError as e:
            sys.stderr.write("wb-dienst: bestand nicht geschrieben: %s\n" % (e.errno,))

    def vorgabe_laden(self):
        """Nachtrag 27.11.2026: the only load of the Firmenvorgabe in a run of the service. Invalid: the service runs, but without a gate."""
        pfad = (self.lauf or {}).get("firmenvorgabe")
        self.vorgabe = None
        if not pfad:
            return
        sha, code = vorgabe_pruefen(pfad)
        self.vorgabe = {"sha256": sha, "code": code, "gesperrt": False}
        if code is not None:
            sys.stderr.write("wb-dienst: firmenvorgabe nicht geladen: %s\n" % code)
            sys.stderr.flush()

    def bestand_lesen(self):
        """DI-L5 point 4: once at the start. Missing: empty. Invalid: exit code 2 (the service never replaces a file it cannot read)."""
        pfad = (self.lauf or {}).get("bestand")
        if not pfad:
            return
        try:
            st = os.lstat(pfad)
        except FileNotFoundError:
            return
        except OSError:
            raise StartFehler(2, "lauf.bestand nicht lesbar")
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.geteuid() or stat.S_IMODE(st.st_mode) != 0o600 or st.st_size > BESTAND_MAX:
            raise StartFehler(2, "lauf.bestand ungueltig")
        try:
            with open(pfad, "rb") as f:
                o = json.loads(f.read(BESTAND_MAX + 1).decode("utf-8"), parse_constant=_konstante)
        except (OSError, ValueError, UnicodeDecodeError):
            raise StartFehler(2, "lauf.bestand ungueltig")
        texte = ("harness", "modell", "route", "kunde", "name", "ordner", "projekt")
        if not (isinstance(o, dict) and o.get("fassung") == 1 and ganz(o.get("gestartet")) and o["gestartet"] >= 0
                and isinstance(o.get("sitzungen"), list) and len(o["sitzungen"]) <= 264):
            raise StartFehler(2, "lauf.bestand ungueltig")
        hoechste = 0
        for x in o["sitzungen"]:
            if not (isinstance(x, dict) and isinstance(x.get("id"), str) and re.fullmatch(r"s-[1-9][0-9]{0,8}", x["id"])
                    and all(isinstance(x.get(k), str) for k in texte) and x.get("zustand") in ("laeuft", "beendet")
                    and isinstance(x.get("start"), dict) and x["id"] not in self.sitz):
                raise StartFehler(2, "lauf.bestand ungueltig")
            hoechste = max(hoechste, int(x["id"][2:]))
            s = Sitzung(x["id"], None, list(self.lauf["groesse"]), {k: x[k] for k in ("harness", "modell", "route", "kunde")},
                        {"id": x["id"], "titel": "", "route": x["route"] or None, "kundenraum": self.cfg["betrachter"]["raum"],
                         "person": self.cfg["betrachter"]["person"], "groesse": list(self.lauf["groesse"])})
            s.name, s.arbeitsordner, s.start = x["name"], x["ordner"], x["start"]
            s.orchestrator, s.rolle = x.get("orchestrator"), x.get("rolle")
            s.fortsetzung_von, s.fortgesetzt_als = x.get("fortsetzung_von"), x.get("fortgesetzt_als")
            s.zustand = "beendet"
            if x["zustand"] == "laeuft" and not m("bestand_laeuft_bleibt"):
                s.ende_art, s.grund, s.kind_ende = "neustart", "dienst.neustart", None
            elif x["zustand"] == "laeuft":
                s.zustand = "laeuft"
            else:
                s.ende_art = "selbst" if x.get("abgestuerzt") else "beenden"
                s.kind_ende = x.get("code") if ganz(x.get("code")) else None
                s.grund = x.get("grund") if isinstance(x.get("grund"), str) else None
            self.enden += 1
            s.beendet_als = self.enden
            self.sitz[s.sid] = s
        self.naechste = max(o["gestartet"], hoechste)
        self.agenten_letzt = self.agenten_liste()

    def liste_senden(self, nur=None):
        if self.lauf is None:
            return
        for c in sorted(self.clients.values(), key=lambda x: x.nr):
            if c.tot or c.schliessen or (nur is not None and c is not nur):
                continue
            self.nachricht_an(c, self.liste_msg(), None)
            if m("ereignisliste_wiederholt"):
                self.senden(c, rahmen(A_NACHRICHT, jbytes(self.ereignisliste(c))))

    # ---------- sending, queue, report (DI4)
    def grenze(self):
        """DI5 (decision of 19.11.2026 F4): hard upper bound of a viewer's queue in bytes; Startdatei warteschlange_max lowers it in the test mode only."""
        if m("stau_grenze_fehlt") or (m("stau_grenze_nur_pruefmodus") and not self.cfg["pruefmodus"]):
            return 1 << 62
        if m("stau_grenze_vorgabe_klein") and "wmax_gesetzt" not in self.cfg:
            return WARTESCHLANGE_MAX // 2
        return self.cfg["wmax"]

    def zu_gross(self, c):
        g = self.grenze()
        if m("stau_grenze_ein_byte_frueh"):
            return c.qlen >= g
        if m("stau_grenze_ein_byte_spaet"):
            return c.qlen > g + 1
        return c.qlen > g

    def abtrennen(self, c):
        """The queue broke the bound: drop it, tell the core `Trennen`, close, one line in the protocol (stderr)."""
        if not m("stau_grenze_kein_protokoll"):
            ks = [s for s in self.sitz.values()]
            zusatz = (" titel=%s" % ks[0].kern.sitzungen[ks[0].sid]["titel"]) if (m("stau_grenze_protokoll_mit_inhalt") and ks) else ""
            print("wb-dienst: betrachter %s getrennt: rueckstau_gerissen (ausstehend %d Byte, Grenze %d Byte)%s" % (c.name, c.qlen, self.grenze(), zusatz),
                  file=sys.stderr, flush=True)
        ziele = [x for x in self.clients.values()] if m("stau_grenze_alle_trennen") else [c]
        self.neu_nach_riss = True
        self.riss_nr = c.nr
        for x in ziele:
            x.queue.clear()
            x.qlen = 0
            if m("stau_grenze_ohne_trennen"):
                x.tot = True
                try:
                    self.sel.unregister(x.sock)
                except (KeyError, ValueError):
                    pass
                try:
                    x.sock.close()
                except OSError:
                    pass
                self.clients.pop(x.name, None)
            else:
                self.getrennt(x)

    def senden(self, c, daten):
        if c.tot:
            return
        if c.stumm and m("stau_warteschlange_verwerfen"):
            return
        if os.environ.get("DI_RAHMENLOG") and c.stumm:
            with open(os.environ["DI_RAHMENLOG"], "a") as f:
                f.write("%s %d %s\n" % (c.name, len(daten), daten[:60].hex()))
        c.queue.append(memoryview(daten))
        c.qlen += len(daten)
        self.schreiben(c)
        if not c.tot and self.zu_gross(c):
            self.abtrennen(c)

    def zaehler(self, c):
        if m("stau_ohne_kopf"):
            return max(0, c.qlen - 5 * len(c.queue))
        return c.qlen

    def schreiben(self, c):
        if m("stau_alles_blockiert"):
            if any(x.stumm for x in self.clients.values()):
                return
        while c.queue and not c.stumm and not c.tot:
            buf = c.queue[0]
            try:
                n = c.sock.send(buf)
            except (BlockingIOError, InterruptedError):
                break
            except OSError:
                self.getrennt(c)
                return
            c.qlen -= n
            if n < len(buf):
                c.queue[0] = buf[n:]
                break
            c.queue.popleft()
        if c.schliessen and not c.queue:
            self.getrennt(c)

    def melden(self, c):
        if m("stau_nicht_gemeldet") or c.tot or c.schliessen:
            return
        e = {"t": self.jetzt(), "art": "Socket", "betrachter": c.name, "ausstehend": self.zaehler(c)}
        c.zuletzt = e["ausstehend"]
        for s in self.kerne():       # the queue is the viewer's, one number for the core of every set (DI12)
            self.arbeit.append((dict(e), None, s))

    def flush_alle(self):
        for c in list(self.clients.values()):
            if c.queue and not c.stumm and not c.tot:
                vorher = c.qlen
                self.schreiben(c)
                if c.qlen < vorher and not (m("stau_bericht_nur_wachsen")) and not (m("stau_ohne_leerbericht") and c.qlen == 0):
                    self.melden(c)
        while self.arbeit:
            e2, q2, s2 = self.arbeit.popleft()
            self.wirkungen(self.schritt(e2, q2, s2), q2, s2)

    # ---------- viewers
    def annehmen(self):
        try:
            s, _ = self.listener.accept()
        except (BlockingIOError, InterruptedError):
            return
        s.setblocking(False)
        self.folge += 1 if not m("betr_name_wiederverwenden") else 0
        if m("betr_name_wiederverwenden"):
            frei = 1
            while "b%d" % frei in self.clients:
                frei += 1
            nr = frei
        else:
            nr = self.folge
        c = Client(s, "b%d" % nr, nr, self.cfg["deflate"])
        self.clients[c.name] = c
        self.sel.register(s, selectors.EVENT_READ, ("c", c))
        b = self.cfg["betrachter"]
        self.ereignisliste_senden(c, zuerst=True)
        self.ereignis_alle({"t": self.jetzt(), "art": "Verbinden", "betrachter": c.name, "person": b["person"], "raum": b["raum"],
                            "ansicht": b["ansicht"]})
        if not m("startwahl_nicht_beim_verbinden"):
            self.startwahl_senden(nur=c)          # the last message of the connect set: Einstellungen, Routen, Sitzungsliste, Anfragen, Agenten, Startwahl
        self.ereignisliste_senden(c, zuerst=False)

    def lokal_annehmen(self):
        """DI-L11 points 2 and 4: the uid of the partner comes from the kernel (SO_PEERCRED, LOCAL_PEERCRED) at connect; then the hello."""
        try:
            hs, _ = self.lokal_hoerer.accept()
        except (BlockingIOError, InterruptedError):
            return
        uid = None
        if not m("lokal_ohne_peerpruefung"):
            try:
                if sys.platform.startswith("linux"):
                    _, uid, _ = struct.unpack("3i", hs.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
                else:
                    roh = hs.getsockopt(0, 1, 76)          # SOL_LOCAL, LOCAL_PEERCRED: struct xucred
                    _, uid = struct.unpack("<Ii", roh[:8])
            except OSError:
                uid = None
            if uid is None:
                hs.close()
                sys.stderr.write("wb-dienst: lokal abgewiesen: pk.peer_unmessbar\n")
                return
        hs.setblocking(False)
        self.lokal_warten[hs] = [b"", time.monotonic() + HALLO_FRIST, uid]
        self.sel.register(hs, selectors.EVENT_READ, ("H", hs))

    def lokal_abweisen(self, hs, grund):
        self.lokal_warten.pop(hs, None)
        try:
            self.sel.unregister(hs)
        except (KeyError, ValueError):
            pass
        hs.close()
        sys.stderr.write("wb-dienst: lokal abgewiesen: %s\n" % grund)

    def lokal_hallo(self, hs):
        """peerkanal stages 3 to 7: exactly the four bytes of the hello, the uid of the service, a name for the uid; then the answer."""
        w = self.lokal_warten.get(hs)
        if w is None:
            return
        try:
            d = hs.recv(4 - len(w[0]))
        except (BlockingIOError, InterruptedError):
            return
        except OSError:
            d = b""
        w[0] += d
        if not HALLO.startswith(w[0]):
            return self.lokal_abweisen(hs, "pk.hallo_falsch")
        if not d:
            return self.lokal_abweisen(hs, "pk.hallo_fehlt")
        if len(w[0]) < 4:
            return
        uid = w[2]
        if uid is not None and uid != os.geteuid():
            return self.lokal_abweisen(hs, "pk.uid_nicht_erlaubt")
        try:
            name = pwd.getpwuid(uid).pw_name if uid is not None else None
        except KeyError:
            return self.lokal_abweisen(hs, "aufrufer:konto-unbekannt")
        self.lokal_warten.pop(hs, None)
        self.sel.unregister(hs)
        hs.setblocking(True)
        try:
            hs.sendall(HALLO)
        except OSError:
            hs.close()
            return
        hs.setblocking(False)
        self.folge += 1
        c = Client(hs, "b%d" % self.folge, self.folge, self.cfg["deflate"])
        c.wer = {"system": "linux" if sys.platform.startswith("linux") else "macos", "uid": uid, "sid": None, "name": name}
        self.clients[c.name] = c
        self.sel.register(hs, selectors.EVENT_READ, ("c", c))
        b = self.cfg["betrachter"]
        self.ereignisliste_senden(c, zuerst=True)
        self.ereignis_alle({"t": self.jetzt(), "art": "Verbinden", "betrachter": c.name, "person": b["person"], "raum": b["raum"],
                            "ansicht": "fenster"})
        if not m("startwahl_nicht_beim_verbinden"):
            self.startwahl_senden(nur=c)
        self.ereignisliste_senden(c, zuerst=False)

    def getrennt(self, c):
        if c.tot:
            return
        c.tot = True
        try:
            self.sel.unregister(c.sock)
        except (KeyError, ValueError):
            pass
        try:
            c.sock.close()
        except OSError:
            pass
        self.clients.pop(c.name, None)
        if not m("betr_trennen_nicht_gemeldet"):
            self.ereignis_alle({"t": self.jetzt(), "art": "Trennen", "betrachter": c.name})

    def fehler(self, c, code):
        """A protocol violation: one Fehler frame behind the pending frames, then the connection ends (DI3)."""
        if c.tot or c.schliessen:
            return
        c.rbuf.clear()
        if m("proto_fehler_ohne_trennen"):
            self.senden(c, rahmen(A_FEHLER, jbytes({"fehler": code})))
            return
        c.stumm = False
        c.schliessen = True
        c.gesperrt = True
        if not m("betr_trennen_nicht_gemeldet"):
            self.ereignis_alle({"t": self.jetzt(), "art": "Trennen", "betrachter": c.name})
        self.senden(c, rahmen(A_FEHLER, jbytes({"fehler": code})))

    def lesen(self, c):
        try:
            d = c.sock.recv(65536)
        except (BlockingIOError, InterruptedError):
            return
        except OSError:
            d = b""
        if not d:
            if c.rbuf and m("proto_abgeschnitten_absturz"):
                raise RuntimeError("abgeschnittener Rahmen")
            self.getrennt(c)
            return
        c.rbuf += d
        while not c.tot and not c.gesperrt:
            if len(c.rbuf) < 4:
                return
            n = struct.unpack(">I", bytes(c.rbuf[:4]))[0]
            if n == 0 and not m("proto_laenge_null_ok"):
                return self.fehler(c, "laenge_ungueltig")
            if n > MAX_HINEIN and not m("proto_laenge_ohne_grenze"):
                return self.fehler(c, "rahmen_zu_gross")
            if len(c.rbuf) < 4 + n:
                return
            if n == 0:
                del c.rbuf[:4]
                continue
            art, nutz = c.rbuf[4], bytes(c.rbuf[5:4 + n])
            del c.rbuf[:4 + n]
            self.rahmen_verarbeiten(c, art, nutz)

    def json_nutz(self, c, nutz):
        try:
            return json.loads(nutz.decode("utf-8"), parse_constant=_konstante)
        except (ValueError, RecursionError):
            if not m("proto_json_fehler_bleibt"):
                self.fehler(c, "json_ungueltig")
            return KEIN

    def ablehnen(self, c, grund):
        self.senden(c, rahmen(A_ABLEHNUNG, jbytes({"grund": grund})))

    def rahmen_verarbeiten(self, c, art, nutz):
        pm = self.cfg["pruefmodus"] or m("proto_pruef_ohne_modus")
        if art == A_EREIGNIS:
            o = self.json_nutz(c, nutz)
            if o is KEIN:
                return
            if not isinstance(o, dict) or not isinstance(o.get("art"), str):
                return self.fehler(c, "ereignis_ungueltig")
            if self.weiche(c, o["art"])[0] == "fehler":
                if not (m("proto_traegerereignis_ok") and o["art"] in ("Ausgabe", "Kindende", "Socket", "Modus", "Titel", "Zwischenablage", "Takt")) \
                        and not (m("proto_verbinden_ok") and o["art"] in ("Verbinden", "Trennen")):
                    if m("lauf_starten_ohne_lauf_ablehnung") and o["art"] in ("Starten", "Beenden"):
                        return self.ablehnen(c, "start_form")
                    return self.fehler(c, "art_nicht_erlaubt")
            if "t" in o and not m("proto_t_vom_client"):
                return self.fehler(c, "feld_verboten")
            if "betrachter" in o and not m("proto_betrachter_vom_client"):
                return self.fehler(c, "feld_verboten")
            # Nachtrag 26.11.2026, DI-L11 point 7: no account from the view, on both ways
            if any(k in o for k in FELD_VERBOTEN if not (k == "konto" and m("konto_aus_ereignis"))):
                return self.fehler(c, "feld_verboten")
            # Nachtrag 27.11.2026, Kanal zur Stelle: `nachweis` in a Freigabe is a field of DI3, the service builds the proof itself
            if o["art"] == "Freigabe" and "nachweis" in o:
                return self.fehler(c, "feld_verboten")
            # Nachtrag 27.11.2026 (DI-E5 point 4.2): the fixed refusals of the switch, right after feld_verboten
            w_ = self.weiche(c, o["art"])
            if w_[0] == "ab":
                return self.ablehnen(c, w_[1])
            if o["art"] in ("OrdnerLesen", "InhaltSuchen"):     # Nachtrag 26.11.2026 (L9): events of the service in both modes
                return self.e_l9(c, o)
            e = dict(o)
            if not (m("proto_t_vom_client") and "t" in o):
                e["t"] = self.jetzt()
            if not (m("proto_betrachter_vom_client") and "betrachter" in o):
                e["betrachter"] = c.name
            if self.lauf is not None and e["art"] == "Starten":
                if m("lokal_ohne_starten") and c.wer:
                    return self.fehler(c, "art_nicht_erlaubt")
                self.starten(c, o)
                self.flush_alle()
                return
            if self.lauf is not None and e["art"] == "Beenden":
                self.beenden_ereignis(c, o)
                self.flush_alle()
                return
            if self.lauf is not None and e["art"] in ("Umbenennen", "Loeschen", "Fortsetzen", "Wiederbeleben", "Freigabe"):
                getattr(self, "e_" + e["art"].lower())(c, o)       # Nachtrag 26.11.2026: DI-L3 and DI-L6, events of the service
                self.flush_alle()
                return
            if e["art"] in ("Abo", "Groesse") and ganz(e.get("spalten")) and not spalten_ok(e["spalten"]) and not m("groesse_eine_spalte_durchlassen"):
                # decision of 20.11.2026: sizes follow the core (2 to 1000 columns); the same reason as the core's, given before it sees the event
                self.senden(c, rahmen(A_ABLEHNUNG, jbytes({"grund": "groesse_ungueltig"})))
                return
            if m("stau_neuverbinden_ohne_abzug") and self.neu_nach_riss and e["art"] == "Abo" and c.nr > self.riss_nr:
                return
            s = self.s0
            if self.lauf is not None:
                # DI12: an event of a session goes to the core of exactly that session; an unknown or missing id is the service's own refusal
                if e["art"] in ("RaumWechsel", "EinstellungSetzen") and not m("lauf_raumwechsel_ok"):
                    return self.ablehnen(c, "nicht_im_lauf")
                sid = e.get("sitzung")
                s = self.sitz.get(sid) if isinstance(sid, str) else None
                if m("abo_immer_erste_sitzung") and e["art"] == "Abo" and self.sitz:
                    s = next(iter(self.sitz.values()))
                    e["sitzung"] = s.sid
                if s is None and e["art"] in ("RaumWechsel", "EinstellungSetzen"):
                    s = self.basis
                if s is None:
                    return self.ablehnen(c, "sitzung_unbekannt")
                # P25 (Abschnitt 19 Punkt 6): a session whose raster number went to a later session cannot be watched live any more
                if e["art"] == "Abo" and e.get("an") is not False and s.nr is None:
                    return self.ablehnen(c, "sitzung_ohne_nummer")
            aus = self.schritt(e, c, s)
            if self.lauf is not None and not aus["ablehnungen"] and s.sid is not None:
                abo_nachfuehren(c.abos, s.sid, e)
            if self.lauf is None and not aus["ablehnungen"] and e["art"] in ("Abo", "Groesse"):
                if e["art"] == "Abo" and e.get("an", True) is True and ganz(e.get("spalten")) and ganz(e.get("zeilen")):
                    c.abo = [e["spalten"], e["zeilen"]]
                elif e["art"] == "Abo" and e.get("an", True) is False:
                    c.abo = None
                elif e["art"] == "Groesse" and c.abo is not None and ganz(e.get("spalten")) and ganz(e.get("zeilen")):
                    c.abo = [e["spalten"], e["zeilen"]]
            self.wirkungen(aus, c, s)
            self.flush_alle()
            return
        if art in (A_UHR, A_STUMM, A_ZUSTAND, A_KERNFEHLER):
            if not pm:
                return self.fehler(c, "art_nicht_erlaubt")
            if art == A_UHR:
                o = self.json_nutz(c, nutz)
                if o is KEIN:
                    return
                if not (isinstance(o, dict) and set(o) == {"t"} and ganz(o["t"]) and o["t"] >= 0):
                    return self.fehler(c, "uhr_ungueltig")
                if o["t"] < self.t_ein and not m("uhr_rueckwaerts_ok"):
                    return self.fehler(c, "uhr_faellt")
                if m("uhr_ignoriert"):
                    return
                self.t_ein = max(o["t"], self.t_ein) if not m("uhr_rueckwaerts_ok") else o["t"]
                if not m("uhr_ohne_takt"):
                    self.ereignis_alle({"t": self.t_ein, "art": "Takt"})
                self.flush_alle()
            elif art == A_STUMM:
                o = self.json_nutz(c, nutz)
                if o is KEIN:
                    return
                if not (isinstance(o, dict) and set(o) == {"an"} and isinstance(o["an"], bool)):
                    return self.fehler(c, "stumm_ungueltig")
                if m("stau_stumm_ignoriert"):
                    return
                if o["an"]:
                    self.senden(c, rahmen(A_STUMM, b""))     # the receipt goes out before the viewer turns mute
                    c.stumm = True
                else:
                    c.stumm = False
                    self.flush_alle()
                    self.senden(c, rahmen(A_STUMM, b""))     # and behind the queue when it turns loud again
                self.flush_alle()
            elif art == A_ZUSTAND:
                self.senden(c, rahmen(A_ZUSTAND, jbytes(self.pruefzustand())))
            else:
                if self.lauf is not None:
                    # DI12 (V): in the multi-session mode the frame Kernfehler is not allowed; the rebuild per session is a duty of the build
                    if m("lauf_kernfehler_ok"):
                        return
                    return self.fehler(c, "art_nicht_erlaubt")
                if m("kernfehler_ohne_neubau"):
                    return
                self.kern_neu()
            return
        if m("proto_unbekannte_art_ignoriert"):
            return
        self.fehler(c, "art_unbekannt")

    def pruefzustand(self):
        if self.lauf is not None:
            return self.pruefzustand_lauf()
        z = self.s0.kern.zustand()
        sitz = [{k: s[k] for k in ("id", "titel", "zustand", "code", "groesse", "letzte_eingabe", "dialog", "bracketed_paste", "invers", "cursor") if k in s}
                for s in z["sitzungen"]]
        betr = {n: {k: b[k] for k in ("person", "raum", "abos", "ausstehend", "verfallen", "rueckstand")} for n, b in z["betrachter"].items()}
        return {"kern": {"sitzungen": sitz, "betrachter": betr},
                "dienst": {"betrachter": {n: dict({"warteschlange": c.qlen, "stumm": c.stumm}, **({"wer": c.wer} if c.wer else {}))
                                          for n, c in sorted(self.clients.items())},     # DI-L11 point 4: `wer` in both modes
                           "kind": {"ende": self.s0.kind_ende is not None, "code": self.s0.kind_ende},
                           "kernbau": self.s0.kern_zaehler, "traeger_zeilen": list(self.s0.traeger.gemeldet),
                           "warteschlange_max": self.grenze()}}

    def pruefzustand_lauf(self):
        """DI12: the state of the multi-session mode: the sessions in the order of their start; per viewer the abos of all cores, the queue of the service."""
        sitz = []
        betr = {}
        zb = self.basis.kern.zustand()["betrachter"]
        for n, b in zb.items():
            betr[n] = {"person": b["person"], "raum": b["raum"], "abos": [], "ausstehend": b["ausstehend"], "verfallen": 0, "rueckstand": {}}
        for s in self.sitz.values():
            z = s.kern.zustand()
            sitz += [{k: x[k] for k in ("id", "titel", "zustand", "code", "groesse", "letzte_eingabe", "dialog", "bracketed_paste", "invers", "cursor") if k in x}
                     for x in z["sitzungen"]]
            for n, b in z["betrachter"].items():
                if n in betr:
                    betr[n]["abos"] += b["abos"]
                    betr[n]["verfallen"] += b["verfallen"]
                    betr[n]["rueckstand"].update(b["rueckstand"])
        return {"kern": {"sitzungen": sitz, "betrachter": betr},
                "dienst": {"betrachter": {n: dict({"warteschlange": c.qlen, "stumm": c.stumm}, **({"wer": c.wer} if c.wer else {}))
                                          for n, c in sorted(self.clients.items())},
                           "kind": {s.sid: {"ende": s.kind_ende is not None, "code": s.kind_ende} for s in self.sitz.values()},
                           "kernbau": self.kernbau(), "traeger_zeilen": {s.sid: list(s.traeger.gemeldet) for s in self.sitz.values()},
                           "nummern": {s.sid: s.nr for s in self.sitz.values()}, "gestartet": self.naechste,
                           "warteschlange_max": self.grenze(), **self.vorgabe_zustand()}}

    def vorgabe_zustand(self):
        """Nachtrag 27.11.2026: `firmenvorgabe` in the answer to Zustand, only with lauf.firmenvorgabe."""
        v = getattr(self, "vorgabe", None)
        if v is None:
            return {}
        if m("vorgabe_neu_geladen"):
            sha, code = vorgabe_pruefen(self.lauf["firmenvorgabe"])
            v = dict(v, sha256=sha, code=code)
        return {"firmenvorgabe": dict(v)}

    def kern_neu(self):
        """An error of the core (DI5): build it again from the state of the carrier; every viewer gets a full Abzug (single mode only)."""
        s = self.s0
        self.kern_bauen(s, neu=True)
        t = self.jetzt()
        if s.kind_ende is not None:
            self.ereignis({"t": t, "art": "Kindende", "sitzung": s.sid, "code": s.kind_ende}, None, s)
        for n in sorted(self.clients, key=lambda x: int(x[1:])):
            c = self.clients[n]
            if c.schliessen or m("kernfehler_betrachter_verloren"):
                continue
            b = self.cfg["betrachter"]
            self.ereignis({"t": t, "art": "Verbinden", "betrachter": c.name, "person": b["person"], "raum": b["raum"], "ansicht": b["ansicht"]}, None, s)
            if c.abo is not None and not m("kernfehler_kein_abo"):
                self.ereignis({"t": t, "art": "Abo", "betrachter": c.name, "sitzung": s.sid, "an": True, "spalten": c.abo[0], "zeilen": c.abo[1]}, None, s)
        self.flush_alle()

    # ---------- Starten and Beenden (DI12)
    def ablehnen_pruefung(self, c, code):
        """A refusal of the check of the start structure (addendum of 20.11.2026): `grund` `start_pruefung`, `code` `start.<code>` and the fixed text of the stage."""
        text = STUFE_VON[code][1]
        p = {"grund": code if m("start_grund_code") else "start_pruefung"}
        if not m("ablehnung_ohne_code"):
            p["code"] = code if m("start_code_ohne_praefix") else "start." + code
        if not m("ablehnung_ohne_text"):
            p["text"] = TEXTE["start_form"] if m("start_text_form") else text
        self.senden(c, rahmen(A_ABLEHNUNG, jbytes(p)))

    def ablehnen_start(self, c, grund):
        """A refusal of Starten (addendum of 20.11.2026): `grund`, `code` (the same word; with the crate `start` the code `start.<code>`) and the fixed `text`."""
        p = {"grund": grund}
        if not m("ablehnung_ohne_code"):
            p["code"] = grund
        if not m("ablehnung_ohne_text"):
            p["text"] = TEXTE.get(grund, "")
        self.senden(c, rahmen(A_ABLEHNUNG, jbytes(p)))

    def einst_lesen(self):
        """The reference reads the settings file (path only from the Startdatei): a regular file of at most 1 048 576 bytes with a JSON object (`einst_aus_objekt`).
        Returns the pieces for the checks and the Startwahl or None."""
        try:
            st = os.stat(self.lauf["einstellungen"])
            if not stat.S_ISREG(st.st_mode) or st.st_size > 65536:
                return None
            with open(self.lauf["einstellungen"], "rb") as h:
                return einst_aus_objekt(json.loads(h.read().decode("utf-8"), parse_constant=_konstante))
        except (OSError, ValueError, RecursionError, TypeError):
            return None

    def einstellungen_lesen(self):
        """The pieces of the Startwahl (message): the harness table of contract 1, the routes (`schluessel`, `art`, `klasse`, `modell`: the text of the route or null),
        the customers and null last. None if the file cannot be read."""
        e = self.einst_lesen()
        if e is None:
            return None
        rr = []
        for r in e["routen"]:
            if m("startwahl_mit_adresse"):
                x = dict(r["roh"])            # the whole route entry, with whatever the file holds (an address, a key, a path)
            else:
                x = {"schluessel": r["schluessel"], "art": r["art"], "klasse": r["klasse"]}
            mo = r["modell"]
            if m("startwahl_modell_ignoriert"):
                mo = None
            elif m("startwahl_modell_leer") and mo is None:
                mo = ""
            elif m("startwahl_modell_fassung1_erfunden") and e["fassung"] == 1:
                mo = "auto"
            if not m("startwahl_ohne_modell"):
                x["modell"] = mo
            rr.append(x)
        if m("startwahl_routen_sortiert"):
            rr = sorted(rr, key=lambda r: r["schluessel"])
        ks = list(e["kunden"]) + ([] if m("startwahl_ohne_ohne_kunde") else [None])
        hs = [dict(h) for h in HARNESS_TABELLE if not (m("startwahl_ohne_copilot") and h["name"] == "copilot")]
        if m("startwahl_harness_falsch"):
            hs[0]["routenart"] = "lokal"
        return {"fassung": 1, "harness": hs, "routen": rr, "kunden": ks}

    def startwahl_msg(self, w=None):
        w = w if w is not None else self.einstellungen_lesen()
        if w is None and not m("startwahl_trotz_unlesbar"):
            return None
        if w is None:
            w = {"fassung": 1, "harness": [], "routen": [], "kunden": [None]}
        return dict({"t": self.jetzt(), "art": "Startwahl"}, **w)

    def startwahl_senden(self, nur=None, nur_wenn_geaendert=False):
        """DI12 / addendum of 20.11.2026: the message Startwahl (what the page needs for the start form) to the viewers; read from `lauf.einstellungen`."""
        if self.lauf is None or m("startwahl_fehlt"):
            return
        msg = self.startwahl_msg()
        if msg is None:
            return
        inhalt = {k: v for k, v in msg.items() if k != "t"}
        if nur_wenn_geaendert and inhalt == self.startwahl_letzt:
            return
        if nur is not None and inhalt != self.startwahl_letzt:
            nur = None                         # a viewer connects and the content differs from the last one sent: every viewer hears it
        self.startwahl_letzt = inhalt
        for c in sorted(self.clients.values(), key=lambda x: x.nr):
            if c.tot or c.schliessen or (nur is not None and c is not nur):
                continue
            if m("startwahl_nur_sender") and nur is None and c is not self.sender:
                continue
            self.nachricht_an(c, dict(msg), None)

    def laufende(self):
        return [s for s in self.sitz.values() if s.zustand == "laeuft"]

    def starten(self, c, o):
        L = self.lauf
        self.sender = c
        if (set(o) - START_EREIGNIS - START_WORKER or "start" not in o) and not (m("starten_fremdfeld_ok") and "start" in o):
            return self.ablehnen_start(c, "start_form")
        start = o["start"]
        if m("einst_vor_form") and self.einst_lesen() is None:
            return self.ablehnen_start(c, "einst_unlesbar")
        code, felder = start_pruefung(start)      # DI12: the one place of the check of the structure, stages 1 to 13 (`start::lesen_anfrage`)
        if code:
            return self.ablehnen_pruefung(c, code)
        # Nachtrag 26.11.2026, DI-L1 point 2: the orchestrator, after the form and before the settings file
        orch = o.get("orchestrator")
        rolle = o.get("rolle")
        if "orchestrator" in o or "rolle" in o:
            if not (isinstance(orch, str) and KENNUNG.fullmatch(orch)) or ("rolle" in o and not (isinstance(rolle, str) and KENNUNG.fullmatch(rolle))):
                return self.ablehnen_start(c, "start_form")
            os_ = self.sitz.get(orch)
            if os_ is None or os_.eintrag is None:
                if not m("worker_orchestrator_ungeprueft"):
                    return self.ablehnen_start(c, "orchestrator_unbekannt")
            elif os_.zustand != "laeuft":
                return self.ablehnen_start(c, "orchestrator_beendet")
            elif os_.orchestrator is not None:
                return self.ablehnen_start(c, "orchestrator_ist_worker")
            else:
                kunde = start.get("kunde") if isinstance(start.get("kunde"), str) else ""
                if kunde != os_.meta["kunde"] and not m("worker_fremder_kunde"):
                    return self.ablehnen_start(c, "orchestrator_fremder_kunde")
            if rolle is None:
                rolle = "worker"
        if not m("starten_einst_nicht_gelesen"):
            einst = self.einst_lesen()
            if einst is None:
                return self.ablehnen_start(c, "einst_unlesbar")     # the settings file (path only from the Startdatei) is read before the child starts
            if not m("startwahl_nicht_nach_aenderung"):
                self.startwahl_senden(nur_wenn_geaendert=True)
            code = stufen_pruefen(felder, einst)   # stages 14 to 17 (`start::pruefen`)
            if code:
                return self.ablehnen_pruefung(c, code)
        zahl = len(self.sitz) if m("sitzung_beendete_zaehlen") else len(self.laufende())
        grenze = SITZUNGEN_MAX + 1 if m("sitzung_neunte_ok") else (SITZUNGEN_MAX - 1 if m("sitzung_achte_abgelehnt") else SITZUNGEN_MAX)
        if zahl >= grenze:
            return self.ablehnen_start(c, "zu_viele_sitzungen")
        if m("sitzung_id_wieder"):
            n = 1
            while "s-%d" % n in {x.sid for x in self.laufende()}:
                n += 1
        else:
            n = self.naechste + 1
        # P25 and 15a (DI12 Punkt 15a, Mit der Rasternummer nach P25): with every raster number held the start is refused BEFORE the identifier loop,
        # so that 15a removes an old target list only for a start that is handed out (whether a number is free does not depend on the identifier)
        if self.rahmen_waehlen(self.naechste + 1, L.get("rahmen_max", NUMMER_MAX)) is None:
            return self.ablehnen_start(c, "nummern_erschoepft")
        nmax = L.get("nummer_max", NUMMER_MAX)
        sd = L.get("sitzungen")
        if m("sitzungen_pruefung_fehlt"):
            sd = None
        zl = None if m("ziellisten_pruefung_fehlt") else L.get("ziellisten")
        lz = None if m("laufzeit_nicht_geprueft") else L.get("laufzeit")
        mo = L.get("marker_ordner")
        gs = None if m("gespraeche_vergabe_ungeprueft") else L.get("gespraeche")
        if m("zaehldatei_ohne_marker_ordner") and mo is not None:
            mo = sd if sd is not None else zl          # the count files are looked for in a wrong folder
        if sd is None and zl is None and lz is None and mo is None and gs is None:
            if n > nmax and not m("sitzung_nr_zu_gross_ok"):
                return self.ablehnen_start(c, "nummern_erschoepft")
        else:
            # DI12 Punkte 13 und 15 (Nachtrag 2): a number is taken when ANY of its entries exists (lstat: any kind, a dead link too): `s-<n>` under lauf.sitzungen,
            # `s-<n>` under lauf.laufzeit, `s-<n>.zielliste.json` under lauf.ziellisten, `s-<n>.suche.json` and `s-<n>.modellweg.json` under lauf.marker_ordner
            # (an entry counts only with its key); only "not found" is free; nummer_max holds for every n
            if sd is not None and m("sitzungen_socket_ordner_geprueft"):
                sd = self.cfg["ordner"]
            erste = n
            while True:
                if n > nmax and not m("sitzung_nr_zu_gross_ok") and not (m("sitzungen_nummer_max_nicht_beim_ueberspringen") and n > erste):
                    return self.ablehnen_start(c, "nummern_erschoepft")
                pfade = []
                if sd is not None:
                    pfade.append(("sitzung", os.path.join(sd, "s-%d" % n)))
                if lz is not None:
                    pfade.append(("laufzeit", os.path.join(lz, "s-%d" % n)))
                if zl is not None:
                    pfade.append(("zielliste", os.path.join(zl, ("s-%d" if m("ziellisten_falscher_name") else "s-%d.zielliste.json") % n)))
                if mo is not None:
                    pfade.append(("zaehl", os.path.join(mo, "s-%d.suche.json" % n)))
                    if not m("zaehldatei_nur_suche"):
                        pfade.append(("zaehl", os.path.join(mo, "s-%d.modellweg.json" % n)))
                if gs is not None:
                    pfade.append(("gespraech", os.path.join(gs, "s-%d" % n)))      # Nachtrag 27.11.2026: the conversation of the number
                da, fehlt = False, False                # an entry that is there / an entry that is missing
                for art, pf in pfade:
                    stat_statt = m("sitzungen_stat_statt_lstat") if art == "sitzung" else m("ziellisten_stat_statt_lstat")
                    try:
                        st = (os.stat if stat_statt else os.lstat)(pf)
                    except FileNotFoundError:
                        fehlt = True
                        continue
                    except OSError:
                        if m("sitzungen_fehler_gilt_frei") if art == "sitzung" else m("ziellisten_fehler_gilt_frei"):
                            fehlt = True
                            continue
                        return self.ablehnen_start(c, "start_ordner")
                    if art == "sitzung" and m("sitzungen_nur_ordner") and not stat_mod.S_ISDIR(st.st_mode):
                        fehlt = True
                        continue
                    if art == "zielliste" and m("ziellisten_nur_regulaer") and not stat_mod.S_ISREG(st.st_mode):
                        fehlt = True
                        continue
                    da = True
                    if not m("eintrag_fehlt_ist_frei"):
                        break                           # the first entry that is there takes the number
                # DI12 Punkt 15a (Nachtrag 23.11.2026): the own regular target list that ALONE takes the number is removed (through the descriptor of
                # lauf.ziellisten), then the number is asked again; every other case keeps the number taken
                if da and zl is not None and not m("eintrag_fehlt_ist_frei") and self.alte_zielliste_raeumen(zl, n, pfade):
                    continue
                # right: taken when any entry is there; the mutant: free as soon as one entry is missing
                if (fehlt if m("eintrag_fehlt_ist_frei") else not da):
                    break
                n += 1
                if m("sitzungen_naechste_beim_ueberspringen"):
                    self.naechste = n
        # P25 (DI12 Punkt 4, Abschnitt 19): the raster number is separate from the identifier; chosen now, taken only after the child started
        nummer = self.rahmen_waehlen(n, L.get("rahmen_max", NUMMER_MAX))
        if nummer is None:
            return self.ablehnen_start(c, "nummern_erschoepft")
        sid = "s-%d" % n
        ordner = os.path.join(self.cfg["ordner"], sid)
        datei = os.path.join(ordner, "start.txt" if m("starten_datei_name") else "start.json")
        angelegt = False
        try:
            try:
                os.mkdir(ordner, 0o700)
            except FileExistsError:
                if not m("starten_ordner_ersetzen"):
                    raise
                shutil.rmtree(ordner)
                os.mkdir(ordner, 0o700)
            angelegt = True
            os.chmod(ordner, 0o755 if m("starten_ordner_modus_offen") else 0o700)
            ganz_start = dict(start) if isinstance(start, dict) else start
            if isinstance(ganz_start, dict) and not m("starten_datei_ohne_sitzung"):
                ganz_start["sitzung"] = sid                  # the service gives the id and writes it into the file (addendum of 20.11.2026)
                # addendum of 21.11.2026: with markers (read once at the start) every start.json carries them; the request never sets them
                mk = MARKER_DIENST
                if m("marker_je_start_neu_gelesen") and MARKER_PFAD is not None:
                    mk = marker_datei_lesen(MARKER_PFAD)
                if m("marker_ohne_datei") and mk is None:
                    mk = {"liste": ["X"], "ordner": "/"}
                if mk is not None and not m("marker_nie_gesetzt") and not (m("marker_nur_erste_sitzung") and self.naechste > 0) \
                        and not (m("marker_anfrage_ersetzt") and "marker" in ganz_start):
                    ganz_start["marker"] = mk
                if self.fortsetzung is not None and not m("fortsetzen_ohne_verweis"):
                    ganz_start["fortsetzung_von"] = self.fortsetzung.sid      # DI-L3 point 2
                elif isinstance(ganz_start.get("fassung"), Ganz) and ganz_start["fassung"].t == "2" or ganz_start.get("fassung") == 2:
                    ganz_start["fortsetzung_von"] = None                       # version 2 always carries the field (contract `start`)
            fd = os.open(datei, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            try:
                os.fchmod(fd, 0o644 if m("starten_datei_modus_offen") else 0o600)
                os.write(fd, kanon(ganz_start))
            finally:
                os.close(fd)
        except OSError:
            if angelegt:
                shutil.rmtree(ordner, ignore_errors=True)       # only a folder the service made itself; a folder that was there before stays
            if m("starten_nr_verbraucht"):
                self.naechste = n
            return self.ablehnen_start(c, "start_ordner")
        meta = {k: (start.get(k) if isinstance(start, dict) and isinstance(start.get(k), str) else "") for k in ("harness", "modell", "route", "kunde")}
        groesse = list(L["groesse"])
        if m("starten_groesse_vom_kind") and self.cfg["groesse"]:
            groesse = list(self.cfg["groesse"])
        argv = [L["programm"], datei] + (["x"] if m("starten_argv_zusatz") else [])
        env = dict(L["env"])
        if m("starten_env_erweitert"):
            env["EXTRA"] = "1"
        cwd = ordner if m("starten_cwd_ordner") else "/"
        try:
            pid, master = self.spawn(argv, env, cwd, groesse)
        except StartFehler:
            shutil.rmtree(ordner, ignore_errors=True)
            if m("starten_nr_verbraucht"):
                self.naechste = n
            self.ablehnen_start(c, "programm_nicht_startbar")
            if m("starten_fehler_trotzdem_gestartet"):
                self.nachricht_an(c, {"t": self.jetzt(), "art": "Gestartet", "sitzung": sid}, None)
            return
        self.naechste = n
        self.rahmen_nehmen(nummer)
        eintrag = {"id": sid, "titel": "", "route": meta["route"] or None, "kundenraum": self.cfg["betrachter"]["raum"],
                   "person": self.cfg["betrachter"]["person"], "groesse": groesse}
        s = Sitzung(sid, nummer, groesse, meta, eintrag)
        s.ordner = ordner
        # Nachtrag 26.11.2026: the fields of the list and of the Bestand
        s.start = copy.deepcopy(start)
        s.arbeitsordner = start.get("arbeitsordner") if isinstance(start.get("arbeitsordner"), str) else ""
        s.orchestrator, s.rolle = orch, (rolle if orch is not None else None)
        if self.fortsetzung is not None:
            alt = self.fortsetzung
            s.fortsetzung_von, s.name = alt.sid, alt.name
            alt.fortgesetzt_als = sid
        s.pid, s.master = pid, master
        s.kind_pid = pid
        st0 = proz_stand(pid)
        s.kind_start = st0["start"] if st0 else None
        os.set_blocking(master, False)
        self.sel.register(master, selectors.EVENT_READ, ("p", s))
        self.sitz[sid] = s
        self.kern_bauen(s)
        for cc in sorted(self.clients.values(), key=lambda x: x.nr):
            if cc.tot:
                continue
            b = self.cfg["betrachter"]
            self.ereignis({"t": self.jetzt(), "art": "Verbinden", "betrachter": cc.name, "person": b["person"], "raum": b["raum"],
                           "ansicht": b["ansicht"]}, None, s)
        gestartet = {"t": self.jetzt(), "art": "Gestartet", "sitzung": sid}

        def liste():
            if m("liste_nicht_nach_aenderung"):
                return
            if m("liste_nur_sender"):
                return self.liste_senden(nur=c)
            self.liste_und_agenten()

        def antwort():
            if m("gestartet_an_alle"):
                for cc in sorted(self.clients.values(), key=lambda x: x.nr):
                    if not cc.tot and not cc.schliessen:
                        self.nachricht_an(cc, dict(gestartet), None)
            else:
                self.nachricht_an(c, gestartet, None)
        if m("gestartet_vor_liste"):
            antwort()
            liste()
        else:
            liste()
            antwort()

    def alte_zielliste_raeumen(self, zl, n, pfade):
        """DI12 Punkt 15a: True when the target list of `n` was removed. Conditions, all at once: every other entry of `n` is NotFound (lstat);
        `zl` opens with O_DIRECTORY|O_NOFOLLOW and belongs to the own user (fstat); the list is a regular file of the own user (fstatat without
        following a link). Removed with unlinkat through the descriptor. In every doubt nothing is removed."""
        name = "s-%d.zielliste.json" % n
        for art, pf in pfade:
            if art == "zielliste":
                continue
            try:
                os.lstat(pf)
            except FileNotFoundError:
                continue
            except OSError:
                return False
            return False
        eigen = os.geteuid()
        try:
            fd = os.open(zl, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        except OSError:
            return False
        try:
            if os.fstat(fd).st_uid != eigen:
                return False
            st = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if not stat_mod.S_ISREG(st.st_mode) or st.st_uid != eigen:
                return False
            os.unlink(name, dir_fd=fd)
            return True
        except OSError:
            return False
        finally:
            os.close(fd)

    def rahmen_waehlen(self, n, rmax):
        """P25 (DI12 Punkt 4, Abschnitt 19): the raster number of a new session s-<n>. Free is the number of an ended session that no
        viewer subscribes; first a number never handed out in this run, then the free one whose session ended longest ago. None: all held."""
        frei = []
        for s in self.sitz.values():
            if s.eintrag is None or s.nr is None or s.beendet_als is None:
                continue
            if any(s.sid in cc.abos for cc in self.clients.values()):
                continue
            frei.append((s.beendet_als, s.nr))
        frei += self.frei_geloescht
        return rahmennummer(n, rmax, self.vergeben, frei)

    def rahmen_nehmen(self, nummer):
        """Takes the chosen number: the session that held it shows nr null from then on, every connection forgets the number (RA R13)."""
        for s in self.sitz.values():
            if s.eintrag is not None and s.nr == nummer:
                s.nr = None
        for cc in self.clients.values():
            cc.ver.vergessen(nummer)
        self.vergeben.add(nummer)
        self.frei_geloescht = [x for x in self.frei_geloescht if x[1] != nummer]

    def pid_pruefen(self, pid, s):
        """The protection against a reused or foreign pid; in doubt False."""
        if s.kind_start is None or pid == s.kind_pid or pid == os.getpid():
            return False
        st = proz_stand(pid)
        if st is None or st["zustand"] in ("Z", "X", "x"):
            return False
        if not m("ende_signal_ohne_leiter") and not (st["sitzung"] == pid and st["gruppe"] == pid):
            return False
        if not m("ende_signal_ohne_startzeit") and not st["start"] > s.kind_start:
            return False
        return True

    def sitzung_ende(self, s, dienstende=False):
        """DI12 point 12: the end of a session cleans up. First SIGTERM to the process group of the pid in suche.pid and modellweg.pid
        (only with the pid protection), then the folder `s-<n>` is removed without following a link. The one place of the rule."""
        ordner = s.ordner
        if not ordner:
            return
        if not dienstende:
            if m("ende_sitzung_ohne_loeschen"):
                return
            if m("ende_nur_bei_normalem_ende") and s.kind_ende != 0:
                return
        s.ordner = None
        if not re.fullmatch(r"s-[0-9]+", os.path.basename(ordner)):
            return

        def signale():
            if m("ende_signal_keine") or (dienstende and m("ende_dienstende_ohne_signal")):
                return
            for name in ("suche.pid", "modellweg.pid"):
                pid = pid_lesen(ordner, name)
                if pid is not None and self.pid_pruefen(pid, s):
                    try:
                        os.killpg(pid, signal.SIGTERM)
                    except OSError:
                        pass

        def loeschen():
            if m("ende_loeschen_folgt_link"):
                try:
                    for n in os.listdir(ordner):
                        p = os.path.join(ordner, n)
                        if os.path.islink(p):
                            ziel = os.path.realpath(p)
                            if os.path.isdir(ziel):
                                shutil.rmtree(ziel, ignore_errors=True)
                            else:
                                os.unlink(ziel)
                except OSError:
                    pass
            try:
                if stat.S_ISDIR(os.lstat(ordner).st_mode):
                    shutil.rmtree(ordner, ignore_errors=True)       # a link inside is removed as a link, never followed
                else:
                    os.unlink(ordner)
            except OSError:
                pass
        if m("ende_signal_nach_loeschen"):
            loeschen()
            signale()
        else:
            signale()
            loeschen()

    def e_umbenennen(self, c, o):
        """DI-L3 point 1."""
        if set(o) != {"art", "sitzung", "name"} or not isinstance(o.get("sitzung"), str):
            return self.ablehnen(c, "umbenennen_form")
        s = self.sitz.get(o["sitzung"])
        if s is None:
            return self.ablehnen(c, "sitzung_unbekannt")
        if not name_ok(o["name"]):
            return self.ablehnen(c, "name_ungueltig")
        s.name = o["name"]
        self.liste_und_agenten()

    def e_loeschen(self, c, o):
        """DI-L3 points 4 and 5: only an ended session, with the identifier as confirmation; never a file outside the service's folder."""
        if set(o) - {"art", "sitzung", "bestaetigung"} or not isinstance(o.get("sitzung"), str):
            return self.ablehnen(c, "loeschen_form")
        s = self.sitz.get(o["sitzung"])
        if s is None:
            return self.ablehnen(c, "sitzung_unbekannt")
        if s.eintrag is None:
            return self.ablehnen(c, "nicht_loeschbar")       # the session of `kind` (DI-L3 point 4)
        if s.zustand == "laeuft" and not m("loeschen_laufend"):
            return self.ablehnen(c, "sitzung_laeuft")
        if o.get("bestaetigung") != s.sid and not m("loeschen_ohne_bestaetigung"):
            return self.ablehnen(c, "bestaetigung_fehlt")
        worker = [x for x in self.sitz.values() if x.orchestrator == s.sid]
        if any(x.zustand == "laeuft" for x in worker):
            return self.ablehnen(c, "worker_laeuft")
        wurzel = (self.lauf or {}).get("gespraeche")
        if wurzel and not m("loeschen_gespraech_bleibt"):
            # Nachtrag 27.11.2026, DI-L3 point 5: first the conversations; an error leaves every entry as it is
            for x in [s] + worker:
                if x.fortgesetzt_als is not None and not m("loeschen_fortgesetzt_gespraech"):
                    continue        # the conversation belongs to the session that continues it
                fehler = gespraech_entfernen(wurzel, x.sid)
                if fehler is not None:
                    sys.stderr.write("wb-dienst: gespraech nicht geloescht: %s\n" % fehler)
                    sys.stderr.flush()
                    return self.ablehnen(c, "gespraech_nicht_geloescht")
        for x in [s] + worker:
            if x.nr is not None and x.beendet_als is not None:
                self.frei_geloescht.append((x.beendet_als, x.nr))
            for cc in self.clients.values():
                cc.abos.pop(x.sid, None)
                if x.nr is not None:
                    cc.ver.vergessen(x.nr)
            if x.master is not None:
                try:
                    os.close(x.master)
                except OSError:
                    pass
                x.master = None
            del self.sitz[x.sid]
        if m("loeschen_kennung_wieder") and s.sid == "s-%d" % self.naechste:
            self.naechste -= 1
        self.liste_und_agenten()

    def e_fortsetzen(self, c, o, art="fortsetzen"):
        """DI-L3 points 2 and 3: the way of Starten with the stored start structure, a new identifier and `fortsetzung_von`."""
        if set(o) != {"art", "sitzung"} or not isinstance(o.get("sitzung"), str):
            return self.ablehnen_start(c, art + "_form")
        s = self.sitz.get(o["sitzung"])
        if s is None:
            return self.ablehnen_start(c, "sitzung_unbekannt")
        if s.zustand == "laeuft" and not m("fortsetzen_laufend"):
            return self.ablehnen_start(c, "sitzung_laeuft")
        if art == "wiederbeleben" and not s.abgestuerzt():
            return self.ablehnen_start(c, "nicht_abgestuerzt")
        if s.fortgesetzt_als is not None:
            return self.ablehnen_start(c, "schon_fortgesetzt")
        if s.start is None:
            return self.ablehnen_start(c, "nicht_fortsetzbar")
        if s.start.get("fassung") in (1,) and not m("fortsetzen_fassung1_ok"):
            # Nachtrag 27.11.2026, DI-L3 point 2: a session of version 1 has no conversation to continue (contract `start`); before any folder
            return self.ablehnen_start(c, "nicht_fortsetzbar")
        o2 = {"art": "Starten", "start": copy.deepcopy(s.start)}
        if s.orchestrator is not None:
            o2["orchestrator"], o2["rolle"] = s.orchestrator, s.rolle
        self.fortsetzung = s
        try:
            self.starten(c, o2)
        finally:
            self.fortsetzung = None

    def grund_aus_plan(self, s, code):
        """Section 18 in the model: the plan `melden` of the fixed child stands for a valid line `<code> <grund>` of lauf (the reference
        has no report socket). Only a reason of the form of the line and the code of the plan count, as a line would."""
        try:
            plan = json.loads(dict(self.lauf.get("env", {})).get("KIND_PLAN", "{}")) if self.lauf is not None else {}
        except ValueError:
            return None
        p = plan.get(s.sid) or plan.get("*") or {}
        g = p.get("grund", "lauf.fehler")
        if p.get("modus") != "melden" or p.get("code") != code or not (isinstance(g, str) and re.fullmatch(r"[a-z0-9_.]{1,64}", g)):
            return None
        return g

    def e_wiederbeleben(self, c, o):
        return self.e_fortsetzen(c, o, "wiederbeleben")

    def weiche(self, c, art):
        """Nachtrag 27.11.2026 (DI-E5 point 4): the one answer of the switch for an art on this connection:
        ("fehler", "art_nicht_erlaubt"), ("ab", <fixed reason>) or ("an", None). The list of the connection is built from it."""
        lauf = self.lauf is not None
        lokal = c.wer is not None
        katalog = ERLAUBT | (set(EIGENE_LAUF) if lauf else set())
        if art not in katalog:
            return ("fehler", "art_nicht_erlaubt")
        if lauf and art in ("RaumWechsel", "EinstellungSetzen") and not m("lauf_raumwechsel_ok"):
            return ("ab", "nicht_im_lauf")
        if art == "Freigabe":
            if not lokal and not m("freigabe_ueber_bruecke"):
                return ("ab", "stelle_nicht_erreichbar" if m("freigabe_grund_falsch") else "freigabe_nur_lokal")
            if lokal and not (lauf and "stelle" in self.lauf) and not m("freigabe_lokal_ohne_stelle"):
                return ("ab", "stelle_nicht_erreichbar")
        return ("an", None)

    def ereignisliste(self, c):
        """DI-E5 points 1 and 3: the arts the switch of this connection takes, in byte order."""
        wer = c.wer
        if m("ereignisliste_je_dienst"):
            c.wer = None
        try:
            liste = [a for a in KATALOG if self.weiche(c, a)[0] == "an"]
        finally:
            c.wer = wer
        if m("ereignisliste_mit_abgelehnter") and c.wer is None and "Freigabe" not in liste:
            liste.append("Freigabe")
        if m("ereignisliste_ohne_angenommene"):
            liste = [a for a in liste if a != "OrdnerLesen"]
        if m("ereignisliste_ohne_lauf_arten"):
            liste = [a for a in liste if a in ERLAUBT]
        if not m("ereignisliste_unsortiert"):
            liste = sorted(liste, key=lambda a: a.encode())
        return {"art": "Ereignisliste", "fassung": 1, "ereignisse": liste}

    def ereignisliste_senden(self, c, zuerst):
        """DI-E5 point 2: once per connection, the first frame to the viewer, before Verbinden; no report Socket (no core knows the viewer yet)."""
        if m("ereignisliste_fehlt") or zuerst == m("ereignisliste_nicht_zuerst"):
            return
        if c.tot or c.schliessen:
            return
        self.senden(c, rahmen(A_NACHRICHT, jbytes(self.ereignisliste(c))))

    def e_l9(self, c, o):
        """Nachtrag 26.11.2026 (L9) point 3, up to klasse_gesperrt: every read and every search ends there (point 6), so the reference
        never touches the file system. Order: form, session, room, limit and text, class."""
        feld = "pfad" if o["art"] == "OrdnerLesen" else "muster"
        sid, wert = o.get("sitzung"), o.get(feld)
        if len(o) != 3 or not isinstance(sid, str) or not isinstance(wert, str):
            return self.ablehnen(c, "form_ungueltig")
        s = self.sitz.get(sid)
        if s is None or s.kern is None:
            return self.ablehnen(c, "sitzung_unbekannt")
        z = s.kern.zustand()
        raum = (z.get("betrachter", {}).get(c.name) or {}).get("raum")
        kr = next((x.get("kundenraum") for x in z.get("sitzungen", []) if x.get("id") == sid), None)
        if not (isinstance(raum, str) and raum == kr):
            return self.ablehnen(c, "fremder_kundenraum")
        if feld == "pfad":
            if len(wert.encode()) > L9_GRENZE_PFAD:
                return self.ablehnen(c, "grenze")
            if wert:
                glieder = wert.split("/")
                if wert.startswith("/") or "\x00" in wert or "\\" in wert or any(
                        g == "" or g in (".", "..") or (len(g) >= 2 and g[0].isascii() and g[0].isalpha() and g[1] == ":") for g in glieder):
                    return self.ablehnen(c, "pfad_ungueltig")
                if len(glieder) > L9_GRENZE_GLIEDER:
                    return self.ablehnen(c, "grenze")
        else:
            if len(wert.encode()) > L9_GRENZE_MUSTER:
                return self.ablehnen(c, "grenze")
            if wert == "" or any(x in wert for x in ("\x00", "\n", "\r")):
                return self.ablehnen(c, "muster_ungueltig")
        return self.ablehnen(c, "klasse_gesperrt")

    def e_freigabe(self, c, o):
        """DI-L6 point 4: in the multi-session mode a decision goes to the decision point, never to a core. The reference has no
        channel to a decision point (DI-L6 point 6 needs its transport), so no core knows an open request: `anfrage_unbekannt`."""
        ids = o.get("anfragen")
        if set(o) - {"art", "anfragen", "nachweis", "entscheidung", "begruendung"} or not isinstance(ids, list) \
                or not all(isinstance(x, str) for x in ids):
            return self.ablehnen(c, "ereignis_ungueltig")
        ent = o.get("entscheidung")
        if ent is None:
            # zustandskern R-L6 point 1 (Projektentscheid, 27.11.2026): a missing field never leads to a release
            return self.ablehnen(c, "entscheidung_fehlt")
        if not isinstance(ent, str) or ent not in ("erteilen", "ablehnen"):
            return self.ablehnen(c, "ereignis_ungueltig")
        g = begruendung_form(o, ent == "ablehnen")
        if g:
            return self.ablehnen(c, g)
        if len(ids) != 1:
            return self.ablehnen(c, "sammelfreigabe")
        person = (self.cfg["anfang"].get("personen") or {}).get(self.cfg["betrachter"]["person"]) or {}
        if person.get("art") != "mensch":
            return self.ablehnen(c, "agent_kennung")
        if "nachweis" not in o or o["nachweis"] is None:
            return self.ablehnen(c, "nachweis_fehlt")
        if not isinstance(o["nachweis"], str):
            return self.ablehnen(c, "nachweis_ungueltig")
        return self.ablehnen(c, "anfrage_unbekannt")

    def beenden_ereignis(self, c, o):
        if set(o) - {"art", "sitzung"} or not isinstance(o.get("sitzung"), str):
            return self.ablehnen(c, "beenden_form")
        s = self.sitz.get(o["sitzung"])
        if s is None:
            if m("beenden_unbekannt_still"):
                return
            return self.ablehnen(c, "sitzung_unbekannt")
        if s.zustand != "laeuft":
            if m("beenden_beendete_ok"):
                return
            return self.ablehnen(c, "sitzung_beendet")
        s.beendet_von_uns = True
        if not m("beenden_ohne_signal") and s.pid:
            try:
                os.kill(s.pid, signal.SIGHUP)
            except OSError:
                pass
        s.kill_ab = time.monotonic() + KILL_FRIST

    # ---------- PTY and child (DI7)
    def pty_groesse(self, s, sp, ze):
        fcntl.ioctl(s.master, termios.TIOCSWINSZ, struct.pack("HHHH", ze, sp, 0, 0))

    def pty_schreiben(self, s, b):
        s.pty_aus += b
        self.pty_flush(s)

    def pty_flush(self, s):
        while s.pty_aus and s.master is not None:
            try:
                n = os.write(s.master, bytes(s.pty_aus))
            except (BlockingIOError, InterruptedError):
                break
            except OSError:
                s.pty_aus.clear()
                break
            del s.pty_aus[:n]

    def spawn(self, argv, env, cwd, groesse):
        """Starts the child in a new PTY of the given size: own session, the PTY as controlling terminal, exactly `env`; returns (pid, master)."""
        master, slave = os.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", groesse[1], groesse[0], 0, 0))
        r, w = os.pipe()
        pid = os.fork()
        if pid == 0:
            try:
                os.close(r)
                os.setsid()
                fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
                for fd in (0, 1, 2):
                    os.dup2(slave, fd)
                if slave > 2:
                    os.close(slave)
                os.close(master)
                if cwd is not None:
                    os.chdir(cwd)
                os.execve(argv[0], argv, env)
            except BaseException as e:  # noqa: BLE001
                try:
                    os.write(w, ("%s" % e).encode()[:200])
                finally:
                    os._exit(127)
        os.close(slave)
        os.close(w)
        meld = os.read(r, 300)
        os.close(r)
        if meld:
            os.waitpid(pid, 0)
            os.close(master)
            raise StartFehler(4, "Kind laesst sich nicht starten: " + meld.decode(errors="replace"))
        return pid, master

    def kind_starten(self):
        cfg = self.cfg
        env = dict(cfg["env"])
        if m("kind_env_erweitert"):
            env["EXTRA"] = "1"
        s = self.s0
        s.pid, s.master = self.spawn(cfg["argv"], env, None if m("kind_cwd_nicht_gesetzt") else cfg["cwd"], cfg["groesse"])
        os.set_blocking(s.master, False)
        self.sel.register(s.master, selectors.EVENT_READ, ("p", s))

    def kind_gelesen(self, s):
        try:
            d = os.read(s.master, 4096)
        except (BlockingIOError, InterruptedError):
            return
        except OSError:
            d = b""
        if d:
            if os.environ.get("DI_MITSCHNITT"):
                with open(os.environ["DI_MITSCHNITT"], "ab") as f:
                    f.write(d)
            for x in s.traeger.bytes_(d):
                self.traeger_ereignis(x, s)
            self.flush_alle()
            return
        self.sel.unregister(s.master)      # the master stays open until the end of the service (the size of an ended session can still be set, as in the single mode)
        try:
            _, st = os.waitpid(s.pid, 0)
        except ChildProcessError:
            st = 0
        if os.WIFSIGNALED(st):
            code = 128 + os.WTERMSIG(st) if not m("kind_signal_roh") else os.WTERMSIG(st)
        else:
            code = os.WEXITSTATUS(st)
        if m("kind_code_plus_eins"):
            code += 1
        s.pid = None
        s.kill_ab = None
        g_ = self.grund_aus_plan(s, code)
        if g_ is not None:
            s.grund = g_
            # Nachtrag 27.11.2026 (DI-L3 point 2): lauf ended before any child, the conversation is still that of the old session
            alt = self.sitz.get(s.fortsetzung_von) if s.fortsetzung_von is not None else None
            if alt is not None and alt.fortgesetzt_als == s.sid and not m("fortgesetzt_bleibt_ohne_kind"):
                alt.fortgesetzt_als = None
        s.kind_ende = code
        s.zustand = "beendet"
        s.ende_art = "beenden" if s.beendet_von_uns else "selbst"
        self.enden += 1
        s.beendet_als = self.enden
        self.sitzung_ende(s)           # DI12 point 12: before the list shows `beendet`
        self.traeger_ereignis({"art": "Kindende", "code": code}, s)
        if not m("liste_nicht_nach_aenderung"):
            self.liste_und_agenten()
        self.flush_alle()

    # ---------- main loop
    def laufen(self):
        cfg = self.cfg
        for s in self.kerne():
            self.kern_bauen(s)
        for s in self.sitz.values():
            if s.eintrag is not None and s.master is None and s.zustand == "beendet":
                # DI-L5 point 4: a session of the Bestand has ended; its core rejects every input with sitzung_beendet
                self.traeger_ereignis({"art": "Kindende", "code": s.kind_ende if s.kind_ende is not None else 0}, s)
        if self.s0 is not None:
            self.kind_starten()          # Exitcode 4 before anything listens; a later error kills the child again
        try:
            old = os.umask(0o177)
            ls = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            try:
                ls.bind(cfg["pfad"])
            except OSError as e:
                os.umask(old)
                raise StartFehler(1, "bind: %s" % e.strerror)
            os.umask(old)
            if not m("rechte_socket_modus_umask"):
                os.chmod(cfg["pfad"], 0o600)
            else:
                os.chmod(cfg["pfad"], 0o666)
            ls.listen(16)
            ls.setblocking(False)
            self.listener = ls
        except StartFehler:
            self.aufraeumen()
            raise
        self.tcp = None
        if m("rechte_tcp_hoerer"):
            self.tcp = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.tcp.bind(("127.0.0.1", 0))
            self.tcp.listen(1)
        self.sel.register(ls, selectors.EVENT_READ, ("l", None))
        if cfg.get("lokal"):
            old = os.umask(0o177)
            lh = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            try:
                lh.bind(cfg["lokal"]["pfad"])
            except OSError as e:
                os.umask(old)
                self.aufraeumen()
                raise StartFehler(1, "bind lokal: %s" % e.strerror)
            os.umask(old)
            os.chmod(cfg["lokal"]["pfad"], 0o600)
            lh.listen(LOKAL_INSTANZEN)
            lh.setblocking(False)
            self.lokal_hoerer = lh
            self.sel.register(lh, selectors.EVENT_READ, ("L", None))
        rd, wr = os.pipe()
        os.set_blocking(rd, False)
        os.set_blocking(wr, False)
        signal.set_wakeup_fd(wr)

        def stop(*_):
            self.ende = True
        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)
        self.sel.register(rd, selectors.EVENT_READ, ("s", None))
        while not self.ende:
            frist = self.frist()
            timeout = None
            if frist is not None and not cfg["pruefmodus"] and not m("monoton_ohne_takt"):
                timeout = max(0.0, (frist - self.jetzt()) / 1000.0)
            kills = [s.kill_ab for s in self.sitz.values() if s.kill_ab is not None]
            if kills:
                bis = max(0.0, min(kills) - time.monotonic())
                timeout = bis if timeout is None else min(timeout, bis)
            for c in list(self.clients.values()):
                if c.tot:
                    continue
                ev = 0
                if not c.gesperrt:
                    ev |= selectors.EVENT_READ
                if c.queue and not c.stumm:
                    ev |= selectors.EVENT_WRITE
                try:
                    self.sel.modify(c.sock, ev or selectors.EVENT_READ, ("c", c))
                except (KeyError, ValueError, OSError):
                    pass
            for key, mask in self.sel.select(timeout):
                art, c = key.data
                if art == "l":
                    self.annehmen()
                elif art == "L":
                    self.lokal_annehmen()
                elif art == "H":
                    self.lokal_hallo(c)
                elif art == "p":
                    self.kind_gelesen(c)
                elif art == "s":
                    try:
                        os.read(rd, 100)
                    except OSError:
                        pass
                elif not c.tot:
                    if mask & selectors.EVENT_WRITE:
                        vorher = c.qlen
                        self.schreiben(c)
                        if c.qlen < vorher and not m("stau_bericht_nur_wachsen") and not (m("stau_ohne_leerbericht") and c.qlen == 0) and not c.tot:
                            self.melden(c)
                            self.flush_alle()
                    if mask & selectors.EVENT_READ and not c.tot:
                        self.lesen(c)
            for s in self.sitz.values():
                if s.pty_aus:
                    self.pty_flush(s)
                if s.kill_ab is not None and s.pid and time.monotonic() >= s.kill_ab and not m("beenden_kein_sigkill"):
                    try:
                        os.kill(s.pid, signal.SIGKILL)       # Beenden: SIGHUP, up to 1 s, then SIGKILL (the end of the PTY reaps the child)
                    except OSError:
                        pass
                    s.kill_ab = None
            for hs, w in list(self.lokal_warten.items()):
                if time.monotonic() >= w[1]:
                    self.lokal_abweisen(hs, "pk.hallo_fehlt")
            if not cfg["pruefmodus"] and not m("monoton_ohne_takt"):
                f = self.frist()
                if f is not None and f <= self.jetzt():
                    self.ereignis_alle({"t": self.jetzt(), "art": "Takt"})
                    self.flush_alle()
        self.aufraeumen()

    def aufraeumen(self):
        if (self.lauf or {}).get("bestand"):
            for s in self.sitz.values():
                if s.zustand == "laeuft" and s.eintrag is not None:
                    s.zustand, s.ende_art, s.grund = "beendet", "dienstende", "dienst.ende"
            self.bestand_schreiben()
        for c in list(self.clients.values()):
            try:
                c.sock.close()
            except OSError:
                pass
        try:
            self.listener.close()
        except (AttributeError, OSError):
            pass
        if not m("rechte_socket_bleibt"):
            try:
                os.unlink(self.cfg["pfad"])
            except OSError:
                pass
        if self.lokal_hoerer is not None:       # DI-L11: the local way goes with the service
            try:
                self.lokal_hoerer.close()
                os.unlink(self.cfg["lokal"]["pfad"])
            except OSError:
                pass
        for hs in list(self.lokal_warten):
            hs.close()
        pids = [s.pid for s in self.sitz.values() if s.pid]
        if m("ende_kinder_bleiben") and self.lauf is not None:
            pids = [s.pid for s in self.sitz.values() if s.pid and s is self.s0]
        if not m("kind_bleibt_am_leben"):
            for pid in pids:
                try:
                    os.kill(pid, signal.SIGHUP)
                except OSError:
                    pass
            rest = list(pids)
            for _ in range(20):
                for pid in list(rest):
                    try:
                        r, _ = os.waitpid(pid, os.WNOHANG)
                    except ChildProcessError:
                        rest.remove(pid)
                        continue
                    if r:
                        rest.remove(pid)
                if not rest:
                    break
                time.sleep(0.05)
            for pid in rest:
                try:
                    os.kill(pid, signal.SIGKILL)
                    os.waitpid(pid, 0)
                except OSError:
                    pass
        if not m("ende_ordner_bleiben"):
            for s in self.sitz.values():
                self.sitzung_ende(s, dienstende=True)       # only what the service made itself (DI12)


# ---------------------------------------------------------------------------------------------- fixed child programs

def zeuge_main(args):
    """DI12 point 12: the witness of the cases. Ignores SIGHUP, writes `TERM` into the marker file when it gets SIGTERM and ends; writes its pid
    (with a line feed) into the ready file once the handlers stand; ends by itself after `sek` seconds."""
    marker, bereit, sek = args[0], args[1], float(args[2])

    def term(*_):
        try:
            with open(marker, "w") as h:
                h.write("TERM\n")
        finally:
            os._exit(0)
    signal.signal(signal.SIGTERM, term)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    tmp = bereit + ".tmp"
    with open(tmp, "w") as h:
        h.write("%d\n" % os.getpid())
    os.rename(tmp, bereit)
    time.sleep(sek)
    os._exit(0)


def zeuge_starten(ordner, art):
    """Starts a witness for the child `raeumen`: art `leiter` (own session), `gruppenleiter` (own group, the session of the child). Waits for
    its ready file; returns the pid."""
    import subprocess
    o = os.path.dirname(ordner)
    bereit = os.path.join(ordner, "zeuge.bereit")
    kw = {"start_new_session": True} if art == "leiter" else {"preexec_fn": os.setpgrp}
    proc = subprocess.Popen([os.path.abspath(sys.argv[0]), "--kind", "zeuge", os.path.join(o, "zeuge.term"), bereit, "6"],
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True, **kw)
    ende = time.monotonic() + 5
    while time.monotonic() < ende and not os.path.exists(bereit):
        time.sleep(0.01)
    return proc.pid


def kind_main(argv):
    """Fixed children. Each one sets the terminal raw without echo, writes BEREIT, reads one start byte, then acts."""
    import tty
    name = argv[0] if argv else ""
    args = argv[1:]
    if name == "zeuge":
        zeuge_main(args)
    tty.setraw(0)
    out = sys.stdout.buffer

    def schreibe(b):
        out.write(b)
        out.flush()

    def lies(n):
        d = b""
        while len(d) < n:
            x = os.read(0, n - len(d))
            if not x:
                break
            d += x
        return d

    if name == "lauf":
        # DI12: the fixed child of the sessions started with Starten. argv = [programm, <start.json>]; the program of the case is a wrapper the Pruefclient
        # writes (`@laufkind`) that runs `wb-dienst --kind lauf <argv…>`. What it does is chosen by the id in the start.json: KIND_PLAN is a JSON object
        # {"<id>": {"modus", "text", "code", "sek", "n"}, "*": …}.
        try:
            with open(args[1], "rb") as h:
                roh = h.read()
            sid = json.loads(roh.decode("utf-8")).get("sitzung")
        except (OSError, ValueError, IndexError, AttributeError):
            roh, sid = b"", None
        plan = json.loads(os.environ.get("KIND_PLAN", "{}"))
        p = plan.get(sid) or plan.get("*") or {"modus": "bericht"}
        modus = p.get("modus", "bericht")
        schreibe(b"BEREIT\r\n")
        lies(1)
        if modus == "bericht":
            try:
                st, sd = os.stat(args[1]), os.stat(os.path.dirname(args[1]))
                modi = "%04o/%04o" % (st.st_mode & 0o7777, sd.st_mode & 0o7777)
                name_ = os.path.basename(os.path.dirname(args[1])) + "/" + os.path.basename(args[1])
            except OSError:
                modi, name_ = "?", "?"
            try:
                start_txt = json.dumps(json.loads(roh.decode("utf-8")), ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            except ValueError:
                start_txt = "?"
            env = {k: v for k, v in os.environ.items() if k not in FILTER_ENV and k != "KIND_PLAN"}
            zeilen = ["START=" + start_txt, "MODI=" + modi, "NAME=" + name_,
                      "ARGV=%d/%s" % (len(args), os.path.basename(args[0])), "ENV=" + json.dumps(env, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                      "CWD=" + os.getcwd(), "ENDE"]
            for z in zeilen:
                schreibe((z + "\r\n").encode())
            time.sleep(float(p.get("sek", 0)))
        elif modus in ("marke", "eingabe"):
            schreibe(("MARKE %s\r\n" % p.get("text", "")).encode())
            if modus == "eingabe":
                d = lies(int(p.get("n", 1)))
                schreibe(b"EINGABE " + d.hex().encode() + b"\r\n")
            time.sleep(float(p.get("sek", 30)))
        elif modus == "ende":
            os._exit(int(p.get("code", 0)))
        elif modus == "melden":
            # Nachtrag 27.11.2026 (DI-L3 point 2): lauf ends with its own reason before the harness; the reference has no report socket
            # (section 18 point 7), its service takes the line from the same plan (`grund_aus_plan`)
            c_ = p.get("code")
            os._exit(c_ if isinstance(c_, int) and 1 <= c_ <= 255 else 1)
        elif modus == "scheitern":
            # DI12 point 12: the child writes the files of the session and ends with 78
            ordner = os.path.dirname(args[1])
            for n, t in (("suche.pid", "%d\n" % os.getpid()), ("modellweg.pid", "999999\n"), ("ca.key", "ATTRAPPE\n"), ("bundle.pem", "ATTRAPPE\n")):
                with open(os.path.join(ordner, n), "w") as h:
                    h.write(t)
            os._exit(78)
        elif modus == "raeumen":
            # DI12 point 12: pid files and links of a session. `pid`: leiter | gruppenleiter | vor | keine; `form`: ok | ohne_zeilenende |
            # leerzeichen | zwei_zeilen; `datei`: suche.pid | modellweg.pid; `ziel`: "link" puts the pid into <ordner>/ziel.pid and links to it;
            # `links`: a link to a file and one to the socket folder inside the session folder; `sek`: how long the child stays; `code`
            ordner = os.path.dirname(args[1])
            o = os.path.dirname(ordner)
            art = p.get("pid", "leiter")
            if p.get("links"):
                with open(os.path.join(o, "fremd.txt"), "w") as h:
                    h.write("FREMD")
                os.symlink(os.path.join(o, "fremd.txt"), os.path.join(ordner, "ca.key"))
                os.symlink(o, os.path.join(ordner, "bundle"))
            if art != "keine":
                if art == "vor":
                    with open(os.path.join(os.path.dirname(o), "vorzeuge.pid")) as h:
                        pid = int(h.read())
                else:
                    pid = zeuge_starten(ordner, art)
                text = {"ok": "%d\n", "ohne_zeilenende": "%d", "leerzeichen": " %d\n", "zwei_zeilen": "%d\n\n"}[p.get("form", "ok")] % pid
                datei = p.get("datei", "suche.pid")
                if p.get("ziel") == "link":
                    with open(os.path.join(o, "ziel.pid"), "w") as h:
                        h.write(text)
                    os.symlink(os.path.join(o, "ziel.pid"), os.path.join(ordner, datei))
                else:
                    with open(os.path.join(ordner, datei), "w") as h:
                        h.write(text)
            if "inhalt" in p:
                with open(os.path.join(ordner, p.get("datei", "suche.pid")), "w") as h:
                    h.write(p["inhalt"])
            schreibe(b"FERTIG\r\n")
            time.sleep(float(p.get("sek", 0)))
            os._exit(int(p.get("code", 0)))
        elif modus == "warte":
            signal.signal(signal.SIGHUP, signal.SIG_IGN)
            schreibe(("PID=%d\r\n" % os.getpid()).encode())
            time.sleep(float(p.get("sek", 20)))
        elif modus == "groesse":
            sz = os.get_terminal_size(0)
            schreibe(("GROESSE %d %d\r\n" % (sz.lines, sz.columns)).encode())
            time.sleep(float(p.get("sek", 30)))
        os._exit(0)
    if name == "still":
        time.sleep(float(args[0]))      # writes nothing, reads nothing (the start cursor before any output, DI11)
        os._exit(0)
    schreibe(b"BEREIT\r\n")
    lies(1)
    if name == "stufen":
        schreibe(bytes.fromhex(args[0]))
        for h in args[1:]:
            lies(1)
            schreibe(bytes.fromhex(h))
    elif name == "fuellen":
        if args and args[0].startswith("@"):
            # a prelude after the start byte (a title, for the check that the protocol line holds no session content): the viewer has seen
            # BEREIT, so it is there when the title comes, whatever the speed of the start
            schreibe(bytes.fromhex(args[0][1:]))
            args = args[1:]
        for k, laenge in enumerate(int(x) for x in args):
            if k:
                lies(1)
            schreibe(b"\x1b[H" + (chr(ord("a") + k % 26) * laenge).encode())
        lies(1)
        schreibe(b"\x1b[2;1HENDE\r\n")
    elif name == "osc52viele":
        wort = args[2].encode() if len(args) > 2 else b""
        roh = base64.b64encode((wort + b"k" * int(args[1]))[:int(args[1])]).decode()
        for k in range(int(args[0])):
            schreibe(b"\x1b]52;c;" + roh.encode() + b"\x07")
        schreibe(b"ENDE\r\n")
    elif name == "ausgabe":
        schreibe(bytes.fromhex(args[0]))
    elif name == "geteilt":
        schreibe(bytes.fromhex(args[0]))
        time.sleep(0.003)
        schreibe(bytes.fromhex(args[1]))
        if len(args) > 2:
            time.sleep(float(args[2]))
    elif name == "ausgabewarte":
        schreibe(bytes.fromhex(args[0]))
        time.sleep(float(args[1]))
    elif name == "ende":
        os._exit(int(args[0]))
    elif name == "zeilen":
        for i in range(1, int(args[0]) + 1):
            schreibe(("Z%06d\r\n" % i).encode())
    elif name == "klammer":
        schreibe(b"\x1b[?2004hKLAMMER\r\n")
        d = lies(int(args[0]))
        schreibe(b"EINGABE " + d.hex().encode() + b"\r\n")
    elif name == "eingabe":
        d = lies(int(args[0]))
        schreibe(b"EINGABE " + d.hex().encode() + b"\r\n")
    elif name == "bilder":
        for k in range(1, int(args[0]) + 1):
            lies(1)
            teile = [b"\x1b[H"]
            for j in range(4):
                teile.append(("Bild %04d Zeile %d " % (k, j)).encode() + b"x" * 40 + b"\r\n")
            schreibe(b"".join(teile))
        lies(1)
        schreibe(b"ENDE\r\n")
        if len(args) > 1:
            time.sleep(float(args[1]))
    elif name == "groesse":
        s = os.get_terminal_size(0)
        schreibe(("GROESSE %d %d\r\n" % (s.lines, s.columns)).encode())
    elif name == "umgebung":
        for n in args:
            schreibe(("%s=%s\r\n" % (n, os.environ.get(n, "-"))).encode())
        schreibe(("CWD=%s\r\n" % os.getcwd()).encode())
        n = sum(1 for k in os.environ if k not in ("CPATH", "LIBRARY_PATH", "MANPATH", "SDKROOT", "__CF_USER_TEXT_ENCODING"))
        schreibe(("ANZAHL=%d\r\n" % n).encode())
    elif name == "signal":
        os.kill(os.getpid(), int(args[0]))
        time.sleep(5)
    elif name == "titel":
        schreibe(b"\x1b]2;" + args[0].encode() + b"\x07fertig\r\n")
    elif name == "warte":
        signal.signal(signal.SIGHUP, signal.SIG_IGN)     # only SIGKILL ends this child: the service must see to it
        schreibe(("PID=%d\r\n" % os.getpid()).encode())
        time.sleep(float(args[0]))
    os._exit(0)


VORGABE_MAX = 1048576           # Nachtrag 27.11.2026: the largest Firmenvorgabe (the limit of a request to the helper)


def vorgabe_form_gut(o):
    """The form the reference checks (regelebenen 15.12.3 H4): an object, admins a list of accounts, leseklassen a list of entries."""
    if not isinstance(o, dict) or not isinstance(o.get("admins"), list) or not isinstance(o.get("leseklassen"), list):
        return False
    if not all(isinstance(a, str) and a for a in o["admins"]):
        return False
    return all(isinstance(e, dict) and set(e) == {"ebene", "wer", "klasse"} and all(isinstance(e[k], str) and e[k] for k in e)
               for e in o["leseklassen"])


def vorgabe_pruefen(pfad):
    """Nachtrag 27.11.2026: the whole check at the start. The folder and the file through descriptors, never a link, not writable for group or
    others, a regular file of at most VORGABE_MAX bytes with the form. Returns (sha256, None) or (None, code). The owner (helper user) and the
    parents up to / are part of the contract but not of the reference (a case cannot make a file of another account)."""
    try:
        roh = None
        if m("vorgabe_start_ungeprueft"):
            with open(pfad, "rb") as h:
                return hashlib.sha256(h.read()).hexdigest(), None
        try:
            fo = os.open(os.path.dirname(pfad) or "/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        except OSError:
            return None, "firmenvorgabe.datei_unsicher"
        try:
            if os.fstat(fo).st_mode & 0o022:
                return None, "firmenvorgabe.datei_unsicher"
            try:
                fd = os.open(os.path.basename(pfad), os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=fo)
            except OSError:
                return None, "firmenvorgabe.datei_unsicher"
            try:
                st = os.fstat(fd)
                if not stat.S_ISREG(st.st_mode) or st.st_mode & 0o022:
                    return None, "firmenvorgabe.datei_unsicher"
                if st.st_size > VORGABE_MAX:
                    return None, "firmenvorgabe.ladefehler"
                with os.fdopen(os.dup(fd), "rb") as h:
                    roh = h.read(VORGABE_MAX + 1)
            finally:
                os.close(fd)
        finally:
            os.close(fo)
        if len(roh) > VORGABE_MAX:
            return None, "firmenvorgabe.ladefehler"
        try:
            o = json.loads(roh.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            return None, "firmenvorgabe.ladefehler"
        if not vorgabe_form_gut(o):
            return None, "firmenvorgabe.ladefehler"
        return hashlib.sha256(roh).hexdigest(), None
    except OSError:
        return None, "firmenvorgabe.datei_unsicher"


def gespraech_baum_entfernen(eltern, name, tiefe=0):
    """Nachtrag 27.11.2026, DI-E4: like `loeschen::sitzung_entfernen` (DI12 point 12): below the open folder `eltern`, through descriptors only.
    A link or a file is removed itself, never followed; a folder is entered only when the opened folder is the one `stat` saw; errors leave the
    rest (the caller checks afterwards whether the entry is gone)."""
    try:
        st = os.stat(name, dir_fd=eltern, follow_symlinks=False)
    except OSError:
        return
    if stat.S_ISLNK(st.st_mode) and m("loeschen_folgt_verweis"):
        try:
            ziel = os.path.realpath(os.readlink(name, dir_fd=eltern))
            if os.path.isdir(ziel):
                for w, ds, fs in os.walk(ziel, topdown=False):
                    for x in fs:
                        os.unlink(os.path.join(w, x))
                    for x in ds:
                        os.rmdir(os.path.join(w, x))
            elif os.path.isfile(ziel):
                os.unlink(ziel)
        except OSError:
            pass
    if not stat.S_ISDIR(st.st_mode):
        try:
            os.unlink(name, dir_fd=eltern)
        except OSError:
            pass
        return
    if tiefe > GESPRAECH_TIEFE_MAX:
        return
    try:
        fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=eltern)
    except OSError:
        return
    try:
        st2 = os.fstat(fd)
        if (st2.st_dev, st2.st_ino) != (st.st_dev, st.st_ino):
            return
        for e in sorted(os.listdir(fd)):
            gespraech_baum_entfernen(fd, e, tiefe + 1)
    except OSError:
        return
    finally:
        os.close(fd)
    try:
        os.rmdir(name, dir_fd=eltern)
    except OSError:
        pass


GESPRAECH_TIEFE_MAX = 64        # like loeschen::TIEFE_MAX


def gespraech_entfernen(wurzel, sid):
    """Nachtrag 27.11.2026, DI-E4 point 4: the conversation `<lauf.gespraeche>/<sid>`. Missing (the folder or the entry): nothing, no error.
    The folder lauf.gespraeche must be a folder of the own account with mode 0700 (opened without following a link); else nothing is removed.
    Afterwards the entry must be gone (`rest` otherwise)."""
    try:
        g = os.open(wurzel, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    except FileNotFoundError:
        return None
    except OSError as e:
        return "gespraeche_unsicher" if e.errno in (errno.ELOOP, errno.ENOTDIR) else "oeffnen"
    try:
        st = os.fstat(g)
        if st.st_uid != os.geteuid() or stat.S_IMODE(st.st_mode) != 0o700:
            return "gespraeche_unsicher"
        namen = sorted(os.listdir(g)) if m("loeschen_gespraeche_ganz") else [sid]
        for n in namen:
            gespraech_baum_entfernen(g, n)
        try:
            os.stat(sid, dir_fd=g, follow_symlinks=False)
            return "rest"
        except FileNotFoundError:
            return None
    except OSError:
        return "oeffnen"
    finally:
        os.close(g)


def main(argv):
    if argv[1:2] == ["--kind"]:
        return kind_main(argv[2:])
    if argv[1:] == ["--gegenproben"]:
        print("\n".join(MUTANTEN))
        return 0
    if MUT and MUT not in MUTANTEN:
        print("dienst-ref: unbekannte Gegenprobe %r" % MUT, file=sys.stderr)
        return 3
    if len(argv) != 2:
        print("wb-dienst: Aufruf: wb-dienst <startdatei.json>", file=sys.stderr)
        return 2
    try:
        cfg = start_lesen(argv[1])
        rechte_pruefen(cfg)
        d = Dienst(cfg)
        d.bestand_lesen()          # Nachtrag 26.11.2026, DI-L5 point 4
        d.vorgabe_laden()          # Nachtrag 27.11.2026: the whole check of the Firmenvorgabe, once at the start
        d.laufen()
    except StartFehler as e:
        print("wb-dienst: " + e.text, file=sys.stderr)
        return e.code
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

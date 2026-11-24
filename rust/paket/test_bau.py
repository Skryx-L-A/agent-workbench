#!/usr/bin/env python3
"""Tests of rust/paket/bau.py (stages 4 to 6 of the deb build) on a small synthetic tree. Standard library only.

Run: python3 -I rust/paket/test_bau.py
It needs no dpkg and no cargo: the synthetic tree holds no ELF file, so `dpkg-shlibdeps` is not called.
"""
import json
import os
import re
import struct
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
RUST = os.path.dirname(HIER)
BAU = os.path.join(HIER, "bau.py")
PLATTFORM = "ubuntu-26.04-x64"
PROFIL = os.path.join(HIER, PLATTFORM, "usr.lib.werkbank.werkbank-einschluss")

LISTE = """programm usr/bin/werkbank-pruefen bin:werkbank-pruefen:werkbank-pruefen
programm usr/lib/werkbank/werkbank-einschluss bin:einschluss:werkbank-einschluss
daten etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss datei:paket/ubuntu-26.04-x64/usr.lib.werkbank.werkbank-einschluss
daten usr/share/werkbank/sbom.cdx.json erzeugt:sbom
"""
SBOM = {"components": [{"bom-ref": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.228", "name": "serde",
                        "version": "1.0.228", "licenses": [{"license": {"id": "MIT"}}]}]}


def elf(interp=False, needed=0):
    """A minimal ELF64 file: an optional PT_INTERP header and an optional dynamic section with `needed` DT_NEEDED entries."""
    koepfe = ([("interp", 3)] if interp else []) + ([("dyn", 2)] if needed else [])
    dyn = b"".join(struct.pack("<qQ", 1, 1) for _ in range(needed)) + struct.pack("<qQ", 0, 0)
    phoff = 64
    daten = phoff + 56 * len(koepfe)
    kopf = b"\x7fELF" + bytes([2, 1, 1, 0]) + bytes(8) + struct.pack("<HHIQQQIHHHHHH", 3, 62, 1, 0, phoff, 0, 0, 64, 56, len(koepfe), 64, 0, 0)
    tabelle = b"".join(struct.pack("<IIQQQQQQ", typ, 4, daten if name == "dyn" else 0, 0, 0, len(dyn) if name == "dyn" else 0, 0, 8)
                       for name, typ in koepfe)
    return kopf + tabelle + (dyn if needed else b"")


def lesen(pfad):
    with open(pfad, encoding="utf-8") as f:
        return f.read()


class Bau(unittest.TestCase):
    def setUp(self):
        self.wurzel = tempfile.mkdtemp(prefix="wb-bau-test-")
        self.rust = os.path.join(self.wurzel, "rust")
        ordner = os.path.join(self.rust, "paket", PLATTFORM)
        os.makedirs(ordner)
        os.makedirs(os.path.join(self.rust, "spec", "paket"))
        os.makedirs(os.path.join(self.rust, "sbom"))
        self.bindir = os.path.join(self.wurzel, "bin")
        os.makedirs(self.bindir)
        shutil.copy(os.path.join(RUST, "spec", "paket", "referenz.py"), os.path.join(self.rust, "spec", "paket"))
        for datei in ("ausschluss.txt", "postinst", "prerm", "postrm", "config", "templates"):
            shutil.copy(os.path.join(HIER, PLATTFORM, datei), ordner)
        with open(os.path.join(ordner, "lieferumfang.txt"), "w", encoding="utf-8") as f:
            f.write(LISTE)
        shutil.copy(PROFIL, ordner)
        with open(os.path.join(self.bindir, "werkbank-pruefen"), "w", encoding="ascii") as f:
            f.write("#!/bin/sh\nexit 0\n")
        with open(os.path.join(self.bindir, "werkbank-einschluss"), "wb") as f:
            f.write(elf())
        for crate in ("werkbank-pruefen", "einschluss"):
            with open(os.path.join(self.rust, "sbom", crate + ".cdx.json"), "w", encoding="ascii") as f:
                json.dump(SBOM, f)
        self.stufe = os.path.join(self.wurzel, "stufe")
        self.ausgabe = os.path.join(self.wurzel, "ausgabe")

    def tearDown(self):
        shutil.rmtree(self.wurzel, ignore_errors=True)

    def stufen(self, *extra):
        return subprocess.run(
            [sys.executable, "-I", BAU, "stufen", "--rust", self.rust, "--bindir", self.bindir, "--stufe", self.stufe,
             "--ausgabe", self.ausgabe, "--version", "0.1.0", *extra], capture_output=True, text=True, check=False)

    def profil_ersetzen(self, text):
        with open(os.path.join(self.rust, "paket", PLATTFORM, os.path.basename(PROFIL)), "w", encoding="utf-8") as f:
            f.write(text)

    def test_version_ohne_und_mit_geruest(self):
        for argumente, erwartet in (([], "0.1.0"), (["--geruest"], "0.1.0~geruest")):
            r = subprocess.run([sys.executable, "-I", BAU, "version", "--version", "0.1.0", *argumente], capture_output=True, text=True, check=False)
            self.assertEqual((r.returncode, r.stdout.strip()), (0, erwartet))

    def test_geruest_traegt_die_version_in_steuerdatei_und_dateinamen(self):
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 0, r.stderr)
        control = lesen(os.path.join(self.stufe, "DEBIAN", "control"))
        self.assertIn("Version: 0.1.0~geruest\n", control)
        self.assertEqual(sorted(n for n in os.listdir(self.ausgabe)),
                         ["werkbank_0.1.0~geruest_amd64.bericht.json", "werkbank_0.1.0~geruest_amd64.manifest.json",
                          "werkbank_0.1.0~geruest_amd64.stueckliste.tsv"])
        manifest = json.loads(lesen(os.path.join(self.stufe, "usr", "share", "werkbank", "manifest.json")))
        self.assertEqual((manifest["version"], manifest["bau"], manifest["merkmale"]), ("0.1.0", "test", ["geruest"]))

    def test_the_question_of_the_package_is_in_the_control_folder_and_debconf_is_a_dependency(self):
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 0, r.stderr)
        debian = os.path.join(self.stufe, "DEBIAN")
        for name in ("config", "templates", "postinst", "prerm", "postrm", "control"):
            self.assertTrue(os.path.isfile(os.path.join(debian, name)), name)
        depends = [z for z in lesen(os.path.join(debian, "control")).split("\n") if z.startswith("Depends:")][0]
        self.assertEqual(depends.count("debconf"), 1)
        for fest in ("python3", "bubblewrap", "apparmor", "ca-certificates", "systemd", "libssl3t64"):
            self.assertIn(fest, depends)
        for name in ("config", "templates"):
            self.assertEqual(lesen(os.path.join(debian, name)), lesen(os.path.join(HIER, PLATTFORM, name)))

    def test_feste_abhaengigkeit_libssl_steht_einmal(self):
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 0, r.stderr)
        depends = [z for z in lesen(os.path.join(self.stufe, "DEBIAN", "control")).split("\n") if z.startswith("Depends:")][0]
        self.assertEqual(depends.count("libssl3t64"), 1)

    def test_profil_mit_unconfined_stoppt_den_bau_mit_dem_code(self):
        text = lesen(PROFIL).replace("profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss {",
                                     "profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss flags=(unconfined) {")
        self.profil_ersetzen(text)
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 65)
        self.assertTrue(r.stderr.startswith("paket.profil_unconfined:"), r.stderr)

    def test_profil_am_system_bwrap_stoppt_den_bau(self):
        text = lesen(PROFIL).replace("profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss {",
                                     "profile werkbank-einschluss /usr/bin/bwrap {")
        self.profil_ersetzen(text)
        r = self.stufen("--geruest")
        self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.profil_system_bwrap"))

    def test_profil_am_pfad_des_laeufers_ist_ungelistet(self):
        ziel = os.path.join(self.rust, "paket", PLATTFORM, "usr.lib.werkbank.werkbank-lauf")
        shutil.copy(PROFIL, ziel)
        # A profile at the path of werkbank-lauf is not in the list, so the staged tree never holds it.
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertFalse(os.path.exists(os.path.join(self.stufe, "etc", "apparmor.d", "usr.lib.werkbank.werkbank-lauf")))

    def starter_schreiben(self, inhalt):
        with open(os.path.join(self.bindir, "werkbank-einschluss"), "wb") as f:
            f.write(inhalt)

    def test_statischer_starter_besteht(self):
        self.assertEqual(self.stufen("--geruest").returncode, 0)

    def test_starter_mit_interpreter_stoppt_den_bau(self):
        self.starter_schreiben(elf(interp=True))
        r = self.stufen("--geruest")
        self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.programm_nicht_statisch"))
        self.assertIn("usr/lib/werkbank/werkbank-einschluss", r.stderr)

    def test_starter_mit_bibliotheksliste_stoppt_den_bau(self):
        self.starter_schreiben(elf(needed=2))
        r = self.stufen("--geruest")
        self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.programm_nicht_statisch"))

    def test_starter_als_skript_ist_nicht_statisch(self):
        self.starter_schreiben(b"#!/bin/sh\nexit 0\n")
        r = self.stufen("--geruest")
        self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.programm_nicht_statisch"))

    def kern_einheit_stufen(self, text):
        """Stages the tree with the unit of the core service as the second entry of the list; `text` is the content of the unit (None: the real file)."""
        ordner = os.path.join(self.rust, "paket", PLATTFORM)
        if text is None:
            shutil.copy(os.path.join(HIER, PLATTFORM, "werkbank-kern.service"), ordner)
        else:
            with open(os.path.join(ordner, "werkbank-kern.service"), "w", encoding="utf-8") as f:
                f.write(text)
        with open(os.path.join(ordner, "lieferumfang.txt"), "w", encoding="utf-8") as f:
            f.write(LISTE + "dienst usr/lib/systemd/user/werkbank-kern.service datei:paket/ubuntu-26.04-x64/werkbank-kern.service\n")
        return self.stufen("--geruest")

    def test_die_echte_kern_einheit_besteht(self):
        r = self.kern_einheit_stufen(None)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(os.path.exists(os.path.join(self.stufe, "usr", "lib", "systemd", "user", "werkbank-kern.service")))

    def test_kern_einheit_mit_nonewprivileges_stoppt_den_bau(self):
        for zusatz in ("NoNewPrivileges=yes\n", "NoNewPrivileges=true\n", "RestrictAddressFamilies=AF_UNIX\n", "SystemCallFilter=@system-service\n"):
            with self.subTest(zusatz=zusatz):
                shutil.rmtree(self.stufe, ignore_errors=True)
                r = self.kern_einheit_stufen(lesen(os.path.join(HIER, PLATTFORM, "werkbank-kern.service")).replace("[Service]\n", "[Service]\n" + zusatz, 1))
                self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.kern_einheit_nonewprivs"), r.stderr)
                self.assertIn("usr/lib/systemd/user/werkbank-kern.service", r.stderr)

    def test_starter_kommt_aus_dem_statischen_ordner(self):
        statisch = os.path.join(self.wurzel, "statisch")
        os.makedirs(statisch)
        with open(os.path.join(statisch, "werkbank-einschluss"), "wb") as f:
            f.write(elf())
        self.starter_schreiben(elf(interp=True))  # the dynamic copy in the normal folder must not be taken
        r = self.stufen("--geruest", "--bindir-statisch", statisch)
        self.assertEqual(r.returncode, 0, r.stderr)

    def modi(self):
        """Mode of every entry of the staged tree (folders with a `/` at the end), without the file type bits."""
        aus = {"./": os.lstat(self.stufe).st_mode & 0o7777}
        for wurzel, ordner, dateien in os.walk(self.stufe):
            for name in ordner + dateien:
                pfad = os.path.join(wurzel, name)
                rel = os.path.relpath(pfad, self.stufe)
                aus[rel + "/" if os.path.isdir(pfad) else rel] = os.lstat(pfad).st_mode & 0o7777
        return aus

    def stufen_umask(self, umask, *extra):
        """`stufen` in a process that runs under `umask`, whatever the umask of the test is."""
        return subprocess.run(
            [sys.executable, "-I", BAU, "stufen", "--rust", self.rust, "--bindir", self.bindir, "--stufe", self.stufe,
             "--ausgabe", self.ausgabe, "--version", "0.1.0", *extra], capture_output=True, text=True, check=False,
            preexec_fn=lambda: os.umask(umask))

    def test_modi_sind_fest_und_unabhaengig_von_der_umask(self):
        ergebnisse = {}
        for umask in (0o022, 0o002, 0o000, 0o077):
            r = self.stufen_umask(umask, "--geruest")
            self.assertEqual(r.returncode, 0, f"umask {umask:03o}: {r.stderr}")
            ergebnisse[umask] = self.modi()
        for umask, modi in ergebnisse.items():
            self.assertEqual(modi, ergebnisse[0o022], f"umask {umask:03o}")
        modi = ergebnisse[0o002]
        for eintrag, modus in modi.items():
            if eintrag.endswith("/") or eintrag == "./":
                self.assertEqual(modus, 0o755, eintrag)
            elif eintrag in ("usr/bin/werkbank-pruefen", "usr/lib/werkbank/werkbank-einschluss",
                             "DEBIAN/postinst", "DEBIAN/prerm", "DEBIAN/postrm", "DEBIAN/config"):
                self.assertEqual(modus, 0o755, eintrag)
            else:
                self.assertEqual(modus, 0o644, eintrag)
        self.assertEqual(modi["usr/lib/werkbank/"], 0o755)
        self.assertEqual(modi["DEBIAN/"], 0o755)
        self.assertEqual(modi["DEBIAN/control"], 0o644)
        self.assertEqual(modi["DEBIAN/templates"], 0o644)

    def test_schreibbarer_eintrag_stoppt_den_bau_mit_dem_code(self):
        sys.path.insert(0, HIER)
        import importlib.util
        spec = importlib.util.spec_from_file_location("bau", BAU)
        bau = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(bau)
        ref = bau.referenz_laden(self.rust)
        os.makedirs(os.path.join(self.stufe, "usr", "lib", "werkbank"))
        with open(os.path.join(self.stufe, "usr", "lib", "werkbank", "x"), "w", encoding="ascii") as f:
            f.write("x")
        for pfad, modus in (("usr/lib/werkbank", 0o775), ("usr/lib/werkbank/x", 0o664), ("usr/lib", 0o757), ("usr/lib/werkbank/x", 0o646)):
            bau.modi_setzen(self.stufe, {})
            bau.modi_pruefen(ref, self.stufe)
            os.chmod(os.path.join(self.stufe, pfad), modus)
            with self.assertRaises(SystemExit) as e:
                bau.modi_pruefen(ref, self.stufe)
            self.assertEqual(e.exception.code, 65, (pfad, modus))

    def test_elternprofil_hat_genau_die_drei_leselinien(self):
        text = lesen(PROFIL)
        eltern = text[text.index("profile werkbank-einschluss /usr"):text.index("profile werkbank-einschluss-bwrap")]
        regeln = [z.strip() for z in eltern.split("\n")[1:] if z.strip() and not z.strip().startswith("#") and z.strip() != "}"]
        self.assertEqual(regeln, ["include <abstractions/base>", "/usr/lib/werkbank/werkbank-einschluss mr,",
                                  "/proc/sys/dev/tty/legacy_tiocsti r,", "/proc/sys/kernel/osrelease r,", "/{**,} r,",
                                  "/usr/bin/bwrap Px -> werkbank-einschluss-bwrap,"])

    def test_fehlende_quelle_ohne_geruest_stoppt(self):
        os.remove(os.path.join(self.bindir, "werkbank-einschluss"))
        r = self.stufen()
        self.assertEqual((r.returncode, r.stderr.split(":")[0]), (65, "paket.quelle_fehlt"))

    def test_geruest_laesst_fehlenden_starter_weg_und_nennt_ihn(self):
        os.remove(os.path.join(self.bindir, "werkbank-einschluss"))
        r = self.stufen("--geruest")
        self.assertEqual(r.returncode, 0, r.stderr)
        bericht = json.loads(lesen(os.path.join(self.ausgabe, "werkbank_0.1.0~geruest_amd64.bericht.json")))
        self.assertEqual([f["ziel"] for f in bericht["fehlend"]], ["usr/lib/werkbank/werkbank-einschluss"])

    def test_echte_listen_sind_gueltig(self):
        sys.path.insert(0, os.path.join(RUST, "spec", "paket"))
        import referenz
        ordner = os.path.join(HIER, PLATTFORM)
        positiv = referenz.liste_lesen(lesen(os.path.join(ordner, "lieferumfang.txt")), PLATTFORM)
        referenz.ausschluss_lesen(lesen(os.path.join(ordner, "ausschluss.txt")))
        ziele = [z for _, z, _ in positiv]
        self.assertEqual(len(ziele), len(set(ziele)))
        self.assertIn("usr/lib/werkbank/werkbank-einschluss", ziele)
        self.assertIn(("programm", "usr/bin/werkbank-anmelden", "bin:anmelden:werkbank-anmelden"), positiv)
        self.assertIn(("programm", "usr/lib/werkbank/einschluss-weiter", "bin:einschluss:einschluss-weiter"), positiv)
        self.assertIn(("programm", "usr/lib/werkbank/werkbank-lauf", "bin:lauf:werkbank-lauf"), positiv)
        self.assertEqual(sorted(referenz.STATISCH),
                         ["usr/lib/werkbank/einschluss-weiter", "usr/lib/werkbank/werkbank-einschluss"])
        self.assertIn("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss", ziele)
        self.assertFalse([z for z in ziele if z.endswith("werkbank-lauf") and z.startswith("etc/")])
        self.assertFalse([z for z in ziele if "test_werkbank_probelauf" in z])
        self.assertIsNone(referenz.profil_code({"text": lesen(PROFIL)}))
        self.assertIn(("programm", "usr/bin/werkbank-ca-zulassung", "datei:paket/ubuntu-26.04-x64/werkbank-ca-zulassung"), positiv)
        for skript in ("postinst", "prerm", "postrm", "config", "werkbank-ca-zulassung"):
            r = subprocess.run(["sh", "-n", os.path.join(ordner, skript)], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)


class LaufJson(unittest.TestCase):
    """The block `lauf.json` of postinst against the reference `referenz.laufkonfig` (the cases `laufkonfig-*`) and a fake tree."""

    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, os.path.join(RUST, "spec", "paket"))
        import referenz
        cls.ref = referenz
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        block = text[text.index("# --- lauf.json begin"):text.index("# --- lauf.json end")]
        cls.block = block
        cls.log = text[text.index("log() {"):text.index("fail() {")]  # the real function of postinst: the block writes its journal line through it
        cls.fail_text = text[text.index("fail() {"):text.index('[ "$1" = "configure" ]')]  # the real `fail` (code 19 of the check of the program folders)

    def pruefer(self, wurzel, verboten, ausgang=None):
        """The stand-in of `werkbank-pruefen --programmordner <heim> <ordner>`: refuses the folders of `verboten` (exit 1), allows every other (exit 0), or ends with `ausgang`.
        The real rule is in the Rust crate; the test `tests/programmordner_faelle.rs` holds it to the same list (`pruefer_verboten` of the cases). Logs every call."""
        pfad = os.path.join(wurzel, "pruefer.sh")
        faelle = "|".join(shlex.quote(v) for v in verboten)
        with open(pfad, "w", encoding="utf-8") as d:
            d.write("#!/bin/sh\n"
                    f"printf '%s|%s|%s\\n' \"$1\" \"$2\" \"$3\" >> {shlex.quote(os.path.join(wurzel, 'pruefer-aufrufe'))}\n"
                    + (f"exit {ausgang}\n" if ausgang is not None else "") +
                    ('[ "$1" = --programmordner ] || exit 64\n'
                     + (f"case \"$3\" in {faelle}) echo pruef.programmordner_verboten; exit 1 ;; esac\n" if verboten else "")
                     + "exit 0\n"))
        os.chmod(pfad, 0o755)
        return pfad

    def lauf(self, wurzel, lage, such=False, debconf=None, rc=0, verboten=None, ausgang=None, pruefer_da=True):
        """Runs the block with ETCW and DATEN below `wurzel` (root-only steps stubbed); returns the content of lauf.json or None.
        `debconf` (or `lage["antwort"]`) stubs the functions of the confmodule: {"seen": bool, "antwort": text}; the stubs write `journal` and `db` below `wurzel`."""
        wurzel = os.path.realpath(wurzel)
        if debconf is None and lage.get("antwort") is not None:
            debconf = {"seen": True, "antwort": lage["antwort"]}
        etc, daten = os.path.join(wurzel, "etc/werkbank"), os.path.join(wurzel, "var/lib/werkbank")
        block = self.block.replace("ETCW=/etc/werkbank", f"ETCW={etc}")
        stummel = self.pruefer(wurzel, lage.get("pruefer_verboten", []) if verboten is None else verboten, ausgang) if pruefer_da else os.path.join(wurzel, "fehlt-pruefen")
        block = block.replace("PRUEFEN=/usr/bin/werkbank-pruefen", f"PRUEFEN={stummel}")
        h = lage.get("harness", {})
        kopf = (
            'install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; mkdir -p "$l"; chmod "$m" "$l"; }\n'
            "chown() { :; }\n"
            f"DATEN={daten}\n"
        )
        q = shlex.quote
        self.journal, self.db = os.path.join(wurzel, "journal"), os.path.join(wurzel, "db")
        kopf += f"logger() {{ printf '%s\\n' \"$*\" >> {q(self.journal)}; }}\n" + self.log + self.fail_text
        if debconf is not None:
            kopf += (f"db_fget() {{ RET={'true' if debconf['seen'] else 'false'}; return 0; }}\n"
                     f"db_get() {{ RET={q(debconf['antwort'])}; return 0; }}\n"
                     f"db_set() {{ printf 'SET %s %s\\n' \"$1\" \"$2\" >> {q(self.db)}; return 0; }}\n"
                     f"db_fset() {{ printf 'FSET %s %s %s\\n' \"$1\" \"$2\" \"$3\" >> {q(self.db)}; return 0; }}\n")
        werte = (f'nutzer={q(lage["nutzer"])}\ngruppe=g\nuid={lage["uid"]}\nheim={q(lage["heim"])}\n'
                 f'LK_PI={q(h.get("pi", {}).get("programm", ""))}\nLK_PI_NODE={q(h.get("pi", {}).get("node") or "")}\n'
                 f'LK_COPILOT={q(h.get("copilot", {}).get("programm", ""))}\nLK_COPILOT_NODE={q(h.get("copilot", {}).get("node") or "")}\n')
        if lage.get("vorher"):
            os.makedirs(etc, exist_ok=True)
            with open(os.path.join(etc, "lauf.json"), "w", encoding="utf-8") as d:
                d.write(self.vorher_text(lage["vorher"], lage.get("vorher_form")))
        aufruf = "lauf_json" if such else "lauf_schreiben"
        os.makedirs(daten, exist_ok=True)  # step 1a of postinst creates it (root:root 0755) before this block
        r = subprocess.run(["sh", "-c", kopf + block + werte + aufruf + "; echo WEITER"], capture_output=True, text=True, check=False)
        self.assertEqual(r.returncode, rc, r.stderr)
        self.stderr = r.stderr
        self.stdout = r.stdout
        self.zustand = self.zustandsdatei(daten)
        pfad = os.path.join(etc, "lauf.json")
        if not os.path.exists(pfad):
            return None
        self.assertEqual(os.stat(pfad).st_mode & 0o7777, 0o644)
        self.assertEqual([n for n in os.listdir(etc) if n.startswith(".lauf")], [])
        with open(pfad, encoding="utf-8") as d:
            text = d.read()
        self.assertTrue(text.endswith("\n") and text.count("\n") == 1)
        return json.loads(text.replace(daten, "/var/lib/werkbank"))

    def zustandsdatei(self, daten):
        """The state file of the refused program folders (`programmordner-abgelehnt` in the state folder) or None; checks mode and that no temporary file stays."""
        self.assertEqual([n for n in os.listdir(daten) if n.startswith(".abgelehnt")], [])
        pfad = os.path.join(daten, "programmordner-abgelehnt")
        if not os.path.exists(pfad):
            return None
        self.assertEqual(os.stat(pfad).st_mode & 0o7777, 0o644)
        with open(pfad, encoding="utf-8") as d:
            return d.read()

    @staticmethod
    def vorher_text(wert, form=None):
        """The text of an earlier lauf.json: byte-exact (default) or rewritten by another tool (`form`)."""
        if form == "indent1":
            return json.dumps({"fassung": 1, "ca_zulassung": wert}, indent=1) + "\n"
        if form == "zeilen":
            return '{\r\n\t"fassung" : 1 ,\r\n  "ca_zulassung"\n :\n\t"%s"\n}\n' % wert
        if form == "doppelt":
            return '{"ca_zulassung":"%s","fassung":1,"ca_zulassung":"%s"}\n' % (wert, wert)
        if form == "als-wert":
            return '{"ca_zulassung":"%s","fassung":1,"x":"ca_zulassung"}\n' % wert
        if form == "leerzeichen-im-wert":
            return '{"fassung": 1, "ca_zulassung": "%s "}\n' % wert
        return '{"fassung":1,"ca_zulassung":"%s"}\n' % wert

    def test_every_case_gives_the_reference_content(self):
        faelle = os.path.join(RUST, "spec", "paket", "faelle")
        namen = sorted(n for n in os.listdir(faelle) if n.startswith("laufkonfig-"))
        self.assertGreaterEqual(len(namen), 10)
        for n in namen:
            with open(os.path.join(faelle, n), encoding="utf-8") as d:
                fall = json.load(d)
            with open(os.path.join(RUST, "spec", "paket", "erwartet", n), encoding="utf-8") as d:
                erw = json.load(d)
            if fall["nutzer"] is None:
                continue  # postinst never calls the block without a user (step 1)
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.lauf(t, fall), erw["datei"], n)
                # a refused program folder never stops postinst: the script goes on (WEITER), the state file and the journal lines are those of the case
                self.assertIn("WEITER", self.stdout, n)
                self.assertEqual(self.zustand, erw.get("abgelehnt_datei"), n)
                self.assertEqual([z.split(" ", 2)[2] for z in self.journal_zeilen() if "Programmordner abgelehnt" in z], erw.get("journal", []), n)
                self.assertEqual("(Code 19)" in self.stderr, "journal" in erw, n)
                self.assertNotIn("postinst_code", erw, n)

    def test_the_longest_socket_path_fits(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, {"nutzer": "a" * 32, "uid": 4294967294, "heim": "/srv/h/" + "a" * 32})
            self.assertLessEqual(len(o["laufzeit"] + "/" + "s" * 64 + "/modell.sock"), 107)

    def test_the_form_of_the_lauf_crate(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, {"nutzer": "n", "uid": 1, "heim": "/srv/h/n", "harness": {"pi": {"programm": "/opt/pi/pi", "node": None}}})
        self.assertEqual(sorted(o), sorted(["fassung", "einstellungen", "vault", "protokoll", "zustand", "laufzeit", "sitzungen", "heim", "path",
                                            "programme", "harness", "programmordner", "schluessel", "ca_zulassung"]))
        self.assertEqual(sorted(o["programme"]), ["kind", "modellweg", "rangkern", "such_mcp", "weiter"])
        self.assertEqual(o["ca_zulassung"], "offen")

    def test_the_session_folder_lies_outside_every_folder_the_enclosure_must_not_meet(self):
        # the enclosure refuses a state folder inside a protected folder (measured in a fresh VM: exit 5 of the starter)
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, {"nutzer": "n", "uid": 1000, "heim": "/srv/h/n"})
        geschuetzt = [o["vault"], o["protokoll"], o["zustand"], o["laufzeit"], os.path.dirname(o["einstellungen"])]
        geschuetzt += [os.path.dirname(p) for p in o["schluessel"].values()]
        for g in geschuetzt:
            self.assertFalse(o["sitzungen"] == g or o["sitzungen"].startswith(g + "/") or g.startswith(o["sitzungen"] + "/"), (o["sitzungen"], g))

    def test_postinst_asks_the_check_for_every_folder_with_the_home_and_writes_after_the_answers(self):
        lage = {"nutzer": "n", "uid": 1000, "heim": "/srv/beispiel/heim/n",
                "harness": {"pi": {"programm": "/opt/pi/lib/node_modules/pi/cli.js", "node": "/opt/pi/bin/node"}}}
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, lage)
            aufrufe = open(os.path.join(os.path.realpath(t), "pruefer-aufrufe"), encoding="utf-8").read().splitlines()
        self.assertEqual(aufrufe, ["--programmordner|/srv/beispiel/heim/n|/opt/pi/lib/node_modules/pi", "--programmordner|/srv/beispiel/heim/n|/opt/pi/bin"])
        self.assertEqual(o["programmordner"], ["/opt/pi/lib/node_modules/pi", "/opt/pi/bin"])

    REFUSED = {"nutzer": "n", "uid": 1000, "heim": "/srv/beispiel/heim/n",
               "harness": {"pi": {"programm": "/opt/pi/lib/node_modules/pi/cli.js", "node": "/opt/pi/bin/node"}}, "pruefer_verboten": ["/opt/pi/bin"]}

    def test_nothing_at_or_below_usr_is_asked_of_the_check(self):
        # the enclosure binds /usr whole: a node under /usr and a package under /usr give no folder, so the check is never asked about them
        for harness, soll in (({"programm": "/opt/pi/lib/node_modules/pi/cli.js", "node": "/usr/bin/node"}, ["/opt/pi/lib/node_modules/pi"]),
                              ({"programm": "/usr/lib/node_modules/pi/cli.js", "node": "/usr/bin/node"}, [])):
            with tempfile.TemporaryDirectory() as t:
                self.lauf(t, {"nutzer": "n", "uid": 1000, "heim": "/srv/beispiel/heim/n", "harness": {"pi": harness}, "pruefer_verboten": ["/usr/bin", "/usr/lib/node_modules/pi"]})
                pfad = os.path.join(os.path.realpath(t), "pruefer-aufrufe")
                aufrufe = open(pfad, encoding="utf-8").read().splitlines() if os.path.exists(pfad) else []
                self.assertEqual([a.split("|")[2] for a in aufrufe], soll)
                self.assertIsNone(self.zustand)

    def test_a_refused_folder_skips_the_harness_and_postinst_ends_with_0_on_a_fresh_install(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, self.REFUSED)
            # nothing of the harness is entered: no `harness.pi` (so no session of pi starts), no folder, PATH fixed
            self.assertEqual((o["harness"], o["programmordner"], o["path"]), ({}, [], "/usr/local/bin:/usr/bin:/bin"))
            self.assertIn("WEITER", self.stdout)
            self.assertEqual(self.zustand, "pi /opt/pi/bin\n")
            self.assertEqual([z.split(" ", 2)[2] for z in self.journal_zeilen() if "Programmordner abgelehnt" in z], ["werkbank: Programmordner abgelehnt, Code 19, Harness pi"])
            self.assertIn("(Code 19)", self.stderr)
            # the line holds no path and no content
            self.assertNotIn("/opt/pi", "".join(self.journal_zeilen()) + self.stderr)

    def test_a_refused_folder_leaves_the_other_harness_and_the_answer_of_the_firm_in_place(self):
        lage = dict(self.REFUSED, vorher="zugelassen")
        lage["harness"] = dict(lage["harness"], copilot={"programm": "/opt/c/bin/copilot", "node": None})
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, lage)
            self.assertEqual((sorted(o["harness"]), o["programmordner"], o["ca_zulassung"]), (["copilot"], ["/opt/c/bin"], "zugelassen"))
            self.assertEqual(self.zustand, "pi /opt/pi/bin\n")

    def test_the_refusal_goes_away_with_a_configuration_without_a_refused_folder(self):
        with tempfile.TemporaryDirectory() as t:
            self.lauf(t, self.REFUSED)
            self.assertEqual(self.zustand, "pi /opt/pi/bin\n")
            # the same machine, the next configuration: the folder passes now (installed under another prefix); the state file is removed
            gut = dict(self.REFUSED, pruefer_verboten=[])
            o = self.lauf(t, gut)  # same root: the state file of the first run is still there
            self.assertIsNone(self.zustand)
            self.assertEqual(sorted(o["harness"]), ["pi"])
            # a missing check command (skeleton build) keeps the file as it is
            self.lauf(t, self.REFUSED)
            self.lauf(t, self.REFUSED, pruefer_da=False)
            self.assertEqual(self.zustand, "pi /opt/pi/bin\n")

    def test_a_refused_folder_does_not_stop_the_steps_after_it_and_the_hard_codes_stay_hard(self):
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        region = text[text.index("lk_harness_pruefen() {"):text.index("lk_text() {")]
        # the program folder check has no `fail`, no `exit` and no `return 1` (the installation goes on: step 4b restarts the units after an upgrade)
        for verboten in ("fail ", "exit ", "return 1", "set -e"):
            self.assertNotIn(verboten, region)
        # the codes 11 to 18 stay hard: each still ends the script with `fail <code>` in its own step
        for code in list(range(11, 18)):
            self.assertRegex(text, rf"\bfail {code} ", code)
        self.assertRegex(text, r"so_sicher \"\$DATEN/l\" 0 0 755 18")  # code 18 goes through `fail "$sp_code"` of the session folders (tests of class Sitzungsordner)
        self.assertLess(text.index("lauf_json || log"), text.index("# Step 4b:"))

    def test_a_check_that_fails_or_answers_badly_counts_as_refused_and_a_missing_command_skips_the_check(self):
        lage = {"nutzer": "n", "uid": 1000, "heim": "/srv/beispiel/heim/n", "harness": {"pi": {"programm": "/opt/pi/pi", "node": None}}}
        for ausgang in (2, 64, 70, 127):
            with tempfile.TemporaryDirectory() as t:
                o = self.lauf(t, lage, ausgang=ausgang)  # fail closed: the harness is not entered, postinst ends with 0
                self.assertEqual(o["harness"], {}, ausgang)
                self.assertEqual(self.zustand, "pi /opt/pi\n", ausgang)
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, lage, pruefer_da=False)  # skeleton build: no command, one journal line, the file is written
            self.assertEqual(o["programmordner"], ["/opt/pi"])
            self.assertIn("Programmordner nicht geprueft", open(self.journal, encoding="utf-8").read())
            self.assertIsNone(self.zustand)

    def folder_of(self, t, harness_dateien, heim="/srv/beispiel"):
        """Builds a fake tree below `t` (files and symlinks of `harness_dateien`: {path: None (file) | target (symlink)}), runs the search for `pi` and prints the folders."""
        for pfad, ziel in harness_dateien.items():
            voll = os.path.join(t, pfad.lstrip("/"))
            os.makedirs(os.path.dirname(voll), exist_ok=True)
            if ziel is None:
                with open(voll, "w", encoding="utf-8") as d:
                    d.write("#!/bin/sh\n")
                os.chmod(voll, 0o755)
            else:
                os.symlink(ziel, voll)
        block = self.block.replace('LK_WURZEL=""', f"LK_WURZEL={t}")
        skript = (f'heim={heim}\nLK_PI="" LK_PI_NODE="" LK_COPILOT="" LK_COPILOT_NODE=""\nlk_suche copilot\nlk_suche pi\nlk_programmordner\n'
                  "printf '%s|%s|%s\\n' \"$LK_PI\" \"$LK_PI_NODE\" \"$LK_ORDNER\"\n")
        r = subprocess.run(["sh", "-c", block + skript], capture_output=True, text=True, check=False)
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.strip()

    def test_the_folder_of_a_user_install_is_the_package_root_never_the_prefix(self):
        cli = "dist/bundle/cli.js"
        for praefix in (".local", ".npm-global"):
            for paket, bin_ziel in (("pi-coding-agent", f"../lib/node_modules/pi-coding-agent/{cli}"),
                                    ("@earendil-works/pi-coding-agent", f"../lib/node_modules/@earendil-works/pi-coding-agent/{cli}")):
                with tempfile.TemporaryDirectory() as t0:
                    t = os.path.realpath(t0)
                    wurzel = f"/srv/beispiel/{praefix}/lib/node_modules/{paket}"
                    aus = self.folder_of(t, {f"{wurzel}/{cli}": None, f"/srv/beispiel/{praefix}/bin/pi": bin_ziel})
                    # the bin folder holds only the link; the real path inside the package folder is entered and the folder is its root
                    self.assertEqual(aus, f"{wurzel}/{cli}||" + f" {wurzel}", (praefix, paket, aus))

    def test_node_in_the_local_bin_folder_gives_that_folder_and_not_the_two_levels_above(self):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            aus = self.folder_of(t, {"/srv/beispiel/.local/bin/pi": None, "/srv/beispiel/.local/bin/node": None})
            self.assertEqual(aus, "/srv/beispiel/.local/bin/pi|/srv/beispiel/.local/bin/node| /srv/beispiel/.local/bin")
            self.assertNotIn(" /srv/beispiel/.local ", aus + " ")

    def test_the_search_in_a_fake_tree(self):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            heim = "/srv/beispiel"
            bin_ = os.path.join(t, "srv/beispiel/.nvm/versions/node/v24.1.0/bin")
            lib = os.path.join(t, "srv/beispiel/.nvm/versions/node/v24.1.0/lib/node_modules/pi-coding-agent/dist")
            os.makedirs(bin_)
            os.makedirs(lib)
            for pfad in (os.path.join(bin_, "node"), os.path.join(lib, "cli.js")):
                with open(pfad, "w", encoding="utf-8") as d:
                    d.write("#!/bin/sh\n")
                os.chmod(pfad, 0o755)
            os.symlink("../lib/node_modules/pi-coding-agent/dist/cli.js", os.path.join(bin_, "pi"))
            os.makedirs(os.path.join(t, "etc"), exist_ok=True)
            block = self.block.replace('LK_WURZEL=""', f'LK_WURZEL={t}')
            skript = ('uid=1\nnutzer=wb\nheim=/srv/beispiel\nLK_PI="" LK_PI_NODE="" LK_COPILOT="" LK_COPILOT_NODE=""\n'
                      "lk_suche copilot\nlk_suche pi\nprintf '%s|%s|%s|%s\\n' \"$LK_PI\" \"$LK_PI_NODE\" \"$LK_COPILOT\" \"$LK_COPILOT_NODE\"\n")
            r = subprocess.run(["sh", "-c", block + skript], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            v = "/srv/beispiel/.nvm/versions/node/v24.1.0"
            self.assertEqual(r.stdout.strip(), f"{v}/lib/node_modules/pi-coding-agent/dist/cli.js|{v}/bin/node||", r.stdout)
        self.assertEqual(heim, "/srv/beispiel")

    def test_the_highest_nvm_version_wins(self):
        # Three versions with `pi`: the order of the glob would take v22.1.0 before v9.0.0 only by chance of the text order; here the
        # numbers decide (v9.0.0 < v22.1.0 < v22.10.0). A folder name that is no version is not taken.
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            for v in ("v9.0.0", "v22.1.0", "v22.10.0", "v99.0.0-rc1", "vlatest"):
                ordner = os.path.join(t, "srv/beispiel/.nvm/versions/node", v, "bin")
                os.makedirs(ordner)
                for name in ("pi", "node"):
                    with open(os.path.join(ordner, name), "w", encoding="utf-8") as d:
                        d.write("#!/bin/sh\n")
                    os.chmod(os.path.join(ordner, name), 0o755)
            block = self.block.replace('LK_WURZEL=""', f'LK_WURZEL={t}')
            skript = 'heim=/srv/beispiel\nLK_PI=""\nlk_suche pi\nprintf "%s" "$LK_PI"\n'
            r = subprocess.run(["sh", "-c", block + skript], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(r.stdout, "/srv/beispiel/.nvm/versions/node/v22.10.0/bin/pi")

    def test_the_lower_nvm_version_is_taken_when_the_higher_has_no_program(self):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            for v, mit in (("v9.0.0", True), ("v22.1.0", False)):
                ordner = os.path.join(t, "srv/beispiel/.nvm/versions/node", v, "bin")
                os.makedirs(ordner)
                if mit:
                    with open(os.path.join(ordner, "pi"), "w", encoding="utf-8") as d:
                        d.write("#!/bin/sh\n")
                    os.chmod(os.path.join(ordner, "pi"), 0o755)
            block = self.block.replace('LK_WURZEL=""', f'LK_WURZEL={t}')
            skript = 'heim=/srv/beispiel\nLK_PI=""\nlk_suche pi\nprintf "%s" "$LK_PI"\n'
            r = subprocess.run(["sh", "-c", block + skript], capture_output=True, text=True, check=False)
            self.assertEqual(r.stdout, "/srv/beispiel/.nvm/versions/node/v9.0.0/bin/pi")

    def test_a_program_outside_the_allowed_roots_is_not_taken(self):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            os.makedirs(os.path.join(t, "srv/beispiel/.local/bin"))
            os.makedirs(os.path.join(t, "srv/beispiel/projekt"))
            ziel = os.path.join(t, "srv/beispiel/projekt/pi")
            with open(ziel, "w", encoding="utf-8") as d:
                d.write("#!/bin/sh\n")
            os.chmod(ziel, 0o755)
            os.symlink("../../projekt/pi", os.path.join(t, "srv/beispiel/.local/bin/pi"))
            block = self.block.replace('LK_WURZEL=""', f'LK_WURZEL={t}')
            skript = 'heim=/srv/beispiel\nLK_PI=""\nlk_suche pi\nprintf "[%s]" "$LK_PI"\n'
            r = subprocess.run(["sh", "-c", block + skript], capture_output=True, text=True, check=False)
            self.assertEqual(r.stdout, "[]", r.stdout + r.stderr)

    def test_a_dirty_value_writes_no_file(self):
        with tempfile.TemporaryDirectory() as t:
            self.assertIsNone(self.lauf(t, {"nutzer": "n", "uid": 1, "heim": "/srv/h/n", "harness": {"pi": {"programm": "/opt/p i/pi", "node": None}}}))

    def test_a_kept_answer_of_the_firm_and_no_other_value(self):
        for vorher, erwartet in (("zugelassen", "zugelassen"), ("abgelehnt", "abgelehnt"), ("ja", "offen")):
            with tempfile.TemporaryDirectory() as t:
                self.assertEqual(self.lauf(t, {"nutzer": "n", "uid": 1, "heim": "/srv/h/n", "vorher": vorher})["ca_zulassung"], erwartet)

    # ---- ca_zulassung: the answer of debconf, the journal line (contract "Zulassung der CA")
    ZEILE = re.compile(r"^-t werkbank-installation werkbank: ca_zulassung (zugelassen|offen|abgelehnt) \(vorher (zugelassen|offen|abgelehnt|keiner|ungueltig)\), "
                       r"Weg (debconf|paket|befehl), uid [0-9]+(, sudo_uid [0-9]+)?, Zeit [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")

    def journal_zeilen(self):
        if not os.path.exists(self.journal):
            return []
        with open(self.journal, encoding="utf-8") as d:
            return d.read().splitlines()

    def db_zeilen(self):
        if not os.path.exists(self.db):
            return []
        with open(self.db, encoding="utf-8") as d:
            return d.read().splitlines()

    LAGE = {"nutzer": "n", "uid": 1, "heim": "/srv/h/n"}

    def test_step_2_never_overwrites_the_settings_file_of_the_operator(self):
        # Admin fields (contract "Welche Felder der Admin setzt"): einstellungen.json (the gateway address) is copied once, never over an existing file.
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        block = text[text.index("nutzerordner() {"):text.index('if [ -n "$nutzer" ]; then\n    nutzerordner ||')]
        with tempfile.TemporaryDirectory() as t:
            t = os.path.realpath(t)
            daten, vorlage, bin_ = os.path.join(t, "daten"), os.path.join(t, "vorlage"), os.path.join(t, "bin")
            os.makedirs(vorlage)
            os.makedirs(bin_)
            with open(os.path.join(vorlage, "einstellungen.json"), "w", encoding="utf-8") as d:
                d.write('{"gateway":"vorlage"}\n')
            # stand-ins: `install` without -o/-g (a test is no root) and `chown`
            with open(os.path.join(bin_, "install"), "w", encoding="utf-8") as d:
                d.write('#!/bin/sh\nargs=""\nwhile [ $# -gt 0 ]; do case "$1" in -o|-g) shift 2;; *) args="$args $1"; shift;; esac; done\nexec /usr/bin/install $args\n')
            os.chmod(os.path.join(bin_, "install"), 0o755)
            with open(os.path.join(bin_, "chown"), "w", encoding="utf-8") as d:
                d.write("#!/bin/sh\nexit 0\n")
            os.chmod(os.path.join(bin_, "chown"), 0o755)
            env = {"PATH": bin_ + ":/usr/bin:/bin"}
            kopf = f"DATEN={shlex.quote(daten)}\nVORLAGE={shlex.quote(vorlage)}\nnutzer=n\ngruppe=g\n"
            ziel = os.path.join(daten, "n", "einstellungen.json")
            r = subprocess.run(["sh", "-c", kopf + block + "nutzerordner"], capture_output=True, text=True, check=False, env=env)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(lesen(ziel), '{"gateway":"vorlage"}\n')  # first install: the copy
            with open(ziel, "w", encoding="utf-8") as d:
                d.write('{"gateway":"https://gateway.beispiel"}\n')
            r = subprocess.run(["sh", "-c", kopf + block + "nutzerordner"], capture_output=True, text=True, check=False, env=env)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(lesen(ziel), '{"gateway":"https://gateway.beispiel"}\n')  # update: what the operator set stays

    def test_a_value_rewritten_by_another_tool_is_read_as_json(self):
        # an administrator rewrote lauf.json (json.dumps(indent=1), blanks around the colon, line breaks): the value stays
        for form in ("indent1", "zeilen"):
            for wert in ("zugelassen", "abgelehnt", "offen"):
                with tempfile.TemporaryDirectory() as t:
                    lage = dict(self.LAGE, vorher=wert, vorher_form=form)
                    self.assertEqual(self.lauf(t, lage)["ca_zulassung"], wert, (form, wert))
                    self.assertEqual(self.journal_zeilen(), [], (form, wert, "no change, no line"))

    def test_a_doubled_key_or_a_value_with_a_blank_gives_offen_and_one_line(self):
        for form in ("doppelt", "als-wert", "leerzeichen-im-wert"):
            with tempfile.TemporaryDirectory() as t:
                lage = dict(self.LAGE, vorher="zugelassen", vorher_form=form)
                self.assertEqual(self.lauf(t, lage)["ca_zulassung"], "offen", form)
                zeilen = self.journal_zeilen()
                self.assertEqual(len(zeilen), 1, form)
                self.assertIn("ca_zulassung offen (vorher ungueltig), Weg paket", zeilen[0])

    def test_a_fresh_install_without_an_answer_writes_offen_and_logs_the_first_value(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, dict(self.LAGE), debconf={"seen": False, "antwort": "offen"})
            self.assertEqual(o["ca_zulassung"], "offen")
            zeilen = self.journal_zeilen()
            self.assertEqual(len(zeilen), 1, zeilen)
            self.assertRegex(zeilen[0], self.ZEILE)
            self.assertIn("ca_zulassung offen (vorher keiner), Weg paket,", zeilen[0])
            # the answer shows the written value and stays consumed
            self.assertEqual(self.db_zeilen(), ["SET werkbank/ca_zulassung offen", "FSET werkbank/ca_zulassung seen false"])

    def test_a_waiting_answer_is_written_logged_and_consumed(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, dict(self.LAGE), debconf={"seen": True, "antwort": "zugelassen"})
            self.assertEqual(o["ca_zulassung"], "zugelassen")
            zeilen = self.journal_zeilen()
            self.assertEqual(len(zeilen), 1, zeilen)
            self.assertRegex(zeilen[0], self.ZEILE)
            self.assertIn("ca_zulassung zugelassen (vorher keiner), Weg debconf,", zeilen[0])
            self.assertEqual(self.db_zeilen(), ["SET werkbank/ca_zulassung zugelassen", "FSET werkbank/ca_zulassung seen false"])

    def test_the_line_names_the_old_value(self):
        for vorher, antwort, alt in (("zugelassen", "offen", "zugelassen"), ("offen", "abgelehnt", "offen"), ("abgelehnt", "zugelassen", "abgelehnt"), ("ja", "zugelassen", "ungueltig")):
            with tempfile.TemporaryDirectory() as t:
                o = self.lauf(t, dict(self.LAGE, vorher=vorher), debconf={"seen": True, "antwort": antwort})
                self.assertEqual(o["ca_zulassung"], antwort)
                zeilen = self.journal_zeilen()
                self.assertEqual(len(zeilen), 1, zeilen)
                self.assertRegex(zeilen[0], self.ZEILE)
                self.assertIn(f"ca_zulassung {antwort} (vorher {alt}), Weg debconf,", zeilen[0])

    def test_an_answer_that_is_not_waiting_never_changes_the_value(self):
        # `seen` false: the question was not answered (an upgrade, a `dpkg-reconfigure` without an answer, DEBIAN_FRONTEND=noninteractive)
        for vorher in ("zugelassen", "abgelehnt"):
            with tempfile.TemporaryDirectory() as t:
                o = self.lauf(t, dict(self.LAGE, vorher=vorher), debconf={"seen": False, "antwort": "offen"})
                self.assertEqual(o["ca_zulassung"], vorher)
                self.assertEqual(self.journal_zeilen(), [], "no change, no line")
                self.assertEqual(self.db_zeilen(), [f"SET werkbank/ca_zulassung {vorher}", "FSET werkbank/ca_zulassung seen false"],
                                 "debconf follows the file")

    def test_the_same_answer_as_the_file_changes_nothing_and_writes_no_line(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, dict(self.LAGE, vorher="zugelassen"), debconf={"seen": True, "antwort": "zugelassen"})
            self.assertEqual(o["ca_zulassung"], "zugelassen")
            self.assertEqual(self.journal_zeilen(), [])

    def test_an_answer_outside_the_three_values_is_ignored(self):
        for antwort in ("ja", "", "Zugelassen", "zugelassen offen", "zugelassen\nx"):
            with tempfile.TemporaryDirectory() as t:
                o = self.lauf(t, dict(self.LAGE, vorher="abgelehnt"), debconf={"seen": True, "antwort": antwort})
                self.assertEqual(o["ca_zulassung"], "abgelehnt", repr(antwort))
                self.assertEqual(self.journal_zeilen(), [], repr(antwort))

    def test_without_the_confmodule_the_file_value_stays(self):
        with tempfile.TemporaryDirectory() as t:
            o = self.lauf(t, dict(self.LAGE, vorher="zugelassen"))
            self.assertEqual(o["ca_zulassung"], "zugelassen")
            self.assertEqual(self.db_zeilen(), [])

    def test_when_no_file_is_written_the_answer_stays_and_nothing_is_logged(self):
        with tempfile.TemporaryDirectory() as t:
            lage = dict(self.LAGE, heim="/srv/h/n m")  # no folder form: no file
            self.assertIsNone(self.lauf(t, lage, debconf={"seen": True, "antwort": "zugelassen"}))
            self.assertEqual(self.journal_zeilen(), [])
            self.assertEqual(self.db_zeilen(), [], "the answer is not consumed")

    def test_the_line_holds_no_path_and_no_secret(self):
        with tempfile.TemporaryDirectory() as t:
            self.lauf(t, dict(self.LAGE), debconf={"seen": True, "antwort": "zugelassen"})
            zeile = self.journal_zeilen()[0]
            self.assertNotIn("/", zeile.replace("-t werkbank-installation ", ""))
            self.assertNotIn("schluessel", zeile)


class DienstJson(unittest.TestCase):
    """The block `dienst.json` of postinst (kern.json, bruecke.json) against the reference `referenz.dienstkonfig` (the cases `dienstkonfig-*`)."""

    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, os.path.join(RUST, "spec", "paket"))
        import referenz
        cls.ref = referenz
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.block = text[text.index("# --- dienst.json begin"):text.index("# --- dienst.json end")]
        LaufJson.setUpClass()

    def schreiben(self, nutzer, uid, vorher_kern=None, vorher_bruecke=None):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            etc = os.path.join(t, "etc/werkbank")
            os.makedirs(etc)
            for n, text in (("kern.json", vorher_kern), ("bruecke.json", vorher_bruecke)):
                if text is not None:
                    with open(os.path.join(etc, n), "w", encoding="utf-8", newline="") as d:
                        d.write(text)
            journal = os.path.join(t, "journal")
            kopf = (f"logger() {{ printf '%s\\n' \"$*\" >> {shlex.quote(journal)}; }}\n" + LaufJson.log + 'install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; mkdir -p "$l"; chmod "$m" "$l"; }\n'
                    "chown() { :; }\n" f"ETCW={shlex.quote(etc)}\nDATEN=/var/lib/werkbank\n")
            werte = f"nutzer={shlex.quote(nutzer)}\nuid={uid}\n"
            r = subprocess.run(["sh", "-c", kopf + self.block + werte + "dk_schreiben kern.json && dk_schreiben bruecke.json"], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            aus = {}
            for n in ("kern", "bruecke"):
                pfad = os.path.join(etc, n + ".json")
                self.assertEqual(os.stat(pfad).st_mode & 0o7777, 0o644)
                aus[n] = json.loads(lesen(pfad))
            self.assertEqual(sorted(os.listdir(etc)), ["bruecke.json", "kern.json"], "no temporary file stays behind")
            self.journal = []
            if os.path.exists(journal):
                with open(journal, encoding="utf-8") as d:
                    self.journal = [z.removeprefix("-t werkbank-installation ") for z in d.read().splitlines()]
            return aus

    def test_the_files_equal_the_reference(self):
        for nutzer, uid in (("nutzer1", 1000), ("anna-b", 1234), ("n", 4294967294), ("a" * 32, 1000), ("n", 0)):
            with self.subTest(nutzer=nutzer, uid=uid):
                self.assertEqual(self.schreiben(nutzer, uid), self.ref.dienstkonfig({"nutzer": nutzer, "uid": uid}))

    def test_every_case_with_an_old_file_gives_the_reference_and_the_journal_lines(self):
        faelle = os.path.join(RUST, "spec", "paket", "faelle")
        namen = sorted(n for n in os.listdir(faelle) if n.startswith("dienstkonfig-admin-"))
        self.assertGreaterEqual(len(namen), 10)
        for n in namen:
            with open(os.path.join(faelle, n), encoding="utf-8") as d:
                fall = json.load(d)
            with open(os.path.join(RUST, "spec", "paket", "erwartet", n), encoding="utf-8") as d:
                erw = json.load(d)
            o = self.schreiben(fall["nutzer"], fall["uid"], fall.get("vorher_kern"), fall.get("vorher_bruecke"))
            self.assertEqual(o, erw["dateien"], n)
            self.assertEqual(self.journal, erw.get("journal", []), n)

    def test_the_admin_fields_survive_an_update_and_a_new_file_has_the_defaults(self):
        o = self.schreiben("n", 1000, '{"lauf":{"groesse":[200,50]}}', '{"port":8765}')
        self.assertEqual((o["kern"]["lauf"]["groesse"], o["bruecke"]["port"]), ([200, 50], 8765))
        # the second run reads what the first wrote: the values stay through any number of updates
        o = self.schreiben("n", 1000)
        self.assertEqual((o["kern"]["lauf"]["groesse"], o["bruecke"]["port"]), ([120, 40], 0))
        self.assertEqual(self.journal, [])

    def test_the_two_files_agree_and_the_socket_path_is_short(self):
        o = self.schreiben("nutzer1", 4294967294)
        socket = o["kern"]["socket"]["ordner"] + "/" + o["kern"]["socket"]["name"]
        self.assertEqual(o["bruecke"]["dienst"]["socket"], socket)
        self.assertLessEqual(len(socket), 100)
        self.assertEqual(o["kern"]["deflate"], o["bruecke"]["deflate"])
        self.assertEqual(os.path.dirname(o["bruecke"]["token_datei"]), o["kern"]["socket"]["ordner"])
        self.assertEqual(os.path.dirname(o["bruecke"]["port_datei"]), o["kern"]["socket"]["ordner"])

    def test_the_environment_of_the_sessions_is_term_only_and_holds_no_tls_variable(self):
        env = self.schreiben("nutzer1", 1000)["kern"]["lauf"]["env"]
        self.assertEqual(env, {"TERM": "xterm-256color"})

    def test_the_core_service_hands_out_no_id_of_an_existing_session_folder(self):
        """kern.json `lauf.sitzungen` is exactly `sitzungen` of lauf.json (both written by the real blocks of postinst), for several users."""
        LaufJson.setUpClass()
        lauf = LaufJson("lauf")
        for nutzer, uid in (("nutzer1", 1000), ("anna-b", 1234), ("n", 4294967294)):
            with self.subTest(nutzer=nutzer, uid=uid), tempfile.TemporaryDirectory() as t:
                gelesen = lauf.lauf(t, {"nutzer": nutzer, "uid": uid, "heim": "/srv/beispiel/heim/" + nutzer})
                kern = self.schreiben(nutzer, uid)["kern"]["lauf"]
                self.assertIn("sitzungen", kern, "a missing field is an error of the package")
                self.assertEqual(kern["sitzungen"], gelesen["sitzungen"])
                self.assertEqual(kern["sitzungen"], f"/var/lib/werkbank/s/{uid}")

    def test_the_marker_fields_of_kern_json_follow_the_uid_of_the_user(self):
        """`lauf.marker_datei` lies under /run/user/<uid> of the same user, `lauf.marker_ordner` is /var/lib/werkbank/z/<uid> (a missing field is an error of the package)."""
        for nutzer, uid in (("nutzer1", 1000), ("anna-b", 1234), ("n", 4294967294)):
            with self.subTest(nutzer=nutzer, uid=uid):
                kern = self.schreiben(nutzer, uid)["kern"]["lauf"]
                self.assertIn("marker_datei", kern)
                self.assertIn("marker_ordner", kern)
                self.assertEqual(kern["marker_datei"], f"/run/user/{uid}/werkbank-probelauf/marker.json")
                self.assertEqual(kern["marker_ordner"], f"/var/lib/werkbank/z/{uid}")
                self.assertTrue(kern["marker_datei"].startswith(f"/run/user/{uid}/"))
                self.assertEqual(kern["marker_ordner"], kern["sitzungen"].replace("/s/", "/z/"))

    def test_the_core_service_gets_the_folder_of_the_target_lists(self):
        """kern.json `lauf.ziellisten` is `<zustand>/sitzungen` of lauf.json, the folder werkbank-lauf writes the target list to; not `lauf.sitzungen`."""
        LaufJson.setUpClass()
        lauf = LaufJson("lauf")
        for nutzer, uid in (("anna", 1000), ("b-c_d", 77), ("a" * 32, 4294967294)):
            with self.subTest(nutzer=nutzer, uid=uid), tempfile.TemporaryDirectory() as t:
                gelesen = lauf.lauf(t, {"nutzer": nutzer, "uid": uid, "heim": "/srv/beispiel/heim/" + nutzer})
                kern = self.schreiben(nutzer, uid)["kern"]["lauf"]
                self.assertEqual(kern["ziellisten"], gelesen["zustand"] + "/sitzungen")
                self.assertEqual(kern["ziellisten"], f"/var/lib/werkbank/{nutzer}/sitzungen")
                self.assertEqual(kern["laufzeit"], gelesen["laufzeit"])
                self.assertEqual(kern["laufzeit"], f"/var/lib/werkbank/l/{uid}")
                self.assertNotEqual(kern["ziellisten"], kern["sitzungen"])
                self.assertEqual(sorted(kern), ["einstellungen", "env", "groesse", "laufzeit", "marker_datei", "marker_ordner", "programm", "sitzungen", "ziellisten"])

    def test_the_units_and_the_list(self):
        kern = lesen(os.path.join(HIER, PLATTFORM, "werkbank-kern.service"))
        bruecke = lesen(os.path.join(HIER, PLATTFORM, "werkbank-bruecke.service"))
        for zeile in ("ExecStart=/usr/lib/werkbank/werkbank-kern /etc/werkbank/kern.json", "RuntimeDirectory=werkbank", "RuntimeDirectoryMode=0700"):
            self.assertIn(zeile + "\n", kern)
        # the core unit carries no NoNewPrivileges and no option that implies it (the starter's profile change to bwrap would be refused)
        self.assertEqual(self.ref.kern_einheit_code({"text": kern}), None)
        for schluessel in self.ref.KERN_EINHEIT_NNP_SCHLUESSEL:
            self.assertNotIn(schluessel, [z.split("=")[0] for z in kern.split("\n")])
        for verboten in ("PrivateTmp", "RestrictAddressFamilies"):
            self.assertNotIn(verboten, [z.split("=")[0] for z in kern.split("\n")])
        for zeile in ("ExecStart=/usr/lib/werkbank/werkbank-bruecke /etc/werkbank/bruecke.json", "BindsTo=werkbank-kern.service", "After=werkbank-kern.service",
                      "NoNewPrivileges=yes", "RestrictAddressFamilies=AF_INET AF_UNIX"):
            self.assertIn(zeile + "\n", bruecke)
        self.assertNotIn("RuntimeDirectory", [z.split("=")[0] for z in bruecke.split("\n")])
        liste = lesen(os.path.join(HIER, PLATTFORM, "lieferumfang.txt"))
        self.assertIn("dienst usr/lib/werkbank/werkbank-kern bin:dienst:wb-dienst\n", liste)
        self.assertIn("dienst usr/lib/werkbank/werkbank-bruecke bin:bruecke:wb-bruecke\n", liste)
        # the model path is no unit of the package (the runner starts it for each session)
        self.assertNotIn("werkbank-modellweg.service", liste)
        self.assertFalse(os.path.exists(os.path.join(HIER, PLATTFORM, "werkbank-modellweg.service")))
        for skript in ("postinst", "prerm"):
            text = lesen(os.path.join(HIER, PLATTFORM, skript))
            for zeile in text.split("\n"):
                if zeile.startswith("EINHEITEN="):
                    self.assertNotIn("modellweg", zeile)
        self.assertIn('EINHEITEN="werkbank-kern.service werkbank-bruecke.service"', lesen(os.path.join(HIER, PLATTFORM, "postinst")))
        self.assertIn("for e in werkbank-bruecke.service werkbank-kern.service; do", lesen(os.path.join(HIER, PLATTFORM, "prerm")))

    def test_reconfigure_leaves_the_units_running_and_upgrade_starts_them_again(self):
        prerm = lesen(os.path.join(HIER, PLATTFORM, "prerm"))
        postinst = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        # dpkg-reconfigure runs `prerm upgrade` with DEBCONF_RECONFIGURE=1 (measured): it must stop nothing
        weiche = 'if [ "$1" = upgrade ] && [ "${DEBCONF_RECONFIGURE:-}" = 1 ]; then\n    exit 0\nfi'
        self.assertIn(weiche, prerm)
        self.assertLess(prerm.index(weiche), prerm.index("stop \"$e\""))
        self.assertIn("/run/werkbank-paket-neustart", prerm)
        self.assertIn('[ "$1" = upgrade ] && systemctl --user --machine="$nutzer@" is-active --quiet "$e"', prerm)
        # the note is written before the unit is stopped, postinst starts the noted units after step 4 and before step 5
        self.assertIn("# Step 4b:", postinst)
        self.assertLess(postinst.index("# Step 4a:"), postinst.index("# Step 4b:"))
        self.assertLess(postinst.index("# Step 4b:"), postinst.index("# Step 5:"))
        self.assertIn('systemctl --user --machine="$nutzer@" start "$e"', postinst)
        self.assertIn('*" $nutzer "*" $e "* | *" $nutzer $e "*) ;;', postinst)  # only the user of step 1 and the fixed units count
        self.assertIn('rm -f "$NEUSTART"', postinst)
        # postinst itself never stops a unit of the core service or the bridge
        for zeile in postinst.split("\n"):
            if " stop " in zeile and not zeile.lstrip().startswith("#"):
                self.assertIn("modellweg", zeile)

    def test_postinst_removes_the_unit_of_the_earlier_package(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        for teil in ("systemctl --global disable werkbank-modellweg.service", "stop werkbank-modellweg.service",
                     "reset-failed werkbank-modellweg.service", "/etc/systemd/user/default.target.wants/werkbank-modellweg.service"):
            self.assertIn(teil, text)
        # postrm keeps the old name in its list, so that remove and purge leave no trace of an earlier unit
        self.assertIn("werkbank-modellweg.service", lesen(os.path.join(HIER, PLATTFORM, "postrm")))


FREMD_UID = 4242


class Sitzungsordner(unittest.TestCase):
    """The block `sitzungsordner` of postinst: the folders s/<uid>, l/<uid> and z/<uid>, fail closed with the fixed codes 17 and 18.
    The user is 4242 (`id` is stubbed), so a wrong variable (a root uid 0 leaking into the user folders) shows as a wrong path."""

    FREMD = 4242

    @classmethod
    def setUpClass(cls):
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.block = text[text.index("# --- sitzungsordner begin"):text.index("# --- sitzungsordner end")]

    def lauf(self, daten, stat=None):
        """Runs the block against `daten`. `install` ignores the owner; `so_stat` answers the owner the block expects (0:0 for s and l,
        4242:4242 below) and the real mode, or `stat` as a fixed text. Returns (rc, stderr)."""
        u = self.FREMD
        kopf = (
            'fail() { echo "F $1" >&2; exit "$1"; }\n'
            f"DATEN={shlex.quote(daten)}\nnutzer=n\n"
            f"id() {{ case \"$1\" in -u) echo {u} ;; -g) echo {u} ;; esac; }}\n"
        )
        if stat:
            so = f'so_stat() {{ echo "{stat}"; }}\n'
        else:
            code = ('import os,sys;p=sys.argv[1];m=os.stat(p).st_mode&0o7777;'
                    f'o="0:0" if os.path.basename(p) in ("s","l","z") else "{u}:{u}";print(o+":%o"%m)')
            so = f"so_stat() {{ python3 -I -c '{code}' \"$1\"; }}\n"
        stub = ('install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; '
                '[ -e "$l" ] || { mkdir "$l" && chmod "$m" "$l"; }; }\n')
        r = subprocess.run(["sh", "-c", kopf + self.block + stub + so + "sitzungsordner"], capture_output=True, text=True, check=False)
        return r.returncode, r.stderr

    def modi(self, daten):
        u = str(self.FREMD)
        return [oct(os.stat(os.path.join(daten, *p)).st_mode & 0o7777) for p in (("s",), ("s", u), ("l",), ("l", u), ("z",), ("z", u))]

    def test_fresh_install_creates_the_six_folders_for_the_user_not_for_root(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.mkdir(d)
            self.assertEqual(self.lauf(d), (0, ""))
            self.assertEqual(self.modi(d), ["0o755", "0o700", "0o755", "0o700", "0o755", "0o700"])
            self.assertEqual(sorted(os.listdir(os.path.join(d, "s"))), [str(self.FREMD)], "no folder for uid 0")
            self.assertEqual(sorted(os.listdir(os.path.join(d, "l"))), [str(self.FREMD)], "no folder for uid 0")
            self.assertEqual(sorted(os.listdir(os.path.join(d, "z"))), [str(self.FREMD)], "no folder for uid 0")
            self.assertEqual(self.lauf(d)[0], 0, "a second run changes nothing and passes")

    def test_a_symlink_for_the_session_folder_stops_with_17_and_is_not_followed(self):
        with tempfile.TemporaryDirectory() as t:
            t = os.path.realpath(t)
            d, ziel = os.path.join(t, "werkbank"), os.path.join(t, "ziel")
            os.makedirs(os.path.join(d, "s"))
            os.mkdir(ziel)
            os.symlink(ziel, os.path.join(d, "s", str(self.FREMD)))
            self.assertEqual(self.lauf(d)[0], 17)
            self.assertEqual(os.listdir(ziel), [])

    def test_a_symlink_for_s_itself_stops_with_17(self):
        with tempfile.TemporaryDirectory() as t:
            t = os.path.realpath(t)
            d, ziel = os.path.join(t, "werkbank"), os.path.join(t, "ziel")
            os.makedirs(d)
            os.mkdir(ziel)
            os.symlink(ziel, os.path.join(d, "s"))
            self.assertEqual(self.lauf(d)[0], 17)
            self.assertEqual(os.listdir(ziel), [])

    def test_a_symlink_for_the_socket_folder_stops_with_18(self):
        with tempfile.TemporaryDirectory() as t:
            t = os.path.realpath(t)
            d, ziel = os.path.join(t, "werkbank"), os.path.join(t, "ziel")
            os.makedirs(os.path.join(d, "l"))
            os.mkdir(ziel)
            os.symlink(ziel, os.path.join(d, "l", str(self.FREMD)))
            self.assertEqual(self.lauf(d)[0], 18)
            self.assertEqual(os.listdir(ziel), [])

    def test_a_symlink_for_the_count_folder_stops_with_17_and_is_not_followed(self):
        for unter in (None, str(FREMD_UID)):
            with tempfile.TemporaryDirectory() as t:
                t = os.path.realpath(t)
                d, ziel = os.path.join(t, "werkbank"), os.path.join(t, "ziel")
                os.makedirs(os.path.join(d, "z") if unter else d)
                os.mkdir(ziel)
                os.symlink(ziel, os.path.join(d, "z", unter) if unter else os.path.join(d, "z"))
                self.assertEqual(self.lauf(d)[0], 17, unter)
                self.assertEqual(os.listdir(ziel), [])

    def test_a_wrong_mode_or_kind_of_the_count_folder_stops_with_17(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.makedirs(os.path.join(d, "z", str(FREMD_UID)))
            os.chmod(os.path.join(d, "z"), 0o755)
            os.chmod(os.path.join(d, "z", str(FREMD_UID)), 0o755)
            self.assertEqual(self.lauf(d)[0], 17)
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.makedirs(os.path.join(d, "z"))
            os.chmod(os.path.join(d, "z"), 0o755)
            open(os.path.join(d, "z", str(FREMD_UID)), "w").close()
            self.assertEqual(self.lauf(d)[0], 17)

    def test_a_symlink_for_the_data_folder_stops_with_17(self):
        with tempfile.TemporaryDirectory() as t:
            t = os.path.realpath(t)
            ziel, d = os.path.join(t, "ziel"), os.path.join(t, "werkbank")
            os.mkdir(ziel)
            os.symlink(ziel, d)
            self.assertEqual(self.lauf(d)[0], 17)
            self.assertEqual(os.listdir(ziel), [])

    def test_a_wrong_mode_stops_with_17_for_s_and_18_for_l(self):
        for ordner, code in (("s", 17), ("l", 18)):
            with tempfile.TemporaryDirectory() as t:
                d = os.path.join(os.path.realpath(t), "werkbank")
                os.makedirs(os.path.join(d, ordner, str(self.FREMD)))
                os.chmod(os.path.join(d, ordner, str(self.FREMD)), 0o755)
                os.chmod(os.path.join(d, ordner), 0o755)
                self.assertEqual(self.lauf(d)[0], code, ordner)

    def test_a_folder_of_the_wrong_kind_stops_with_the_code(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.makedirs(os.path.join(d, "l"))
            os.chmod(os.path.join(d, "l"), 0o755)
            open(os.path.join(d, "l", str(self.FREMD)), "w").close()
            self.assertEqual(self.lauf(d)[0], 18)

    def test_a_foreign_owner_stops(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.mkdir(d)
            self.assertEqual(self.lauf(d)[0], 0)
            self.assertEqual(self.lauf(d, stat="1:1:755")[0], 17)
            self.assertEqual(self.lauf(d, stat="0:0:755")[0], 17, "root owns s but not s/<uid>: the user folder must belong to the user")

class CaBefehl(unittest.TestCase):
    """The command `werkbank-ca-zulassung` (contract "Zulassung der CA"): run as a copy of the script in a fake tree. Two lines of the copy differ from
    the file of the package: `ETCW` and the body of `ca_uid` (so a test needs no root); the programs `logger`, `chown` and `debconf-communicate` are
    stand-ins on the PATH. Everything else is the real text."""

    SKRIPT = os.path.join(HIER, PLATTFORM, "werkbank-ca-zulassung")
    LAUF = ('{"fassung":1,"einstellungen":"/srv/beispiel/e.json","vault":"/srv/beispiel/v","protokoll":"/srv/beispiel/p","zustand":"/srv/beispiel/z",'
            '"laufzeit":"/srv/beispiel/l","sitzungen":"/srv/beispiel/s","heim":"/srv/beispiel/h","path":"/usr/bin","programme":{"rangkern":"/srv/beispiel/r"},'
            '"harness":{},"programmordner":[],"schluessel":{"copilot":"/srv/beispiel/c","lokal":"/srv/beispiel/m"},"ca_zulassung":"%s"}\n')

    def setUp(self):
        self.w = os.path.realpath(tempfile.mkdtemp(prefix="wb-ca-befehl-"))
        self.etc = os.path.join(self.w, "etc")
        os.makedirs(self.etc)
        self.bin = os.path.join(self.w, "bin")
        os.makedirs(self.bin)
        self.journal = os.path.join(self.w, "journal")
        self.db = os.path.join(self.w, "db")
        self.stub("logger", "#!/bin/sh\nprintf '%s\\n' \"$*\" >> " + shlex.quote(self.journal) + "\n")
        self.stub("chown", "#!/bin/sh\nexit 0\n")
        self.stub_db("0 ok")
        text = lesen(self.SKRIPT)
        self.assertIn("ETCW=/etc/werkbank\n", text)
        self.assertIn("ca_uid() {\n    id -u 2>/dev/null\n}\n", text)
        text = text.replace("ETCW=/etc/werkbank\n", "ETCW=" + shlex.quote(self.etc) + "\n", 1)
        text = text.replace("ca_uid() {\n    id -u 2>/dev/null\n}\n", 'ca_uid() {\n    echo "${WB_TEST_UID-0}"\n}\n', 1)
        self.skript = os.path.join(self.w, "werkbank-ca-zulassung")
        with open(self.skript, "w", encoding="utf-8") as d:
            d.write(text)

    def tearDown(self):
        os.chmod(self.etc, 0o755)
        shutil.rmtree(self.w, ignore_errors=True)

    def stub(self, name, text):
        pfad = os.path.join(self.bin, name)
        with open(pfad, "w", encoding="utf-8") as d:
            d.write(text)
        os.chmod(pfad, 0o755)

    def stub_db(self, antwort):
        self.stub("debconf-communicate", "#!/bin/sh\ncat >> " + shlex.quote(self.db) + "\necho " + shlex.quote(antwort) + "\n")

    def lauf_json(self, ca="offen"):
        pfad = os.path.join(self.etc, "lauf.json")
        with open(pfad, "w", encoding="utf-8") as d:
            d.write(self.LAUF % ca)
        os.chmod(pfad, 0o644)
        return pfad

    def aufruf(self, *args, uid="0", env=None):
        umgebung = {"PATH": self.bin + ":" + os.environ.get("PATH", "/usr/bin:/bin"), "WB_TEST_UID": uid, "LC_ALL": "C"}
        umgebung.update(env or {})
        return subprocess.run(["sh", self.skript, *args], capture_output=True, text=True, check=False, env=umgebung)

    def zeilen(self, pfad):
        return lesen(pfad).splitlines() if os.path.exists(pfad) else []

    def schreibe(self, text):
        pfad = os.path.join(self.etc, "lauf.json")
        with open(pfad, "w", encoding="utf-8", newline="") as d:
            d.write(text)
        os.chmod(pfad, 0o644)
        return pfad

    def test_a_file_rewritten_by_another_tool_is_read_and_only_the_value_changes(self):
        formen = {
            "indent1": json.dumps(json.loads(self.LAUF % "zugelassen"), indent=1) + "\n",
            "zeilen": '{\r\n\t"fassung" : 1 ,\r\n  "ca_zulassung"\n :\n\t"zugelassen"\n}\n',
        }
        for name, text in formen.items():
            pfad = self.schreibe(text)
            r = self.aufruf("zugelassen")
            self.assertEqual((r.returncode, "schon zugelassen" in r.stdout), (0, True), (name, r.stdout, r.stderr))
            with open(pfad, encoding="utf-8", newline="") as d:
                self.assertEqual(d.read(), text, name)
            self.assertEqual(self.zeilen(self.journal), [], name)
            r = self.aufruf("abgelehnt")
            self.assertEqual(r.returncode, 0, (name, r.stderr))
            with open(pfad, encoding="utf-8", newline="") as d:
                neu = d.read()
            self.assertEqual(neu, text.replace('zugelassen"', 'abgelehnt"'), name)  # every other byte stays
            self.assertIn("(vorher zugelassen)", self.zeilen(self.journal)[-1], name)
            os.remove(self.journal)

    def test_a_doubled_key_or_a_second_form_stops_the_command(self):
        for text in ('{"ca_zulassung":"offen","fassung":1,"ca_zulassung":"offen"}\n',
                     '{"ca_zulassung": "offen", "x": "ca_zulassung"}\n',
                     '{"ca_zulassung": 3}\n'):
            pfad = self.schreibe(text)
            r = self.aufruf("zugelassen")
            self.assertEqual(r.returncode, 66, (text, r.stderr))
            self.assertEqual(lesen(pfad), text)

    def test_it_sets_each_value_and_changes_nothing_else(self):
        pfad = self.lauf_json("offen")
        alt = "offen"
        for neu in ("zugelassen", "abgelehnt", "offen"):
            vorher = lesen(pfad)
            r = self.aufruf(neu)
            self.assertEqual((r.returncode, r.stderr), (0, ""), r.stdout)
            self.assertIn("ca_zulassung ist jetzt %s (vorher %s)" % (neu, alt), r.stdout)
            self.assertEqual(lesen(pfad), vorher.replace('"ca_zulassung":"%s"' % alt, '"ca_zulassung":"%s"' % neu), neu)
            alt = neu
            self.assertEqual(os.stat(pfad).st_mode & 0o7777, 0o644)
            self.assertEqual([n for n in os.listdir(self.etc) if n.startswith(".lauf")], [], "no temporary file stays")
        self.assertEqual(lesen(pfad), self.LAUF % "offen", "back to the first text, byte for byte")

    def test_it_writes_one_line_per_change_in_the_form_of_postinst(self):
        self.lauf_json("offen")
        self.assertEqual(self.aufruf("zugelassen").returncode, 0)
        self.assertEqual(self.aufruf("zugelassen").returncode, 0)  # unchanged: no line
        self.assertEqual(self.aufruf("abgelehnt", env={"SUDO_UID": "1000"}).returncode, 0)
        zeilen = self.zeilen(self.journal)
        self.assertEqual(len(zeilen), 2, zeilen)
        for z in zeilen:
            self.assertRegex(z, LaufJson.ZEILE)
        self.assertIn("ca_zulassung zugelassen (vorher offen), Weg befehl, uid 0, Zeit ", zeilen[0])
        self.assertNotIn("sudo_uid", zeilen[0])
        self.assertIn("ca_zulassung abgelehnt (vorher zugelassen), Weg befehl, uid 0, sudo_uid 1000, Zeit ", zeilen[1])

    def test_a_sudo_uid_that_is_no_number_is_not_written(self):
        self.lauf_json("offen")
        self.assertEqual(self.aufruf("zugelassen", env={"SUDO_UID": "1000; echo x"}).returncode, 0)
        self.assertNotIn("sudo_uid", self.zeilen(self.journal)[0])

    def test_the_same_value_changes_nothing_and_says_so(self):
        pfad = self.lauf_json("zugelassen")
        os.utime(pfad, (1, 1))
        r = self.aufruf("zugelassen")
        self.assertEqual(r.returncode, 0)
        self.assertIn("schon zugelassen", r.stdout)
        self.assertEqual(os.stat(pfad).st_mtime, 1, "the file was not written")
        self.assertEqual(self.zeilen(self.journal), [])

    def test_debconf_gets_the_same_value_and_the_answer_counts_as_consumed(self):
        self.lauf_json("offen")
        self.assertEqual(self.aufruf("zugelassen").returncode, 0)
        self.assertEqual(self.zeilen(self.db), ["SET werkbank/ca_zulassung zugelassen", "FSET werkbank/ca_zulassung seen false"])

    def test_a_failed_debconf_step_warns_but_the_file_counts(self):
        pfad = self.lauf_json("offen")
        self.stub_db("10 werkbank/ca_zulassung doesn't exist")
        r = self.aufruf("zugelassen")
        self.assertEqual(r.returncode, 0)
        self.assertIn("debconf wurde nicht abgeglichen", r.stderr)
        self.assertIn('"ca_zulassung":"zugelassen"', lesen(pfad))

    def test_a_wrong_call_changes_nothing_and_gives_64(self):
        pfad = self.lauf_json("offen")
        vorher = lesen(pfad)
        for args in ((), ("zugelassen", "offen"), ("ja",), ("",), ("Zugelassen",), ("zugelassen ",), ("zugelassen\n",), ("--hilfe",), ("-",), ("zugelassen;x",)):
            r = self.aufruf(*args)
            self.assertEqual(r.returncode, 64, args)
            self.assertEqual(r.stdout, "", args)
            self.assertEqual(lesen(pfad), vorher, args)
        self.assertEqual(self.zeilen(self.journal), [])
        self.assertEqual(self.zeilen(self.db), [])

    def test_only_root_may_use_it(self):
        pfad = self.lauf_json("offen")
        vorher = lesen(pfad)
        for uid in ("1000", "1", "65534", ""):
            r = self.aufruf("zugelassen", uid=uid)
            self.assertEqual(r.returncode, 77, uid)
            self.assertEqual(lesen(pfad), vorher)
        self.assertEqual(self.zeilen(self.journal), [])

    def test_a_missing_or_foreign_form_of_the_file_gives_66_and_changes_nothing(self):
        self.assertEqual(self.aufruf("zugelassen").returncode, 66, "no file")
        pfad = os.path.join(self.etc, "lauf.json")
        ziel = os.path.join(self.w, "ziel.json")
        with open(ziel, "w", encoding="utf-8") as d:
            d.write(self.LAUF % "offen")
        os.symlink(ziel, pfad)
        self.assertEqual(self.aufruf("zugelassen").returncode, 66, "link")
        self.assertEqual(lesen(ziel), self.LAUF % "offen")
        os.remove(pfad)
        zwei = self.LAUF.replace('"fassung":1', '"fassung":1,"ca_zulassung":"offen"') % "offen"
        for text in ('{"fassung":1}\n', zwei, self.LAUF % "Offen", '{"ca_zulassung":1}\n'):
            with open(pfad, "w", encoding="utf-8") as d:
                d.write(text)
            self.assertEqual(self.aufruf("zugelassen").returncode, 66, text)
            self.assertEqual(lesen(pfad), text)
        self.assertEqual(self.zeilen(self.journal), [])

    def test_a_broken_value_in_the_known_form_is_repaired_and_logged_as_invalid(self):
        pfad = self.lauf_json("ja")
        self.assertEqual(self.aufruf("offen").returncode, 0)
        self.assertEqual(lesen(pfad), self.LAUF % "offen")
        z = self.zeilen(self.journal)
        self.assertEqual(len(z), 1)
        self.assertRegex(z[0], LaufJson.ZEILE)
        self.assertIn("(vorher ungueltig)", z[0])

    @unittest.skipIf(os.geteuid() == 0, "root can write into a folder that has mode 0555")
    def test_a_write_error_gives_74_and_the_file_stays(self):
        pfad = self.lauf_json("offen")
        os.chmod(self.etc, 0o555)
        r = self.aufruf("zugelassen")
        os.chmod(self.etc, 0o755)
        self.assertEqual(r.returncode, 74, r.stderr)
        self.assertEqual(lesen(pfad), self.LAUF % "offen")
        self.assertEqual(self.zeilen(self.journal), [], "no change, no line")

    def test_a_failing_step_of_the_write_leaves_the_file_and_no_temporary_file(self):
        pfad = self.lauf_json("offen")
        self.stub("chown", "#!/bin/sh\nexit 1\n")
        r = self.aufruf("zugelassen")
        self.assertEqual(r.returncode, 74, r.stderr)
        self.assertEqual(lesen(pfad), self.LAUF % "offen")
        self.assertEqual([n for n in os.listdir(self.etc) if n != "lauf.json"], [])
        self.assertEqual(self.zeilen(self.journal), [])

    def test_the_script_reads_no_variable_of_the_environment_but_sudo_uid(self):
        text = lesen(self.SKRIPT)
        for name in re.findall(r"\$\{?([A-Z_][A-Z0-9_]*)", text):
            self.assertIn(name, ("ETCW", "SUDO_UID"), name)


class CaFrage(unittest.TestCase):
    """The files `config` and `templates` of the debconf question (contract "Zulassung der CA")."""

    @classmethod
    def setUpClass(cls):
        cls.config = lesen(os.path.join(HIER, PLATTFORM, "config"))
        cls.templates = lesen(os.path.join(HIER, PLATTFORM, "templates"))
        cls.block = cls.config[cls.config.index("# --- ca-frage begin"):cls.config.index("# --- ca-frage end")]

    def test_the_template_has_the_three_values_and_the_default_offen(self):
        felder = {}
        for z in self.templates.split("\n"):
            if z and not z.startswith(" ") and ":" in z:
                k, _, v = z.partition(":")
                felder[k] = v.strip()
        self.assertEqual(felder["Template"], "werkbank/ca_zulassung")
        self.assertEqual(felder["Type"], "select")
        self.assertEqual(felder["Choices"], "zugelassen, offen, abgelehnt")
        self.assertEqual(felder["Default"], "offen")
        self.assertEqual(self.templates.count("Template:"), 1)

    def test_the_text_says_what_is_approved(self):
        self.assertTrue(self.templates.isascii())
        glatt = " ".join(self.templates.split())
        for teil in ("Zertifikat", "hoechstens 23 Stunden", "Ziele der Zielliste", "Schluessel liegt ausserhalb des Einschlusses", "werkbank-ca-zulassung",
                     '"offen" (Vorgabe)'):
            self.assertIn(teil, glatt, teil)

    def test_the_question_is_asked_at_low_priority_so_an_installation_asks_nothing(self):
        self.assertIn("db_input low werkbank/ca_zulassung", self.config)
        for hoch in ("db_input high", "db_input critical", "db_input medium"):
            self.assertNotIn(hoch, self.config)

    def test_config_never_stops_the_installation(self):
        self.assertNotRegex(self.config, r"(?m)^\s*set -e")
        self.assertTrue(self.config.rstrip().endswith("exit 0"))

    def lauf(self, text, seen):
        """Runs the block against a file with `text` (or none) and stubs of the confmodule; returns the lines of the `db_set` calls."""
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            if text is not None:
                with open(os.path.join(t, "lauf.json"), "w", encoding="utf-8") as d:
                    d.write(text)
            db = os.path.join(t, "db")
            kopf = ("ETCW=" + shlex.quote(t) + "\ndb_fget() { RET=" + ("true" if seen else "false") + "; return 0; }\n"
                    "db_set() { printf 'SET %s %s\\n' \"$1\" \"$2\" >> " + shlex.quote(db) + "; return 0; }\n")
            r = subprocess.run(["sh", "-c", kopf + self.block + "ca_abgleich"], capture_output=True, text=True, check=False)
            self.assertEqual((r.returncode, r.stderr), (0, ""))
            return lesen(db).splitlines() if os.path.exists(db) else []

    def test_the_file_value_becomes_the_default_unless_an_answer_waits(self):
        for wert in ("zugelassen", "offen", "abgelehnt"):
            text = CaBefehl.LAUF % wert
            self.assertEqual(self.lauf(text, seen=False), ["SET werkbank/ca_zulassung " + wert])
            self.assertEqual(self.lauf(text, seen=True), [], "a waiting answer (flag seen) is not overwritten")

    def test_no_file_or_an_unclear_value_sets_nothing(self):
        self.assertEqual(self.lauf(None, seen=False), [])
        for text in ('{"fassung":1}\n', CaBefehl.LAUF % "ja", CaBefehl.LAUF % "Offen", "kein json\n"):
            self.assertEqual(self.lauf(text, seen=False), [], text)

    def test_a_file_rewritten_by_another_tool_gives_its_value_and_a_doubled_key_none(self):
        self.assertEqual(self.lauf(json.dumps(json.loads(CaBefehl.LAUF % "zugelassen"), indent=1) + "\n", seen=False), ["SET werkbank/ca_zulassung zugelassen"])
        self.assertEqual(self.lauf('{\n\t"ca_zulassung" :\r\n "abgelehnt"\n}\n', seen=False), ["SET werkbank/ca_zulassung abgelehnt"])
        for text in ('{"ca_zulassung":"zugelassen","fassung":1,"ca_zulassung":"zugelassen"}\n', '{"ca_zulassung":"zugelassen","x":"ca_zulassung"}\n'):
            self.assertEqual(self.lauf(text, seen=False), [], text)


if __name__ == "__main__":
    unittest.main()

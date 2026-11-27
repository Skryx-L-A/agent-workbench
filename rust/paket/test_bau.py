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
import stat
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
        for datei in ("ausschluss.txt", "preinst", "postinst", "prerm", "postrm", "config", "templates"):
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
        for name in ("config", "templates", "preinst", "postinst", "prerm", "postrm", "control"):
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
                             "DEBIAN/preinst", "DEBIAN/postinst", "DEBIAN/prerm", "DEBIAN/postrm", "DEBIAN/config"):
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
        for skript in ("preinst", "postinst", "prerm", "postrm", "config", "werkbank-ca-zulassung"):
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
        self.assertEqual(sorted(o), sorted(["fassung", "einstellungen", "vault", "protokoll", "zustand", "laufzeit", "sitzungen", "gespraeche", "heim", "path",
                                            "programme", "harness", "programmordner", "schluessel", "ca_zulassung"]))
        self.assertEqual(sorted(o["programme"]), ["kind", "modellweg", "rangkern", "such_mcp", "weiter"])
        self.assertEqual(o["ca_zulassung"], "offen")

    def test_the_folder_of_the_conversations_lies_in_the_state_folder_by_the_name_of_the_user(self):
        """`gespraeche` of lauf.json is `<zustand>/gespraeche` (by the name like `einstellungen`, not by the uid; contract start, "Ablage des Gesprächs"), for every user."""
        for nutzer, uid in (("n", 1000), ("anna-b", 1234), ("a" * 32, 4294967294), ("n", 0)):
            with self.subTest(nutzer=nutzer, uid=uid), tempfile.TemporaryDirectory() as t:
                o = self.lauf(t, {"nutzer": nutzer, "uid": uid, "heim": "/srv/h/" + nutzer})
                self.assertEqual(o["gespraeche"], f"/var/lib/werkbank/{nutzer}/gespraeche")
                self.assertEqual(o["gespraeche"], o["zustand"] + "/gespraeche")
                self.assertEqual(os.path.dirname(o["gespraeche"]), os.path.dirname(o["einstellungen"]))
                self.assertNotEqual(o["gespraeche"], o["sitzungen"])
                self.assertNotIn(str(uid), o["gespraeche"].split("/")[3:], "not by the uid")
                self.assertEqual(self.ref.laufkonfig({"nutzer": nutzer, "uid": uid, "heim": "/srv/h/" + nutzer})["gespraeche"], o["gespraeche"])

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
            with open(os.path.join(vorlage, "erststart.json"), "w", encoding="utf-8") as d:
                d.write('{"gateway":"vorlage"}\n')
            with open(os.path.join(vorlage, "einstellungen.json"), "w", encoding="utf-8") as d:
                d.write('{"gateway":"vorfuehrung"}\n')  # step I copies the first-start profile, never the demo settings
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
                self.assertEqual(sorted(kern), ["bestand", "einstellungen", "env", "gespraeche", "groesse", "laufzeit", "marker_datei", "marker_ordner", "programm", "sitzungen", "ziellisten"])

    def test_kern_json_carries_the_file_of_the_session_list_by_the_name_of_the_user(self):
        """`lauf.bestand` of kern.json is $DATEN/<nutzer>/dienst/bestand.json (by the name like `lauf.einstellungen`, not by the uid), for every user, also after an update of an old file."""
        for nutzer, uid in (("nutzer1", 1000), ("anna-b", 1234), ("n", 4294967294), ("a" * 32, 1000), ("n", 0)):
            with self.subTest(nutzer=nutzer, uid=uid):
                kern = self.schreiben(nutzer, uid)["kern"]["lauf"]
                self.assertEqual(kern["bestand"], f"/var/lib/werkbank/{nutzer}/dienst/bestand.json")
                self.assertEqual(os.path.dirname(kern["bestand"]), kern["einstellungen"].rsplit("/", 1)[0] + "/dienst")
                self.assertEqual(self.ref.dienstkonfig({"nutzer": nutzer, "uid": uid})["kern"]["lauf"]["bestand"], kern["bestand"])
        kern = self.schreiben("n", 1000, '{"lauf":{"groesse":[200,50]}}')["kern"]["lauf"]
        self.assertEqual(kern["bestand"], "/var/lib/werkbank/n/dienst/bestand.json", "an update replaces an old file that lacks the key")

    def test_kern_json_and_lauf_json_carry_the_same_place_for_the_conversations(self):
        """`lauf.gespraeche` of kern.json is exactly `gespraeche` of lauf.json (both written by the real blocks of postinst), by the name of the user, also after an update of an old file."""
        LaufJson.setUpClass()
        lauf = LaufJson("lauf")
        for nutzer, uid in (("nutzer1", 1000), ("anna-b", 1234), ("n", 4294967294), ("a" * 32, 1000), ("n", 0)):
            with self.subTest(nutzer=nutzer, uid=uid), tempfile.TemporaryDirectory() as t:
                gelesen = lauf.lauf(t, {"nutzer": nutzer, "uid": uid, "heim": "/srv/beispiel/heim/" + nutzer})
                kern = self.schreiben(nutzer, uid)["kern"]["lauf"]
                self.assertIn("gespraeche", kern, "a missing field is an error of the package")
                self.assertEqual(kern["gespraeche"], gelesen["gespraeche"])
                self.assertEqual(kern["gespraeche"], f"/var/lib/werkbank/{nutzer}/gespraeche")
                self.assertEqual(kern["gespraeche"], gelesen["zustand"] + "/gespraeche")
                self.assertNotIn(kern["gespraeche"], (kern["sitzungen"], kern["ziellisten"], kern["laufzeit"], kern["bestand"]))
                self.assertEqual(self.ref.dienstkonfig({"nutzer": nutzer, "uid": uid})["kern"]["lauf"]["gespraeche"], kern["gespraeche"])
        kern = self.schreiben("n", 1000, '{"lauf":{"groesse":[200,50]}}')["kern"]["lauf"]
        self.assertEqual(kern["gespraeche"], "/var/lib/werkbank/n/gespraeche", "an update replaces an old file that lacks the key")

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
        self.assertIn('EINHEITEN="werkbank-kern.service werkbank-bruecke.service wb-halter.service"', lesen(os.path.join(HIER, PLATTFORM, "postinst")))
        self.assertIn("for e in wb-halter.service werkbank-bruecke.service werkbank-kern.service; do", lesen(os.path.join(HIER, PLATTFORM, "prerm")))

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

class Dienstordner(unittest.TestCase):
    """The block `dienstordner` of postinst (end of step 2): $DATEN/<nutzer>/dienst, the user, 0700, fail closed with the fixed code 29.
    The user is the one who runs the test (`id` is stubbed with the real uid and gid, `so_stat` reads the real owner and mode), so the owner check is real and needs no root;
    a foreign owner is a different uid in the stub. `install` ignores the owner (as in the class Sitzungsordner)."""

    UID, GID = os.getuid(), os.getgid()

    @classmethod
    def setUpClass(cls):
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.so = text[text.index("# --- sitzungsordner begin"):text.index("# --- sitzungsordner end")]
        cls.block = text[text.index("# --- dienstordner begin"):text.index("# --- dienstordner end")]

    def lauf(self, daten, uid=None, block=None, nutzer="anna", gid=None):
        """Runs the block against `daten` (the user folder `daten/<nutzer>` exists, as after step 2). Returns (rc, stderr)."""
        u = self.UID if uid is None else uid
        g = self.GID if gid is None else gid
        kopf = (
            'fail() { echo "F $1" >&2; exit "$1"; }\n'
            f"DATEN={shlex.quote(daten)}\nnutzer={nutzer}\n"
            f"id() {{ case \"$1\" in -u) echo {u} ;; -g) echo {g} ;; esac; }}\n"
        )
        code = 'import os,sys;s=os.lstat(sys.argv[1]);print("%d:%d:%o"%(s.st_uid,s.st_gid,s.st_mode&0o7777))'
        so = f"so_stat() {{ python3 -I -c '{code}' \"$1\"; }}\n"
        stub = ('install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; '
                '[ -e "$l" ] || { mkdir "$l" && chmod "$m" "$l"; }; }\n')
        r = subprocess.run(["sh", "-c", kopf + self.so + (block or self.block) + stub + so + "dienstordner"], capture_output=True, text=True, check=False)
        return r.returncode, r.stderr

    def frisch(self, t):
        d = os.path.join(os.path.realpath(t), "werkbank")
        os.makedirs(os.path.join(d, "anna"))
        return d, os.path.join(d, "anna", "dienst")

    def test_the_folder_is_made_with_the_mode_0700_and_the_user_as_owner_and_no_file_is_made(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            self.assertEqual(self.lauf(d), (0, ""))
            s = os.lstat(dienst)
            self.assertTrue(stat.S_ISDIR(s.st_mode))
            self.assertEqual((s.st_uid, s.st_gid, s.st_mode & 0o7777), (self.UID, self.GID, 0o700))
            self.assertEqual(os.listdir(dienst), [], "postinst never makes bestand.json")
            self.assertEqual(sorted(os.listdir(os.path.join(d, "anna"))), ["dienst"], "no other folder: not by the uid, not for root")
            self.assertEqual(self.lauf(d), (0, ""), "a second run changes nothing and passes")

    def test_an_update_leaves_an_existing_bestand_json_byte_for_byte(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.mkdir(dienst)
            os.chmod(dienst, 0o700)
            inhalt = b'{"fassung":1,"gestartet":3,"sitzungen":[]}\n\xff\x00 not even valid'
            datei = os.path.join(dienst, "bestand.json")
            rest = os.path.join(dienst, "bestand.json.neu.4242")
            for pfad, b in ((datei, inhalt), (rest, b"rest of a crashed run")):
                with open(pfad, "wb") as f:
                    f.write(b)
                os.chmod(pfad, 0o600)
            vorher = {n: (open(os.path.join(dienst, n), "rb").read(), os.lstat(os.path.join(dienst, n)).st_mtime_ns) for n in os.listdir(dienst)}
            self.assertEqual(self.lauf(d), (0, ""))
            self.assertEqual(self.lauf(d), (0, ""))
            nachher = {n: (open(os.path.join(dienst, n), "rb").read(), os.lstat(os.path.join(dienst, n)).st_mtime_ns) for n in os.listdir(dienst)}
            self.assertEqual(nachher, vorher, "content, name and time of every file stay")

    def test_a_foreign_owner_stops_with_28(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.mkdir(dienst)
            os.chmod(dienst, 0o700)
            rc, fehler = self.lauf(d, uid=self.UID + 1)
            self.assertEqual(rc, 29)
            self.assertIn("F 29", fehler)
            self.assertEqual(os.listdir(dienst), [], "nothing is changed")
            self.assertEqual(self.lauf(d)[0], 0, "the same folder is fine for its owner")

    def test_a_foreign_group_stops_with_28(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.mkdir(dienst)
            os.chmod(dienst, 0o700)
            self.assertEqual(self.lauf(d, gid=self.GID + 1)[0], 29, "the group comes from `id -g` of the user")
            self.assertEqual(self.lauf(d)[0], 0)

    def test_a_wrong_mode_stops_with_28_whatever_the_mode_is(self):
        for modus in (0o750, 0o755, 0o770, 0o701, 0o500, 0o2700):
            with self.subTest(modus=oct(modus)), tempfile.TemporaryDirectory() as t:
                d, dienst = self.frisch(t)
                os.mkdir(dienst)
                os.chmod(dienst, modus)
                self.assertEqual(self.lauf(d)[0], 29)
                self.assertEqual(os.lstat(dienst).st_mode & 0o7777, modus, "the mode is not corrected: nothing is changed")

    def test_a_symlink_stops_with_28_and_is_not_followed(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            ziel = os.path.join(os.path.realpath(t), "ziel")
            os.mkdir(ziel)
            os.chmod(ziel, 0o700)
            os.symlink(ziel, dienst)
            self.assertEqual(self.lauf(d)[0], 29)
            self.assertEqual(os.listdir(ziel), [])
            self.assertTrue(os.path.islink(dienst))
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.symlink(os.path.join(os.path.realpath(t), "gibt-es-nicht"), dienst)
            self.assertEqual(self.lauf(d)[0], 29, "a dangling symlink is no free place either")

    def test_a_file_instead_of_a_folder_stops_with_28(self):
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            with open(dienst, "w", encoding="utf-8") as f:
                f.write("x")
            os.chmod(dienst, 0o700)
            self.assertEqual(self.lauf(d)[0], 29)
            self.assertEqual(open(dienst, encoding="utf-8").read(), "x")

    def test_a_missing_user_folder_stops_with_28(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.makedirs(d)
            self.assertEqual(self.lauf(d)[0], 29, "the folder cannot be made: the installation does not go on without it")

    def test_without_the_check_the_deviations_pass(self):
        """Counter test: the same block with the call of `so_sicher` removed lets every deviation pass, so the tests above fail because of the check and nothing else."""
        ohne = self.block.replace('so_sicher "$DATEN/$nutzer/dienst" "$do_uid" "$do_gid" 700 29', ":")
        self.assertNotEqual(ohne, self.block)
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.mkdir(dienst)
            os.chmod(dienst, 0o750)
            self.assertEqual(self.lauf(d, block=ohne)[0], 0)
            self.assertEqual(self.lauf(d)[0], 29)
        with tempfile.TemporaryDirectory() as t:
            d, dienst = self.frisch(t)
            os.mkdir(dienst)
            os.chmod(dienst, 0o700)
            self.assertEqual(self.lauf(d, block=ohne, uid=self.UID + 1)[0], 0)

    def test_postinst_calls_the_block_after_the_user_folder_and_before_the_files_of_step_2a_and_2b(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        aufruf = text.index('if [ -n "$nutzer" ]; then\n    dienstordner\nfi\n')
        self.assertLess(text.index('nutzerordner || log "werkbank: Nutzerordner konnte nicht angelegt werden"'), aufruf)
        self.assertLess(aufruf, text.index("# Step 2a:"))
        self.assertLess(aufruf, text.index("# --- dienst.json begin"))
        self.assertEqual([z.strip() for z in text.split("\n")].count("dienstordner"), 1, "one call")
        self.assertEqual(text.count("dienstordner() {"), 1)
        # the file is never made by postinst: no command of postinst names bestand.json outside the comments and the text of kern.json
        for zeile in text.split("\n"):
            if "bestand.json" in zeile and not zeile.lstrip().startswith("#"):
                self.assertIn('"bestand":"%s/%s/dienst/bestand.json"', zeile)
                self.assertNotRegex(zeile, r"\b(touch|install|cp|mv|ln|tee)\b|>>?\s*\S*bestand")
        postrm = lesen(os.path.join(HIER, PLATTFORM, "postrm"))
        self.assertIn("rm -rf /var/lib/werkbank", postrm, "purge removes bestand.json with the state folder")


class Gespraecheordner(unittest.TestCase):
    """The block `gespraecheordner` of postinst (end of step 2): $DATEN/<nutzer>/gespraeche, the user, 0700, fail closed with the fixed code 30.
    Same set-up as the class Dienstordner: `id` is stubbed with the real uid and gid, `so_stat` reads the real owner and mode, a foreign owner is a different uid in the stub,
    `install` ignores the owner."""

    UID, GID = os.getuid(), os.getgid()

    @classmethod
    def setUpClass(cls):
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.so = text[text.index("# --- sitzungsordner begin"):text.index("# --- sitzungsordner end")]
        cls.block = text[text.index("# --- gespraecheordner begin"):text.index("# --- gespraecheordner end")]

    def lauf(self, daten, uid=None, block=None, nutzer="anna", gid=None):
        """Runs the block against `daten` (the user folder `daten/<nutzer>` exists, as after step 2). Returns (rc, stderr)."""
        u = self.UID if uid is None else uid
        g = self.GID if gid is None else gid
        kopf = (
            'fail() { echo "F $1" >&2; exit "$1"; }\n'
            f"DATEN={shlex.quote(daten)}\nnutzer={nutzer}\n"
            f"id() {{ case \"$1\" in -u) echo {u} ;; -g) echo {g} ;; esac; }}\n"
        )
        code = 'import os,sys;s=os.lstat(sys.argv[1]);print("%d:%d:%o"%(s.st_uid,s.st_gid,s.st_mode&0o7777))'
        so = f"so_stat() {{ python3 -I -c '{code}' \"$1\"; }}\n"
        stub = ('install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; '
                '[ -e "$l" ] || { mkdir "$l" && chmod "$m" "$l"; }; }\n')
        r = subprocess.run(["sh", "-c", kopf + self.so + (block or self.block) + stub + so + "gespraecheordner"], capture_output=True, text=True, check=False)
        return r.returncode, r.stderr

    def frisch(self, t):
        d = os.path.join(os.path.realpath(t), "werkbank")
        os.makedirs(os.path.join(d, "anna"))
        return d, os.path.join(d, "anna", "gespraeche")

    def test_the_folder_is_made_with_the_mode_0700_and_the_user_as_owner_and_nothing_is_put_into_it(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            self.assertEqual(self.lauf(d), (0, ""))
            s = os.lstat(g)
            self.assertTrue(stat.S_ISDIR(s.st_mode))
            self.assertEqual((s.st_uid, s.st_gid, s.st_mode & 0o7777), (self.UID, self.GID, 0o700))
            self.assertEqual(os.listdir(g), [], "postinst makes no conversation")
            self.assertEqual(sorted(os.listdir(os.path.join(d, "anna"))), ["gespraeche"], "no other folder: not by the uid, not for root")
            self.assertEqual(self.lauf(d), (0, ""), "a second run changes nothing and passes")

    def test_an_update_leaves_the_conversations_byte_for_byte(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.mkdir(g)
            os.chmod(g, 0o700)
            s = os.path.join(g, "s-1")
            os.mkdir(s)
            os.chmod(s, 0o700)
            datei = os.path.join(s, "gespraech.json")
            with open(datei, "wb") as f:
                f.write(b'{"fassung": 1}\n\xff\x00 not even valid')
            os.chmod(datei, 0o600)

            def stand():
                return sorted((os.path.relpath(os.path.join(w, n), g), os.lstat(os.path.join(w, n)).st_mtime_ns,
                               open(os.path.join(w, n), "rb").read() if os.path.isfile(os.path.join(w, n)) else None)
                              for w, ds, fs in os.walk(g) for n in ds + fs)
            vorher = stand()
            self.assertEqual(self.lauf(d), (0, ""))
            self.assertEqual(self.lauf(d), (0, ""))
            self.assertEqual(stand(), vorher, "content, name and time of every entry stay")

    def test_a_foreign_owner_stops_with_30(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.mkdir(g)
            os.chmod(g, 0o700)
            rc, fehler = self.lauf(d, uid=self.UID + 1)
            self.assertEqual(rc, 30)
            self.assertIn("F 30", fehler)
            self.assertEqual(os.listdir(g), [], "nothing is changed")
            self.assertEqual(self.lauf(d)[0], 0, "the same folder is fine for its owner")

    def test_a_foreign_group_stops_with_30(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.mkdir(g)
            os.chmod(g, 0o700)
            self.assertEqual(self.lauf(d, gid=self.GID + 1)[0], 30, "the group comes from `id -g` of the user")
            self.assertEqual(self.lauf(d)[0], 0)

    def test_a_wrong_mode_stops_with_30_whatever_the_mode_is(self):
        for modus in (0o750, 0o755, 0o770, 0o701, 0o500, 0o2700):
            with self.subTest(modus=oct(modus)), tempfile.TemporaryDirectory() as t:
                d, g = self.frisch(t)
                os.mkdir(g)
                os.chmod(g, modus)
                self.assertEqual(self.lauf(d)[0], 30)
                self.assertEqual(os.lstat(g).st_mode & 0o7777, modus, "the mode is not corrected: nothing is changed")

    def test_a_symlink_stops_with_30_and_is_not_followed(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            ziel = os.path.join(os.path.realpath(t), "ziel")
            os.mkdir(ziel)
            os.chmod(ziel, 0o700)
            os.symlink(ziel, g)
            self.assertEqual(self.lauf(d)[0], 30)
            self.assertEqual(os.listdir(ziel), [])
            self.assertTrue(os.path.islink(g))
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.symlink(os.path.join(os.path.realpath(t), "gibt-es-nicht"), g)
            self.assertEqual(self.lauf(d)[0], 30, "a dangling symlink is no free place either")

    def test_a_file_instead_of_a_folder_stops_with_30(self):
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            with open(g, "w", encoding="utf-8") as f:
                f.write("x")
            os.chmod(g, 0o700)
            self.assertEqual(self.lauf(d)[0], 30)
            self.assertEqual(open(g, encoding="utf-8").read(), "x")

    def test_a_missing_user_folder_stops_with_30(self):
        with tempfile.TemporaryDirectory() as t:
            d = os.path.join(os.path.realpath(t), "werkbank")
            os.makedirs(d)
            self.assertEqual(self.lauf(d)[0], 30, "the folder cannot be made: the installation does not go on without it")

    def test_an_unreadable_uid_or_gid_stops_with_30(self):
        for stub in ("id() { return 1; }", 'id() { case "$1" in -u) echo 1000 ;; -g) echo x ;; esac; }', 'id() { case "$1" in -u) echo "" ;; -g) echo 1000 ;; esac; }'):
            with self.subTest(stub=stub), tempfile.TemporaryDirectory() as t:
                d, g = self.frisch(t)
                r = subprocess.run(["sh", "-c", 'fail() { echo "F $1" >&2; exit "$1"; }\nDATEN=' + shlex.quote(d) + "\nnutzer=anna\n" + self.so + self.block + stub + "\ngespraecheordner"],
                                   capture_output=True, text=True, check=False)
                self.assertEqual(r.returncode, 30, r.stderr)
                self.assertFalse(os.path.exists(g), "nothing is made")

    def test_without_the_check_the_deviations_pass(self):
        """Counter test: the same block with the call of `so_sicher` removed lets every deviation pass, so the tests above fail because of the check and nothing else."""
        ohne = self.block.replace('so_sicher "$DATEN/$nutzer/gespraeche" "$go_uid" "$go_gid" 700 30', ":")
        self.assertNotEqual(ohne, self.block)
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.mkdir(g)
            os.chmod(g, 0o750)
            self.assertEqual(self.lauf(d, block=ohne)[0], 0)
            self.assertEqual(self.lauf(d)[0], 30)
        with tempfile.TemporaryDirectory() as t:
            d, g = self.frisch(t)
            os.mkdir(g)
            os.chmod(g, 0o700)
            self.assertEqual(self.lauf(d, block=ohne, uid=self.UID + 1)[0], 0)

    def test_postinst_calls_the_block_after_the_folder_dienst_and_before_the_files_of_step_2a_and_2b(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        aufruf = text.index('if [ -n "$nutzer" ]; then\n    gespraecheordner\nfi\n')
        self.assertLess(text.index('nutzerordner || log "werkbank: Nutzerordner konnte nicht angelegt werden"'), aufruf)
        self.assertLess(text.index('if [ -n "$nutzer" ]; then\n    dienstordner\nfi\n'), aufruf)
        self.assertLess(aufruf, text.index("# Step 2a:"))
        self.assertLess(aufruf, text.index("# --- dienst.json begin"))
        self.assertEqual([z.strip() for z in text.split("\n")].count("gespraecheordner"), 1, "one call")
        self.assertEqual(text.count("gespraecheordner() {"), 1)
        postrm = lesen(os.path.join(HIER, PLATTFORM, "postrm"))
        self.assertIn("rm -rf /var/lib/werkbank", postrm, "purge removes the conversations with the state folder")

    def test_no_command_of_postinst_makes_or_reads_an_entry_of_the_folder(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        stellen = [z for z in text.split("\n") if "gespraeche" in z and not z.lstrip().startswith("#")]
        erlaubt = ('so_sicher "$DATEN/$nutzer/gespraeche"', '"gespraeche":"%s/gespraeche"', '"gespraeche":"%s/%s/gespraeche"', "gespraecheordner")
        for z in stellen:
            self.assertTrue(any(e in z for e in erlaubt), z)
            self.assertNotRegex(z, r"\b(touch|cp|mv|ln|tee|rm|find)\b")

    def test_the_code_30_is_in_the_header_and_free_in_the_other_scripts(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        self.assertRegex(text.split("\n\n")[0], r"fixed code 29 or 30")
        self.assertEqual(len(re.findall(r"\bfail 30\b", text)), 4, "the four checks of the block, nowhere else")
        for name in ("postrm", "prerm"):
            self.assertNotRegex(lesen(os.path.join(HIER, PLATTFORM, name)), r"\b(exit|fail) 30\b", name)


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


def _journal_kopf(journal):
    """A logger that writes to a file instead of the journal, and stubs for the root-only steps."""
    return (f"logger() {{ shift 2; printf '%s\\n' \"$*\" >> {shlex.quote(journal)}; }}\n"
            "log() { logger -t werkbank-installation \"$1\"; return 0; }\n"
            'install() { while [ $# -gt 0 ]; do case "$1" in -d) ;; -m) m=$2; shift ;; -o|-g) shift ;; *) l=$1 ;; esac; shift; done; mkdir -p "$l"; chmod "$m" "$l"; }\n'
            "chown() { :; }\n")


def _journal(pfad):
    if not os.path.exists(pfad):
        return []
    with open(pfad, encoding="utf-8") as d:
        return d.read().splitlines()


class HalterJson(unittest.TestCase):
    """Addendum "Rolle aus der Gruppe": step 2c of postinst writes halter.json in the form $defs/konfig of the holder, from the template."""

    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, os.path.join(RUST, "spec", "paket"))
        import referenz
        cls.ref = referenz
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.block = text[text.index("# --- halter.json begin"):text.index("# --- halter.json end")]
        # Addendum first start: the template of step 2c is the first-start profile; the demo settings are the counter-check
        with open(os.path.join(RUST, "spec", "einstellungen", "erststart.json"), encoding="utf-8") as d:
            cls.vorlage = json.load(d)
        with open(os.path.join(RUST, "spec", "einstellungen", "vorfuehrung.json"), encoding="utf-8") as d:
            cls.vorfuehrung = json.load(d)

    MODELL_HOST = "api.individual.githubcopilot.com"
    # addendum Business: the model host of the plan Business carries the same paths and the same switch
    BUSINESS_HOST = "api.business.githubcopilot.com"
    MODELL_HOSTS = (MODELL_HOST, BUSINESS_HOST)
    ZWEI = ["GET /models", "POST /chat/completions"]
    # the targets with a key from the profile: both plans, never telemetry, exp or a subdomain form
    GEBUNDEN = ["api.business.githubcopilot.com", "api.github.com", "api.githubcopilot.com", "api.individual.githubcopilot.com"]

    def erststart_fehler(self, wert):
        """The faults of a halter.json against the first start: at each model host exactly the two paths and the switch for exactly them,
        no switch at any other target (empty: fine)."""
        fehler = []
        for host in self.MODELL_HOSTS:
            z = wert["ziele"].get(host)
            if z is None:
                fehler.append("kein Ziel an " + host)
                continue
            if z["pfade"] != self.ZWEI:
                fehler.append("pfade an %s: %d" % (host, len(z["pfade"])))
            if z.get("erststart_antwortform") != self.ZWEI:
                fehler.append("Schalter an " + host)
        fehler += ["Schalter an " + n for n, x in wert["ziele"].items() if n not in self.MODELL_HOSTS and "erststart_antwortform" in x]
        return fehler

    def ziel_fehler(self, wert, geheimnis="anmeldung.copilot"):
        """The faults of a halter.json against the target list of both plans (addendum Business): exactly the bound hosts, each with the one
        secret `geheimnis`, never a target at telemetry or at a subdomain form (empty: fine)."""
        fehler = []
        if sorted(wert["ziele"]) != self.GEBUNDEN:
            fehler.append("Ziele: " + ",".join(sorted(wert["ziele"])))
        fehler += ["Unterdomaene " + n for n in wert["ziele"] if n.startswith("*.") or wert["ziele"][n]["host"].startswith("*.")]
        fehler += ["Telemetrie " + n for n in wert["ziele"] if n.startswith("telemetry.")]
        fehler += ["Geheimnis an %s: %s" % (n, z["schluessel"]) for n, z in wert["ziele"].items() if z["schluessel"] != geheimnis]
        return fehler

    def schreiben(self, vorlage, uid="1000", name="erststart.json"):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            etc = os.path.join(t, "etc")
            vl = os.path.join(t, "vorlage")
            os.makedirs(vl)
            with open(os.path.join(vl, name), "w", encoding="utf-8") as d:
                json.dump(vorlage, d)
            journal = os.path.join(t, "journal")
            kopf = _journal_kopf(journal) + f"id() {{ printf '%s\\n' {shlex.quote(uid)}; }}\nETCW={shlex.quote(etc)}\nVORLAGE={shlex.quote(vl)}\nnutzer=nutzer1\n"
            r = subprocess.run(["sh", "-c", kopf + self.block + "halter_json"], capture_output=True, text=True, check=False)
            self.journal = _journal(journal)
            if r.returncode != 0:
                self.assertFalse(os.path.exists(os.path.join(etc, "halter.json")))
                return None, r.returncode
            pfad = os.path.join(etc, "halter.json")
            self.assertEqual(os.stat(pfad).st_mode & 0o7777, 0o644)
            self.assertEqual(sorted(os.listdir(etc)), ["halter.json"], "no temporary file stays behind")
            with open(pfad, "rb") as d:
                return d.read(), 0

    def test_the_file_is_the_reference_in_canonical_form(self):
        roh, rc = self.schreiben(self.vorlage)
        self.assertEqual(rc, 0)
        wert, weg = self.ref.halter_konfig(self.vorlage, 1000)
        self.assertEqual(weg, 0)
        self.assertEqual(roh, (json.dumps(wert, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode())
        self.assertEqual(self.journal, [])

    def test_the_first_start_profile_gives_two_paths_and_the_switch(self):
        # Addendum first start: from the profile in the package, two paths at the model host and the switch for exactly these two
        wert = json.loads(self.schreiben(self.vorlage)[0])
        self.assertEqual(self.erststart_fehler(wert), [])
        self.assertEqual(sorted(wert["ziele"]), self.GEBUNDEN)
        # counter-check: the demo settings as template give the seven paths at both model hosts, so the case is red
        roh, rc = self.schreiben(self.vorfuehrung)
        self.assertEqual(rc, 0)
        gegen = json.loads(roh)
        self.assertEqual(len(gegen["ziele"][self.MODELL_HOST]["pfade"]), 7)
        self.assertEqual(self.erststart_fehler(gegen), ["pfade an %s: 7" % h for h in self.MODELL_HOSTS])
        self.assertEqual(gegen, self.ref.halter_konfig(self.vorfuehrung, 1000)[0])

    def test_step_2c_reads_only_the_first_start_profile(self):
        # the demo settings next to it are never the template of halter.json: without erststart.json no file is written
        self.assertEqual(self.schreiben(self.vorlage, name="einstellungen.json")[1], 1)
        self.assertIn('[ -f "$VORLAGE/erststart.json" ] || return 1', self.block)
        self.assertNotIn("einstellungen.json", self.block)
        self.assertEqual(self.ref.HALTER_VORLAGE, "/usr/share/werkbank/probelauf/erststart.json")

    def test_the_switch_never_names_a_rule_outside_pfade(self):
        # the switch opens no path: a model host without POST /chat/completions gets the switch only for GET /models, without rule no field
        v = json.loads(json.dumps(self.vorlage))
        nz = [x for x in v["routen"][0]["netzziele"] if x["host"] == self.MODELL_HOST][0]
        nz["pfade"] = [{"methode": "GET", "pfad": "/models"}]
        wert = json.loads(self.schreiben(v)[0])
        self.assertEqual(wert["ziele"][self.MODELL_HOST]["erststart_antwortform"], ["GET /models"])
        self.assertEqual(wert, self.ref.halter_konfig(v, 1000)[0])
        nz["pfade"] = [{"methode": "GET", "pfad": "/anderes"}]
        wert = json.loads(self.schreiben(v)[0])
        self.assertNotIn("erststart_antwortform", wert["ziele"][self.MODELL_HOST])
        self.assertEqual(wert, self.ref.halter_konfig(v, 1000)[0])

    def test_the_package_delivers_the_first_start_profile(self):
        liste = lesen(os.path.join(HIER, PLATTFORM, "lieferumfang.txt"))
        self.assertIn("daten usr/share/werkbank/probelauf/erststart.json datei:spec/einstellungen/erststart.json\n", liste)
        self.assertIn("daten usr/share/werkbank/probelauf/einstellungen.json datei:spec/einstellungen/vorfuehrung.json\n", liste)

    def test_the_rules_of_the_template_stay_unchanged(self):
        # F12: the path rules (also those of api.github.com) come from the template as they are; this step changes none
        wert = json.loads(self.schreiben(self.vorlage)[0])
        for route in self.vorlage["routen"]:
            for nz in route["netzziele"]:
                if route["art"] != "cloud" or nz["schluessel"] is not True:
                    self.assertNotIn(nz["host"], wert["ziele"])
                    continue
                z = wert["ziele"][nz["host"]]
                self.assertEqual(z["pfade"], [f"{r['methode']} {r['pfad']}" for r in nz["pfade"]])
                self.assertEqual(z["schluessel"], "anmeldung." + route["schluessel"])
        self.assertEqual(wert["rollen"], {"bruecke": "wb-bruecke"})
        self.assertEqual(wert["konten"], {"1000": {"person": None}})
        self.assertEqual((wert["server"], wert["kids"]), ({}, {}))

    def test_the_file_passes_the_contract_of_the_holder(self):
        sys.path.insert(0, os.path.join(RUST, "spec", "halter"))
        import importlib
        pruefen = importlib.import_module("pruefen")
        # addendum Business: also the file with the admin switch, whose targets name anmeldung.copilot.eingeschraenkt
        mit_schalter = json.loads(json.dumps(self.vorlage))
        mit_schalter["routen"][0]["eingeschraenktes_token"] = True
        for vorlage in (self.vorlage, mit_schalter):
            self.vertrag_des_halters(pruefen, vorlage)

    def vertrag_des_halters(self, pruefen, vorlage):
        roh, _ = self.schreiben(vorlage)
        group = "root:x:0:\nnutzer1:x:1000:\nwb-bruecke:x:991:\n"
        passwd = "root:x:0:0:root:/root:/bin/sh\nnutzer1:x:1000:1000::/srv/beispiel/heim/nutzer1:/bin/sh\n"
        fall = {"form": "konfig", "umgebung": {"uid": 1000, "group": group, "passwd": passwd}}
        try:
            import jsonschema
        except ImportError:
            pruefer = None
        else:
            schema = json.loads(open(os.path.join(RUST, "spec", "halter", "halter.schema.json"), encoding="utf-8").read())
            pruefer = {"konfig": jsonschema.Draft202012Validator(dict(schema, **{"$ref": "#/$defs/konfig"}))}
        ist = pruefen.konfig_referenz(fall, roh, pruefer)
        if pruefer is None:
            self.assertEqual(ist, "BAU")
            self.assertIsNone(pruefen.rollengruppen(json.loads(roh), fall["umgebung"]))
        else:
            self.assertIsNone(ist)

    def test_a_target_without_identifier_or_rule_is_left_out_with_a_line(self):
        v = json.loads(json.dumps(self.vorlage))
        nz = v["routen"][0]["netzziele"]
        nz.append({"host": "Gross.Example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
                   "pfade": [{"methode": "GET", "pfad": "/a"}]})
        nz.append({"host": "nur-put.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
                   "pfade": [{"methode": "PUT", "pfad": "/a"}]})
        roh, rc = self.schreiben(v)
        self.assertEqual(rc, 0)
        wert = json.loads(roh)
        self.assertNotIn("Gross.Example", wert["ziele"])
        self.assertNotIn("nur-put.example", wert["ziele"])
        self.assertEqual(wert, self.ref.halter_konfig(v, 1000)[0])
        self.assertEqual(self.journal, ["werkbank: halter.json: Netzziele ausgelassen: 2"])

    def test_both_plans_bind_the_device_token_without_the_switch(self):
        # addendum Business: the targets of both plans; the key only at the exact hosts, never at telemetry or a subdomain form; without
        # the admin switch every target names the device token, as before
        wert = json.loads(self.schreiben(self.vorlage)[0])
        self.assertEqual(self.ziel_fehler(wert), [])
        self.assertEqual(wert["ziele"][self.BUSINESS_HOST]["pfade"], wert["ziele"][self.MODELL_HOST]["pfade"])
        self.assertEqual(wert, self.ref.halter_konfig(self.vorlage, 1000)[0])
        # counter-check of the case itself: a key at telemetry, at the subdomain form, a missing plan each give a fault
        for aendern, erwartet in [
            (lambda nz: nz.append({"host": "telemetry.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
                                   "schluessel": True, "pfade": [{"methode": "POST", "pfad": "/telemetry"}]}), "Telemetrie"),
            (lambda nz: nz.remove(next(x for x in nz if x["host"] == self.BUSINESS_HOST)), "Ziele"),
            (lambda nz: nz.remove(next(x for x in nz if x["host"] == self.MODELL_HOST)), "Ziele"),
        ]:
            v = json.loads(json.dumps(self.vorlage))
            aendern(v["routen"][0]["netzziele"])
            fehler = self.ziel_fehler(json.loads(self.schreiben(v)[0]))
            self.assertTrue(any(f.startswith(erwartet) for f in fehler), fehler)
        # a subdomain form never becomes a target of the holder: it has no identifier form, so it is left out with a line
        v = json.loads(json.dumps(self.vorlage))
        v["routen"][0]["netzziele"].append({"host": "*.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
                                            "schluessel": True, "pfade": [{"methode": "POST", "pfad": "/chat/completions"}]})
        wert = json.loads(self.schreiben(v)[0])
        self.assertEqual(self.ziel_fehler(wert), [])
        self.assertEqual(self.journal, ["werkbank: halter.json: Netzziele ausgelassen: 1"])

    def test_the_admin_switch_names_only_the_restricted_token(self):
        # addendum Business: with eingeschraenktes_token true every target of the route names the restricted token, none the device token;
        # whether the secret exists decides the holder (halter.kein_schluessel, fail closed), never this step
        v = json.loads(json.dumps(self.vorlage))
        v["routen"][0]["eingeschraenktes_token"] = True
        wert = json.loads(self.schreiben(v)[0])
        self.assertEqual(self.ziel_fehler(wert, "anmeldung.copilot.eingeschraenkt"), [])
        self.assertEqual(self.erststart_fehler(wert), [])
        self.assertEqual(wert, self.ref.halter_konfig(v, 1000)[0])
        self.assertEqual(self.journal, [])
        # false is the default: the same file as without the field
        v["routen"][0]["eingeschraenktes_token"] = False
        self.assertEqual(self.schreiben(v)[0], self.schreiben(self.vorlage)[0])
        # counter-check: the switch ignored or a fallback to the device token each give a fault at every target
        self.assertEqual(len(self.ziel_fehler(wert, "anmeldung.copilot")), len(self.GEBUNDEN))

    def test_a_switch_that_is_no_boolean_never_falls_back_to_the_device_token(self):
        # a value outside true and false (also 1 and "true") drops every target of the route with a line: no target, no token, fail closed
        for wert in (1, 0, "true", None, [True]):
            v = json.loads(json.dumps(self.vorlage))
            v["routen"][0]["eingeschraenktes_token"] = wert
            roh, rc = self.schreiben(v)
            self.assertEqual(rc, 0, wert)
            erg = json.loads(roh)
            self.assertEqual(erg["ziele"], {}, wert)
            self.assertEqual(erg, self.ref.halter_konfig(v, 1000)[0], wert)
            self.assertEqual(self.journal, ["werkbank: halter.json: Netzziele ausgelassen: %d" % len(self.GEBUNDEN)], wert)

    def test_a_broken_template_or_uid_writes_no_file(self):
        self.assertEqual(self.schreiben({"routen": "kaputt"})[1], 1)
        self.assertEqual(self.schreiben(self.vorlage, uid="10a")[1], 1)


class VorgabeJson(unittest.TestCase):
    """Addendum "Vorgabe der Firma": step 2d of postinst writes /etc/werkbank/vorgabe.json as a copy of the first-start profile."""

    @classmethod
    def setUpClass(cls):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        cls.block = text[text.index("# --- vorgabe.json begin"):text.index("# --- vorgabe.json end")]
        cls.profil = lesen(os.path.join(RUST, "spec", "einstellungen", "erststart.json"))

    def schreiben(self, mit_profil=True):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            etc, vl, bin_ = os.path.join(t, "etc"), os.path.join(t, "vorlage"), os.path.join(t, "bin")
            os.makedirs(vl)
            os.makedirs(bin_)
            if mit_profil:
                with open(os.path.join(vl, "erststart.json"), "w", encoding="utf-8") as d:
                    d.write(self.profil)
            # stand-ins: `install` without -o/-g and `chown` (a test is no root)
            with open(os.path.join(bin_, "install"), "w", encoding="utf-8") as d:
                d.write('#!/bin/sh\nargs=""\nwhile [ $# -gt 0 ]; do case "$1" in -o|-g) shift 2;; *) args="$args $1"; shift;; esac; done\nexec /usr/bin/install $args\n')
            os.chmod(os.path.join(bin_, "install"), 0o755)
            with open(os.path.join(bin_, "chown"), "w", encoding="utf-8") as d:
                d.write("#!/bin/sh\nexit 0\n")
            os.chmod(os.path.join(bin_, "chown"), 0o755)
            kopf = f"ETCW={shlex.quote(etc)}\nVORLAGE={shlex.quote(vl)}\n"
            r = subprocess.run(["sh", "-c", kopf + self.block + "vorgabe_json"], capture_output=True, text=True, check=False,
                               env={"PATH": bin_ + ":/usr/bin:/bin"})
            pfad = os.path.join(etc, "vorgabe.json")
            if not os.path.exists(pfad):
                return r.returncode, None, None
            self.assertEqual(sorted(os.listdir(etc)), ["vorgabe.json"], "no temporary file stays behind")
            return r.returncode, lesen(pfad), os.stat(pfad).st_mode & 0o7777

    def test_the_default_is_the_profile_with_mode_0644(self):
        rc, inhalt, modus = self.schreiben()
        self.assertEqual((rc, inhalt, modus), (0, self.profil, 0o644))
        # the default carries the targets of both plans; the key never at telemetry
        ziele = {z["host"]: z for z in json.loads(inhalt)["routen"][0]["netzziele"]}
        self.assertTrue(ziele["api.business.githubcopilot.com"]["schluessel"])
        self.assertFalse(ziele["telemetry.business.githubcopilot.com"]["schluessel"])

    def test_without_the_profile_no_default_is_written(self):
        self.assertEqual(self.schreiben(mit_profil=False), (1, None, None))
        self.assertNotIn("einstellungen.json", self.block)

    def test_purge_removes_the_default(self):
        self.assertIn("/etc/werkbank/vorgabe.json", lesen(os.path.join(HIER, PLATTFORM, "postrm")))


class Rollengruppe(unittest.TestCase):
    """Addendum "Rolle aus der Gruppe": step 1b of postinst (the group wb-bruecke and dpkg-statoverride) and its part in postrm purge."""

    @classmethod
    def setUpClass(cls):
        text = open(os.path.join(HIER, PLATTFORM, "postinst"), encoding="utf-8").read()
        cls.block = text[text.index("# --- rollengruppe begin"):text.index("# --- rollengruppe end")]
        rm = open(os.path.join(HIER, PLATTFORM, "postrm"), encoding="utf-8").read()
        cls.rm_block = rm[rm.index("# --- rolle begin"):rm.index("# --- rolle end")]
        cls.raeumen = rm[rm.index("ABLAGE_RAEUMEN='") + len("ABLAGE_RAEUMEN='"):rm.index("\nexit 0'") + len("\nexit 0")]

    def lauf(self, group, passwd="root:x:0:0:root:/root:/bin/sh\nnutzer1:x:1000:1000::/srv/beispiel/heim/nutzer1:/bin/sh\n",
             override="", override_rc=0):
        """Runs step 1b against made-up files; returns (calls, journal, group afterwards, mark there)."""
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            g, p, aufrufe, journal = (os.path.join(t, n) for n in ("group", "passwd", "aufrufe", "journal"))
            daten = os.path.join(t, "daten")
            with open(g, "w") as d:
                d.write(group)
            with open(p, "w") as d:
                d.write(passwd)
            kopf = _journal_kopf(journal) + (
                f"addgroup() {{ echo \"addgroup $*\" >> {shlex.quote(aufrufe)}; printf 'wb-bruecke:x:990:\\n' >> {shlex.quote(g)}; }}\n"
                f"dpkg_statoverride() {{ echo \"dpkg-statoverride $*\" >> {shlex.quote(aufrufe)}; "
                f"if [ \"$1\" = --list ]; then printf '%s' {shlex.quote(override)}; return 0; fi; return {override_rc}; }}\n"
                f"DATEN={shlex.quote(daten)}\n")
            block = self.block.replace("GROUP_DATEI=/etc/group", f"GROUP_DATEI={shlex.quote(g)}") \
                .replace("PASSWD_DATEI=/etc/passwd", f"PASSWD_DATEI={shlex.quote(p)}") \
                .replace("dpkg-statoverride", "dpkg_statoverride")
            r = subprocess.run(["sh", "-c", kopf + block + "rollengruppe"], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            with open(g) as d:
                nachher = d.read()
            marke = os.path.join(daten, "rollengruppe-angelegt")
            marke_text = open(marke).read() if os.path.exists(marke) else None
            return _journal(aufrufe), _journal(journal), nachher, marke_text

    def test_a_missing_group_is_made_marked_and_the_program_set(self):
        aufrufe, journal, nachher, marke = self.lauf("root:x:0:\n")
        self.assertEqual(aufrufe, ["addgroup --system wb-bruecke", "dpkg-statoverride --list /usr/lib/werkbank/werkbank-modellweg",
                                   "dpkg-statoverride --update --add root wb-bruecke 2755 /usr/lib/werkbank/werkbank-modellweg"])
        self.assertEqual((journal, marke), ([], "wb-bruecke\n"))
        self.assertIn("wb-bruecke:x:990:", nachher)

    def test_a_good_existing_group_is_taken_without_a_mark(self):
        aufrufe, journal, _, marke = self.lauf("root:x:0:\nwb-bruecke:!:991:\n")
        self.assertNotIn("addgroup --system wb-bruecke", aufrufe)
        self.assertIn("dpkg-statoverride --update --add root wb-bruecke 2755 /usr/lib/werkbank/werkbank-modellweg", aufrufe)
        self.assertEqual((journal, marke), ([], None))

    def test_a_group_of_another_form_is_refused_with_code_25_and_nothing_set(self):
        passwd = "root:x:0:0:root:/root:/bin/sh\nnutzer1:x:1000:1000::/srv/beispiel/heim/nutzer1:/bin/sh\n"
        for group, pw, grund in (("wb-bruecke:x:991:nutzer1\n", passwd, "member"),
                                 ("wb-bruecke:$6$a$b:991:\n", passwd, "password"),
                                 ("wb-bruecke:x:1000:\n", passwd, "gid outside 100 to 999"),
                                 ("wb-bruecke:x:0991:\n", passwd, "leading zero"),
                                 ("wb-bruecke:x:991:\nwb-bruecke:x:992:\n", passwd, "twice"),
                                 ("wb-bruecke:x:991:\n", passwd + "dienst:x:1002:991::/srv/beispiel/d:/usr/sbin/nologin\n", "primary group")):
            with self.subTest(grund):
                aufrufe, journal, nachher, marke = self.lauf("root:x:0:\n" + group, pw)
                self.assertEqual(aufrufe, [])
                self.assertEqual(journal, ["werkbank: Rollengruppe wb-bruecke abgelehnt, Code 25"])
                self.assertEqual((nachher, marke), ("root:x:0:\n" + group, None))

    def test_the_entry_of_statoverride_is_kept_replaced_or_reported(self):
        gut = "root wb-bruecke 2755 /usr/lib/werkbank/werkbank-modellweg"
        aufrufe, journal, _, _ = self.lauf("wb-bruecke:x:991:\n", override=gut)
        self.assertEqual(aufrufe, ["dpkg-statoverride --list /usr/lib/werkbank/werkbank-modellweg"])
        aufrufe, journal, _, _ = self.lauf("wb-bruecke:x:991:\n", override="root root 0755 /usr/lib/werkbank/werkbank-modellweg")
        self.assertEqual(aufrufe[1:], ["dpkg-statoverride --remove /usr/lib/werkbank/werkbank-modellweg",
                                       "dpkg-statoverride --update --add root wb-bruecke 2755 /usr/lib/werkbank/werkbank-modellweg"])
        _, journal, _, _ = self.lauf("wb-bruecke:x:991:\n", override_rc=2)
        self.assertEqual(journal, ["werkbank: Rollenprogramm nicht gesetzt, Code 26"])

    def entfernen(self, marke, override, delgroup_rc=0):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            aufrufe, journal = os.path.join(t, "aufrufe"), os.path.join(t, "journal")
            daten = os.path.join(t, "daten")
            os.makedirs(daten)
            if marke is not None:
                with open(os.path.join(daten, "rollengruppe-angelegt"), "w") as d:
                    d.write(marke)
            kopf = (f"logger() {{ shift 2; printf '%s\\n' \"$*\" >> {shlex.quote(journal)}; }}\n"
                    f"dpkg_statoverride() {{ echo \"dpkg-statoverride $*\" >> {shlex.quote(aufrufe)}; [ \"$1\" = --list ] && printf '%s' {shlex.quote(override)}; return 0; }}\n"
                    "getent() { return 0; }\n"
                    f"delgroup() {{ echo \"delgroup $*\" >> {shlex.quote(aufrufe)}; return {delgroup_rc}; }}\n"
                    f"DATEN={shlex.quote(daten)}\n")
            block = self.rm_block.replace("DATEN=/var/lib/werkbank\n", "").replace("dpkg-statoverride", "dpkg_statoverride")
            r = subprocess.run(["sh", "-c", kopf + block + "rolle_entfernen"], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            return _journal(aufrufe), _journal(journal)

    def test_purge_removes_the_own_entry_and_the_marked_group_only(self):
        gut = "root wb-bruecke 2755 /usr/lib/werkbank/werkbank-modellweg"
        aufrufe, journal = self.entfernen("wb-bruecke\n", gut)
        self.assertEqual(aufrufe, ["dpkg-statoverride --list /usr/lib/werkbank/werkbank-modellweg",
                                   "dpkg-statoverride --remove /usr/lib/werkbank/werkbank-modellweg",
                                   "delgroup --system --only-if-empty wb-bruecke"])
        self.assertEqual(journal, [])
        # a foreign entry and a group from before the installation (no mark) stay
        aufrufe, _ = self.entfernen(None, "root root 0755 /usr/lib/werkbank/werkbank-modellweg")
        self.assertEqual(aufrufe, ["dpkg-statoverride --list /usr/lib/werkbank/werkbank-modellweg"])
        # a group with a member stays, with code 27
        _, journal = self.entfernen("wb-bruecke\n", gut, delgroup_rc=1)
        self.assertEqual(journal, ["werkbank: Rollengruppe wb-bruecke bleibt, Code 27"])

    def test_purge_clears_the_chain_folder_of_the_holder(self):
        # F16: protokoll/ goes with the store, and with it halter/ and agent-workbench/; something foreign under .local/state stays
        with tempfile.TemporaryDirectory() as t0:
            heim = os.path.join(os.path.realpath(t0), "heim")
            h = os.path.join(heim, ".local", "state", "agent-workbench", "halter")
            for o in ("geheim", "protokoll"):
                os.makedirs(os.path.join(h, o))
                os.chmod(os.path.join(h, o), 0o700)
            for n, inhalt in (("geheim/anmeldung.copilot", "hgx-marke\n"), ("protokoll/halter-2026-11.jsonl", "{}\n"),
                              ("protokoll/halter.anker.jsonl", "{}\n")):
                with open(os.path.join(h, n), "w") as d:
                    d.write(inhalt)
            os.makedirs(os.path.join(heim, ".local", "state", "fremd"))
            r = subprocess.run(["sh", "-c", self.raeumen, "sh", heim], capture_output=True, text=True, check=False)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(sorted(os.listdir(os.path.join(heim, ".local", "state"))), ["fremd"])
            # counter-check: a link instead of protokoll is not followed, and halter/ stays
            os.makedirs(os.path.join(h, "geheim"))
            ziel = os.path.join(os.path.realpath(t0), "ziel")
            os.makedirs(ziel)
            with open(os.path.join(ziel, "datei"), "w") as d:
                d.write("bleibt\n")
            os.symlink(ziel, os.path.join(h, "protokoll"))
            subprocess.run(["sh", "-c", self.raeumen, "sh", heim], capture_output=True, text=True, check=False)
            self.assertEqual(os.listdir(ziel), ["datei"])
            self.assertTrue(os.path.islink(os.path.join(h, "protokoll")))


# --- The stand-ins of the tests are never delivered (decision of the line, 25.11.2026) ------------------------------
# The programs `einschluss-attrappe` and `lauf-attrappe` exist only with the features `attrappe` and `pruefbau`. Three
# conditions hold the delivery clean: (1) the positive list names no stand-in; (2) no call of the build scripts turns a
# feature on; (3) no crate turns the features on by default or through a dependency, and every stand-in program has
# `required-features`. Each check is a function on text, so the counter-checks below run it on a changed copy.
MERKMALE = ("attrappe", "pruefbau")
LIEFERBAU = ("paket.sh", os.path.join("paket", "bau.py"), os.path.join("paket", "bauhost.sh"))
FLAGGE = re.compile(r"(^|\s)(--features|--all-features|-F)(\s|=|$)")


def attrappen_in_liste(text):
    """The lines of the positive list that name a stand-in program or a source with `attrappe`."""
    return [z for z in text.splitlines() if not z.lstrip().startswith("#") and "attrappe" in z]


def merkmale_im_skript(text):
    """The non-comment lines of a build script that name a cargo feature option."""
    return [z for z in text.splitlines() if not z.lstrip().startswith("#") and FLAGGE.search(z)]


def merkmale_in_cargo_toml(text):
    """Findings in one Cargo.toml: a stand-in feature in `default`, in the features of a delivery dependency, or a
    stand-in program without `required-features`."""
    funde = []
    abschnitt = ""
    block = None  # (name, hat required-features) of the current [[bin]]
    for z in text.splitlines():
        t = z.strip()
        if t.startswith("["):
            if block is not None and "attrappe" in block[0] and not block[1]:
                funde.append(f"[[bin]] {block[0]} has no required-features")
            block = None
            abschnitt = t
            if t == "[[bin]]":
                block = ["", False]
            continue
        if block is not None:
            if t.startswith("name") and "=" in t:
                block[0] = t.split("=", 1)[1].strip().strip('"')
            if t.startswith("required-features"):
                block[1] = True
        if t.startswith("#") or not any(m in t for m in MERKMALE):
            continue
        if abschnitt == "[features]" and t.startswith("default"):
            funde.append(f"default feature: {t}")
        elif "features" in t and "dependencies" in abschnitt and "dev-dependencies" not in abschnitt:
            funde.append(f"dependency feature: {t}")
    if block is not None and "attrappe" in block[0] and not block[1]:
        funde.append(f"[[bin]] {block[0]} has no required-features")
    return funde


class OhneAttrappen(unittest.TestCase):
    def test_die_positivliste_nennt_keine_attrappe(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "lieferumfang.txt"))
        self.assertEqual(attrappen_in_liste(text), [])
        self.assertIn("programm usr/lib/werkbank/werkbank-einschluss", text)  # the check reads a real list

    def test_gegenprobe_der_eintrag_einer_attrappe_macht_die_pruefung_rot(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "lieferumfang.txt"))
        for zeile in ("programm usr/lib/werkbank/einschluss-attrappe bin:einschluss:einschluss-attrappe",
                      "programm usr/lib/werkbank/lauf-attrappe bin:lauf:lauf-attrappe"):
            self.assertEqual(attrappen_in_liste(text + zeile + "\n"), [zeile])
        # a comment that names the word stays free
        self.assertEqual(attrappen_in_liste(text + "# the attrappe is never delivered\n"), [])

    def test_kein_lieferbau_schaltet_ein_merkmal_ein(self):
        for name in LIEFERBAU:
            self.assertEqual(merkmale_im_skript(lesen(os.path.join(RUST, name))), [], name)

    def test_gegenprobe_ein_cargo_aufruf_mit_merkmal_ist_rot(self):
        for zeile in ("cargo build --release --locked --features pruefbau $CRATES",
                      "cargo build --release --all-features",
                      "RUSTFLAGS=x cargo build -F attrappe -p einschluss",
                      "cargo build --features=attrappe"):
            self.assertEqual(merkmale_im_skript(zeile + "\n"), [zeile])
        self.assertEqual(merkmale_im_skript("# cargo build --features pruefbau\ncargo build --release --locked\n"), [])

    def test_keine_kiste_schaltet_die_merkmale_ein_und_jede_attrappe_hat_required_features(self):
        crates = os.path.join(RUST, "crates")
        namen = sorted(os.listdir(crates))
        self.assertIn("einschluss", namen)
        self.assertIn("lauf", namen)
        for n in namen:
            toml = os.path.join(crates, n, "Cargo.toml")
            if os.path.isfile(toml):
                self.assertEqual(merkmale_in_cargo_toml(lesen(toml)), [], toml)
        wurzel = os.path.join(RUST, "Cargo.toml")
        self.assertEqual(merkmale_in_cargo_toml(lesen(wurzel)), [], wurzel)
        # the two stand-in programs exist and are the ones the checks are about
        for n, prog in (("einschluss", "einschluss-attrappe"), ("lauf", "lauf-attrappe")):
            self.assertIn(f'name = "{prog}"', lesen(os.path.join(crates, n, "Cargo.toml")))

    def test_gegenprobe_cargo_toml_mit_eingeschaltetem_merkmal_oder_ohne_required_features_ist_rot(self):
        basis = '[package]\nname = "x"\n\n[[bin]]\nname = "x-attrappe"\npath = "src/bin/attrappe.rs"\nrequired-features = ["attrappe"]\n\n[features]\nattrappe = []\n'
        self.assertEqual(merkmale_in_cargo_toml(basis), [])
        self.assertEqual(len(merkmale_in_cargo_toml(basis.replace("attrappe = []", 'default = ["attrappe"]'))), 1)
        self.assertEqual(len(merkmale_in_cargo_toml(basis + '\n[dependencies]\neinschluss = { path = "../einschluss", features = ["attrappe"] }\n')), 1)
        self.assertEqual(merkmale_in_cargo_toml(basis + '\n[dev-dependencies]\neinschluss = { path = "../einschluss", features = ["attrappe"] }\n'), [])
        ohne = basis.replace('required-features = ["attrappe"]\n', "")
        self.assertEqual(len(merkmale_in_cargo_toml(ohne)), 1)
        # a stand-in program as the last block of the file is checked too
        letzter = '[package]\nname = "x"\n\n[[bin]]\nname = "x-attrappe"\npath = "src/bin/attrappe.rs"\n'
        self.assertEqual(len(merkmale_in_cargo_toml(letzter)), 1)


def _block(skript, name):
    text = open(os.path.join(HIER, PLATTFORM, skript), encoding="utf-8").read()
    return text[text.index(f"# --- {name} begin"):text.index(f"# --- {name} end")]


def _referenz():
    sys.path.insert(0, os.path.join(RUST, "spec", "paket"))
    import referenz
    return referenz


class Vorgabe(unittest.TestCase):
    """Part A3: the account werkbank-vorgabe (preinst), the units, step 2d of postinst (folder and first admin) and purge (postrm), checks V-1 to
    V-17 against referenz.py. The blocks run against made-up files; the account of the helper is the user of the test."""

    @classmethod
    def setUpClass(cls):
        cls.ref = _referenz()
        cls.konto_block = _block("preinst", "vorgabe-konto")
        cls.block = _block("postinst", "firmenvorgabe")
        cls.rm_block = _block("postrm", "vorgabe")
        cls.uid, cls.gid = os.getuid(), os.getgid()

    # ---- V-1, V-2: preinst
    def konto(self, passwd, group, adduser_neu=None):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            p, g, aufrufe, journal = (os.path.join(t, n) for n in ("passwd", "group", "aufrufe", "journal"))
            for pfad, inhalt in ((p, passwd), (g, group)):
                with open(pfad, "w") as d:
                    d.write(inhalt)
            neu = adduser_neu or ("werkbank-vorgabe:x:990:990::/nonexistent:/usr/sbin/nologin\n", "werkbank-vorgabe:x:990:\n")
            kopf = _journal_kopf(journal) + (
                f"adduser() {{ echo \"adduser $*\" >> {shlex.quote(aufrufe)}; printf '%s' {shlex.quote(neu[0])} >> {shlex.quote(p)}; "
                f"printf '%s' {shlex.quote(neu[1])} >> {shlex.quote(g)}; }}\n")
            block = self.konto_block.replace("PASSWD_DATEI=/etc/passwd", f"PASSWD_DATEI={shlex.quote(p)}") \
                .replace("GROUP_DATEI=/etc/group", f"GROUP_DATEI={shlex.quote(g)}")
            r = subprocess.run(["sh", "-c", kopf + block + "vorgabe_konto"], capture_output=True, text=True, check=False)
            return r, _journal(aufrufe), _journal(journal), lesen(p), lesen(g)

    def test_v1_a_missing_account_is_made_in_the_form(self):
        basis_p, basis_g = "root:x:0:0:root:/root:/bin/sh\n", "root:x:0:\n"
        r, aufrufe, journal, p, g = self.konto(basis_p, basis_g)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(aufrufe, ["adduser --system --group --no-create-home --home /nonexistent --shell /usr/sbin/nologin werkbank-vorgabe"])
        self.assertIsNone(self.ref.vorgabe_konto_code(p, g))
        self.assertTrue(p.startswith(basis_p) and g.startswith(basis_g), "no other account changes")
        self.assertEqual(journal, [])
        # an account in the form stays as it is
        r, aufrufe, _, p2, g2 = self.konto(p, g)
        self.assertEqual((r.returncode, aufrufe, p2, g2), (0, [], p, g))

    def test_v2_a_foreign_account_stops_with_25_and_changes_nothing(self):
        gut_p = "werkbank-vorgabe:x:990:990::/nonexistent:/usr/sbin/nologin\n"
        gut_g = "werkbank-vorgabe:x:990:\n"
        faelle = [
            (gut_p.replace("/usr/sbin/nologin", "/bin/bash"), gut_g),
            (gut_p.replace("/nonexistent", "/var/lib/werkbank-vorgabe"), gut_g),
            (gut_p.replace(":990:990:", ":1000:1000:"), gut_g.replace("990", "1000")),
            (gut_p.replace(":990:990:", ":99:99:"), gut_g.replace("990", "99")),
            (gut_p.replace(":990:990:", ":990:991:"), gut_g),
            (gut_p, gut_g.replace("990:", "990:nutzer1")),
            (gut_p, ""),
            ("", gut_g),
            (gut_p + gut_p, gut_g),
            (gut_p.replace(":990:990:", ":0990:0990:"), gut_g.replace(":990:", ":0990:")),
        ]
        for p, g in faelle:
            with self.subTest(p=p, g=g):
                r, aufrufe, journal, p2, g2 = self.konto(p, g)
                self.assertEqual(r.returncode, 25, r.stderr)
                self.assertEqual(r.stderr.strip(), self.ref.VORGABE_ZEILE_25)
                self.assertEqual((aufrufe, p2, g2), ([], p, g))
                self.assertEqual(journal, [self.ref.VORGABE_ZEILE_25])
                self.assertEqual(self.ref.vorgabe_konto_code(p, g), "fremd")
        # adduser that makes something of another form: 25 as well
        r, _, _, _, _ = self.konto("", "", ("werkbank-vorgabe:x:990:990::/:/bin/sh\n", "werkbank-vorgabe:x:990:\n"))
        self.assertEqual(r.returncode, 25)
        self.assertIsNone(self.ref.vorgabe_konto_code(gut_p, gut_g))
        self.assertEqual(self.ref.vorgabe_konto_code("", ""), "fehlt")

    def test_preinst_runs_only_for_install_and_upgrade(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "preinst"))
        self.assertIn('    install | upgrade) ;;\n    *) exit 0 ;;', text)
        r = subprocess.run(["sh", os.path.join(HIER, PLATTFORM, "preinst"), "abort-upgrade"], capture_output=True, text=True, check=False)
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, "", ""))

    # ---- V-3, V-4, V-5: units and the list
    def test_v3_v4_the_units_have_the_form_of_part_a3(self):
        sock = lesen(os.path.join(HIER, PLATTFORM, "werkbank-vorgabe.socket"))
        dienst = lesen(os.path.join(HIER, PLATTFORM, "werkbank-vorgabe.service"))
        self.assertIsNone(self.ref.vorgabe_einheit_code(sock, dienst))
        self.assertEqual([z for z in sock.splitlines() if z.startswith("Listen")], ["ListenStream=/run/werkbank-vorgabe/vorgabe.sock"])
        self.assertEqual([z for z in dienst.splitlines() if z.startswith("ReadWritePaths")], ["ReadWritePaths=/etc/werkbank/vorgabe"])
        # counter-checks: each deviation is found
        for alt, neu in (("SocketMode=0666", "SocketMode=0660"), ("Accept=no", "Accept=yes"),
                         ("WantedBy=sockets.target", "WantedBy=sockets.target\nListenStream=/tmp/x")):
            self.assertEqual(self.ref.vorgabe_einheit_code(sock.replace(alt, neu), dienst), "socket", neu)
        for alt, neu, code in (
                ("ReadWritePaths=/etc/werkbank/vorgabe", "ReadWritePaths=/etc/werkbank", "pflicht:ReadWritePaths"),
                ("PrivateNetwork=yes", "PrivateNetwork=no", "pflicht:PrivateNetwork"),
                ("RestrictAddressFamilies=AF_UNIX", "RestrictAddressFamilies=AF_UNIX AF_INET", "pflicht:RestrictAddressFamilies"),
                ("CapabilityBoundingSet=", "CapabilityBoundingSet=CAP_CHOWN", "pflicht:CapabilityBoundingSet"),
                ("User=werkbank-vorgabe", "User=root", "pflicht:User"),
                ("RemoveIPC=yes", "RemoveIPC=yes\nSupplementaryGroups=adm", "verboten:SupplementaryGroups"),
                ("RemoveIPC=yes", "RemoveIPC=yes\nEnvironment=A=1", "verboten:Environment"),
                ("RemoveIPC=yes", "RemoveIPC=yes\n[Install]\nWantedBy=multi-user.target", "install")):
            self.assertEqual(self.ref.vorgabe_einheit_code(sock, dienst.replace(alt, neu)), code, neu)

    def test_v5_the_list_has_the_four_paths_and_nothing_below_the_folder_of_the_helper(self):
        positiv = self.ref.liste_lesen(lesen(os.path.join(HIER, PLATTFORM, "lieferumfang.txt")), PLATTFORM)
        for eintrag in (("dienst", "usr/lib/werkbank/werkbank-vorgabe", "bin:vorgabe-helfer:werkbank-vorgabe"),
                        ("daten", "usr/lib/systemd/system/werkbank-vorgabe.socket", "datei:paket/ubuntu-26.04-x64/werkbank-vorgabe.socket"),
                        ("daten", "usr/lib/systemd/system/werkbank-vorgabe.service", "datei:paket/ubuntu-26.04-x64/werkbank-vorgabe.service"),
                        ("daten", "usr/share/werkbank/vorgabe/firmenvorgabe.json", "datei:paket/ubuntu-26.04-x64/firmenvorgabe.json")):
            self.assertIn(eintrag, positiv)
        self.assertFalse([z for _, z, _ in positiv if z.startswith("etc/werkbank/vorgabe")])
        self.assertEqual(json.loads(lesen(os.path.join(HIER, PLATTFORM, "firmenvorgabe.json"))), self.ref.VORGABE_VORLAGE)
        self.assertEqual(lesen(os.path.join(HIER, PLATTFORM, "firmenvorgabe.json")).encode(),
                         (json.dumps(self.ref.VORGABE_VORLAGE, indent=1, sort_keys=True) + "\n").encode())

    # ---- V-6 to V-8, V-12 to V-17: postinst step 2d
    def erst(self, *, ordner="fehlt", eintrag=None, sudo_user="nutzer1", konten=None, vg_uid=None, vorlage=None, erscheint=False):
        """Runs step 2d against a folder of the test; returns what the folder holds afterwards and the journal."""
        konten = {"nutzer1": 1000, "nutzer2": 1001, "root": 0} if konten is None else konten
        vg_uid = self.uid if vg_uid is None else vg_uid
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            eltern = os.path.join(t, "etc-werkbank")
            os.mkdir(eltern)
            vg = os.path.join(eltern, "vorgabe")
            vorlage_pfad = os.path.join(t, "vorlage.json")
            with open(vorlage_pfad, "w") as d:
                d.write(lesen(os.path.join(HIER, PLATTFORM, "firmenvorgabe.json")) if vorlage is None else vorlage)
            if ordner == "ordner":
                os.mkdir(vg)
                os.chmod(vg, 0o755)
            elif ordner == "ordner-0775":
                os.mkdir(vg)
                os.chmod(vg, 0o775)
            elif ordner == "verweis":
                os.mkdir(os.path.join(t, "anderswo"))
                os.symlink(os.path.join(t, "anderswo"), vg)
            ziel = os.path.join(vg, "firmenvorgabe.json")
            vorher = {"datei": b'{"admins": ["konto-it"], "leseklassen": []}', "kaputt": b"kein json",
                      "ohne-liste": b'{"leseklassen": []}'}.get(eintrag)
            if vorher is not None:
                with open(ziel, "wb") as d:
                    d.write(vorher)
                os.chmod(ziel, 0o600)
            if eintrag == "verweis":
                with open(os.path.join(t, "andere.json"), "wb") as d:
                    d.write(b"andere")
                os.symlink(os.path.join(t, "andere.json"), ziel)
            journal = os.path.join(t, "journal")
            passwd = "".join(f"{n}:x:{u}:{u}::/x:/bin/sh\n" for n, u in konten.items())
            getent = (
                "getent() { case \"$1:$2\" in "
                f"passwd:werkbank-vorgabe) echo 'werkbank-vorgabe:x:{vg_uid}:{self.gid}::/nonexistent:/usr/sbin/nologin' ;; "
                f"group:werkbank-vorgabe) echo 'werkbank-vorgabe:x:{self.gid}:' ;; "
                f"passwd:*) printf '%s' {shlex.quote(passwd)} | awk -F: -v u=\"$2\" '$3==u' ;; *) return 2 ;; esac; }}\n"
                f"id() {{ [ \"$1\" = -u ] || return 1; printf '%s' {shlex.quote(passwd)} | awk -F: -v n=\"$2\" '$1==n {{print $3; f=1}} END {{exit !f}}'; }}\n")
            block = self.block.replace("VG_ORDNER=/etc/werkbank/vorgabe", f"VG_ORDNER={shlex.quote(vg)}") \
                .replace("VG_ELTERN=/etc/werkbank", f"VG_ELTERN={shlex.quote(eltern)}") \
                .replace("VG_VORLAGE=/usr/share/werkbank/vorgabe/firmenvorgabe.json", f"VG_VORLAGE={shlex.quote(vorlage_pfad)}")
            if erscheint:
                block = block.replace("    os.link(temp, name,", "    open(os.path.join(ordner, name), \"wb\").write(b\"von der IT\")\n    os.link(temp, name,")
                self.assertIn("von der IT", block)
            umgebung = {k: v for k, v in os.environ.items() if k != "SUDO_USER"}
            if sudo_user is not None:
                umgebung["SUDO_USER"] = sudo_user
            r = subprocess.run(["sh", "-c", _journal_kopf(journal) + getent + block + "firmenvorgabe"], capture_output=True, text=True,
                               check=False, env=umgebung)
            self.assertEqual(r.returncode, 0, r.stderr)
            inhalt, modus, namen, uid = None, None, None, None
            if os.path.isdir(vg) and not os.path.islink(vg):
                namen = sorted(os.listdir(vg))
            if os.path.lexists(ziel):
                if os.path.islink(ziel):
                    inhalt = os.readlink(ziel).encode()
                else:
                    with open(ziel, "rb") as d:
                        inhalt = d.read()
                modus = os.lstat(ziel).st_mode & 0o7777
                uid = os.lstat(ziel).st_uid
            andere = None
            if eintrag == "verweis":
                with open(os.path.join(t, "andere.json"), "rb") as d:
                    andere = d.read()
            ordner_modus = os.lstat(vg).st_mode & 0o7777 if os.path.lexists(vg) else None
            return {"journal": _journal(journal), "inhalt": inhalt, "modus": modus, "namen": namen, "vorher": vorher, "andere": andere,
                    "ordner_modus": ordner_modus, "uid": uid}

    def soll(self, **fall):
        return self.ref.vorgabe_erstausstattung(fall)

    def test_v6_a_fresh_install_makes_folder_and_file_with_the_installing_account(self):
        e = self.erst()
        self.assertEqual(e["ordner_modus"], 0o755)
        self.assertEqual(e["inhalt"], self.ref.vorgabe_inhalt("nutzer1"))
        self.assertEqual((e["modus"], e["uid"], e["namen"]), (0o644, self.uid, ["firmenvorgabe.json"]))
        self.assertEqual(e["journal"], [])
        s = self.soll(ordner=None, eintrag=None, sudo_user="nutzer1", uid_von={"nutzer1": 1000}, name_von={1000: "nutzer1"},
                      vorlage=self.ref.VORGABE_VORLAGE)
        self.assertEqual((s["datei"], s["journal"]), (e["inhalt"], e["journal"]))
        # an existing folder in the form is taken as it is
        e = self.erst(ordner="ordner")
        self.assertEqual((e["inhalt"], e["journal"]), (self.ref.vorgabe_inhalt("nutzer1"), []))
        # the name comes from passwd by the uid, never from the variable
        e = self.erst(sudo_user="nutzer2", konten={"nutzer2": 1001, "nutzer1": 1001})
        self.assertEqual(e["inhalt"], self.ref.vorgabe_inhalt("nutzer2"))

    def test_v7_v12_an_existing_company_default_stays_byte_for_byte(self):
        for sudo_user in ("nutzer2", "nutzer1", None):
            e = self.erst(ordner="ordner", eintrag="datei", sudo_user=sudo_user)
            self.assertEqual((e["inhalt"], e["modus"]), (e["vorher"], 0o600), sudo_user)
            self.assertEqual(e["journal"], [], "no code 28 for an existing file")
            self.assertNotIn(b"nutzer", e["inhalt"])
        self.assertEqual(self.soll(ordner={"art": "ordner", "eigner_helfer": True, "modus": 0o755}, eintrag="datei", sudo_user="nutzer2"),
                         {"datei": "unveraendert", "journal": []})

    def test_v8_v13_a_folder_of_another_form_changes_nothing_and_gives_26(self):
        fremd = {"verweis": {"art": "verweis", "eigner_helfer": True, "modus": 0o755},
                 "ordner": {"art": "ordner", "eigner_helfer": False, "modus": 0o755},
                 "ordner-0775": {"art": "ordner", "eigner_helfer": True, "modus": 0o775}}
        for ordner, vg_uid, eintrag in (("verweis", None, None), ("ordner", self.uid + 1, None), ("ordner", self.uid + 1, "datei"),
                                        ("ordner-0775", None, None)):
            with self.subTest(ordner=ordner, eintrag=eintrag):
                e = self.erst(ordner=ordner, vg_uid=vg_uid, eintrag=eintrag)
                self.assertEqual(e["journal"], [self.ref.VORGABE_ZEILE_26])
                self.assertEqual(e["inhalt"], e["vorher"])
                self.assertEqual(self.soll(ordner=fremd[ordner], eintrag=eintrag, sudo_user="nutzer1")["journal"], [self.ref.VORGABE_ZEILE_26])
        self.assertEqual(self.erst(ordner="ordner-0775")["ordner_modus"], 0o775, "nothing changed")

    def test_v14_without_an_installing_account_no_file_and_code_28(self):
        for sudo_user, konten in ((None, None), ("", None), ("root", None), ("fehlt", None), ("Nutzer1", {"Nutzer1": 1005}),
                                  ("nutzer1", {"nutzer1": 0}), ("x", {"1nutzer": 1005, "x": 1005})):
            with self.subTest(sudo_user=sudo_user):
                e = self.erst(sudo_user=sudo_user, konten=konten)
                self.assertEqual((e["inhalt"], e["journal"], e["namen"]), (None, [self.ref.VORGABE_ZEILE_28], []))
        self.assertEqual(self.soll(ordner=None, sudo_user=""), {"datei": None, "journal": [self.ref.VORGABE_ZEILE_28]})
        self.assertEqual(self.soll(ordner=None, sudo_user="root", uid_von={"root": 0}, name_von={0: "root"})["datei"], None)
        # a later run with an account (dpkg-reconfigure) makes the file after V-6
        self.assertEqual(self.erst(sudo_user="nutzer1")["inhalt"], self.ref.vorgabe_inhalt("nutzer1"))

    def test_v15_an_invalid_existing_entry_stays_and_is_not_followed(self):
        for eintrag in ("kaputt", "ohne-liste", "verweis"):
            with self.subTest(eintrag=eintrag):
                e = self.erst(ordner="ordner", eintrag=eintrag)
                self.assertEqual(e["journal"], [])
                if eintrag == "verweis":
                    self.assertTrue(e["inhalt"].endswith(b"andere.json"))
                    self.assertEqual(e["andere"], b"andere")
                else:
                    self.assertEqual(e["inhalt"], e["vorher"])

    def test_v16_a_file_that_appears_before_the_link_stays_and_no_temporary_file(self):
        e = self.erst(ordner="ordner", erscheint=True)
        self.assertEqual(e["inhalt"], b"von der IT")
        self.assertEqual(e["namen"], ["firmenvorgabe.json"])
        self.assertEqual(e["journal"], [self.ref.VORGABE_ZEILE_26])
        self.assertEqual(self.soll(ordner=None, sudo_user="nutzer1", uid_von={"nutzer1": 1000}, name_von={1000: "nutzer1"},
                                   vorlage=self.ref.VORGABE_VORLAGE, erscheint=True)["journal"], [self.ref.VORGABE_ZEILE_26])

    def test_a_wrong_template_gives_26_and_no_file(self):
        for vorlage in ('{"admins": ["x"], "leseklassen": []}', "kein json", '{"admins": [], "leseklassen": [], "y": 1}'):
            e = self.erst(vorlage=vorlage)
            self.assertEqual((e["inhalt"], e["journal"], e["namen"]), (None, [self.ref.VORGABE_ZEILE_26], []))

    def test_v17_the_block_never_overwrites_never_reads_and_takes_the_account_from_passwd(self):
        self.assertIsNone(self.ref.vorgabe_block_code(self.block))
        for alt, neu, code in (
                ("os.link(temp, name,", "os.rename(temp, name,", "ueberschreiben"),
                ("os.link(temp, name, src_dir_fd=fd, dst_dir_fd=fd, follow_symlinks=False)",
                 "os.replace(temp, name, src_dir_fd=fd, dst_dir_fd=fd)", "ueberschreiben"),
                ("os.O_EXCL | ", "", "verlinken"),
                ("    return 0\n}\n\nvorgabe_einheiten", "    cat \"$VG_ORDNER/firmenvorgabe.json\" >/dev/null\n    return 0\n}\n\nvorgabe_einheiten",
                 "lesen"),
                ('vg_k=$(getent passwd "$vg_kuid"', 'vg_k=$(printf %s "$SUDO_USER"', "konto")):
            self.assertIn(alt, self.block)
            self.assertEqual(self.ref.vorgabe_block_code(self.block.replace(alt, neu)), code, neu)

    def test_postinst_runs_step_2d_before_the_enclosure_and_enables_the_socket(self):
        text = lesen(os.path.join(HIER, PLATTFORM, "postinst"))
        self.assertLess(text.index("\nfirmenvorgabe\nvorgabe_einheiten\n"), text.index("\neinschluss\n"))
        self.assertIn("systemctl enable --now werkbank-vorgabe.socket", self.block)
        self.assertNotIn("werkbank-vorgabe.service", self.block, "only the socket starts the helper")

    # ---- V-9 to V-11: remove and purge
    def purge(self, findmnt="/ ext4\n", findmnt_rc=0, find_aus="", find_rc=0, pgrep_rc=1, konto=True):
        with tempfile.TemporaryDirectory() as t0:
            t = os.path.realpath(t0)
            binordner = os.path.join(t, "bin")
            os.mkdir(binordner)
            aufrufe, journal = os.path.join(t, "aufrufe"), os.path.join(t, "journal")
            for name, aus, rc in (("findmnt", findmnt, findmnt_rc), ("find", find_aus, find_rc), ("pgrep", "", pgrep_rc)):
                with open(os.path.join(binordner, name), "w") as d:
                    d.write(f"#!/bin/sh\necho \"{name} $*\" >> {shlex.quote(aufrufe)}\nprintf '%s' {shlex.quote(aus)}\nexit {rc}\n")
                os.chmod(os.path.join(binordner, name), 0o755)
            vg = os.path.join(t, "vorgabe")
            os.mkdir(vg)
            with open(os.path.join(vg, "firmenvorgabe.json"), "w") as d:
                d.write("{}")
            os.symlink(os.path.join(t, "ziel"), os.path.join(vg, "verweis"))
            with open(os.path.join(t, "ziel"), "w") as d:
                d.write("bleibt")
            kopf = _journal_kopf(journal) + (
                f"systemctl() {{ echo \"systemctl $*\" >> {shlex.quote(aufrufe)}; }}\n"
                f"deluser() {{ echo \"deluser $*\" >> {shlex.quote(aufrufe)}; }}\n"
                f"delgroup() {{ echo \"delgroup $*\" >> {shlex.quote(aufrufe)}; }}\n"
                "getent() { case \"$1:$2\" in "
                + ("passwd:werkbank-vorgabe) echo 'werkbank-vorgabe:x:990:990::/nonexistent:/usr/sbin/nologin' ;; "
                   "group:werkbank-vorgabe) echo 'werkbank-vorgabe:x:990:' ;; " if konto else "")
                + "*) return 2 ;; esac; }\n")
            block = self.rm_block.replace("VG_ORDNER=/etc/werkbank/vorgabe", f"VG_ORDNER={shlex.quote(vg)}") \
                .replace("rm -rf /run/werkbank-vorgabe", "true")
            # step 2 uses the find of the system; the fake find in PATH answers only the rest check of python
            block = block.replace("        find \"$VG_ORDNER\"", "        /usr/bin/find \"$VG_ORDNER\"")
            self.assertIn("/usr/bin/find \"$VG_ORDNER\"", block)
            umgebung = dict(os.environ, PATH=binordner + os.pathsep + "/usr/bin:/bin")
            r = subprocess.run(["sh", "-c", kopf + block + "vorgabe_purge"], capture_output=True, text=True, check=False, env=umgebung)
            self.assertEqual(r.returncode, 0, r.stderr)
            return {"aufrufe": _journal(aufrufe), "journal": _journal(journal), "ordner": os.path.exists(vg),
                    "ziel": lesen(os.path.join(t, "ziel"))}

    def test_v10_purge_without_a_rest_removes_folder_account_and_group(self):
        e = self.purge(findmnt="/ ext4\n/boot/efi vfat\n/run tmpfs\n/proc proc\n")
        self.assertFalse(e["ordner"])
        self.assertEqual(e["ziel"], "bleibt", "a link in the folder is never followed")
        self.assertEqual(e["journal"], [])
        self.assertIn("deluser --system werkbank-vorgabe", e["aufrufe"])
        self.assertIn("delgroup --system werkbank-vorgabe", e["aufrufe"])
        self.assertIn("find / -xdev ( -uid 990 -o -gid 990 ) -print -quit", e["aufrufe"])
        self.assertIn("find /boot/efi -xdev ( -uid 990 -o -gid 990 ) -print -quit", e["aufrufe"])
        self.assertFalse([a for a in e["aufrufe"] if a.startswith(("find /run", "find /proc"))])
        self.assertIn("pgrep -u werkbank-vorgabe", e["aufrufe"])
        self.assertEqual(e["aufrufe"][:2], ["systemctl stop werkbank-vorgabe.socket werkbank-vorgabe.service",
                                            "systemctl disable werkbank-vorgabe.socket"])
        self.assertEqual(self.ref.vorgabe_purge({"punkte": [{"typ": "ext4"}, {"typ": "tmpfs"}]}), {"konto_bleibt": False, "journal": []})
        # a mount point with a blank comes back from the escape of findmnt
        e = self.purge(findmnt="/ ext4\n/mnt/mit\\x20leer ext4\n")
        self.assertIn("find /mnt/mit leer -xdev ( -uid 990 -o -gid 990 ) -print -quit", e["aufrufe"])

    def test_v11_purge_with_a_rest_keeps_account_and_group_with_code_27(self):
        faelle = (
            ({"find_aus": "/srv/beispiel/datei\n"}, {"punkte": [{"typ": "ext4", "treffer": True}]}),
            ({"find_rc": 1}, {"punkte": [{"typ": "ext4", "find_fehler": True}]}),
            ({"pgrep_rc": 0}, {"punkte": [{"typ": "ext4"}], "prozesse": True}),
            ({"pgrep_rc": 2}, {"prozesse": True}),
            ({"findmnt_rc": 1}, {"findmnt_fehler": True}),
            ({"findmnt": ""}, {"findmnt_fehler": True}),
            ({"findmnt": "/ ext4\n/daten neufs\n"}, {"punkte": [{"typ": "ext4"}, {"typ": "neufs"}]}),
            ({"findmnt": "/ ext4 zuviel\n"}, {"findmnt_fehler": True}),
        )
        for lage, fall in faelle:
            with self.subTest(lage=lage):
                e = self.purge(**lage)
                self.assertEqual(e["journal"], [self.ref.VORGABE_ZEILE_27])
                self.assertFalse([a for a in e["aufrufe"] if a.startswith(("deluser", "delgroup"))])
                self.assertEqual(self.ref.vorgabe_purge(fall), {"konto_bleibt": True, "journal": [self.ref.VORGABE_ZEILE_27]})
        # without the account there is nothing to delete and no line
        e = self.purge(konto=False)
        self.assertEqual((e["journal"], [a for a in e["aufrufe"] if a.startswith(("deluser", "find"))]), ([], []))

    def test_the_lists_of_the_rest_check_are_the_lists_of_the_reference(self):
        text = self.rm_block
        a = text.index("AUSGENOMMEN = {")
        d = text.index("DURCHSUCHT = {")
        aus = set(re.findall(r'"([a-z0-9._-]+)"', text[a:text.index("}", a)]))
        durch = set(re.findall(r'"([a-z0-9._-]+)"', text[d:text.index("}", d)]))
        self.assertEqual((aus, durch), (set(self.ref.MOUNT_AUSGENOMMEN), set(self.ref.MOUNT_DURCHSUCHT)))

    def test_v9_remove_and_prerm_stop_the_units_and_keep_account_folder_and_file(self):
        rm = lesen(os.path.join(HIER, PLATTFORM, "postrm"))
        teil = rm[rm.index("    remove)\n"):rm.index("    purge)\n")]
        self.assertIn("systemctl disable werkbank-vorgabe.socket", teil)
        self.assertNotIn("vorgabe_purge", teil)
        self.assertNotIn("deluser", teil)
        self.assertIn("vorgabe_purge", rm[rm.index("    purge)\n"):])
        pre = lesen(os.path.join(HIER, PLATTFORM, "prerm"))
        self.assertIn("systemctl stop werkbank-vorgabe.socket werkbank-vorgabe.service", pre)


if __name__ == "__main__":
    unittest.main()

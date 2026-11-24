#!/usr/bin/env python3
"""Build helper of `rust/paket.sh deb` (contract rust/spec/paket, part A, steps 3 to 6).

It uses the reference checker of the contract (`rust/spec/paket/referenz.py`) for every contract check, so that the build
and the visible cases of the contract cannot drift apart. Standard library only. Exit codes: 0 done, 65 the package
violates the contract (the line `paket.<code>: <reason>` goes to stderr, then up to 20 indented lines), 70 build error,
127 a tool is missing.

Subcommands:
  crates   --rust DIR                         print `-p <crate>` for every crate named by a `bin:` source that exists
  statisch --rust DIR                         print `-p <crate> --bin <program>` for every program that is built static and exists
  stufen   --rust DIR --bindir DIR [--bindir-statisch DIR] --stufe DIR --ausgabe DIR --version A.B.C [--geruest]
  version  --version A.B.C [--geruest]       print the version of the file name: A.B.C, with --geruest A.B.C~geruest
           stage the tree, check the lists, the profiles, the bill of materials and the manifest, write the control file
"""
import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys

PLATTFORM = "ubuntu-26.04-x64"
FEST = ("python3", "bubblewrap", "apparmor", "ca-certificates", "systemd", "libssl3t64", "debconf")
# `debconf` is no part of the six dependencies of the contract (referenz.STEUER_ABHAENGIGKEITEN): the package asks one question (templates, config) and its
# scripts source the confmodule of debconf. The build adds it to the control file; test_bau.py checks that.
GERUEST_ENDUNG = "~geruest"
BESCHREIBUNG_GERUEST = (
    " Skeleton build (Geruest) of the werkbank trial package. It holds only the programs that exist so far and is\n"
    " never for the company laptop. The check command werkbank-pruefen reports it red on purpose.\n"
)
BESCHREIBUNG_VOLL = " Werkbank trial package: programs, units and the AppArmor profile of the fixed enclosure.\n"


def deb_version(version, geruest):
    """The version of the control file and of every file name: a skeleton build carries `~geruest` visibly (contract, build command)."""
    return version + (GERUEST_ENDUNG if geruest else "")


def fehler(code, grund, zeilen=(), rc=65):
    sys.stderr.write(f"{code}: {grund}\n")
    for z in list(zeilen)[:20]:
        sys.stderr.write(f"  {z}\n")
    sys.exit(rc)


def referenz_laden(rust):
    sys.path.insert(0, os.path.join(rust, "spec", "paket"))
    import referenz  # noqa: E402
    return referenz


def lesen(pfad):
    with open(pfad, encoding="utf-8") as f:
        return f.read()


def bindir_von(ziel, bindir, bindir_statisch, ref):
    """The folder a program is taken from: the programs of `referenz.STATISCH` come from the static build."""
    return bindir_statisch if bindir_statisch and ziel in ref.STATISCH else bindir


def elf_befund(pfad):
    """ELF64 little endian: `{"interp": bool, "needed": [offset, ...]}`; None if the file is no such ELF file.
    `interp` is a PT_INTERP header, `needed` lists every DT_NEEDED entry of the dynamic section (the values are string offsets)."""
    with open(pfad, "rb") as f:
        roh = f.read()
    if roh[:4] != b"\x7fELF" or len(roh) < 64 or roh[4] != 2 or roh[5] != 1:
        return None
    phoff, = struct.unpack_from("<Q", roh, 32)
    phentsize, phnum = struct.unpack_from("<HH", roh, 54)
    interp, needed = False, []
    for i in range(phnum):
        o = phoff + i * phentsize
        if o + 56 > len(roh):
            return None
        typ, _, offset, _, _, filesz = struct.unpack_from("<IIQQQQ", roh, o)
        if typ == 3:
            interp = True
        elif typ == 2:
            for k in range(offset, min(offset + filesz, len(roh) - 15), 16):
                tag, wert = struct.unpack_from("<qQ", roh, k)
                if tag == 0:
                    break
                if tag == 1:
                    needed.append(wert)
    return {"interp": interp, "needed": needed}


def statisch_pruefen(ref, stufe, baum):
    """Contract step 4 (static build): every staged program of `referenz.STATISCH` must be a static ELF file."""
    programme = []
    for rel in baum:
        if rel in ref.STATISCH:
            befund = elf_befund(os.path.join(stufe, rel))
            # A file that is no ELF file is not static either: it counts as having an interpreter.
            programme.append({"ziel": rel, "interp": befund is None or befund["interp"], "needed": [] if befund is None else befund["needed"]})
    code = ref.statisch_code({"programme": programme})
    if code:
        fehler(f"paket.{code}", "Programm ist nicht statisch gelinkt", [p["ziel"] for p in programme if p["interp"] or p["needed"]])


def kern_einheit_pruefen(ref, stufe, baum):
    """Contract step 4 (core unit): the staged unit of the core service carries no NoNewPrivileges and no option that implies it
    (`paket.kern_einheit_nonewprivs`): the profile change of the starter to bwrap would be refused and no session would start."""
    for rel in baum:
        if rel == ref.KERN_EINHEIT:
            code = ref.kern_einheit_code({"text": lesen(os.path.join(stufe, rel))})
            if code:
                fehler(f"paket.{code}", "Kern-Einheit setzt NoNewPrivileges oder eine Option, die es nach sich zieht", [rel])


def modi_setzen(stufe, arten):
    """Contract stage 9: set the mode of every entry of the staged tree, independent of the umask of the builder: folders 0755,
    programs and services 0755, other files 0644 (the control file, the templates and the manifest 0644, the maintainer scripts and `config` 0755)."""
    for wurzel, ordner, dateien in os.walk(stufe):
        os.chmod(wurzel, 0o755)
        for name in ordner:
            os.chmod(os.path.join(wurzel, name), 0o755)
        for name in dateien:
            pfad = os.path.join(wurzel, name)
            rel = os.path.relpath(pfad, stufe)
            programm = arten.get(rel) in ("programm", "dienst") or rel in ("DEBIAN/postinst", "DEBIAN/prerm", "DEBIAN/postrm", "DEBIAN/config")
            os.chmod(pfad, 0o755 if programm else 0o644)


def modi_pruefen(ref, stufe):
    """Contract stage 9: no entry of the staged tree is writable for the group or the world (`paket.modus_schreibbar`)."""
    modi = {"./": format(os.lstat(stufe).st_mode & 0o7777, "04o")}
    for wurzel, ordner, dateien in os.walk(stufe):
        for name in ordner + dateien:
            pfad = os.path.join(wurzel, name)
            rel = os.path.relpath(pfad, stufe)
            modi[rel + "/" if os.path.isdir(pfad) and not os.path.islink(pfad) else rel] = format(os.lstat(pfad).st_mode & 0o7777, "04o")
    if ref.modus_code(modi):
        fehler("paket.modus_schreibbar", "Eintrag ist fuer Gruppe oder Welt schreibbar",
               [f"{pfad} {modus}" for pfad, modus in sorted(modi.items()) if int(modus, 8) & 0o022])


def quellen_vorhanden(rust, positiv, bindir, bindir_statisch=None, ref=None):
    """The sources of the positive list that exist now, as the strings of the list."""
    da = set()
    for _, ziel, quelle in positiv:
        if quelle.startswith("bin:"):
            ordner = bindir_von(ziel, bindir, bindir_statisch, ref) if ref else bindir
            if os.path.isfile(os.path.join(ordner, quelle.split(":")[2])):
                da.add(quelle)
        elif quelle.startswith("datei:"):
            if os.path.isfile(os.path.join(rust, quelle[len("datei:"):])):
                da.add(quelle)
        else:
            da.add(quelle)
    return da


def befehl_statisch(a):
    ref = referenz_laden(a.rust)
    positiv = ref.liste_lesen(lesen(os.path.join(a.rust, "paket", PLATTFORM, "lieferumfang.txt")), PLATTFORM)
    teile = []
    for _, ziel, q in positiv:
        if ziel in ref.STATISCH and q.startswith("bin:") and os.path.isfile(os.path.join(a.rust, "crates", q.split(":")[1], "Cargo.toml")):
            teile.append(f"-p {q.split(':')[1]} --bin {q.split(':')[2]}")
    print(" ".join(teile))
    return 0


def befehl_crates(a):
    ref = referenz_laden(a.rust)
    positiv = ref.liste_lesen(lesen(os.path.join(a.rust, "paket", PLATTFORM, "lieferumfang.txt")), PLATTFORM)
    namen = sorted({q.split(":")[1] for _, _, q in positiv if q.startswith("bin:")
                    and os.path.isfile(os.path.join(a.rust, "crates", q.split(":")[1], "Cargo.toml"))})
    print(" ".join(f"-p {n}" for n in namen))
    return 0


def lizenz_von(komponente):
    for eintrag in komponente.get("licenses", []):
        if "expression" in eintrag:
            return eintrag["expression"]
        lic = eintrag.get("license", {})
        if "id" in lic:
            return lic["id"]
    return None


def stueckliste(rust, crates):
    """The registry libraries of the SBOMs of the shipped crates: list of (name, version, licence, origin)."""
    sammlung = {}
    for crate in crates:
        pfad = os.path.join(rust, "sbom", f"{crate}.cdx.json")
        if not os.path.isfile(pfad):
            fehler("paket.stueckliste_fehlt", f"SBOM of crate {crate} is missing; run ./gates.sh licenses first", [pfad], 70)
        for k in json.loads(lesen(pfad)).get("components", []):
            if not k.get("bom-ref", "").startswith("registry+https://github.com/rust-lang/crates.io-index#"):
                continue
            lizenz = lizenz_von(k)
            if not lizenz:
                fehler("paket.stueckliste_leer_feld", "a library has no licence", [f"{k['name']} {k['version']}"])
            sammlung[(k["name"], k["version"])] = (lizenz, "crates.io")
    return [(n, v, l, h) for (n, v), (l, h) in sorted(sammlung.items(), key=lambda kv: (kv[0][0].encode(), kv[0][1].encode()))]


def sbom_schreiben(zeilen, version):
    komponenten = [{
        "type": "library", "name": n, "version": v, "licenses": [{"expression": l}],
        "purl": f"pkg:cargo/{n}@{v}",
    } for n, v, l, _ in zeilen]
    doc = {
        "bomFormat": "CycloneDX", "specVersion": "1.6", "version": 1,
        "metadata": {"component": {"type": "application", "name": "werkbank", "version": version}},
        "components": komponenten,
    }
    return (json.dumps(doc, indent=1) + "\n").encode()


def sha256_datei(pfad):
    h = hashlib.sha256()
    with open(pfad, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def elf_dateien(stufe, dateien):
    """The staged ELF files that link something dynamically; a static file has no dependency for `dpkg-shlibdeps` to name."""
    aus = []
    for rel in dateien:
        with open(os.path.join(stufe, rel), "rb") as f:
            if f.read(4) == b"\x7fELF":
                befund = elf_befund(os.path.join(stufe, rel))
                if befund is None or befund["interp"] or befund["needed"]:
                    aus.append(rel)
    return aus


def shlibs(stufe, elfs, ausgabe):
    if not elfs:
        return ""
    if shutil.which("dpkg-shlibdeps") is None:
        fehler("werkzeug", "dpkg-shlibdeps is missing (package dpkg-dev)", rc=127)
    arbeit = os.path.join(ausgabe, "shlibdeps")
    shutil.rmtree(arbeit, ignore_errors=True)
    os.makedirs(os.path.join(arbeit, "debian"))
    with open(os.path.join(arbeit, "debian", "control"), "w", encoding="ascii") as f:
        f.write("Source: werkbank\n\nPackage: werkbank\nArchitecture: amd64\n")
    r = subprocess.run(["dpkg-shlibdeps", "-O", "--ignore-missing-info"] + [os.path.join(stufe, e) for e in elfs],
                       cwd=arbeit, capture_output=True, text=True, check=False)
    shutil.rmtree(arbeit, ignore_errors=True)
    if r.returncode != 0:
        fehler("bau", "dpkg-shlibdeps failed", r.stderr.strip().split("\n")[-3:], 70)
    for zeile in r.stdout.split("\n"):
        if zeile.startswith("shlibs:Depends="):
            return zeile[len("shlibs:Depends="):].strip()
    return ""


def befehl_stufen(a):
    ref = referenz_laden(a.rust)
    ordner = os.path.join(a.rust, "paket", PLATTFORM)
    liste_text = lesen(os.path.join(ordner, "lieferumfang.txt"))
    ausschluss_text = lesen(os.path.join(ordner, "ausschluss.txt"))
    try:
        positiv = ref.liste_lesen(liste_text, PLATTFORM)
    except ValueError as e:
        fehler("paket.liste_form", "Positivliste hat falsche Form", [str(e)])
    try:
        ref.ausschluss_lesen(ausschluss_text)
    except ValueError as e:
        fehler("paket.ausschluss_form", "Ausschlussliste hat falsche Form", [str(e)])
    da = quellen_vorhanden(a.rust, positiv, a.bindir, a.bindir_statisch, ref)
    fehlend = [(art, ziel, q) for art, ziel, q in positiv if q not in da]
    if fehlend and not a.geruest:
        fehler("paket.quelle_fehlt", "Quelle fehlt", [f"{ziel} <- {q}" for _, ziel, q in fehlend])
    # A unit of a program that is not in the tree is left out too: it would start nothing.
    einheiten_weg = []
    for art, ziel, q in positiv:
        if q in da and ziel.startswith("usr/lib/systemd/user/") and a.geruest:
            programm = [z for z in lesen(os.path.join(a.rust, q[len("datei:"):])).split("\n") if z.startswith("ExecStart=")]
            if programm and programm[0][len("ExecStart="):].split()[0].lstrip("/") in [z for _, z, qq in fehlend]:
                einheiten_weg.append((art, ziel, q))
    fehlend_alle = fehlend + einheiten_weg
    behalten = [(art, ziel, q) for art, ziel, q in positiv if (art, ziel, q) not in fehlend_alle]

    shutil.rmtree(a.stufe, ignore_errors=True)
    os.makedirs(a.stufe)
    os.makedirs(a.ausgabe, exist_ok=True)
    crates = sorted({q.split(":")[1] for _, _, q in behalten if q.startswith("bin:")})
    zeilen = stueckliste(a.rust, crates)
    if not zeilen:
        fehler("paket.stueckliste_form", "the bill of materials is empty")
    deb_ver = deb_version(a.version, a.geruest)
    sbom = sbom_schreiben(zeilen, a.version)
    tsv = "".join(f"{n}\t{v}\t{l}\t{h}\n" for n, v, l, h in zeilen)
    code = ref.stueckliste_code({"text": tsv, "im_paket": [{"name": n, "version": v} for n, v, _, _ in zeilen]})
    if code:
        fehler(f"paket.{code}", "bill of materials violates the contract", [tsv.split("\n")[0]])
    for art, ziel, q in behalten:
        ziel_pfad = os.path.join(a.stufe, ziel)
        os.makedirs(os.path.dirname(ziel_pfad), exist_ok=True)
        if q == "erzeugt:sbom":
            with open(ziel_pfad, "wb") as f:
                f.write(sbom)
        elif q.startswith("bin:"):
            shutil.copyfile(os.path.join(bindir_von(ziel, a.bindir, a.bindir_statisch, ref), q.split(":")[2]), ziel_pfad)
        else:
            shutil.copyfile(os.path.join(a.rust, q[len("datei:"):]), ziel_pfad)
        os.chmod(ziel_pfad, 0o755 if art in ("programm", "dienst") else 0o644)
        if q.startswith("bin:") and shutil.which("strip"):
            subprocess.run(["strip", "--strip-all", ziel_pfad], check=False, capture_output=True)
    baum = sorted(os.path.relpath(os.path.join(w, f), a.stufe) for w, _, fs in os.walk(a.stufe) for f in fs)
    code = ref.liste_code({
        "liste": "".join(f"{art} {ziel} {q}\n" for art, ziel, q in behalten), "ausschluss": ausschluss_text,
        "quellen": sorted(da), "baum": baum, "plattform": PLATTFORM,
    })
    if code:
        fehler(f"paket.{code}", "Lieferumfang verletzt den Vertrag", baum)
    for rel in baum:
        if rel.startswith("etc/apparmor.d/"):
            code = ref.profil_code({"text": lesen(os.path.join(a.stufe, rel))})
            if code:
                fehler(f"paket.{code}", "Profil verletzt den Vertrag", [rel])

    statisch_pruefen(ref, a.stufe, baum)
    kern_einheit_pruefen(ref, a.stufe, baum)

    arten = {ziel: art for art, ziel, _ in behalten}
    manifest = {
        "fassung": 1, "paket": "werkbank", "version": a.version, "plattform": PLATTFORM,
        "bau": "test" if a.geruest else "auslieferung", "merkmale": ["geruest"] if a.geruest else [],
        "dateien": [{"pfad": rel, "groesse": os.path.getsize(os.path.join(a.stufe, rel)),
                     "sha256": sha256_datei(os.path.join(a.stufe, rel)), "art": arten[rel]} for rel in baum],
        "bibliotheken": len(zeilen),
    }
    roh = (json.dumps(manifest, indent=1) + "\n").encode()
    code = ref.manifest_code(roh)
    if code and not (a.geruest and code == "pflichtdatei_fehlt"):
        fehler(f"paket.{code}", "Manifest verletzt den Vertrag")
    manifest_pfad = os.path.join(a.stufe, "usr", "share", "werkbank", "manifest.json")
    os.makedirs(os.path.dirname(manifest_pfad), exist_ok=True)
    with open(manifest_pfad, "wb") as f:
        f.write(roh)
    os.chmod(manifest_pfad, 0o644)

    # DEBIAN: control, maintainer scripts
    debian = os.path.join(a.stufe, "DEBIAN")
    os.makedirs(debian)
    gemessen = shlibs(a.stufe, elf_dateien(a.stufe, baum), a.ausgabe)
    # A fixed dependency that dpkg-shlibdeps names itself (with its version) is not written twice.
    fest = [f for f in FEST if f not in {d.split("(")[0].split("|")[0].strip() for d in gemessen.split(",")}]
    abhaengig = ", ".join(fest + ([gemessen] if gemessen else []))
    groesse = (sum(m["groesse"] for m in manifest["dateien"]) + len(roh) + 1023) // 1024
    control = (
        "Package: werkbank\n"
        f"Version: {deb_ver}\n"
        "Architecture: amd64\n"
        "Maintainer: Skryx-L-A <Skryx-L-A@users.noreply.github.com>\n"
        "Section: utils\n"
        "Priority: optional\n"
        f"Installed-Size: {groesse}\n"
        f"Depends: {abhaengig}\n"
        "Description: werkbank trial package for Ubuntu 26.04\n"
        + (BESCHREIBUNG_GERUEST if a.geruest else BESCHREIBUNG_VOLL)
    )
    code = ref.steuer_code({"text": control, "manifest_version": a.version, "geruest": a.geruest})
    if code:
        fehler(f"paket.{code}", "Steuerdatei verletzt den Vertrag", control.split("\n")[:3])
    with open(os.path.join(debian, "control"), "w", encoding="ascii") as f:
        f.write(control)
    for skript in ("postinst", "prerm", "postrm", "config"):
        shutil.copyfile(os.path.join(ordner, skript), os.path.join(debian, skript))
        os.chmod(os.path.join(debian, skript), 0o755)
    shutil.copyfile(os.path.join(ordner, "templates"), os.path.join(debian, "templates"))
    os.chmod(os.path.join(debian, "templates"), 0o644)

    # Stage 9: the modes of the tree are fixed here, whatever the umask of the builder was; then they are checked.
    modi_setzen(a.stufe, arten)
    modi_pruefen(ref, a.stufe)

    shutil.copyfile(manifest_pfad, os.path.join(a.ausgabe, f"werkbank_{deb_ver}_amd64.manifest.json"))
    with open(os.path.join(a.ausgabe, f"werkbank_{deb_ver}_amd64.stueckliste.tsv"), "w", encoding="ascii") as f:
        f.write(tsv)
    bericht = {
        "version": a.version, "deb_version": deb_ver, "geruest": a.geruest, "dateien": len(baum), "bibliotheken": len(zeilen),
        "enthalten": [ziel for _, ziel, _ in behalten],
        "fehlend": [{"ziel": ziel, "quelle": q, "grund": "source not in the tree"} for _, ziel, q in fehlend]
                   + [{"ziel": ziel, "quelle": q, "grund": "program of the unit is missing"} for _, ziel, q in einheiten_weg],
        "manifest_code": ref.manifest_code(roh) or "gueltig",
    }
    with open(os.path.join(a.ausgabe, f"werkbank_{deb_ver}_amd64.bericht.json"), "w", encoding="ascii") as f:
        json.dump(bericht, f, indent=1)
        f.write("\n")
    print(json.dumps(bericht))
    return 0


def main():
    p = argparse.ArgumentParser()
    s = p.add_subparsers(dest="cmd", required=True)
    c = s.add_parser("crates")
    c.add_argument("--rust", required=True)
    sc = s.add_parser("statisch")
    sc.add_argument("--rust", required=True)
    v = s.add_parser("version")
    v.add_argument("--version", required=True)
    v.add_argument("--geruest", action="store_true")
    st = s.add_parser("stufen")
    st.add_argument("--rust", required=True)
    st.add_argument("--bindir", required=True)
    st.add_argument("--bindir-statisch", dest="bindir_statisch", default=None)
    st.add_argument("--stufe", required=True)
    st.add_argument("--ausgabe", required=True)
    st.add_argument("--version", required=True)
    st.add_argument("--geruest", action="store_true")
    a = p.parse_args()
    if a.cmd == "version":
        print(deb_version(a.version, a.geruest))
        return 0
    if a.cmd == "statisch":
        return befehl_statisch(a)
    return befehl_crates(a) if a.cmd == "crates" else befehl_stufen(a)


if __name__ == "__main__":
    sys.exit(main())

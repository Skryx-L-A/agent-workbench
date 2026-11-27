//! Tests of the profile builder: the exact argument list, every refusal of the form, the deny lists and the overlaps,
//! and the check of the finished text (with a list that lacks one protection at a time).

use std::path::PathBuf;

use super::*;
use crate::beschreibung::{System, Umgebung, Zustand};

fn muster() -> Beschreibung {
    Beschreibung {
        harness: "claude".to_owned(),
        arbeitsordner: PathBuf::from("/srv/arbeit/p"),
        programmordner: vec![
            PathBuf::from("/home/u/.local/bin"),
            PathBuf::from("/home/u/.local/share/claude"),
        ],
        zustand: Zustand {
            host: PathBuf::from("/var/tmp/z/s-1"),
            name: ".claude".to_owned(),
            fortsetzen: false,
        },
        heim: PathBuf::from("/home/u"),
        such_socket: PathBuf::from("/var/tmp/k/s-1/such.sock"),
        modell_socket: Some(PathBuf::from("/var/tmp/k/s-1/modell.sock")),
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: None,
        paket: None,
        umgebung: Umgebung {
            home: "/home/u".to_owned(),
            path: "/home/u/.local/bin:/usr/bin".to_owned(),
            term: "xterm-256color".to_owned(),
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: vec![
            PathBuf::from("/srv/vault"),
            PathBuf::from("/var/lib/wb/protokoll"),
            PathBuf::from("/var/lib/wb/start"),
        ],
        argv: vec![
            "/home/u/.local/bin/claude".to_owned(),
            "--model".to_owned(),
            "m".to_owned(),
        ],
        system: System::usr_merge(1000),
    }
}

fn bauen(b: &Beschreibung) -> Result<Profil, Fehler> {
    Profil::bauen(b)
}

/// The whole list as words (no argument holds a space).
fn erwartet_kopf() -> Vec<String> {
    let text = "\
        --unshare-user --disable-userns --unshare-pid --unshare-net --unshare-ipc --unshare-uts \
        --unshare-cgroup --die-with-parent --cap-drop ALL --clearenv --hostname wb-einschluss --ro-bind /usr \
        /usr --symlink usr/bin /bin --symlink usr/bin /sbin --symlink usr/lib /lib --symlink usr/lib /lib64 \
        --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group --ro-bind /etc/nsswitch.conf \
        /etc/nsswitch.conf --ro-bind /etc/ld.so.cache /etc/ld.so.cache --ro-bind /etc/localtime \
        /etc/localtime --proc /proc --dev /dev --size 268435456 --perms 0700 --tmpfs /tmp --size 16777216 \
        --perms 0700 --tmpfs /run --perms 0700 --dir /run/user --perms 0700 --dir /run/user/1000 --size \
        16777216 --perms 0700 --tmpfs /home/u --ro-bind /home/u/.local/bin /home/u/.local/bin --ro-bind \
        /home/u/.local/share/claude /home/u/.local/share/claude --bind /var/tmp/z/s-1 /home/u/.claude --bind \
        /srv/arbeit/p /srv/arbeit/p --ro-bind /var/tmp/k/s-1/such.sock /run/brain/such.sock --ro-bind \
        /var/tmp/k/s-1/modell.sock /run/brain/modell.sock --setenv HOME /home/u --setenv PATH \
        /home/u/.local/bin:/usr/bin --setenv TERM xterm-256color --setenv LANG C.UTF-8 --setenv TMPDIR /tmp \
        --chdir /srv/arbeit/p -- /home/u/.local/bin/claude --model m
        ";
    text.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn the_argument_list_is_exactly_this() {
    let profil = bauen(&muster()).expect("the sample is valid");
    assert_eq!(profil.argumente(), erwartet_kopf());
    assert_eq!(profil.bwrap(), Path::new("/usr/bin/bwrap"));
}

#[test]
fn without_the_model_socket_the_list_has_no_model_mount() {
    let mut b = muster();
    b.modell_socket = None;
    let args = bauen(&b).expect("valid").argumente();
    assert!(args.iter().any(|a| a == GAST_SUCH_SOCKET));
    assert!(!args.iter().any(|a| a == GAST_MODELL_SOCKET));
}

#[test]
fn the_profile_never_names_the_root_or_a_protected_folder() {
    let args = bauen(&muster()).expect("valid").argumente();
    for (i, a) in args.iter().enumerate() {
        if a == "--ro-bind" || a == "--bind" {
            assert_ne!(args[i + 1], "/", "mount of the root");
            assert_ne!(args[i + 2], "/", "mount of the root");
        }
    }
    for schutz in muster().geschuetzt {
        let s = schutz.to_string_lossy();
        assert!(!args.iter().any(|a| a.contains(s.as_ref())), "{s}");
    }
    for verboten in VERBOTEN {
        assert!(!args.iter().any(|a| a == verboten), "{verboten}");
    }
}

#[test]
fn the_home_is_empty_before_any_mount_below_it() {
    let args = bauen(&muster()).expect("valid").argumente();
    let heim = args
        .windows(2)
        .position(|w| w[0] == "--tmpfs" && w[1] == "/home/u")
        .expect("home tmpfs");
    for (i, a) in args.iter().enumerate() {
        if (a == "--bind" || a == "--ro-bind") && args[i + 2].starts_with("/home/u/") {
            assert!(i > heim, "{a} {} before the home tmpfs", args[i + 2]);
        }
    }
}

#[test]
fn the_command_has_an_empty_environment_and_the_fixed_path() {
    let profil = bauen(&muster()).expect("valid");
    let befehl = profil.befehl();
    assert_eq!(befehl.get_program(), "/usr/bin/bwrap");
    assert_eq!(
        befehl.get_envs().count(),
        0,
        "env_clear leaves no variable set"
    );
    let args: Vec<String> = befehl
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, profil.argumente_gebunden());
    let anders = profil.clone().mit_bwrap(PathBuf::from("/opt/x/bwrap"));
    assert_eq!(anders.bwrap(), Path::new("/opt/x/bwrap"));
    assert_eq!(anders.befehl().get_program(), "/opt/x/bwrap");
    assert_eq!(profil.beschreibung(), &muster());
}

#[test]
fn argv_can_be_replaced_for_the_test_run() {
    let profil = bauen(&muster()).expect("valid");
    let probe = profil.argumente_mit(&["/usr/bin/true".to_owned()]);
    assert_eq!(&probe[probe.len() - 2..], ["--", "/usr/bin/true"]);
    assert_eq!(probe[..probe.len() - 2], profil.kopf[..]);
}

fn abgelehnt(aendern: impl FnOnce(&mut Beschreibung)) -> Fehler {
    let mut b = muster();
    aendern(&mut b);
    bauen(&b).expect_err("must be refused")
}

#[test]
fn a_malformed_description_is_refused_with_the_field() {
    let ungueltig = |name: &'static str| Fehler::BeschreibungUngueltig(name);
    assert_eq!(
        abgelehnt(|b| b.harness = "Claude".into()),
        ungueltig("harness")
    );
    assert_eq!(
        abgelehnt(|b| b.harness = String::new()),
        ungueltig("harness")
    );
    assert_eq!(
        abgelehnt(|b| b.harness = "a".repeat(65)),
        ungueltig("harness")
    );
    assert_eq!(abgelehnt(|b| b.harness = "-x".into()), ungueltig("harness"));
    assert_eq!(abgelehnt(|b| b.argv.clear()), ungueltig("argv"));
    assert_eq!(abgelehnt(|b| b.argv[0] = String::new()), ungueltig("argv"));
    assert_eq!(abgelehnt(|b| b.argv.push("a\0b".into())), ungueltig("argv"));
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "relativ".into()),
        ungueltig("arbeitsordner")
    );
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/srv/../x".into()),
        ungueltig("arbeitsordner")
    );
    assert_eq!(
        abgelehnt(|b| b.zustand.host = "/a/".into()),
        ungueltig("zustand.host")
    );
    assert_eq!(abgelehnt(|b| b.heim = "/home//u".into()), ungueltig("heim"));
    assert_eq!(
        abgelehnt(|b| b.such_socket = "such.sock".into()),
        ungueltig("such_socket")
    );
    assert_eq!(
        abgelehnt(|b| b.modell_socket = Some("m".into())),
        ungueltig("modell_socket")
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("x".into())),
        ungueltig("programmordner")
    );
    assert_eq!(
        abgelehnt(|b| b.geschuetzt.push("/x/".into())),
        ungueltig("geschuetzt")
    );
    for name in ["", ".", "..", "a/b", "a\nb"] {
        assert_eq!(
            abgelehnt(|b| b.zustand.name = name.into()),
            ungueltig("zustand.name"),
            "{name:?}"
        );
    }
    assert_eq!(
        abgelehnt(|b| b.zustand.name = "a".repeat(256)),
        ungueltig("zustand.name")
    );
    assert_eq!(
        abgelehnt(|b| b.umgebung.home = "/home/v".into()),
        ungueltig("umgebung.home")
    );
    assert_eq!(
        abgelehnt(|b| b.umgebung.path = String::new()),
        ungueltig("umgebung")
    );
    assert_eq!(
        abgelehnt(|b| b.umgebung.term = String::new()),
        ungueltig("umgebung")
    );
    assert_eq!(
        abgelehnt(|b| b.umgebung.lang = "a\nb".into()),
        ungueltig("umgebung")
    );
    assert_eq!(
        abgelehnt(|b| b.umgebung.tmpdir = "tmp".into()),
        ungueltig("umgebung.tmpdir")
    );
    assert_eq!(
        abgelehnt(|b| b.system.symlinks.push(("a".into(), "rel".into()))),
        ungueltig("system")
    );
    assert_eq!(
        abgelehnt(|b| b.system.ro_binds.push(("a\0".into(), "/b".into()))),
        ungueltig("system")
    );
}

#[test]
fn a_valid_name_of_the_longest_allowed_shape_is_taken() {
    let mut b = muster();
    b.zustand.name = ".zustand".into();
    b.harness = format!("a{}", "b".repeat(63));
    assert!(bauen(&b).is_ok());
    b.harness = "a.b_c-1".into();
    assert!(bauen(&b).is_ok());
    b.zustand.name = "a".repeat(255);
    assert!(bauen(&b).is_ok());
}

#[test]
fn a_working_folder_that_is_a_system_path_or_the_home_is_refused() {
    let verboten = |was| Fehler::Verboten(was);
    for arbeit in [
        "/home/u",
        "/home",
        "/usr/share/x",
        "/etc/x",
        "/tmp",
        "/proc/x",
        "/opt/x",
    ] {
        let f = abgelehnt(|b| b.arbeitsordner = arbeit.into());
        assert_eq!(
            f,
            verboten("working folder is a system path, the home or above it"),
            "{arbeit}"
        );
    }
}

#[test]
fn a_home_in_a_system_path_is_refused() {
    for heim in ["/tmp", "/usr/x", "/run/user/1", "/etc", "/proc/9", "/opt/h"] {
        let f = abgelehnt(|b| {
            b.heim = heim.into();
            b.umgebung.home = heim.into();
            b.programmordner.clear();
        });
        assert_eq!(f, Fehler::Verboten("home is a system path"), "{heim}");
    }
}

#[test]
fn a_secret_segment_in_any_mount_is_refused() {
    let geheim = Fehler::Verboten("path with a secret segment");
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/srv/.ssh/p".into()),
        geheim
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("/srv/.config/bin".into())),
        geheim
    );
    assert_eq!(
        abgelehnt(|b| b.zustand.host = "/var/tmp/.aws/z".into()),
        geheim
    );
    assert_eq!(
        abgelehnt(|b| b.such_socket = "/var/tmp/.env/s.sock".into()),
        geheim
    );
    assert_eq!(
        abgelehnt(|b| b.modell_socket = Some("/var/tmp/90-secrets/m.sock".into())),
        geheim
    );
    // the state folder place inside: a name that is a secret segment
    assert_eq!(abgelehnt(|b| b.zustand.name = ".ssh".into()), geheim);
}

#[test]
fn a_login_store_of_the_account_in_any_mount_is_refused() {
    let anmeldung = Fehler::Verboten("path in or over a login store of the account");
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/home/u/.mozilla/x".into()),
        anmeldung
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("/home/u/.local/share".into())),
        anmeldung
    );
    assert_eq!(
        abgelehnt(|b| b
            .programmordner
            .push("/home/u/.local/share/keyrings".into())),
        anmeldung
    );
    assert_eq!(
        abgelehnt(|b| b.zustand.host = "/home/u/.password-store".into()),
        anmeldung
    );
    assert_eq!(
        abgelehnt(|b| b.such_socket = "/home/u/.gnome2/keyrings/s.sock".into()),
        anmeldung
    );
    // the state folder of another harness covers its login file
    let f = abgelehnt(|b| {
        b.harness = "pi".into();
        b.zustand.name = ".claude".into();
    });
    assert_eq!(f, anmeldung);
    // the own state folder is fine
    let mut b = muster();
    b.harness = "pi".into();
    b.zustand.name = ".pi".into();
    assert!(bauen(&b).is_ok());
}

#[test]
fn a_mount_that_meets_a_protected_folder_is_refused() {
    let schutz = Fehler::Ueberlappt("mount overlaps a protected folder");
    assert_eq!(abgelehnt(|b| b.arbeitsordner = "/srv/vault".into()), schutz);
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/srv/vault/inner".into()),
        schutz
    );
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/var/lib/wb".into()),
        schutz
    );
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/var/lib/wb/start/x".into()),
        schutz
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("/var/lib/wb".into())),
        schutz
    );
    // the state folder may lie below a protected folder (the conversations below the state folder of the
    // installation), but it is never one and never holds one
    let mut b = muster();
    b.zustand.host = "/var/lib/wb/protokoll/z".into();
    assert!(bauen(&b).is_ok());
    assert_eq!(
        abgelehnt(|b| b.zustand.host = "/var/lib/wb/protokoll".into()),
        schutz
    );
    assert_eq!(abgelehnt(|b| b.zustand.host = "/var/lib/wb".into()), schutz);
    assert_eq!(abgelehnt(|b| b.zustand.host = "/srv".into()), schutz);
    // a sibling that only shares a text prefix is no overlap
    let mut b = muster();
    b.arbeitsordner = "/srv/vault2/p".into();
    assert!(bauen(&b).is_ok());
}

#[test]
fn the_state_folder_and_the_working_folder_stay_apart() {
    let zustand = Fehler::Ueberlappt("state folder overlaps another mount");
    assert_eq!(
        abgelehnt(|b| b.zustand.host = "/srv/arbeit/p/z".into()),
        zustand
    );
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/var/tmp/z/s-1/a".into()),
        zustand
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("/var/tmp/z".into())),
        zustand
    );
    let platz = Fehler::Ueberlappt("state folder place overlaps another mount");
    assert_eq!(
        abgelehnt(|b| b.arbeitsordner = "/home/u/.claude/p".into()),
        platz
    );
    assert_eq!(
        abgelehnt(|b| b.programmordner.push("/home/u/.claude".into())),
        platz
    );
    let mut b = muster();
    b.harness = "pi".into();
    b.zustand.name = ".pi".into();
    b.arbeitsordner = "/home/u/.pi/p".into();
    assert_eq!(bauen(&b).expect_err("refused"), platz);
}

#[test]
fn a_socket_inside_a_mounted_folder_is_refused() {
    let sockel = Fehler::Ueberlappt("socket lies inside a mounted folder");
    assert_eq!(
        abgelehnt(|b| b.such_socket = "/srv/arbeit/p/such.sock".into()),
        sockel
    );
    assert_eq!(
        abgelehnt(|b| b.modell_socket = Some("/home/u/.local/bin/m.sock".into())),
        sockel
    );
    assert_eq!(
        abgelehnt(|b| b.such_socket = "/var/tmp/z/s-1/such.sock".into()),
        sockel
    );
    assert_eq!(
        abgelehnt(|b| b.modell_socket = Some(b.such_socket.clone())),
        Fehler::Ueberlappt("both sockets are the same file")
    );
}

fn kopf(b: &Beschreibung) -> Vec<String> {
    kopf_bauen(b)
}

fn kopf_gut(k: &[String]) -> Result<(), Fehler> {
    kopf_pruefen(k, Path::new("/home/u"), "claude", 1000)
}

#[test]
fn the_check_of_the_text_takes_the_built_list() {
    assert_eq!(kopf_gut(&kopf(&muster())), Ok(()));
}

#[test]
fn the_check_of_the_text_names_each_missing_protection() {
    let gut = kopf(&muster());
    let ohne = |s: &str| -> Vec<String> { gut.iter().filter(|a| *a != s).cloned().collect() };
    for pflicht in PFLICHT {
        assert_eq!(
            kopf_gut(&ohne(pflicht)),
            Err(Fehler::Verboten("profile lacks a required switch")),
            "{pflicht}"
        );
    }
    let mut ohne_alle = gut.clone();
    let i = ohne_alle.iter().position(|a| a == "ALL").expect("ALL");
    ohne_alle[i] = "NET_ADMIN".into();
    assert_eq!(
        kopf_gut(&ohne_alle),
        Err(Fehler::Verboten("profile lacks a required switch"))
    );
}

#[test]
fn the_check_of_the_text_refuses_each_forbidden_switch() {
    let gut = kopf(&muster());
    for verboten in VERBOTEN {
        let mut k = gut.clone();
        k.insert(3, verboten.to_owned());
        assert_eq!(
            kopf_gut(&k),
            Err(Fehler::Verboten("profile carries a forbidden switch")),
            "{verboten}"
        );
    }
}

#[test]
fn the_check_of_the_text_wants_the_private_run_and_home() {
    let gut = kopf(&muster());
    let streiche = |paar: [&str; 2]| -> Vec<String> {
        let mut k = gut.clone();
        let i = k
            .windows(2)
            .position(|w| w[0] == paar[0] && w[1] == paar[1])
            .expect("pair");
        k.drain(i..i + 2);
        k
    };
    assert_eq!(
        kopf_gut(&streiche(["--tmpfs", "/home/u"])),
        Err(Fehler::Verboten("home is not a private tmpfs"))
    );
    let privat = Err(Fehler::Verboten(
        "private /run with an empty /run/user/<uid> missing",
    ));
    assert_eq!(kopf_gut(&streiche(["--tmpfs", "/run"])), privat);
    assert_eq!(kopf_gut(&streiche(["--dir", "/run/user/1000"])), privat);
    // the empty folder of another user does not count
    assert_eq!(
        kopf_pruefen(&gut, Path::new("/home/u"), "claude", 1001),
        privat
    );
}

#[test]
fn the_check_of_the_text_wants_the_environment_cleared_first() {
    let mut k = kopf(&muster());
    let clear = k.iter().position(|a| a == "--clearenv").expect("clearenv");
    k.remove(clear);
    k.push("--clearenv".into());
    assert_eq!(
        kopf_gut(&k),
        Err(Fehler::Verboten("environment set before it is cleared"))
    );
}

fn mit_einbindung(option: &str, quelle: &str, ziel: &str, an: usize) -> Vec<String> {
    let mut k = kopf(&muster());
    for (i, t) in [option, quelle, ziel].iter().enumerate() {
        k.insert(an + i, (*t).to_owned());
    }
    k
}

#[test]
fn the_check_of_the_text_refuses_a_mount_that_breaks_the_boundary() {
    let ende = kopf(&muster()).len() - 8;
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/", "/", ende)),
        Err(Fehler::Verboten("mount of the root"))
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/", "/x", ende)),
        Err(Fehler::Verboten("mount of the root"))
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--bind", "/x", "/", ende)),
        Err(Fehler::Verboten("mount of the root"))
    );
    let bruch = Err(Fehler::Verboten("mount breaks the private run or home"));
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/x", "/run/user", ende)),
        bruch
    );
    assert_eq!(
        kopf_gut(&mit_einbindung(
            "--ro-bind",
            "/x",
            "/run/user/1000/bus",
            ende
        )),
        bruch
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/x", "/home", ende)),
        bruch
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/x", "/home/u", ende)),
        bruch
    );
    // a place below the home before its tmpfs
    let davor = mit_einbindung("--ro-bind", "/x", "/home/u/y", 2);
    assert_eq!(
        kopf_gut(&davor),
        Err(Fehler::Verboten("mount in the home before its tmpfs"))
    );
    // a source or a place with a secret or a login store
    let geheim = Err(Fehler::Verboten("secret or login store in a mount"));
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/srv/.ssh", "/x", ende)),
        geheim
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/x", "/srv/.aws", ende)),
        geheim
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/home/u/.mozilla", "/x", ende)),
        geheim
    );
    assert_eq!(
        kopf_gut(&mit_einbindung("--ro-bind", "/x", "/home/u/.gnupg", ende)),
        geheim
    );
}

#[test]
fn a_mount_in_the_home_after_other_tmpfs_but_before_the_home_tmpfs_is_refused() {
    let gut = kopf(&muster());
    let heim = gut
        .windows(2)
        .position(|w| w[0] == "--tmpfs" && w[1] == "/home/u")
        .expect("home tmpfs");
    // directly in front of the options of the home tmpfs (`--size N --perms M --tmpfs`), after `--tmpfs /run`
    let k = mit_einbindung("--ro-bind", "/x", "/home/u/y", heim - 4);
    assert_eq!(
        kopf_gut(&k),
        Err(Fehler::Verboten("mount in the home before its tmpfs"))
    );
    // the same mount after the home tmpfs is fine
    let nach = mit_einbindung("--ro-bind", "/x", "/home/u/y", heim + 2);
    assert_eq!(kopf_gut(&nach), Ok(()));
}

#[test]
fn a_link_is_no_mount_of_a_host_path() {
    // `--symlink usr/bin /bin` has a link text as its first argument, not a host path
    let ende = kopf(&muster()).len() - 8;
    assert_eq!(
        kopf_gut(&mit_einbindung("--symlink", "usr/.ssh", "/bin2", ende)),
        Ok(())
    );
}

#[test]
fn the_form_of_a_name_is_the_form_of_contract_1() {
    for gut in ["a", "0", "a.b", "a_b", "a-b", "claude", "pi"] {
        assert!(kennung(gut), "{gut}");
    }
    for schlecht in ["", "A", "-a", ".a", "_a", "a b", "a/b", "ä"] {
        assert!(!kennung(schlecht), "{schlecht}");
    }
    assert!(kennung(&"a".repeat(64)));
    assert!(!kennung(&"a".repeat(65)));
}

// ---------------------------------------------------------------------------------------------------------------
// the cloud exit

use crate::beschreibung::{COPILOT_PLATZHALTER, Cloud, Verteilung};

fn cloud_muster(verteilung: Verteilung) -> Beschreibung {
    let mut b = muster();
    b.harness = "copilot".to_owned();
    b.modell_socket = None;
    ".copilot".clone_into(&mut b.zustand.name);
    b.cloud = Some(Cloud {
        weiterreicher: PathBuf::from("/var/tmp/k/s-1/relay.sock"),
        weiter_programm: PathBuf::from("/usr/lib/werkbank/einschluss-weiter"),
        bundle: PathBuf::from("/var/tmp/k/s-1/bundle.pem"),
        verteilung,
    });
    b
}

fn cloud_aendern(aendern: impl FnOnce(&mut Cloud)) -> Fehler {
    let mut b = cloud_muster(Verteilung::Ubuntu);
    aendern(b.cloud.as_mut().expect("cloud"));
    bauen(&b).expect_err("must be refused")
}

fn worte(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn the_placeholder_is_gho_and_36_zeros() {
    assert_eq!(COPILOT_PLATZHALTER.len(), 40);
    assert!(COPILOT_PLATZHALTER.starts_with("gho_"));
    assert!(COPILOT_PLATZHALTER[4..].chars().all(|c| c == '0'));
    assert_eq!(COPILOT_PLATZHALTER[4..].len(), 36);
}

#[test]
fn the_cloud_list_binds_the_relay_the_forwarder_and_the_bundle_and_sets_four_variables() {
    let args = bauen(&cloud_muster(Verteilung::Ubuntu))
        .expect("valid")
        .argumente();
    let mut text = erwartet_kopf()
        .join(" ")
        .replacen("/home/u/.claude", "/home/u/.copilot", 1);
    let mut ersetze = |alt: &str, neu: &str| {
        assert!(text.contains(alt), "{alt}");
        text = text.replacen(alt, neu, 1);
    };
    // the relay at the place of the model socket, then the forwarder and the bundle
    ersetze(
        "--ro-bind /var/tmp/k/s-1/modell.sock /run/brain/modell.sock",
        "--ro-bind /var/tmp/k/s-1/relay.sock /run/brain/modell.sock --ro-bind \
         /usr/lib/werkbank/einschluss-weiter /run/brain/weiter --ro-bind /var/tmp/k/s-1/bundle.pem \
         /etc/ssl/certs/ca-certificates.crt",
    );
    // four more variables, then the program below the forwarder
    let drei = format!(
        "--setenv COPILOT_GITHUB_TOKEN {COPILOT_PLATZHALTER} --setenv HTTPS_PROXY \
         http://127.0.0.1:8402 --setenv COPILOT_AUTO_UPDATE false --setenv \
         COPILOT_MASCOT_BLINK 0 --chdir /srv/arbeit/p -- \
         /run/brain/weiter 8402 /run/brain/modell.sock -- /home/u/.local/bin/claude"
    );
    ersetze("--chdir /srv/arbeit/p -- /home/u/.local/bin/claude", &drei);
    let soll = worte(&text);
    assert_eq!(args, soll);
}

#[test]
fn arch_binds_the_session_bundle_at_both_places_of_the_system_bundle() {
    let args = bauen(&cloud_muster(Verteilung::Arch))
        .expect("valid")
        .argumente();
    for ort in ["/etc/ssl/certs/ca-certificates.crt", "/etc/ssl/cert.pem"] {
        let stelle = args
            .windows(3)
            .any(|w| w[0] == "--ro-bind" && w[1] == "/var/tmp/k/s-1/bundle.pem" && w[2] == ort);
        assert!(stelle, "{ort} not bound");
    }
    let ubuntu = bauen(&cloud_muster(Verteilung::Ubuntu))
        .expect("valid")
        .argumente();
    assert!(!ubuntu.iter().any(|a| a == "/etc/ssl/cert.pem"));
}

#[test]
fn the_cloud_program_runs_below_the_forwarder_and_a_plain_one_does_not() {
    let profil = bauen(&cloud_muster(Verteilung::Ubuntu)).expect("valid");
    let args = profil.argumente_mit(&["/usr/bin/true".to_owned()]);
    let naht = args.iter().position(|a| a == "--").expect("--");
    assert_eq!(
        args[naht + 1..],
        worte("/run/brain/weiter 8402 /run/brain/modell.sock -- /usr/bin/true")
    );
    let schlicht = bauen(&muster()).expect("valid");
    let args = schlicht.argumente_mit(&["/usr/bin/true".to_owned()]);
    let naht = args.iter().position(|a| a == "--").expect("--");
    assert_eq!(args[naht + 1..], ["/usr/bin/true".to_owned()]);
}

#[test]
fn the_environment_of_a_plain_profile_has_the_five_variables_only() {
    let args = bauen(&muster()).expect("valid").argumente();
    let namen: Vec<&str> = args
        .windows(3)
        .filter(|w| w[0] == "--setenv")
        .map(|w| w[1].as_str())
        .collect();
    assert_eq!(namen, ["HOME", "PATH", "TERM", "LANG", "TMPDIR"]);
}

#[test]
fn a_cloud_exit_needs_the_cloud_harness_and_no_second_model_socket() {
    let ungueltig = |name: &'static str| Fehler::BeschreibungUngueltig(name);
    let mut b = cloud_muster(Verteilung::Ubuntu);
    b.harness = "claude".to_owned();
    assert_eq!(
        bauen(&b).expect_err("refused"),
        ungueltig("cloud (only for the cloud harness)")
    );
    let mut b = cloud_muster(Verteilung::Ubuntu);
    b.modell_socket = Some(PathBuf::from("/var/tmp/k/s-1/modell.sock"));
    assert_eq!(
        bauen(&b).expect_err("refused"),
        ungueltig("cloud and modell_socket together")
    );
    for aendern in [
        (|c: &mut Cloud| c.weiterreicher = PathBuf::from("relay.sock")) as fn(&mut Cloud),
        |c| c.weiter_programm = PathBuf::from("/usr/lib/../x"),
        |c| c.bundle = PathBuf::from("/x//y"),
    ] {
        assert_eq!(cloud_aendern(aendern), ungueltig("cloud"));
    }
}

#[test]
fn a_file_of_the_cloud_exit_in_a_folder_the_harness_can_write_is_refused() {
    let ueber = |was: &'static str| Fehler::Ueberlappt(was);
    let schreibbar = ueber("a file of the cloud exit lies in a folder the harness can write");
    assert_eq!(
        cloud_aendern(|c| c.bundle = PathBuf::from("/srv/arbeit/p/bundle.pem")),
        schreibbar
    );
    assert_eq!(
        cloud_aendern(|c| c.weiter_programm = PathBuf::from("/srv/arbeit/p/weiter")),
        schreibbar
    );
    assert_eq!(
        cloud_aendern(|c| c.bundle = PathBuf::from("/var/tmp/z/s-1/bundle.pem")),
        schreibbar
    );
    assert_eq!(
        cloud_aendern(|c| c.weiter_programm = PathBuf::from("/var/tmp/z/s-1/weiter")),
        schreibbar
    );
    // the same file twice is a fault of the caller
    assert_eq!(
        cloud_aendern(|c| c.bundle = c.weiter_programm.clone()),
        ueber("the program and the bundle of the cloud exit are the same file")
    );
}

#[test]
fn the_relay_socket_is_checked_like_the_other_sockets_and_the_files_like_the_other_sources() {
    assert_eq!(
        cloud_aendern(|c| c.weiterreicher = PathBuf::from("/var/tmp/k/s-1/such.sock")),
        Fehler::Ueberlappt("both sockets are the same file")
    );
    assert_eq!(
        cloud_aendern(|c| c.weiterreicher = PathBuf::from("/srv/arbeit/p/relay.sock")),
        Fehler::Ueberlappt("socket lies inside a mounted folder")
    );
    for aendern in [
        (|c: &mut Cloud| c.weiterreicher = PathBuf::from("/srv/.ssh/r.sock")) as fn(&mut Cloud),
        |c| c.bundle = PathBuf::from("/var/tmp/90-secrets/b.pem"),
        |c| c.weiter_programm = PathBuf::from("/home/u/.ssh/weiter"),
    ] {
        assert!(
            matches!(cloud_aendern(aendern), Fehler::Verboten(_)),
            "a secret or login store must be refused"
        );
    }
}

#[test]
fn the_check_of_the_environment_text_takes_only_the_allow_list_and_the_fixed_values() {
    let gut = kopf(&cloud_muster(Verteilung::Ubuntu));
    assert_eq!(umgebung_pruefen(&gut, "copilot"), Ok(()));
    assert_eq!(umgebung_pruefen(&kopf(&muster()), "claude"), Ok(()));
    let mit = |name: &str, wert: &str| {
        let mut k = gut.clone();
        let i = k.iter().position(|a| a == "--chdir").expect("--chdir");
        k.splice(
            i..i,
            worte("--setenv")
                .into_iter()
                .chain([name.to_owned(), wert.to_owned()]),
        );
        k
    };
    for name in [
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NODE_EXTRA_CA_CERTS",
        "HTTP_PROXY",
        "NO_PROXY",
        "GIT_CONFIG_COUNT",
        "LD_PRELOAD",
        "COPILOT_HOME",
    ] {
        assert_eq!(
            umgebung_pruefen(&mit(name, "x"), "copilot"),
            Err(Fehler::Verboten("variable outside the allow list")),
            "{name}"
        );
    }
    let eigen = Fehler::Verboten("cloud variable with a value of its own");
    let eins_daneben = format!("{}1", &COPILOT_PLATZHALTER[..COPILOT_PLATZHALTER.len() - 1]);
    for (name, wert) in [
        ("COPILOT_GITHUB_TOKEN", "gho_MARKE"),
        ("COPILOT_GITHUB_TOKEN", eins_daneben.as_str()),
        ("HTTPS_PROXY", "http://127.0.0.1:8403"),
        ("HTTPS_PROXY", "http://example.org:8402"),
        ("COPILOT_AUTO_UPDATE", "true"),
        ("COPILOT_MASCOT_BLINK", "1"),
    ] {
        assert_eq!(
            umgebung_pruefen(&mit(name, wert), "copilot"),
            Err(eigen.clone()),
            "{name}={wert}"
        );
    }
    // a switch without its value
    let mut kurz = gut;
    kurz.truncate(kurz.iter().rposition(|a| a == "--setenv").expect("setenv") + 1);
    assert_eq!(
        umgebung_pruefen(&kurz, "copilot"),
        Err(Fehler::Verboten("variable without a value"))
    );
}

#[test]
fn the_distribution_is_read_from_the_release_text() {
    let lesen = Verteilung::aus_os_release;
    assert_eq!(
        lesen("NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n"),
        Some(Verteilung::Ubuntu)
    );
    assert_eq!(lesen("ID=debian\n"), Some(Verteilung::Ubuntu));
    assert_eq!(
        lesen("ID=\"linuxmint\"\nID_LIKE=\"ubuntu debian\"\n"),
        Some(Verteilung::Ubuntu)
    );
    assert_eq!(lesen("ID=arch\n"), Some(Verteilung::Arch));
    assert_eq!(lesen("ID=omarchy\nID_LIKE=arch\n"), Some(Verteilung::Arch));
    assert_eq!(lesen("ID='arch'\n"), Some(Verteilung::Arch));
    assert_eq!(lesen("ID=fedora\nID_LIKE=\"rhel centos\"\n"), None);
    assert_eq!(lesen("PRETTY_NAME=\"arch ubuntu\"\n"), None);
    assert_eq!(lesen(""), None);
    assert_eq!(
        Verteilung::Ubuntu.bundle_orte(),
        ["/etc/ssl/certs/ca-certificates.crt"]
    );
    assert_eq!(
        Verteilung::Arch.bundle_orte(),
        ["/etc/ssl/certs/ca-certificates.crt", "/etc/ssl/cert.pem"]
    );
}

/// The local route of the harness `pi`: the model socket with its forwarder on 11434 and the provider file.
fn pi_muster() -> Beschreibung {
    let mut b = muster();
    b.harness = HARNESS_PI.to_owned();
    ".pi".clone_into(&mut b.zustand.name);
    b.lokal_weiter = Some(PathBuf::from("/usr/lib/werkbank/einschluss-weiter"));
    b.dateien = vec![crate::Datei {
        quelle: PathBuf::from("/var/tmp/d/s-1/models.json"),
        ziel: gast_pfad(&b.heim, ".pi/agent/models.json"),
    }];
    b.argv = worte("/srv/bin/pi --provider wb-lokal --model m");
    b
}

fn folge(liste: &[String], teil: &str) -> Option<usize> {
    let w = worte(teil);
    liste.windows(w.len()).position(|f| f == w.as_slice())
}

#[test]
fn the_local_route_runs_the_harness_below_the_forwarder_on_11434() {
    let a = bauen(&pi_muster()).expect("builds").argumente();
    let trenner = a.iter().position(|x| x == "--").expect("--");
    assert_eq!(
        a[trenner + 1..].to_vec(),
        worte(
            "/run/brain/weiter 11434 /run/brain/modell.sock -- /srv/bin/pi --provider wb-lokal --model m"
        )
    );
    assert!(
        folge(
            &a,
            "--ro-bind /usr/lib/werkbank/einschluss-weiter /run/brain/weiter"
        )
        .is_some()
    );
    assert!(
        folge(
            &a,
            "--ro-bind /var/tmp/k/s-1/modell.sock /run/brain/modell.sock"
        )
        .is_some()
    );
    // the model socket alone (no forwarder) keeps the old list: nothing listens inside
    let mut ohne = pi_muster();
    ohne.lokal_weiter = None;
    let a = bauen(&ohne).expect("builds").argumente();
    let trenner = a.iter().position(|x| x == "--").expect("--");
    assert_eq!(a[trenner + 1], "/srv/bin/pi");
    assert!(!a.iter().any(|x| x == GAST_WEITER));
}

#[test]
fn a_session_starts_below_the_anchor_and_the_anchor_below_nothing() {
    for b in [muster(), pi_muster()] {
        let profil = bauen(&b).expect("builds");
        let ohne = profil.argumente();
        let mit = profil.argumente_gebunden();
        let trenner = ohne.iter().position(|x| x == "--").expect("--");
        // the switches are the same, the anchor is the first program, the rest follows unchanged (forwarder included)
        assert_eq!(mit[..=trenner], ohne[..=trenner]);
        assert_eq!(
            mit[trenner + 1..trenner + 5],
            ["/usr/bin/sh", "-c", ANKER_SKRIPT, ANKER_NAME]
        );
        assert_eq!(mit[trenner + 5..], ohne[trenner + 1..]);
        assert_eq!(mit.len(), ohne.len() + 4);
    }
}

#[test]
fn the_anchor_reads_the_lifeline_on_its_descriptor_and_closes_it() {
    let fd = LEINE_FD.to_string();
    assert!(ANKER_SKRIPT.contains(&format!(
        "[ -p /proc/self/fd/{fd} ] || exit {ANKER_ABBRUCH}"
    )));
    assert!(ANKER_SKRIPT.contains(&format!("dd if=/proc/self/fd/{fd} iflag=nonblock")));
    assert!(ANKER_SKRIPT.contains(&format!("\nexec {fd}<&-\n")));
    assert!(ANKER_SKRIPT.ends_with("\nexec /usr/bin/env -u SHLVL -- \"$@\""));
    // every way out before the `exec` of the program is the one code
    // the exit code of the read decides, never its text: only 1 (`EAGAIN`) goes on
    assert!(ANKER_SKRIPT.contains(&format!(
        "count=0 status=none >/dev/null 2>&1 || exit {ANKER_ABBRUCH}\n"
    )));
    assert!(ANKER_SKRIPT.contains(&format!("\n[ \"$?\" = 1 ] || exit {ANKER_ABBRUCH}\n")));
    assert!(!ANKER_SKRIPT.contains("LC_ALL"));
    // every way out before the `exec` of the program is the one code: two in the wait, one for the pipe, two for the
    // probe, one for the open, one for the read, one for a program path with `=`
    assert_eq!(ANKER_SKRIPT.matches("exit ").count(), 8);
    assert_eq!(
        ANKER_SKRIPT
            .matches(&format!("exit {ANKER_ABBRUCH}"))
            .count(),
        8
    );
}

#[test]
fn the_configuration_file_is_bound_read_only_after_the_state_folder() {
    let a = bauen(&pi_muster()).expect("builds").argumente();
    let heim = muster().heim.to_string_lossy().into_owned();
    let zustand = folge(&a, &format!("--bind /var/tmp/z/s-1 {heim}/.pi")).expect("state");
    let datei = folge(
        &a,
        &format!("--ro-bind /var/tmp/d/s-1/models.json {heim}/.pi/agent/models.json"),
    )
    .expect("file");
    assert!(
        zustand < datei,
        "the file must cover the writable state folder"
    );
    let mut copilot = cloud_muster(Verteilung::Ubuntu);
    copilot.dateien = vec![crate::Datei {
        quelle: PathBuf::from("/var/tmp/d/s-1/wb-mcp.json"),
        ziel: gast_pfad(&copilot.heim, ".copilot/wb-mcp.json"),
    }];
    let a = bauen(&copilot)
        .expect("the copilot place passes the login lists")
        .argumente();
    assert!(
        folge(
            &a,
            &format!("--ro-bind /var/tmp/d/s-1/wb-mcp.json {heim}/.copilot/wb-mcp.json")
        )
        .is_some()
    );
}

#[test]
fn a_configuration_file_at_another_place_or_twice_is_refused() {
    let ziel = Fehler::BeschreibungUngueltig("dateien.ziel (not a fixed place of the harness)");
    let mit = |aendern: fn(&mut Beschreibung)| {
        let mut b = pi_muster();
        aendern(&mut b);
        bauen(&b).expect_err("must be refused")
    };
    assert_eq!(
        mit(|b| b.dateien[0].ziel = gast_pfad(&b.heim, ".pi/agent/auth.json")),
        ziel
    );
    assert_eq!(
        mit(|b| b.dateien[0].ziel = gast_pfad(&b.heim, ".copilot/wb-mcp.json")),
        ziel
    );
    assert_eq!(
        mit(|b| b.dateien[0].ziel = gast_pfad(&b.heim.with_file_name("v"), ".pi/agent/models.json")),
        ziel
    );
    assert_eq!(mit(|b| b.harness = "claude".into()), ziel);
    assert_eq!(
        mit(|b| b.dateien.push(b.dateien[0].clone())),
        Fehler::BeschreibungUngueltig("dateien.ziel twice")
    );
    assert_eq!(
        mit(|b| b.dateien[0].quelle = "/var/tmp/d/../models.json".into()),
        Fehler::BeschreibungUngueltig("dateien.quelle")
    );
    let schreibbar = Fehler::Ueberlappt("a read-only file lies in a folder the harness can write");
    assert_eq!(
        mit(|b| b.dateien[0].quelle = "/srv/arbeit/p/models.json".into()),
        schreibbar
    );
    assert_eq!(
        mit(|b| b.dateien[0].quelle = "/var/tmp/z/s-1/models.json".into()),
        schreibbar
    );
    assert_eq!(
        mit(|b| b.lokal_weiter = Some("/srv/arbeit/p/weiter".into())),
        schreibbar
    );
    assert!(matches!(
        mit(|b| b.dateien[0].quelle = "/var/tmp/90-secrets/models.json".into()),
        Fehler::Verboten(_)
    ));
    assert!(matches!(
        mit(|b| b.lokal_weiter = Some(b.heim.join(".ssh/weiter"))),
        Fehler::Verboten(_)
    ));
    assert_eq!(
        mit(|b| b.modell_socket = None),
        Fehler::BeschreibungUngueltig("lokal_weiter without modell_socket")
    );
    assert_eq!(
        mit(|b| b.lokal_weiter = Some("weiter".into())),
        Fehler::BeschreibungUngueltig("lokal_weiter")
    );
}

#[test]
fn only_pi_gets_its_two_variables_with_fixed_values() {
    let a = bauen(&pi_muster()).expect("builds").argumente();
    assert!(folge(&a, "--setenv PI_OFFLINE 1").is_some());
    assert!(folge(&a, "--setenv PI_TELEMETRY 0").is_some());
    for b in [muster(), cloud_muster(Verteilung::Ubuntu)] {
        let a = bauen(&b).expect("builds").argumente();
        assert!(!a.iter().any(|x| x.starts_with("PI_")), "{}", b.harness);
    }
    let gut = kopf(&pi_muster());
    assert_eq!(umgebung_pruefen(&gut, "pi"), Ok(()));
    assert_eq!(
        umgebung_pruefen(&gut, "claude"),
        Err(Fehler::Verboten("variable of another harness"))
    );
    for (name, wert) in [
        ("PI_OFFLINE", "0"),
        ("PI_TELEMETRY", "1"),
        ("PI_OFFLINE", ""),
    ] {
        let mut k = gut.clone();
        let i = k.iter().position(|x| x == "--chdir").expect("--chdir");
        k.splice(
            i..i,
            [String::from("--setenv"), name.to_owned(), wert.to_owned()],
        );
        assert_eq!(
            umgebung_pruefen(&k, "pi"),
            Err(Fehler::Verboten("cloud variable with a value of its own")),
            "{name}={wert}"
        );
    }
}

fn mit_ordnermarke() -> Beschreibung {
    let mut b = muster();
    b.system
        .ro_binds
        .push(("/dev/null".to_owned(), ORDNERMARKE_PFAD.to_owned()));
    b
}

#[test]
fn the_command_of_the_human_is_hidden_by_dev_null_right_after_the_bind_of_usr() {
    let ohne = kopf(&muster());
    assert!(
        !ohne.iter().any(|a| a == "/dev/null"),
        "without the file there is no bind"
    );
    let profil = bauen(&mit_ordnermarke()).expect("the pair is accepted");
    let k = &profil.kopf;
    let usr = k
        .windows(3)
        .position(|w| w == ["--ro-bind", "/usr", "/usr"])
        .expect("usr");
    let versteckt = k
        .windows(3)
        .position(|w| w == ["--ro-bind", "/dev/null", ORDNERMARKE_PFAD])
        .expect("the bind over the command");
    assert!(versteckt > usr, "after the bind of /usr");
    assert_eq!(
        k.iter().filter(|a| *a == "/dev/null").count(),
        1,
        "one bind of /dev/null"
    );
    assert_eq!(kopf_gut(k), Ok(()));
}

#[test]
fn every_other_bind_of_dev_null_is_refused_by_the_text_check() {
    let gut = kopf(&mit_ordnermarke());
    assert_eq!(kopf_gut(&gut), Ok(()));
    let neu = |option: &str, ziel: &str| -> Vec<String> {
        let mut k = gut.clone();
        let i = k
            .windows(3)
            .position(|w| w[1] == "/dev/null")
            .expect("the bind");
        k[i] = option.to_owned();
        k[i + 2] = ziel.to_owned();
        k
    };
    let verboten = Fehler::Verboten("bind of /dev/null other than over the command of the human");
    for (option, ziel) in [
        ("--bind", ORDNERMARKE_PFAD),
        ("--ro-bind", "/usr/bin/werkbank-lauf"),
        ("--ro-bind", "/etc/passwd"),
        ("--ro-bind", "/usr/bin/werkbank-ordnermarke2"),
        ("--ro-bind-try", ORDNERMARKE_PFAD),
    ] {
        assert_eq!(
            kopf_gut(&neu(option, ziel)),
            Err(verboten.clone()),
            "{option} {ziel}"
        );
    }
    // and through the description
    let mut b = muster();
    b.system
        .ro_binds
        .push(("/dev/null".to_owned(), "/usr/bin/anderes".to_owned()));
    assert_eq!(bauen(&b).expect_err("refused"), verboten);
}

#[cfg(target_os = "linux")]
#[test]
fn the_host_binds_dev_null_over_the_command_only_when_the_file_exists() {
    let paar = ("/dev/null".to_owned(), ORDNERMARKE_PFAD.to_owned());
    let da = std::env::temp_dir().join(format!("einschluss-om-{}", std::process::id()));
    std::fs::write(&da, b"x").expect("file");
    let mit = System::vom_host_mit(&da).expect("host");
    let ohne = System::vom_host_mit(&da.with_extension("fehlt")).expect("host");
    let _ = std::fs::remove_file(&da);
    assert_eq!(mit.ro_binds.iter().filter(|p| **p == paar).count(), 1);
    assert!(!ohne.ro_binds.contains(&paar));
    assert_eq!(mit.ro_binds[..mit.ro_binds.len() - 1], ohne.ro_binds[..]);
    assert_eq!(mit.ro_binds.last(), Some(&paar), "after the files of /etc");
}

/// The mechanism of the bind in a real `bwrap` (Linux, a host that lets `bwrap` make a user namespace; otherwise the
/// test says so and stops): a file of `/usr/bin` that is bound over by `/dev/null` is empty inside, and without the
/// bind it is not. The place `ORDNERMARKE_PFAD` itself exists only in the package, so the test takes `/usr/bin/true`.
#[cfg(target_os = "linux")]
#[test]
#[allow(clippy::print_stderr)]
fn a_bind_of_dev_null_over_a_file_of_usr_makes_it_empty_inside_a_real_enclosure() {
    let laeuft = |extra: &[&str], befehl: &str| -> Option<bool> {
        let mut c = Command::new(BWRAP_PFAD);
        c.args(["--unshare-user", "--ro-bind", "/", "/"])
            .args(extra)
            .args(["--", "/bin/sh", "-c", befehl])
            .env_clear();
        c.status().ok().map(|s| s.success())
    };
    let Some(ohne) = laeuft(&[], "test -s /usr/bin/true") else {
        eprintln!("no bwrap on this host: the mechanism is not measured here");
        return;
    };
    if !ohne {
        eprintln!("bwrap cannot make a user namespace here: the mechanism is not measured here");
        return;
    }
    let versteckt = ["--ro-bind", "/dev/null", "/usr/bin/true"];
    assert_eq!(
        laeuft(&versteckt, "test -s /usr/bin/true"),
        Some(false),
        "the file is empty inside"
    );
    assert_eq!(
        laeuft(&[], "/usr/bin/true"),
        Some(true),
        "it runs without the bind"
    );
    assert_eq!(
        laeuft(&versteckt, "/usr/bin/true"),
        Some(false),
        "it cannot be started with the bind"
    );
}

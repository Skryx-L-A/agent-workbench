//! One session of the service: the state core, the carrier with its child and what the service keeps of them. The service holds
//! its sessions in a table by identifier (`Dienst::sitzungen`); with the start file of today the table has one entry.

#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process};
use serde_json::{Value, json};
use traeger::{Kind, Raster, Start};
use zustandskern::Sitzungskern;

use crate::aufraeumen::{KindZeit, sitzung_ende_raeumt_auf};
use crate::fehler::StartFehler;
use crate::meldung::{CODE_UNGUELTIG, Ende, GRUND_UNGUELTIG, Meldung};
use crate::startdatei::Startdatei;
use crate::starten::KILL_FRIST_MS;
use crate::waisen::Kindblick;

/// What the carrier holds that the core is built from (DI6, DI11).
pub(crate) struct Traegerstand {
    /// Columns and rows.
    pub(crate) groesse: (usize, usize),
    /// The lines the carrier reported last, for a rebuild; `None` at the start (the core knows empty lines).
    pub(crate) zeilen: Option<Vec<String>>,
    /// The last `stand` of the carrier.
    pub(crate) stand: u64,
    /// DEC mode 2004.
    pub(crate) bracketed_paste: bool,
    /// DEC mode 5.
    pub(crate) invers: bool,
    /// Row, column and visibility the carrier reported last; at the start `(0, 0, true)` (DI11).
    pub(crate) cursor: (usize, usize, bool),
}

impl Traegerstand {
    /// The state before any output: the size of the start file and the start cursor `[0, 0, true]` (DI11).
    pub(crate) fn anfang(groesse: (usize, usize)) -> Self {
        Self {
            groesse,
            zeilen: None,
            stand: 0,
            bracketed_paste: false,
            invers: false,
            cursor: (0, 0, true),
        }
    }

    /// The state of a running carrier: its screen, and the modes the service saw in the events.
    pub(crate) fn von(raster: &Raster, bracketed_paste: bool, invers: bool) -> Self {
        Self {
            groesse: raster.groesse_jetzt(),
            zeilen: Some(raster.zeilen()),
            stand: raster.stand(),
            bracketed_paste,
            invers,
            cursor: raster.cursor(),
        }
    }
}

/// The `anfang` for `Sitzungskern::neu` (DI6 and DI11): the `anfang` of the start file with, in its one session, the size of the
/// carrier, the cursor and (at a rebuild) lines, `stand` and modes of the carrier.
pub(crate) fn anfang_fuer(anfang: &Value, eintrag: Option<&Value>, stand: &Traegerstand) -> Value {
    let mut a = anfang.clone();
    if let (Some(e), Some(o)) = (eintrag, a.as_object_mut()) {
        // a session started with `Starten`: its core holds exactly this entry (DI12)
        o.insert("sitzungen".to_owned(), json!([e]));
    }
    if let Some(s0) = a
        .get_mut("sitzungen")
        .and_then(|s| s.get_mut(0))
        .and_then(Value::as_object_mut)
    {
        s0.insert(
            "groesse".to_owned(),
            json!([stand.groesse.0, stand.groesse.1]),
        );
        s0.insert(
            "cursor".to_owned(),
            json!([stand.cursor.0, stand.cursor.1, stand.cursor.2]),
        );
        if let Some(zeilen) = &stand.zeilen {
            s0.insert("zeilen".to_owned(), json!(zeilen));
            s0.insert("stand".to_owned(), json!(stand.stand));
            s0.insert("bracketed_paste".to_owned(), json!(stand.bracketed_paste));
            s0.insert("invers".to_owned(), json!(stand.invers));
        }
    }
    a
}

/// The `anfang` of the room core (DI12): the start without any session.
pub(crate) fn anfang_raum(anfang: &Value) -> Value {
    let mut a = anfang.clone();
    if let Some(o) = a.as_object_mut() {
        o.insert("sitzungen".to_owned(), json!([]));
    }
    a
}

/// The entry of `anfang.sitzungen` for a session of `Starten` (DI12): the id, the route of the start structure, the room and the
/// person of the viewers and the size of its PTY.
pub(crate) fn eintrag_fuer(
    kennung: &str,
    route: &str,
    raum: &str,
    person: &str,
    groesse: (usize, usize),
) -> Value {
    json!({"id": kennung, "titel": "", "route": if route.is_empty() { Value::Null } else { json!(route) },
           "kundenraum": raum, "person": person, "groesse": [groesse.0, groesse.1]})
}

/// What the carrier needs to start a child.
pub(crate) struct KindStart {
    pub(crate) argv: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) cwd: String,
    pub(crate) groesse: (usize, usize),
    pub(crate) ruecklauf: usize,
}

impl KindStart {
    /// The child of the start file (`kind`, `groesse`).
    pub(crate) fn von_start(start: &Startdatei) -> Self {
        Self {
            argv: start.argv.clone(),
            env: start.env.clone(),
            cwd: start.cwd.clone(),
            groesse: start.groesse,
            ruecklauf: start.ruecklauf,
        }
    }

    /// The child of a session of `Starten` (DI12, section 18): `argv` is exactly `[lauf.programm, "--meldung", <meldung>, <start.json>]`
    /// (the paths are absolute), the environment exactly `lauf.env`, the working directory `/`.
    pub(crate) fn von_lauf(start: &Startdatei, datei: &Path, meldung: &Path) -> Option<Self> {
        let l = start.lauf.as_ref()?;
        Some(Self {
            argv: vec![
                l.programm.clone(),
                "--meldung".to_owned(),
                meldung.to_string_lossy().into_owned(),
                datei.to_string_lossy().into_owned(),
            ],
            env: l.env.clone(),
            cwd: "/".to_owned(),
            groesse: l.groesse,
            ruecklauf: start.ruecklauf,
        })
    }
}

/// The preconditions of the start of the child, checked here because the carrier cannot be relied on for them: `portable-pty`
/// takes the home folder when the working directory is no folder, and the program is started through [`crate::huelle`], so that
/// its failure shows only after the start (DI8: exit code 4 before anything listens).
pub(crate) fn kind_vorpruefen(start: &KindStart) -> Result<(), StartFehler> {
    let kein_kind =
        |text: &str| StartFehler::neu(4, &format!("Kind laesst sich nicht starten: {text}"));
    if !std::fs::metadata(&start.cwd).is_ok_and(|m| m.is_dir()) {
        return Err(kein_kind("Arbeitsverzeichnis fehlt"));
    }
    let programm = start.argv.first().map_or("", String::as_str);
    match std::fs::metadata(programm) {
        #[cfg(unix)]
        Ok(m) if m.is_file() && m.permissions().mode() & 0o111 != 0 => Ok(()),
        // Windows has no execute bit: a file is a program (section 21)
        #[cfg(windows)]
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => Err(kein_kind("Programm nicht ausfuehrbar")),
        Err(_) => Err(kein_kind("Programm fehlt")),
    }
}

/// The argv of the carrier: the program of the start file behind the cover of [`crate::huelle`] (the same program as `wb-dienst`).
#[cfg(unix)]
pub(crate) fn kind_argv(start: &KindStart, huelle: &str) -> Vec<String> {
    let mit_shell = start.env.iter().any(|(n, _)| n == "SHELL");
    let mut argv = vec![
        huelle.to_owned(),
        "--exec".to_owned(),
        if mit_shell { "mit-shell" } else { "ohne-shell" }.to_owned(),
    ];
    argv.extend(start.argv.iter().cloned());
    argv
}

/// Windows (section 21): the argv of the start file as it is. `portable-pty` adds `SHELL` only under Unix, and the carrier clears
/// the environment, so the child gets exactly `kind.env` without the cover; the process of the carrier is the program itself, as
/// after `exec` under Unix.
#[cfg(windows)]
pub(crate) fn kind_argv(start: &KindStart, _huelle: &str) -> Vec<String> {
    start.argv.clone()
}

/// Starts the child of the start file in a PTY.
///
/// SECURITY (DI8): the environment of the child is exactly `start.env`. `Kind::starten` of the carrier clears the environment
/// (`env_clear`) and sets these variables, but `portable-pty` adds `SHELL` to every child. So the child is started through a
/// cover (`wb-dienst --exec`, `huelle.rs`) that clears the environment once more, keeps what the start file names and exec's the
/// program: the service adds none and takes none from its own environment.
pub(crate) fn kind_starten(start: &KindStart, huelle: Option<&Path>) -> Result<Kind, StartFehler> {
    kind_vorpruefen(start)?;
    let huelle = match huelle {
        Some(h) => h.to_path_buf(),
        None => std::env::current_exe().map_err(|e| {
            StartFehler::neu(4, &format!("Kind laesst sich nicht starten: {}", e.kind()))
        })?,
    };
    Kind::starten(Start {
        argv: kind_argv(start, &huelle.to_string_lossy()),
        env: start.env.clone(),
        cwd: PathBuf::from(&start.cwd),
        spalten: start.groesse.0,
        zeilen: start.groesse.1,
        ruecklauf: start.ruecklauf,
    })
    .map_err(|e| StartFehler::neu(4, &format!("Kind laesst sich nicht starten: {e}")))
}

/// Windows (section 21a point 4): the job of a new child. `Ok(None)`: the child had ended before it could join, so no process of it is
/// left to hold (the session runs without a job and reports the end as usual). `Err`: the job could not be made or set, or a running
/// child could not join; the start is refused (fail closed).
#[cfg(windows)]
fn job_fuer(kind: &mut Kind) -> Result<Option<crate::sys::Job>, ()> {
    let job = crate::sys::Job::neu().map_err(drop)?;
    let griff = kind.prozess_griff().ok_or(())?;
    match job.aufnehmen(griff) {
        Ok(()) => Ok(Some(job)),
        Err(_) if kind.prozess_beendet() => Ok(None),
        Err(_) => Err(()),
    }
}

/// One core of the service: a session (carrier, child, core) or the room core without a session (DI12).
pub(crate) struct Sitzung {
    /// The identifier; `None` for the room core.
    pub(crate) sid: Option<String>,
    /// The raster number in the frame (DI12 point 4, P25): 0 for the session of `kind` and the room core, one of 1 to 255 for a
    /// session of `Starten`; `None` once a later session took the number (the session cannot be watched live any more).
    pub(crate) nr: Option<u8>,
    /// The place of this session in the order of the ends of the run (1 for the first end); `None` while it runs.
    pub(crate) beendet_als: Option<u64>,
    /// The entry of `anfang.sitzungen` this core is built from (a session of `Starten`); `None` for the session of `kind`
    /// (its entry is in the start file) and for the room core.
    pub(crate) eintrag: Option<Value>,
    /// `harness`, `modell`, `route`, `kunde` of the start structure, for the list (DI12).
    pub(crate) meta: [String; 4],
    /// The folder `<ordner>/<kennung>` the service made for `Starten`; removed at the end.
    pub(crate) ordner: Option<PathBuf>,
    /// The socket of the report of `lauf` (section 18); `None` for the session of `kind`, for the room core and after the end.
    pub(crate) meldung: Option<Meldung>,
    /// The reason the report gave (or [`GRUND_UNGUELTIG`]), once the child has ended; the list shows it.
    pub(crate) grund: Option<String>,
    /// The open socket folder through which the folder above is removed (`loeschen.rs`).
    wurzel: Option<Arc<crate::sys::Ordner>>,
    /// Pid and start time of the child at the start of the session, for the protection of the pid files at the end.
    pub(crate) kind_zeit: KindZeit,
    /// The moment after which `Beenden` sends SIGKILL to a child that is still there.
    pub(crate) kill_ab: Option<Instant>,
    /// The core; built anew after an error (DI6).
    pub(crate) kern: Sitzungskern,
    /// The cores built so far (`kernbau` in the state).
    pub(crate) kernbau: u32,
    /// The carrier with its child; `None` once the child was ended at the end of the service, and for the room core.
    pub(crate) kind: Option<Kind>,
    /// The code of `Kindende` once it came.
    pub(crate) kind_ende: Option<u32>,
    /// A pidfd of the child from its start: its end is seen before the carrier collects it (`waisen.rs`).
    pub(crate) blick: Kindblick,
    /// Windows: the job object of the child (section 21); closing it ends the child and every process the child started. `Ok(None)` for
    /// the room core, for a child that had ended before it could join, and once the session has ended. `Err`: the child runs, but its
    /// job could not be made or the child could not join it (section 21a point 4); the caller ends the child and refuses the start
    /// ([`Sitzung::job_fehlt`]).
    #[cfg(windows)]
    job: Result<Option<crate::sys::Job>, ()>,
    /// `true` once the hunt for the orphans of this session was started (once per session, `waisen.rs`).
    pub(crate) gejagt: bool,
    /// DEC mode 2004, from the events of the carrier.
    pub(crate) bracketed_paste: bool,
    /// DEC mode 5, from the events of the carrier.
    pub(crate) invers: bool,
    /// The `t` of the last event given to the core; the next may not fall below it.
    pub(crate) zuletzt: i64,
}

impl Sitzung {
    /// The session of `kind` (number 0).
    pub(crate) fn neu(sid: String, kern: Sitzungskern, kind: Kind) -> Self {
        Self::mit(Some(sid), Some(0), kern, Some(kind))
    }

    /// The room core (DI12).
    pub(crate) fn raum(kern: Sitzungskern) -> Self {
        Self::mit(None, Some(0), kern, None)
    }

    /// A session of `Starten` (DI12).
    pub(crate) fn gestartet(
        sid: String,
        nr: u8,
        kern: Sitzungskern,
        kind: Kind,
        eintrag: Value,
        meta: [String; 4],
        ordner: (PathBuf, Arc<crate::sys::Ordner>, Meldung),
    ) -> Self {
        let kind_zeit = KindZeit::von(kind.pid());
        let mut s = Self::mit(Some(sid), Some(nr), kern, Some(kind));
        s.kind_zeit = kind_zeit;
        s.eintrag = Some(eintrag);
        s.meta = meta;
        s.ordner = Some(ordner.0);
        s.wurzel = Some(ordner.1);
        s.meldung = Some(ordner.2);
        s
    }

    fn mit(sid: Option<String>, nr: Option<u8>, kern: Sitzungskern, kind: Option<Kind>) -> Self {
        // the carrier has not been asked for the end of the child yet, so its pid is still the child
        let blick = Kindblick::von(kind.as_ref().and_then(Kind::pid));
        // Windows: the child joins its job right after its start (section 21a point 4); without a job the start is refused
        #[cfg(windows)]
        let mut kind = kind;
        #[cfg(windows)]
        let job = kind.as_mut().map_or(Ok(None), job_fuer);
        Self {
            sid,
            nr,
            beendet_als: None,
            eintrag: None,
            meta: Default::default(),
            ordner: None,
            meldung: None,
            grund: None,
            wurzel: None,
            kind_zeit: KindZeit::default(),
            kill_ab: None,
            kern,
            kernbau: 1,
            kind,
            kind_ende: None,
            blick,
            #[cfg(windows)]
            job,
            gejagt: false,
            bracketed_paste: false,
            invers: false,
            zuletzt: 0,
        }
    }

    /// Windows (section 21a point 4, fail closed): `true` when the child runs without a job. The caller ends the child and refuses the
    /// start: the session of `kind` with exit code 4, a session of `Starten` with `programm_nicht_startbar`.
    #[cfg(windows)]
    pub(crate) fn job_fehlt(&self) -> bool {
        self.job.is_err()
    }

    /// The end of the child has come with `code`: the socket of the report is read empty first, then judged (section 18). Returns the
    /// code the session ends with. Once only; later calls give `code` back.
    pub(crate) fn ende_auswerten(&mut self, code: u32) -> u32 {
        let Some(mut meldung) = self.meldung.take() else {
            return code;
        };
        match meldung.auswerten_am_ende() {
            Ende::Keine | Ende::Starter => code,
            Ende::Gemeldet { code, grund } => {
                self.grund = Some(grund);
                u32::from(code)
            }
            Ende::Ungueltig => {
                self.grund = Some(GRUND_UNGUELTIG.to_owned());
                CODE_UNGUELTIG
            }
        }
    }

    /// The end of the session cleans up (see `aufraeumen.rs`): signals first, then the folder. `eigene` are the pids of the own
    /// children of the service that have not been collected (the children of the other sessions): never signalled. Once only; a second
    /// call finds no folder.
    pub(crate) fn ende_aufraeumen(&mut self, eigene: &[u32]) {
        // Windows: closing the job ends what the child left behind (the counterpart of the orphans of section 17)
        #[cfg(windows)]
        {
            self.job = Ok(None);
        }
        if let (Some(ordner), Some(wurzel)) = (self.ordner.take(), self.wurzel.take()) {
            sitzung_ende_raeumt_auf(&ordner, wurzel.as_fd(), self.kind_zeit, eigene);
        }
    }

    /// The session id of the orphans of this session, once its child has ended (also before the carrier collected it), once per
    /// session: the pid of the child, which leads its session in the PTY.
    pub(crate) fn waisen_ab(&mut self) -> Option<u32> {
        if self.gejagt {
            return None;
        }
        let kind = self.kind.as_ref()?;
        if !kind.ist_zu_ende() && !self.blick.beendet() {
            return None;
        }
        self.gejagt = true;
        self.blick.pid()
    }

    /// The pid of a child the carrier has not collected yet: it is no orphan and is never collected by `waisen.rs`.
    pub(crate) fn eigenes_kind(&self) -> Option<u32> {
        self.kind
            .as_ref()
            .filter(|k| !k.ist_zu_ende())
            .and_then(Kind::pid)
    }

    /// `true` for the room core.
    pub(crate) fn ist_raum(&self) -> bool {
        self.sid.is_none()
    }

    /// What the carrier holds now, for a rebuild of the core.
    pub(crate) fn traegerstand(&self) -> Option<Traegerstand> {
        self.kind
            .as_ref()
            .map(|k| Traegerstand::von(k.raster(), self.bracketed_paste, self.invers))
    }

    /// Sends SIGHUP to a child that is still there and sets the moment of SIGKILL (`Beenden`, DI12 point 6). Windows (section 21):
    /// the carrier ends the child at once (`Kind::beenden`, `TerminateProcess`, code 129).
    pub(crate) fn beenden_anstossen(&mut self) {
        #[cfg(windows)]
        if let Some(kind) = self.kind.as_mut().filter(|k| !k.ist_zu_ende()) {
            let _ = kind.beenden();
        }
        #[cfg(unix)]
        if let Some(kind) = self.kind.as_ref().filter(|k| !k.ist_zu_ende())
            && let Some(pid) = pid_von(kind)
        {
            let _ = kill_process(pid, Signal::HUP);
        }
        self.kill_ab = Some(Instant::now() + Duration::from_millis(KILL_FRIST_MS));
    }

    /// SIGKILL once the moment has come and the child is still there. The pid is used only while the output of the child has not
    /// ended: after that the process may be collected and its number given to another process.
    pub(crate) fn kill_pruefen(&mut self, jetzt: Instant) {
        let Some(ab) = self.kill_ab else {
            return;
        };
        let ende = self.kind.as_ref().is_none_or(Kind::ist_zu_ende);
        if ende {
            self.kill_ab = None;
        } else if jetzt >= ab {
            #[cfg(unix)]
            if let Some(pid) = self.kind.as_ref().and_then(pid_von) {
                let _ = kill_process(pid, Signal::KILL);
            }
            // Windows: once more through the carrier (it does nothing to a child that has ended)
            #[cfg(windows)]
            if let Some(kind) = self.kind.as_mut() {
                let _ = kind.beenden();
            }
            self.kill_ab = None;
        }
    }
}

#[cfg(unix)]
fn pid_von(kind: &Kind) -> Option<Pid> {
    kind.pid()
        .and_then(|p| i32::try_from(p).ok())
        .and_then(Pid::from_raw)
}

/// Ends children (DI8): SIGHUP to all, up to 1 s of waiting for all together, then (in `Kind::drop`) SIGKILL, then the wait for
/// the processes. A child that has ended already gets no signal. No child survives. Windows (section 21): the carrier ends every
/// running child at once (`Kind::beenden`), the same wait follows; the jobs of the sessions end the rest when they close.
pub(crate) fn kinder_beenden(mut kinder: Vec<Kind>) {
    #[cfg(windows)]
    for kind in kinder.iter_mut().filter(|k| !k.ist_zu_ende()) {
        let _ = kind.beenden();
    }
    #[cfg(unix)]
    for kind in kinder.iter().filter(|k| !k.ist_zu_ende()) {
        if let Some(pid) = pid_von(kind) {
            let _ = kill_process(pid, Signal::HUP);
        }
    }
    let ende = Instant::now() + Duration::from_millis(KILL_FRIST_MS);
    loop {
        for kind in &mut kinder {
            if !kind.ist_zu_ende() {
                let _ = kind.lies(Duration::ZERO);
            }
        }
        if kinder.iter().all(Kind::ist_zu_ende) || Instant::now() >= ende {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // `Kind::drop` ends what still runs (SIGKILL) and waits for the process
    drop(kinder);
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn anfang() -> Value {
        json!({"personen": {"anna": {}}, "sitzungen": [{"id": "s", "titel": "T"}]})
    }

    #[test]
    fn start_ohne_zeilen_mit_cursor() {
        let a = anfang_fuer(&anfang(), None, &Traegerstand::anfang((40, 10)));
        let s = &a["sitzungen"][0];
        assert_eq!(s["groesse"], json!([40, 10]));
        assert_eq!(s["cursor"], json!([0, 0, true]));
        assert_eq!(s["titel"], json!("T"));
        for feld in ["zeilen", "stand", "bracketed_paste", "invers"] {
            assert!(s.get(feld).is_none(), "{feld}");
        }
        assert_eq!(a["personen"], anfang()["personen"]);
    }

    #[test]
    fn neubau_mit_stand_des_traegers() {
        let st = Traegerstand {
            groesse: (3, 2),
            zeilen: Some(vec!["ab".to_owned(), String::new()]),
            stand: 9,
            bracketed_paste: true,
            invers: true,
            cursor: (1, 2, false),
        };
        let a = anfang_fuer(&anfang(), None, &st);
        let s = &a["sitzungen"][0];
        assert_eq!(s["groesse"], json!([3, 2]));
        assert_eq!(s["zeilen"], json!(["ab", ""]));
        assert_eq!(s["stand"], json!(9));
        assert_eq!(s["bracketed_paste"], json!(true));
        assert_eq!(s["invers"], json!(true));
        assert_eq!(s["cursor"], json!([1, 2, false]));
        assert!(Sitzungskern::neu(&a).is_ok(), "the core takes the rebuild");
    }

    fn start_mit_env(env: &Value) -> KindStart {
        KindStart::von_start(
            &crate::startdatei::pruefen(&json!({
                "socket": {"ordner": "/tmp/x"},
                "betrachter": {"person": "a", "raum": "r", "ansicht": "fenster"},
                "kind": {"argv": ["/bin/echo", "eins", "zwei"], "env": env},
                "groesse": [10, 5],
                "anfang": {"personen": {"a": {}}, "sitzungen": [{"id": "s"}]}
            }))
            .expect("start file"),
        )
    }

    #[test]
    fn argv_der_huelle() {
        let ohne = kind_argv(&start_mit_env(&json!({"TERM": "x"})), "/pfad/wb-dienst");
        assert_eq!(
            ohne,
            [
                "/pfad/wb-dienst",
                "--exec",
                "ohne-shell",
                "/bin/echo",
                "eins",
                "zwei"
            ]
        );
        let mit = kind_argv(
            &start_mit_env(&json!({"SHELL": "/bin/sh"})),
            "/pfad/wb-dienst",
        );
        assert_eq!(
            mit,
            [
                "/pfad/wb-dienst",
                "--exec",
                "mit-shell",
                "/bin/echo",
                "eins",
                "zwei"
            ]
        );
        let name_aehnlich = kind_argv(&start_mit_env(&json!({"MYSHELL": "x"})), "h");
        assert_eq!(name_aehnlich[2], "ohne-shell");
    }

    #[test]
    fn vorpruefung_cwd_und_programm() {
        let mut start = start_mit_env(&json!({}));
        assert!(kind_vorpruefen(&start).is_ok());
        start.cwd = "/nirgends/ordner".to_owned();
        assert_eq!(kind_vorpruefen(&start).expect_err("no folder").code, 4);
        start.cwd = "/bin/echo".to_owned();
        assert_eq!(
            kind_vorpruefen(&start)
                .expect_err("a file is no folder")
                .code,
            4
        );
        start.cwd = "/".to_owned();
        start.argv[0] = "/nirgends/programm".to_owned();
        assert_eq!(kind_vorpruefen(&start).expect_err("no program").code, 4);
        start.argv[0] = "/tmp".to_owned();
        assert_eq!(
            kind_vorpruefen(&start)
                .expect_err("a folder is no program")
                .code,
            4
        );
        start.argv[0] = "/etc/hosts".to_owned();
        assert_eq!(kind_vorpruefen(&start).expect_err("not executable").code, 4);
        start.argv[0] = "/bin/echo".to_owned();
        assert!(kind_vorpruefen(&start).is_ok());
    }

    #[test]
    fn anfang_ohne_sitzung_bleibt_ganz() {
        let a = json!({"personen": {}});
        assert_eq!(anfang_fuer(&a, None, &Traegerstand::anfang((2, 1))), a);
    }

    #[test]
    fn anfang_einer_gestarteten_sitzung_und_des_raums() {
        let e = eintrag_fuer("s-4", "r1", "raum", "anna", (80, 24));
        assert_eq!(e["id"], json!("s-4"));
        assert_eq!(e["route"], json!("r1"));
        assert_eq!(e["groesse"], json!([80, 24]));
        assert_eq!(
            eintrag_fuer("s-4", "", "r", "a", (2, 1))["route"],
            Value::Null
        );
        let basis = json!({"personen": {"anna": {}}, "sitzungen": [{"id": "alt"}]});
        let a = anfang_fuer(&basis, Some(&e), &Traegerstand::anfang((80, 24)));
        assert_eq!(a["sitzungen"].as_array().map(Vec::len), Some(1));
        assert_eq!(a["sitzungen"][0]["id"], json!("s-4"));
        assert_eq!(a["sitzungen"][0]["cursor"], json!([0, 0, true]));
        assert!(Sitzungskern::neu(&a).is_ok());
        let r = anfang_raum(&basis);
        assert_eq!(r["sitzungen"], json!([]));
        assert!(Sitzungskern::neu(&r).is_ok(), "a core without a session");
    }

    #[test]
    fn kind_von_lauf_hat_vier_argumente_und_wurzel() {
        let mut start = crate::startdatei::pruefen(&json!({
            "socket": {"ordner": "/tmp/x"},
            "betrachter": {"person": "a", "raum": "r", "ansicht": "fenster"},
            "lauf": {"programm": "/bin/echo", "env": {"A": "1"}, "groesse": [30, 9], "einstellungen": "/e.json"},
            "anfang": {"personen": {"a": {}}}
        }))
        .expect("start file");
        let datei = Path::new("/tmp/x/s-1/start.json");
        let sock = Path::new("/tmp/x/s-1/meldung.sock");
        let k = KindStart::von_lauf(&start, datei, sock).expect("lauf");
        assert_eq!(
            k.argv,
            [
                "/bin/echo",
                "--meldung",
                "/tmp/x/s-1/meldung.sock",
                "/tmp/x/s-1/start.json"
            ]
        );
        assert_eq!(k.env, [("A".to_owned(), "1".to_owned())]);
        assert_eq!((k.cwd.as_str(), k.groesse), ("/", (30, 9)));
        start.lauf = None;
        assert!(KindStart::von_lauf(&start, Path::new("/x"), Path::new("/y")).is_none());
    }

    #[test]
    fn kinder_beenden_endet_kinder_die_sighup_annehmen_sofort_und_zusammen() {
        let start = Start {
            argv: vec!["/bin/sleep".to_owned(), "30".to_owned()],
            env: Vec::new(),
            cwd: PathBuf::from("/"),
            spalten: 20,
            zeilen: 5,
            ruecklauf: 10,
        };
        let kinder: Vec<Kind> = (0..2)
            .map(|_| Kind::starten(start.clone()).expect("child starts"))
            .collect();
        let pids: Vec<u32> = kinder.iter().filter_map(Kind::pid).collect();
        assert_eq!(pids.len(), 2);
        let t = Instant::now();
        kinder_beenden(kinder);
        // SIGHUP ends `sleep` at once: far less than the second of grace, and both end together
        assert!(
            t.elapsed() < Duration::from_millis(700),
            "{:?}",
            t.elapsed()
        );
        for pid in pids {
            let lebt = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            assert!(!lebt, "no child survives");
        }
    }
}

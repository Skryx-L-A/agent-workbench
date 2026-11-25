//! The few calls into the system without `unsafe`, through `rustix`.

#[cfg(target_os = "linux")]
use std::io;

use rustix::fs::Mode;

/// After the shielding (crate `kernabbild`, step 1): the process gets SIGTERM when its parent ends. Only Linux has the call, so it exists
/// only there; the start skips it elsewhere, as before.
///
/// # Errors
/// The system refuses the setting.
#[cfg(target_os = "linux")]
pub(crate) fn todessignal() -> io::Result<()> {
    rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))?;
    Ok(())
}

/// The real user id of the process.
pub(crate) fn eigene_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Runs `f` with the umask 0177, so a socket made in `f` never has wider rights than 0600, then restores the old mask.
pub(crate) fn mit_umask<T>(f: impl FnOnce() -> T) -> T {
    let alt = rustix::process::umask(Mode::from_bits_truncate(0o177));
    let ergebnis = f();
    rustix::process::umask(alt);
    ergebnis
}

/// The process id of the parent in this moment, or `None` where the process has no parent (a parent id of 0).
/// The start remembers it in its first line and compares it again after the death signal (contract 8, section 3):
/// a parent that died before the `prctl` sends no signal any more.
pub(crate) fn eltern() -> Option<i32> {
    rustix::process::getppid().map(|p| p.as_raw_nonzero().get())
}

/// Duty 4 of the safe mode (contract 8, section 3a): the umask 077 for the whole process, before it makes the first
/// file, folder or socket. Under setgid a new entry belongs to the group of the program; with this mask none of them
/// ever carries a right of the group or of others, whatever mode a call asks for.
#[cfg(target_os = "linux")]
pub(crate) fn umask_sicher() {
    rustix::process::umask(Mode::from_bits_truncate(0o077));
}

/// Duty 3 of the safe mode (contract 8, section 3a): replaces the program by itself with an empty environment. The
/// image is `/proc/self/exe`, the file the kernel runs now, not a path that a caller could change; argv stays except
/// for `name`, which takes the place of argv\[0\]. The process stays the same (same PID, same parent, no child), and
/// the kernel gives the new image a fresh block of the environment: only so the own environment is empty also in
/// `/proc/<pid>/environ`, which keeps the strings of the start as long as the image lives. Returns only on a failure.
#[cfg(target_os = "linux")]
pub(crate) fn neu_ohne_umgebung(name: &std::ffi::OsStr, args: &[std::ffi::OsString]) -> io::Error {
    use std::os::unix::process::CommandExt;
    std::process::Command::new("/proc/self/exe")
        .arg0(name)
        .args(args)
        .env_clear()
        .exec()
}

/// Duty 5 of the safe mode (contract 8, section 3a): whether one thread of this process has a child. Two sources,
/// and only both together may mean "no child": the files `/proc/self/task/<tid>/children` (the PIDs of the children
/// of each thread, separated by spaces) and the answer of the kernel from [`kinder_aus_warten`]. The files name every
/// child, whatever exit signal it carries, but the listing of `/proc/self/task` can break off early; the answer of
/// the kernel comes from the list of children itself and knows no such gap.
///
/// # Errors
/// `/proc` cannot be read, or the kernel refuses the question; the caller ends fail closed.
#[cfg(target_os = "linux")]
pub(crate) fn hat_kinder() -> io::Result<bool> {
    hat_kinder_mit(lesungen_der_faeden()?)
}

/// The readings of the `children` files of all threads that the listing of `/proc/self/task` showed.
#[cfg(target_os = "linux")]
fn lesungen_der_faeden() -> io::Result<Vec<io::Result<String>>> {
    let mut lesungen = Vec::new();
    for eintrag in std::fs::read_dir("/proc/self/task")? {
        lesungen.push(std::fs::read_to_string(eintrag?.path().join("children")));
    }
    Ok(lesungen)
}

/// [`hat_kinder`] with the readings already taken: a reading that names a PID counts at once, and otherwise the
/// kernel decides. So an incomplete reading never means "no child". The listing of `/proc/self/task` ends early when
/// a thread ends between two of its steps: the kernel holds the thread it just named, and a thread that is gone has
/// no next one, so the rest of the threads are never named (measured on Linux 7.2.5: of 1800 readings under ending
/// threads, 16 missed a living thread and with it its child).
#[cfg(target_os = "linux")]
fn hat_kinder_mit(lesungen: Vec<io::Result<String>>) -> io::Result<bool> {
    if kinder_aus(lesungen)? {
        return Ok(true);
    }
    kinder_aus_warten()
}

/// The answer of the kernel: has any thread of this process a child that nobody has waited for? `waitid` reads the
/// list of children of the thread group, not `/proc`; `NOHANG` returns at once, and `NOWAIT` leaves every child in
/// its state, so the question takes nothing away from a caller that waits for its own children. Without a child the
/// call fails with `ECHILD`.
///
/// # Errors
/// The kernel refuses the question for another reason; the caller ends fail closed.
#[cfg(target_os = "linux")]
fn kinder_aus_warten() -> io::Result<bool> {
    kinder_aus_antwort(
        rustix::process::waitid(rustix::process::WaitId::All, WARTEN_OPTIONEN).map(|_| ()),
    )
}

/// The options of the question of [`kinder_aus_warten`]. They stand as one constant, not as an expression in the
/// function: which state of a child the kernel may report (`EXITED`, `STOPPED`, `CONTINUED`) does not change the
/// answer, because every `Ok` means "there is a child"; only `NOHANG` (do not wait) and `NOWAIT` (take nothing away)
/// carry a meaning of their own.
#[cfg(target_os = "linux")]
const WARTEN_OPTIONEN: rustix::process::WaitIdOptions = rustix::process::WaitIdOptions::NOHANG
    .union(rustix::process::WaitIdOptions::NOWAIT)
    .union(rustix::process::WaitIdOptions::EXITED)
    .union(rustix::process::WaitIdOptions::STOPPED)
    .union(rustix::process::WaitIdOptions::CONTINUED);

/// The answer of the kernel as the duty reads it: `Ok` means a child, `ECHILD` means no child, and every other
/// failure goes to the caller, who ends fail closed.
///
/// # Errors
/// The kernel refused the question for another reason than "no child".
#[cfg(target_os = "linux")]
fn kinder_aus_antwort(antwort: Result<(), rustix::io::Errno>) -> io::Result<bool> {
    match antwort {
        Ok(()) => Ok(true),
        Err(f) if f == rustix::io::Errno::CHILD => Ok(false),
        Err(f) => Err(f.into()),
    }
}

/// Whether the readings of the `children` files of the threads name a child; a failure other than an ended thread is
/// passed on.
#[cfg(target_os = "linux")]
fn kinder_aus(lesungen: Vec<io::Result<String>>) -> io::Result<bool> {
    let mut texte = Vec::new();
    for lesung in lesungen {
        match lesung {
            Ok(t) => texte.push(t),
            // A thread that ended in between has no children any more: the kernel gave them to another thread of
            // this process. Whether the listing named that thread is not certain, so a reading without a PID is
            // never the whole answer (see `hat_kinder_mit`).
            Err(f) if task_weg(&f) => {}
            Err(f) => return Err(f),
        }
    }
    Ok(kinder_in(&texte))
}

/// Whether a failure of reading a file of a thread means that the thread ended (`ENOENT` or `ESRCH`).
#[cfg(target_os = "linux")]
fn task_weg(f: &io::Error) -> bool {
    f.kind() == io::ErrorKind::NotFound
        || f.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error())
}

/// Whether one of the texts of `children` names a PID.
#[cfg(target_os = "linux")]
pub(crate) fn kinder_in(texte: &[String]) -> bool {
    texte.iter().any(|t| !t.trim().is_empty())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{
        hat_kinder, hat_kinder_mit, kinder_aus, kinder_aus_antwort, kinder_aus_warten, kinder_in,
        lesungen_der_faeden, task_weg,
    };
    use std::io;

    fn errno(e: rustix::io::Errno) -> io::Error {
        io::Error::from_raw_os_error(e.raw_os_error())
    }

    /// Only an ended thread (`ENOENT`, `ESRCH`) is tolerated; any other failure of reading is not.
    #[test]
    fn only_an_ended_thread_is_tolerated() {
        assert!(task_weg(&io::Error::from(io::ErrorKind::NotFound)));
        assert!(task_weg(&errno(rustix::io::Errno::NOENT)));
        assert!(task_weg(&errno(rustix::io::Errno::SRCH)));
        assert!(!task_weg(&errno(rustix::io::Errno::ACCESS)));
        assert!(!task_weg(&io::Error::from(io::ErrorKind::PermissionDenied)));
        assert!(!task_weg(&errno(rustix::io::Errno::IO)));
    }

    /// The readings: an ended thread is skipped, a child in another thread is seen, any other failure ends fail
    /// closed.
    #[test]
    fn a_failure_of_reading_is_passed_on() {
        assert!(
            kinder_aus(vec![
                Err(errno(rustix::io::Errno::SRCH)),
                Ok("5\n".to_owned())
            ])
            .expect("ok")
        );
        assert!(
            !kinder_aus(vec![
                Err(io::Error::from(io::ErrorKind::NotFound)),
                Ok(String::new())
            ])
            .expect("ok")
        );
        let f = kinder_aus(vec![
            Ok("5".to_owned()),
            Err(errno(rustix::io::Errno::ACCESS)),
        ])
        .expect_err("err");
        assert_eq!(
            f.raw_os_error(),
            Some(rustix::io::Errno::ACCESS.raw_os_error())
        );
    }

    #[test]
    fn a_pid_in_one_file_is_a_child() {
        assert!(!kinder_in(&[]));
        assert!(!kinder_in(&[
            String::new(),
            "\n".to_owned(),
            " ".to_owned()
        ]));
        assert!(kinder_in(&[String::new(), "4242 ".to_owned()]));
        assert!(kinder_in(&["7".to_owned()]));
    }

    /// The reading of the system sees a child of this test process. (That it sees none is checked from outside, on the
    /// program: other tests of this binary start children of their own at any moment.)
    #[test]
    fn the_files_of_the_threads_show_a_child() {
        let mut kind = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        let gesehen = hat_kinder();
        let _ = kind.kill();
        let _ = kind.wait();
        assert!(gesehen.expect("proc"));
    }

    /// The gap of the listing of `/proc/self/task`: an incomplete reading never means "no child". The extreme case of
    /// an incomplete reading is the empty one; the files alone answer "no child" there, the whole check sees the child
    /// anyway, because the kernel answers from the list of children.
    #[test]
    fn an_incomplete_reading_never_means_no_child() {
        let mut kind = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        let aus_dateien = kinder_aus(Vec::new());
        let gesehen = hat_kinder_mit(Vec::new());
        let aus_warten = kinder_aus_warten();
        let _ = kind.kill();
        let _ = kind.wait();
        assert!(!aus_dateien.expect("ok"));
        assert!(gesehen.expect("warten"));
        assert!(aus_warten.expect("warten"));
    }

    /// The readings really come from the files of the threads: with a child of this thread one of them names its PID.
    /// The listing of `/proc/self/task` can break off (see `hat_kinder_mit`), so the reading is tried up to 20 times;
    /// readings that never name the child mean that no file was read.
    #[test]
    fn the_readings_of_the_threads_name_the_child() {
        let mut kind = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        let pid = kind.id().to_string();
        let mut gefunden = false;
        for _ in 0..20 {
            let texte: Vec<String> = lesungen_der_faeden()
                .expect("proc")
                .into_iter()
                .flatten()
                .collect();
            if texte.iter().any(|t| t.split_whitespace().any(|p| p == pid)) {
                gefunden = true;
                break;
            }
        }
        let _ = kind.kill();
        let _ = kind.wait();
        assert!(gefunden, "keine Lesung nannte das Kind {pid}");
    }

    /// The answer of the kernel: only `ECHILD` means "no child"; every other failure goes to the caller, who ends
    /// fail closed. A failure that counted as "no child" would be fail open for duty 5.
    #[test]
    fn only_echild_means_no_child() {
        assert!(kinder_aus_antwort(Ok(())).expect("ok"));
        assert!(!kinder_aus_antwort(Err(rustix::io::Errno::CHILD)).expect("ok"));
        for f in [
            rustix::io::Errno::ACCESS,
            rustix::io::Errno::INVAL,
            rustix::io::Errno::INTR,
            rustix::io::Errno::AGAIN,
        ] {
            assert_eq!(
                kinder_aus_antwort(Err(f)).expect_err("err").raw_os_error(),
                Some(f.raw_os_error()),
                "{f:?}"
            );
        }
    }
}

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

/// Duty 5 of the safe mode (contract 8, section 3a): whether one thread of this process has a child, from
/// `/proc/self/task/<tid>/children` (the PIDs of the children of each thread, separated by spaces).
///
/// # Errors
/// `/proc` cannot be read; the caller ends fail closed.
#[cfg(target_os = "linux")]
pub(crate) fn hat_kinder() -> io::Result<bool> {
    let mut lesungen = Vec::new();
    for eintrag in std::fs::read_dir("/proc/self/task")? {
        lesungen.push(std::fs::read_to_string(eintrag?.path().join("children")));
    }
    kinder_aus(lesungen)
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
            // this process, whose file is read as well.
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
    use super::{hat_kinder, kinder_aus, kinder_in, task_weg};
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
}

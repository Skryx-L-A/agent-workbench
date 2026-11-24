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

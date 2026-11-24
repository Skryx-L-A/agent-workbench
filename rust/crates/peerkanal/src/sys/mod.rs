//! The system calls. Windows: Win32 over `windows-sys` in the submodule `windows`, the only place of the crate with `unsafe`
//! (decision E1). Other systems: version 1 of this crate has no Unix side yet; the stand-in [`Os`] refuses every call.

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::{Os, job_oeffnen};

#[cfg(not(windows))]
mod ohne {
    use std::time::Duration;

    use crate::ablauf::{Lesen, Plattform};
    use crate::pruefung::Token;
    use crate::vorgabe::JobBezug;

    /// No system: the Unix side is not part of this build. `Server::anlegen` and `verbinden` return before they reach it.
    pub(crate) struct Os;

    impl Plattform for Os {
        type Griff = ();
        type Prozess = ();
        type Job = JobBezug;

        fn eigene_sid(&self) -> Result<String, i32> {
            Err(0)
        }
        fn instanz_anlegen(
            &self,
            _name: &str,
            _erste: bool,
            _n: u8,
            _sddl: &str,
        ) -> Result<(), i32> {
            Err(0)
        }
        fn auf_client_warten(&self, (): &()) -> Result<(), i32> {
            Err(0)
        }
        fn jetzt(&self) -> u64 {
            0
        }
        fn client_pid(&self, (): &()) -> Option<u32> {
            None
        }
        fn prozess_oeffnen(&self, _pid: u32) -> Option<((), u64)> {
            None
        }
        fn programm(&self, (): &()) -> Option<String> {
            None
        }
        fn lebt(&self, (): &()) -> Option<bool> {
            None
        }
        fn im_job(&self, (): &(), _job: &JobBezug) -> Option<bool> {
            None
        }
        fn kennung_messen(&self, (): &()) -> Result<Token, i32> {
            Err(0)
        }
        fn trennen(&self, (): &()) {}
        fn lesen(&self, (): &(), _puffer: &mut [u8], _frist: Option<Duration>) -> Lesen {
            Lesen::Fehler(0)
        }
        fn schreiben(&self, (): &(), _daten: &[u8]) -> Result<usize, i32> {
            Err(0)
        }
        fn oeffnen(&self, _name: &str) -> Result<(), i32> {
            Err(0)
        }
        fn warten(&self, _name: &str, _frist: Duration) {}
        fn besitzer(&self, (): &()) -> Result<String, i32> {
            Err(0)
        }
        fn server_pid(&self, (): &()) -> Option<u32> {
            None
        }
    }
}

#[cfg(not(windows))]
pub(crate) use ohne::Os;

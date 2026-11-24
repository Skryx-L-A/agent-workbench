//! The ACL building blocks of the Windows pipe (build plan section 3.2, B1 to B5 and B7) as pure values: the security
//! descriptor as SDDL text and the flags of `CreateNamedPipeW` and `CreateFileW`.
//!
//! The numbers stand here without `windows-sys`, so they are testable on every system. The module `sys` checks at compile
//! time that each number equals the constant of `windows-sys`.

use std::fmt::Write as _;

use crate::vorgabe::Erlaubnis;

/// `PIPE_ACCESS_DUPLEX`.
pub(crate) const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
/// `FILE_FLAG_FIRST_PIPE_INSTANCE`.
pub(crate) const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
/// `FILE_FLAG_OVERLAPPED`.
pub(crate) const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
/// `PIPE_TYPE_BYTE`.
pub(crate) const PIPE_TYPE_BYTE: u32 = 0;
/// `PIPE_READMODE_BYTE`.
pub(crate) const PIPE_READMODE_BYTE: u32 = 0;
/// `PIPE_REJECT_REMOTE_CLIENTS`.
pub(crate) const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x0000_0008;
/// `GENERIC_READ`.
pub(crate) const GENERIC_READ: u32 = 0x8000_0000;
/// `GENERIC_WRITE` (never asked for; only for the test).
#[cfg(test)]
const GENERIC_WRITE: u32 = 0x4000_0000;
/// `FILE_WRITE_DATA`.
pub(crate) const FILE_WRITE_DATA: u32 = 0x0000_0002;
/// `FILE_APPEND_DATA`, at a pipe `FILE_CREATE_PIPE_INSTANCE` (never asked for by the client).
pub(crate) const FILE_CREATE_PIPE_INSTANCE: u32 = 0x0000_0004;
/// `SECURITY_SQOS_PRESENT`.
pub(crate) const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
/// `SECURITY_IDENTIFICATION`.
pub(crate) const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;

/// The access of the own SID: read, write and `FILE_CREATE_PIPE_INSTANCE`.
pub(crate) const MASKE_EIGEN: u32 = 0x0012_019F;
/// The access of every other allowed SID and package SID: the same without `FILE_CREATE_PIPE_INSTANCE`.
pub(crate) const MASKE_ANDERE: u32 = 0x0012_019B;

/// The SIDs that never get an ACE: groups and the package groups.
#[cfg(test)]
pub(crate) const NIE_IN_DER_DACL: [&str; 9] = [
    "S-1-1-0",
    "S-1-5-11",
    "S-1-5-32-545",
    "S-1-5-32-544",
    "S-1-5-4",
    "S-1-5-2",
    "S-1-5-7",
    "S-1-15-2-1",
    "S-1-15-2-2",
];

/// The open mode of an instance. Only the first instance carries `FILE_FLAG_FIRST_PIPE_INSTANCE` (B4); every further instance
/// joins the pipe the server already holds.
pub(crate) fn oeffnungsmodus(erste: bool) -> u32 {
    let modus = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    if erste {
        modus | FILE_FLAG_FIRST_PIPE_INSTANCE
    } else {
        modus
    }
}

/// The pipe mode: bytes, local only (B5).
pub(crate) const PIPE_MODUS: u32 = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_REJECT_REMOTE_CLIENTS;

/// The access of the client (B7): `GENERIC_READ | FILE_WRITE_DATA`, never `GENERIC_WRITE`.
pub(crate) const CLIENT_ZUGRIFF: u32 = GENERIC_READ | FILE_WRITE_DATA;

/// The flags of the client (B7): identification only, so the server may read the identity but not act in its name.
pub(crate) const CLIENT_FLAGS: u32 =
    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION | FILE_FLAG_OVERLAPPED;

/// The security descriptor of the pipe as SDDL (B1 to B3).
///
/// Owner: the own SID. Protected DACL: the own SID with [`MASKE_EIGEN`], then in the order of the list every other allowed SID
/// and every package SID with [`MASKE_ANDERE`], each once. With a package SID in the list, the label `S:(ML;;NW;;;LW)`;
/// without one, no label.
///
/// `zusatz` is a seam of the outside runner only (feature `testnaht`): package SIDs with an ACE but without an entry in the list.
pub(crate) fn sddl(eigene: &str, erlaubt: &[Erlaubnis], zusatz: &[&str]) -> String {
    fn nehmen<'a>(sid: &'a str, eigene: &str, weitere: &mut Vec<&'a str>) {
        if sid != eigene && !weitere.contains(&sid) {
            weitere.push(sid);
        }
    }
    let mut weitere: Vec<&str> = Vec::new();
    let mut paket = false;
    for a in erlaubt {
        if let Erlaubnis::Windows {
            sid,
            appcontainer_sid,
        } = a
        {
            nehmen(sid.als_str(), eigene, &mut weitere);
            if let Some(p) = appcontainer_sid {
                paket = true;
                nehmen(p.als_str(), eigene, &mut weitere);
            }
        }
    }
    for z in zusatz {
        paket = true;
        nehmen(z, eigene, &mut weitere);
    }
    let mut text = format!("O:{eigene}D:P(A;;0x{MASKE_EIGEN:X};;;{eigene})");
    for sid in weitere {
        let _ = write!(text, "(A;;0x{MASKE_ANDERE:X};;;{sid})");
    }
    if paket {
        text.push_str("S:(ML;;NW;;;LW)");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kennung::{PackageSid, Sid};

    const PERSON: &str = "S-1-5-21-1000000001-1000000002-1000000003-1001";
    const DIENST: &str = "S-1-5-80-1000000011-1000000012-1000000013-1000000014-1000000015";
    const PAKET: &str =
        "S-1-15-2-1000000021-1000000022-1000000023-1000000024-1000000025-1000000026-1000000027";

    fn w(sid: &str, ac: Option<&str>) -> Erlaubnis {
        Erlaubnis::Windows {
            sid: Sid::neu(sid).expect("form"),
            appcontainer_sid: ac.map(|p| PackageSid::neu(p).expect("form")),
        }
    }

    #[test]
    fn such_pipe_wie_im_vertrag() {
        assert_eq!(
            sddl(PERSON, &[w(PERSON, Some(PAKET))], &[]),
            format!("O:{PERSON}D:P(A;;0x12019F;;;{PERSON})(A;;0x12019B;;;{PAKET})S:(ML;;NW;;;LW)")
        );
    }

    #[test]
    fn dienst_mit_person_und_container() {
        // Case a-dacl-windows: service, person without and with container.
        assert_eq!(
            sddl(DIENST, &[w(PERSON, None), w(PERSON, Some(PAKET))], &[]),
            format!(
                "O:{DIENST}D:P(A;;0x12019F;;;{DIENST})(A;;0x12019B;;;{PERSON})(A;;0x12019B;;;{PAKET})S:(ML;;NW;;;LW)"
            )
        );
    }

    #[test]
    fn ohne_paket_kein_label() {
        assert_eq!(
            sddl(DIENST, &[w(PERSON, None)], &[]),
            format!("O:{DIENST}D:P(A;;0x12019F;;;{DIENST})(A;;0x12019B;;;{PERSON})")
        );
        assert_eq!(
            sddl(PERSON, &[w(PERSON, None)], &[]),
            format!("O:{PERSON}D:P(A;;0x12019F;;;{PERSON})")
        );
    }

    #[test]
    fn nie_eine_gruppe() {
        let text = sddl(
            DIENST,
            &[w(PERSON, None), w(PERSON, Some(PAKET)), w("S-1-5-18", None)],
            &[],
        );
        for gruppe in NIE_IN_DER_DACL {
            assert!(
                !text.contains(&format!(";;;{gruppe})")),
                "{gruppe} in {text}"
            );
        }
        assert!(
            text.starts_with(&format!("O:{DIENST}D:P(")),
            "Besitzer und geschützte DACL: {text}"
        );
        assert_eq!(
            text.matches("0x12019F").count(),
            1,
            "nur die eigene SID legt Instanzen an: {text}"
        );
    }

    #[test]
    fn masken() {
        assert_eq!(MASKE_EIGEN & !MASKE_ANDERE, FILE_CREATE_PIPE_INSTANCE);
        assert_eq!(MASKE_ANDERE & FILE_CREATE_PIPE_INSTANCE, 0);
    }

    #[test]
    fn flags_der_instanz() {
        assert_ne!(
            oeffnungsmodus(true) & FILE_FLAG_FIRST_PIPE_INSTANCE,
            0,
            "B4"
        );
        assert_eq!(oeffnungsmodus(false) & FILE_FLAG_FIRST_PIPE_INSTANCE, 0);
        for erste in [true, false] {
            assert_eq!(
                oeffnungsmodus(erste) & PIPE_ACCESS_DUPLEX,
                PIPE_ACCESS_DUPLEX
            );
            assert_ne!(oeffnungsmodus(erste) & FILE_FLAG_OVERLAPPED, 0);
        }
        assert_ne!(PIPE_MODUS & PIPE_REJECT_REMOTE_CLIENTS, 0, "B5");
    }

    #[test]
    fn maske_des_clients() {
        assert_eq!(CLIENT_ZUGRIFF, GENERIC_READ | FILE_WRITE_DATA);
        assert_eq!(CLIENT_ZUGRIFF & GENERIC_WRITE, 0, "nie GENERIC_WRITE");
        assert_eq!(
            CLIENT_ZUGRIFF & FILE_CREATE_PIPE_INSTANCE,
            0,
            "nie FILE_CREATE_PIPE_INSTANCE"
        );
        assert_eq!(
            CLIENT_FLAGS & (SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION),
            SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION
        );
    }
}

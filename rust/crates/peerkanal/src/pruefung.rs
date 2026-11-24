//! Steps 4 to 6 of "Annehmen und Messen" as pure decisions over the measured values. The order of the reasons inside each
//! step is the order of the contract table.

use crate::fehler::Grund;
use crate::vorgabe::Erlaubnis;

/// The identity read from the impersonated token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// `TokenUser` as text.
    pub sid: String,
    /// `TokenAppContainerSid` as text, `None` outside a container.
    pub appcontainer_sid: Option<String>,
    /// `IsTokenRestricted`.
    pub eingeschraenkt: bool,
}

/// Step 4, after the token was read: `pk.prozess_nicht_offen` when the process duty holds and the process could not be
/// bound; `pk.job_unbekannt` when a job is set, the process is bound and `IsProcessInJob` gave no answer. A failed token
/// (`pk.messung_gescheitert`) is decided by the caller before, because without a token there is no peer.
pub(crate) fn stufe_messung(
    prozess_pflicht: bool,
    gebunden: bool,
    job_gesetzt: bool,
    im_job: Option<bool>,
) -> Option<Grund> {
    if prozess_pflicht && !gebunden {
        return Some(Grund::ProzessNichtOffen);
    }
    if job_gesetzt && gebunden && im_job.is_none() {
        return Some(Grund::JobUnbekannt);
    }
    None
}

/// Step 5 (Windows): the bound process has ended (`GetExitCodeProcess` not `STILL_ACTIVE`, or no answer), or it started after
/// the moment of the connection. Without a bound process the step is skipped.
pub(crate) fn stufe_prozess(gebunden: Option<(bool, u64)>, verbindungszeit: u64) -> Option<Grund> {
    let (lebt, startzeit) = gebunden?;
    if !lebt {
        return Some(Grund::ProzessBeendet);
    }
    if startzeit > verbindungszeit {
        return Some(Grund::StartzeitWeichtAb);
    }
    None
}

/// Step 6 (Windows): restricting SIDs first, then a permission with this SID, then the exact pair. A process in a container
/// never counts as the person without a container.
pub(crate) fn stufe_kennung_windows(erlaubt: &[Erlaubnis], token: &Token) -> Option<Grund> {
    if token.eingeschraenkt {
        return Some(Grund::Eingeschraenkt);
    }
    let mut sid_bekannt = false;
    for a in erlaubt {
        if let Erlaubnis::Windows {
            sid,
            appcontainer_sid,
        } = a
        {
            if sid.als_str() != token.sid {
                continue;
            }
            sid_bekannt = true;
            if appcontainer_sid
                .as_ref()
                .map(crate::kennung::PackageSid::als_str)
                == token.appcontainer_sid.as_deref()
            {
                return None;
            }
        }
    }
    Some(if sid_bekannt {
        Grund::AppcontainerFalsch
    } else {
        Grund::SidNichtErlaubt
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kennung::{PackageSid, Sid};

    const PERSON: &str = "S-1-5-21-1000000001-1000000002-1000000003-1001";
    const FREMD: &str = "S-1-5-21-1000000001-1000000002-1000000003-1003";
    const A: &str =
        "S-1-15-2-1000000021-1000000022-1000000023-1000000024-1000000025-1000000026-1000000027";
    const B: &str =
        "S-1-15-2-1000000031-1000000032-1000000033-1000000034-1000000035-1000000036-1000000037";

    fn w(sid: &str, ac: Option<&str>) -> Erlaubnis {
        Erlaubnis::Windows {
            sid: Sid::neu(sid).expect("form"),
            appcontainer_sid: ac.map(|p| PackageSid::neu(p).expect("form")),
        }
    }

    fn t(sid: &str, ac: Option<&str>, eingeschraenkt: bool) -> Token {
        Token {
            sid: sid.to_owned(),
            appcontainer_sid: ac.map(str::to_owned),
            eingeschraenkt,
        }
    }

    #[test]
    fn paar_gilt_genau() {
        let nur_person = [w(PERSON, None)];
        let nur_a = [w(PERSON, Some(A))];
        assert_eq!(
            stufe_kennung_windows(&nur_person, &t(PERSON, None, false)),
            None
        );
        // n-container-ohne-eintrag-windows: the container never counts as the person.
        assert_eq!(
            stufe_kennung_windows(&nur_person, &t(PERSON, Some(A), false)),
            Some(Grund::AppcontainerFalsch)
        );
        assert_eq!(
            stufe_kennung_windows(&nur_a, &t(PERSON, Some(A), false)),
            None
        );
        assert_eq!(
            stufe_kennung_windows(&nur_a, &t(PERSON, Some(B), false)),
            Some(Grund::AppcontainerFalsch)
        );
        assert_eq!(
            stufe_kennung_windows(&nur_a, &t(PERSON, None, false)),
            Some(Grund::AppcontainerFalsch)
        );
        assert_eq!(
            stufe_kennung_windows(&nur_person, &t(FREMD, None, false)),
            Some(Grund::SidNichtErlaubt)
        );
        assert_eq!(
            stufe_kennung_windows(&nur_a, &t(FREMD, Some(A), false)),
            Some(Grund::SidNichtErlaubt)
        );
        let beide = [w(PERSON, None), w(PERSON, Some(A))];
        assert_eq!(
            stufe_kennung_windows(&beide, &t(PERSON, Some(A), false)),
            None
        );
        assert_eq!(stufe_kennung_windows(&beide, &t(PERSON, None, false)), None);
    }

    #[test]
    fn eingeschraenkt_zuerst() {
        let liste = [w(PERSON, None)];
        assert_eq!(
            stufe_kennung_windows(&liste, &t(PERSON, None, true)),
            Some(Grund::Eingeschraenkt)
        );
        assert_eq!(
            stufe_kennung_windows(&liste, &t(FREMD, None, true)),
            Some(Grund::Eingeschraenkt)
        );
    }

    #[test]
    fn prozess_stufe() {
        assert_eq!(stufe_prozess(None, 100), None);
        assert_eq!(stufe_prozess(Some((true, 100)), 100), None);
        assert_eq!(stufe_prozess(Some((true, 99)), 100), None);
        assert_eq!(
            stufe_prozess(Some((true, 101)), 100),
            Some(Grund::StartzeitWeichtAb)
        );
        assert_eq!(
            stufe_prozess(Some((false, 101)), 100),
            Some(Grund::ProzessBeendet)
        );
        assert_eq!(
            stufe_prozess(Some((false, 1)), 100),
            Some(Grund::ProzessBeendet)
        );
    }

    #[test]
    fn messung_stufe() {
        assert_eq!(stufe_messung(false, false, false, None), None);
        assert_eq!(
            stufe_messung(true, false, false, None),
            Some(Grund::ProzessNichtOffen)
        );
        assert_eq!(
            stufe_messung(true, false, true, None),
            Some(Grund::ProzessNichtOffen)
        );
        assert_eq!(
            stufe_messung(false, true, true, None),
            Some(Grund::JobUnbekannt)
        );
        assert_eq!(
            stufe_messung(false, false, true, None),
            None,
            "ohne Prozess ist im_job null"
        );
        assert_eq!(
            stufe_messung(true, true, true, Some(false)),
            None,
            "im_job false weist nicht ab"
        );
    }
}

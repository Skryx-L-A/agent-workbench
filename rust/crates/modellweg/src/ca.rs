//! The throw-away CA of a session of mode 2 (contract 8, sections 12 and 13.5): the start check (`mw.ca_ungueltig`) and
//! the leaf certificates per host, made in the own memory and valid at most as long as the CA. Linux only (OpenSSL).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use openssl::asn1::{Asn1Integer, Asn1Time};
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{
    BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
};
use openssl::x509::{X509, X509NameBuilder};
use rustix::fs::{Mode, OFlags};

use crate::der;
use crate::schluessel::datei_ok;

/// Largest CA file in bytes.
const DATEI_MAX: u64 = 65_536;
/// Longest allowed lifetime of the CA in seconds (24 h).
const LAUFZEIT_MAX: i64 = 86_400;

/// A leaf certificate with its key.
pub type Blatt = Arc<(X509, PKey<Private>)>;

/// A checked CA with its leaf certificates.
pub struct Ca {
    schluessel: PKey<Private>,
    zertifikat: X509,
    blaetter: Mutex<BTreeMap<String, Blatt>>,
}

/// Reads a session file: own, 0600, no link, at most 64 KiB.
pub(crate) fn lies_datei(pfad: &Path, eigene_uid: u32) -> Option<Vec<u8>> {
    let flags = OFlags::NOFOLLOW
        .union(OFlags::CLOEXEC)
        .union(OFlags::NONBLOCK);
    let datei = File::from(rustix::fs::open(pfad, flags, Mode::empty()).ok()?);
    let meta = datei.metadata().ok()?;
    if !datei_ok(meta.is_file(), meta.uid(), meta.mode(), eigene_uid) {
        return None;
    }
    let mut inhalt = Vec::new();
    datei.take(DATEI_MAX + 1).read_to_end(&mut inhalt).ok()?;
    (u64::try_from(inhalt.len()).ok()? <= DATEI_MAX).then_some(inhalt)
}

/// How many PEM blocks the text has.
fn bloecke(pem: &[u8]) -> usize {
    pem.windows(11).filter(|w| *w == b"-----BEGIN ").count()
}

impl Ca {
    /// The start check: both files own, 0600, no link; exactly one key and one certificate that belong together;
    /// `CA:TRUE` with path length 0; name constraints with permitted DNS names exactly `hosts` and excluded IP ranges
    /// exactly `0.0.0.0/0` and `::/0`; a lifetime of at most 24 h that has begun and not ended. `None` is
    /// `mw.ca_ungueltig`.
    #[must_use]
    pub fn pruefe(
        schluessel: &Path,
        zertifikat: &Path,
        eigene_uid: u32,
        hosts: &[&str],
    ) -> Option<Self> {
        let k = lies_datei(schluessel, eigene_uid)?;
        let z = lies_datei(zertifikat, eigene_uid)?;
        if bloecke(&k) != 1 || bloecke(&z) != 1 {
            return None;
        }
        let schluessel = PKey::private_key_from_pem(&k).ok()?;
        let zertifikat = X509::from_pem(&z).ok()?;
        let passt = zertifikat
            .public_key()
            .is_ok_and(|p| p.public_eq(&schluessel));
        let der = zertifikat.to_der().ok()?;
        let basis =
            der::erweiterung(&der, &der::BASIC_CONSTRAINTS).is_some_and(der::ca_pfadlaenge_null);
        let grenzen = der::erweiterung(&der, &der::NAME_CONSTRAINTS)
            .and_then(der::namensgrenzen)
            .is_some_and(|(erlaubt, aus)| grenzen_gut(&erlaubt, &aus, hosts));
        let laufzeit = laufzeit_gut(&zertifikat)?;
        (passt && basis && grenzen && laufzeit).then(|| Self {
            schluessel,
            zertifikat,
            blaetter: Mutex::new(BTreeMap::new()),
        })
    }

    /// The leaf certificate and key for `host` (made once per host), signed by the CA, valid until the CA ends.
    ///
    /// # Errors
    /// OpenSSL fails to make the leaf.
    pub fn blatt(&self, host: &str) -> Result<Blatt, openssl::error::ErrorStack> {
        let mut blaetter = self.blaetter.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(b) = blaetter.get(host) {
            return Ok(Arc::clone(b));
        }
        let b = Arc::new(self.neues_blatt(host)?);
        blaetter.insert(host.to_owned(), Arc::clone(&b));
        Ok(b)
    }

    fn neues_blatt(&self, host: &str) -> Result<(X509, PKey<Private>), openssl::error::ErrorStack> {
        let gruppe = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
        let schluessel = PKey::from_ec_key(EcKey::generate(&gruppe)?)?;
        let mut name = X509NameBuilder::new()?;
        name.append_entry_by_text("CN", host)?;
        let name = name.build();
        let mut b = X509::builder()?;
        b.set_version(2)?;
        let mut zufall = BigNum::new()?;
        zufall.rand(127, MsbOption::MAYBE_ZERO, false)?;
        let serial: Asn1Integer = zufall.to_asn1_integer()?;
        b.set_serial_number(&serial)?;
        b.set_subject_name(&name)?;
        b.set_issuer_name(self.zertifikat.subject_name())?;
        b.set_pubkey(&schluessel)?;
        let jetzt = Asn1Time::days_from_now(0)?;
        b.set_not_before(&jetzt)?;
        b.set_not_after(self.zertifikat.not_after())?;
        b.append_extension(BasicConstraints::new().critical().build()?)?;
        b.append_extension(KeyUsage::new().critical().digital_signature().build()?)?;
        b.append_extension(ExtendedKeyUsage::new().server_auth().build()?)?;
        let san = SubjectAlternativeName::new()
            .dns(host)
            .build(&b.x509v3_context(Some(&self.zertifikat), None))?;
        b.append_extension(san)?;
        b.sign(&self.schluessel, MessageDigest::sha256())?;
        Ok((b.build(), schluessel))
    }

    /// The certificate of the CA (the chain the leaf is sent with).
    #[must_use]
    pub fn zertifikat(&self) -> &X509 {
        &self.zertifikat
    }
}

/// Permitted: only DNS names, exactly the set `hosts`. Excluded: exactly the IP ranges `0.0.0.0/0` and `::/0`.
fn grenzen_gut(
    erlaubt: &[(u8, Vec<u8>)],
    ausgeschlossen: &[(u8, Vec<u8>)],
    hosts: &[&str],
) -> bool {
    let mut namen: Vec<&[u8]> = Vec::new();
    for (tag, wert) in erlaubt {
        if *tag != 0x82 {
            return false;
        }
        namen.push(wert);
    }
    namen.sort_unstable();
    let mut soll: Vec<&[u8]> = hosts.iter().map(|h| h.as_bytes()).collect();
    soll.sort_unstable();
    soll.dedup();
    let mut aus: Vec<&(u8, Vec<u8>)> = ausgeschlossen.iter().collect();
    aus.sort();
    let v4 = (0x87, vec![0; 8]);
    let v6 = (0x87, vec![0; 32]);
    namen == soll && aus == [&v4, &v6]
}

/// The lifetime has begun, has not ended and is at most 24 h. `None` if OpenSSL cannot compare the times.
fn laufzeit_gut(z: &X509) -> Option<bool> {
    let jetzt = Asn1Time::days_from_now(0).ok()?;
    let begonnen = z.not_before().diff(&jetzt).ok()?;
    let bis_ende = jetzt.diff(z.not_after()).ok()?;
    let ganz = z.not_before().diff(z.not_after()).ok()?;
    let sekunden = |d: openssl::asn1::TimeDiff| i64::from(d.days) * 86_400 + i64::from(d.secs);
    Some(sekunden(begonnen) >= 0 && sekunden(bis_ende) > 0 && sekunden(ganz) <= LAUFZEIT_MAX)
}

#[cfg(test)]
mod tests {
    use super::{bloecke, grenzen_gut, laufzeit_gut, lies_datei};
    use openssl::asn1::Asn1Time;
    use openssl::x509::X509;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_session_file_has_at_most_65536_bytes() {
        let p = std::env::temp_dir().join(format!("mw-unit-ca-{}", std::process::id()));
        let uid = crate::os::eigene_uid();
        for (n, gut) in [(65_536, true), (65_537, false)] {
            std::fs::write(&p, vec![b' '; n]).expect("write");
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
            assert_eq!(
                lies_datei(&p, uid).map(|v| v.len()),
                gut.then_some(n),
                "{n}"
            );
        }
        std::fs::remove_file(&p).expect("remove");
    }

    #[test]
    fn a_lifetime_that_ends_now_has_ended() {
        let jetzt = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_secs(),
        )
        .expect("secs");
        let mut b = X509::builder().expect("builder");
        b.set_not_before(&Asn1Time::from_unix(jetzt - 60).expect("time"))
            .expect("before");
        b.set_not_after(&Asn1Time::from_unix(jetzt).expect("time"))
            .expect("after");
        assert_eq!(laufzeit_gut(&b.build()), Some(false));
        let mut b = X509::builder().expect("builder");
        b.set_not_before(&Asn1Time::from_unix(jetzt + 600).expect("time"))
            .expect("before");
        b.set_not_after(&Asn1Time::from_unix(jetzt + 1200).expect("time"))
            .expect("after");
        assert_eq!(laufzeit_gut(&b.build()), Some(false), "not yet begun");
        let mut b = X509::builder().expect("builder");
        b.set_not_before(&Asn1Time::from_unix(jetzt - 60).expect("time"))
            .expect("before");
        b.set_not_after(&Asn1Time::from_unix(jetzt - 60 + 86_400).expect("time"))
            .expect("after");
        assert_eq!(laufzeit_gut(&b.build()), Some(true), "exactly 24 h");
        let mut b = X509::builder().expect("builder");
        b.set_not_before(&Asn1Time::from_unix(jetzt - 60).expect("time"))
            .expect("before");
        b.set_not_after(&Asn1Time::from_unix(jetzt - 59 + 86_400).expect("time"))
            .expect("after");
        assert_eq!(laufzeit_gut(&b.build()), Some(false), "24 h and one second");
    }

    #[test]
    fn exactly_the_hosts_and_the_two_ip_ranges() {
        let v4 = (0x87, vec![0; 8]);
        let v6 = (0x87, vec![0; 32]);
        let dns = |h: &str| (0x82, h.as_bytes().to_vec());
        let aus = [v6.clone(), v4.clone()];
        assert!(grenzen_gut(
            &[dns("b.c"), dns("a.b")],
            &aus,
            &["a.b", "b.c", "a.b"]
        ));
        assert!(!grenzen_gut(&[dns("a.b")], &aus, &["a.b", "b.c"]));
        assert!(!grenzen_gut(&[dns("a.b"), dns("x.y")], &aus, &["a.b"]));
        assert!(!grenzen_gut(
            &[dns("a.b"), (0x87, vec![0; 8])],
            &aus,
            &["a.b"]
        ));
        assert!(!grenzen_gut(
            &[dns("a.b")],
            std::slice::from_ref(&v4),
            &["a.b"]
        ));
        assert!(!grenzen_gut(
            &[dns("a.b")],
            &[v4, (0x87, vec![1; 32])],
            &["a.b"]
        ));
        assert!(!grenzen_gut(&[dns("a.b")], &[], &["a.b"]));
    }

    #[test]
    fn pem_blocks_are_counted() {
        assert_eq!(bloecke(b"-----BEGIN A-----\n-----BEGIN B-----"), 2);
        assert_eq!(bloecke(b"x"), 0);
    }
}

//! The throw-away CA of a cloud session (contract 1 "Wegwerf-CA", contract 8 section 12): EC P-256, `CA:TRUE` with
//! `pathlen` 0, name constraints with a permitted DNS name for exactly every host of the target list and the excluded
//! IP ranges `0.0.0.0/0` and `::/0`, a lifetime of at most 24 h that has begun. Its key stays outside the enclosure.
//! Linux only (OpenSSL, as mode 2 of the model path).

use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{BasicConstraints, KeyUsage, SubjectKeyIdentifier};
use openssl::x509::{X509, X509Extension, X509NameBuilder};

/// The lifetime of the CA in seconds: 23 h, below the limit of 24 h of contract 8.
pub const LAUFZEIT: i64 = 23 * 3600;
/// How far `notBefore` lies before the moment of the start (clock steps of the host).
pub const VORLAUF: i64 = 60;

/// A CA as PEM: the private key (PKCS#8) and the certificate.
pub struct Ca {
    /// The key, PEM.
    pub schluessel: Vec<u8>,
    /// The certificate, PEM.
    pub zertifikat: Vec<u8>,
}

/// The value of the extension `nameConstraints` in the text form of OpenSSL.
#[must_use]
pub fn grenzen_text(hosts: &[&str]) -> String {
    let mut wert = String::from("critical");
    for h in hosts {
        wert.push_str(",permitted;DNS:");
        wert.push_str(h);
    }
    wert.push_str(",excluded;IP:0.0.0.0/0.0.0.0,excluded;IP:::/::");
    wert
}

/// Makes the CA for `hosts`, valid from `jetzt - VORLAUF` for [`LAUFZEIT`] seconds.
///
/// # Errors
/// OpenSSL fails.
pub fn erzeugen(hosts: &[&str], sitzung: &str, jetzt: i64) -> Result<Ca, ErrorStack> {
    let gruppe = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    let schluessel: PKey<Private> = PKey::from_ec_key(EcKey::generate(&gruppe)?)?;
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_text("CN", &format!("werkbank sitzung {sitzung}"))?;
    let name = name.build();
    let mut b = X509::builder()?;
    b.set_version(2)?;
    let mut zufall = BigNum::new()?;
    zufall.rand(127, MsbOption::MAYBE_ZERO, false)?;
    let serial = zufall.to_asn1_integer()?;
    b.set_serial_number(&serial)?;
    b.set_subject_name(&name)?;
    b.set_issuer_name(&name)?;
    b.set_pubkey(&schluessel)?;
    let vor = Asn1Time::from_unix(jetzt - VORLAUF)?;
    let nach = Asn1Time::from_unix(jetzt - VORLAUF + LAUFZEIT)?;
    b.set_not_before(&vor)?;
    b.set_not_after(&nach)?;
    b.append_extension(BasicConstraints::new().critical().ca().pathlen(0).build()?)?;
    b.append_extension(
        KeyUsage::new()
            .critical()
            .key_cert_sign()
            .crl_sign()
            .build()?,
    )?;
    let ski = SubjectKeyIdentifier::new().build(&b.x509v3_context(None, None))?;
    b.append_extension(ski)?;
    #[allow(deprecated)]
    let grenzen = X509Extension::new_nid(
        None,
        Some(&b.x509v3_context(None, None)),
        Nid::NAME_CONSTRAINTS,
        &grenzen_text(hosts),
    )?;
    b.append_extension(grenzen)?;
    b.sign(&schluessel, MessageDigest::sha256())?;
    Ok(Ca {
        schluessel: schluessel.private_key_to_pem_pkcs8()?,
        zertifikat: b.build().to_pem()?,
    })
}

#[cfg(test)]
mod tests;

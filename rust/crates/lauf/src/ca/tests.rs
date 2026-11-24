//! Tests of the throw-away CA (G2): the model path accepts it with its own start check, a leaf for a bound host passes
//! the verification, a leaf for a host outside the list is refused (name constraints), and the lifetime is at most 24 h.

use std::os::unix::fs::PermissionsExt;

use openssl::stack::Stack;
use openssl::x509::extension::SubjectAlternativeName;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::{X509StoreContext, X509VerifyResult};

use super::*;

const HOSTS: [&str; 2] = ["api.cloud.example", "telemetry.cloud.example"];

fn jetzt() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs(),
    )
    .expect("secs")
}

/// A leaf for `host`, signed by the CA (as the model path makes it).
fn blatt(ca: &Ca, host: &str) -> X509 {
    let ca_schluessel = PKey::private_key_from_pem(&ca.schluessel).expect("key");
    let ca_zert = X509::from_pem(&ca.zertifikat).expect("cert");
    let k = PKey::from_ec_key(
        EcKey::generate(&EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("group"))
            .expect("key"),
    )
    .expect("pkey");
    let mut name = X509NameBuilder::new().expect("name");
    name.append_entry_by_text("CN", host).expect("cn");
    let name = name.build();
    let mut b = X509::builder().expect("builder");
    b.set_version(2).expect("version");
    b.set_serial_number(
        &BigNum::from_u32(7)
            .expect("bn")
            .to_asn1_integer()
            .expect("serial"),
    )
    .expect("serial");
    b.set_subject_name(&name).expect("subject");
    b.set_issuer_name(ca_zert.subject_name()).expect("issuer");
    b.set_pubkey(&k).expect("pubkey");
    b.set_not_before(&Asn1Time::days_from_now(0).expect("t"))
        .expect("before");
    b.set_not_after(ca_zert.not_after()).expect("after");
    let san = SubjectAlternativeName::new()
        .dns(host)
        .build(&b.x509v3_context(Some(&ca_zert), None))
        .expect("san");
    b.append_extension(san).expect("ext");
    b.sign(&ca_schluessel, MessageDigest::sha256())
        .expect("sign");
    b.build()
}

fn pruefen(ca: &Ca, blatt: &X509) -> X509VerifyResult {
    let mut s = X509StoreBuilder::new().expect("store");
    s.add_cert(X509::from_pem(&ca.zertifikat).expect("cert"))
        .expect("add");
    let s = s.build();
    let kette = Stack::new().expect("stack");
    let mut ctx = X509StoreContext::new().expect("ctx");
    ctx.init(&s, blatt, &kette, |c| {
        c.verify_cert()?;
        Ok(c.error())
    })
    .expect("verify")
}

#[test]
fn a_leaf_outside_the_list_is_refused_by_the_name_constraints() {
    let ca = erzeugen(&HOSTS, "s-3", jetzt()).expect("ca");
    for host in HOSTS {
        assert_eq!(
            pruefen(&ca, &blatt(&ca, host)),
            X509VerifyResult::OK,
            "{host}"
        );
    }
    for fremd in [
        "github.com",
        "api.cloud.example.evil",
        "cloud.example",
        "xapi.cloud.example",
    ] {
        let r = pruefen(&ca, &blatt(&ca, fremd));
        assert_ne!(r, X509VerifyResult::OK, "{fremd}");
        assert_eq!(
            r.as_raw(),
            47,
            "{fremd}: X509_V_ERR_PERMITTED_VIOLATION, got {}",
            r.error_string()
        );
    }
    // RFC 5280, 4.2.1.10: a permitted dNSName also covers every name below it. A leaf for a subdomain of a host of the
    // list passes; the model path issues leaves only for the exact hosts, and the key of the CA never enters the
    // enclosure. Recorded here so that a change of this behaviour is seen.
    assert_eq!(
        pruefen(&ca, &blatt(&ca, "unter.api.cloud.example")),
        X509VerifyResult::OK
    );
}

#[test]
fn the_lifetime_is_at_most_24_hours_and_has_begun() {
    let t = jetzt();
    let ca = erzeugen(&HOSTS, "s-3", t).expect("ca");
    let z = X509::from_pem(&ca.zertifikat).expect("cert");
    let ganz = z.not_before().diff(z.not_after()).expect("diff");
    let sekunden = i64::from(ganz.days) * 86_400 + i64::from(ganz.secs);
    assert_eq!(sekunden, LAUFZEIT);
    assert_eq!(
        LAUFZEIT, 82_800,
        "23 hours: below the limit of 24 h, long enough for a working day"
    );
    assert!(sekunden <= 24 * 3600);
    assert!(z.not_before() <= Asn1Time::from_unix(t).expect("now"));
}

#[test]
fn the_name_constraints_text_lists_every_host_and_excludes_every_address() {
    assert_eq!(
        grenzen_text(&["a.example", "b.example"]),
        "critical,permitted;DNS:a.example,permitted;DNS:b.example,excluded;IP:0.0.0.0/0.0.0.0,excluded;IP:::/::"
    );
}

/// The start check of the model path (contract 8, section 13.5) takes the CA for exactly this list and refuses it for
/// a list with one host more or less.
#[test]
fn the_model_path_accepts_the_ca_for_exactly_its_hosts() {
    let ca = erzeugen(&HOSTS, "s-3", jetzt()).expect("ca");
    let ordner = std::env::temp_dir().join(format!("lauf-ca-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("folder");
    let k = ordner.join("ca.key");
    let z = ordner.join("ca.crt");
    for (p, inhalt) in [(&k, &ca.schluessel), (&z, &ca.zertifikat)] {
        std::fs::write(p, inhalt).expect("write");
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }
    let uid = rustix::process::getuid().as_raw();
    assert!(modellweg::ca::Ca::pruefe(&k, &z, uid, &HOSTS).is_some());
    assert!(modellweg::ca::Ca::pruefe(&k, &z, uid, &HOSTS[..1]).is_none());
    assert!(
        modellweg::ca::Ca::pruefe(
            &k,
            &z,
            uid,
            &["api.cloud.example", "telemetry.cloud.example", "x.example"]
        )
        .is_none()
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

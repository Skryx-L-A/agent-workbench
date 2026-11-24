//! A small DER reader for the two extensions the start check of mode 2 needs from the certificate of the throw-away CA
//! (contract 8, section 13.5): `basicConstraints` and `nameConstraints`. The `openssl` crate gives neither of them.

/// One TLV: tag and content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tlv<'a> {
    pub(crate) tag: u8,
    pub(crate) inhalt: &'a [u8],
}

/// Reads one TLV from the start of `b` (definite lengths up to four bytes); returns it and the rest.
pub(crate) fn tlv(b: &[u8]) -> Option<(Tlv<'_>, &[u8])> {
    let (&tag, rest) = b.split_first()?;
    let (&erste, rest) = rest.split_first()?;
    let (laenge, rest) = if erste < 0x80 {
        (usize::from(erste), rest)
    } else {
        let n = usize::from(erste & 0x7f);
        if !(1..=4).contains(&n) || rest.len() < n {
            return None;
        }
        let (bytes, rest) = rest.split_at(n);
        let laenge = bytes.iter().fold(0usize, |a, b| a * 256 + usize::from(*b));
        (laenge, rest)
    };
    if rest.len() < laenge {
        return None;
    }
    let (inhalt, rest) = rest.split_at(laenge);
    Some((Tlv { tag, inhalt }, rest))
}

/// All TLVs of a content, or `None` if one is broken.
pub(crate) fn folge(mut b: &[u8]) -> Option<Vec<Tlv<'_>>> {
    let mut v = Vec::new();
    while !b.is_empty() {
        let (t, rest) = tlv(b)?;
        v.push(t);
        b = rest;
    }
    Some(v)
}

/// OID 2.5.29.19 (`basicConstraints`), content bytes.
pub(crate) const BASIC_CONSTRAINTS: [u8; 3] = [0x55, 0x1d, 0x13];
/// OID 2.5.29.30 (`nameConstraints`), content bytes.
pub(crate) const NAME_CONSTRAINTS: [u8; 3] = [0x55, 0x1d, 0x1e];

/// The value (inside the OCTET STRING) of the extension `oid` of a DER certificate.
pub(crate) fn erweiterung<'a>(zertifikat: &'a [u8], oid: &[u8]) -> Option<&'a [u8]> {
    let (cert, _) = tlv(zertifikat)?;
    let (tbs, _) = tlv(cert.inhalt)?;
    let erweiterungen = folge(tbs.inhalt)?.into_iter().find(|t| t.tag == 0xa3)?;
    let (liste, _) = tlv(erweiterungen.inhalt)?;
    for e in folge(liste.inhalt)? {
        let teile = folge(e.inhalt)?;
        if teile
            .first()
            .is_some_and(|t| t.tag == 0x06 && t.inhalt == oid)
        {
            return teile.iter().rev().find(|t| t.tag == 0x04).map(|t| t.inhalt);
        }
    }
    None
}

/// `basicConstraints` with `cA` true and `pathLenConstraint` 0.
pub(crate) fn ca_pfadlaenge_null(wert: &[u8]) -> bool {
    let Some((seq, rest)) = tlv(wert) else {
        return false;
    };
    let Some(teile) = folge(seq.inhalt) else {
        return false;
    };
    rest.is_empty()
        && seq.tag == 0x30
        && teile.len() == 2
        && teile[0]
            == (Tlv {
                tag: 0x01,
                inhalt: &[0xff],
            })
        && teile[1]
            == (Tlv {
                tag: 0x02,
                inhalt: &[0x00],
            })
}

/// General names as (tag, bytes).
pub(crate) type Basen = Vec<(u8, Vec<u8>)>;

/// The bases of `nameConstraints`: permitted and excluded, each as (tag of the general name, bytes).
pub(crate) fn namensgrenzen(wert: &[u8]) -> Option<(Basen, Basen)> {
    let (seq, rest) = tlv(wert)?;
    if !rest.is_empty() || seq.tag != 0x30 {
        return None;
    }
    let mut erlaubt = Vec::new();
    let mut ausgeschlossen = Vec::new();
    for teil in folge(seq.inhalt)? {
        let ziel = match teil.tag {
            0xa0 => &mut erlaubt,
            0xa1 => &mut ausgeschlossen,
            _ => return None,
        };
        for baum in folge(teil.inhalt)? {
            let teile = folge(baum.inhalt)?;
            if baum.tag != 0x30 || teile.len() != 1 {
                return None;
            }
            ziel.push((teile[0].tag, teile[0].inhalt.to_vec()));
        }
    }
    Some((erlaubt, ausgeschlossen))
}

#[cfg(test)]
mod tests {
    use super::{ca_pfadlaenge_null, folge, namensgrenzen, tlv};

    #[test]
    fn lengths_short_and_long() {
        assert_eq!(
            tlv(&[0x04, 0x01, 0xaa, 0xbb]).map(|(t, r)| (t.inhalt.to_vec(), r.to_vec())),
            Some((vec![0xaa], vec![0xbb]))
        );
        let mut lang = vec![0x04, 0x81, 0x80];
        lang.extend(vec![7; 128]);
        assert_eq!(tlv(&lang).map(|(t, _)| t.inhalt.len()), Some(128));
        let mut zwei = vec![0x04, 0x82, 0x01, 0x00];
        zwei.extend(vec![7; 256]);
        assert_eq!(tlv(&zwei).map(|(t, _)| t.inhalt.len()), Some(256));
        assert!(tlv(&[0x04, 0x85, 1, 1, 1, 1, 1]).is_none());
        let mut achtzig = vec![0x04, 0x80];
        achtzig.extend(vec![7; 128]);
        assert!(tlv(&achtzig).is_none(), "0x80 is no short length");
        assert_eq!(
            tlv(&[0x04, 0x81, 0x00]).map(|(t, r)| (t.inhalt.len(), r.len())),
            Some((0, 0))
        );
        let mut drei = vec![0x04, 0x83, 0x01, 0x02, 0x03];
        drei.extend(vec![7; 0x01_0203]);
        assert_eq!(tlv(&drei).map(|(t, _)| t.inhalt.len()), Some(0x01_0203));
        assert!(tlv(&[0x04, 0x80]).is_none());
        assert!(tlv(&[0x04, 0x82, 0x01]).is_none());
        assert!(tlv(&[0x04, 0x02, 0x01]).is_none());
        assert!(tlv(&[0x04]).is_none());
        assert_eq!(folge(&[0x05, 0x00, 0x05, 0x00]).map(|v| v.len()), Some(2));
        assert!(folge(&[0x05, 0x01]).is_none());
    }

    #[test]
    fn basic_constraints() {
        assert!(ca_pfadlaenge_null(&[
            0x30, 0x06, 0x01, 0x01, 0xff, 0x02, 0x01, 0x00
        ]));
        assert!(!ca_pfadlaenge_null(&[
            0x30, 0x06, 0x01, 0x01, 0xff, 0x02, 0x01, 0x01
        ]));
        assert!(!ca_pfadlaenge_null(&[0x30, 0x03, 0x01, 0x01, 0xff]));
        assert!(!ca_pfadlaenge_null(&[
            0x30, 0x06, 0x01, 0x01, 0x00, 0x02, 0x01, 0x00
        ]));
        assert!(!ca_pfadlaenge_null(&[
            0x31, 0x06, 0x01, 0x01, 0xff, 0x02, 0x01, 0x00
        ]));
        assert!(!ca_pfadlaenge_null(&[
            0x30, 0x06, 0x01, 0x01, 0xff, 0x02, 0x01, 0x00, 0x00
        ]));
        assert!(!ca_pfadlaenge_null(&[0x30]));
    }

    #[test]
    fn name_constraints() {
        let wert = [
            0x30, 0x0b, 0xa0, 0x04, 0x30, 0x02, 0x82, 0x00, 0xa1, 0x03, 0x30, 0x01, 0x87,
        ];
        assert_eq!(namensgrenzen(&wert[..12]), None);
        let wert = [
            0x30, 0x0c, 0xa0, 0x04, 0x30, 0x02, 0x82, 0x00, 0xa1, 0x04, 0x30, 0x02, 0x87, 0x00,
        ];
        assert_eq!(
            namensgrenzen(&wert),
            Some((vec![(0x82, vec![])], vec![(0x87, vec![])]))
        );
        assert_eq!(namensgrenzen(&[0x30, 0x02, 0xa2, 0x00]), None);
        assert_eq!(
            namensgrenzen(&[0x30, 0x06, 0xa0, 0x04, 0x31, 0x02, 0x82, 0x00]),
            None
        );
        assert_eq!(
            namensgrenzen(&[0x30, 0x08, 0xa0, 0x06, 0x30, 0x04, 0x82, 0x00, 0x82, 0x00]),
            None
        );
        assert_eq!(namensgrenzen(&[0x31, 0x00]), None);
        assert_eq!(namensgrenzen(&[0x30, 0x00, 0x00]), None);
        assert_eq!(namensgrenzen(&[0x30, 0x04, 0xa0, 0x02, 0x31, 0x00]), None);
        assert_eq!(
            namensgrenzen(&[0x30, 0x06, 0xa0, 0x04, 0x30, 0x02, 0x80, 0x00, 0x30]),
            None
        );
    }
}

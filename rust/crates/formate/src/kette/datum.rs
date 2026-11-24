//! Time as text: milliseconds since 1970 to `JJJJ-MM-TTThh:mm:ss.mmmZ`, without a date crate.

use super::{Fehler, zeit_muster};

/// 9999-12-31T23:59:59.999Z in milliseconds since 1970.
const MS_MAX: i64 = 253_402_300_799_999;
const MS_JE_TAG: i64 = 86_400_000;

/// Year, month and day of a day count since 1970-01-01 (the algorithm `civil_from_days` of Howard Hinnant).
fn buergerlich(tage: i64) -> (i64, i64, i64) {
    let z = tage + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let tag = doy - (153 * mp + 2) / 5 + 1;
    let monat = if mp < 10 { mp + 3 } else { mp - 9 };
    let jahr = yoe + era * 400 + i64::from(monat <= 2);
    (jahr, monat, tag)
}

/// `JJJJ-MM-TTThh:mm:ss.mmmZ` for 1970-01-01 up to 9999-12-31, else `zeit_ungueltig`.
///
/// # Errors
/// `zeit_ungueltig` outside of 1970-01-01T00:00:00.000Z up to 9999-12-31T23:59:59.999Z.
pub fn zeit_text(ms: i64) -> Result<String, Fehler> {
    if !(0..=MS_MAX).contains(&ms) {
        return Err(Fehler::ZeitUngueltig);
    }
    let (jahr, monat, tag) = buergerlich(ms.div_euclid(MS_JE_TAG));
    let rest = ms.rem_euclid(MS_JE_TAG);
    Ok(format!(
        "{jahr:04}-{monat:02}-{tag:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    ))
}

/// Days since 1970-01-01 of a civil date (the algorithm `days_from_civil` of Howard Hinnant).
fn tage_seit_1970(jahr: i64, monat: i64, tag: i64) -> i64 {
    let j = if monat <= 2 { jahr - 1 } else { jahr };
    let era = j.div_euclid(400);
    let yoe = j - era * 400;
    let mp = (monat + 9) % 12;
    let doy = (153 * mp + 2) / 5 + tag - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The milliseconds since 1970 of a text `JJJJ-MM-TTThh:mm:ss.mmmZ`, or `None` when the text does not follow the
/// pattern, is no date of the calendar (`2026-02-31`) or lies before 1970-01-01. The inverse of [`zeit_text`].
pub(super) fn text_zeit(text: &str) -> Option<i64> {
    if !zeit_muster(text) {
        return None;
    }
    let zahl = |von: usize, bis: usize| text.get(von..bis)?.parse::<i64>().ok();
    let tage = tage_seit_1970(zahl(0, 4)?, zahl(5, 7)?, zahl(8, 10)?);
    let ms = tage * MS_JE_TAG
        + zahl(11, 13)? * 3_600_000
        + zahl(14, 16)? * 60_000
        + zahl(17, 19)? * 1000
        + zahl(20, 23)?;
    zeit_text(ms).is_ok_and(|t| t == text).then_some(ms)
}

#[cfg(test)]
mod tests {
    use super::{Fehler, text_zeit, zeit_text};

    #[test]
    fn dates_around_leap_days_and_ends() {
        let faelle = [
            (0, "1970-01-01T00:00:00.000Z"),
            (86_399_999, "1970-01-01T23:59:59.999Z"),
            (86_400_000, "1970-01-02T00:00:00.000Z"),
            (951_782_400_000, "2000-02-29T00:00:00.000Z"),
            (951_868_800_000, "2000-03-01T00:00:00.000Z"),
            (4_107_542_400_000, "2100-03-01T00:00:00.000Z"),
            (1_709_164_800_000, "2024-02-29T00:00:00.000Z"),
            (1_735_689_599_999, "2024-12-31T23:59:59.999Z"),
            (1_735_689_600_000, "2025-01-01T00:00:00.000Z"),
            (1_804_567_330_123, "2027-03-09T04:42:10.123Z"),
            (253_402_300_799_999, "9999-12-31T23:59:59.999Z"),
        ];
        for (ms, text) in faelle {
            assert_eq!(zeit_text(ms), Ok(text.to_owned()), "ms {ms}");
        }
    }

    #[test]
    fn out_of_range_is_an_error() {
        assert_eq!(zeit_text(-1), Err(Fehler::ZeitUngueltig));
        assert_eq!(zeit_text(253_402_300_800_000), Err(Fehler::ZeitUngueltig));
        assert_eq!(zeit_text(i64::MIN), Err(Fehler::ZeitUngueltig));
    }

    #[test]
    fn text_to_milliseconds_and_back() {
        for ms in [
            0,
            86_399_999,
            951_782_400_000,
            1_709_164_800_000,
            1_795_179_600_123,
            253_402_300_799_999,
        ] {
            let text = zeit_text(ms).expect("in range");
            assert_eq!(text_zeit(&text), Some(ms), "{text}");
        }
        for falsch in [
            "",
            "2026-02-31T00:00:00.000Z",
            "2026-02-29T00:00:00.000Z",
            "2026-04-31T00:00:00.000Z",
            "1969-12-31T23:59:59.999Z",
            "0000-01-01T00:00:00.000Z",
            "2026-11-20T13:00:00Z",
            "2026-13-01T00:00:00.000Z",
        ] {
            assert_eq!(text_zeit(falsch), None, "{falsch}");
        }
        assert_eq!(
            text_zeit("2024-02-29T00:00:00.000Z"),
            Some(1_709_164_800_000)
        );
        assert_eq!(text_zeit("1970-01-01T00:00:00.000Z"), Some(0));
    }
}

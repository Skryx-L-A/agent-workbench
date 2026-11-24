//! The difference of two pictures with detection of a scroll up (contract rule RA7, model M2).
//!
//! The search follows `diff` of the measuring runner line by line; equal lines are found through small numbers
//! (one per distinct line) instead of comparing runs, which gives the same result at a fraction of the cost.

use std::collections::HashMap;

use crate::bild::Schub;
use crate::zeile::Zeile;

/// The scroll is searched only when more lines than this differ.
const SUCHE_AB_KOSTEN: usize = 3;
/// A run of equal lines shorter than this is no scroll.
const LAUF_MIN: usize = 3;
/// A scroll is taken only when its cost plus this is below the best cost so far.
const GEWINN: usize = 2;

/// The number of the empty line.
const LEER: u32 = 0;

/// The lines of both pictures as numbers: equal lines have equal numbers, the empty line has [`LEER`].
fn nummern(alt: &[Zeile], neu: &[Zeile]) -> (Vec<u32>, Vec<u32>) {
    /// Numbers the lines, giving a new number to each line the table does not know yet.
    fn vergeben<'a>(tabelle: &mut HashMap<&'a Zeile, u32>, zeilen: &'a [Zeile]) -> Vec<u32> {
        zeilen
            .iter()
            .map(|z| {
                let frei = u32::try_from(tabelle.len()).unwrap_or(u32::MAX);
                *tabelle.entry(z).or_insert(frei)
            })
            .collect()
    }
    let leer: Zeile = Vec::new();
    let mut tabelle: HashMap<&Zeile, u32> = HashMap::new();
    tabelle.insert(&leer, LEER);
    let a = vergeben(&mut tabelle, alt);
    let n = vergeben(&mut tabelle, neu);
    (a, n)
}

/// The line the model of a scroll puts at row `i`: for `i` in `a..b` the old line `i + sh`, for `i` in
/// `b..b + sh` the empty line, else the old line `i`.
fn modell(alt: &[u32], i: usize, a: usize, b: usize, sh: usize) -> u32 {
    if (a..b).contains(&i) {
        alt[i + sh]
    } else if (b..b + sh).contains(&i) {
        LEER
    } else {
        alt[i]
    }
}

/// The longest run of rows `i` in `0..n - sh` with `neu[i] == alt[i + sh]` and `neu[i]` not empty; the first of
/// several equally long ones. Returns its start and its length (0 when there is none).
fn laengster_lauf(alt: &[u32], neu: &[u32], sh: usize) -> (usize, usize) {
    let (mut beste_start, mut beste_laenge) = (0, 0);
    let (mut start, mut laenge) = (0, 0);
    for i in 0..neu.len() - sh {
        if neu[i] == alt[i + sh] && neu[i] != LEER {
            if laenge == 0 {
                start = i;
            }
            laenge += 1;
            if laenge > beste_laenge {
                beste_laenge = laenge;
                beste_start = start;
            }
        } else {
            laenge = 0;
        }
    }
    (beste_start, beste_laenge)
}

/// The difference of the picture `alt` (the model picture) to `neu`, both with the same number of lines: the
/// scroll, if one is taken, and the indexes of the lines that differ from the model after the scroll, rising.
///
/// # Panics
///
/// Does not panic when both slices have the same length; callers keep that.
#[must_use]
pub fn unterschied(alt: &[Zeile], neu: &[Zeile]) -> (Option<Schub>, Vec<usize>) {
    let (a_nr, n_nr) = nummern(alt, neu);
    let n = n_nr.len();
    let kosten0 = (0..n).filter(|&i| a_nr[i] != n_nr[i]).count();
    let mut schub: Option<Schub> = None;
    let mut beste = kosten0;
    if kosten0 > SUCHE_AB_KOSTEN {
        for sh in 1..n {
            let (a, laenge) = laengster_lauf(&a_nr, &n_nr, sh);
            if laenge < LAUF_MIN {
                continue;
            }
            let b = a + laenge;
            let kosten = (0..n)
                .filter(|&i| modell(&a_nr, i, a, b, sh) != n_nr[i])
                .count();
            if kosten + GEWINN < beste {
                beste = kosten;
                schub = Some(Schub {
                    oben: a,
                    unten: b + sh,
                    n: sh,
                });
            }
        }
    }
    let zeilen = (0..n)
        .filter(|&i| {
            let m = match schub {
                Some(s) => modell(&a_nr, i, s.oben, s.unten - s.n, s.n),
                None => a_nr[i],
            };
            m != n_nr[i]
        })
        .collect();
    (schub, zeilen)
}

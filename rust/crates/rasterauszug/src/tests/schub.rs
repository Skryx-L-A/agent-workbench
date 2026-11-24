//! The difference of two pictures with the scroll of model M2 (RA7).

use super::zeile;
use crate::{Schub, Zeile, unterschied};

/// The line `L<i>`.
fn l(i: usize) -> Zeile {
    zeile(&[(0, 0, 0, &format!("L{i}"))])
}

fn x(i: usize) -> Zeile {
    zeile(&[(0, 0, 0, &format!("X{i}"))])
}

#[test]
fn equal_pictures_have_no_difference() {
    let bild: Vec<Zeile> = (0..8).map(l).collect();
    assert_eq!(unterschied(&bild, &bild), (None, Vec::new()));
    assert_eq!(unterschied(&[], &[]), (None, Vec::new()));
}

#[test]
fn up_to_three_changed_lines_are_listed_without_a_search() {
    let alt: Vec<Zeile> = (0..10).map(l).collect();
    // The text of the old picture moved up by one, but only three lines differ: no search.
    let mut neu = alt.clone();
    neu[2] = x(2);
    neu[3] = x(3);
    neu[7] = Vec::new();
    assert_eq!(unterschied(&alt, &neu), (None, vec![2, 3, 7]));
}

#[test]
fn a_scroll_up_is_found_and_only_the_new_line_is_listed() {
    let alt: Vec<Zeile> = (0..10).map(l).collect();
    let mut neu: Vec<Zeile> = (1..10).map(l).collect();
    neu.push(x(10));
    assert_eq!(
        unterschied(&alt, &neu),
        (
            Some(Schub {
                oben: 0,
                unten: 10,
                n: 1
            }),
            vec![9]
        )
    );
    // Scroll by three inside a region with a header line and a footer.
    let mut alt: Vec<Zeile> = (0..12).map(l).collect();
    alt[0] = x(100);
    alt[11] = x(111);
    let mut neu = alt.clone();
    neu[1..8].clone_from_slice(&alt[4..11]);
    for (i, z) in neu.iter_mut().enumerate().take(11).skip(8) {
        *z = x(200 + i);
    }
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(
        schub,
        Some(Schub {
            oben: 1,
            unten: 11,
            n: 3
        })
    );
    assert_eq!(zeilen, vec![8, 9, 10]);
}

#[test]
fn a_run_of_two_lines_is_no_scroll() {
    let alt: Vec<Zeile> = (0..10).map(l).collect();
    let mut neu: Vec<Zeile> = (3..13).map(x).collect();
    neu[0] = l(1);
    neu[1] = l(2);
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(schub, None);
    assert_eq!(zeilen, (0..10).collect::<Vec<_>>());
}

#[test]
fn a_saving_of_three_lines_is_taken_and_a_saving_of_two_is_not() {
    // Three lines saved: 5 differ without the scroll, 2 with it.
    let alt: Vec<Zeile> = (0..6).map(l).collect();
    let neu = vec![l(1), l(2), l(3), x(0), x(1), l(5)];
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(
        schub,
        Some(Schub {
            oben: 0,
            unten: 4,
            n: 1
        })
    );
    assert_eq!(zeilen, vec![3, 4]);
    // Two lines saved: 4 differ without the scroll, 2 with it.
    let neu = vec![l(1), l(2), l(3), l(3), x(1), l(5)];
    assert_eq!(unterschied(&alt, &neu), (None, vec![0, 1, 2, 4]));
}

#[test]
fn of_two_runs_of_the_same_length_the_first_counts() {
    let alt: Vec<Zeile> = (0..12).map(l).collect();
    let mut neu: Vec<Zeile> = (0..12).map(|i| x(i + 50)).collect();
    neu[0..3].clone_from_slice(&alt[1..4]);
    neu[4..7].clone_from_slice(&alt[5..8]);
    let (schub, _) = unterschied(&alt, &neu);
    assert_eq!(schub.map(|s| s.oben), Some(0));
}

#[test]
fn a_longer_run_beats_an_earlier_shorter_one() {
    let alt: Vec<Zeile> = (0..14).map(l).collect();
    let mut neu: Vec<Zeile> = (0..14).map(|i| x(i + 50)).collect();
    neu[0..3].clone_from_slice(&alt[1..4]);
    neu[6..11].clone_from_slice(&alt[7..12]);
    let (schub, _) = unterschied(&alt, &neu);
    assert_eq!(schub.map(|s| s.oben), Some(6));
}

#[test]
fn empty_lines_do_not_make_a_scroll() {
    // Four lines become empty and the old picture holds four empty lines below them. Counting equal empty lines as
    // a run would make a scroll that costs nothing; the rule lists the four lines instead.
    let leer: Zeile = Vec::new();
    let alt = vec![
        l(0),
        l(1),
        l(2),
        l(3),
        leer.clone(),
        leer.clone(),
        leer.clone(),
        leer.clone(),
    ];
    let neu = vec![leer; 8];
    assert_eq!(unterschied(&alt, &neu), (None, vec![0, 1, 2, 3]));
}

#[test]
fn only_an_upward_scroll_is_found() {
    let alt: Vec<Zeile> = (0..10).map(l).collect();
    let mut neu: Vec<Zeile> = vec![x(0)];
    neu.extend((0..9).map(l));
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(schub, None);
    assert_eq!(zeilen, (0..10).collect::<Vec<_>>());
}

#[test]
fn a_later_scroll_must_beat_the_best_one_so_far() {
    // The scroll by 1 leaves 5 lines to list; the scroll by 2 would leave 11 and is no better than that, though it
    // is still better than listing all 14.
    let alt: Vec<Zeile> = (0..14).map(l).collect();
    let mut neu: Vec<Zeile> = Vec::new();
    neu.extend((1..10).map(|i| alt[i].clone()));
    neu.extend((11..14).map(|i| alt[i].clone()));
    neu.push(x(12));
    neu.push(x(13));
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(
        schub,
        Some(Schub {
            oben: 0,
            unten: 10,
            n: 1
        })
    );
    assert_eq!(zeilen, vec![9, 10, 11, 12, 13]);
}

#[test]
fn lines_of_different_style_are_different_lines() {
    let alt: Vec<Zeile> = (0..6).map(l).collect();
    let mut neu = alt.clone();
    for z in &mut neu {
        z[0].stil.fg = 1;
    }
    let (schub, zeilen) = unterschied(&alt, &neu);
    assert_eq!(schub, None);
    assert_eq!(zeilen, (0..6).collect::<Vec<_>>());
}

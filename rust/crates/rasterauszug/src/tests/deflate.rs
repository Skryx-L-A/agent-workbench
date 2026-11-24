//! Raw Deflate with one context per connection: round trips, the context between frames, streams from another
//! compressor, the size limit and the distance check (RA11).

use flate2::{Compress, Compression, FlushCompress, Status};

use crate::deflate::{Entpacker, Verpacker};
use crate::{Fehler, RAHMEN_MAX};

const ENDE: [u8; 4] = [0, 0, 0xff, 0xff];

/// Pseudo-random bytes that do not compress.
fn zufall(n: usize, samen: u64) -> Vec<u8> {
    let mut z = samen;
    (0..n)
        .map(|_| {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            z.to_le_bytes()[3]
        })
        .collect()
}

/// A frame as another compressor makes it: the bytes of `roh`, then a sync flush, without the four end bytes.
fn fremd_verpacken(c: &mut Compress, roh: &[u8]) -> Vec<u8> {
    let mut aus: Vec<u8> = Vec::new();
    let mut ein = roh;
    loop {
        aus.reserve(65_536);
        let vor = c.total_in();
        c.compress_vec(ein, &mut aus, FlushCompress::Sync)
            .expect("compresses");
        ein = &ein[usize::try_from(c.total_in() - vor).expect("fits")..];
        if ein.is_empty() && aus.len() < aus.capacity() {
            break;
        }
    }
    assert!(aus.ends_with(&ENDE));
    aus.truncate(aus.len() - ENDE.len());
    aus
}

#[test]
fn frames_of_every_size_go_through_one_context_each_way() {
    let mut v = Verpacker::neu();
    let mut e = Entpacker::neu();
    let frames = [
        b"hello world, hello world".to_vec(),
        vec![7; 1000],
        vec![],
        zufall(70_000, 1),
        b"x".to_vec(),
        zufall(1_000_000, 2),
        vec![0; 100_000],
    ];
    for (i, f) in frames.iter().enumerate() {
        let wire = v.verpacken(f);
        assert!(!wire.ends_with(&ENDE), "frame {i}: the end bytes are cut");
        assert_eq!(e.entpacken(&wire).expect("inflates"), *f, "frame {i}");
    }
}

#[test]
fn a_repeated_frame_is_cheap_through_the_shared_context() {
    let mut v = Verpacker::neu();
    let frame = zufall(3000, 3);
    let erst = v.verpacken(&frame).len();
    let zweit = v.verpacken(&frame).len();
    assert!(erst > 2900, "random data does not shrink: {erst}");
    assert!(zweit * 4 <= erst, "second {zweit} first {erst}");
}

#[test]
fn a_second_sender_context_does_not_share_with_the_first() {
    let frame = zufall(3000, 4);
    let a = Verpacker::neu().verpacken(&frame).len();
    let b = Verpacker::neu().verpacken(&frame).len();
    assert_eq!(a, b);
}

#[test]
fn streams_of_another_compressor_inflate_at_every_level() {
    for stufe in 0..=9 {
        let mut c = Compress::new(Compression::new(stufe), false);
        let mut e = Entpacker::neu();
        let frames = [
            b"abc abc abc abc".to_vec(),
            zufall(5000, 5),
            [zufall(2000, 6), zufall(2000, 6)].concat(),
            vec![b'z'; 40_000],
        ];
        for (i, f) in frames.iter().enumerate() {
            let wire = fremd_verpacken(&mut c, f);
            assert_eq!(
                e.entpacken(&wire).expect("inflates"),
                *f,
                "level {stufe} frame {i}"
            );
        }
    }
}

#[test]
fn a_frame_may_reach_back_into_the_window_the_connection_kept() {
    let mut c = Compress::new(Compression::new(6), false);
    let mut e = Entpacker::neu();
    let erst = zufall(100_000, 7);
    // The compressor does not match at the very edge of its window, so the repeat reaches back 30 000 bytes.
    let zweit = erst[erst.len() - 30_000..].to_vec();
    let dritt = [zweit.clone(), zufall(10, 8), zweit[1000..9000].to_vec()].concat();
    for (i, f) in [erst, zweit, dritt].iter().enumerate() {
        let wire = fremd_verpacken(&mut c, f);
        if i > 0 {
            assert!(
                wire.len() * 4 < f.len(),
                "frame {i} is made of matches: {}",
                wire.len()
            );
        }
        assert_eq!(e.entpacken(&wire).expect("inflates"), *f, "frame {i}");
    }
}

#[test]
fn a_frame_may_hold_exactly_the_limit_and_not_one_byte_more() {
    let mut c = Compress::new(Compression::new(6), false);
    let wire = fremd_verpacken(&mut c, &vec![0; RAHMEN_MAX]);
    let mut e = Entpacker::neu();
    assert_eq!(
        e.entpacken(&wire).expect("the limit is allowed").len(),
        RAHMEN_MAX
    );

    let mut c = Compress::new(Compression::new(6), false);
    let zu_gross = fremd_verpacken(&mut c, &vec![0; RAHMEN_MAX + 1]);
    let mut e = Entpacker::neu();
    assert_eq!(e.entpacken(&zu_gross), Err(Fehler::RahmenZuGross));
    // The inflater is broken for good; even a valid small frame gives the same error.
    let mut klein = Compress::new(Compression::new(6), false);
    let klein_wire = fremd_verpacken(&mut klein, b"ok");
    assert_eq!(e.entpacken(&klein_wire), Err(Fehler::RahmenZuGross));

    let mut c = Compress::new(Compression::new(6), false);
    let weit_drueber = fremd_verpacken(&mut c, &vec![0; RAHMEN_MAX + 1_000_000]);
    assert_eq!(
        Entpacker::neu().entpacken(&weit_drueber),
        Err(Fehler::RahmenZuGross)
    );
}

#[test]
fn a_match_before_the_start_of_the_stream_is_an_error() {
    // Fixed block: the length 3 at distance 1 as the first symbol. zlib: "invalid distance too far back".
    let mut e = Entpacker::neu();
    assert_eq!(e.entpacken(&[0x02, 0x02, 0x00]), Err(Fehler::InflateFehler));
    assert_eq!(
        e.entpacken(&[0x4a, 0x04, 0x02, 0x00]),
        Err(Fehler::InflateFehler),
        "broken for good"
    );
    // The same match after one literal is fine.
    assert_eq!(
        Entpacker::neu().entpacken(&[0x4a, 0x04, 0x02, 0x00]),
        Ok(b"aaaa".to_vec())
    );
}

#[test]
fn garbage_is_an_error_and_an_empty_wire_gives_no_bytes() {
    assert_eq!(
        Entpacker::neu().entpacken(&[0xff, 0xff, 0xff, 0xff]),
        Err(Fehler::InflateFehler)
    );
    assert_eq!(Entpacker::neu().entpacken(&[]), Ok(Vec::new()));
}

#[test]
fn after_the_final_block_later_frames_give_no_bytes() {
    let mut c = Compress::new(Compression::new(6), false);
    let mut wire = Vec::with_capacity(100);
    let status = c
        .compress_vec(b"hello", &mut wire, FlushCompress::Finish)
        .expect("finishes");
    assert_eq!(status, Status::StreamEnd);
    let mut e = Entpacker::neu();
    assert_eq!(e.entpacken(&wire), Ok(b"hello".to_vec()));
    assert_eq!(e.entpacken(&[1, 2, 3]), Ok(Vec::new()));
    assert_eq!(e.entpacken(&wire), Ok(Vec::new()));
}

#[test]
fn the_sender_of_a_second_frame_continues_its_stream() {
    // The bytes a connection sends for the second frame only inflate behind the first.
    let mut v = Verpacker::neu();
    let a = zufall(2000, 9);
    let b = [a.clone(), a.clone()].concat();
    let wa = v.verpacken(&a);
    let wb = v.verpacken(&b);
    let mut e = Entpacker::neu();
    assert_eq!(e.entpacken(&wa), Ok(a));
    assert_eq!(e.entpacken(&wb), Ok(b));
    // A receiver that never saw the first frame cannot read the second.
    assert!(
        Entpacker::neu()
            .entpacken(&wb)
            .map_or(true, |roh| roh.len() != 4000)
    );
}

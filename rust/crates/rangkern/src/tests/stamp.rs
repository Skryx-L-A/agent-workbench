#[cfg(unix)]
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use super::TempDir;
#[cfg(unix)]
use crate::stamp::RACY_NS;
use crate::stamp::{Detect, Key, Stamp, now_ns, system_ns};

fn key() -> Key {
    Key {
        dev: 1,
        ino: 2,
        size: 3,
        mtime: 4_000_000_000_000,
        ctime: 5_000_000_000_000,
    }
}

fn stamp(key: Key, racy: bool) -> Stamp {
    Stamp { key, racy }
}

#[cfg(unix)]
fn newest_ns(meta: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    let ns = |seconds: i64, nanos: i64| i128::from(seconds) * 1_000_000_000 + i128::from(nanos);
    ns(meta.mtime(), meta.mtime_nsec()).max(ns(meta.ctime(), meta.ctime_nsec()))
}

#[test]
fn the_platform_decides_how_changes_are_found() {
    assert_eq!(
        Detect::native(),
        if cfg!(unix) {
            Detect::Metadata
        } else {
            Detect::Content
        }
    );
}

#[test]
fn a_stamp_trusts_the_metadata_only_when_it_is_settled_and_equal() {
    let settled = stamp(key(), false);
    assert!(settled.unchanged(&stamp(key(), false)));
    assert!(settled.unchanged(&stamp(key(), true)));
    assert!(!stamp(key(), true).unchanged(&stamp(key(), false)));
    assert!(!stamp(key(), true).unchanged(&stamp(key(), true)));
}

#[test]
fn every_part_of_the_key_counts() {
    let settled = stamp(key(), false);
    let others = [
        Key { dev: 9, ..key() },
        Key { ino: 9, ..key() },
        Key { size: 9, ..key() },
        Key { mtime: 9, ..key() },
        Key { ctime: 9, ..key() },
    ];
    for other in others {
        assert!(!settled.unchanged(&stamp(other, false)), "{other:?}");
    }
}

#[test]
fn the_racy_flag_is_readable() {
    assert!(stamp(key(), true).racy());
    assert!(!stamp(key(), false).racy());
}

#[cfg(unix)]
#[test]
fn a_stamp_is_racy_until_two_seconds_after_the_last_change() {
    let dir = TempDir::new();
    let path = dir.write("a.md", b"x");
    let meta = fs::metadata(&path).expect("file exists");
    let newest = newest_ns(&meta);
    let at = |now: i128| Stamp::new(&meta, now, Detect::Metadata);
    assert!(at(newest).racy());
    assert!(at(newest + RACY_NS - 1).racy());
    assert!(!at(newest + RACY_NS).racy());
    assert!(!at(newest + 10 * RACY_NS).racy());
    assert!(at(newest - 5).racy());
    assert!(Stamp::new(&meta, newest + 10 * RACY_NS, Detect::Content).racy());
    assert!(Stamp::new(&meta, i128::MAX, Detect::Content).racy());
    assert!(!Stamp::new(&meta, i128::MAX, Detect::Metadata).racy());
}

#[cfg(unix)]
#[test]
fn the_newer_of_mtime_and_ctime_decides() {
    use std::fs::{FileTimes, OpenOptions};
    let dir = TempDir::new();
    let path = dir.write("a.md", b"x");
    let old = SystemTime::now() - Duration::from_secs(100_000);
    OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("opens")
        .set_times(FileTimes::new().set_modified(old))
        .expect("mtime is set");
    let meta = fs::metadata(&path).expect("file exists");
    let ctime = newest_ns(&meta);
    assert!(ctime > system_ns(old) + RACY_NS);
    assert!(Stamp::new(&meta, ctime + RACY_NS - 1, Detect::Metadata).racy());
    assert!(!Stamp::new(&meta, ctime + RACY_NS, Detect::Metadata).racy());
}

#[cfg(unix)]
#[test]
fn a_rewrite_of_the_same_size_with_the_old_mtime_changes_the_stamp() {
    use std::fs::{FileTimes, OpenOptions};
    let dir = TempDir::new();
    let path = dir.write("a.md", b"aaaa");
    let settled = |path: &std::path::Path| {
        let meta = fs::metadata(path).expect("file exists");
        Stamp::new(&meta, i128::MAX, Detect::Metadata)
    };
    let before = settled(&path);
    assert!(before.unchanged(&settled(&path)));
    let old = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    std::thread::sleep(Duration::from_millis(30));
    fs::write(&path, b"bbbb").expect("rewritten");
    OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("opens")
        .set_times(FileTimes::new().set_modified(old))
        .expect("mtime is reset");
    let after = settled(&path);
    assert_eq!(after.key.size, before.key.size);
    assert_eq!(after.key.mtime, before.key.mtime);
    assert!(!before.unchanged(&after));
}

#[test]
fn times_count_in_nanoseconds_from_the_epoch() {
    assert_eq!(system_ns(UNIX_EPOCH), 0);
    assert_eq!(system_ns(UNIX_EPOCH + Duration::new(3, 500)), 3_000_000_500);
    // a time before the epoch keeps its nanoseconds; Windows holds a time in ticks of 100 ns, so there the part below
    // the second is a whole number of ticks
    #[cfg(not(windows))]
    assert_eq!(system_ns(UNIX_EPOCH - Duration::new(2, 7)), -2_000_000_007);
    #[cfg(windows)]
    assert_eq!(
        system_ns(UNIX_EPOCH - Duration::new(2, 700)),
        -2_000_000_700
    );
    let before = system_ns(SystemTime::now());
    let now = now_ns();
    let after = system_ns(SystemTime::now());
    assert!(before <= now && now <= after);
}

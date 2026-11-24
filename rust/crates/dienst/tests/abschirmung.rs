//! Kernabbild (step 2a), Linux: the running service shields its memory from core images. Its `/proc/<pid>/stat` no longer belongs to the user
//! (the process cannot be dumped, its `/proc` files belong to root), and `/proc/<pid>/limits` shows `Max core file size 1 1`. A child of the
//! service inherits the limit 1/1 and, after `exec`, can be dumped again (its files belong to the user): only the inherited limit protects it.
#![cfg(target_os = "linux")]

mod gemeinsam;

use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

use gemeinsam::{Prozess, Wegwerf, startdatei};
use serde_json::json;

/// The owner of `/proc/<pid>/stat`: the user of the process, or root while the process cannot be dumped.
fn eigentuemer(pid: u32) -> u32 {
    std::fs::metadata(format!("/proc/{pid}/stat"))
        .expect("proc")
        .uid()
}

fn eigener_nutzer() -> u32 {
    std::fs::metadata("/proc/self/stat").expect("proc").uid()
}

/// The soft and the hard value of the line `Max core file size` of `/proc/<pid>/limits`.
fn core_grenze(pid: u32) -> (String, String) {
    let text = std::fs::read_to_string(format!("/proc/{pid}/limits")).expect("limits");
    let zeile = text
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .expect("the line of the core limit");
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    // Max core file size  <soft>  <hard>  bytes
    (felder[4].to_owned(), felder[5].to_owned())
}

/// The process ids whose parent is `eltern`: the fourth field of `/proc/<pid>/stat` counts from behind the last `)`.
fn kinder_von(eltern: u32) -> Vec<u32> {
    let mut kinder = Vec::new();
    for eintrag in std::fs::read_dir("/proc").expect("proc").flatten() {
        let Some(pid) = eintrag
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let Some(ende) = stat.rfind(')') else {
            continue;
        };
        let elternkennung = stat[ende + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|f| f.parse::<u32>().ok());
        if elternkennung == Some(eltern) {
            kinder.push(pid);
        }
    }
    kinder
}

#[test]
fn der_laufende_dienst_ist_abgeschirmt_und_sein_kind_erbt_die_grenze_und_ist_nach_exec_abzugsfaehig()
 {
    if eigener_nutzer() == 0 {
        // as root the files of a process that cannot be dumped belong to root as before: the test cannot say anything
        return;
    }
    let w = Wegwerf::neu("abschirmung");
    let start = startdatei(&w, &["/bin/sleep", "60"], &json!({}));
    let p = Prozess::starten(&w, &start);
    let pid = p.kind.id();
    assert_ne!(
        eigentuemer(pid),
        eigener_nutzer(),
        "the files of /proc/<pid> of the service belong to root while it cannot be dumped"
    );
    assert_eq!(core_grenze(pid), ("1".to_owned(), "1".to_owned()));
    // the child: the cover `wb-dienst --exec` starts it, then it becomes `sleep`; it inherits 1/1 and can be dumped again after `exec`
    let frist = Instant::now() + Duration::from_secs(10);
    let kind = loop {
        if let Some(k) = kinder_von(pid).into_iter().find(|k| {
            std::fs::read_to_string(format!("/proc/{k}/comm")).is_ok_and(|c| c.trim() == "sleep")
        }) {
            break k;
        }
        assert!(
            Instant::now() < frist,
            "the child (sleep) of the service did not appear"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(core_grenze(kind), ("1".to_owned(), "1".to_owned()));
    assert_eq!(
        eigentuemer(kind),
        eigener_nutzer(),
        "after exec the child can be dumped again: its files belong to the user"
    );
}

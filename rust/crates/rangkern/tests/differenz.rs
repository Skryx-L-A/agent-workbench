//! Differential test of `rangkern serve` against `rangkern search`.
//!
//! A warm [`Server`] and a fresh [`search`] look at the same invented vault. After every change of a
//! random sequence of changes (notes written, rewritten, rewritten with the same size and the old
//! `mtime`, deleted, renamed, moved into excluded folders; folders made, renamed, removed; the
//! manifests of spaces and source spaces; the contradictions; the vector file written, damaged,
//! removed; permissions taken away and given back), both answer a few random questions, and the
//! answers must be equal in every field and every bit of every number. The vault is invented;
//! nothing here reads a real one.
//!
//! The runs use three clocks: the real one (every fresh file is racy and checked by its content),
//! one ten seconds ahead with a pause before each change (every stamp is trusted, so the change time
//! alone has to show each change), and the change detection by content that Windows uses.

// Permissions need Unix.
#![cfg(unix)]

use std::fs::{self, FileTimes, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use rangkern::hit::SearchHit;
use rangkern::search::{Params, search};
use rangkern::serve::{SearchRequest, Server};
use rangkern::stamp::{Detect, now_ns};
use rangkern::vault::{Vectors, parse_note_vectors};

/// A small deterministic generator (xorshift64*), so that a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("fits")).expect("fits")
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, list: &'a [T]) -> &'a T {
        &list[self.below(list.len())]
    }
}

/// A number between -1 and 1 with three digits.
fn unit(rng: &mut Rng) -> f64 {
    f64::from(i32::try_from(rng.below(2001)).expect("small")) / 1000.0 - 1.0
}

const WORDS: [&str; 48] = [
    "garten",
    "beet",
    "kaffee",
    "wasserfall",
    "bergwanderung",
    "entscheidung",
    "aufgabe",
    "häuser",
    "straße",
    "überraschung",
    "kochbuch",
    "fahrrad",
    "werkzeugkasten",
    "rechnung",
    "projekt",
    "notiz",
    "und",
    "der",
    "die",
    "das",
    "ist",
    "nicht",
    "mit",
    "für",
    "sonne",
    "regen",
    "schneeflocke",
    "lautsprecher",
    "kerzenlicht",
    "zukunft",
    "vergangenheit",
    "gärten",
    "beeten",
    "kaffeemaschine",
    "wanderweg",
    "tagebuch",
    "planung",
    "speicher",
    "suchindex",
    "ranglisten",
    "quelle",
    "original",
    "widerspruch",
    "nachfolger",
    "gültigkeit",
    "auszug",
    "titelbonus",
    "zebrawort",
];

const DIRS: [&str; 12] = [
    "",
    "",
    "a",
    "b",
    "a/sub",
    "q",
    "q/quellen",
    "p",
    "90-secrets",
    "_meta",
    ".versteckt",
    "00-sources/drop",
];

const ROOT_NAMES: [&str; 3] = ["hot", "index", "log"];

const QUERIES: [&str; 14] = [
    "garten",
    "kaffee wasserfall",
    "Gärten Beeten",
    "der die das",
    "bergwanderung",
    "entscheidung zu",
    "nachfolger",
    "zeb+rawort",
    "übersicht",
    "notiz",
    "xyzzy",
    "[",
    "projekt planung",
    "titelbonus quelle",
];

/// Appends a line to the text.
fn push(text: &mut String, line: &str) {
    text.push_str(line);
}

fn sentence(rng: &mut Rng, count: usize) -> String {
    (0..count)
        .map(|_| *rng.pick(&WORDS))
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_of(rng: &mut Rng) -> String {
    let mut chars = rng.pick(&WORDS).chars();
    let first: String = chars
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    format!("{first}{}", chars.as_str())
}

fn note_text(rng: &mut Rng) -> String {
    let mut text = String::new();
    if rng.chance(55) {
        text.push_str("---\n");
        if rng.chance(70) {
            push(&mut text, &format!("title: {}\n", title_of(rng)));
        }
        if rng.chance(15) {
            text.push_str("class: derived\n");
        }
        if rng.chance(12) {
            push(
                &mut text,
                &format!("superseded-by: [[{}]]\n", title_of(rng)),
            );
        }
        if rng.chance(8) {
            text.push_str("status: archived\n");
        }
        if rng.chance(10) {
            push(
                &mut text,
                &format!("valid-until: 2026-{:02}\n", 1 + rng.below(12)),
            );
        }
        if rng.chance(15) {
            text.push_str("quelle: x\n");
        }
        if rng.chance(15) {
            push(
                &mut text,
                &format!("aliases: [{}, {}]\n", title_of(rng), title_of(rng)),
            );
        }
        text.push_str("---\n");
    }
    for _ in 0..=rng.below(6) {
        let count = 2 + rng.below(10);
        text.push_str(&sentence(rng, count));
        if rng.chance(8) {
            push(&mut text, &format!(" [[{}]]", title_of(rng)));
        }
        text.push('\n');
    }
    text
}

/// Every file below `root`, as paths relative to it.
fn files_below(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                out.push(
                    path.strip_prefix(base)
                        .expect("below the base")
                        .to_path_buf(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn dirs_below(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.is_symlink() {
                out.push(
                    path.strip_prefix(base)
                        .expect("below the base")
                        .to_path_buf(),
                );
                walk(&path, base, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn notes_below(root: &Path) -> Vec<PathBuf> {
    files_below(root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect()
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("has a parent")).expect("parent is created");
    let _ = fs::write(path, text);
}

/// Writes `text` and sets the modification time back to what it was.
fn write_keep_mtime(path: &Path, text: &str) {
    let Ok(old) = fs::metadata(path).and_then(|meta| meta.modified()) else {
        write(path, text);
        return;
    };
    if fs::write(path, text).is_ok()
        && let Ok(file) = OpenOptions::new().write(true).open(path)
    {
        let _ = file.set_times(FileTimes::new().set_modified(old));
    }
}

/// The text with one letter replaced by another letter of the same width.
fn same_size(rng: &mut Rng, text: &str) -> String {
    let letters: Vec<(usize, char)> = text
        .char_indices()
        .filter(|(_, c)| c.is_ascii_lowercase())
        .collect();
    if letters.is_empty() {
        return text.to_string();
    }
    let (at, old) = letters[rng.below(letters.len())];
    let new = if old == 'e' { 'a' } else { 'e' };
    let mut changed = text.to_string();
    changed.replace_range(at..=at, &new.to_string());
    changed
}

struct World {
    root: PathBuf,
    aux: PathBuf,
    locked: Vec<(PathBuf, u32)>,
    can_lock: bool,
    log: Vec<String>,
}

impl World {
    fn new(scratch: &Path) -> Self {
        let root = scratch.join("vault");
        let aux = scratch.join("aux");
        fs::create_dir_all(&root).expect("vault is created");
        fs::create_dir_all(&aux).expect("aux is created");
        let probe = scratch.join("probe.md");
        fs::write(&probe, "x").expect("probe is written");
        fs::set_permissions(&probe, fs::Permissions::from_mode(0o000)).expect("mode is set");
        let can_lock = fs::read(&probe).is_err();
        fs::set_permissions(&probe, fs::Permissions::from_mode(0o644)).expect("mode is reset");
        Self {
            root,
            aux,
            locked: Vec::new(),
            can_lock,
            log: Vec::new(),
        }
    }

    fn vector_path(&self) -> PathBuf {
        self.aux.join("vectors.json")
    }

    fn new_note_path(&self, rng: &mut Rng) -> PathBuf {
        let dir = *rng.pick(&DIRS);
        let name = if dir.is_empty() && rng.chance(30) {
            format!("{}.md", rng.pick(&ROOT_NAMES))
        } else {
            format!("n{}.md", rng.below(14))
        };
        self.root.join(dir).join(name)
    }

    fn unlock(&mut self) {
        for (path, mode) in self.locked.drain(..).rev() {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode));
        }
    }

    fn vector_file(&self, rng: &mut Rng) -> String {
        let dim = 3 + rng.below(3);
        let mut notes = notes_below(&self.root);
        notes.push(PathBuf::from("90-secrets/relikt.md"));
        notes.push(PathBuf::from("fehlt.md"));
        let mut entries: Vec<String> = Vec::new();
        for rel in &notes {
            if !rng.chance(70) {
                continue;
            }
            let numbers: Vec<String> = (0..dim).map(|_| format!("{:.3}", unit(rng))).collect();
            entries.push(format!(
                "\"{}\": [{}]",
                rel.to_string_lossy().replace('\\', "/"),
                numbers.join(", ")
            ));
        }
        format!("{{\"notes\": {{{}}}}}", entries.join(", "))
    }

    /// One random change of the vault, the manifests or the vector file.
    #[allow(clippy::too_many_lines)]
    // One flat table of the kinds of change; splitting it would only hide the probabilities.
    fn mutate(&mut self, rng: &mut Rng) {
        if !self.locked.is_empty() && rng.chance(50) {
            self.unlock();
            self.log.push("unlock".into());
            return;
        }
        let notes = notes_below(&self.root);
        let roll = rng.below(100);
        let pick_note = |rng: &mut Rng| -> Option<PathBuf> {
            (!notes.is_empty()).then(|| self.root.join(rng.pick(&notes)))
        };
        let entry = match roll {
            0..=17 => {
                let path = self.new_note_path(rng);
                write(&path, &note_text(rng));
                format!("write {}", path.display())
            }
            18..=31 => match pick_note(rng) {
                Some(path) => {
                    write(&path, &note_text(rng));
                    format!("rewrite {}", path.display())
                }
                None => "nothing".into(),
            },
            32..=43 => match pick_note(rng) {
                Some(path) => {
                    let old = fs::read_to_string(&path).unwrap_or_default();
                    let new = same_size(rng, &old);
                    write_keep_mtime(&path, &new);
                    format!("same size, old mtime {}", path.display())
                }
                None => "nothing".into(),
            },
            44..=50 => match pick_note(rng) {
                Some(path) => {
                    let _ = fs::remove_file(&path);
                    format!("delete {}", path.display())
                }
                None => "nothing".into(),
            },
            51..=58 => match pick_note(rng) {
                Some(path) => {
                    let target = self.new_note_path(rng);
                    let _ = fs::create_dir_all(target.parent().expect("has a parent"));
                    let _ = fs::rename(&path, &target);
                    format!("rename {} {}", path.display(), target.display())
                }
                None => "nothing".into(),
            },
            59..=62 => {
                let dir = self
                    .root
                    .join(rng.pick(&DIRS))
                    .join(format!("d{}", rng.below(3)));
                let _ = fs::create_dir_all(&dir);
                write(&dir.join("neu.md"), &note_text(rng));
                format!("new folder {}", dir.display())
            }
            63..=65 => {
                let dirs = dirs_below(&self.root);
                if dirs.is_empty() {
                    "nothing".into()
                } else {
                    let from = self.root.join(rng.pick(&dirs));
                    let to = self
                        .root
                        .join(rng.pick(&DIRS))
                        .join(format!("r{}", rng.below(3)));
                    let _ = fs::create_dir_all(to.parent().expect("has a parent"));
                    let _ = fs::rename(&from, &to);
                    format!("rename folder {} {}", from.display(), to.display())
                }
            }
            66..=67 => {
                let dirs = dirs_below(&self.root);
                if dirs.is_empty() {
                    "nothing".into()
                } else {
                    let dir = self.root.join(rng.pick(&dirs));
                    let _ = fs::remove_dir_all(&dir);
                    format!("remove folder {}", dir.display())
                }
            }
            68..=73 => {
                let dir = self.root.join(*rng.pick(&["q", "a", "p", ""]));
                let path = dir.join(".space.yml");
                match rng.below(4) {
                    0 => {
                        let _ = fs::remove_file(&path);
                    }
                    1 => write(&path, "{\"art\": \"normal\"}"),
                    2 => write(&path, "kein json"),
                    _ => write(&path, "{\"art\": \"quellen\"}"),
                }
                format!(".space.yml {}", path.display())
            }
            74..=77 => {
                let path = self.root.join(".brain/spaces.lock");
                if rng.chance(25) {
                    let _ = fs::remove_file(&path);
                } else {
                    write(
                        &path,
                        &format!(
                            "{{\"spaces\": [{{\"space\": \"pp\", \"mount\": \"{}\"}}, {{\"space\": \"qq\", \"mount\": \"q\"}}]}}",
                            rng.pick(&["p", "a", "a/sub", "b"])
                        ),
                    );
                }
                "spaces.lock".into()
            }
            78..=81 => {
                let path = self.root.join("_meta/state/contradictions.json");
                if notes.is_empty() || rng.chance(25) {
                    let _ = fs::remove_file(&path);
                } else {
                    let a = rng.pick(&notes).to_string_lossy().replace('\\', "/");
                    let b = rng.pick(&notes).to_string_lossy().replace('\\', "/");
                    write(
                        &path,
                        &format!(
                            "{{\"findings\": {{\"1\": {{\"status\": \"open\", \"note_a\": {{\"rel\": \"{a}\"}}, \"note_b\": {{\"rel\": \"{b}\"}}}}}}}}"
                        ),
                    );
                }
                "contradictions".into()
            }
            82..=91 => {
                let path = self.vector_path();
                match rng.below(5) {
                    0 => {
                        let _ = fs::remove_file(&path);
                    }
                    1 => write(&path, "{\"notes\": [1, 2"),
                    2 => {
                        let old = fs::read_to_string(&path).unwrap_or_default();
                        let new = same_size(rng, &old);
                        write_keep_mtime(&path, &new);
                    }
                    _ => {
                        let text = self.vector_file(rng);
                        write(&path, &text);
                    }
                }
                "vectors".into()
            }
            92..=96 => {
                if self.can_lock {
                    let target = if rng.chance(50) {
                        pick_note(rng)
                    } else {
                        let dirs = dirs_below(&self.root);
                        (!dirs.is_empty()).then(|| self.root.join(rng.pick(&dirs)))
                    };
                    if let Some(path) = target
                        && let Ok(meta) = fs::metadata(&path)
                    {
                        let mode = meta.permissions().mode();
                        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o000));
                        self.locked.push((path.clone(), mode));
                        format!("chmod 000 {}", path.display())
                    } else {
                        "nothing".into()
                    }
                } else {
                    "nothing".into()
                }
            }
            _ => "no change".into(),
        };
        self.log.push(entry);
    }
}

fn random_request(rng: &mut Rng) -> SearchRequest {
    SearchRequest {
        query: (*rng.pick(&QUERIES)).to_string(),
        k: *rng.pick(&[1, 3, 5, 5, 8]),
        space: (*rng.pick(&[
            None,
            None,
            Some("kern"),
            Some("pp"),
            Some("pp,kern"),
            Some("qq"),
            Some(""),
        ]))
        .map(ToString::to_string),
        today: Some((*rng.pick(&["2026-11-18", "2027-03-01"])).to_string()),
        query_vector: rng.chance(50).then(|| {
            let dim = 3 + rng.below(3);
            (0..dim).map(|_| unit(rng)).collect()
        }),
    }
}

/// What `rangkern search` gives for the request on the files as they are now.
fn fresh(world: &World, request: &SearchRequest) -> Result<(Vec<SearchHit>, bool), ()> {
    let vectors = match &request.query_vector {
        Some(query) => {
            let bytes = fs::read(world.vector_path()).map_err(|_| ())?;
            let notes = parse_note_vectors(&bytes).map_err(|_| ())?;
            Some(Vectors {
                query: query.clone(),
                notes,
            })
        }
        None => None,
    };
    let params = Params {
        k: request.k,
        vectors: vectors.as_ref().map(Vectors::view),
        space: request.space.as_deref(),
        today: request.today.as_deref().expect("the test gives a date"),
    };
    search(&world.root, &request.query, &params).map_err(|_| ())
}

/// Runs `steps` random changes; after each, two random questions go to server and fresh search.
fn run(name: &str, seed: u64, steps: usize, detect: Detect, settled: bool) {
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("differenz-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("scratch is created");
    let mut world = World::new(&scratch);
    let mut rng = Rng(seed);
    for _ in 0..12 {
        let path = world.new_note_path(&mut rng);
        write(&path, &note_text(&mut rng));
    }
    let mut server = Server::start_with(world.root.clone(), Some(world.vector_path()), detect);
    let mut compared = 0_usize;
    for step in 0..steps {
        if settled {
            sleep(Duration::from_millis(12));
        }
        world.mutate(&mut rng);
        for _ in 0..2 {
            let request = random_request(&mut rng);
            let now = if settled {
                now_ns() + 10_000_000_000
            } else {
                now_ns()
            };
            let warm = server.search_hits(&request, now).map_err(|_| ());
            let cold = fresh(&world, &request);
            if warm != cold {
                world.unlock();
                let report = format!(
                    "seed {seed}, step {step}, request {request:?}\nlast changes: {:#?}\nserver: {warm:#?}\nsearch: {cold:#?}",
                    &world.log[world.log.len().saturating_sub(6)..]
                );
                let _ = fs::remove_dir_all(&scratch);
                panic!("{report}");
            }
            compared += 1;
        }
    }
    world.unlock();
    fs::remove_dir_all(&scratch).expect("scratch is removed");
    assert_eq!(compared, steps * 2);
}

#[test]
fn server_equals_search_after_random_changes_1() {
    run("r1", 0x9E37_79B9_7F4A_7C15, 450, Detect::Metadata, false);
}

#[test]
fn server_equals_search_after_random_changes_2() {
    run("r2", 0xD1B5_4A32_D192_ED03, 450, Detect::Metadata, false);
}

#[test]
fn server_equals_search_after_random_changes_3() {
    run("r3", 0x8CB9_2BA7_2F3D_8DD7, 450, Detect::Metadata, false);
}

#[test]
fn server_equals_search_after_random_changes_4() {
    run("r4", 0x1234_5678_9ABC_DEF1, 450, Detect::Metadata, false);
}

#[test]
fn server_equals_search_with_every_stamp_trusted() {
    run(
        "settled",
        0xA5A5_1234_FEED_BEEF,
        300,
        Detect::Metadata,
        true,
    );
}

#[test]
fn server_equals_search_when_changes_are_found_by_content() {
    run(
        "content",
        0x0BAD_CAFE_1357_9BDF,
        450,
        Detect::Content,
        false,
    );
}

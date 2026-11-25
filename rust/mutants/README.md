# Exclusions of the mutation gate

`./gates.sh mutants` reads every file `mutants/<crate>.liste` of this folder. There is one file per crate, named after
its folder below `crates/`. The lists stood in `gates.sh` until 25.11.2026; three branches changed them on one day and
met in that file.

## Format

One entry per line:

```
<list> <kind> <pattern>
```

- `<list>` is one of three lists of the gate:
  - `erster`: left out of the first run only, which builds without the feature `pruefbau`. This is for code that stands
    only behind `cfg(feature = "pruefbau")` and code that is equivalent in the delivered build. The second run, with the
    feature, keeps these mutants.
  - `gleichwertig`: equivalent mutants, left out of both runs.
  - `testhilfe`: test help that is never delivered, left out of every run.
- `<kind>` is `re` (passed as `--exclude-re`, a regular expression on the name of the mutant) or `datei` (passed as
  `--exclude`, a glob on the file).
- `<pattern>` is the rest of the line, spaces included, without quotes. It starts with `crates/<crate>/` of its own
  file (or `^crates/<crate>/`).

A line that starts with `#` is a comment; empty lines are skipped. Each entry has its reason in the comment lines above
it.

## Rules

- The gate is red, and names the file and the line, for a file whose name is no crate folder, an unknown list or kind,
  an empty pattern, a pattern of another crate, or a carriage return.
- A pattern with a line number that no longer fits leaves its mutant in the run. The run then reports it as missed and
  fails closed; the entry is then moved to the new line.
- A changed file in this folder selects the crate of its name for `WB_MUTANTS_BASIS`, in full.
- The exclusions of `.cargo/mutants.toml` (code behind `cfg(windows)` and `rangkern serve`) stay there: cargo-mutants
  reads that file itself.

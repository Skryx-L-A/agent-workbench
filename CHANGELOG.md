# Changelog

This file lists the changes of each published state, newest first. Both branches are snapshots
of a private source tree, so a state is one commit on `main` or `development`; the heading names
the branch and the date it was published. Changes inside a state are grouped as Added, Changed,
Fixed and Removed.

Diese Datei nennt die Änderungen jedes veröffentlichten Stands, den neuesten zuerst. Beide Zweige
sind Schnappschüsse eines privaten Quellbaums; ein Stand ist ein Commit auf `main` oder
`development`, und die Überschrift nennt Zweig und Veröffentlichungsdatum.

## Unreleased

### Changed

- License: the Business Source License 1.1 with an additional use grant now covers both branches.
  Use, also in companies and in production, passing it on to customers and paid services around
  it are free; only offering it to more than one customer under your own product name, or built
  into a product under another name, needs a partner agreement. Each version
  also becomes available under the Apache License 2.0 two years after it was first published. See
  [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md).
- Branches: `main` carries the tested workbench and is updated only after a full test run of
  the source tree; `development` carries the product, the Rust rebuild in `rust/`, and is updated
  after the Rust gates.
- Contributions: pull requests go against `development` and are taken over into the source tree
  as a patch that names the contributor. The Contributor License Agreement applies.

### Added

- Community files: contributing guide, code of conduct, security policy, support page, issue
  forms for bugs, features and questions, pull request template, and Discussions.
- Continuous integration and CodeQL analysis on pushes and pull requests to `main`, and the
  OpenSSF Scorecard on `main`. All actions are pinned to a full commit hash.

## Earlier states

- Up to and including the commit tagged `last-agpl`: GNU Affero General Public License v3.0 only.
- Later states up to and including the commit tagged `last-polyform`: PolyForm Noncommercial
  License 1.0.0 with the additional permissions published with them.

Earlier states keep the license under which they were published.

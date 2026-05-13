# Final Product Roadmap

This roadmap turns the AGENTS.md final product image into implementation
milestones. Each milestone must end with formatting, linting, tests,
compatibility checks where relevant, documentation updates, and a commit.

For day-to-day status, active queues, and completion checklists, update
`docs/milestones.md` alongside code changes.

## M0: Baseline and Rules

- Keep `git --version`, `git help -a`, and command help notes current.
- Maintain `docs/compatibility.md` and `docs/implementation-notes.md`.
- Keep quality gates fixed: `cargo fmt --all`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo test --workspace`.

## M1: Compatibility Test Harness

- Provide `rit-testkit` as both a library and CLI.
- Compare Git and rit command outputs and repository snapshots.
- Grow fixtures from simple read-only commands to mutating repository commands.

## M2: Core Repository Model

- Stabilize repository discovery, `Repository::open`, bare repositories,
  common-dir handling, and repository format guards.
- Reject unsupported repository format versions before writes.
- Keep public API documentation clear and beginner-readable.

## M3-M8: Local Git Engine

- Complete object database support, refs, revision parsing, index/worktree,
  read-only CLI, local write CLI, and diff.
- Prefer readable code over clever optimizations.
- Use Git comparison tests for every externally visible command behavior.

## M9-M13: Collaboration and Large Repository Features

- Implement merge/cherry-pick/rebase/stash state handling.
- Add local, HTTP, and SSH transport with auth separated from transport.
- Add LFS, Xet, sparse checkout, partial clone, and workspace profiles behind
  feature gates where appropriate.
- Add optional SQLite `indexdb` as a reproducible acceleration layer, never as
  the source of truth.

## M14-M18: rit Differentiators and Release

- Add semantic diff, policy engine, VFS, doctor/repair, and release packaging.
- Provide `rit-min` and `rit-full` build profiles.
- Finish README, module docs, compatibility docs, and release notes.

## M16-M25: Product Differentiators

- Add Operation Journal and Universal Undo on top of safe `.git/rit/`
  metadata.
- Add Transaction Plan / Dry-run APIs for every write command.
- Expand Explainable Git to ignore, pathspec, merge, auth, LFS/Xet, and
  workspace decisions.
- Add Smartlog / Local Work Graph, Doctor fix plans, workspace recommendation,
  impact analysis, stable JSON schema commands, compatibility oracle, and
  large-file audit/migration planning.

## Commit Discipline

Each milestone should be split into small commits:

- compatibility baseline or fixture
- core behavior
- CLI behavior
- tests
- docs

No commit should mix unrelated feature work with mechanical formatting churn.

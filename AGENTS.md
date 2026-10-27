---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

# cron-person-cli

Native Rust command-line tool for approving per-directory `.cronrc` files and installing guarded system cron entries.

## Development commands

- Rust 1.89+ is required. Development build: `just build` writes `bin/cron-person`.
- Run: `just run <args>`; compact mode: `just run-ai <args>`.
- Unit and integration tests: `just test`.
- Full gate: `just check` includes formatting, strict Clippy, tests, and at least 90% line coverage. Install cargo-llvm-cov and llvm-tools for coverage.
- Format with `just fmt`, scoped to this package; sibling cli-command-kit is an independent repository.
- Rust artifacts are unpublished. Existing Go release archives are not Rust binaries. The compiled canonical skill URL is a discovery pointer rather than evidence of a published Rust skill or release.

## Authorization and process contracts

- Keep SHA-256 over exact file bytes, including the final newline. Read approved bytes once and derive the execution environment from that owned snapshot.
- Reload state inside each native file-lock transaction. Atomic replacement and file/directory synchronization protect committed state; do not retain an independent mutable cache.
- Preserve opaque shell commands through cron's percent transformation and outer-shell quoting. The authorization gate must run before shell redirection, pipelines, substitution, or other command effects.
- Generated crontab entries and hooks embed the actual executable and selected store paths. These executable machine artifacts require absolute runtime paths; fold paths only in displayed messages and diagnostics.
- Preserve inherited standard I/O and environment. Child shells own a process group for cancellation. Foreground terminal ownership is handed to an interactive child and restored afterward; block SIGTTOU only around synchronous terminal operations on the current thread.
- Recognize managed markers only as complete lines. Preserve unmanaged crontab bytes and reject malformed or duplicate managed blocks.
- Serialize sync and uninstall through the same per-user lock regardless of custom stores. Recheck the system crontab before installation; the system provider has no atomic compare-and-swap primitive, so concurrent external writers remain a documented limitation.

## Tests and documentation

- Tests use real subprocesses, Bash, Zsh, native PTYs, and isolated fake crontab executables inside `.tmp`. Never use a user's live crontab as a fixture. Fish runtime and cron-daemon firing need separate verification.
- Preserve compact AGENT output, raw version bytes, generated help alerts, native completion generation, and once-only runtime errors. Keep argument-only validation in Clap.
- Maintain skills/cron-person and references with command, argument, flag, default, environment, output, or side-effect changes. Test the text against the live command tree and update last_modified.
- Runtime changes require corresponding behavioral test changes and the coverage gate. Temporarily disable the implementation, verify a genuine test failure, restore it exactly, and rerun checks.
- Keep task progress and evidence outside these standing instructions. No commits, remotes, publishing, or live external writes are part of the migration task.

## Provenance

The source project is alexgorbatchev/cron-person-cli (MIT). Croner provides native schedule validation, shell-quote supplies shell quoting, std File locks provide native synchronization, and cli-command-kit provides shared terminal formatting/help. Preserve their native APIs rather than recreating compatibility facades.

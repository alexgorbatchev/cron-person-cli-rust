---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## Directory approval

`cron-person dir allow [path]` and `cron-person allow [path]` validate and approve exact `.cronrc` bytes. Optional path defaults to cwd. Reapproval replaces the hash and approval time. Agent output is `dir`, `status: allowed`, and `hash`.

`cron-person dir deny [path]` and `cron-person deny [path]` revoke the selected directory, including an already-untracked directory. Optional path defaults to cwd. Agent output is `dir` and `status: denied`. Existing guarded cron entries refuse further execution without requiring sync first.

`cron-person dir check [path]` validates syntax without state changes or authorization requirements. Optional path defaults to cwd. Agent output is `path`, `tasks_count`, and exact-byte `hash`.

`cron-person dir status [path]` inspects the optional path, default cwd. Agent output is `dir`, `status`, and stored `hash` when tracked. Status is `allowed`, `changed`, `unauthorized`, `missing_cronrc`, or `orphan` (tracked file removed). Human output also includes approval time and parsed task count when available.

`cron-person dir list` accepts no positional arguments and lists tracked records in sorted directory order. Agent rows are tab-separated `dir`, `status`, and `hash`; an empty store prints no rows. Human output shows a message for an empty store.

`cron-person status` accepts no positional arguments and prints `version`, `tracked_dirs`, `allowed_dirs`, and `total_tasks` in agent mode. Only currently approved readable files contribute active tasks; skipped files emit warnings on stderr.

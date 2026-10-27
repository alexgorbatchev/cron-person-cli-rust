---
name: cron-person
description: Use when operating cron-person to approve .cronrc files, inspect tasks, manage system cron entries, or configure shell hooks.
author: alexgorbatchev
metadata:
  created_on: 2026-10-14 17:33
  last_modified: 2026-10-14 17:33
  status: current
---

Use `AGENT=1 cron-person` for compact output. Inspect the exact `.cronrc` commands and environment, validate the file, and approve only the intended contents before installing or executing tasks.

Read [setup](references/setup.md) for global flags, state, schedules, environment, and exits; [directories](references/directories.md) for approval and status; [tasks](references/tasks.md) for listing and execution; [crontab](references/crontab.md) for installation and removal; [hooks](references/hooks.md) for prompt integration; [completion](references/completion.md) for shell scripts; and [help](references/help.md) for every generated help query.

## Workflow

1. Inspect `.cronrc`, then run `AGENT=1 cron-person dir check <path>`.
2. Run `AGENT=1 cron-person dir allow <path>` to approve the current bytes.
3. Run `AGENT=1 cron-person task list` and `cron-person crontab show` to inspect active jobs and generated entries.
4. Run `AGENT=1 cron-person crontab sync` when installation is intended.
5. Use `cron-person dir deny <path>` to revoke approval immediately. Use `cron-person crontab uninstall` to remove the managed block. Reapprove intentional file edits before synchronizing again.

## Metadata and help

- `cron-person` prints root help. `cron-person dir`, `cron-person task`, `cron-person crontab`, `cron-person hook`, and `cron-person completion` print group help without changing state.
- `cron-person skill` accepts no arguments or command-specific flags and prints the canonical Skills CLI install command and GitHub skill URL verbatim in both modes. Rust artifacts and the local Rust skill are unpublished; this URL is a discovery pointer.
- `cron-person --version` prints `0.1.0` followed by one newline.
- `cron-person help [command]...` prints a selected help screen. Every command accepts `--help` / `-h`. Agent help starts with the instruction to read `AGENT=1 cron-person skill`.

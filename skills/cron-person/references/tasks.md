---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## Tasks

`cron-person task list` accepts no positional arguments and lists tasks from currently approved files, sorted by directory then file order. Agent rows are tab-separated `dir`, `schedule`, one-based source `line`, and `cmd`. An empty list prints no agent rows. Skipped changed or unreadable files emit warnings on stderr.

`cron-person exec <dir> [--] <command>...` and `cron-person task run <dir> [--] <command>...` require an approved directory and one or more command strings. Multiple strings are joined with single spaces; pass the entire opaque shell command as one quoted argument to preserve its original argument boundaries. `--` separates command input from CLI options.

Execution checks approval before shell effects, runs in dir, inherits standard streams/environment, and applies `.cronrc` assignments plus CRON_PERSON_DIR and CRON_PERSON_CRONRC. The file's nonempty SHELL selects the interpreter; otherwise inherited SHELL or the system shell is used. Child failure returns CLI exit 1. SIGINT/SIGTERM cancellation received by the wrapper terminates its child shell process group and returns 130. Interactive stdin uses native foreground terminal handoff and restoration.

```sh
cron-person exec ./project -- 'printf "%s\n" "$VALUE" | sort > result.txt'
```

The command need not be listed as a scheduled task: authorization approves the directory's `.cronrc` environment and permits arbitrary explicitly requested commands in that directory.

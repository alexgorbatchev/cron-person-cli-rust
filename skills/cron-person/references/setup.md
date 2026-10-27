---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## Globals and runtime

- `--store <path>` is a global path flag; default empty selects `$XDG_STATE_HOME/cron-person/allowed.json` when XDG_STATE_HOME is absolute, otherwise `~/.local/state/cron-person/allowed.json`. Relative explicit paths resolve against cwd. Paths are used literally; shell expansion supplies `~` in arguments.
- `--version` / `-v` is a bool flag, default false; it prints only the raw version and newline.
- `--help` / `-h` is a bool help flag, default false, on every command.
- AGENT values `1`, `true`, or `yes` select compact output. Human output is the default. Displayed paths fold the home prefix; generated executable scripts retain absolute paths.
- HOME supplies the home fallback. PATH resolves the `crontab` program and named shell executables. System crontab installation requires a Unix cron provider; local inspection and approval do not invoke it.
- Authorization JSON contains records with `dir`, `path`, `hash`, and UTC RFC3339 `allowed_at`. SHA-256 covers exact bytes. State writes lock, reload, and atomically replace the file; execution uses one approved owned snapshot.
- Exit 0 is success; 1 is an argument, authorization, child, provider, or runtime error; 6 is committed authorization state with failed durability synchronization; 130 is cancellation. Runtime errors print once on stderr without usage. Argument errors print parser guidance.

## File grammar

Use UTF-8 `.cronrc` text with blank lines, full-line `#` comments, environment assignments, and cron tasks. Assignments accept `KEY=value` or spaces around `=`; matching outer single or double quotes are removed, with no variable expansion. Keys are ASCII identifiers; later assignments win and apply to all tasks.

Use five fields (minute, hour, day of month, month, weekday), followed by a nonempty shell command. Fields support numeric ranges, lists, steps, wildcards, month names JAN–DEC and weekday names SUN–SAT. Sunday can be 0 or 7. Standard descriptors are `@hourly`, `@daily`, `@midnight`, `@weekly`, `@monthly`, `@yearly`, `@annually`, and `@reboot`. Use five-field schedules for intervals; `@every`, seconds, years, and Quartz extensions are unsupported by system cron.

Command text retains internal quoted whitespace and shell operators. An inline `#` belongs to the shell command. Commands with percent characters are quoted by generated entries so cron receives them literally rather than treating them as stdin delimiters.

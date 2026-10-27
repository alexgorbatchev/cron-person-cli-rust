---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## System crontab

`cron-person crontab show` accepts no positional arguments and prints a complete generated managed block to stdout without invoking crontab. It includes currently approved tasks; skipped files produce warnings on stderr. Each entry embeds the actual executable, selected --store, directory, and opaque shell command. Generated executable paths remain absolute in both output modes.

`cron-person crontab sync` accepts no positional arguments and reads the current user's crontab, preserves unmanaged bytes, and replaces or appends the exact managed block. The native provider is invoked as `crontab -l` and `crontab -` with input on stdin. Agent output is `synced_directories` and `status: success`; the count represents contributing directories, including approved empty task files.

`cron-person crontab uninstall` accepts no positional arguments and removes only the managed block. Agent output is `crontab: uninstalled`. Repeating sync or uninstall avoids an unchanged write. Malformed, nested, duplicate, or unmatched marker blocks fail rather than modifying unrelated text.

Sync and uninstall serialize per user even with distinct stores. They reread before writing and fail on an observed external change. A concurrent external writer can still race final installation because the native crontab API lacks atomic compare-and-swap. A failed provider write may have changed system state; inspect the current crontab before retrying. Retain an independent backup for recovery.

The markers are complete lines `# >>> cron-person managed block (DO NOT EDIT DIRECTLY) >>>` and `# <<< cron-person managed block <<<`. Marker-like substrings in personal commands remain untouched. Schedule firing and platform cron policy belong to the installed daemon.

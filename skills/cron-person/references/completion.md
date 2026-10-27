---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## Shell completion

`cron-person completion bash`, `cron-person completion fish`, `cron-person completion powershell`, and `cron-person completion zsh` accept no positional arguments and print the native shell script to stdout without state access.

Each accepts `--no-descriptions`, a bool flag with default false, to omit completion descriptions. Scripts retain runtime path text and shell syntax in both modes.

Save the selected output and load it through that shell's completion mechanism, for example `cron-person completion zsh > _cron-person`. `cron-person completion` prints group help.

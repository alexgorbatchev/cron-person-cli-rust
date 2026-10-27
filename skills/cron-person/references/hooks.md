---
created_on: 2026-10-14 17:33
last_modified: 2026-10-14 17:33
status: current
---

## Prompt integration

`cron-person hook <shell>` accepts shell `bash`, `zsh`, or `fish` and prints its integration script to stdout. With no shell, it prints group help. It embeds the actual executable and chosen store. Load the output in the selected shell when prompt integration is intended, for example `eval "$(cron-person hook bash)"` in Bash.

Bash registers a prompt callback while preserving string or array PROMPT_COMMAND. Zsh registers precmd_functions and chpwd_functions. Both register once, preserve existing callbacks and signal traps, and return the previous exit status. Fish registers a fish_prompt event function. The generated script invokes the checker; it supplies warnings rather than adding environment variables to the interactive shell.

`cron-person hook export [shell]` is the hidden checker used by hooks. Optional shell accepts `bash`, `zsh`, or `fish`; the check is the same for each. It discovers the nearest ancestor `.cronrc` from cwd. Stdout is empty. Unauthorized or modified files emit a warning on stderr, with compact WARN output in agent mode. Allowed files and directories with no ancestor file are silent. File/state access failures are returned errors.

Runtime Bash and Zsh behavior has local subprocess coverage. Fish script generation is covered; Fish runtime remains unverified.

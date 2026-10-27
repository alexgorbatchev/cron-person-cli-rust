`cron-person` runs scheduled commands from per-directory `.cronrc` files after you approve their exact contents. It manages a guarded block in your system crontab and warns when an approved file changes.

# What It Does

- **Approve files:** Bind authorization to the SHA-256 of the complete `.cronrc` bytes.
- **Inspect tasks:** Validate cron syntax, list approved commands, and report changed or missing files.
- **Manage cron:** Generate, synchronize, and remove a managed block while retaining personal entries.
- **Run commands:** Check approval before launching a shell command with the file's environment and working directory.
- **Integrate shells:** Generate Bash, Zsh, and Fish prompt hooks plus four shell completion formats.

# How It Works

- Create a `.cronrc` in the task's working directory.
- Validate it with `cron-person dir check` and approve it with `cron-person dir allow`.
- Preview `cron-person crontab show`, then install it with `cron-person crontab sync`.
- Approve the file again after intentional edits. Changed or revoked files cannot launch commands through the generated entries.

# How it Really Works

- Authorization lives at `$XDG_STATE_HOME/cron-person/allowed.json`, or `~/.local/state/cron-person/allowed.json`. Relative XDG_STATE_HOME values use the home fallback. `--store` selects another JSON file.
- State changes use a native file lock, reload the current JSON, and atomically replace it. Approval includes the final newline; execution uses one owned approved snapshot.
- `crontab show` writes a script to stdout; `sync` and `uninstall` invoke the installed `crontab` executable. Both writers share a per-user lock and recheck the current content before installation. An external writer can still race the final installation because system crontab has no atomic compare-and-swap API.
- Generated commands retain custom store paths and quote the whole shell command before cron percent processing. Absolute paths in executable scripts remain usable; displayed paths use `~`.
- Results go to stdout, warnings and one returned error go to stderr. `AGENT=1`, `true`, or `yes` selects compact output. Shell hooks keep stdout empty while reporting changed or unauthorized files on stderr.
- The file's nonempty SHELL selects the interpreter, otherwise the inherited SHELL or the system shell is used. Commands inherit standard I/O; terminal foreground ownership is restored afterward. Cancellation terminates the shell process group.
- Exit 0 means success, 1 an argument, authorization, child, or runtime failure, 6 a committed state change whose durability synchronization failed, and 130 cancellation.
- Supported schedules have five fields or standard descriptors including `@reboot`. `@every` is rejected because system crontab does not implement it. Quoted command whitespace remains intact; environment assignments accept spaces around `=`.

# Prerequisites

A Unix-like system with a [system cron implementation](https://man7.org/linux/man-pages/man5/crontab.5.html) and `crontab` on PATH is needed for installation. The selected shell must be executable. Bash, Zsh, or Fish is needed only for its respective prompt hook. Fish runtime, macOS runtime, and actual daemon firing remain unverified by local tests.

# Installation

Rust binaries are unpublished. Existing Go release archives do not contain this Rust implementation. Development builds are documented in [AGENTS.md](AGENTS.md).

# Quick Start

Create `.cronrc` with a task such as:

```cron
SHELL=/bin/sh
0 9 * * MON-FRI printf 'Daily task\n'
```

```sh
cron-person dir check
cron-person dir allow
AGENT=1 cron-person task list
cron-person crontab show
cron-person crontab sync
```

Sample Output from `AGENT=1 cron-person status` with an empty store:

```text
version: 0.1.0
tracked_dirs: 0
allowed_dirs: 0
total_tasks: 0
```

# Options & Flags

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--store <path>` | | Empty | Authorization file; empty selects XDG state or home fallback |
| `--version` | `-v` | `false` | Print the raw version |
| `--help` | `-h` | `false` | Print selected command help |

`cron-person completion <bash|fish|powershell|zsh>`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--no-descriptions` | | `false` | Omit completion descriptions |

Read [the usage skill](skills/cron-person/SKILL.md) for command arguments, outputs, shell integration, and scheduling details. `cron-person skill` prints its canonical discovery/install pointer; the local Rust skill is unpublished.

# License

[MIT](LICENSE), copyright 2026 Alex Gorbatchev.

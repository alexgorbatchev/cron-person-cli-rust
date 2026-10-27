use cli_command_kit::{
    HelpOptions, Mode,
    clap::{
        Arg, ArgAction, Command,
        builder::{NonEmptyStringValueParser, OsStringValueParser, TypedValueParser},
    },
};
use std::path::PathBuf;
pub const SKILL_POINTER: &str = "Install the cron-person skill using the Skills CLI:\n\n  npx skills add alexgorbatchev/cron-person-cli --skill cron-person\n\nSkill:\n\n  https://github.com/alexgorbatchev/cron-person-cli/tree/main/skills/cron-person\n";
pub fn help_options() -> HelpOptions {
    HelpOptions {
        skill: Some(SKILL_POINTER.into()),
        tree: cli_command_kit::TreeOptions {
            terminal_width: Some(match cli_command_kit::terminal_width() {
                0 => 80,
                w => w,
            }),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn cmd(name: &'static str, help: &'static str) -> Command {
    Command::new(name).about(help)
}
fn path() -> Arg {
    Arg::new("path")
        .value_parser(cli_command_kit::clap::value_parser!(PathBuf))
        .help("Directory; defaults to current directory")
}
fn directory(name: &'static str, help: &'static str) -> Command {
    cmd(name, help).arg(path())
}
fn run(name: &'static str, help: &'static str) -> Command {
    cmd(name, help)
        .arg(
            Arg::new("dir")
                .required(true)
                .value_parser(cli_command_kit::clap::value_parser!(PathBuf))
                .help("Approved working directory"),
        )
        .arg(
            Arg::new("command")
                .required(true)
                .num_args(1..)
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
                .value_parser(NonEmptyStringValueParser::new())
                .help("Shell command; quote the complete string to preserve argument boundaries"),
        )
}
pub fn command(mode: Mode) -> Command {
    let root = cmd(
        "cron-person",
        "Authorize and schedule per-directory cron tasks",
    )
    .disable_version_flag(true)
    .version(env!("CARGO_PKG_VERSION"))
    .arg(
        Arg::new("version")
            .long("version")
            .short('v')
            .action(ArgAction::SetTrue)
            .help("Print the raw version"),
    )
    .arg(
        Arg::new("store")
            .long("store")
            .global(true)
            .value_parser(OsStringValueParser::new().map(PathBuf::from))
            .default_value("")
            .help("Authorization JSON path; empty selects XDG state or ~/.local/state"),
    )
    .subcommand(
        cmd("dir", "Manage .cronrc authorization")
            .subcommand(directory("allow", "Approve the current .cronrc bytes"))
            .subcommand(directory("deny", "Revoke directory authorization"))
            .subcommand(cmd("list", "List tracked directories and status"))
            .subcommand(directory(
                "check",
                "Validate .cronrc syntax without approving",
            ))
            .subcommand(directory(
                "status",
                "Inspect directory approval and task count",
            )),
    )
    .subcommand(directory("allow", "Approve the current .cronrc bytes"))
    .subcommand(directory("deny", "Revoke directory authorization"))
    .subcommand(cmd("status", "Show tracked and active task totals"))
    .subcommand(
        cmd("task", "Inspect or execute approved tasks")
            .subcommand(cmd("list", "List tasks from approved files"))
            .subcommand(run("run", "Run a shell command in an approved directory")),
    )
    .subcommand(run("exec", "Run a shell command in an approved directory"))
    .subcommand(
        cmd("crontab", "Manage the system crontab block")
            .subcommand(cmd(
                "show",
                "Print the generated managed block without installing",
            ))
            .subcommand(cmd("sync", "Install or replace the managed block"))
            .subcommand(cmd("uninstall", "Remove only the managed block")),
    )
    .subcommand(
        cmd("hook", "Print a shell integration script")
            .arg(
                Arg::new("shell")
                    .value_parser(["bash", "zsh", "fish"])
                    .help("Interactive shell"),
            )
            .subcommand(
                cmd("export", "Check the nearest ancestor .cronrc")
                    .hide(true)
                    .arg(
                        Arg::new("shell")
                            .value_parser(["bash", "zsh", "fish"])
                            .help("Calling shell; optional"),
                    ),
            ),
    )
    .subcommand(["bash", "fish", "powershell", "zsh"].into_iter().fold(
        cmd("completion", "Generate shell completions"),
        |c, s| {
            c.subcommand(
                cmd(s, "Print the shell completion script").arg(
                    Arg::new("no-descriptions")
                        .long("no-descriptions")
                        .action(ArgAction::SetTrue)
                        .help("Omit completion descriptions"),
                ),
            )
        },
    ));
    cli_command_kit::setup(root, mode, &help_options()).expect("valid cron-person command tree")
}
